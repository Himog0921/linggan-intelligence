use linggan_contracts::{
    EvidenceQuery, ProducerTaskSpec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilitySignal, AuthorizationGrant, CheckInOutcome, DispatchDecision,
    DispatchFailureCode, DispatchFailureOutcome, InstallationCheckIn, MonitorCommandActor,
    MonitorCommandKind, MonitorRuleCommand, MonitorRuleDraft, MonitorRuleMode,
    ProducerRuntimeError, RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome,
    activate_installation_credential, apply_monitor_rule_command, bind_observation_account,
    check_in_installation, create_producer_task, decide_dispatch, expire_lapsed_leases,
    grant_authorization, issue_work_order_lease, open_claim_window, read_collection_task_timeline,
    read_work_resources, recover_released_orphaned_work_orders, report_account_eligibility,
    requeue_failed_dispatch, rotate_installation_credential, set_station_accepting,
    start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;
use uuid::Uuid;

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
    include_str!("../../../database/migrations/0045_deep_archive_recovery.sql"),
);

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn target_cannot_claim_monitoring_without_a_rule_and_scheduler_stays_idle() {
    let database = proof_database_for("collection_scheduler_enabled_target").await;
    let target_ref = Uuid::new_v4();
    let invalid = sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,monitoring_enabled) \
         VALUES ($1,'xhs','creator','creator-auto-proof','自动调度证明','manual','monitoring',true)",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await;
    assert!(
        invalid.is_err(),
        "0036 rejects monitoring without an active rule"
    );

    linggan_evidence::record_scheduler_started(&database, Uuid::new_v4())
        .await
        .unwrap();
    let first = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert!(first.queued.is_empty());
    assert!(first.dispatched.is_empty());
    assert!(first.skipped.is_empty());
    let first_heartbeat = linggan_evidence::read_scheduler_heartbeat(&database)
        .await
        .unwrap()
        .expect("scheduler heartbeat exists after an idle tick");
    assert_eq!(first_heartbeat.state, "running");
    assert_eq!(first_heartbeat.last_outcome, "idle");
    assert_eq!(first_heartbeat.dispatched_count, 0);
    assert_eq!(first_heartbeat.skipped_count, 0);
    let work_order_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(work_order_count, 0);
    let live_lease_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order w JOIN collection_work_order_lease l USING(work_order_ref) \
         WHERE w.target_ref=$1 AND l.released_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(live_lease_count, 0);

    let second = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert!(second.queued.is_empty());
    assert!(second.dispatched.is_empty());
    let work_order_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        work_order_count_after, 0,
        "an enabled flag does not bypass the versioned rule and capacity gates"
    );
    let heartbeat = linggan_evidence::read_scheduler_heartbeat(&database)
        .await
        .unwrap()
        .expect("scheduler heartbeat exists");
    assert_eq!(heartbeat.state, "running");
    assert_eq!(heartbeat.last_outcome, "idle");
    assert_eq!(heartbeat.dispatched_count, 0);
    assert_eq!(heartbeat.skipped_count, 0);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_rule_queues_once_without_a_baseline_or_a_preassigned_station() {
    let database = proof_database_for("collection_scheduler_creator_queue").await;
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs','creator','creator-rule-proof','规则入队证明','manual','pending_decision')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("creator target is seeded without any archive receipt");
    grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "patrol",
            purpose: "creator patrol proof",
            max_targets: Some(10),
            max_works_per_target: Some(30),
            valid_for_days: 1,
        },
    )
    .await
    .expect("structured patrol authorization is granted");
    let saved = apply_monitor_rule_command(
        &database,
        &MonitorRuleCommand {
            target_ref,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: MonitorCommandKind::SaveRule,
            actor: MonitorCommandActor::Person,
            source: "targets_ui",
            draft: Some(MonitorRuleDraft {
                mode: MonitorRuleMode::Fixed,
                automatic_enabled: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start_minute: None,
                window_end_minute: None,
                fixed_interval_seconds: Some(21_600),
                fallback_interval_seconds: 21_600,
                surface_key: "creator_patrol".to_owned(),
                ranking_key: None,
                task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
            }),
        },
    )
    .await
    .expect("baseline completeness does not block creator rule save");
    assert_eq!(saved.reason_code, "rule_saved");
    sqlx::query(
        "UPDATE collection_observation_target \
         SET monitor_next_run_at=scope_001_now()-interval '1 second' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the isolated clock makes the rule due");

    let tick = linggan_evidence::run_due_patrols(&database)
        .await
        .expect("due rule is queued");
    assert_eq!(tick.queued, vec![target_ref]);
    assert!(tick.dispatched.is_empty());
    assert!(tick.skipped.is_empty());
    let queued: (
        String,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT queue_state,dispatch_lane,station_ref,installation_ref,account_ref, \
                    monitor_rule_revision_ref \
             FROM collection_work_order WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the durable queued work order exists");
    assert_eq!(queued.0, "queued");
    assert_eq!(queued.1, "scheduled");
    assert_eq!((queued.2, queued.3, queued.4), (None, None, None));
    assert_eq!(queued.5, saved.applied_rule_revision_ref);
    let schedule: (bool, Option<String>, i32) = sqlx::query_as(
        "SELECT monitoring_enabled,monitor_next_run_at::text,monitor_missed_run_count \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the next schedule advances atomically with queueing");
    assert!(schedule.0);
    assert!(schedule.1.is_some());
    assert_eq!(schedule.2, 0);
    let active_lease_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease WHERE work_order_ref=( \
             SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1)",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("scheduler did not pre-lease browser work");
    assert_eq!(active_lease_count, 0);

    // A later plugin poll, not the scheduler tick, supplies the actual station
    // and account. The fixture's own legacy order stays unqueued and therefore
    // cannot be chosen instead of this scheduled Work Order.
    let claimant = seed_creator_work_order(&database).await;
    let claimed = decide_dispatch(
        &database,
        &claimant.install_key,
        &claimant.installation_credential,
    )
    .await
    .expect("eligible station claims the queued creator patrol");
    assert_eq!(capability(&claimed), "author_profile");
    let leased: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT queue_state,station_ref,installation_ref,account_ref \
         FROM collection_work_order WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("claim freezes the actual station route only now");
    assert_eq!(leased.0, "leased");
    assert_eq!(leased.1, Some(claimant.station_ref));
    assert_eq!(leased.2, Some(claimant.installation_ref));
    assert!(leased.3.is_some());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_lease_claims_and_completes_two_scheduled_tasks_in_order() {
    let database = proof_database().await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order receives one ordered lease");
    assert_eq!(lease.task_ids.len(), 2);

    let legacy_task_id: Option<Uuid> =
        sqlx::query_scalar("SELECT task_id FROM collection_work_order_lease WHERE lease_ref = $1")
            .bind(lease.lease_ref)
            .fetch_one(database.pool())
            .await
            .expect("legacy compatibility column is readable");
    assert_eq!(
        legacy_task_id, None,
        "new writers do not dual-write task identity"
    );

    let rows = sqlx::query(
        "SELECT sequence_no, execution_state FROM collection_work_order_lease_task \
         WHERE lease_ref = $1 ORDER BY sequence_no",
    )
    .bind(lease.lease_ref)
    .fetch_all(database.pool())
    .await
    .expect("lease task sequence is readable");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<i32, _>("sequence_no"), 1);
    assert_eq!(rows[1].get::<i32, _>("sequence_no"), 2);
    assert!(
        rows.iter()
            .all(|row| row.get::<String, _>("execution_state") == "pending")
    );

    let (left, right) = tokio::join!(
        decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential
        ),
        decide_dispatch(
            &database,
            &fixture.install_key,
            &fixture.installation_credential
        ),
    );
    let (first, first_replay) = same_dispatch(
        left.expect("first concurrent dispatch decides"),
        right.expect("second concurrent dispatch decides"),
    );
    assert_eq!(capability(&first), "author_profile");
    assert_eq!(task_id(&first), task_id(&first_replay));

    let response_loss_replay = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("lost response retry decides");
    assert_eq!(task_id(&first), task_id(&response_loss_replay));

    let first_task = task_from_dispatch(&first);
    let wrong_attempt = attempt(first_task.task_id(), Uuid::new_v4());
    assert!(matches!(
        start_producer_attempt(&database, &wrong_attempt).await,
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
    ));
    run_scheduled_task(&database, &first_task, fixture.producer_instance_id).await;
    assert_task_state(&database, first_task.task_id(), "completed").await;
    assert!(lease_is_live(&database, lease.lease_ref).await);

    let second = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("second sequence dispatch decides");
    assert_eq!(capability(&second), "profile_discovery");
    let second_replay = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("second response replay decides");
    assert_eq!(task_id(&second), task_id(&second_replay));
    let second_task = task_from_dispatch(&second);
    run_scheduled_task(&database, &second_task, fixture.producer_instance_id).await;
    assert_task_state(&database, second_task.task_id(), "completed").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
    let lifecycle_state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target \
         WHERE identity_key='creator-fixture'",
    )
    .fetch_one(database.pool())
    .await
    .expect("completed baseline advances target lifecycle");
    assert_eq!(
        lifecycle_state, "archiving",
        "two accepted empty packages complete the lease but cannot manufacture a qualified baseline"
    );

    let manual_task = manual_task();
    assert!(matches!(
        create_producer_task(&database, &manual_task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let manual_attempt = attempt(manual_task.task_id(), Uuid::new_v4());
    assert!(matches!(
        start_producer_attempt(&database, &manual_attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task / Attempt / Package / Receipt read projection is available");
    assert_eq!(timeline.tasks.len(), 3);
    assert_eq!(timeline.accepted_count, 2);
    assert_eq!(
        timeline.active_count, 1,
        "manual Attempt has no Receipt yet"
    );
    assert_eq!(timeline.expired_lease_count, 0);
    let completed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task.task_id())
        .expect("first scheduled task is listed");
    assert_eq!(completed.capabilities, "author_profile");
    assert_eq!(completed.queue_state.as_deref(), Some("completed"));
    assert_eq!(completed.has_live_lease, Some(false));
    assert_eq!(completed.material_admission.as_deref(), Some("ACCEPTED"));
    assert_eq!(
        completed.target_display_name.as_deref(),
        Some("顺序派发夹具"),
        "scheduled task retains its collection target instead of inventing an author"
    );
    let unreceived = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == manual_task.task_id())
        .expect("manual task is listed");
    assert!(unreceived.attempt_id.is_some());
    assert!(unreceived.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn task_read_projection_keeps_queue_attempt_package_and_receipt_distinct() {
    let database = proof_database_for("collection_task_read_projection").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed");
    let first_task = task_from_dispatch(&first);
    run_scheduled_task(&database, &first_task, fixture.producer_instance_id).await;
    assert_task_state(&database, first_task.task_id(), "completed").await;
    assert!(lease_is_live(&database, lease.lease_ref).await);

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task projection is readable");
    assert_eq!(timeline.tasks.len(), 2);
    assert_eq!(timeline.accepted_count, 1);
    assert_eq!(timeline.active_count, 1);
    assert_eq!(timeline.expired_lease_count, 0);

    let completed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task.task_id())
        .expect("completed first step is present");
    assert_eq!(completed.capabilities, "author_profile");
    assert_eq!(completed.queue_state.as_deref(), Some("completed"));
    assert_eq!(completed.has_live_lease, Some(true));
    assert!(completed.attempt_id.is_some());
    assert!(completed.package_ref.is_some());
    assert!(completed.receipt_ref.is_some());
    assert_eq!(completed.material_admission.as_deref(), Some("ACCEPTED"));
    assert_eq!(
        completed.target_display_name.as_deref(),
        Some("顺序派发夹具")
    );

    let pending = timeline
        .tasks
        .iter()
        .find(|row| row.task_id != first_task.task_id())
        .expect("second ordered step is present");
    assert_eq!(pending.capabilities, "profile_discovery");
    assert_eq!(pending.queue_state.as_deref(), Some("pending"));
    assert_eq!(pending.has_live_lease, Some(true));
    assert!(pending.attempt_id.is_none());
    assert!(pending.package_ref.is_none());
    assert!(pending.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn expired_scheduled_lease_is_historical_not_active_in_task_projection() {
    let database = proof_database_for("collection_task_read_expired_lease").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed before the lease expires");
    let claimed_task_id = task_id(&dispatch);
    assert_task_state(&database, claimed_task_id, "in_progress").await;

    // A deterministic isolated fixture: real expiry code must record this as
    // `released_at = expires_at`, rather than the read projection guessing from
    // a stale queue state.
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET issued_at = scope_001_now() - interval '2 seconds', \
             expires_at = scope_001_now() - interval '1 second' \
         WHERE lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("fixture lease is made lapsed");
    assert_eq!(
        expire_lapsed_leases(&database)
            .await
            .expect("lapsed lease is recorded"),
        1
    );
    assert!(!lease_is_live(&database, lease.lease_ref).await);

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("expired lease stays visible as historical task data");
    assert_eq!(timeline.tasks.len(), 2);
    assert_eq!(timeline.active_count, 0);
    assert_eq!(timeline.expired_lease_count, 2);
    let claimed = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == claimed_task_id)
        .expect("claimed task remains in bounded history");
    assert_eq!(claimed.queue_state.as_deref(), Some("in_progress"));
    assert_eq!(claimed.has_live_lease, Some(false));
    assert!(claimed.attempt_id.is_none());
    assert!(claimed.receipt_ref.is_none());
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn failed_browser_start_is_audited_then_returns_work_order_to_shared_queue() {
    let database = proof_database_for("collection_dispatch_failure_recovery").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let first_dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first task is claimed before page startup");
    let first_task_id = task_id(&first_dispatch);
    assert_task_state(&database, first_task_id, "in_progress").await;

    let failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageTimeout,
    )
    .await
    .expect("the owning installation can return a failed page start to the queue");
    assert_eq!(
        outcome,
        DispatchFailureOutcome::Requeued {
            retry_after_seconds: 60
        }
    );
    assert_task_state(&database, first_task_id, "pending").await;
    let claim_owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("requeued task remains readable");
    assert_eq!(claim_owner, None, "no installation owns a requeued task");
    let failure: (Uuid, Uuid, String) = sqlx::query_as(
        "SELECT task_id,installation_ref,failure_code \
         FROM collection_work_order_lease_task_dispatch_failure WHERE failure_ref=$1",
    )
    .bind(failure_ref)
    .fetch_one(database.pool())
    .await
    .expect("the page-start failure remains auditable");
    assert_eq!(failure.0, first_task_id);
    assert_eq!(failure.1, fixture.installation_ref);
    assert_eq!(failure.2, "page_timeout");
    let attempt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(first_task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt count is readable");
    assert_eq!(
        attempt_count, 0,
        "a page-start failure is not a producer Attempt"
    );

    let timeline = read_collection_task_timeline(&database, 100)
        .await
        .expect("Task read projection keeps dispatch failures visible");
    assert_eq!(timeline.expired_lease_count, 2);
    let task = timeline
        .tasks
        .iter()
        .find(|row| row.task_id == first_task_id)
        .expect("requeued task is listed");
    assert_eq!(task.queue_state.as_deref(), Some("pending"));
    assert_eq!(task.has_live_lease, Some(false));
    assert_eq!(
        task.last_dispatch_failure_code.as_deref(),
        Some("page_timeout")
    );
    assert!(task.attempt_id.is_none());

    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        failure_ref,
        DispatchFailureCode::PageTimeout,
    )
    .await
    .expect("a lost failure response replays without writing a second failure");
    assert_eq!(
        replay,
        DispatchFailureOutcome::Replay {
            retry_after_seconds: 60
        }
    );
    let failure_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task_dispatch_failure WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("failure count is readable");
    assert_eq!(failure_count, 1, "same failure id is an idempotent replay");

    let cooling: bool = sqlx::query_scalar(
        "SELECT retry_not_before_at>scope_001_now() AND dispatch_failure_count=1 \
         FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .fetch_one(database.pool())
    .await
    .expect("the retry delay is a persisted work-order fact");
    assert!(
        cooling,
        "a different station cannot immediately reopen the same failed page"
    );
    let during_cooling = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the shared queue remains readable while cooling");
    assert!(matches!(during_cooling, DispatchDecision::NothingWaiting));
    sqlx::query(
        "UPDATE collection_work_order SET retry_not_before_at=scope_001_now()-interval '1 second' \
         WHERE work_order_ref=$1",
    )
    .bind(fixture.work_order_ref)
    .execute(database.pool())
    .await
    .expect("proof advances only the isolated retry clock");
    let retry = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("a fresh eligible claim is made after persistent cooling expires");
    assert_ne!(
        task_id(&retry),
        first_task_id,
        "a new Lease makes a fresh immutable RuntimeTask; the failed one stays history"
    );
    assert_task_state(&database, first_task_id, "pending").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn unavailable_detail_is_audited_without_blocking_later_materials() {
    let database = proof_database_for("collection_dispatch_page_unavailable").await;
    let fixture = seed_creator_work_order(&database).await;
    for (ordinal, content_external_id) in [(1, "unavailable-first"), (2, "available-second")] {
        submit_profile_discovery(
            &database,
            content_external_id,
            &format!(
                "https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user"
            ),
        )
        .await;
        let content_public_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id=$1",
        )
        .bind(content_external_id)
        .fetch_one(database.pool())
        .await
        .expect("accepted discovery creates the stable material identity");
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
             VALUES ($1,$2,$3,30,2,true)",
        )
        .bind(fixture.work_order_ref)
        .bind(content_public_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .expect("both exact materials are frozen by the same approved WorkOrder");
    }
    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("deepening lease is issued");
    let first = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first detail work is claimed");
    let first_task_id = task_id(&first);
    assert_eq!(
        task_from_dispatch(&first).raw()["target"]["contentExternalId"],
        "unavailable-first"
    );

    let unavailable_failure_ref = Uuid::new_v4();
    let outcome = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        unavailable_failure_ref,
        DispatchFailureCode::PageUnavailable,
    )
    .await
    .expect("a producer-confirmed unavailable page is recorded");
    assert_eq!(outcome, DispatchFailureOutcome::Unavailable);
    let unavailable_lanes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease_task task \
         JOIN linggan_runtime_task runtime USING(task_id) \
         WHERE runtime.task_spec #>> '{target,contentExternalId}'='unavailable-first' \
           AND task.execution_state='unavailable'",
    )
    .fetch_one(database.pool())
    .await
    .expect("all dependent lanes remain readable");
    assert_eq!(
        unavailable_lanes, 4,
        "one unreadable work closes only its own lanes"
    );
    let attempts: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_attempt WHERE task_id=$1")
            .bind(first_task_id)
            .fetch_one(database.pool())
            .await
            .expect("attempt history is readable");
    assert_eq!(attempts, 0, "page unavailability is not a producer Attempt");

    let next = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the next independent material is still eligible");
    assert_eq!(
        task_from_dispatch(&next).raw()["target"]["contentExternalId"],
        "available-second"
    );
    let failure_code: String = sqlx::query_scalar(
        "SELECT failure_code FROM collection_work_order_lease_task_dispatch_failure \
         WHERE task_id=$1",
    )
    .bind(first_task_id)
    .fetch_one(database.pool())
    .await
    .expect("unavailability remains an append-only dispatch fact");
    assert_eq!(failure_code, "page_unavailable");
    let replay = requeue_failed_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        first_task_id,
        unavailable_failure_ref,
        DispatchFailureCode::PageUnavailable,
    )
    .await
    .expect("a lost terminal acknowledgement remains idempotent");
    assert_eq!(replay, DispatchFailureOutcome::Unavailable);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn released_orphaned_work_order_is_recovered_without_rewriting_old_lease_history() {
    let database = proof_database_for("collection_dispatch_orphaned_work_order").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("fixture work is leased once");
    let claimed = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("first historical task is claimed");
    let historical_task_id = task_id(&claimed);
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at=scope_001_now(),release_reason='station_unavailable' \
         WHERE lease_ref=$1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("simulate the pre-recovery release defect");
    assert_eq!(
        recover_released_orphaned_work_orders(&database)
            .await
            .expect("released orphan is safely returned to the queue"),
        1
    );
    let state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(fixture.work_order_ref)
            .fetch_one(database.pool())
            .await
            .expect("recovered work order is readable");
    assert_eq!(state, "queued");
    assert_task_state(&database, historical_task_id, "in_progress").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn late_scheduled_submission_keeps_material_without_advancing_revoked_execution() {
    let database = proof_database_for("collection_dispatch_submission_fence").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("task is claimed");
    let task = task_from_dispatch(&dispatch);
    let attempt = attempt(task.task_id(), fixture.producer_instance_id);
    assert!(matches!(
        start_producer_attempt(&database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = 'revoked' WHERE lease_ref = $1",
    )
    .bind(lease.lease_ref)
    .execute(database.pool())
    .await
    .expect("lease is revoked after attempt start");

    let submission = scheduled_submission(&task, &attempt, fixture.producer_instance_id);
    let outcome = submit_producer_package(&database, &submission)
        .await
        .expect("already observed material is retained after authority loss");
    assert!(matches!(
        outcome,
        RuntimeSubmissionOutcome::Acknowledged {
            ref execution_effect,
            ref material_admission,
            ..
        } if execution_effect == "LOST_AUTHORITY" && material_admission == "ACCEPTED"
    ));
    let package_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_capture_package")
            .fetch_one(database.pool())
            .await
            .expect("package count is readable");
    let receipt_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_submission_receipt")
            .fetch_one(database.pool())
            .await
            .expect("receipt count is readable");
    assert_eq!((package_count, receipt_count), (1, 1));
    assert!(lease_is_live(&database, lease.lease_ref).await == false);
    assert_task_state(&database, task.task_id(), "in_progress").await;
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn replacement_installation_releases_stale_work_instead_of_adopting_it() {
    let database = proof_database_for("collection_dispatch_installation_takeover").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let old_dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("old installation claims the first task");
    let original_task_id = task_id(&old_dispatch);

    open_claim_window(&database, fixture.station_ref, 1)
        .await
        .expect("the same station accepts the replacement installation");
    let intermediate_instance_id = Uuid::new_v4();
    let intermediate_install_key = intermediate_instance_id.to_string();
    let intermediate = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &intermediate_install_key,
            plugin_version: "0.8.34",
            browser_label: Some("intermediate fixture"),
            capabilities: serde_json::json!(["author_profile", "profile_discovery"]),
            installation_credential: None,
        },
    )
    .await
    .expect("intermediate installation checks in");
    let intermediate_installation_ref = match intermediate {
        CheckInOutcome::Claimed {
            installation_ref,
            superseded,
            ..
        } => {
            assert_eq!(superseded, Some(fixture.installation_ref));
            installation_ref
        }
        other => panic!("intermediate install must claim the station; got {other:?}"),
    };
    assert!(
        !lease_is_live(&database, lease.lease_ref).await,
        "superseding an installation releases its old frozen lease instead of silently adopting it"
    );

    let replacement_instance_id = Uuid::new_v4();
    let replacement_install_key = replacement_instance_id.to_string();
    let outcome = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &replacement_install_key,
            plugin_version: "0.8.34",
            browser_label: Some("replacement fixture"),
            capabilities: serde_json::json!([
                "author_profile",
                "profile_discovery",
                "content_detail",
                "media_slots",
                "comments",
                "replies"
            ]),
            installation_credential: None,
        },
    )
    .await
    .expect("replacement installation checks in");
    let replacement_installation_ref = match &outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref,
            superseded,
            ..
        } => {
            assert_eq!(*station_ref, fixture.station_ref);
            assert_eq!(*superseded, Some(intermediate_installation_ref));
            *installation_ref
        }
        other => panic!("replacement must claim the open station; got {other:?}"),
    };
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(original_task_id)
    .fetch_one(database.pool())
    .await
    .expect("task owner is readable");
    assert_ne!(owner, Some(replacement_installation_ref));
    assert_eq!(task_id(&old_dispatch), original_task_id);
    assert!(
        !lease_is_live(&database, lease.lease_ref).await,
        "a replacement requires a fresh account binding, admission and lease"
    );
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn detail_dispatch_uses_the_latest_accepted_signed_discovery_url_outside_task_spec() {
    let database = proof_database_for("collection_dispatch_signed_source").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-signed-execution";
    let signed_url = format!(
        "https://www.xiaohongshu.com/user/profile/creator-fixture/{content_external_id}?xsec_token=SIGNED_FIXTURE%3D&xsec_source=pc_user"
    );
    submit_profile_discovery(&database, content_external_id, &signed_url).await;
    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("work resource query is valid");
    let resources = read_work_resources(&database, &query)
        .await
        .expect("shared work resource interface reads the accepted discovery");
    let resource = resources
        .items
        .first()
        .expect("one work resource is projected");
    assert_eq!(
        resource.collection_context.target_display_name.as_deref(),
        Some("顺序派发夹具")
    );
    assert_eq!(
        resource.collection_context.relationship_state,
        "OBSERVED_ON_TARGET_SURFACE"
    );
    assert_eq!(
        resource.collection_context.author_identity_match_state,
        "NOT_VERIFIED"
    );
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,30,2,true)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("the work order freezes one authorized material target");

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("material deepening lease is issued");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("detail dispatch is decided");
    match dispatch {
        DispatchDecision::Dispatch {
            task_spec,
            execution_source_url,
            page_session_plan,
            ..
        } => {
            assert_eq!(task_spec["capabilitiesRequested"][0], "content_detail");
            assert_eq!(
                task_spec["target"]["contentExternalId"],
                content_external_id
            );
            assert!(
                !task_spec.to_string().contains("xsec_token"),
                "short-lived execution credentials never enter immutable TaskSpec"
            );
            assert_eq!(execution_source_url.as_deref(), Some(signed_url.as_str()));
            let plan = page_session_plan.expect("fixed detail work exposes one same-page plan");
            assert_eq!(plan["contractVersion"], "linggan.detail-page-session.v1");
            assert_eq!(plan["contentExternalId"], content_external_id);
            assert_eq!(
                plan["lanes"],
                serde_json::json!(["content_detail", "media_slots", "comments", "replies"])
            );
            assert_eq!(plan["commentLimit"], 30);
            assert_eq!(plan["replyExpandLimit"], 2);
            assert!(
                plan["cacheTtlSeconds"]
                    .as_i64()
                    .is_some_and(|value| value > 0)
            );
        }
        other => panic!("signed discovery must produce a detail dispatch; got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn detail_only_scope_claims_only_detail_and_rejects_orphaned_replies() {
    let database = proof_database_for("collection_dispatch_detail_only_scope").await;
    let fixture = seed_creator_work_order(&database).await;
    let content_external_id = "note-detail-only-execution";
    submit_profile_discovery(
        &database,
        content_external_id,
        "https://www.xiaohongshu.com/user/profile/creator-fixture/note-detail-only-execution?xsec_token=SIGNED_DETAIL_ONLY%3D&xsec_source=pc_user",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .expect("accepted discovery creates the stable content identity");
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,1,0,0,false)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .expect("detail-only is an accepted frozen scope");
    let invalid_reply_scope = sqlx::query(
        "INSERT INTO collection_work_order_material_target \
         (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media) \
         VALUES ($1,$2,2,0,1,false)",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await;
    assert!(
        invalid_reply_scope.is_err(),
        "0038 refuses replies when the approved comment scope is zero"
    );

    issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("a content-detail-capable station can lease the narrow scope");
    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("one narrow task is claimed through the shared dispatcher");
    match dispatch {
        DispatchDecision::Dispatch {
            task_spec,
            page_session_plan,
            ..
        } => {
            assert_eq!(
                task_spec["capabilitiesRequested"],
                serde_json::json!(["content_detail"])
            );
            let plan = page_session_plan.expect("detail work carries its bounded same-page plan");
            assert_eq!(plan["lanes"], serde_json::json!(["content_detail"]));
            assert_eq!(plan["commentLimit"], 0);
            assert_eq!(plan["replyExpandLimit"], 0);
        }
        other => panic!("detail-only scope must dispatch detail first; got {other:?}"),
    }
}

struct Fixture {
    work_order_ref: Uuid,
    station_ref: Uuid,
    installation_ref: Uuid,
    producer_instance_id: Uuid,
    install_key: String,
    installation_credential: String,
}

async fn proof_database() -> Database {
    proof_database_for("collection_dispatch_sequence").await
}

async fn proof_database_for(schema: &str) -> Database {
    let url = std::env::var("COLLECTION_DISPATCH_PROOF_DATABASE_URL")
        .expect("proof database URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("focused migrations apply")
}

async fn seed_creator_work_order(database: &Database) -> Fixture {
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    let install_key = producer_instance_id.to_string();

    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, source, lifecycle_state) \
         VALUES ($1, 'xhs', 'creator', 'creator-fixture', '顺序派发夹具', 'manual', 'archiving')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) \
         VALUES ($1, 'xhs', 'creator', 'deep_archive', 1, 10, \
                 ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],10, \
                 'focused sequence proof', 'person',scope_001_now() + interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("authorization is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref, target_ref, lane, purpose, requested_by) \
         VALUES ($1, $2, 'deep_archive', 'focused sequence proof', 'person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("request is seeded");
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref, request_ref, outcome, reason_code, authorization_ref) \
         VALUES ($1, $2, 'admitted', 'focused_sequence_proof', $3)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("admission is seeded");
    sqlx::query(
        "INSERT INTO execution_station (station_ref, display_name, daily_work_quota) \
         VALUES ($1, 'focused sequence station', 200)",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("station is seeded");
    sqlx::query(
        "INSERT INTO plugin_installation \
             (installation_ref, install_key, station_ref, claim_kind, claimed_at, plugin_version, capabilities) \
         VALUES ($1, $2, $3, 'person', scope_001_now(), '0.8.34', \
                 '[\"author_profile\",\"profile_discovery\",\"content_detail\",\"media_slots\",\"comments\",\"replies\"]'::jsonb)",
    )
    .bind(installation_ref)
    .bind(&install_key)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("installation is seeded");
    set_station_accepting(database, station_ref, true, "person")
        .await
        .expect("fixture station explicitly accepts new work");
    let credential = rotate_installation_credential(database, installation_ref)
        .await
        .expect("fixture installation receives a high-entropy credential");
    let installation_credential = credential.raw_credential.expose_once().to_owned();
    activate_installation_credential(
        database,
        installation_ref,
        credential.credential_ref,
        &installation_credential,
    )
    .await
    .expect("fixture activates the pending credential before reporting account state");
    let account = report_account_eligibility(
        database,
        installation_ref,
        &installation_credential,
        Some("xhs-account-dispatch-fixture"),
        AccountEligibilitySignal::AuthenticatedObserved,
        b"collection-dispatch-fixture-digest-key-32-plus",
    )
    .await
    .expect("fixture account observation is accepted");
    let account_ref = account.account_ref.expect("fixture account is projected");
    bind_observation_account(database, account_ref, installation_ref, "person")
        .await
        .expect("fixture account is explicitly bound");
    sqlx::query(
        "UPDATE collection_admission_decision \
         SET target_ref=$2,station_ref=$3,installation_ref=$4,account_ref=$5 \
         WHERE decision_ref=$1",
    )
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("fixture admission freezes the selected control tuple");
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions, \
              station_ref,installation_ref,account_ref) \
         VALUES ($1, $2, $3, 'deep_archive', 10, '[\"maximum_quota\",\"time_budget\"]'::jsonb, $4,$5,$6)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .execute(database.pool())
    .await
    .expect("work order is seeded");

    Fixture {
        work_order_ref,
        station_ref,
        installation_ref,
        producer_instance_id,
        install_key,
        installation_credential,
    }
}

async fn submit_profile_discovery(
    database: &Database,
    content_external_id: &str,
    signed_url: &str,
) {
    let task = parse_producer_task_spec(
        &serde_json::json!({
            "contractVersion":"linggan.producer.task-spec.v1",
            "taskId":Uuid::new_v4(),
            "source":"manual",
            "platform":"xhs",
            "pageType":"profile",
            "target":{"authorExternalId":"creator-fixture"},
            "capabilitiesRequested":["profile_discovery"],
            "maximumQuota":1,
            "commentLimit":"not_requested",
            "acquireMedia":"not_requested",
            "riskPolicy":"local_trusted_user_initiated",
            "stopConditions":["surface_ended","maximum_quota"]
        })
        .to_string(),
    )
    .expect("profile discovery task is valid");
    assert!(matches!(
        create_producer_task(database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let producer_instance_id = Uuid::new_v4();
    let attempt = attempt(task.task_id(), producer_instance_id);
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission = parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"profile_discovery",
                "platform":"xhs",
                "observedAt":"2026-08-30T00:00:00Z",
                "capturedAt":"2026-08-30T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":"creator-fixture"},
                    "layers":[{
                        "capability":"profile_discovery","observed":1,"attempted":1,
                        "acquired":1,"verified":0,"failed":0,"notAttempted":0,
                        "unknown":0,"stoppedReason":"surface_ended"
                    }]
                },
                "records":[{
                    "kind":"profile_discovery_card",
                    "resultPosition":1,
                    "sourceObject":{
                        "platform":"xhs","type":"content","externalId":content_external_id
                    },
                    "payload":{"title":"signed fixture","url":signed_url}
                }]
            }
        })
        .to_string(),
    )
    .expect("signed profile discovery submission is valid");
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

fn same_dispatch(
    left: DispatchDecision,
    right: DispatchDecision,
) -> (DispatchDecision, DispatchDecision) {
    match (left, right) {
        (left @ DispatchDecision::Dispatch { .. }, right @ DispatchDecision::Dispatch { .. }) => {
            (left, right)
        }
        pair => {
            panic!("concurrent callers must receive one idempotent task identity; got {pair:?}")
        }
    }
}

fn task_id(decision: &DispatchDecision) -> Uuid {
    match decision {
        DispatchDecision::Dispatch { task_id, .. } => *task_id,
        other => panic!("expected dispatch, got {other:?}"),
    }
}

fn capability(decision: &DispatchDecision) -> &str {
    match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec["capabilitiesRequested"][0]
            .as_str()
            .expect("scheduled capability is present"),
        other => panic!("expected dispatch, got {other:?}"),
    }
}

fn task_from_dispatch(decision: &DispatchDecision) -> ProducerTaskSpec {
    let task_spec = match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec,
        other => panic!("expected dispatch, got {other:?}"),
    };
    parse_producer_task_spec(&task_spec.to_string()).expect("dispatched task remains valid")
}

fn attempt(task_id: Uuid, producer_instance_id: Uuid) -> linggan_contracts::ProducerAttempt {
    parse_producer_attempt(
        &serde_json::json!({
            "contractVersion": "linggan.producer.attempt.v1",
            "producerInstanceId": producer_instance_id,
            "taskId": task_id,
            "attemptId": Uuid::new_v4(),
        })
        .to_string(),
    )
    .expect("attempt is valid")
}

async fn run_scheduled_task(
    database: &Database,
    task: &ProducerTaskSpec,
    producer_instance_id: Uuid,
) {
    assert!(matches!(
        create_producer_task(database, task).await,
        Ok(RuntimeTaskOutcome::Replay { .. })
    ));
    let attempt = attempt(task.task_id(), producer_instance_id);
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission = scheduled_submission(task, &attempt, producer_instance_id);
    assert!(matches!(
        submit_producer_package(database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
}

fn scheduled_submission(
    task: &ProducerTaskSpec,
    attempt: &linggan_contracts::ProducerAttempt,
    producer_instance_id: Uuid,
) -> linggan_contracts::ProducerSubmission {
    let capability = task.raw()["capabilitiesRequested"][0]
        .as_str()
        .expect("capability is present");
    parse_producer_submission(
        &serde_json::json!({
            "contractVersion": "linggan.producer.capture-package.v1",
            "producerInstanceId": producer_instance_id,
            "taskId": task.task_id(),
            "attemptId": attempt.attempt_id(),
            "submissionId": Uuid::new_v4(),
            "capturePackage": {
                "contractVersion": "linggan.producer.capture-package.v1",
                "packageRef": Uuid::new_v4(),
                "packageKind": capability,
                "platform": "xhs",
                "observedAt": "2026-08-29T00:00:00Z",
                "capturedAt": "2026-08-29T00:00:01Z",
                "coverage": {
                    "target": {"basis": "known_set", "authorExternalId": "creator-fixture"},
                    "layers": [{
                        "capability": capability,
                        "observed": 0,
                        "attempted": 0,
                        "acquired": 0,
                        "verified": 0,
                        "failed": 0,
                        "notAttempted": 0,
                        "unknown": 0,
                        "stoppedReason": "surface_ended"
                    }]
                },
                "records": []
            }
        })
        .to_string(),
    )
    .expect("scheduled submission is valid")
}

async fn lease_is_live(database: &Database, lease_ref: Uuid) -> bool {
    sqlx::query_scalar(
        "SELECT released_at IS NULL FROM collection_work_order_lease WHERE lease_ref = $1",
    )
    .bind(lease_ref)
    .fetch_one(database.pool())
    .await
    .expect("lease is readable")
}

async fn assert_task_state(database: &Database, task_id: Uuid, expected: &str) {
    let actual: String = sqlx::query_scalar(
        "SELECT execution_state FROM collection_work_order_lease_task WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(database.pool())
    .await
    .expect("lease task state is readable");
    assert_eq!(actual, expected);
}

fn manual_task() -> ProducerTaskSpec {
    parse_producer_task_spec(
        &serde_json::json!({
            "contractVersion": "linggan.producer.task-spec.v1",
            "taskId": Uuid::new_v4(),
            "source": "manual",
            "platform": "xhs",
            "pageType": "note_detail",
            "target": {"contentExternalId": "manual-fixture"},
            "capabilitiesRequested": ["content_detail"],
            "maximumQuota": 1,
            "commentLimit": "not_requested",
            "acquireMedia": "not_requested",
            "riskPolicy": "local_trusted_user_initiated",
            "stopConditions": ["manual_stop", "maximum_quota"]
        })
        .to_string(),
    )
    .expect("manual task remains valid")
}
