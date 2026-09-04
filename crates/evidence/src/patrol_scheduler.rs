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

use crate::acquisition_chain::{RequestLeaseError, request_admit_and_lease};
use crate::collection_control::{ComparableObservationRound, DynamicCadence, dynamic_cadence};
use crate::work_order_lease::LeaseError;
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
const PATROL_MAX_CONSIDERED_PER_TICK: usize = 500;

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
    let scheduler_run_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_scheduler_run (scheduler_run_ref,scheduler_key) \
         VALUES ($1,'patrol')",
    )
    .bind(scheduler_run_ref)
    .execute(database.pool())
    .await?;

    type CandidateRow = (
        Uuid,
        String,
        Option<Uuid>,
        Option<String>,
        Option<bool>,
        Option<bool>,
        Option<i32>,
        Option<i32>,
    );
    let mut considered_refs: Vec<Uuid> = Vec::new();
    let mut summary = PatrolTickSummary::default();
    while considered_refs.len() < PATROL_MAX_CONSIDERED_PER_TICK {
        let page: Vec<CandidateRow> = sqlx::query_as(
            "SELECT target.target_ref,target.lifecycle_state, \
                    target.active_monitor_rule_revision_ref,rule.mode,rule.automatic_enabled, \
                    CASE WHEN rule.rule_revision_ref IS NULL THEN false ELSE ( \
                      ((EXTRACT(ISODOW FROM scope_001_now() AT TIME ZONE rule.timezone)<6 \
                         AND rule.run_on_weekdays) OR \
                       (EXTRACT(ISODOW FROM scope_001_now() AT TIME ZONE rule.timezone)>=6 \
                         AND rule.run_on_weekends)) \
                      AND (rule.all_day OR \
                           ((EXTRACT(HOUR FROM scope_001_now() AT TIME ZONE rule.timezone)::integer*60) \
                             + EXTRACT(MINUTE FROM scope_001_now() AT TIME ZONE rule.timezone)::integer) \
                               >= rule.window_start_minute \
                           AND ((EXTRACT(HOUR FROM scope_001_now() AT TIME ZONE rule.timezone)::integer*60) \
                             + EXTRACT(MINUTE FROM scope_001_now() AT TIME ZONE rule.timezone)::integer) \
                               < rule.window_end_minute)) END, \
                    rule.fixed_interval_seconds,rule.fallback_interval_seconds \
             FROM collection_observation_target target \
             LEFT JOIN collection_monitor_rule_revision rule \
               ON rule.rule_revision_ref=target.active_monitor_rule_revision_ref \
             WHERE target.monitoring_enabled \
               AND NOT (target.target_ref=ANY($1)) \
               AND NOT EXISTS (SELECT 1 FROM collection_scheduler_target_decision prior \
                               WHERE prior.target_ref=target.target_ref \
                                 AND prior.next_eligible_at>scope_001_now()) \
             ORDER BY target.last_scheduler_considered_at NULLS FIRST,target.target_ref \
             LIMIT $2",
        )
        .bind(&considered_refs)
        .bind(PATROL_PAGE_SIZE)
        .fetch_all(database.pool())
        .await?;
        if page.is_empty() {
            break;
        }
        for (
            target_ref,
            lifecycle_state,
            rule_revision_ref,
            mode,
            automatic_enabled,
            schedule_open,
            fixed_interval_seconds,
            fallback_interval_seconds,
        ) in page
        {
            considered_refs.push(target_ref);
            let mut cadence_source: Option<&'static str> = None;
            let mut effective_interval_seconds: Option<i32> = None;
            let mut dynamic_reason_code: Option<&'static str> = None;
            let (outcome, reason_code, work_order_ref, lease_ref, retry_after_seconds) =
                if rule_revision_ref.is_none() {
                    ("rejected", "rule_missing", None, None, 300)
                } else if automatic_enabled != Some(true) {
                    (
                        "skipped",
                        if mode.as_deref() == Some("manual_only") {
                            "manual_only"
                        } else {
                            "monitoring_paused"
                        },
                        None,
                        None,
                        300,
                    )
                } else if lifecycle_state != "monitoring" {
                    ("rejected", "target_not_requestable", None, None, 300)
                } else if schedule_open != Some(true) {
                    ("deferred", "not_due", None, None, 300)
                } else {
                    let interval_seconds = match mode.as_deref() {
                        Some("fixed") => {
                            cadence_source = Some("fixed");
                            fixed_interval_seconds
                        }
                        Some("dynamic") => match read_dynamic_cadence_for_rule(
                            database,
                            target_ref,
                            rule_revision_ref.expect("checked rule"),
                        )
                        .await?
                        {
                            DynamicCadence::Available { interval_seconds } => {
                                cadence_source = Some("dynamic");
                                Some(interval_seconds)
                            }
                            DynamicCadence::Unavailable { reason_code } => {
                                cadence_source = Some("fixed_fallback");
                                dynamic_reason_code = Some(reason_code);
                                fallback_interval_seconds
                            }
                        },
                        _ => None,
                    };
                    if let Some(interval_seconds) = interval_seconds {
                        effective_interval_seconds = Some(interval_seconds);
                        let seconds_until_due: i64 = sqlx::query_scalar(
                        "SELECT CASE WHEN last_patrol_dispatched_at IS NULL THEN 0 ELSE \
                             GREATEST(0,CEIL(EXTRACT(EPOCH FROM \
                               (last_patrol_dispatched_at+make_interval(secs=>$2)-scope_001_now()))))::bigint END \
                         FROM collection_observation_target WHERE target_ref=$1",
                    )
                    .bind(target_ref)
                    .bind(interval_seconds)
                    .fetch_one(database.pool())
                    .await?;
                        if seconds_until_due > 0 {
                            (
                                "deferred",
                                "not_due",
                                None,
                                None,
                                i32::try_from(seconds_until_due).unwrap_or(i32::MAX),
                            )
                        } else {
                            match dispatch_one(database, target_ref).await {
                                Ok((work_order_ref, lease_ref)) => {
                                    summary.dispatched.push(target_ref);
                                    (
                                        "dispatched",
                                        "dispatched",
                                        Some(work_order_ref),
                                        Some(lease_ref),
                                        interval_seconds,
                                    )
                                }
                                Err(reason_code) => ("rejected", reason_code, None, None, 300),
                            }
                        }
                    } else {
                        cadence_source = None;
                        ("rejected", "manual_only", None, None, 300)
                    }
                };
            if outcome != "dispatched" {
                summary.skipped.push((target_ref, reason_code.to_owned()));
            }

            sqlx::query(
                "WITH considered AS ( \
                   UPDATE collection_observation_target \
                   SET last_scheduler_considered_at=scope_001_now() WHERE target_ref=$2) \
                 INSERT INTO collection_scheduler_target_decision \
                   (target_decision_ref,scheduler_run_ref,target_ref,rule_revision_ref, \
                    outcome,reason_code,cadence_source,effective_interval_seconds, \
                    dynamic_reason_code,next_eligible_at,work_order_ref,lease_ref) \
                 VALUES ($1,$3,$2,$4,$5,$6,$7,$8,$9, \
                         scope_001_now()+make_interval(secs=>$10),$11,$12)",
            )
            .bind(Uuid::new_v4())
            .bind(target_ref)
            .bind(scheduler_run_ref)
            .bind(rule_revision_ref)
            .bind(outcome)
            .bind(reason_code)
            .bind(cadence_source)
            .bind(effective_interval_seconds)
            .bind(dynamic_reason_code)
            .bind(retry_after_seconds)
            .bind(work_order_ref)
            .bind(lease_ref)
            .execute(database.pool())
            .await?;
        }
    }

    let run_outcome = if summary.dispatched.is_empty() && summary.skipped.is_empty() {
        "idle"
    } else if summary.skipped.is_empty() {
        "dispatched"
    } else {
        "partial"
    };
    sqlx::query(
        "UPDATE collection_scheduler_run SET completed_at=scope_001_now(),outcome=$2, \
             considered_count=$3,dispatched_count=$4 WHERE scheduler_run_ref=$1",
    )
    .bind(scheduler_run_ref)
    .bind(run_outcome)
    .bind(i32::try_from(considered_refs.len()).unwrap_or(i32::MAX))
    .bind(i32::try_from(summary.dispatched.len()).unwrap_or(i32::MAX))
    .execute(database.pool())
    .await?;
    Ok(summary)
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

/// 为一个到期目标走完整条授权链。
///
/// **自动化不是豁免权**：它走的路与人点一次按钮完全一样。准入若说资源不够、风险暂停生效
/// 或额度触顶，调度也只能记下理由然后等——不会因为「是定时任务」就放行。
async fn dispatch_one(database: &Database, target_ref: Uuid) -> Result<(Uuid, Uuid), &'static str> {
    let execution = // 申请人是 `agent` 而不是 `person`：调度器不是人。合同写着「Agent 可以提出需要，
    // 不能自行扩大观察面」——调度器提出巡检申请正是这个位置：它能申请，能不能跑仍由
    // 准入决定。记成 person 会让追责链指向一个当时并不在场的人。
    request_admit_and_lease(
        database,
        target_ref,
        "patrol",
        "定时巡检",
        "agent",
        PATROL_LEASE_MINUTES,
    )
    .await
    .map_err(scheduler_error_code)?;

    let AdmissionOutcome::Admitted { .. } = execution.request.outcome else {
        return Err(scheduler_decision_reason(execution.request.reason_code));
    };
    let Some(work_order_ref) = execution.request.work_order_ref else {
        return Err("admission_refused");
    };
    let Some(lease) = execution.lease else {
        return Err("lease_issue_failed");
    };

    Ok((work_order_ref, lease.lease_ref))
}

fn scheduler_error_code(error: RequestLeaseError) -> &'static str {
    match error {
        RequestLeaseError::Lease(LeaseError::ControlBlocked { reason_code }) => {
            match reason_code.as_str() {
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
                "station_daily_budget_reached" => "station_daily_budget_reached",
                "rule_revision_changed" => "rule_revision_changed",
                "monitoring_paused" => "monitoring_paused",
                "authorization_expired_or_revoked" => "authorization_expired_or_revoked",
                "target_not_requestable" => "target_not_requestable",
                _ => "lease_issue_failed",
            }
        }
        RequestLeaseError::Lease(LeaseError::AuthorizationLapsed) => {
            "authorization_expired_or_revoked"
        }
        RequestLeaseError::Lease(_) => "lease_issue_failed",
        RequestLeaseError::Acquisition(
            crate::acquisition_chain::AcquisitionChainError::TargetNotRequestable { .. },
        ) => "target_not_requestable",
        RequestLeaseError::Acquisition(_) => "database_error",
    }
}

fn scheduler_decision_reason(value: &str) -> &'static str {
    match value {
        "authorization_missing" => "authorization_missing",
        "authorization_purpose_mismatch" => "authorization_purpose_mismatch",
        "authorization_target_limit_reached" => "authorization_target_limit_reached",
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
        "station_daily_budget_reached" => "station_daily_budget_reached",
        "capacity_unknown" => "capacity_unknown",
        _ => "admission_refused",
    }
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
