use linggan_contracts::{
    parse_local_producer_attempt, parse_local_producer_submission, parse_local_task_spec,
};
use linggan_evidence::{
    LocalAttemptOutcome, LocalSubmissionOutcome, LocalTaskOutcome, create_manual_task,
    start_local_attempt, submit_local_package,
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
    include_str!("../../../database/migrations/0004_local_001_discovery_surface_coverage.sql"),
);

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
        r#"{{"contractVersion":"linggan.local-trusted.submission.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333","submissionId":"44444444-4444-4444-8444-444444444444","discoveryPackage":{{"contractVersion":"xhs.discovery.visible-card.v1","acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},"observedAt":"2026-08-25T00:00:00Z","coverage":{{"unit":"visible_search_card","visibleCards":2,"discoveredCards":2,"emittedCards":2,"failedCards":0,"notAttemptedCards":0,"stoppedReason":"risk_control"}},"cards":[{{"content":{{"platformContentId":"note-a","title":"synthetic A"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"2026-08-25T00:00:00Z","resultPosition":1}}}},{{"content":{{"platformContentId":"note-b","title":"synthetic B"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"2026-08-25T00:00:00Z","resultPosition":2}}}}]}}}}"#,
    )
}
