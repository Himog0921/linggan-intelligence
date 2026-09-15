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

use crate::collection_control::{ComparableObservationRound, DynamicCadence, dynamic_cadence};
use linggan_contracts::AdmissionOutcome;
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一次 tick 的结果。**派了什么与没派什么同样重要。**
#[derive(Debug, Default)]
pub struct PatrolTickSummary {
    /// Work Orders durably queued in this tick. A queued order is intentionally
    /// not reported as a browser dispatch: the plugin has not claimed it yet.
    pub queued: Vec<Uuid>,
    /// Retained for API compatibility with older readers. New scheduling never
    /// writes this field because lease/task creation belongs to station claim.
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

/// A tick does only due-rule selection and durable queueing. Browser execution
/// happens later at station claim, so this bounded scan is intentionally light.
const PATROL_PAGE_SIZE: i64 = 50;
// Retained only while the pre-0036 scheduler helper remains compiled for
// historical test fixtures; the live path above does not pre-lease work.

pub async fn patrol_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_monitor_rule_revision') IS NOT NULL \
                AND to_regclass('collection_scheduler_run') IS NOT NULL \
                AND to_regclass('collection_scheduler_target_decision') IS NOT NULL",
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
        sqlx::query(
            "INSERT INTO collection_scheduler_heartbeat \
                 (scheduler_key,last_tick_started_at,last_outcome,last_error) \
             VALUES ('patrol',scope_001_now(),'unknown',NULL) \
             ON CONFLICT (scheduler_key) DO UPDATE SET \
                 last_tick_started_at=EXCLUDED.last_tick_started_at,last_outcome='unknown',last_error=NULL",
        )
        .execute(database.pool())
        .await?;
    }
    let result = run_due_patrols_inner(database).await;
    if heartbeat_ready {
        let (outcome, dispatched, skipped, error) = match &result {
            Ok(summary) if summary.queued.is_empty() && summary.skipped.is_empty() => {
                ("idle", 0, 0, None)
            }
            Ok(summary) if summary.skipped.is_empty() => (
                "queued",
                i32::try_from(summary.queued.len()).unwrap_or(i32::MAX),
                0,
                None,
            ),
            Ok(summary) => (
                "partial",
                i32::try_from(summary.queued.len()).unwrap_or(i32::MAX),
                i32::try_from(summary.skipped.len()).unwrap_or(i32::MAX),
                None,
            ),
            Err(_) => ("failed", 0, 0, Some("database_error")),
        };
        let heartbeat_result = sqlx::query(
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
        if result.is_ok() {
            heartbeat_result?;
        }
    }
    result
}

/// Queue only rules that are valid *and already due*. In particular, this never
/// scans arbitrary `monitoring_enabled` targets and then emits `rule_missing`:
/// that was an implementation leak from a state the database now forbids.
///
/// The rule's mutable `next_run_at` advances in the same transaction as the
/// Work Order. A delayed scheduler therefore produces one catch-up order and
/// records the number of missed intervals rather than emitting an accidental
/// backlog of browser work.
async fn run_due_patrols_inner(database: &Database) -> Result<PatrolTickSummary, sqlx::Error> {
    if !patrol_schema_is_ready(database).await? {
        return Ok(PatrolTickSummary::default());
    }
    let scheduler_run_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_scheduler_run (scheduler_run_ref,scheduler_key) VALUES ($1,'patrol')",
    )
    .bind(scheduler_run_ref)
    .execute(database.pool())
    .await?;

    // 到期是**按规则**算的，不是按目标：一个关键词可以同时盯综合榜和点赞榜，两条规则
    // 各有各的周期。共用一个目标级的 `monitor_next_run_at` 说不清是谁该跑了。
    //
    // 闸门只认「没有推进排期」的决定，也就是被拒（`rejected`）。推进了排期的派发不写闸门：
    // 排期在派发那笔事务里已经翻到下一次，再按「派发时刻 + 周期」写一张便条，等于用另一套
    // 算法把同一句话说了第二遍——排期从固定网格推，便条从事情实际发生的时刻推，而派发总比
    // 网格晚，两个条件又必须同时成立，于是更晚的便条长期压过排期，并且每天再多累积几十秒
    // （调度器每分钟才醒一次，过了点最长得等到下一分钟）。界面上改周期 / 暂停恢复只重算
    // 排期、不动便条，就会出现「排期说到点了、便条说不行」的空档：2026-09-14 adhd 那条
    // 排期到点却被自己的旧闸门挡住，下一次要晚 27 小时。历史行里那些旧值因此不再有资格
    // 挡排期，但审计记录保持原样，不改写。
    let due_rules: Vec<Uuid> = sqlx::query_scalar(
        "SELECT rule.rule_ref \
         FROM collection_monitor_rule rule \
         JOIN collection_observation_target target USING(target_ref) \
         JOIN collection_monitor_rule_revision revision \
           ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.retired_at IS NULL \
           AND target.monitoring_enabled \
           AND target.lifecycle_state='monitoring' \
           AND revision.automatic_enabled \
           AND revision.mode='fixed' \
           AND rule.monitor_next_run_at IS NOT NULL \
           AND rule.monitor_next_run_at<=scope_001_now() \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_scheduler_target_decision prior \
             WHERE prior.rule_ref=rule.rule_ref \
               AND prior.outcome='rejected' \
               AND prior.next_eligible_at>scope_001_now()) \
         ORDER BY rule.monitor_next_run_at,rule.rule_ref LIMIT $1",
    )
    .bind(PATROL_PAGE_SIZE)
    .fetch_all(database.pool())
    .await?;

    let mut summary = PatrolTickSummary::default();
    for rule_ref in &due_rules {
        let (outcome, reason, target_ref, work_order_ref) =
            queue_one_due_rule(database, scheduler_run_ref, *rule_ref).await?;
        // 汇总仍按目标报：人关心的是「哪个词跑了」，规则是它内部的口径。
        if outcome == "queued" {
            summary.queued.push(target_ref);
        } else {
            summary.skipped.push((target_ref, reason.to_owned()));
        }
        let _ = work_order_ref;
    }

    let run_outcome = if summary.queued.is_empty() && summary.skipped.is_empty() {
        "idle"
    } else if summary.skipped.is_empty() {
        "queued"
    } else {
        "partial"
    };
    sqlx::query(
        "UPDATE collection_scheduler_run SET completed_at=scope_001_now(),outcome=$2, \
             considered_count=$3,dispatched_count=$4 WHERE scheduler_run_ref=$1",
    )
    .bind(scheduler_run_ref)
    .bind(run_outcome)
    .bind(i32::try_from(due_rules.len()).unwrap_or(i32::MAX))
    // The legacy column is retained for projection compatibility. It means
    // “orders queued by this scheduler run”, never “browser collection ran”.
    .bind(i32::try_from(summary.queued.len()).unwrap_or(i32::MAX))
    .execute(database.pool())
    .await?;
    Ok(summary)
}

/// Lock one due target, admit its bounded patrol and atomically move schedule
/// time. The transaction owns the Target lock through Request → Decision → Work
/// Order so a second tick cannot create the same scheduled order.
async fn queue_one_due_rule(
    database: &Database,
    scheduler_run_ref: Uuid,
    rule_ref: Uuid,
) -> Result<(&'static str, &'static str, Uuid, Option<Uuid>), sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    // 锁**规则**而不是目标：同一个目标的另一条规则该跑就让它跑，两条互不阻塞。
    let due: Option<(Uuid, Uuid, i32)> = sqlx::query_as(
        "SELECT rule.target_ref,revision.rule_revision_ref,revision.fixed_interval_seconds \
         FROM collection_monitor_rule rule \
         JOIN collection_observation_target target USING(target_ref) \
         JOIN collection_monitor_rule_revision revision \
           ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.rule_ref=$1 AND rule.retired_at IS NULL \
           AND target.monitoring_enabled AND target.lifecycle_state='monitoring' \
           AND revision.automatic_enabled AND revision.mode='fixed' \
           AND rule.monitor_next_run_at IS NOT NULL \
           AND rule.monitor_next_run_at<=scope_001_now() \
         FOR UPDATE OF rule",
    )
    .bind(rule_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((target_ref, rule_revision_ref, interval_seconds)) = due else {
        transaction.rollback().await?;
        // 目标引用拿不到时用规则引用回报：调用方只是要在汇总里说清是谁没跑。
        let target_ref: Uuid =
            sqlx::query_scalar("SELECT target_ref FROM collection_monitor_rule WHERE rule_ref=$1")
                .bind(rule_ref)
                .fetch_one(database.pool())
                .await?;
        return Ok(("deferred", "not_due", target_ref, None));
    };

    // **冻进工单的必须是这一条到期规则的版本**，不能让准入去目标行上读「当前规则」：
    // 一个关键词可以同时盯综合榜和点赞榜，点赞榜到期时读到综合榜的口径，插件就会按错误的
    // 排序和取样上限去采——而回执、覆盖度、材料全都自洽，没有任何一处看得出来采错了。
    let request = crate::acquisition_chain::request_and_admit_in_transaction_scoped(
        &mut transaction,
        target_ref,
        "patrol",
        "定时巡检",
        "agent",
        &[],
        &[],
        None,
        Some(rule_revision_ref),
        false,
    )
    .await;
    let (outcome, reason_code, work_order_ref, advance_schedule) = match request {
        Ok(request) => match request.outcome {
            AdmissionOutcome::Admitted { .. } => (
                "queued",
                "queued",
                request.work_order_ref,
                request.work_order_ref.is_some(),
            ),
            AdmissionOutcome::Merge { .. } => ("deferred", "in_flight_work_covers_it", None, true),
            _ => (
                "rejected",
                scheduler_decision_reason(request.reason_code),
                None,
                false,
            ),
        },
        Err(crate::acquisition_chain::AcquisitionChainError::Database(error)) => return Err(error),
        // **每种准入失败说出自己的原因。**
        //
        // 此前这里是 `Err(_) => "target_not_requestable"`：一个字符串吞掉了「这个目标还没
        // 归属领域」「授权没签」「授权额度不够 200 篇」「schema 没装」「目标不存在」全部
        // 情况。界面上只看到「目标不可请求」——而真实原因是目标没有领域，排查多花了两轮。
        // 一个压平的原因码比没有原因码更坏：它看起来是个答案。
        Err(error) => (
            "rejected",
            scheduler_admission_failure_reason(&error),
            None,
            false,
        ),
    };

    if let Some(work_order_ref) = work_order_ref {
        // The generic request has a queue identity before scheduler context is
        // known. Freeze the due timestamp into this scheduled identity now.
        sqlx::query(
            // 冻结的是**这条规则**该跑的那个时刻，不是目标行上那一列。排期搬到规则上之后
            // 还读目标行，会把工单冻结在一个未来的时间点——它排进了队列却永远派不出去，
            // 而队列上看不出任何异常。
            "UPDATE collection_work_order SET scheduled_for=( \
                 SELECT monitor_next_run_at FROM collection_monitor_rule WHERE rule_ref=$4), \
                 dedupe_key=concat('scheduled:', $2::text, ':', $3::text, ':', \
                   (SELECT monitor_next_run_at::text FROM collection_monitor_rule \
                     WHERE rule_ref=$4)) \
             WHERE work_order_ref=$1 AND queue_state='queued'",
        )
        .bind(work_order_ref)
        .bind(target_ref)
        .bind(rule_revision_ref)
        .bind(rule_ref)
        .execute(&mut *transaction)
        .await?;
    }

    if advance_schedule {
        sqlx::query(
            "UPDATE collection_monitor_rule \
             SET monitor_missed_run_count=monitor_missed_run_count + GREATEST(0, \
                   FLOOR(EXTRACT(EPOCH FROM (scope_001_now()-monitor_next_run_at))/$2)::integer), \
                 monitor_next_run_at=monitor_next_run_at + make_interval(secs => $2 * \
                   (GREATEST(0,FLOOR(EXTRACT(EPOCH FROM \
                       (scope_001_now()-monitor_next_run_at))/$2)::integer)+1)), \
                 last_patrol_dispatched_at=scope_001_now() \
             WHERE rule_ref=$1",
        )
        .bind(rule_ref)
        .bind(interval_seconds)
        .execute(&mut *transaction)
        .await?;
    }
    // 「最近考虑过」是目标级的公平性事实，两种情况都要记：它决定下一轮先看谁。
    sqlx::query(
        "UPDATE collection_observation_target SET last_scheduler_considered_at=scope_001_now() \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(&mut *transaction)
    .await?;

    // 闸门便条只在「这次没推进排期」时留：被拒（额度不够、没授权、目标不可请求……）之后等
    // 300 秒再来看，免得调度器每分钟都对着同一个目标重试同一件注定失败的事。
    //
    // 推进了排期的派发（`queued` / `deferred`）不留便条：排期在同一笔事务里已经翻到下一次，
    // 再写一张「派发时刻 + 周期」的便条，是用第二套算法把「下一次什么时候」又说了一遍，而
    // 两个起点不同（网格 vs 事件）、又必须同时成立，更晚的那张便条会长期压过排期。留空不是
    // 缺省，而是如实表示「这次没有需要记下的重试冷却」。
    let gate_seconds: Option<i32> = if advance_schedule { None } else { Some(300) };
    sqlx::query(
        "INSERT INTO collection_scheduler_target_decision \
             (target_decision_ref,scheduler_run_ref,target_ref,rule_ref,rule_revision_ref, \
              outcome,reason_code, \
              cadence_source,effective_interval_seconds,next_eligible_at,work_order_ref,lease_ref) \
         VALUES ($1,$2,$3,$10,$4,$5,$6,'fixed',$7, \
                 scope_001_now()+make_interval(secs=>$8),$9,NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(scheduler_run_ref)
    .bind(target_ref)
    .bind(rule_revision_ref)
    .bind(outcome)
    .bind(reason_code)
    .bind(interval_seconds)
    .bind(gate_seconds)
    .bind(work_order_ref)
    .bind(rule_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok((outcome, reason_code, target_ref, work_order_ref))
}

/// Read the exact, durable observations that are eligible to teach a dynamic cadence. The
/// package observation timestamp is deliberately not a publication-time substitute. Until two
/// distinct completed rounds under the same frozen rule/account lens qualify, callers receive
/// `dynamic_unavailable` and use the rule's explicit fixed fallback. A round may contain several
/// records; `dynamic_cadence` collapses those records to one latest exact publication timestamp
/// before calculating gaps between rounds.
pub async fn read_dynamic_cadence_for_rule(
    database: &Database,
    target_ref: Uuid,
    rule_revision_ref: Uuid,
) -> Result<DynamicCadence, sqlx::Error> {
    let lens: Option<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT surface_key,ranking_key,task_contract_version \
         FROM collection_monitor_rule_revision \
         WHERE target_ref=$1 AND rule_revision_ref=$2",
    )
    .bind(target_ref)
    .bind(rule_revision_ref)
    .fetch_optional(database.pool())
    .await?;
    let Some((surface_key, ranking_key, task_contract_version)) = lens else {
        return Ok(DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable",
        });
    };
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT work_order.work_order_ref,work_order.account_ref, \
                MAX((record.value #>> '{payload,publishedAt}')::bigint) \
         FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
         JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         CROSS JOIN LATERAL jsonb_array_elements( \
           CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
         CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') \
              WITH ORDINALITY AS record(value,ordinality) \
         JOIN linggan_runtime_record_disposition disposition \
           ON disposition.package_ref=package.package_ref \
          AND disposition.record_ordinal=record.ordinality-1 \
         WHERE work_order.target_ref=$1 \
           AND work_order.monitor_rule_revision_ref=$2 \
           AND work_order.account_ref IS NOT NULL \
           AND task.task_spec->>'contractVersion'=$3 \
           AND package.package_kind='profile_discovery' \
           AND receipt.material_admission='ACCEPTED' \
           AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
           AND disposition.disposition='accepted_for_library_discovery' \
           AND layer->>'capability'='profile_discovery' \
           AND COALESCE(layer->>'observed','') ~ '^[0-9]+$' \
           AND COALESCE(layer->>'attempted','') ~ '^[0-9]+$' \
           AND COALESCE(layer->>'acquired','') ~ '^[0-9]+$' \
           AND (layer->>'observed')::integer>0 \
           AND (layer->>'attempted')::integer>0 \
           AND (layer->>'acquired')::integer>0 \
           AND COALESCE((layer->>'failed')::integer,0)=0 \
           AND COALESCE((layer->>'notAttempted')::integer,0)=0 \
           AND COALESCE((layer->>'unknown')::integer,0)=0 \
           AND layer->>'stoppedReason' IN ('surface_ended','maximum_quota') \
           AND record.value #>> '{payload,publishedAtSourceKind}'='platform_epoch' \
           AND record.value #>> '{payload,publishedAtPrecision}' IN ('second','millisecond') \
           AND COALESCE(record.value #>> '{payload,publishedAt}','') ~ '^[0-9]+$' \
           AND (record.value #>> '{payload,publishedAt}')::numeric>0 \
         GROUP BY work_order.work_order_ref,work_order.account_ref \
         ORDER BY work_order.work_order_ref",
    )
    .bind(target_ref)
    .bind(rule_revision_ref)
    .bind(&task_contract_version)
    .fetch_all(database.pool())
    .await?;
    let Some(account_ref) = rows.first().map(|row| row.1) else {
        return Ok(DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable",
        });
    };
    let rounds: Vec<ComparableObservationRound> = rows
        .into_iter()
        .map(
            |(round_ref, row_account_ref, publication)| ComparableObservationRound {
                observation_round_ref: round_ref,
                rule_revision_ref,
                publication_epoch_seconds: if publication > 100_000_000_000 {
                    publication / 1_000
                } else {
                    publication
                },
                exact_publication_time: true,
                accepted_receipt: true,
                coverage_qualified: true,
                surface_key: surface_key.clone(),
                ranking_key: ranking_key.clone(),
                task_contract_version: task_contract_version.clone(),
                account_ref: row_account_ref,
            },
        )
        .collect();
    Ok(dynamic_cadence(
        &rounds,
        &surface_key,
        ranking_key.as_deref(),
        &task_contract_version,
        account_ref,
        rule_revision_ref,
    ))
}

fn scheduler_decision_reason(value: &str) -> &'static str {
    match value {
        "authorization_missing" => "authorization_missing",
        "authorization_scope_mismatch" => "authorization_scope_mismatch",
        "authorization_purpose_mismatch" => "authorization_purpose_mismatch",
        "authorization_target_limit_reached" => "authorization_target_limit_reached",
        "authorization_work_unit_limit_reached" => "authorization_work_unit_limit_reached",
        "authorization_expired_or_revoked" => "authorization_expired_or_revoked",
        "risk_paused" => "risk_paused",
        "station_unavailable" => "station_unavailable",
        "station_not_accepting" => "station_not_accepting",
        "installation_credential_missing" => "installation_credential_missing",
        "plugin_version_unsupported" => "plugin_version_unsupported",
        "installation_stale" => "installation_stale",
        "capability_missing" => "capability_missing",
        "account_unbound" => "account_unbound",
        "account_binding_changed" => "account_binding_changed",
        "account_binding_expired" => "account_binding_expired",
        "account_eligibility_stale" => "account_eligibility_stale",
        "account_cooling" => "account_cooling",
        "account_needs_login" => "account_needs_login",
        "account_restricted" => "account_restricted",
        "account_unknown" => "account_unknown",
        "account_busy" => "account_busy",
        "station_busy" => "station_busy",
        "station_daily_budget_reached" => "station_daily_budget_reached",
        "capacity_unknown" => "capacity_unknown",
        _ => "admission_refused",
    }
}

/// Legacy compatibility helper for disabling a current rule or restoring a *valid* active rule.
///
/// New product paths must use `apply_monitor_rule_command`, which writes the immutable revision,
/// target pointer and schedule atomically with a durable command receipt.  This helper cannot
/// manufacture monitoring for a target with no active automatic rule.
pub async fn set_monitoring_for_many(
    database: &Database,
    target_refs: &[Uuid],
    enabled: bool,
) -> Result<u64, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(0);
    }
    let query = if enabled {
        // 开观察：目标级开关打开，条件是它至少有一条自动巡检开着的规则——没有规则可跑的
        // 目标「在观察中」是一句空话。排期由规则自己持有，这里不再复制一份到目标行。
        "UPDATE collection_observation_target target \
         SET monitoring_enabled=true, lifecycle_state='monitoring', \
             lifecycle_changed_at=scope_001_now() \
         WHERE target.target_ref=ANY($1) \
           AND target.lifecycle_state <> 'dismissed' \
           AND EXISTS (SELECT 1 FROM collection_monitor_rule rule \
                       JOIN collection_monitor_rule_revision revision \
                         ON revision.rule_revision_ref=rule.active_revision_ref \
                       WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
                         AND revision.automatic_enabled)"
    } else {
        "UPDATE collection_observation_target \
         SET monitoring_enabled=false, \
             lifecycle_state=CASE WHEN lifecycle_state='monitoring' THEN 'paused' ELSE lifecycle_state END, \
             lifecycle_changed_at=CASE WHEN lifecycle_state='monitoring' THEN scope_001_now() \
                                       ELSE lifecycle_changed_at END \
         WHERE target_ref=ANY($1)"
    };
    Ok(sqlx::query(query)
        .bind(target_refs)
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

/// Legacy compatibility helper. It cannot alter an immutable rule's interval or create a rule;
/// use the versioned rule command for any new UI or integration path.
pub async fn set_target_monitoring(
    database: &Database,
    target_ref: Uuid,
    enabled: bool,
    interval_seconds: Option<i32>,
) -> Result<(), sqlx::Error> {
    let _ = interval_seconds;
    if enabled {
        sqlx::query(
            "UPDATE collection_observation_target target \
             SET monitoring_enabled=true,lifecycle_state='monitoring', \
                 lifecycle_changed_at=scope_001_now() \
             WHERE target.target_ref=$1 \
               AND target.lifecycle_state <> 'dismissed' \
               AND EXISTS (SELECT 1 FROM collection_monitor_rule rule \
                           JOIN collection_monitor_rule_revision revision \
                             ON revision.rule_revision_ref=rule.active_revision_ref \
                           WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
                             AND revision.automatic_enabled)",
        )
        .bind(target_ref)
        .execute(database.pool())
        .await?;
    } else {
        sqlx::query(
            "UPDATE collection_observation_target \
             SET monitoring_enabled=false, \
                 lifecycle_state=CASE WHEN lifecycle_state='monitoring' THEN 'paused' ELSE lifecycle_state END, \
                 lifecycle_changed_at=CASE WHEN lifecycle_state='monitoring' THEN scope_001_now() \
                                           ELSE lifecycle_changed_at END \
             WHERE target_ref=$1",
        )
        .bind(target_ref)
        .execute(database.pool())
        .await?;
    }
    Ok(())
}

/// 准入直接报错（还没走到决策）时，如实说出是哪一种。
///
/// 返回的是闭集里的机器原因码，与决策上持久化的那一套同源——调度写进
/// `collection_scheduler_target_decision.reason_code`，界面按它显示。
fn scheduler_admission_failure_reason(
    error: &crate::acquisition_chain::AcquisitionChainError,
) -> &'static str {
    use crate::acquisition_chain::AcquisitionChainError as Failure;
    match error {
        Failure::SchemaUnavailable => "acquisition_schema_unavailable",
        Failure::UnknownTarget => "unknown_target",
        Failure::TargetDomainUnassigned => "target_domain_unassigned",
        Failure::TargetNotRequestable { .. } => "target_not_requestable",
        Failure::KeywordArchiveIncomplete => "baseline_not_ready",
        Failure::InvalidMaterialTargets => "invalid_material_targets",
        Failure::ProgressiveArchiveAuthorizationTooSmall { .. } => "authorization_bound_too_small",
        Failure::ProgressiveArchiveAuthorizationMissing => "authorization_missing",
        Failure::ProgressiveArchivePurposeMismatch => "progressive_purpose_mismatch",
        Failure::ProgressiveArchiveNotReady { .. } => "progressive_archive_not_ready",
        // 数据库错误在调用点上一条 match 臂就 `return Err` 了，走不到这里。仍然显式写出
        // 而不是留 `_`：留了兜底，将来新增一个错误变体时编译器不会拦，它会悄悄落进通用桶。
        // 用词表里既有的 `database_error`——**不为一个走不到的分支往闭集里新造一个词**，
        // 那等于在词表里留一个永远不会出现的答案。
        Failure::Database(_) => "database_error",
    }
}
