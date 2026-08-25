use linggan_contracts::{
    parse_local_producer_attempt, parse_local_producer_submission, parse_local_task_spec,
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    LocalAttemptOutcome, LocalSubmissionOutcome, LocalTaskOutcome, MediaUploadFinalizeClaim,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, admit_media_blob,
    begin_media_upload, claim_media_upload_finalize, complete_media_upload, create_manual_task,
    create_producer_task, read_runtime_library, record_media_upload_chunk, start_local_attempt,
    start_producer_attempt, submit_local_package, submit_producer_package,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
);

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn full_runtime_accepts_each_capability_without_collapsing_partial_media_or_replay() {
    let database = proof_database("plugin_runtime_all_capabilities").await;
    let task = parse_producer_task_spec(runtime_task_spec()).expect("runtime task is valid");
    assert!(matches!(
        create_producer_task(&database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let attempt = parse_producer_attempt(runtime_attempt()).expect("runtime attempt is valid");
    assert!(matches!(
        start_producer_attempt(&database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission =
        parse_producer_submission(runtime_submission()).expect("runtime submission is valid");
    assert!(matches!(
        submit_producer_package(&database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
    assert!(matches!(
        submit_producer_package(&database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Replay { .. })
    ));
    let slot_count: i64 = sqlx::query("SELECT count(*) AS count FROM linggan_media_slot")
        .fetch_one(database.pool())
        .await
        .expect("slots exist")
        .get("count");
    assert_eq!(
        slot_count, 2,
        "URL observations retain two stable media slots"
    );
    assert_count(&database, "linggan_runtime_record_disposition", 2).await;
    let layer = sqlx::query("SELECT coverage FROM linggan_runtime_capture_package")
        .fetch_one(database.pool())
        .await
        .expect("package exists")
        .get::<serde_json::Value, _>("coverage");
    assert_eq!(
        layer
            .pointer("/layers/0/acquired")
            .and_then(|value| value.as_i64()),
        Some(6)
    );
    assert_eq!(
        layer
            .pointer("/layers/0/notAttempted")
            .and_then(|value| value.as_i64()),
        Some(2)
    );
    let first = admit_media_blob(
        &database,
        "ffffffff-ffff-4fff-8fff-ffffffffffff"
            .parse()
            .expect("observation id"),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/jpeg",
        4,
        "blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await
    .expect("first blob admission");
    let second = admit_media_blob(
        &database,
        "11111111-2222-4333-8444-555555555555"
            .parse()
            .expect("observation id"),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/jpeg",
        4,
        "blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await
    .expect("the same bytes may serve another slot");
    assert_eq!(
        first.local_asset_path, second.local_asset_path,
        "presentation uses the Linggan local asset route"
    );
    assert_count(&database, "linggan_media_blob", 1).await;
    assert_count(&database, "linggan_media_materialization", 2).await;
    assert_count(&database, "linggan_media_processing_job_event", 4).await;
    prove_resumable_media_upload(&database).await;
    let query = serde_json::from_str(r#"{"text":null,"scope":"all_accepted_material","window":"last_30_days","sort":"latest_discovery"}"#)
        .expect("read query is valid");
    let projection = read_runtime_library(&database, &query)
        .await
        .expect("runtime read projection executes");
    assert!(
        projection.cards.is_empty(),
        "media-only packages do not fabricate content cards"
    );
}

async fn prove_resumable_media_upload(database: &Database) {
    let observation = "ffffffff-ffff-4fff-8fff-ffffffffffff"
        .parse()
        .expect("observation id");
    let sha256 = "88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589";
    let upload = begin_media_upload(
        database,
        observation,
        sha256,
        "image/jpeg",
        4,
        "uploads/synthetic/abcd.part",
    )
    .await
    .expect("upload session starts");
    assert_eq!(upload.next_offset, 0);
    assert_eq!(
        record_media_upload_chunk(database, upload.session_ref, 0, 2)
            .await
            .expect("first chunk recorded")
            .next_offset,
        2
    );
    assert_eq!(
        record_media_upload_chunk(database, upload.session_ref, 2, 2)
            .await
            .expect("second chunk recorded")
            .state,
        "ready_to_finalize"
    );
    assert!(matches!(
        claim_media_upload_finalize(database, upload.session_ref)
            .await
            .expect("upload is finalizable"),
        MediaUploadFinalizeClaim::Ready(_)
    ));
    let admission = admit_media_blob(
        database,
        observation,
        sha256,
        "image/jpeg",
        4,
        "blobs/88/88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589",
    )
    .await
    .expect("resumable blob admission");
    complete_media_upload(database, upload.session_ref, admission.download_attempt_ref)
        .await
        .expect("upload receipt persists");
    match claim_media_upload_finalize(database, upload.session_ref)
        .await
        .expect("finalize replay")
    {
        MediaUploadFinalizeClaim::Materialized(replayed) => {
            assert_eq!(replayed.materialization_ref, admission.materialization_ref);
            assert_ne!(replayed.materialization_ref, uuid::Uuid::nil());
        }
        _ => panic!("a completed upload must replay its actual materialization receipt"),
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn local_manual_submission_freezes_one_terminal_package_per_attempt() {
    let database = proof_database("local_trusted_producer").await;
    let task = parse_local_task_spec(task_spec()).expect("fixed manual task is valid");
    assert!(matches!(
        create_manual_task(&database, &task).await,
        Ok(LocalTaskOutcome::Created { .. })
    ));
    assert!(matches!(
        create_manual_task(&database, &task).await,
        Ok(LocalTaskOutcome::Replay { .. })
    ));

    let attempt = parse_local_producer_attempt(attempt_wire()).expect("attempt is valid");
    assert!(matches!(
        start_local_attempt(&database, &attempt).await,
        Ok(LocalAttemptOutcome::Started { .. })
    ));
    assert!(matches!(
        start_local_attempt(&database, &attempt).await,
        Ok(LocalAttemptOutcome::Replay { .. })
    ));

    let submission =
        parse_local_producer_submission(&submission_wire()).expect("submission is valid");
    assert!(matches!(
        submit_local_package(&database, &submission).await,
        Ok(LocalSubmissionOutcome::Acknowledged { .. })
    ));
    assert!(matches!(
        submit_local_package(&database, &submission).await,
        Ok(LocalSubmissionOutcome::Replay { .. })
    ));
    let conflicting_submission = parse_local_producer_submission(
        &submission_wire()
            .replace(
                "44444444-4444-4444-8444-444444444444",
                "55555555-5555-4555-8555-555555555555",
            )
            .replace("note-a", "note-c")
            .replace("note-b", "note-d"),
    )
    .expect("a distinct submission is valid");
    assert!(matches!(
        submit_local_package(&database, &conflicting_submission).await,
        Ok(LocalSubmissionOutcome::Conflict { .. })
    ));
    assert_count(&database, "local_trusted_task", 1).await;
    assert_count(&database, "local_trusted_attempt", 1).await;
    assert_count(&database, "local_trusted_submission", 1).await;
    assert_count(&database, "local_discovery_occurrence", 2).await;

    let next_attempt_wire = attempt_wire().replace(
        "33333333-3333-4333-8333-333333333333",
        "66666666-6666-4666-8666-666666666666",
    );
    let next_attempt =
        parse_local_producer_attempt(&next_attempt_wire).expect("new attempt is valid");
    assert!(matches!(
        start_local_attempt(&database, &next_attempt).await,
        Ok(LocalAttemptOutcome::Started { .. })
    ));
    let next_submission_wire = submission_wire()
        .replace(
            "33333333-3333-4333-8333-333333333333",
            "66666666-6666-4666-8666-666666666666",
        )
        .replace(
            "44444444-4444-4444-8444-444444444444",
            "77777777-7777-4777-8777-777777777777",
        )
        .replace("note-a", "note-c")
        .replace("note-b", "note-d");
    let next_submission = parse_local_producer_submission(&next_submission_wire)
        .expect("new attempt submission is valid");
    assert!(matches!(
        submit_local_package(&database, &next_submission).await,
        Ok(LocalSubmissionOutcome::Acknowledged { .. })
    ));
    assert_count(&database, "local_trusted_attempt", 2).await;
    assert_count(&database, "local_trusted_submission", 2).await;
    assert_count(&database, "local_discovery_occurrence", 4).await;
    let coverage: i32 = sqlx::query("SELECT visible_cards FROM local_discovery_coverage")
        .fetch_one(database.pool())
        .await
        .expect("coverage exists")
        .get("visible_cards");
    assert_eq!(coverage, 2, "partial visible cards remain accepted facts");
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}

async fn assert_count(database: &Database, table: &str, expected: i64) {
    let statement = match table {
        "local_trusted_task" => "SELECT count(*) AS count FROM local_trusted_task",
        "local_trusted_attempt" => "SELECT count(*) AS count FROM local_trusted_attempt",
        "local_trusted_submission" => "SELECT count(*) AS count FROM local_trusted_submission",
        "local_discovery_occurrence" => "SELECT count(*) AS count FROM local_discovery_occurrence",
        "linggan_media_blob" => "SELECT count(*) AS count FROM linggan_media_blob",
        "linggan_media_materialization" => {
            "SELECT count(*) AS count FROM linggan_media_materialization"
        }
        "linggan_media_processing_job_event" => {
            "SELECT count(*) AS count FROM linggan_media_processing_job_event"
        }
        "linggan_runtime_record_disposition" => {
            "SELECT count(*) AS count FROM linggan_runtime_record_disposition"
        }
        _ => panic!("the proof only permits named LOCAL-TRUSTED tables"),
    };
    let actual: i64 = sqlx::query(statement)
        .fetch_one(database.pool())
        .await
        .expect("count works")
        .get("count");
    assert_eq!(actual, expected, "unexpected {table} count");
}

fn task_spec() -> &'static str {
    r#"{"contractVersion":"linggan.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"manual","platform":"xhs","pageType":"search_results","target":"current_visible_search_surface","capabilitiesRequested":["discover_visible_cards"],"maximumQuota":20,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["current_surface_read_once","maximum_quota"]}"#
}

fn attempt_wire() -> &'static str {
    r#"{"contractVersion":"linggan.local-trusted.attempt.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333"}"#
}

#[allow(clippy::useless_format)]
fn submission_wire() -> String {
    // `format!` intentionally unescapes the doubled JSON object braces in this fixed fixture.
    format!(
        r#"{{"contractVersion":"linggan.local-trusted.submission.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333","submissionId":"44444444-4444-4444-8444-444444444444","discoveryPackage":{{"contractVersion":"xhs.discovery.visible-card.v1","acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},"observedAt":"2026-08-25T00:00:00Z","coverage":{{"unit":"visible_search_card","visibleCards":2,"stoppedReason":"risk_control"}},"cards":[{{"content":{{"platformContentId":"note-a","title":"synthetic A"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"2026-08-25T00:00:00Z","resultPosition":1}}}},{{"content":{{"platformContentId":"note-b","title":"synthetic B"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"2026-08-25T00:00:00Z","resultPosition":2}}}}]}}}}"#,
    )
}

fn runtime_task_spec() -> &'static str {
    r#"{"contractVersion":"linggan.producer.task-spec.v1","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","source":"manual","platform":"xhs","pageType":"note_detail","target":{"contentExternalId":"note-a"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#
}

fn runtime_attempt() -> &'static str {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc"}"#
}

fn runtime_submission() -> &'static str {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","submissionId":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee","packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-a"},"layers":[{"capability":"media_slots","observed":9,"attempted":7,"acquired":6,"verified":6,"failed":1,"notAttempted":2,"unknown":0,"stoppedReason":"risk_control"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-a:image:1","slot":{"role":"image","ordinal":1},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://cdn.example/one.jpg"},"observationRef":"ffffffff-ffff-4fff-8fff-ffffffffffff"},{"kind":"media_slot","slotKey":"xhs:note-a:image:2","slot":{"role":"image","ordinal":2},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://cdn.example/two.jpg"},"observationRef":"11111111-2222-4333-8444-555555555555"}]}}"#
}
