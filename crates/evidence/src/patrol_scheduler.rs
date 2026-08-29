//! COLLECTION-001 · 巡检调度：把「到期了」变成一张有界的工单与租约。
//!
//! 骨架取自内容工作台在真实环境跑了数月的 `runDueMonitorScheduler`：翻页扫活跃目标 →
//! 逐个判到期 → 到期就派 → 返回「派了哪些、跳过哪些及原因」。**跳过的理由必须逐条留下**，
//! 那边的经验是：一个只报「本轮派了 3 个」的调度器，在没派的时候没人说得清为什么。
//!
//! 三条与那边不同的地方，都是有意的：
//!
//! 1. **不另建监控配置表**。内容工作台把配置单列一张表，是因为那边没有一等的观察目标；
//!    这里目标本身就是一等对象，再建一张只会让同一个博主有两个身份。
//! 2. **调度只做决策，不做执行**（规则文档）：这里不访问任何平台，只推进状态并入队。
//! 3. **调度不绕过授权链**。它做的事与人点一次按钮完全一样——申请、准入、工单、租约，
//!    一步不少。自动化不是豁免权：如果准入说资源不够，调度也只能等。

use crate::acquisition_chain::request_and_admit;
use crate::work_order_lease::issue_work_order_lease;
use linggan_contracts::AdmissionOutcome;
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一次 tick 的结果。**派了什么与没派什么同样重要。**
#[derive(Debug, Default)]
pub struct PatrolTickSummary {
    pub dispatched: Vec<Uuid>,
    /// 每个被跳过的目标，以及具体原因。
    pub skipped: Vec<(Uuid, String)>,
}

#[derive(Debug, Clone)]
pub struct SchedulerHeartbeat {
    pub state: &'static str,
    pub last_tick_completed_at: Option<String>,
    pub last_outcome: String,
    pub dispatched_count: i32,
    pub skipped_count: i32,
    pub last_error: Option<String>,
}

/// 租约时长：一次巡检该在多久内跑完。
///
/// 比巡检间隔短得多——租约是「这次允许你跑多久」，不是「下次什么时候再跑」。给得过长，
/// 一次卡住的执行会一直占着工单，直到下一轮都无法重派。
const PATROL_LEASE_MINUTES: i32 = 30;

/// 一次扫描最多处理多少个目标。分页是为了让 tick 保持轻量（规则文档：tick 只做轻量
/// 状态推进与入队，重活另开进程）。
const PATROL_PAGE_SIZE: i64 = 50;

/// A baseline that never reaches a Receipt must not generate work forever. This is the same
/// bounded execution rule the previous content workbench used; delivery replay is counted
/// elsewhere and does not consume one of these platform attempts.
const MAX_BASELINE_WORK_ORDERS: i64 = 3;

pub async fn patrol_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
                        WHERE table_name = 'collection_observation_target' \
                          AND column_name = 'monitoring_enabled')",
    )
    .fetch_one(database.pool())
    .await
}

pub async fn record_scheduler_started(
    database: &Database,
    worker_instance_ref: Uuid,
) -> Result<(), sqlx::Error> {
    if !heartbeat_schema_is_ready(database).await? {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO collection_scheduler_heartbeat \
             (scheduler_key,worker_instance_ref,worker_started_at) \
         VALUES ('patrol',$1,scope_001_now()) \
         ON CONFLICT (scheduler_key) DO UPDATE SET \
             worker_instance_ref=EXCLUDED.worker_instance_ref, \
             worker_started_at=EXCLUDED.worker_started_at",
    )
    .bind(worker_instance_ref)
    .execute(database.pool())
    .await?;
    Ok(())
}

pub async fn read_scheduler_heartbeat(
    database: &Database,
) -> Result<Option<SchedulerHeartbeat>, sqlx::Error> {
    if !heartbeat_schema_is_ready(database).await? {
        return Ok(None);
    }
    let row: Option<(bool, Option<String>, String, i32, i32, Option<String>)> = sqlx::query_as(
        "SELECT COALESCE(last_tick_completed_at >= scope_001_now() - interval '180 seconds',false), \
                to_char(last_tick_completed_at,'YYYY-MM-DD\"T\"HH24:MI:SSOF'),last_outcome, \
                dispatched_count,skipped_count,last_error \
         FROM collection_scheduler_heartbeat WHERE scheduler_key='patrol'",
    )
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| SchedulerHeartbeat {
        state: if row.0 { "running" } else { "stale" },
        last_tick_completed_at: row.1,
        last_outcome: row.2,
        dispatched_count: row.3,
        skipped_count: row.4,
        last_error: row.5,
    }))
}

async fn heartbeat_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT to_regclass('collection_scheduler_heartbeat') IS NOT NULL")
        .fetch_one(database.pool())
        .await
}

/// 跑一轮巡检调度。
///
/// 到期判据只看「上次**派出**的时间」，不看采集是否成功：采集失败也算看过了，否则一个
/// 持续失败的目标会被无限重试，把当天额度吃光。
pub async fn run_due_patrols(database: &Database) -> Result<PatrolTickSummary, sqlx::Error> {
    let heartbeat_ready = heartbeat_schema_is_ready(database).await.unwrap_or(false);
    if heartbeat_ready {
        let _ = sqlx::query(
            "INSERT INTO collection_scheduler_heartbeat \
                 (scheduler_key,last_tick_started_at,last_outcome,last_error) \
             VALUES ('patrol',scope_001_now(),'unknown',NULL) \
             ON CONFLICT (scheduler_key) DO UPDATE SET \
                 last_tick_started_at=EXCLUDED.last_tick_started_at,last_outcome='unknown',last_error=NULL",
        )
        .execute(database.pool())
        .await;
    }
    let result = run_due_patrols_inner(database).await;
    if heartbeat_ready {
        let (outcome, dispatched, skipped, error) = match &result {
            Ok(summary) if summary.dispatched.is_empty() && summary.skipped.is_empty() => {
                ("idle", 0, 0, None)
            }
            Ok(summary) if summary.skipped.is_empty() => (
                "dispatched",
                i32::try_from(summary.dispatched.len()).unwrap_or(i32::MAX),
                0,
                None,
            ),
            Ok(summary) => (
                "partial",
                i32::try_from(summary.dispatched.len()).unwrap_or(i32::MAX),
                i32::try_from(summary.skipped.len()).unwrap_or(i32::MAX),
                None,
            ),
            Err(error) => ("failed", 0, 0, Some(error.to_string())),
        };
        let _ = sqlx::query(
            "UPDATE collection_scheduler_heartbeat SET \
                 last_tick_completed_at=scope_001_now(),last_outcome=$1, \
                 dispatched_count=$2,skipped_count=$3,last_error=$4 \
             WHERE scheduler_key='patrol'",
        )
        .bind(outcome)
        .bind(dispatched)
        .bind(skipped)
        .bind(error)
        .execute(database.pool())
        .await;
    }
    result
}

async fn run_due_patrols_inner(database: &Database) -> Result<PatrolTickSummary, sqlx::Error> {
    if !patrol_schema_is_ready(database).await? {
        return Ok(PatrolTickSummary::default());
    }
    // Persist the expiry before deciding whether an archiving target needs a bounded recovery.
    // The old rows remain the history; a recovery creates a new Work Order and lease.
    sqlx::query(
        "UPDATE collection_work_order_lease SET released_at=expires_at,release_reason='expired' \
         WHERE released_at IS NULL AND expires_at <= scope_001_now()",
    )
    .execute(database.pool())
    .await?;
    let due: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT target_ref,lifecycle_state FROM collection_observation_target \
         WHERE monitoring_enabled \
           AND (lifecycle_state IN ('pending_decision','archiving') OR ( \
                 lifecycle_state='monitoring' AND (last_patrol_dispatched_at IS NULL \
                 OR last_patrol_dispatched_at + make_interval(secs => patrol_interval_seconds) \
                    <= scope_001_now()))) \
         ORDER BY last_patrol_dispatched_at NULLS FIRST \
         LIMIT $1",
    )
    .bind(PATROL_PAGE_SIZE)
    .fetch_all(database.pool())
    .await?;

    let mut summary = PatrolTickSummary::default();
    for (target_ref, lifecycle_state) in due {
        let lane = if lifecycle_state == "monitoring" {
            "patrol"
        } else {
            let attempts: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM collection_work_order \
                 WHERE target_ref=$1 AND lane='deep_archive'",
            )
            .bind(target_ref)
            .fetch_one(database.pool())
            .await?;
            if attempts >= MAX_BASELINE_WORK_ORDERS {
                summary.skipped.push((
                    target_ref,
                    format!("基线执行已达到 {MAX_BASELINE_WORK_ORDERS} 次上限"),
                ));
                continue;
            }
            "deep_archive"
        };
        match dispatch_one(database, target_ref, lane).await {
            Ok(Some(())) => summary.dispatched.push(target_ref),
            Ok(None) => {}
            Err(reason) => summary.skipped.push((target_ref, reason)),
        }
    }
    Ok(summary)
}

/// 为一个到期目标走完整条授权链。
///
/// **自动化不是豁免权**：它走的路与人点一次按钮完全一样。准入若说资源不够、风险暂停生效
/// 或额度触顶，调度也只能记下理由然后等——不会因为「是定时任务」就放行。
async fn dispatch_one(
    database: &Database,
    target_ref: Uuid,
    lane: &str,
) -> Result<Option<()>, String> {
    let purpose = if lane == "deep_archive" {
        "观察规则启用后的自动基线"
    } else {
        "定时巡检"
    };
    let outcome = // 申请人是 `agent` 而不是 `person`：调度器不是人。合同写着「Agent 可以提出需要，
    // 不能自行扩大观察面」——调度器提出巡检申请正是这个位置：它能申请，能不能跑仍由
    // 准入决定。记成 person 会让追责链指向一个当时并不在场的人。
    request_and_admit(database, target_ref, lane, purpose, "agent")
        .await
        .map_err(|error| error.to_string())?;

    let AdmissionOutcome::Admitted { .. } = outcome.outcome else {
        // 未获准入不是故障：资源不够、风险暂停中都是正常结论。理由原样留下。
        return Err(format!("准入未通过：{}", outcome.outcome.code()));
    };
    let Some(work_order_ref) = outcome.work_order_ref else {
        return Err("准入通过但没有工单".to_owned());
    };
    let lease_minutes = if lane == "deep_archive" {
        60
    } else {
        PATROL_LEASE_MINUTES
    };
    issue_work_order_lease(database, work_order_ref, lease_minutes)
        .await
        .map_err(|error| error.to_string())?;

    // 派出即记时。到期判据只看这个时间戳，因此它必须在派出后立刻落库，否则一次 tick 内
    // 的重复扫描会把同一个目标派两次。
    if lane == "patrol" {
        sqlx::query(
            "UPDATE collection_observation_target \
             SET last_patrol_dispatched_at = scope_001_now() WHERE target_ref = $1",
        )
        .bind(target_ref)
        .execute(database.pool())
        .await
        .map_err(|error| error.to_string())?;
    }
    Ok(Some(()))
}

/// 批量设置巡检开关。
///
/// 批量是**明确指定开或关**，不是逐个取反：取反会让一次操作里有的开有的关，人点了
/// 「批量开启巡检」却得到一半关掉，那不是他要的。
pub async fn set_monitoring_for_many(
    database: &Database,
    target_refs: &[Uuid],
    enabled: bool,
) -> Result<u64, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(0);
    }
    Ok(sqlx::query(
        "UPDATE collection_observation_target SET monitoring_enabled = $2 \
         WHERE target_ref = ANY($1)",
    )
    .bind(target_refs)
    .bind(enabled)
    .execute(database.pool())
    .await?
    .rows_affected())
}

/// 批量设置分组。空名字表示取消分组——那是一个正常操作，不是错误输入。
pub async fn set_group_for_many(
    database: &Database,
    target_refs: &[Uuid],
    group_name: Option<&str>,
) -> Result<u64, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(0);
    }
    Ok(sqlx::query(
        "UPDATE collection_observation_target SET group_name = $2 WHERE target_ref = ANY($1)",
    )
    .bind(target_refs)
    .bind(group_name.map(str::trim).filter(|value| !value.is_empty()))
    .execute(database.pool())
    .await?
    .rows_affected())
}

/// 读一个目标当前的巡检开关。切换是「读了再写反」，不是盲写——盲写会让两个入口同时
/// 操作时互相覆盖。
pub async fn target_monitoring_enabled(
    database: &Database,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT monitoring_enabled FROM collection_observation_target WHERE target_ref = $1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await
    .map(|value| value.unwrap_or(false))
}

/// 开或关一个目标的巡检，并设定间隔。
///
/// 间隔由人给定或由建档数据算出（产品规则 §4.2：发布间隔中位数 ÷ 2），上下限
/// 6 小时 ~ 7 天由数据库 CHECK 保证——**边界写在库上，不写在调用方**，否则每个新入口
/// 都要重新记得校验一次。
pub async fn set_target_monitoring(
    database: &Database,
    target_ref: Uuid,
    enabled: bool,
    interval_seconds: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE collection_observation_target \
         SET monitoring_enabled = $2, \
             patrol_interval_seconds = coalesce($3, patrol_interval_seconds) \
         WHERE target_ref = $1",
    )
    .bind(target_ref)
    .bind(enabled)
    .bind(interval_seconds)
    .execute(database.pool())
    .await?;
    Ok(())
}
