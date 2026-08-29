use linggan_contracts::{
    ProducerTaskSpec, parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    DispatchDecision, ProducerRuntimeError, RuntimeAttemptOutcome, RuntimeSubmissionOutcome,
    RuntimeTaskOutcome, create_producer_task, decide_dispatch, issue_work_order_lease,
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
    include_str!("../../../database/migrations/0019_work_order_lease_task_sequence.sql"),
);

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
        decide_dispatch(&database, &fixture.install_key),
        decide_dispatch(&database, &fixture.install_key),
    );
    let (first, first_replay) = same_dispatch(
        left.expect("first concurrent dispatch decides"),
        right.expect("second concurrent dispatch decides"),
    );
    assert_eq!(capability(&first), "author_profile");
    assert_eq!(task_id(&first), task_id(&first_replay));

    let response_loss_replay = decide_dispatch(&database, &fixture.install_key)
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

    let second = decide_dispatch(&database, &fixture.install_key)
        .await
        .expect("second sequence dispatch decides");
    assert_eq!(capability(&second), "profile_discovery");
    let second_replay = decide_dispatch(&database, &fixture.install_key)
        .await
        .expect("second response replay decides");
    assert_eq!(task_id(&second), task_id(&second_replay));
    let second_task = task_from_dispatch(&second);
    run_scheduled_task(&database, &second_task, fixture.producer_instance_id).await;
    assert_task_state(&database, second_task.task_id(), "completed").await;
    assert!(!lease_is_live(&database, lease.lease_ref).await);

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
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn scheduled_submission_rechecks_live_claim_before_admission() {
    let database = proof_database_for("collection_dispatch_submission_fence").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let dispatch = decide_dispatch(&database, &fixture.install_key)
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
    assert!(matches!(
        submit_producer_package(&database, &submission).await,
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed)
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
    assert_eq!((package_count, receipt_count), (0, 0));
}

struct Fixture {
    work_order_ref: Uuid,
    producer_instance_id: Uuid,
    install_key: String,
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
         VALUES ($1, 'xhs', 'creator', 'creator-fixture', '顺序派发夹具', 'manual', 'monitoring')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target, \
              purpose, granted_by, expires_at) \
         VALUES ($1, 'xhs', 'creator', 'patrol', 1, 10, 'focused sequence proof', 'person', \
                 scope_001_now() + interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("authorization is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref, target_ref, lane, purpose, requested_by) \
         VALUES ($1, $2, 'patrol', 'focused sequence proof', 'person')",
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
         VALUES ($1, $2, $3, 'person', scope_001_now(), '0.5.1', \
                 '[\"author_profile\",\"profile_discovery\"]'::jsonb)",
    )
    .bind(installation_ref)
    .bind(&install_key)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("installation is seeded");
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions, station_ref) \
         VALUES ($1, $2, $3, 'patrol', 10, '[\"maximum_quota\",\"time_budget\"]'::jsonb, $4)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("work order is seeded");

    Fixture {
        work_order_ref,
        producer_instance_id,
        install_key,
    }
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
