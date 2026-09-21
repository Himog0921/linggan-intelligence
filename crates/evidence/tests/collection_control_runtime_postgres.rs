use linggan_contracts::{
    AdmissionOutcome, Capacity, ProducerTaskSpec, parse_producer_attempt,
    parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilityObservation, AuthorizationGrant, CheckInOutcome, CollectionControlError,
    DispatchDecision, InstallationCheckIn, MonitorCommandActor, MonitorCommandKind,
    MonitorCommandOutcomeKind, MonitorRuleCommand, MonitorRuleDraft, MonitorRuleMode,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, StationError,
    activate_installation_credential, apply_monitor_rule_command, bind_observation_account,
    check_in_installation, decide_dispatch, grant_authorization, open_claim_window, read_capacity,
    read_runtime_capacity, register_station, report_account_eligibility, request_admit_and_lease,
    request_and_admit, retire_station, rotate_installation_credential, run_due_patrols,
    set_station_accepting, start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;
use uuid::Uuid;

const DIGEST_KEY: &[u8] = b"collection-control-runtime-proof-digest-key-v1";

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../database/migrations/0005_collection_observation_target.sql"),
    "\n",
    include_str!("../../../database/migrations/0006_collection_acquisition_chain.sql"),
    "\n",
    include_str!("../../../database/migrations/0007_execution_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0008_collection_risk_pause.sql"),
    "\n",
    include_str!("../../../database/migrations/0009_work_order_station.sql"),
    "\n",
    include_str!("../../../database/migrations/0010_work_order_lease.sql"),
    "\n",
    include_str!("../../../database/migrations/0011_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0012_target_monitor_schedule.sql"),
    "\n",
    include_str!("../../../database/migrations/0013_drop_execution_gate.sql"),
    "\n",
    include_str!("../../../database/migrations/0014_target_group.sql"),
    "\n",
    include_str!("../../../database/migrations/0015_material_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    include_str!("../../../database/migrations/0019_work_order_lease_task_sequence.sql"),
    "\n",
    include_str!("../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    include_str!("../../../database/migrations/0021_discovery_cover_media_acquisition.sql"),
    "\n",
    include_str!("../../../database/migrations/0022_material_deepening_scope.sql"),
    "\n",
    include_str!("../../../database/migrations/0023_material_engagement_and_media_components.sql"),
    "\n",
    include_str!("../../../database/migrations/0024_media_processing_runtime.sql"),
    "\n",
    include_str!("../../../database/migrations/0025_comment_current_projection.sql"),
    "\n",
    include_str!("../../../database/migrations/0026_work_resource_read.sql"),
    "\n",
    include_str!("../../../database/migrations/0027_unified_media_resource.sql"),
    "\n",
    include_str!("../../../database/migrations/0029_author_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0030_comment_image_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0031_topic_workspace.sql"),
    "\n",
    include_str!("../../../database/migrations/0032_author_profile_avatar_media.sql"),
    "\n",
    include_str!("../../../database/migrations/0033_dispatch_failure_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0034_collection_control_closure.sql"),
    "\n",
    include_str!("../../../database/migrations/0035_claimed_station_auto_acceptance.sql"),
    "\n",
    include_str!("../../../database/migrations/0036_monitor_scheduling_clarity.sql"),
    "\n",
    include_str!("../../../database/migrations/0037_collection_scheduler_scale.sql"),
    "\n",
    include_str!("../../../database/migrations/0038_detail_only_material_scope.sql"),
    "\n",
    include_str!("../../../database/migrations/0042_keyword_monitoring_lifecycle.sql"),
    "\n",
    include_str!("../../../database/migrations/0045_deep_archive_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0046_keyword_sampling_policy.sql"),
    "\n",
    include_str!("../../../database/migrations/0047_collection_detail_failure_boundary.sql"),
    "\n",
    include_str!("../../../database/migrations/0052_work_order_expiry.sql"),
    "\n",
    include_str!("../../../database/migrations/0061_material_retirement.sql"),
    "\n",
    include_str!("../../../database/migrations/0062_human_moment.sql"),
    "\n",
    include_str!("../../../database/migrations/0063_content_author_attribution.sql"),
    "\n",
    include_str!("../../../database/migrations/0064_account_observation_normalization.sql"),
    "\n",
    include_str!("../../../database/migrations/0065_account_observation_bootstrap.sql"),
    "\n",
    include_str!("../../../database/migrations/0080_scheduler_admission_failure_reasons.sql"),
    "\n",
    include_str!("../../../database/migrations/0076_monitor_rule_slots.sql"),
    "\n",
    include_str!("../../../database/migrations/0078_monitor_rule_owns_its_schedule.sql"),
    "\n",
    include_str!("../../../database/migrations/0089_detail_page_session_replay_safety.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0090_detail_page_grant_recovery_and_risk_cooldown.sql"
    ),
    "\n",
    include_str!("../../../database/migrations/0092_detail_page_session_recovery_boundary.sql"),
    "\n",
    include_str!("../../../database/migrations/0093_capture_delivery_rejection.sql"),
    "\n",
    include_str!("../../../database/migrations/0094_corpus_evidence_read_recovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0095_detail_page_url_rejection.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0096_detail_page_session_lane_delivery_identities.sql"
    ),
    "\n",
    include_str!("../../../database/migrations/0097_collection_execution_input_eligibility.sql"),
    "\n",
    include_str!(
        "../../../database/migrations/0098_scheduler_tick_steps_and_readiness.sql"
    ),
);

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn failed_lease_rolls_back_request_decision_work_and_target_transition() {
    let database = proof_database("control_runtime_atomic_rollback").await;
    let _installation = ready_installation(&database, "atomic-rollback").await;
    let target_ref = seed_target(&database, "creator", "pending_decision", "atomic-target").await;
    authorize(&database, "deep_archive", "atomic purpose", 1).await;

    let result = request_admit_and_lease(
        &database,
        target_ref,
        "deep_archive",
        "atomic purpose",
        "person",
        0,
    )
    .await;
    assert!(result.is_err(), "a zero-length lease must fail");

    let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM collection_acquisition_request), \
           (SELECT count(*) FROM collection_admission_decision), \
           (SELECT count(*) FROM collection_work_order), \
           (SELECT count(*) FROM collection_work_order_lease), \
           (SELECT count(*) FROM collection_work_order_lease_task)",
    )
    .fetch_one(database.pool())
    .await
    .expect("atomic-chain rows are inspectable");
    assert_eq!(counts, (0, 0, 0, 0, 0));
    let state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("target state remains readable");
    assert_eq!(state, "pending_decision");
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn concurrent_station_claims_create_at_most_one_live_lease_for_one_account() {
    let database = proof_database("control_runtime_account_mutex").await;
    let installation = ready_installation(&database, "account-mutex").await;
    let first = seed_target(&database, "creator", "pending_decision", "mutex-first").await;
    let second = seed_target(&database, "creator", "pending_decision", "mutex-second").await;
    authorize(&database, "deep_archive", "mutex purpose", 2).await;
    admitted_work(&database, first, "mutex purpose").await;
    admitted_work(&database, second, "mutex purpose").await;

    let (left, right) = tokio::join!(
        decide_dispatch(&database, &installation.install_key, &installation.secret),
        decide_dispatch(&database, &installation.install_key, &installation.secret),
    );
    let decisions = [
        left.expect("first concurrent poll returns a durable decision"),
        right.expect("second concurrent poll returns a durable decision"),
    ];
    let claims = decisions.map(|decision| match decision {
        DispatchDecision::Dispatch {
            task_id, lease_ref, ..
        } => (task_id, lease_ref),
        other => panic!("a concurrent poll must replay the one in-progress claim; got {other:?}"),
    });
    assert_eq!(
        claims[0], claims[1],
        "concurrent polls replay one claimed task instead of creating two browser tasks"
    );
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease lease \
         JOIN collection_work_order work_order USING(work_order_ref) \
         WHERE work_order.account_ref=$1 AND lease.released_at IS NULL \
           AND lease.expires_at>scope_001_now()",
    )
    .bind(installation.account_ref)
    .fetch_one(database.pool())
    .await
    .expect("live account leases are inspectable");
    assert_eq!(live, 1);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn platform_dispatch_cap_is_a_cross_station_hard_lease_limit() {
    let database = proof_database("control_runtime_platform_cap").await;
    sqlx::query(
        "UPDATE collection_platform_dispatch_policy SET concurrent_cap=1 WHERE platform='xhs'",
    )
    .execute(database.pool())
    .await
    .expect("proof narrows only its isolated platform cap");
    let first_installation = ready_installation(&database, "platform-cap-first").await;
    let second_installation = ready_installation(&database, "platform-cap-second").await;
    let first = seed_target(
        &database,
        "creator",
        "pending_decision",
        "platform-cap-first",
    )
    .await;
    let second = seed_target(
        &database,
        "creator",
        "pending_decision",
        "platform-cap-second",
    )
    .await;
    authorize(&database, "deep_archive", "platform cap purpose", 2).await;
    admitted_work(&database, first, "platform cap purpose").await;
    admitted_work(&database, second, "platform cap purpose").await;

    assert!(matches!(
        decide_dispatch(
            &database,
            &first_installation.install_key,
            &first_installation.secret
        )
        .await
        .expect("first eligible station claims one work order"),
        DispatchDecision::Dispatch { .. }
    ));
    let blocked = decide_dispatch(
        &database,
        &second_installation.install_key,
        &second_installation.secret,
    )
    .await
    .expect("second station receives a durable capacity decision");
    assert!(matches!(
        blocked,
        DispatchDecision::ControlBlocked { ref reason_code }
            if reason_code == "platform_concurrency_reached"
    ));
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease \
         WHERE released_at IS NULL AND expires_at>scope_001_now()",
    )
    .fetch_one(database.pool())
    .await
    .expect("live platform leases are inspectable");
    assert_eq!(live, 1);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn authorization_revoke_or_expiry_after_lease_blocks_dispatch() {
    for (suffix, expire) in [("revoked", false), ("expired", true)] {
        let database = proof_database(&format!("control_runtime_auth_{suffix}")).await;
        let (installation, _target_ref, authorization_ref) =
            leased_creator(&database, &format!("auth-{suffix}"), "auth purpose").await;
        if expire {
            sqlx::query(
                "UPDATE collection_acquisition_authorization \
                 SET granted_at=scope_001_now()-interval '2 days', \
                     expires_at=scope_001_now()-interval '1 second' \
                 WHERE authorization_ref=$1",
            )
            .bind(authorization_ref)
            .execute(database.pool())
            .await
            .expect("proof expires the authorization");
        } else {
            sqlx::query(
                "UPDATE collection_acquisition_authorization \
                 SET revoked_at=scope_001_now(),revoke_reason='runtime proof' \
                 WHERE authorization_ref=$1",
            )
            .bind(authorization_ref)
            .execute(database.pool())
            .await
            .expect("proof revokes the authorization");
        }
        let decision = decide_dispatch(&database, &installation.install_key, &installation.secret)
            .await
            .expect("dispatch returns a closed control decision");
        assert!(matches!(
            decision,
            DispatchDecision::ControlBlocked { ref reason_code }
                if reason_code == "authorization_expired_or_revoked"
        ));
        let claimed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM collection_work_order_lease_task \
             WHERE execution_state='in_progress'",
        )
        .fetch_one(database.pool())
        .await
        .expect("claim state is inspectable");
        assert_eq!(claimed, 0);
    }
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn station_closed_after_lease_issuance_blocks_dispatch() {
    let database = proof_database("control_runtime_station_closes_after_lease").await;
    let (installation, _target_ref, _authorization_ref) = leased_creator(
        &database,
        "station-close-after-lease",
        "station close purpose",
    )
    .await;

    set_station_accepting(&database, installation.station_ref, false, "person")
        .await
        .expect("person closes station acceptance");

    let decision = decide_dispatch(&database, &installation.install_key, &installation.secret)
        .await
        .expect("dispatch returns a closed control decision");
    assert!(matches!(
        decision,
        DispatchDecision::ControlBlocked { ref reason_code }
            if reason_code == "station_not_accepting"
    ));
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task \
         WHERE execution_state='pending'",
    )
    .fetch_one(database.pool())
    .await
    .expect("closed station leaves the task pending");
    assert_eq!(pending, 2);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn non_progressive_creator_archive_never_claims_a_completed_baseline() {
    let cases = [
        ("empty_records", CoverageCase::EmptyRecords, "archiving"),
        ("zero", CoverageCase::Zero, "archiving"),
        ("scan_limited", CoverageCase::ScanLimited, "archiving"),
        // A complete pair of receipts is evidence, but it was dispatched under the ordinary
        // 20-work path.  Only an explicitly frozen 200-work progressive root can establish an
        // archive baseline; see the dedicated progressive-root proof for that positive path.
        ("qualified", CoverageCase::Qualified, "archiving"),
    ];
    for (suffix, coverage_case, expected_state) in cases {
        let database = proof_database(&format!("control_runtime_baseline_{suffix}")).await;
        let (installation, target_ref, _authorization_ref) =
            leased_creator(&database, &format!("baseline-{suffix}"), "baseline purpose").await;
        complete_creator_tasks(&database, &installation, coverage_case).await;
        let state: String = sqlx::query_scalar(
            "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .expect("baseline lifecycle is readable");
        assert_eq!(state, expected_state, "coverage case {suffix}");
    }
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn automatic_save_rule_moves_a_paused_creator_target_to_monitoring() {
    let database = proof_database("control_runtime_save_rule_resume").await;
    // 时钟冻住：下面要断言两次存同一条规则落在同一个发车时刻。用真实时钟的话，两次读数
    // 之间只要跨过一个整秒边界，取整后就会差 1——那种红是机器快慢造成的，不是缺陷。
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-13T02:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .expect("the proof clock is frozen");
    let target_ref =
        seed_target(&database, "creator", "pending_decision", "save-rule-resume").await;
    let first =
        apply_monitor_rule_command(&database, &save_rule(target_ref, 0, false, Uuid::new_v4()))
            .await
            .expect("manual fixed rule is saved");
    assert_eq!(first.outcome, MonitorCommandOutcomeKind::Applied);
    assert_target_state(&database, target_ref, "paused", false).await;

    let second =
        apply_monitor_rule_command(&database, &save_rule(target_ref, 1, true, Uuid::new_v4()))
            .await
            .expect("automatic rule is saved");
    assert_eq!(second.outcome, MonitorCommandOutcomeKind::Applied);
    assert_target_state(&database, target_ref, "monitoring", true).await;
    // 首次巡查不排在「现在」，而是排在这条规则自己的相位上：同一批目标同时开监控时不会
    // 挤在同一秒里发车。相位由规则身份决定，所以再存一次同一条规则，落点不会漂。
    let (interval_seconds, first_delay_seconds): (i32, i64) = sqlx::query_as(
        "SELECT revision.fixed_interval_seconds, \
                EXTRACT(EPOCH FROM (rule.monitor_next_run_at-scope_001_now()))::bigint \
         FROM collection_monitor_rule rule \
         JOIN collection_monitor_rule_revision revision \
           ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the rule carries its own schedule phase and first next run");
    assert!(
        (0..i64::from(interval_seconds)).contains(&first_delay_seconds),
        "首次巡查必须落在一个周期之内，实际 {first_delay_seconds} 秒"
    );

    let third =
        apply_monitor_rule_command(&database, &save_rule(target_ref, 2, true, Uuid::new_v4()))
            .await
            .expect("the same rule is saved again");
    assert_eq!(third.outcome, MonitorCommandOutcomeKind::Applied);
    let replayed_delay_seconds: i64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (rule.monitor_next_run_at-scope_001_now()))::bigint \
         FROM collection_monitor_rule rule \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the rule schedule is readable again");
    assert_eq!(
        replayed_delay_seconds, first_delay_seconds,
        "相位由规则身份决定，改一次设置不该把发车时刻推走"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn runtime_scale_projection_reads_policy_lanes_and_persisted_rule_schedule() {
    let database = proof_database("control_runtime_scale_projection").await;
    let target_ref = seed_target(&database, "creator", "paused", "runtime-scale-rule").await;
    apply_monitor_rule_command(&database, &save_rule(target_ref, 0, true, Uuid::new_v4()))
        .await
        .expect("automatic fixed rule is persisted before a read-only runtime projection");

    let overview = read_runtime_capacity(&database)
        .await
        .expect("runtime read projects source-of-truth scheduling rows");
    assert_eq!(overview.platform_dispatch.len(), 1);
    assert_eq!(overview.platform_dispatch[0].platform, "xhs");
    assert_eq!(overview.dispatch_backlog.len(), 3);
    assert_eq!(
        overview
            .dispatch_backlog
            .iter()
            .map(|lane| lane.dispatch_lane.as_str())
            .collect::<Vec<_>>(),
        vec!["immediate", "scheduled", "batch"]
    );
    // 一条规则一行；创作者的唯一默认槽位也必须带着稳定身份进入运行时投影。
    assert_eq!(overview.monitor_rule_schedules.len(), 1);
    assert_eq!(
        overview.monitor_rule_schedules[0].target_label,
        "runtime-scale-rule"
    );
    assert!(overview.monitor_rule_schedules[0].next_run_at.is_some());
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn paused_target_queues_person_observation_while_dismissed_target_is_rejected() {
    let database = proof_database("control_runtime_manual_observe").await;
    let _installation = ready_installation(&database, "manual-observe").await;
    authorize(&database, "patrol", "人工立即观察", 2).await;

    let paused = seed_target(&database, "creator", "paused", "manual-paused").await;
    let first_key = Uuid::new_v4();
    let applied = apply_monitor_rule_command(&database, &manual_observe(paused, first_key))
        .await
        .expect("a person may observe a paused target immediately");
    assert_eq!(applied.outcome, MonitorCommandOutcomeKind::Applied);
    assert_eq!(applied.reason_code, "manual_observe_created");
    assert!(applied.applied_rule_revision_ref.is_none());
    let work_order_ref = applied.work_order_ref.expect("receipt links real work");
    assert_eq!(
        applied.lease_ref, None,
        "a click queues work; a station claim creates the lease"
    );
    let queued: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
           SELECT 1 FROM collection_monitor_rule_command_receipt receipt \
           JOIN collection_work_order work_order ON work_order.work_order_ref=receipt.work_order_ref \
           WHERE receipt.command_receipt_ref=$1 AND work_order.work_order_ref=$2 \
             AND receipt.lease_ref IS NULL AND work_order.queue_state='queued')",
    )
    .bind(applied.receipt_ref)
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("receipt/work queue linkage is inspectable");
    assert!(queued);
    assert_target_state(&database, paused, "paused", false).await;

    let replay = apply_monitor_rule_command(&database, &manual_observe(paused, first_key))
        .await
        .expect("same command identity replays");
    assert_eq!(replay.outcome, MonitorCommandOutcomeKind::Replay);
    assert_eq!(replay.reason_code, "manual_observe_created");
    assert_eq!(replay.work_order_ref, Some(work_order_ref));
    assert_eq!(replay.lease_ref, None);
    assert!(replay.applied_rule_revision_ref.is_none());

    let reused = apply_monitor_rule_command(&database, &manual_observe(paused, Uuid::new_v4()))
        .await
        .expect("a different command reuses the target's live execution");
    assert_eq!(reused.outcome, MonitorCommandOutcomeKind::Applied);
    assert_eq!(reused.reason_code, "manual_observe_reused");
    assert_eq!(reused.work_order_ref, Some(work_order_ref));
    assert_eq!(reused.lease_ref, None);
    assert!(reused.applied_rule_revision_ref.is_none());
    let rule_revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_monitor_rule_revision WHERE target_ref=$1",
    )
    .bind(paused)
    .fetch_one(database.pool())
    .await
    .expect("manual observation rule side effects are inspectable");
    assert_eq!(rule_revisions, 0);

    let unauthorized = seed_target(&database, "keyword", "paused", "manual-unauthorized").await;
    let refused =
        apply_monitor_rule_command(&database, &manual_observe(unauthorized, Uuid::new_v4()))
            .await
            .expect("authorization refusal returns a durable receipt");
    assert_eq!(refused.outcome, MonitorCommandOutcomeKind::Rejected);
    assert_eq!(refused.reason_code, "database_unavailable");
    assert!(refused.work_order_ref.is_none());
    assert!(refused.lease_ref.is_none());
    assert!(refused.applied_rule_revision_ref.is_none());
    let refused_facts: (i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM collection_monitor_rule_command_receipt \
             WHERE command_receipt_ref=$1 AND reason_code='database_unavailable'), \
           (SELECT count(*) FROM collection_work_order WHERE target_ref=$2), \
           (SELECT count(*) FROM collection_work_order_lease lease \
             JOIN collection_work_order work_order USING(work_order_ref) \
             WHERE work_order.target_ref=$2)",
    )
    .bind(refused.receipt_ref)
    .bind(unauthorized)
    .fetch_one(database.pool())
    .await
    .expect("refused receipt and execution absence are inspectable");
    assert_eq!(refused_facts, (1, 0, 0));

    let dismissed = seed_target(&database, "creator", "dismissed", "manual-dismissed").await;
    let rejected =
        apply_monitor_rule_command(&database, &manual_observe(dismissed, Uuid::new_v4()))
            .await
            .expect("dismissal returns a durable closed receipt");
    assert_eq!(rejected.outcome, MonitorCommandOutcomeKind::Rejected);
    assert_eq!(rejected.reason_code, "target_not_requestable");
    assert!(rejected.work_order_ref.is_none());
    assert!(rejected.lease_ref.is_none());
    assert!(rejected.applied_rule_revision_ref.is_none());
    let dismissed_work: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
            .bind(dismissed)
            .fetch_one(database.pool())
            .await
            .expect("dismissed work absence is inspectable");
    assert_eq!(dismissed_work, 0);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn retiring_station_atomically_ends_control_and_execution_ownership() {
    let database = proof_database("control_runtime_station_retire").await;
    let (installation, _target_ref, _authorization_ref) =
        leased_creator(&database, "station-retire", "retire purpose").await;
    retire_station(
        &database,
        installation.station_ref,
        "person retired station",
    )
    .await
    .expect("station retirement succeeds");

    let station: (bool, bool) = sqlx::query_as(
        "SELECT retired_at IS NOT NULL,accepting_tasks FROM execution_station WHERE station_ref=$1",
    )
    .bind(installation.station_ref)
    .fetch_one(database.pool())
    .await
    .expect("retired station is retained");
    assert_eq!(station, (true, false));
    let installation_state: bool = sqlx::query_scalar(
        "SELECT superseded_at IS NOT NULL FROM plugin_installation WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("installation retirement is readable");
    assert!(installation_state);
    let credential_reason: String = sqlx::query_scalar(
        "SELECT revoke_reason_code FROM installation_credential \
         WHERE installation_ref=$1 AND activated_at IS NOT NULL ORDER BY issued_at DESC LIMIT 1",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("credential revocation is retained");
    assert_eq!(credential_reason, "station_retired");
    let binding_reason: String = sqlx::query_scalar(
        "SELECT end_reason_code FROM platform_observation_account_binding \
         WHERE installation_ref=$1 ORDER BY bound_at DESC LIMIT 1",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("binding end is retained");
    assert_eq!(binding_reason, "station_retired");
    let lease_reason: String = sqlx::query_scalar(
        "SELECT release_reason FROM collection_work_order_lease lease \
         WHERE lease.station_ref=$1 ORDER BY lease.issued_at DESC LIMIT 1",
    )
    .bind(installation.station_ref)
    .fetch_one(database.pool())
    .await
    .expect("lease release is retained");
    assert_eq!(lease_reason, "station_unavailable");
    let transition: (bool, String) = sqlx::query_as(
        "SELECT to_accepting,reason_code FROM execution_station_acceptance_transition \
         WHERE station_ref=$1 ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(installation.station_ref)
    .fetch_one(database.pool())
    .await
    .expect("acceptance transition is retained");
    assert_eq!(transition, (false, "station_retired".to_owned()));
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn scheduler_scans_due_valid_rules_in_bounded_pages_without_rule_missing() {
    let database = proof_database("control_runtime_scheduler_fairness").await;
    let mut targets = Vec::new();
    for ordinal in 0..75 {
        let target_ref = seed_target(
            &database,
            "creator",
            "paused",
            &format!("scheduler-{ordinal:03}"),
        )
        .await;
        let applied =
            apply_monitor_rule_command(&database, &save_rule(target_ref, 0, true, Uuid::new_v4()))
                .await
                .expect("a fixed automatic rule is saved before scheduling");
        assert_eq!(applied.outcome, MonitorCommandOutcomeKind::Applied);
        // 排期状态住在**规则**上（`0076`）：一个目标可以有几条规则，各自的下次运行时间
        // 互不相干。改目标行已经不会让任何规则到期。
        sqlx::query(
            "UPDATE collection_monitor_rule \
             SET monitor_next_run_at=scope_001_now()-interval '1 second' WHERE target_ref=$1",
        )
        .bind(target_ref)
        .execute(database.pool())
        .await
        .expect("valid scheduler rule is due");
        targets.push(target_ref);
    }
    let summary = run_due_patrols(&database)
        .await
        .expect("bounded scheduler tick completes");
    assert!(summary.queued.is_empty());
    assert_eq!(summary.dispatched.len(), 0);
    assert_eq!(summary.skipped.len(), 50);
    let run = sqlx::query(
        "SELECT scheduler_run_ref,considered_count FROM collection_scheduler_run \
         ORDER BY started_at DESC LIMIT 1",
    )
    .fetch_one(database.pool())
    .await
    .expect("scheduler run is durable");
    assert_eq!(run.get::<i32, _>("considered_count"), 50);
    let decisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_scheduler_target_decision \
         WHERE scheduler_run_ref=$1 AND reason_code='authorization_missing' \
           AND next_eligible_at>decided_at",
    )
    .bind(run.get::<Uuid, _>("scheduler_run_ref"))
    .fetch_one(database.pool())
    .await
    .expect("per-target decisions and backoff are inspectable");
    assert_eq!(decisions, 50);
    let considered: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_observation_target \
         WHERE target_ref=ANY($1) AND last_scheduler_considered_at IS NOT NULL",
    )
    .bind(&targets)
    .fetch_one(database.pool())
    .await
    .expect("all target cursors are inspectable");
    assert_eq!(considered, 50);
    let untouched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_observation_target \
         WHERE target_ref=ANY($1) AND last_scheduler_considered_at IS NULL",
    )
    .bind(&targets)
    .fetch_one(database.pool())
    .await
    .expect("the next bounded page remains for a later tick");
    assert_eq!(untouched, 25);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn credential_response_loss_rotation_and_activation_are_recoverable_and_hash_only() {
    let database = proof_database("control_runtime_credential_activation").await;
    let (station_ref, installation_ref, install_key, first_pending_ref, first_pending_raw) =
        claim_pending_installation(&database, "credential-recovery").await;
    set_station_accepting(&database, station_ref, true, "person")
        .await
        .expect("person opens station acceptance");
    assert_capacity(&database, "installation_credential_missing").await;

    let (second_pending_ref, second_pending_raw) = match check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &install_key,
            installation_credential: None,
            plugin_version: "0.8.47",
            browser_label: Some("credential-recovery"),
            capabilities: capabilities(),
        },
    )
    .await
    .expect("lost response is retried")
    {
        CheckInOutcome::Heartbeat {
            credential: Some(issued),
            ..
        } => (
            issued.credential_ref,
            issued.raw_credential.expose_once().to_owned(),
        ),
        other => panic!("retry must issue a recoverable pending credential; got {other:?}"),
    };
    assert_ne!(first_pending_ref, second_pending_ref);
    assert_ne!(first_pending_raw, second_pending_raw);
    let first_reason: String = sqlx::query_scalar(
        "SELECT revoke_reason_code FROM installation_credential WHERE credential_ref=$1",
    )
    .bind(first_pending_ref)
    .fetch_one(database.pool())
    .await
    .expect("lost pending issuance remains auditable");
    assert_eq!(first_reason, "pending_replaced");

    activate_installation_credential(
        &database,
        installation_ref,
        second_pending_ref,
        &second_pending_raw,
    )
    .await
    .expect("raw hash acknowledgement activates the pending issuance");
    let receipt = report_account_eligibility(
        &database,
        installation_ref,
        &second_pending_raw,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "credential-recovery-account",
        },
        Some(DIGEST_KEY),
    )
    .await
    .expect("activated credential authenticates");
    bind_observation_account(
        &database,
        receipt.account_ref.expect("account ref exists"),
        installation_ref,
        "person",
    )
    .await
    .expect("person confirms account binding");
    assert_capacity(&database, "available").await;

    let rotated = rotate_installation_credential(&database, installation_ref)
        .await
        .expect("rotation creates a pending issuance");
    let rotated_ref = rotated.credential_ref;
    let rotated_raw = rotated.raw_credential.expose_once().to_owned();
    report_account_eligibility(
        &database,
        installation_ref,
        &second_pending_raw,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "credential-recovery-account",
        },
        Some(DIGEST_KEY),
    )
    .await
    .expect("the old active credential survives pending rotation");
    assert!(matches!(
        report_account_eligibility(
            &database,
            installation_ref,
            &rotated_raw,
            AccountEligibilityObservation::Authenticated {
                raw_platform_account_id: "credential-recovery-account",
            },
            Some(DIGEST_KEY),
        )
        .await,
        Err(CollectionControlError::InvalidCredential)
    ));
    assert_capacity(&database, "available").await;

    activate_installation_credential(&database, installation_ref, rotated_ref, &rotated_raw)
        .await
        .expect("rotation activation succeeds");
    assert!(matches!(
        report_account_eligibility(
            &database,
            installation_ref,
            &second_pending_raw,
            AccountEligibilityObservation::Authenticated {
                raw_platform_account_id: "credential-recovery-account",
            },
            Some(DIGEST_KEY),
        )
        .await,
        Err(CollectionControlError::InvalidCredential)
    ));
    report_account_eligibility(
        &database,
        installation_ref,
        &rotated_raw,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "credential-recovery-account",
        },
        Some(DIGEST_KEY),
    )
    .await
    .expect("new credential becomes active atomically");

    let credential_states: Vec<(Uuid, bool, Option<String>)> = sqlx::query_as(
        "SELECT credential_ref,activated_at IS NOT NULL AND revoked_at IS NULL,revoke_reason_code \
         FROM installation_credential WHERE installation_ref=$1 ORDER BY issued_at",
    )
    .bind(installation_ref)
    .fetch_all(database.pool())
    .await
    .expect("credential history is inspectable");
    assert_eq!(credential_states.iter().filter(|row| row.1).count(), 1);
    assert_eq!(credential_states.last().map(|row| row.0), Some(rotated_ref));
    let forbidden_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='installation_credential' \
           AND (column_name LIKE '%raw%' OR column_name LIKE '%secret%' \
                OR column_name LIKE '%token%' OR column_name LIKE '%password%')",
    )
    .fetch_one(database.pool())
    .await
    .expect("credential schema is inspectable");
    assert_eq!(forbidden_columns, 0);
    let persisted: String = sqlx::query_scalar(
        "SELECT COALESCE(string_agg(to_jsonb(row_value)::text,''),'') \
         FROM installation_credential row_value WHERE installation_ref=$1",
    )
    .bind(installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("credential persistence is inspectable");
    for raw in [first_pending_raw, second_pending_raw, rotated_raw] {
        assert!(!persisted.contains(&raw));
    }
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn active_installation_heartbeat_rejects_spoofing_before_mutating_installation_facts() {
    let database = proof_database("control_runtime_heartbeat_spoof_gate").await;
    let installation = ready_installation(&database, "heartbeat-spoof").await;
    sqlx::query(
        "UPDATE plugin_installation SET last_seen_at=scope_001_now()-interval '1 hour' \
         WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .expect("proof makes mutation visible");
    let before: (String, serde_json::Value, String) = sqlx::query_as(
        "SELECT plugin_version,capabilities,to_char(last_seen_at,'YYYY-MM-DD HH24:MI:SS.USOF') \
         FROM plugin_installation WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("installation facts are readable");
    let spoofed_capabilities = serde_json::json!(["discovery_search"]);

    for supplied_credential in [None, Some("lgi_ic_wrong_credential")] {
        let result = check_in_installation(
            &database,
            &InstallationCheckIn {
                install_key: &installation.install_key,
                installation_credential: supplied_credential,
                plugin_version: "99.99.99",
                browser_label: Some("spoofed"),
                capabilities: spoofed_capabilities.clone(),
            },
        )
        .await;
        assert!(matches!(
            result,
            Err(StationError::Control(
                CollectionControlError::InvalidCredential
            ))
        ));
        let after: (String, serde_json::Value, String) = sqlx::query_as(
            "SELECT plugin_version,capabilities,to_char(last_seen_at,'YYYY-MM-DD HH24:MI:SS.USOF') \
             FROM plugin_installation WHERE installation_ref=$1",
        )
        .bind(installation.installation_ref)
        .fetch_one(database.pool())
        .await
        .expect("rejected heartbeat cannot mutate facts");
        assert_eq!(after, before);
    }

    let accepted = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &installation.install_key,
            installation_credential: Some(&installation.secret),
            plugin_version: "0.8.35",
            browser_label: Some("authenticated"),
            capabilities: spoofed_capabilities.clone(),
        },
    )
    .await
    .expect("correct active credential authenticates heartbeat");
    assert!(matches!(
        accepted,
        CheckInOutcome::Heartbeat {
            credential: None,
            ..
        }
    ));
    let after: (String, serde_json::Value, bool) = sqlx::query_as(
        "SELECT plugin_version,capabilities, \
                last_seen_at>scope_001_now()-interval '1 minute' \
         FROM plugin_installation WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .fetch_one(database.pool())
    .await
    .expect("authenticated heartbeat mutation is readable");
    assert_eq!(after, ("0.8.35".to_owned(), spoofed_capabilities, true));
}

#[derive(Clone, Copy, Debug)]
enum CoverageCase {
    EmptyRecords,
    Zero,
    ScanLimited,
    Qualified,
}

#[derive(Debug)]
struct Installed {
    station_ref: Uuid,
    installation_ref: Uuid,
    install_key: String,
    secret: String,
    account_ref: Uuid,
}

async fn claim_pending_installation(
    database: &Database,
    label: &str,
) -> (Uuid, Uuid, String, Uuid, String) {
    let station_ref = register_station(database, label, 200)
        .await
        .expect("station is registered");
    open_claim_window(database, station_ref, 1)
        .await
        .expect("claim window opens");
    let install_key = Uuid::new_v4().to_string();
    let outcome = check_in_installation(
        database,
        &InstallationCheckIn {
            install_key: &install_key,
            installation_credential: None,
            plugin_version: "0.8.47",
            browser_label: Some(label),
            capabilities: capabilities(),
        },
    )
    .await
    .expect("installation checks in");
    match outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref: claimed_station_ref,
            credential: Some(issued),
            ..
        } => {
            assert_eq!(station_ref, claimed_station_ref);
            (
                station_ref,
                installation_ref,
                install_key,
                issued.credential_ref,
                issued.raw_credential.expose_once().to_owned(),
            )
        }
        other => panic!("claimed compatible installation must get pending issuance; got {other:?}"),
    }
}

async fn ready_installation(database: &Database, label: &str) -> Installed {
    let (station_ref, installation_ref, install_key, credential_ref, secret) =
        claim_pending_installation(database, label).await;
    activate_installation_credential(database, installation_ref, credential_ref, &secret)
        .await
        .expect("credential is activated after durable local storage");
    set_station_accepting(database, station_ref, true, "person")
        .await
        .expect("person enables the station");
    let receipt = report_account_eligibility(
        database,
        installation_ref,
        &secret,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: &format!("{label}-account"),
        },
        Some(DIGEST_KEY),
    )
    .await
    .expect("account eligibility is current");
    let account_ref = receipt.account_ref.expect("account ref exists");
    bind_observation_account(database, account_ref, installation_ref, "person")
        .await
        .expect("person confirms account binding");
    Installed {
        station_ref,
        installation_ref,
        install_key,
        secret,
        account_ref,
    }
}

fn capabilities() -> serde_json::Value {
    serde_json::json!(["author_profile", "profile_discovery", "discovery_search"])
}

async fn authorize(database: &Database, lane: &str, purpose: &str, max_targets: i32) -> Uuid {
    grant_authorization(
        database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane,
            purpose,
            max_targets: Some(max_targets),
            max_works_per_target: Some(20),
            valid_for_days: 1,
        },
    )
    .await
    .expect("person grants bounded authorization")
}

async fn admitted_work(database: &Database, target_ref: Uuid, purpose: &str) -> Uuid {
    let outcome = request_and_admit(database, target_ref, "deep_archive", purpose, "person")
        .await
        .expect("request is decided");
    assert!(matches!(outcome.outcome, AdmissionOutcome::Admitted { .. }));
    outcome.work_order_ref.expect("admission creates work")
}

async fn leased_creator(
    database: &Database,
    label: &str,
    purpose: &str,
) -> (Installed, Uuid, Uuid) {
    let installation = ready_installation(database, label).await;
    let target_ref = seed_target(
        database,
        "creator",
        "pending_decision",
        &format!("{label}-creator"),
    )
    .await;
    let authorization_ref = authorize(database, "deep_archive", purpose, 1).await;
    let outcome =
        request_admit_and_lease(database, target_ref, "deep_archive", purpose, "person", 30)
            .await
            .expect("atomic request through lease succeeds");
    assert!(matches!(
        outcome.request.outcome,
        AdmissionOutcome::Admitted { .. }
    ));
    assert!(outcome.lease.is_some());
    (installation, target_ref, authorization_ref)
}

async fn complete_creator_tasks(
    database: &Database,
    installation: &Installed,
    coverage_case: CoverageCase,
) {
    for _ in 0..2 {
        let decision = decide_dispatch(database, &installation.install_key, &installation.secret)
            .await
            .expect("scheduled task dispatches");
        let task = task_from_dispatch(&decision);
        let producer_instance_id =
            Uuid::parse_str(&installation.install_key).expect("install key fixture is a UUID");
        let attempt = parse_producer_attempt(
            &serde_json::json!({
                "contractVersion":"linggan.producer.attempt.v1",
                "producerInstanceId":producer_instance_id,
                "taskId":task.task_id(),
                "attemptId":Uuid::new_v4(),
            })
            .to_string(),
        )
        .expect("attempt contract is valid");
        assert!(matches!(
            start_producer_attempt(database, &attempt).await,
            Ok(RuntimeAttemptOutcome::Started { .. })
        ));
        let submission = creator_submission(&task, &attempt, producer_instance_id, coverage_case);
        assert!(matches!(
            submit_producer_package(database, &submission).await,
            Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
        ));
    }
}

fn task_from_dispatch(decision: &DispatchDecision) -> ProducerTaskSpec {
    let task_spec = match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec,
        other => panic!("expected a dispatch decision, got {other:?}"),
    };
    parse_producer_task_spec(&task_spec.to_string()).expect("dispatched TaskSpec remains valid")
}

fn creator_submission(
    task: &ProducerTaskSpec,
    attempt: &linggan_contracts::ProducerAttempt,
    producer_instance_id: Uuid,
    coverage_case: CoverageCase,
) -> linggan_contracts::ProducerSubmission {
    let capability = task.raw()["capabilitiesRequested"][0]
        .as_str()
        .expect("scheduled capability exists");
    let identity = task.raw()["target"]["authorExternalId"]
        .as_str()
        .expect("creator identity exists");
    let (count, stopped_reason, include_record) = match coverage_case {
        CoverageCase::EmptyRecords => (1, "surface_ended", false),
        CoverageCase::Zero => (0, "surface_ended", false),
        CoverageCase::ScanLimited => (1, "maximum_quota", true),
        CoverageCase::Qualified => (1, "surface_ended", true),
    };
    let records = if !include_record {
        Vec::new()
    } else if capability == "author_profile" {
        vec![serde_json::json!({
            "kind":"author_profile",
            "sourceObject":{"platform":"xhs","type":"author","externalId":identity},
            "payload":{"userId":identity,"nickname":"runtime baseline proof","fans":17}
        })]
    } else {
        vec![serde_json::json!({
            "kind":"profile_discovery_card",
            "resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":format!("{identity}-note")},
            "payload":{"title":"runtime baseline proof","url":"https://www.xiaohongshu.com/explore/runtime-proof?xsec_token=proof"}
        })]
    };
    parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":capability,
                "platform":"xhs",
                "observedAt":"2026-09-04T00:00:00Z",
                "capturedAt":"2026-09-04T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":identity},
                    "layers":[{
                        "capability":capability,
                        "observed":count,"attempted":count,"acquired":count,"verified":0,
                        "failed":0,"notAttempted":0,"unknown":0,
                        "stoppedReason":stopped_reason
                    }]
                },
                "records":records
            }
        })
        .to_string(),
    )
    .expect("creator capture package is contract-valid")
}

fn save_rule(
    target_ref: Uuid,
    expected_revision: i32,
    automatic_enabled: bool,
    idempotency_key: Uuid,
) -> MonitorRuleCommand {
    MonitorRuleCommand {
        target_ref,
        expected_revision,
        idempotency_key,
        kind: MonitorCommandKind::SaveRule,
        actor: MonitorCommandActor::Person,
        source: "targets_ui",
        slot_key: None,
        draft: Some(MonitorRuleDraft {
            mode: MonitorRuleMode::Fixed,
            automatic_enabled,
            run_on_weekdays: true,
            run_on_weekends: true,
            all_day: true,
            window_start_minute: None,
            window_end_minute: None,
            fixed_interval_seconds: Some(43_200),
            fallback_interval_seconds: 43_200,
            surface_key: "creator_patrol".to_owned(),
            ranking_key: None,
            scroll_rounds: None,
            top_by_likes: None,
            published_within_days: None,
            task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
        }),
    }
}

fn manual_observe(target_ref: Uuid, idempotency_key: Uuid) -> MonitorRuleCommand {
    MonitorRuleCommand {
        target_ref,
        expected_revision: 0,
        idempotency_key,
        kind: MonitorCommandKind::ManualObserve,
        actor: MonitorCommandActor::Person,
        source: "targets_ui",
        draft: None,
        slot_key: None,
    }
}

async fn seed_target(
    database: &Database,
    target_kind: &str,
    lifecycle_state: &str,
    identity_key: &str,
) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs',$2,$3,$3,'manual',$4)",
    )
    .bind(target_ref)
    .bind(target_kind)
    .bind(identity_key)
    .bind(lifecycle_state)
    .execute(database.pool())
    .await
    .expect("target fixture is seeded");
    target_ref
}

async fn assert_target_state(
    database: &Database,
    target_ref: Uuid,
    expected_state: &str,
    expected_enabled: bool,
) {
    let row: (String, bool) = sqlx::query_as(
        "SELECT lifecycle_state,monitoring_enabled FROM collection_observation_target \
         WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("target control state is readable");
    assert_eq!(row, (expected_state.to_owned(), expected_enabled));
}

async fn assert_capacity(database: &Database, expected_reason: &str) {
    let capacity = read_capacity(database, "xhs", "creator", "deep_archive")
        .await
        .expect("capacity is readable");
    assert_eq!(
        capacity.reason_code(),
        expected_reason,
        "capacity: {capacity:?}"
    );
    if expected_reason == "available" {
        assert!(matches!(capacity, Capacity::Available { .. }));
    }
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("COLLECTION_CONTROL_PROOF_DATABASE_URL")
        .or_else(|_| std::env::var("COLLECTION_DISPATCH_PROOF_DATABASE_URL"))
        .expect("a disposable PostgreSQL proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("complete migrations through 0036 apply")
}
