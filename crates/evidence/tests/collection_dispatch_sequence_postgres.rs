use linggan_contracts::{
    ProducerTaskSpec, parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    CheckInOutcome, DispatchDecision, InstallationCheckIn, ProducerRuntimeError,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, check_in_installation,
    create_producer_task, decide_dispatch, issue_work_order_lease, open_claim_window,
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
);

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn enabled_pending_target_is_automatically_dispatched_once_and_heartbeat_is_visible() {
    let database = proof_database_for("collection_scheduler_enabled_target").await;
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state,monitoring_enabled) \
         VALUES ($1,'xhs','creator','creator-auto-proof','自动调度证明','manual','pending_decision',true)",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target,purpose,granted_by,expires_at) \
         VALUES ($1,'xhs','creator','deep_archive',10,20,'automatic baseline proof','person',scope_001_now()+interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO execution_station (station_ref,display_name,daily_work_quota) \
         VALUES ($1,'automatic scheduler station',200)",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO plugin_installation \
             (installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) \
         VALUES ($1,$2,$3,'person',scope_001_now(),'0.6.0', \
                 '[\"author_profile\",\"profile_discovery\",\"discovery_search\"]'::jsonb)",
    )
    .bind(installation_ref)
    .bind(Uuid::new_v4().to_string())
    .bind(station_ref)
    .execute(database.pool())
    .await
    .unwrap();

    linggan_evidence::record_scheduler_started(&database, Uuid::new_v4())
        .await
        .unwrap();
    let first = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert_eq!(first.dispatched, vec![target_ref]);
    let work_order_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(work_order_count, 1);
    let live_lease_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order w JOIN collection_work_order_lease l USING(work_order_ref) \
         WHERE w.target_ref=$1 AND l.released_at IS NULL",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(live_lease_count, 1);

    let second = linggan_evidence::run_due_patrols(&database).await.unwrap();
    assert!(second.dispatched.is_empty());
    let work_order_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(work_order_count_after, 1, "a live lease is not duplicated");
    let heartbeat = linggan_evidence::read_scheduler_heartbeat(&database)
        .await
        .unwrap()
        .expect("scheduler heartbeat exists");
    assert_eq!(heartbeat.state, "running");
    assert_eq!(heartbeat.last_outcome, "partial");
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
    let lifecycle_state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target \
         WHERE identity_key='creator-fixture'",
    )
    .fetch_one(database.pool())
    .await
    .expect("completed baseline advances target lifecycle");
    assert_eq!(lifecycle_state, "archived");

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
async fn late_scheduled_submission_keeps_material_without_advancing_revoked_execution() {
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
async fn replacement_installation_adopts_a_stale_two_generation_task_without_recreating_it() {
    let database = proof_database_for("collection_dispatch_installation_takeover").await;
    let fixture = seed_creator_work_order(&database).await;
    let lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("creator work order is leased");
    let old_dispatch = decide_dispatch(&database, &fixture.install_key)
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
            plugin_version: "0.8.1",
            browser_label: Some("intermediate fixture"),
            capabilities: serde_json::json!(["author_profile", "profile_discovery"]),
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
    // Recreate the already-observed pre-fix state: 0.8.1 is active, while the live task still
    // names 0.8.0. The next version must not depend on an intermediate heartbeat to repair it.
    sqlx::query(
        "UPDATE collection_work_order_lease_task SET claimed_by_installation_ref=$2 WHERE task_id=$1",
    )
    .bind(original_task_id)
    .bind(fixture.installation_ref)
    .execute(database.pool())
    .await
    .expect("stale two-generation ownership is recreated");

    let replacement_instance_id = Uuid::new_v4();
    let replacement_install_key = replacement_instance_id.to_string();
    let outcome = check_in_installation(
        &database,
        &InstallationCheckIn {
            install_key: &replacement_install_key,
            plugin_version: "0.8.2",
            browser_label: Some("replacement fixture"),
            capabilities: serde_json::json!([
                "author_profile",
                "profile_discovery",
                "content_detail",
                "media_slots",
                "comments",
                "replies"
            ]),
        },
    )
    .await
    .expect("replacement installation checks in");
    let replacement_installation_ref = match outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref,
            superseded,
        } => {
            assert_eq!(station_ref, fixture.station_ref);
            assert_eq!(superseded, Some(intermediate_installation_ref));
            installation_ref
        }
        other => panic!("replacement must claim the open station; got {other:?}"),
    };

    let replacement_dispatch = decide_dispatch(&database, &replacement_install_key)
        .await
        .expect("replacement installation receives the live task");
    assert_eq!(task_id(&replacement_dispatch), original_task_id);
    let owner: Uuid = sqlx::query_scalar(
        "SELECT claimed_by_installation_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(original_task_id)
    .fetch_one(database.pool())
    .await
    .expect("task owner is readable");
    assert_eq!(owner, replacement_installation_ref);
    let task = task_from_dispatch(&replacement_dispatch);
    let replacement_attempt = attempt(task.task_id(), replacement_instance_id);
    assert!(matches!(
        start_producer_attempt(&database, &replacement_attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    assert!(lease_is_live(&database, lease.lease_ref).await);
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
    let dispatch = decide_dispatch(&database, &fixture.install_key)
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

struct Fixture {
    work_order_ref: Uuid,
    station_ref: Uuid,
    installation_ref: Uuid,
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
         VALUES ($1, 'xhs', 'creator', 'creator-fixture', '顺序派发夹具', 'manual', 'archiving')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is seeded");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, max_targets, max_works_per_target, \
              purpose, granted_by, expires_at) \
         VALUES ($1, 'xhs', 'creator', 'deep_archive', 1, 10, 'focused sequence proof', 'person', \
                 scope_001_now() + interval '1 day')",
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
         VALUES ($1, $2, $3, 'person', scope_001_now(), '0.5.1', \
                 '[\"author_profile\",\"profile_discovery\",\"content_detail\",\"media_slots\",\"comments\",\"replies\"]'::jsonb)",
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
         VALUES ($1, $2, $3, 'deep_archive', 10, '[\"maximum_quota\",\"time_budget\"]'::jsonb, $4)",
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
        station_ref,
        installation_ref,
        producer_instance_id,
        install_key,
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
