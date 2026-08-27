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

/// 租约时长：一次巡检该在多久内跑完。
///
/// 比巡检间隔短得多——租约是「这次允许你跑多久」，不是「下次什么时候再跑」。给得过长，
/// 一次卡住的执行会一直占着工单，直到下一轮都无法重派。
const PATROL_LEASE_MINUTES: i32 = 30;

/// 一次扫描最多处理多少个目标。分页是为了让 tick 保持轻量（规则文档：tick 只做轻量
/// 状态推进与入队，重活另开进程）。
const PATROL_PAGE_SIZE: i64 = 50;

pub async fn patrol_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
                        WHERE table_name = 'collection_observation_target' \
                          AND column_name = 'monitoring_enabled')",
    )
    .fetch_one(database.pool())
    .await
}

/// 跑一轮巡检调度。
///
/// 到期判据只看「上次**派出**的时间」，不看采集是否成功：采集失败也算看过了，否则一个
/// 持续失败的目标会被无限重试，把当天额度吃光。
pub async fn run_due_patrols(database: &Database) -> Result<PatrolTickSummary, sqlx::Error> {
    if !patrol_schema_is_ready(database).await? {
        return Ok(PatrolTickSummary::default());
    }
    let due: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT target_ref FROM collection_observation_target \
         WHERE monitoring_enabled \
           AND lifecycle_state IN ('monitoring', 'archiving') \
           AND ( \
                 last_patrol_dispatched_at IS NULL \
                 OR last_patrol_dispatched_at \
                    + make_interval(secs => patrol_interval_seconds) <= scope_001_now() \
               ) \
         ORDER BY last_patrol_dispatched_at NULLS FIRST \
         LIMIT $1",
    )
    .bind(PATROL_PAGE_SIZE)
    .fetch_all(database.pool())
    .await?;

    let mut summary = PatrolTickSummary::default();
    for (target_ref,) in due {
        match dispatch_one_patrol(database, target_ref).await {
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
async fn dispatch_one_patrol(database: &Database, target_ref: Uuid) -> Result<Option<()>, String> {
    let outcome = // 申请人是 `agent` 而不是 `person`：调度器不是人。合同写着「Agent 可以提出需要，
    // 不能自行扩大观察面」——调度器提出巡检申请正是这个位置：它能申请，能不能跑仍由
    // 准入决定。记成 person 会让追责链指向一个当时并不在场的人。
    request_and_admit(database, target_ref, "patrol", "定时巡检", "agent")
        .await
        .map_err(|error| error.to_string())?;

    let AdmissionOutcome::Admitted { .. } = outcome.outcome else {
        // 未获准入不是故障：资源不够、风险暂停中都是正常结论。理由原样留下。
        return Err(format!("准入未通过：{}", outcome.outcome.code()));
    };
    let Some(work_order_ref) = outcome.work_order_ref else {
        return Err("准入通过但没有工单".to_owned());
    };
    issue_work_order_lease(database, work_order_ref, PATROL_LEASE_MINUTES)
        .await
        .map_err(|error| error.to_string())?;

    // 派出即记时。到期判据只看这个时间戳，因此它必须在派出后立刻落库，否则一次 tick 内
    // 的重复扫描会把同一个目标派两次。
    sqlx::query(
        "UPDATE collection_observation_target \
         SET last_patrol_dispatched_at = scope_001_now() WHERE target_ref = $1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .map_err(|error| error.to_string())?;
    Ok(Some(()))
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
