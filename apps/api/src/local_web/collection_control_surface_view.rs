// Durable Package 2 control projections for Operations, Attention, Tasks and Runtime.
//
// The scheduler decision and frozen resource references are history. Current Runtime capacity
// is read through `read_capacity`, the same evaluator Admission uses. The resource rows below
// only expose bounded control facts; they never select a credential hash, account digest,
// platform identity, Cookie, HTML, payload, Evidence content or free-form platform error.

use linggan_contracts::Capacity;
use linggan_evidence::{collection_control_schema_is_ready, read_capacity};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

use super::{
    OperationsMode, READOUT_SLOT_END, READOUT_SLOT_START, context_readout,
    replace_bounded_slot,
};

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";
const BODY_OPEN: &str = "<div class=\"c-body\">";
const BODY_SLOT_START: &str = "<!-- collection-body:start -->";
const BODY_SLOT_END: &str = "<!-- collection-body:end -->";

#[derive(Debug, Clone)]
pub enum CollectionControlSurfaceRead {
    Ready(CollectionControlSurfaceProjection),
    SchemaUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskControlUnavailable {
    SchemaUnavailable,
    ReadFailed,
}

#[derive(Debug, Clone)]
pub struct CollectionControlSurfaceProjection {
    pub latest_run: Option<SchedulerRunView>,
    pub decisions: Vec<SchedulerDecisionView>,
    pub works: Vec<FrozenWorkView>,
    pub runtime_lanes: Vec<RuntimeLaneControlView>,
    pub runtime_resources: Vec<RuntimeResourceView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerRunView {
    pub scheduler_run_ref: Uuid,
    pub outcome: Option<String>,
    pub considered_count: i32,
    pub dispatched_count: i32,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerDecisionView {
    pub target_decision_ref: Uuid,
    pub scheduler_run_ref: Uuid,
    pub target_ref: Uuid,
    pub target_name: String,
    pub target_kind: String,
    pub rule_revision_ref: Option<Uuid>,
    pub outcome: String,
    pub reason_code: String,
    pub next_eligible_at: Option<String>,
    pub work_order_ref: Option<Uuid>,
    pub lease_ref: Option<Uuid>,
    pub decided_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenWorkView {
    pub work_order_ref: Uuid,
    pub target_ref: Uuid,
    pub target_name: String,
    pub lane: String,
    pub station_ref: Option<Uuid>,
    pub installation_ref: Option<Uuid>,
    pub account_ref: Option<Uuid>,
    pub eligibility_ref: Option<Uuid>,
    pub monitor_rule_revision_ref: Option<Uuid>,
    pub lease_ref: Option<Uuid>,
    pub lease_state: String,
    pub task_id: Option<Uuid>,
    pub task_state: Option<String>,
    pub station_is_current: Option<bool>,
    pub installation_is_current: Option<bool>,
    pub account_is_current: Option<bool>,
    pub rule_is_current: Option<bool>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLaneControlView {
    pub label: &'static str,
    pub target_kind: &'static str,
    pub lane: &'static str,
    /// A request is admissible and can enter the common queue, but no specific station is
    /// selected until a later eligible installation claims it.
    pub queueable: bool,
    pub available: bool,
    pub reason_code: Option<&'static str>,
    pub reason: Option<String>,
    pub station_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeResourceView {
    pub station_ref: Uuid,
    pub station_name: String,
    pub accepting_tasks: bool,
    pub installation_ref: Option<Uuid>,
    pub plugin_version: Option<String>,
    pub last_seen_at: Option<String>,
    pub has_valid_credential: bool,
    pub account_ref: Option<Uuid>,
    pub bound_account_ref: Option<Uuid>,
    pub binding_state: String,
    pub eligibility_ref: Option<Uuid>,
    pub eligibility_state: Option<String>,
    pub eligibility_reason_code: Option<String>,
    pub eligibility_observed_at: Option<String>,
    pub account_has_live_lease: bool,
}

pub async fn read_collection_control_surface(
    database: &Database,
    limit: i64,
) -> Result<CollectionControlSurfaceRead, sqlx::Error> {
    if !collection_control_schema_is_ready(database).await? {
        return Ok(CollectionControlSurfaceRead::SchemaUnavailable);
    }
    let limit = limit.clamp(1, 100);
    let latest_run = read_latest_run(database).await?;
    let decisions = read_recent_decisions(database, limit).await?;
    let works = read_frozen_works(database, limit).await?;
    let runtime_resources = read_runtime_resources(database).await?;
    let mut runtime_lanes = Vec::with_capacity(3);
    for (label, target_kind, lane) in [
        ("创作者基线", "creator", "deep_archive"),
        ("创作者巡检", "creator", "patrol"),
        ("关键词巡检", "keyword", "patrol"),
    ] {
        let capacity = read_capacity(database, "xhs", target_kind, lane).await?;
        runtime_lanes.push(runtime_lane(label, target_kind, lane, capacity));
    }
    Ok(CollectionControlSurfaceRead::Ready(
        CollectionControlSurfaceProjection {
            latest_run,
            decisions,
            works,
            runtime_lanes,
            runtime_resources,
        },
    ))
}

async fn read_latest_run(database: &Database) -> Result<Option<SchedulerRunView>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT scheduler_run_ref,outcome,considered_count,dispatched_count, \
                started_at::text AS started_at,completed_at::text AS completed_at \
         FROM collection_scheduler_run ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(SchedulerRunView {
            scheduler_run_ref: row.try_get("scheduler_run_ref")?,
            outcome: row.try_get("outcome")?,
            considered_count: row.try_get("considered_count")?,
            dispatched_count: row.try_get("dispatched_count")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
        })
    })
    .transpose()
}

async fn read_recent_decisions(
    database: &Database,
    limit: i64,
) -> Result<Vec<SchedulerDecisionView>, sqlx::Error> {
    sqlx::query(
        "SELECT decision.target_decision_ref,decision.scheduler_run_ref,decision.target_ref, \
                COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key) AS target_name, \
                target.target_kind,decision.rule_revision_ref,decision.outcome,decision.reason_code, \
                decision.next_eligible_at::text AS next_eligible_at,decision.work_order_ref, \
                decision.lease_ref,decision.decided_at::text AS decided_at \
         FROM collection_scheduler_target_decision decision \
         JOIN collection_observation_target target USING(target_ref) \
         ORDER BY decision.decided_at DESC,decision.target_decision_ref LIMIT $1",
    )
    .bind(limit)
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(|row| {
        Ok(SchedulerDecisionView {
            target_decision_ref: row.try_get("target_decision_ref")?,
            scheduler_run_ref: row.try_get("scheduler_run_ref")?,
            target_ref: row.try_get("target_ref")?,
            target_name: row.try_get("target_name")?,
            target_kind: row.try_get("target_kind")?,
            rule_revision_ref: row.try_get("rule_revision_ref")?,
            outcome: row.try_get("outcome")?,
            reason_code: row.try_get("reason_code")?,
            next_eligible_at: row.try_get("next_eligible_at")?,
            work_order_ref: row.try_get("work_order_ref")?,
            lease_ref: row.try_get("lease_ref")?,
            decided_at: row.try_get("decided_at")?,
        })
    })
    .collect()
}

async fn read_frozen_works(
    database: &Database,
    limit: i64,
) -> Result<Vec<FrozenWorkView>, sqlx::Error> {
    sqlx::query(
        "WITH recent_work AS ( \
             SELECT candidate.* FROM collection_work_order candidate \
             ORDER BY candidate.created_at DESC,candidate.work_order_ref LIMIT $1) \
         SELECT work.work_order_ref,work.target_ref, \
                COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key) AS target_name, \
                work.lane,work.station_ref,work.installation_ref,work.account_ref,work.eligibility_ref, \
                work.monitor_rule_revision_ref,lease.lease_ref, \
                CASE WHEN lease.lease_ref IS NULL THEN 'not_issued' \
                     WHEN lease.released_at IS NOT NULL THEN 'released' \
                     WHEN lease.expires_at<=scope_001_now() THEN 'expired' ELSE 'live' END AS lease_state, \
                task.task_id,task.execution_state AS task_state, \
                CASE WHEN work.station_ref IS NULL THEN NULL \
                     ELSE station.retired_at IS NULL END AS station_is_current, \
                CASE WHEN work.installation_ref IS NULL THEN NULL ELSE EXISTS ( \
                    SELECT 1 FROM plugin_installation current_installation \
                    WHERE current_installation.installation_ref=work.installation_ref \
                      AND current_installation.station_ref=work.station_ref \
                      AND current_installation.superseded_at IS NULL) END AS installation_is_current, \
                CASE WHEN work.account_ref IS NULL THEN NULL ELSE EXISTS ( \
                    SELECT 1 FROM platform_observation_account_binding current_binding \
                    WHERE current_binding.account_ref=work.account_ref \
                      AND current_binding.installation_ref=work.installation_ref \
                      AND current_binding.ended_at IS NULL) END AS account_is_current, \
                CASE WHEN work.monitor_rule_revision_ref IS NULL THEN NULL \
                     ELSE target.active_monitor_rule_revision_ref=work.monitor_rule_revision_ref \
                     END AS rule_is_current,work.created_at::text AS created_at \
         FROM recent_work work \
         JOIN collection_observation_target target USING(target_ref) \
         LEFT JOIN execution_station station ON station.station_ref=work.station_ref \
         LEFT JOIN LATERAL ( \
             SELECT candidate.lease_ref,candidate.expires_at,candidate.released_at,candidate.issued_at \
             FROM collection_work_order_lease candidate \
             WHERE candidate.work_order_ref=work.work_order_ref \
             ORDER BY candidate.issued_at DESC LIMIT 1) lease ON true \
         LEFT JOIN collection_work_order_lease_task task ON task.lease_ref=lease.lease_ref \
         ORDER BY work.created_at DESC,work.work_order_ref,task.sequence_no",
    )
    .bind(limit)
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(|row| {
        Ok(FrozenWorkView {
            work_order_ref: row.try_get("work_order_ref")?,
            target_ref: row.try_get("target_ref")?,
            target_name: row.try_get("target_name")?,
            lane: row.try_get("lane")?,
            station_ref: row.try_get("station_ref")?,
            installation_ref: row.try_get("installation_ref")?,
            account_ref: row.try_get("account_ref")?,
            eligibility_ref: row.try_get("eligibility_ref")?,
            monitor_rule_revision_ref: row.try_get("monitor_rule_revision_ref")?,
            lease_ref: row.try_get("lease_ref")?,
            lease_state: row.try_get("lease_state")?,
            task_id: row.try_get("task_id")?,
            task_state: row.try_get("task_state")?,
            station_is_current: row.try_get("station_is_current")?,
            installation_is_current: row.try_get("installation_is_current")?,
            account_is_current: row.try_get("account_is_current")?,
            rule_is_current: row.try_get("rule_is_current")?,
            created_at: row.try_get("created_at")?,
        })
    })
    .collect()
}

async fn read_runtime_resources(
    database: &Database,
) -> Result<Vec<RuntimeResourceView>, sqlx::Error> {
    sqlx::query(
        "SELECT station.station_ref,station.display_name AS station_name,station.accepting_tasks, \
                installation.installation_ref,installation.plugin_version, \
                installation.last_seen_at::text AS last_seen_at, \
                COALESCE(EXISTS (SELECT 1 FROM installation_credential credential \
                    WHERE credential.installation_ref=installation.installation_ref \
                      AND credential.revoked_at IS NULL AND credential.activated_at IS NOT NULL \
                      AND credential.expires_at>scope_001_now()),false) AS has_valid_credential, \
                COALESCE(eligibility.account_ref,binding.account_ref) AS account_ref, \
                binding.account_ref AS bound_account_ref, \
                CASE WHEN eligibility.account_ref IS NULL AND binding.account_ref IS NULL THEN 'missing' \
                     WHEN eligibility.account_ref IS NULL THEN 'bound_without_eligibility' \
                     WHEN binding.account_ref IS NULL THEN 'unconfirmed' \
                     WHEN binding.account_ref<>eligibility.account_ref THEN 'changed' \
                     ELSE 'current' END AS binding_state, \
                eligibility.eligibility_ref, \
                eligibility.eligibility_state,eligibility.reason_code AS eligibility_reason_code, \
                eligibility.observed_at::text AS eligibility_observed_at, \
                COALESCE(CASE WHEN COALESCE(eligibility.account_ref,binding.account_ref) IS NULL THEN false ELSE EXISTS ( \
                    SELECT 1 FROM collection_work_order busy_work \
                    JOIN collection_work_order_lease busy_lease USING(work_order_ref) \
                    WHERE busy_work.account_ref=COALESCE(eligibility.account_ref,binding.account_ref) \
                      AND busy_lease.released_at IS NULL \
                      AND busy_lease.expires_at>scope_001_now()) END,false) AS account_has_live_lease \
         FROM execution_station station \
         LEFT JOIN plugin_installation installation \
           ON installation.station_ref=station.station_ref AND installation.superseded_at IS NULL \
         LEFT JOIN LATERAL ( \
             SELECT observation.eligibility_ref,observation.account_ref, \
                    observation.eligibility_state,observation.reason_code, \
                    observation.observed_at \
             FROM platform_observation_account_eligibility_observation observation \
             WHERE observation.installation_ref=installation.installation_ref \
             ORDER BY observation.observed_at DESC LIMIT 1) eligibility ON true \
         LEFT JOIN LATERAL ( \
             SELECT candidate.account_ref \
             FROM platform_observation_account_binding candidate \
             WHERE candidate.installation_ref=installation.installation_ref \
               AND candidate.ended_at IS NULL LIMIT 1) binding ON true \
         WHERE station.retired_at IS NULL ORDER BY station.registered_at,station.station_ref",
    )
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(|row| {
        Ok(RuntimeResourceView {
            station_ref: row.try_get("station_ref")?,
            station_name: row.try_get("station_name")?,
            accepting_tasks: row.try_get("accepting_tasks")?,
            installation_ref: row.try_get("installation_ref")?,
            plugin_version: row.try_get("plugin_version")?,
            last_seen_at: row.try_get("last_seen_at")?,
            has_valid_credential: row.try_get("has_valid_credential")?,
            account_ref: row.try_get("account_ref")?,
            bound_account_ref: row.try_get("bound_account_ref")?,
            binding_state: row.try_get("binding_state")?,
            eligibility_ref: row.try_get("eligibility_ref")?,
            eligibility_state: row.try_get("eligibility_state")?,
            eligibility_reason_code: row.try_get("eligibility_reason_code")?,
            eligibility_observed_at: row.try_get("eligibility_observed_at")?,
            account_has_live_lease: row.try_get("account_has_live_lease")?,
        })
    })
    .collect()
}

fn runtime_lane(
    label: &'static str,
    target_kind: &'static str,
    lane: &'static str,
    capacity: Capacity,
) -> RuntimeLaneControlView {
    match capacity {
        Capacity::Queueable => RuntimeLaneControlView {
            label,
            target_kind,
            lane,
            queueable: true,
            available: false,
            reason_code: None,
            reason: Some("准入允许入队；等待满足资格的工位主动认领。".to_owned()),
            station_ref: None,
        },
        Capacity::Available { station_ref } => RuntimeLaneControlView {
            label,
            target_kind,
            lane,
            queueable: false,
            available: true,
            reason_code: None,
            reason: None,
            station_ref: Some(station_ref),
        },
        Capacity::NoStaffedStation => blocked_lane(
            label,
            target_kind,
            lane,
            "station_unavailable",
            "没有任何登记工位拥有在岗安装。".to_owned(),
        ),
        Capacity::MissingCapabilities { missing } => blocked_lane(
            label,
            target_kind,
            lane,
            "capability_missing",
            format!("在岗安装缺少本次 lane 所需能力：{}。", missing.join("、")),
        ),
        Capacity::DailyQuotaCommitted { quota, committed } => blocked_lane(
            label,
            target_kind,
            lane,
            "station_daily_budget_reached",
            format!("当天额度已被既有工单占满：每日 {quota} 篇，已下发 {committed} 篇。"),
        ),
        Capacity::RiskPaused { reason } => blocked_lane(
            label,
            target_kind,
            lane,
            "risk_paused",
            format!("风险暂停仍在生效：{reason}"),
        ),
        Capacity::Unavailable {
            reason_code,
            reason,
        } => blocked_lane(label, target_kind, lane, reason_code.as_str(), reason),
    }
}

fn blocked_lane(
    label: &'static str,
    target_kind: &'static str,
    lane: &'static str,
    reason_code: &'static str,
    reason: String,
) -> RuntimeLaneControlView {
    RuntimeLaneControlView {
        label,
        target_kind,
        lane,
        queueable: false,
        available: false,
        reason_code: Some(reason_code),
        reason: Some(reason),
        station_ref: None,
    }
}

pub fn render_operations(
    base: &str,
    projection: &CollectionControlSurfaceProjection,
    mode: OperationsMode,
) -> String {
    if mode != OperationsMode::Now {
        return base.to_owned();
    }
    let content = operations_markup(projection);
    let rendered = if base.contains(BODY_SLOT_START) {
        replace_bounded_slot(base, BODY_SLOT_START, BODY_SLOT_END, &content)
    } else {
        replace_empty(base, &content)
    };
    replace_bounded_slot(
        &rendered,
        READOUT_SLOT_START,
        READOUT_SLOT_END,
        &operations_readout(projection),
    )
}

pub fn render_attention(base: &str, projection: &CollectionControlSurfaceProjection) -> String {
    let content = attention_markup(projection);
    let rendered = if base.contains(BODY_SLOT_START) {
        replace_bounded_slot(base, BODY_SLOT_START, BODY_SLOT_END, &content)
    } else {
        replace_empty(base, &content)
    };
    replace_bounded_slot(
        &rendered,
        READOUT_SLOT_START,
        READOUT_SLOT_END,
        &attention_readout(projection),
    )
}

pub fn render_tasks_control(base: &str, projection: &CollectionControlSurfaceProjection) -> String {
    let templates = frozen_work_templates(&projection.works);
    let rendered = selected_task_id(base).map_or_else(
        || base.to_owned(),
        |task_id| {
            let works = works_for_task(&projection.works, task_id);
            replace_bounded_slot(
                base,
                "<!-- frozen-work:start -->",
                "<!-- frozen-work:end -->",
                &frozen_work_markup(&works),
            )
        },
    );
    prepend_body(&rendered, &templates)
}

pub fn render_tasks_control_unavailable(base: &str, state: TaskControlUnavailable) -> String {
    let detail = match state {
        TaskControlUnavailable::SchemaUnavailable => {
            "冻结 Work 所需的控制 schema 尚未就绪；当前任务、Attempt、Package 与 Receipt 仍可读，但不能核对准入时冻结的资源引用。请先完成受控 schema 就绪检查。"
        }
        TaskControlUnavailable::ReadFailed => {
            "冻结 Work 控制投影本次读取失败；当前任务、Attempt、Package 与 Receipt 仍可读，但不能核对准入时冻结的资源引用。请恢复本机数据库读取后重试。"
        }
    };
    replace_bounded_slot(
        base,
        "<!-- frozen-work:start -->",
        "<!-- frozen-work:end -->",
        &format!(
            r#"<section class="c-control-empty" data-task-control-unavailable><h2>冻结 Work 投影当前不可用</h2><p>{detail}</p></section>"#
        ),
    )
}

pub fn render_runtime_control(
    base: &str,
    projection: &CollectionControlSurfaceProjection,
) -> String {
    prepend_body(base, &runtime_control_markup(projection))
}

fn operations_markup(projection: &CollectionControlSurfaceProjection) -> String {
    let run = projection.latest_run.as_ref().map_or_else(
        || {
            r#"<section class="c-control-empty"><h2>尚无调度轮次</h2><p>调度轮次读模型已接通，当前读取范围没有持久 scheduler run；这不是失败，也不触发采集。</p></section>"#.to_owned()
        },
        |run| {
            format!(
                r#"<section class="c-control-run" data-scheduler-run="{run_ref}">
                     <div><p>最近一轮 · {started}</p><h2>{outcome}</h2></div>
                     <dl><div><dt>考虑目标</dt><dd>{considered}</dd></div><div><dt>派出</dt><dd>{dispatched}</dd></div><div><dt>完成</dt><dd>{completed}</dd></div></dl>
                   </section>"#,
                run_ref = run.scheduler_run_ref,
                started = escape(&run.started_at),
                outcome = escape(run.outcome.as_deref().unwrap_or("RUNNING")),
                considered = run.considered_count,
                dispatched = run.dispatched_count,
                completed = escape(run.completed_at.as_deref().unwrap_or("尚未完成")),
            )
        },
    );
    let decisions = if projection.decisions.is_empty() {
        r#"<div class="c-decision-stream-empty"><b>当前读取范围没有持久目标决定</b><p>这不是“世界没有变化”，只表示最近的 scheduler decision 投影为空。</p></div>"#.to_owned()
    } else {
        projection
            .decisions
            .iter()
            .map(decision_row)
            .collect::<String>()
    };
    format!(
        r#"<section class="c-control-surface c-operations-v4" data-collection-control="operations">
             {run}
             <div class="c-operations-workspace">
               <div class="c-flow-ledger">
                 <div class="c-control-section-head"><div><p>最近持久投影</p><h2>观察生产流</h2></div><span>不是转化漏斗</span></div>
                 <p class="c-control-boundary">各阶段读数分别来自最近的 decision 与 Work 冻结投影；它们不是同一批对象的完成率，也不代表平台全量。</p>
                 <div class="c-flow-v4">{flow}</div>
               </div>
               <aside class="c-decision-stream" aria-label="持久调度决定流">
                 <div class="c-decision-stream-head"><div><i></i><b>调度决定流</b></div><span>最近 {count} 条持久记录</span></div>
                 <div class="c-decision-stream-feed">{decisions}</div>
                 <div class="c-decision-stream-foot"><span>仅控制事实</span><span>语料内容与价值评分归属语料页</span></div>
               </aside>
             </div>
           </section>"#,
        count = projection.decisions.len(),
        flow = flow_stage_markup(projection),
    )
}

fn operations_readout(projection: &CollectionControlSurfaceProjection) -> String {
    let runs = if projection.latest_run.is_some() { "1" } else { "—" };
    let recoverable = projection
        .decisions
        .iter()
        .filter(|decision| recovery_for(&decision.reason_code).is_some())
        .count()
        .to_string();
    context_readout(&[
        (runs, "最近一轮", "最近持久调度读取范围"),
        (&recoverable, "有恢复动作", "仅计入存在明确恢复责任的决定"),
    ])
}

fn flow_stage_markup(projection: &CollectionControlSurfaceProjection) -> String {
    let stages = [
        ("01", "目标判定", "调度为目标写下决定", projection.decisions.len()),
        (
            "02",
            "规则准入",
            "决定冻结规则版本",
            projection
                .decisions
                .iter()
                .filter(|decision| decision.rule_revision_ref.is_some())
                .count(),
        ),
        (
            "03",
            "建立工单",
            "准入后形成 WorkOrder",
            projection
                .decisions
                .iter()
                .filter(|decision| decision.work_order_ref.is_some())
                .count(),
        ),
        (
            "04",
            "签发租约",
            "Work 获得独占 Lease",
            projection
                .decisions
                .iter()
                .filter(|decision| decision.lease_ref.is_some())
                .count(),
        ),
        (
            "05",
            "任务执行",
            "Lease 已映射到 Task",
            projection
                .works
                .iter()
                .filter(|work| work.task_id.is_some())
                .count(),
        ),
        (
            "06",
            "恢复与重排",
            "决定存在但尚无 Work",
            projection
                .decisions
                .iter()
                .filter(|decision| recovery_for(&decision.reason_code).is_some())
                .count(),
        ),
    ];
    stages
        .into_iter()
        .map(|(number, name, note, count)| {
            format!(
                r#"<article class="c-flow-v4-row"><span class="c-flow-v4-no">{number}</span><div><h3>{name}</h3><p>{note}</p></div><div class="c-flow-v4-state"><b>{count}</b><span>最近读取</span></div></article>"#,
            )
        })
        .collect()
}

fn decision_row(decision: &SchedulerDecisionView) -> String {
    let refs = match (decision.work_order_ref, decision.lease_ref) {
        (Some(work), Some(lease)) => {
            format!("Work {} · Lease {}", short_ref(work), short_ref(lease))
        }
        (Some(work), None) => format!("Work {} · 尚无 Lease", short_ref(work)),
        _ => "未建立 Work / Lease".to_owned(),
    };
    format!(
        r#"<article class="c-decision-event" data-decision-outcome="{outcome}" data-control-reason="{reason}">
             <time>{time}</time><div class="c-decision-kind">{kind}</div>
             <div class="c-decision-copy"><h3>{target}</h3><p>{refs}</p><span>{next}</span></div>
             <div class="c-control-outcome"><b>{outcome}</b><code>{reason}</code></div>
             <span class="v7-sr-only" data-rule-revision-ref>{rule_ref}</span>
           </article>"#,
        outcome = escape(&decision.outcome),
        reason = escape(&decision.reason_code),
        kind = target_kind_label(&decision.target_kind),
        time = escape(&decision.decided_at),
        target = escape(&decision.target_name),
        refs = escape(&refs),
        next = escape(
            decision
                .next_eligible_at
                .as_deref()
                .map_or("没有下一次资格时间", |value| value)
        ),
        rule_ref = optional_ref(decision.rule_revision_ref),
    )
}

fn attention_markup(projection: &CollectionControlSurfaceProjection) -> String {
    let entries = attention_entries(projection);
    let body = if entries.is_empty() {
        r#"<section class="c-control-empty"><h2>当前没有可恢复的真实阻断</h2><p>控制投影读取成功，当前范围没有带恢复动作的阻断。DYNAMIC_UNAVAILABLE、not_due、manual_only 与 PARTIAL + VALID 不会被冒充为失败。</p></section>"#.to_owned()
    } else {
        entries.iter().map(attention_row).collect::<String>()
    };
    let inspector = entries.first().map_or_else(
        || {
            r#"<aside class="c-attention-inspector c-attention-inspector-empty"><p>当前没有可选择的阻断。</p></aside>"#.to_owned()
        },
        attention_inspector,
    );
    format!(
        r#"<section class="c-control-surface" data-collection-control="attention">
             <div class="c-control-section-head"><div><p>仅持久控制事实</p><h2>有恢复动作的阻断</h2></div><span>{count} 项</span></div>
             <div class="c-attention-workspace">
               <div class="c-attention-ledger">
                 <div class="c-attention-head"><span>来源</span><span>对象</span><span>阻断</span><span>责任</span><span>观察时间</span></div>
                 <div class="c-attention-list">{body}</div>
               </div>
               {inspector}
             </div>
           </section>"#,
        count = entries.len(),
    )
}

fn attention_readout(projection: &CollectionControlSurfaceProjection) -> String {
    let entries = attention_entries(projection);
    let count_owner = |owner: &str| entries.iter().filter(|entry| entry.owner == owner).count();
    context_readout(&[
        (&entries.len().to_string(), "需要处理", "当前有明确恢复动作的持久阻断"),
        (&count_owner("你").to_string(), "你负责", "当前需要人工处理的恢复事项"),
    ])
}

fn attention_entries(projection: &CollectionControlSurfaceProjection) -> Vec<AttentionEntry> {
    let mut entries = Vec::new();
    for lane in &projection.runtime_lanes {
        if let Some(reason_code) = lane.reason_code
            && let Some(recovery) = recovery_for(reason_code)
        {
            entries.push(AttentionEntry::new(
                lane.label,
                reason_code,
                lane.reason.as_deref().unwrap_or("控制闸门关闭。"),
                recovery,
                "当前 capacity",
            ));
        }
    }
    for decision in &projection.decisions {
        if let Some(recovery) = recovery_for(&decision.reason_code) {
            entries.push(AttentionEntry::new(
                &decision.target_name,
                &decision.reason_code,
                decision_reason(&decision.reason_code),
                recovery,
                &decision.decided_at,
            ));
        }
    }
    entries
}

#[derive(Clone, Copy)]
struct Recovery {
    owner: &'static str,
    action: &'static str,
}

struct AttentionEntry {
    title: String,
    reason: String,
    detail: String,
    owner: &'static str,
    action: &'static str,
    observed: String,
}

impl AttentionEntry {
    fn new(title: &str, reason: &str, detail: &str, recovery: Recovery, observed: &str) -> Self {
        Self {
            title: title.to_owned(),
            reason: reason.to_owned(),
            detail: detail.to_owned(),
            owner: recovery.owner,
            action: recovery.action,
            observed: observed.to_owned(),
        }
    }
}

fn recovery_for(reason: &str) -> Option<Recovery> {
    match reason {
        "rule_missing" => Some(Recovery {
            owner: "你",
            action: "打开目标的监控规则并保存首个版本。",
        }),
        // Retained historical decision vocabulary.  It has no recovery action
        // in the current product: archive coverage no longer gates observation.
        "baseline_not_ready" => None,
        "risk_paused" => Some(Recovery {
            owner: "你",
            action: "核对风险暂停；确认风险解除后再恢复。",
        }),
        "station_unavailable" => Some(Recovery {
            owner: "你",
            action: "登记或认领一台在岗执行工位。",
        }),
        "station_not_accepting" => Some(Recovery {
            owner: "你",
            action: "该工位被显式暂停；确认后在执行工位页恢复自动接活。",
        }),
        "installation_credential_missing" => Some(Recovery {
            owner: "你",
            action: "让在岗安装重新报到并完成服务端凭据激活。",
        }),
        "plugin_version_unsupported" => Some(Recovery {
            owner: "你",
            action: "更新插件到当前最低合同版本。",
        }),
        "installation_stale" => Some(Recovery {
            owner: "执行工位",
            action: "恢复插件心跳；过期状态不会自动当成健康。",
        }),
        "capability_missing" => Some(Recovery {
            owner: "你",
            action: "换到具备该 lane 能力的在岗安装。",
        }),
        "account_unbound" => Some(Recovery {
            owner: "你",
            action: "把安装绑定到一个平台观察账号。",
        }),
        "account_binding_changed" | "account_binding_expired" => Some(Recovery {
            owner: "你",
            action: "重新核对并确认安装的观察账号绑定。",
        }),
        "account_eligibility_stale" => Some(Recovery {
            owner: "诊断记录",
            action: "这是历史兼容状态；当前不以观察时间单独限制接活。",
        }),
        "account_cooling" => Some(Recovery {
            owner: "账号",
            action: "等待冷却解除后由新鲜信号重判。",
        }),
        "account_needs_login" => Some(Recovery {
            owner: "你",
            action: "在受控浏览器中重新登录该观察账号。",
        }),
        "account_restricted" => Some(Recovery {
            owner: "你",
            action: "停止使用受限账号，核对平台侧限制。",
        }),
        "account_unknown" => Some(Recovery {
            owner: "执行工位",
            action: "补充可判定的账号资格信号；UNKNOWN 保持关闭。",
        }),
        "account_busy" => Some(Recovery {
            owner: "调度",
            action: "等待这个账号的当前 Lease 结束。",
        }),
        "station_daily_budget_reached" => Some(Recovery {
            owner: "调度",
            action: "等待下一个自然日预算窗口。",
        }),
        "admission_refused" | "lease_issue_failed" | "database_error" => Some(Recovery {
            owner: "工程",
            action: "沿 durable decision / Work / Lease 引用排查，不在页面重试写事实。",
        }),
        _ => None,
    }
}

fn attention_row(entry: &AttentionEntry) -> String {
    format!(
        r#"<button type="button" class="c-attention-row" data-attention-row data-control-reason="{reason}" data-title="{title}" data-detail="{detail}" data-owner="{owner}" data-action="{action}" data-observed="{observed}">
             <span class="c-attention-source"><i></i>控制阻断</span><span class="c-attention-target">{title}</span><code>{reason}</code><b>{owner}</b><time>{observed}</time>
           </button>"#,
        reason = escape(&entry.reason),
        observed = escape(&entry.observed),
        title = escape(&entry.title),
        detail = escape(&entry.detail),
        owner = entry.owner,
        action = escape(entry.action),
    )
}

fn attention_inspector(entry: &AttentionEntry) -> String {
    format!(
        r#"<aside class="c-attention-inspector" data-attention-inspector aria-live="polite">
             <div class="c-attention-inspector-head"><span data-attention-reason>{reason}</span><h2 data-attention-title>{title}</h2><p data-attention-observed>{observed}</p></div>
             <div class="c-attention-inspector-body">
               <div class="c-state-line"><span><i></i>有恢复动作</span><span>{owner}负责</span></div>
               <section><h3>为什么需要处理</h3><p data-attention-detail>{detail}</p></section>
               <section><h3>恢复责任</h3><dl><div><dt>责任</dt><dd data-attention-owner>{owner}</dd></div><div><dt>下一步</dt><dd data-attention-action>{action}</dd></div></dl></section>
               <section class="c-attention-boundary"><h3>边界</h3><p>这里仅显示持久控制阻断；语料内容、价值评分与影响数量不属于这个页面。</p></section>
             </div>
           </aside>"#,
        reason = escape(&entry.reason),
        title = escape(&entry.title),
        observed = escape(&entry.observed),
        owner = entry.owner,
        detail = escape(&entry.detail),
        action = escape(entry.action),
    )
}

fn frozen_work_markup(works: &[FrozenWorkView]) -> String {
    let rows = if works.is_empty() {
        r#"<p class="c-control-none">该任务没有关联的冻结 Work 投影。</p>"#.to_owned()
    } else {
        works.iter().map(frozen_work_row).collect::<String>()
    };
    format!(
        r#"<section class="c-control-surface c-frozen-works" data-collection-control="tasks">
             <div class="c-control-section-head"><div><p>冻结来源链</p><h2>工单冻结资源</h2></div><span>当前任务 {count} 张</span></div>
             <p class="c-control-boundary">冻结引用说明准入当时依赖什么；“当前不同”不会覆盖旧 Work、Attempt 或 Receipt。</p>
             <div class="c-control-list">{rows}</div>
           </section>"#,
        count = works.len(),
    )
}

fn works_for_task(works: &[FrozenWorkView], task_id: Uuid) -> Vec<FrozenWorkView> {
    works
        .iter()
        .filter(|work| work.task_id == Some(task_id))
        .cloned()
        .collect()
}

fn selected_task_id(base: &str) -> Option<Uuid> {
    const ATTRIBUTE: &str = "data-task-id=\"";
    let start = base.find(ATTRIBUTE)? + ATTRIBUTE.len();
    let end = base[start..].find('"')? + start;
    Uuid::parse_str(&base[start..end]).ok()
}

fn frozen_work_templates(works: &[FrozenWorkView]) -> String {
    let mut task_ids = works.iter().filter_map(|work| work.task_id).collect::<Vec<_>>();
    task_ids.sort_unstable();
    task_ids.dedup();
    task_ids
        .into_iter()
        .map(|task_id| {
            let scoped = works_for_task(works, task_id);
            format!(
                r#"<template data-task-frozen-template="{task_id}">{content}</template>"#,
                content = frozen_work_markup(&scoped),
            )
        })
        .collect()
}

fn frozen_work_row(work: &FrozenWorkView) -> String {
    format!(
        r#"<article class="c-frozen-row" data-work-order-ref="{work_ref}">
             <div class="c-frozen-head"><div><p>{lane} · {created}</p><h3>{target}</h3></div><span>{lease_state}{task_state}</span></div>
             <dl class="c-frozen-grid">
               {station}{installation}{account}{eligibility}{rule}
             </dl>
             <p class="c-frozen-links">Work {work_short} · Lease {lease_ref} · Task {task_ref}</p>
           </article>"#,
        work_ref = work.work_order_ref,
        lane = escape(lane_label(&work.lane)),
        created = escape(&work.created_at),
        target = escape(&work.target_name),
        lease_state = escape(lease_label(&work.lease_state)),
        task_state = work
            .task_state
            .as_deref()
            .map_or(String::new(), |state| format!(" · {}", escape(state))),
        station = frozen_cell(
            "工位",
            "data-frozen-station-ref",
            work.station_ref,
            work.station_is_current
        ),
        installation = frozen_cell(
            "安装",
            "data-frozen-installation-ref",
            work.installation_ref,
            work.installation_is_current
        ),
        account = frozen_cell(
            "观察账号",
            "data-frozen-account-ref",
            work.account_ref,
            work.account_is_current
        ),
        eligibility = frozen_cell(
            "资格回执",
            "data-frozen-eligibility-ref",
            work.eligibility_ref,
            None,
        ),
        rule = frozen_cell(
            "规则版本",
            "data-frozen-rule-ref",
            work.monitor_rule_revision_ref,
            work.rule_is_current
        ),
        work_short = short_ref(work.work_order_ref),
        lease_ref = optional_short_ref(work.lease_ref),
        task_ref = optional_short_ref(work.task_id),
    )
}

fn frozen_cell(label: &str, selector: &str, value: Option<Uuid>, current: Option<bool>) -> String {
    let state = match current {
        Some(true) => "当前一致",
        Some(false) => "当前已变化",
        None => "当时未冻结",
    };
    format!(
        r#"<div {selector}><dt>{label}</dt><dd>{value}</dd><span>{state}</span></div>"#,
        value = optional_short_ref(value),
    )
}

fn runtime_control_markup(projection: &CollectionControlSurfaceProjection) -> String {
    let lanes = projection
        .runtime_lanes
        .iter()
        .map(runtime_lane_row)
        .collect::<String>();
    let resources = if projection.runtime_resources.is_empty() {
        r#"<p class="c-control-none">当前没有未退役工位；这不是账号健康结论。</p>"#.to_owned()
    } else {
        projection
            .runtime_resources
            .iter()
            .map(runtime_resource_row)
            .collect::<String>()
    };
    format!(
        r#"<section class="c-control-surface c-runtime-control" data-collection-control="runtime">
             <div class="c-control-section-head"><div><p>准入第 5 问・同一评估器</p><h2>当前控制资格</h2></div><span>实时重算</span></div>
             <div class="c-runtime-lanes">{lanes}</div>
             <div class="c-control-section-head c-control-subhead"><div><p>有界控制事实</p><h2>工位 / 安装 / 观察账号</h2></div><span>不含原始账号身份</span></div>
             <div class="c-runtime-resources">{resources}</div>
           </section>"#,
    )
}

fn runtime_lane_row(lane: &RuntimeLaneControlView) -> String {
    let (state, reason, capacity_state) = if lane.available {
        ("可接活", "capacity_available", "available")
    } else if lane.queueable {
        ("可入队，待工位认领", "capacity_queueable", "queueable")
    } else {
        (
            "关闭",
            lane.reason_code.unwrap_or("capacity_unknown"),
            "blocked",
        )
    };
    format!(
        r#"<article class="c-runtime-lane" data-capacity-state="{available}" data-control-reason="{reason}">
             <div><p>{kind} / {lane}</p><h3>{label}</h3></div>
             <div><b>{state}</b><code>{reason}</code><span>{detail}</span></div>
           </article>"#,
        available = capacity_state,
        reason = escape(reason),
        kind = target_kind_label(lane.target_kind),
        lane = escape(lane_label(lane.lane)),
        label = lane.label,
        state = state,
        detail = escape(
            lane.reason
                .as_deref()
                .or(lane.station_ref.as_deref())
                .unwrap_or("当前判定没有返回详情。")
        ),
    )
}

fn runtime_resource_row(resource: &RuntimeResourceView) -> String {
    let eligibility_reason = resource
        .eligibility_reason_code
        .as_deref()
        .unwrap_or("account_unknown");
    let account_state = resource.eligibility_state.as_deref().unwrap_or("UNKNOWN");
    let acceptance_form = format!(
        r#"<form class="c-runtime-control-form" method="post" action="/collection/runtime/accepting" data-station-accepting-form>
             <input type="hidden" name="station_ref" value="{station_ref}">
             <button class="c-btn-quiet" type="submit" name="accepting" value="{next}">{label}</button>
           </form>"#,
        station_ref = resource.station_ref,
        next = !resource.accepting_tasks,
        label = if resource.accepting_tasks {
            "暂停未来接活"
        } else {
            "恢复自动接活"
        },
    );
    let binding_form = match (
        resource.installation_ref,
        resource.account_ref,
        resource.binding_state.as_str(),
    ) {
        (Some(installation_ref), Some(account_ref), state) if binding_required(state) => format!(
            r#"<form class="c-runtime-control-form" method="post" action="/collection/runtime/account-bindings" data-account-binding-form>
                 <input type="hidden" name="installation_ref" value="{installation_ref}">
                 <input type="hidden" name="account_ref" value="{account_ref}">
                 <button class="c-btn-quiet" type="submit">确认这个观察账号</button>
               </form>"#,
        ),
        _ => String::new(),
    };
    format!(
        r#"<article class="c-runtime-resource" data-station-ref="{station_ref}" data-control-fact="runtime-resource">
             <div class="c-runtime-resource-head"><div><p>工位</p><h3>{name}</h3></div><div><b>{accepting}</b>{acceptance_form}</div></div>
             <dl>
               <div><dt>安装</dt><dd>{installation}</dd><span>{version} · 心跳 {last_seen}</span></div>
               <div><dt>服务端凭据</dt><dd>{credential}</dd><span>只显示有效性，不显示密钥或摘要</span></div>
               <div data-account-binding-required="{binding_required}"><dt>观察账号</dt><dd>{account}</dd><span>{binding_state} · 当前绑定 {bound_account} · 仅人工替换或结束会改变绑定</span>{binding_form}</div>
               <div data-account-eligibility-reason="{reason}"><dt>账号资格</dt><dd>{account_state}</dd><span>{reason} · 最后观察 {observed}（仅供排障，不因时间经过阻断接活）{busy} · Eligibility {eligibility_ref}</span></div>
             </dl>
           </article>"#,
        station_ref = resource.station_ref,
        reason = escape(eligibility_reason),
        name = escape(&resource.station_name),
        accepting = if resource.accepting_tasks {
            "自动接活"
        } else {
            "已暂停"
        },
        acceptance_form = acceptance_form,
        installation = optional_short_ref(resource.installation_ref),
        version = escape(resource.plugin_version.as_deref().unwrap_or("版本未知")),
        last_seen = escape(resource.last_seen_at.as_deref().unwrap_or("UNKNOWN")),
        credential = if resource.has_valid_credential {
            "有效"
        } else {
            "缺失 / 失效"
        },
        account = optional_short_ref(resource.account_ref),
        bound_account = optional_short_ref(resource.bound_account_ref),
        binding_state = escape(binding_state_label(&resource.binding_state)),
        binding_required = binding_required(&resource.binding_state),
        account_state = escape(account_state),
        observed = escape(
            resource
                .eligibility_observed_at
                .as_deref()
                .unwrap_or("UNKNOWN")
        ),
        busy = if resource.account_has_live_lease {
            " · 已有有效 Lease"
        } else {
            ""
        },
        eligibility_ref = optional_short_ref(resource.eligibility_ref),
        binding_form = binding_form,
    )
}

fn replace_empty(base: &str, content: &str) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();
    format!("{}{content}{}", &base[..open], &base[close..])
}

fn prepend_body(base: &str, content: &str) -> String {
    let Some(body) = base.find(BODY_OPEN) else {
        return base.to_owned();
    };
    let at = body + BODY_OPEN.len();
    format!("{}{content}{}", &base[..at], &base[at..])
}

fn target_kind_label(value: &str) -> &'static str {
    if value == "keyword" {
        "关键词"
    } else {
        "创作者"
    }
}

fn lane_label(value: &str) -> &'static str {
    match value {
        "deep_archive" => "基线建档",
        "patrol" => "巡检",
        _ => "未知 lane",
    }
}

fn lease_label(value: &str) -> &'static str {
    match value {
        "live" => "Lease 有效",
        "released" => "Lease 已结束",
        "expired" => "Lease 已过期",
        _ => "尚无 Lease",
    }
}

fn decision_reason(value: &str) -> &'static str {
    match value {
        "rule_missing" => "目标还没有活动规则版本。",
        "baseline_not_ready" => "历史基线门禁记录；当前规则不会因建档完整度而停止观察。",
        "station_not_accepting" => "工位已由人显式暂停未来接活。",
        "account_needs_login" => "观察账号需要重新登录。",
        "account_restricted" => "观察账号受到访问限制。",
        "account_cooling" => "观察账号当前处于冷却状态。",
        "account_unknown" => "观察账号资格是 UNKNOWN，控制层按关闭处理。",
        "database_error" => "scheduler 写下了数据库错误决定，没有伪造成功。",
        _ => "scheduler 写下了一个有明确恢复责任的控制阻断。",
    }
}

fn binding_state_label(value: &str) -> &'static str {
    match value {
        "current" => "当前一致",
        "unconfirmed" => "未确认",
        "changed" => "观察账号已变化",
        "bound_without_eligibility" => "已绑定，资格信号缺失",
        _ => "尚无观察账号",
    }
}

fn binding_required(value: &str) -> bool {
    matches!(value, "unconfirmed" | "changed")
}

fn optional_ref(value: Option<Uuid>) -> String {
    value.map_or_else(|| "NONE".to_owned(), |value| value.to_string())
}

fn optional_short_ref(value: Option<Uuid>) -> String {
    value.map_or_else(|| "—".to_owned(), short_ref)
}

fn short_ref(value: Uuid) -> String {
    value.to_string().chars().take(8).collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection() -> CollectionControlSurfaceProjection {
        CollectionControlSurfaceProjection {
            latest_run: Some(SchedulerRunView {
                scheduler_run_ref: Uuid::new_v4(),
                outcome: Some("partial".to_owned()),
                considered_count: 2,
                dispatched_count: 1,
                started_at: "2026-09-04 09:00:00+08".to_owned(),
                completed_at: Some("2026-09-04 09:00:01+08".to_owned()),
            }),
            decisions: vec![SchedulerDecisionView {
                target_decision_ref: Uuid::new_v4(),
                scheduler_run_ref: Uuid::new_v4(),
                target_ref: Uuid::new_v4(),
                target_name: "目标 <一>".to_owned(),
                target_kind: "creator".to_owned(),
                rule_revision_ref: Some(Uuid::new_v4()),
                outcome: "deferred".to_owned(),
                reason_code: "account_needs_login".to_owned(),
                next_eligible_at: None,
                work_order_ref: None,
                lease_ref: None,
                decided_at: "2026-09-04 09:00:00+08".to_owned(),
            }],
            works: vec![FrozenWorkView {
                work_order_ref: Uuid::new_v4(),
                target_ref: Uuid::new_v4(),
                target_name: "目标 <一>".to_owned(),
                lane: "patrol".to_owned(),
                station_ref: Some(Uuid::new_v4()),
                installation_ref: Some(Uuid::new_v4()),
                account_ref: Some(Uuid::new_v4()),
                eligibility_ref: Some(Uuid::new_v4()),
                monitor_rule_revision_ref: Some(Uuid::new_v4()),
                lease_ref: Some(Uuid::new_v4()),
                lease_state: "live".to_owned(),
                task_id: Some(Uuid::new_v4()),
                task_state: Some("in_progress".to_owned()),
                station_is_current: Some(true),
                installation_is_current: Some(false),
                account_is_current: Some(false),
                rule_is_current: Some(true),
                created_at: "2026-09-04 09:00:00+08".to_owned(),
            }],
            runtime_lanes: vec![blocked_lane(
                "创作者巡检",
                "creator",
                "patrol",
                "account_needs_login",
                "观察账号需要重新登录。".to_owned(),
            )],
            runtime_resources: vec![RuntimeResourceView {
                station_ref: Uuid::new_v4(),
                station_name: "Mac mini <主机>".to_owned(),
                accepting_tasks: true,
                installation_ref: Some(Uuid::new_v4()),
                plugin_version: Some("0.8.46".to_owned()),
                last_seen_at: Some("2026-09-04 09:00:00+08".to_owned()),
                has_valid_credential: true,
                account_ref: Some(Uuid::new_v4()),
                bound_account_ref: None,
                binding_state: "unconfirmed".to_owned(),
                eligibility_ref: Some(Uuid::new_v4()),
                eligibility_state: Some("needs_login".to_owned()),
                eligibility_reason_code: Some("login_required".to_owned()),
                eligibility_observed_at: Some("2026-09-04 09:00:00+08".to_owned()),
                account_has_live_lease: false,
            }],
        }
    }

    #[test]
    fn four_surfaces_share_stable_reason_and_frozen_reference_selectors() {
        let projection = projection();
        let empty = format!("{EMPTY_STATE_OPEN}old{EMPTY_STATE_CLOSE}");
        let operations = render_operations(&empty, &projection, OperationsMode::Now);
        let attention = render_attention(&empty, &projection);
        let body = format!("{BODY_OPEN}old</div>");
        let tasks = render_tasks_control(&body, &projection);
        let runtime = render_runtime_control(&body, &projection);

        assert!(operations.contains("data-control-reason=\"account_needs_login\""));
        assert!(operations.contains("data-rule-revision-ref"));
        assert!(attention.contains("data-control-reason=\"account_needs_login\""));
        for selector in [
            "data-frozen-station-ref",
            "data-frozen-installation-ref",
            "data-frozen-account-ref",
            "data-frozen-rule-ref",
        ] {
            assert!(tasks.contains(selector), "missing {selector}");
        }
        assert!(tasks.contains("当前已变化"));
        assert!(runtime.contains("data-capacity-state=\"blocked\""));
        assert!(runtime.contains("login_required"));
        assert!(runtime.contains("data-station-accepting-form"));
        assert!(runtime.contains("action=\"/collection/runtime/accepting\""));
        assert!(runtime.contains("data-account-binding-form"));
        assert!(runtime.contains("action=\"/collection/runtime/account-bindings\""));
        assert!(runtime.contains("data-account-binding-required=\"true\""));
    }

    #[test]
    fn v4_control_surfaces_replace_the_workspace_slots_with_bounded_real_projections() {
        let projection = projection();
        let base = format!(
            "<main>{READOUT_SLOT_START}<div>placeholder readout</div>{READOUT_SLOT_END}<div class=\"c-body\">{BODY_SLOT_START}<section>placeholder body</section>{BODY_SLOT_END}</div></main>"
        );

        let operations = render_operations(&base, &projection, OperationsMode::Now);
        assert!(operations.contains("c-operations-workspace"));
        assert!(operations.contains("c-decision-stream"));
        assert!(operations.contains("data-control-reason=\"account_needs_login\""));
        assert!(!operations.contains("placeholder body"));
        assert!(!operations.contains("placeholder readout"));
        assert_eq!(operations.matches("class=\"v7-kpi\"").count(), 2);
        assert!(!operations.contains("c-readout-strip"));

        let attention = render_attention(&base, &projection);
        assert!(attention.contains("c-attention-workspace"));
        assert!(attention.contains("data-attention-row"));
        assert!(attention.contains("data-attention-inspector"));
        assert!(attention.contains("data-owner=\"你\""));
        assert!(!attention.contains("placeholder body"));
        assert!(!attention.contains("placeholder readout"));
        assert_eq!(attention.matches("class=\"v7-kpi\"").count(), 2);
        assert!(!attention.contains("c-readout-strip"));
    }

    #[test]
    fn trace_and_review_keep_their_mode_specific_base_instead_of_mounting_now() {
        let projection = projection();
        for (mode, marker) in [
            (OperationsMode::Trace, "trace-mode-body"),
            (OperationsMode::Review, "review-mode-body"),
        ] {
            let base = format!("<main>{marker}</main>");
            let rendered = render_operations(&base, &projection, mode);
            assert_eq!(rendered, base);
            assert!(!rendered.contains("c-operations-workspace"));
        }
    }

    #[test]
    fn collection_control_ui_has_no_evidence_or_monitoring_value_module_copy() {
        let projection = projection();
        let base = format!(
            "<main>{READOUT_SLOT_START}{READOUT_SLOT_END}<div class=\"c-body\">{BODY_SLOT_START}{BODY_SLOT_END}</div></main>"
        );
        let html = format!(
            "{}{}",
            render_operations(&base, &projection, OperationsMode::Now),
            render_attention(&base, &projection)
        );
        for forbidden in ["Evidence", "监控价值", "代表证据", "机会评分"] {
            assert!(
                !html.contains(forbidden),
                "Collection control UI must not expose the retired {forbidden} module"
            );
        }
        assert!(html.contains("语料内容与价值评分归属语料页"));
    }

    #[test]
    fn attention_excludes_non_actionable_dynamic_and_ordinary_wait_states() {
        assert!(recovery_for("dynamic_unavailable").is_none());
        assert!(recovery_for("not_due").is_none());
        assert!(recovery_for("manual_only").is_none());
        assert!(recovery_for("monitoring_paused").is_none());
        assert!(recovery_for("account_needs_login").is_some());
        assert!(recovery_for("lease_issue_failed").is_some());
    }

    #[test]
    fn operations_recovery_count_excludes_ordinary_non_dispatch_decisions() {
        let mut projection = projection();
        let template = projection.decisions[0].clone();
        projection.decisions = [
            "not_due",
            "manual_only",
            "monitoring_paused",
            "dynamic_unavailable",
            "account_needs_login",
            "lease_issue_failed",
        ]
        .into_iter()
        .map(|reason| SchedulerDecisionView {
            target_decision_ref: Uuid::new_v4(),
            reason_code: reason.to_owned(),
            ..template.clone()
        })
        .collect();
        let html = operations_readout(&projection);
        assert!(html.contains("有恢复动作 2"));
        let flow = flow_stage_markup(&projection);
        let recovery = flow
            .split("恢复与重排")
            .nth(1)
            .expect("recovery stage exists");
        assert!(recovery.contains("<b>2</b>"));
        assert!(!recovery.contains("<b>6</b>"));
    }

    #[test]
    fn frozen_work_projection_is_scoped_to_the_selected_task() {
        let mut first = projection().works.remove(0);
        let first_task = Uuid::new_v4();
        first.task_id = Some(first_task);
        first.work_order_ref = Uuid::new_v4();
        let mut second = first.clone();
        let second_task = Uuid::new_v4();
        second.task_id = Some(second_task);
        second.work_order_ref = Uuid::new_v4();
        second.target_name = "另一个目标".to_owned();
        let works = vec![first.clone(), second.clone()];

        let first_only = works_for_task(&works, first_task);
        assert_eq!(first_only.len(), 1);
        assert_eq!(first_only[0].work_order_ref, first.work_order_ref);
        let templates = frozen_work_templates(&works);
        let first_marker = format!("data-task-frozen-template=\"{first_task}\"");
        let second_marker = format!("data-task-frozen-template=\"{second_task}\"");
        let first_start = templates.find(&first_marker).expect("first template");
        let second_start = templates.find(&second_marker).expect("second template");
        let (first_fragment, second_fragment) = if first_start < second_start {
            (&templates[first_start..second_start], &templates[second_start..])
        } else {
            (&templates[first_start..], &templates[second_start..first_start])
        };
        assert!(first_fragment.contains(&first.work_order_ref.to_string()));
        assert!(!first_fragment.contains(&second.work_order_ref.to_string()));
        assert!(second_fragment.contains(&second.work_order_ref.to_string()));
        assert!(!second_fragment.contains(&first.work_order_ref.to_string()));

        let script = include_str!("collection_workspace.js");
        assert!(script.contains("candidate.dataset.taskFrozenTemplate === row.dataset.taskId"));
        assert!(script.contains("该任务没有关联的冻结 Work 投影"));
    }

    #[test]
    fn one_work_order_keeps_a_frozen_projection_for_each_lease_task() {
        let mut first = projection().works.remove(0);
        let shared_work = Uuid::new_v4();
        let first_task = Uuid::new_v4();
        first.work_order_ref = shared_work;
        first.task_id = Some(first_task);
        first.task_state = Some("completed".to_owned());
        let mut second = first.clone();
        let second_task = Uuid::new_v4();
        second.task_id = Some(second_task);
        second.task_state = Some("pending".to_owned());

        let templates = frozen_work_templates(&[first, second]);
        assert!(templates.contains(&format!("data-task-frozen-template=\"{first_task}\"")));
        assert!(templates.contains(&format!("data-task-frozen-template=\"{second_task}\"")));
        assert_eq!(templates.matches(&shared_work.to_string()).count(), 2);
        assert!(templates.contains("completed"));
        assert!(templates.contains("pending"));
    }

    #[test]
    fn multi_task_work_does_not_duplicate_work_or_lease_stage_counts() {
        let mut projection = projection();
        let shared_work = Uuid::new_v4();
        let shared_lease = Uuid::new_v4();
        let mut first = projection.works.remove(0);
        first.work_order_ref = shared_work;
        first.lease_ref = Some(shared_lease);
        first.task_id = Some(Uuid::new_v4());
        let mut second = first.clone();
        second.task_id = Some(Uuid::new_v4());
        projection.works = vec![first, second];
        projection.decisions[0].work_order_ref = Some(shared_work);
        projection.decisions[0].lease_ref = Some(shared_lease);

        let flow = flow_stage_markup(&projection);
        let work_stage = flow.split("建立工单").nth(1).expect("work stage exists");
        let lease_stage = flow.split("签发租约").nth(1).expect("lease stage exists");
        let task_stage = flow.split("任务执行").nth(1).expect("task stage exists");
        assert!(work_stage.contains("<b>1</b>"));
        assert!(lease_stage.contains("<b>1</b>"));
        assert!(task_stage.contains("<b>2</b>"));
    }

    #[tokio::test]
    #[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
    async fn postgres_frozen_projection_expands_one_latest_lease_to_both_tasks() {
        use linggan_storage_postgres::testing::isolated_proof_schema;

        let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
            .expect("the isolated proof script provides a database URL");
        let database = isolated_proof_schema(
            &url,
            "collection_v4_multi_task_frozen",
            crate::local_web::full_schema_fixture::FULL_MIGRATIONS,
        )
        .await
        .expect("the full isolated schema applies");
        sqlx::raw_sql(
            "INSERT INTO collection_observation_target \
                 (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
             VALUES ('a1000000-0000-4000-8000-000000000001','xhs','creator','v4-multi-task','双任务冻结证明','manual','archiving'); \
             INSERT INTO execution_station (station_ref,display_name) \
             VALUES ('a5000000-0000-4000-8000-000000000001','双任务冻结工位'); \
             INSERT INTO collection_acquisition_authorization \
                 (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target, \
                  allowed_task_templates,allowed_dispatch_lanes,max_work_units, \
                  purpose,granted_by,expires_at) \
             VALUES ('a2000000-0000-4000-8000-000000000001','xhs','creator','deep_archive',1,20, \
                     ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],20, \
                     '双任务冻结读取证明','person',scope_001_now()+interval '1 day'); \
             INSERT INTO collection_acquisition_request \
                 (request_ref,target_ref,lane,purpose,requested_by) \
             VALUES ('a3000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','deep_archive','双任务冻结读取证明','person'); \
             INSERT INTO collection_admission_decision \
                 (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref,station_ref) \
             VALUES ('a4000000-0000-4000-8000-000000000001','a3000000-0000-4000-8000-000000000001','admitted','proof_admitted','a2000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','a5000000-0000-4000-8000-000000000001'); \
             INSERT INTO collection_work_order \
                 (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,station_ref) \
             VALUES ('a8000000-0000-4000-8000-000000000001','a4000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','deep_archive',20,'{}','a5000000-0000-4000-8000-000000000001'); \
             INSERT INTO linggan_runtime_task \
                 (task_id,task_spec_hash,task_spec,source,platform,page_type) VALUES \
                 ('a6000000-0000-4000-8000-000000000001',repeat('1',64),'{}','scheduled','xhs','author_profile'), \
                 ('a7000000-0000-4000-8000-000000000001',repeat('2',64),'{}','scheduled','xhs','profile_discovery'); \
             INSERT INTO collection_work_order_lease \
                 (lease_ref,work_order_ref,station_ref,capture_identity,expires_at) \
             VALUES ('a9000000-0000-4000-8000-000000000001','a8000000-0000-4000-8000-000000000001','a5000000-0000-4000-8000-000000000001','{}',scope_001_now()+interval '1 hour'); \
             INSERT INTO collection_work_order_lease_task \
                 (lease_ref,task_id,sequence_no,execution_state) VALUES \
                 ('a9000000-0000-4000-8000-000000000001','a6000000-0000-4000-8000-000000000001',1,'pending'), \
                 ('a9000000-0000-4000-8000-000000000001','a7000000-0000-4000-8000-000000000001',2,'pending');",
        )
        .execute(database.pool())
        .await
        .expect("the synthetic multi-task Work is stored");

        let projection = read_collection_control_surface(&database, 100)
            .await
            .expect("the control projection reads");
        let CollectionControlSurfaceRead::Ready(projection) = projection else {
            panic!("the complete isolated schema must be ready");
        };
        let work_ref = Uuid::parse_str("a8000000-0000-4000-8000-000000000001").unwrap();
        let first_task = Uuid::parse_str("a6000000-0000-4000-8000-000000000001").unwrap();
        let second_task = Uuid::parse_str("a7000000-0000-4000-8000-000000000001").unwrap();
        let works = projection
            .works
            .iter()
            .filter(|work| work.work_order_ref == work_ref)
            .collect::<Vec<_>>();
        assert_eq!(works.len(), 2);
        assert_eq!(works[0].task_id, Some(first_task));
        assert_eq!(works[1].task_id, Some(second_task));

        let templates = frozen_work_templates(&projection.works);
        assert!(templates.contains(&format!("data-task-frozen-template=\"{first_task}\"")));
        assert!(templates.contains(&format!("data-task-frozen-template=\"{second_task}\"")));
        assert_eq!(templates.matches(&work_ref.to_string()).count(), 2);
    }

    #[test]
    fn a_ready_control_projection_does_not_reopen_empty_task_loading() {
        let base = format!(
            "{READOUT_SLOT_START}{READOUT_SLOT_END}{EMPTY_STATE_OPEN}old{EMPTY_STATE_CLOSE}"
        );
        let tasks = crate::local_web::collection_tasks_view::render_tasks(
            &base,
            &linggan_evidence::CollectionTaskTimeline {
                tasks: Vec::new(),
                accepted_count: 0,
                active_count: 0,
                expired_lease_count: 0,
            },
        );
        let rendered = render_tasks_control(&tasks, &projection());

        assert!(rendered.contains("当前没有任务，因此没有可核对的冻结 Work 引用"));
        assert!(!rendered.contains("冻结资源正在读取"));
    }

    #[test]
    fn unavailable_task_control_reads_are_terminal_not_loading() {
        let task_id = Uuid::new_v4();
        let base = format!(
            r#"<div data-task-id="{task_id}"><!-- frozen-work:start --><p>该任务的冻结 Work 投影正在读取。</p><!-- frozen-work:end --></div>"#
        );

        for (state, expected) in [
            (TaskControlUnavailable::SchemaUnavailable, "控制 schema 尚未就绪"),
            (TaskControlUnavailable::ReadFailed, "本次读取失败"),
        ] {
            let rendered = render_tasks_control_unavailable(&base, state);
            assert!(rendered.contains("冻结 Work 投影当前不可用"));
            assert!(rendered.contains(expected));
            assert!(rendered.contains("当前任务、Attempt、Package 与 Receipt 仍可读"));
            assert!(!rendered.contains("正在读取"));
        }
    }

    #[test]
    fn binding_action_is_only_shown_for_an_observed_candidate_needing_confirmation() {
        let mut projection = projection();
        let base = format!("{BODY_OPEN}old</div>");
        let candidate = render_runtime_control(&base, &projection);
        assert!(candidate.contains("data-account-binding-form"));

        projection.runtime_resources[0].binding_state = "bound_without_eligibility".to_owned();
        projection.runtime_resources[0].eligibility_state = None;
        projection.runtime_resources[0].eligibility_reason_code = None;
        let already_bound = render_runtime_control(&base, &projection);
        assert!(!already_bound.contains("data-account-binding-form"));
        assert!(already_bound.contains("已绑定，资格信号缺失"));
    }

    #[test]
    fn runtime_keeps_last_observation_for_diagnosis_not_as_a_claim_deadline() {
        let mut projection = projection();
        let base = format!("{BODY_OPEN}old</div>");
        projection.runtime_resources[0].eligibility_state = Some("usable".to_owned());
        projection.runtime_resources[0].eligibility_reason_code = Some("authenticated".to_owned());
        let rendered = render_runtime_control(&base, &projection);
        assert!(rendered.contains("最后观察"));
        assert!(rendered.contains("不因时间经过阻断接活"));

        projection.runtime_resources[0].eligibility_state = None;
        projection.runtime_resources[0].eligibility_reason_code = None;
        let never_observed = render_runtime_control(&base, &projection);
        assert!(never_observed.contains("data-account-eligibility-reason=\"account_unknown\""));
        assert!(!never_observed.contains("account_eligibility_stale"));
    }

    #[test]
    fn renderers_escape_names_and_never_expose_forbidden_payload_fields() {
        let projection = projection();
        let base = format!("{BODY_OPEN}old</div>");
        let html = format!(
            "{}{}",
            render_tasks_control(&base, &projection),
            render_runtime_control(&base, &projection)
        );
        assert!(html.contains("&lt;一&gt;"));
        assert!(html.contains("Mac mini &lt;主机&gt;"));
        for forbidden in ["credential_hash", "identity_digest", "Cookie", "payload"] {
            assert!(!html.contains(forbidden), "leaked {forbidden}");
        }
    }
}
