use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

const MIGRATIONS: &str = concat!(
    "CREATE TABLE linggan_local_schema_migration (migration_id text PRIMARY KEY, migration_sha256 text NOT NULL, applied_at timestamptz NOT NULL DEFAULT clock_timestamp());\n",
    include_str!("../../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../../database/migrations/0002_local_001_discovery.sql"),
    "\n",
    include_str!("../../../../database/migrations/0003_local_trusted_producer.sql"),
    "\n",
    include_str!("../../../../database/migrations/0004_plugin_runtime_all_capabilities.sql"),
    "\n",
    include_str!("../../../../database/migrations/0015_material_projection.sql"),
    "\nINSERT INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES \
      ('0001_scope_001_capture_evidence','1'),('0002_local_001_discovery','2'), \
      ('0003_local_trusted_producer','3'),('0004_plugin_runtime_all_capabilities','4'), \
      ('0015_material_projection','15');\n",
);

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_material_query_applies_lane_filter_instead_of_returning_unrelated_detail() {
    let database = proof_database("material_loopback_lane_filter").await;
    seed_detail(&database).await;
    let response = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?q=可检索&lane=comments")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.pointer("/queryScope").and_then(Value::as_str),
        Some("accepted_typed_material_text_only")
    );
    assert_eq!(
        payload
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

async fn seed_detail(database: &Database) {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let task = json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual",
        "platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"note-api-1"},
        "capabilitiesRequested":["content_detail"],"maximumQuota":1,"commentLimit":"not_requested",
        "acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]
    });
    let task = parse_producer_task_spec(&task.to_string()).unwrap();
    create_producer_task(database, &task).await.unwrap();
    let attempt = json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id});
    let attempt = parse_producer_attempt(&attempt.to_string()).unwrap();
    start_producer_attempt(database, &attempt).await.unwrap();
    let package = json!({
        "contractVersion":"linggan.producer.capture-package.v1","packageRef":uuid::Uuid::new_v4(),
        "packageKind":"content_detail","platform":"xhs","observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:01Z",
        "coverage":{"target":{"basis":"known_set","contentExternalId":"note-api-1"},"layers":[{"capability":"content_detail","observed":1,"attempted":1,"acquired":1,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"fixture_complete"}]},
        "records":[{"kind":"content_detail","sourceObject":{"externalId":"note-api-1"},"payload":{"title":"可检索标题","bodyText":"受限正文"}}]
    });
    let submission = json!({"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,"submissionId":uuid::Uuid::new_v4(),"capturePackage":package});
    let submission = parse_producer_submission(&submission.to_string()).unwrap();
    submit_producer_package(database, &submission)
        .await
        .unwrap();
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
