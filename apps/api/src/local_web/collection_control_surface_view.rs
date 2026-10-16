// Durable Package 2 control projections for Operations, Attention, Tasks and Runtime.
//
// The scheduler decision and frozen resource references are history. Current Runtime capacity
// is read through `read_capacity`, the same evaluator Admission uses. The resource rows below
// only expose bounded control facts; they never select a credential hash, account digest,
// platform identity, Cookie, HTML, payload, Evidence content or free-form platform error.

use linggan_contracts::Capacity;
use linggan_evidence::{
    MINIMUM_PLUGIN_VERSION, collection_control_schema_is_ready, collection_governance_enabled,
    read_capacity,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

const RUNTIME_LANES: [(&str, &str, &str); 3] = [
    ("创作者基线", "creator", "deep_archive"),
    ("创作者巡检", "creator", "patrol"),
    ("关键词巡检", "keyword", "patrol"),
];

#[derive(Debug, Clone)]
pub enum CollectionControlSurfaceRead {
    Ready(CollectionControlSurfaceProjection),
    SchemaUnavailable,
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
    let governance_enabled = collection_governance_enabled();
    let runtime_lanes = if governance_enabled {
        let mut lanes = Vec::with_capacity(RUNTIME_LANES.len());
        for (label, target_kind, lane) in RUNTIME_LANES {
            let capacity = read_capacity(database, "xhs", target_kind, lane).await?;
            lanes.push(runtime_lane(label, target_kind, lane, capacity));
        }
        lanes
    } else {
        recovery_runtime_lanes()
    };
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

pub enum CollectionTaskControlRead {
    Ready(Vec<FrozenWorkView>),
    SchemaUnavailable,
}

/// Tasks consume only frozen Work facts. Runtime's live admission/capacity reads remain separate.
pub async fn read_task_control_surface(
    database: &Database,
    limit: i64,
) -> Result<CollectionTaskControlRead, sqlx::Error> {
    if !collection_control_schema_is_ready(database).await? {
        return Ok(CollectionTaskControlRead::SchemaUnavailable);
    }
    Ok(CollectionTaskControlRead::Ready(
        read_frozen_works(database, limit.clamp(1, 100)).await?,
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
                -- 「这张工单绑的规则还是当前的吗」比的是**同一条规则**的当前版本，
                -- 不是目标的某一条规则：一个关键词可以同时盯几个榜。规则被停用时为假。
                CASE WHEN work.monitor_rule_revision_ref IS NULL THEN NULL \
                     ELSE EXISTS ( \
                       SELECT 1 FROM collection_monitor_rule_revision frozen \
                       JOIN collection_monitor_rule owning_rule \
                         ON owning_rule.rule_ref=frozen.rule_ref \
                        AND owning_rule.retired_at IS NULL \
                       WHERE frozen.rule_revision_ref=work.monitor_rule_revision_ref \
                         AND owning_rule.active_revision_ref=work.monitor_rule_revision_ref) \
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
               AND observation.eligibility_state IN ('usable','cooling','needs_login','restricted') \
             ORDER BY observation.observation_sequence DESC LIMIT 1) eligibility ON true \
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

/// The global dispatch gate is known even when the detailed control read fails. The runtime
/// route uses this projection in recovery so its capacity-only fallback cannot claim readiness.
pub(crate) fn recovery_runtime_lanes() -> Vec<RuntimeLaneControlView> {
    RUNTIME_LANES
        .into_iter()
        .map(|(label, target_kind, lane)| {
            blocked_lane(
                label,
                target_kind,
                lane,
                "collection_upgrade_recovery_only",
                "采集处于恢复阶段，暂不派发新任务。".to_owned(),
            )
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_phase_blocks_dispatch_even_when_a_station_has_capacity() {
        let paused = recovery_runtime_lanes()
            .into_iter()
            .find(|lane| lane.target_kind == "creator" && lane.lane == "patrol")
            .expect("the patrol lane exists");
        assert!(!paused.available);
        assert!(!paused.queueable);
        assert_eq!(paused.reason_code, Some("collection_upgrade_recovery_only"));
        assert_eq!(
            paused.reason.as_deref(),
            Some("采集处于恢复阶段，暂不派发新任务。")
        );

        let ready = runtime_lane(
            "创作者巡检",
            "creator",
            "patrol",
            Capacity::Available {
                station_ref: "ready-station".to_owned(),
            },
        );
        assert!(ready.available);
    }

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL proof database"]
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
             INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
             VALUES ('00000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','primary'); \
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
                 (request_ref,target_ref,lane,purpose,requested_by,domain_ref,observation_role) \
             VALUES ('a3000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','deep_archive','双任务冻结读取证明','person','00000000-0000-4000-8000-000000000001','primary'); \
             INSERT INTO collection_admission_decision \
                 (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref,station_ref) \
             VALUES ('a4000000-0000-4000-8000-000000000001','a3000000-0000-4000-8000-000000000001','admitted','proof_admitted','a2000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','a5000000-0000-4000-8000-000000000001'); \
             INSERT INTO collection_work_order \
                 (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,station_ref,dispatch_lane,queue_state,scheduled_for) \
             VALUES ('a8000000-0000-4000-8000-000000000001','a4000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','deep_archive',20,'{}','a5000000-0000-4000-8000-000000000001','immediate','queued',scope_001_now()); \
             INSERT INTO collection_work_order_domain_usage \
                 (work_order_ref,request_ref,domain_ref,role,basis_kind) \
             VALUES ('a8000000-0000-4000-8000-000000000001','a3000000-0000-4000-8000-000000000001','00000000-0000-4000-8000-000000000001','primary','admitted'); \
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

        let CollectionTaskControlRead::Ready(task_projection) =
            read_task_control_surface(&database, 100).await.unwrap()
        else {
            panic!("task schema must be ready");
        };
        assert_eq!(task_projection, projection.works);
        // An unrelated Runtime credential read cannot erase Tasks' frozen Work facts.
        sqlx::raw_sql("ALTER TABLE installation_credential RENAME COLUMN activated_at TO runtime_only_unavailable")
            .execute(database.pool()).await.unwrap();
        assert!(
            read_collection_control_surface(&database, 100)
                .await
                .is_err()
        );
        let CollectionTaskControlRead::Ready(task_projection) =
            read_task_control_surface(&database, 100).await.unwrap()
        else {
            panic!("the bounded task read remains ready");
        };
        assert_eq!(task_projection, projection.works);
        let current_tasks = task_projection
            .iter()
            .filter(|work| work.work_order_ref == work_ref)
            .map(|work| work.task_id)
            .collect::<Vec<_>>();
        assert!(current_tasks.contains(&Some(first_task)));
        assert!(current_tasks.contains(&Some(second_task)));
        assert_eq!(current_tasks.len(), 2);
    }
}
