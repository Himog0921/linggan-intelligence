use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

const MIGRATIONS: &str = full_schema_fixture::FULL_MIGRATIONS;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_material_query_applies_lane_filter_instead_of_returning_unrelated_detail() {
    let database = proof_database("material_loopback_lane_filter").await;
    seed_detail(&database).await;
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/work-resources?q=可检索&lane=comments")
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
    let unfiltered = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/work-resources")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let unfiltered: Value =
        serde_json::from_slice(&to_bytes(unfiltered.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    for ordinal in [5, 6, 7, 8] {
        assert_eq!(
            unfiltered
                .pointer(&format!("/items/0/laneSummaries/{ordinal}/state"))
                .and_then(Value::as_str),
            Some("UNKNOWN")
        );
        assert_eq!(
            unfiltered.pointer(&format!("/items/0/laneSummaries/{ordinal}/observed")),
            Some(&Value::Null)
        );
        assert_eq!(
            unfiltered
                .pointer(&format!("/items/0/laneSummaries/{ordinal}/valueState"))
                .and_then(Value::as_str),
            Some("UNKNOWN")
        );
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_comment_lane_hides_sensitive_body_and_external_identity() {
    let database = proof_database("material_loopback_comments").await;
    seed_comment(&database).await;
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/work-resources?lane=comments")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert!(payload.pointer("/items/0/inspector").is_none());
    let detail = fetch_detail(&database, &payload).await;
    assert_eq!(
        detail.pointer("/item/inspector/commentThreads"),
        Some(&json!([]))
    );
    assert_eq!(
        detail
            .pointer("/item/inspector/commentsAccess/accessLevel")
            .and_then(Value::as_str),
        Some("RESTRICTED_SOURCE")
    );
    assert_eq!(
        detail
            .pointer("/item/inspector/commentsAccess/bodyReturned")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        detail
            .pointer("/item/inspector/commentsAccess/externalIdentityReturned")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        detail
            .pointer("/item/inspector/commentsCoverage/countState")
            .and_then(Value::as_str),
        Some("UNKNOWN")
    );
    assert_eq!(
        payload
            .pointer("/items/0/laneSummaries/1/state")
            .and_then(Value::as_str),
        Some("UNKNOWN"),
        "a comments-only observation must not manufacture searchable detail"
    );
    assert_eq!(
        payload
            .pointer("/items/0/laneSummaries/2/state")
            .and_then(Value::as_str),
        Some("PARTIAL")
    );
    assert!(!payload.to_string().contains("评论命中"));
    assert!(!payload.to_string().contains("secret-raw-token"));
    assert!(!payload.to_string().contains("comment-api-1"));
    let comments_url = detail
        .pointer("/channels/comments/url")
        .and_then(Value::as_str)
        .unwrap();
    let research = request_json(&database, comments_url).await;
    assert_eq!(
        research.pointer("/accessLevel").and_then(Value::as_str),
        Some("LOCAL_AUTHORIZED_RESEARCH")
    );
    assert!(research.to_string().contains("评论命中"));
    assert!(!research.to_string().contains("comment-api-1"));
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn detail_exposes_the_bounded_reobservation_action_and_refuses_targetless_fallback() {
    let database = proof_database("material_loopback_reobservation_boundary").await;
    seed_detail(&database).await;
    let list = request_json(&database, "/api/local/work-resources").await;
    let detail = fetch_detail(&database, &list).await;
    assert_eq!(
        detail.pointer("/channels/reobservation/url"),
        Some(&Value::Null),
        "a work without an active linked authorization has no actionable URL"
    );
    assert_eq!(
        detail
            .pointer("/channels/reobservation/available")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        detail
            .pointer("/channels/reobservation/eligible")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        detail
            .pointer("/channels/reobservation/supported")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        detail
            .pointer("/channels/reobservation/reason")
            .and_then(Value::as_str),
        Some("TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED")
    );
    assert_eq!(
        detail
            .pointer("/channels/reobservation/requires")
            .and_then(Value::as_str),
        Some("TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION")
    );
    let public_ref = detail
        .pointer("/item/identity/publicRef")
        .and_then(Value::as_str)
        .expect("detail keeps the stable work reference");
    let response = app_with_database(database)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/local/work-resources/{public_ref}/reobserve"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        body.get("code").and_then(Value::as_str),
        Some("reobservation_authorization_not_linked"),
        "a work without target-linked authorization never falls back to its author or a guessed target"
    );
}

pub(super) async fn fetch_detail(database: &Database, list: &Value) -> Value {
    let url = list
        .pointer("/items/0/detailUrl")
        .and_then(Value::as_str)
        .expect("list item exposes its bounded detail URL");
    request_json(database, url).await
}

async fn request_json(database: &Database, url: &str) -> Value {
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(url)
                .header("Host", "127.0.0.1:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
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
        "records":[{"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-api-1"},"payload":{"title":"可检索标题","bodyText":"受限正文"}}]
    });
    let submission = json!({"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,"submissionId":uuid::Uuid::new_v4(),"capturePackage":package});
    let submission = parse_producer_submission(&submission.to_string()).unwrap();
    submit_producer_package(database, &submission)
        .await
        .unwrap();
}

async fn seed_comment(database: &Database) {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let task = json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual",
        "platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"note-api-comments"},
        "capabilitiesRequested":["comments"],"maximumQuota":3,"commentLimit":3,
        "acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]
    });
    let task = parse_producer_task_spec(&task.to_string()).unwrap();
    create_producer_task(database, &task).await.unwrap();
    let attempt = json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id});
    let attempt = parse_producer_attempt(&attempt.to_string()).unwrap();
    start_producer_attempt(database, &attempt).await.unwrap();
    let package = json!({
        "contractVersion":"linggan.producer.capture-package.v1","packageRef":uuid::Uuid::new_v4(),
        "packageKind":"comments","platform":"xhs","observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:01Z",
        "coverage":{"target":{"basis":"known_set","contentExternalId":"note-api-comments"},"layers":[{"capability":"comments","observed":2,"attempted":1,"acquired":1,"verified":0,"failed":0,"notAttempted":1,"unknown":1,"stoppedReason":"collector_budget"}]},
        "records":[{"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"note-api-comments"},"payload":{"commentId":"comment-api-1","noteId":"note-api-comments","text":"评论命中 secret-raw-token"}}]
    });
    let submission = json!({"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,"submissionId":uuid::Uuid::new_v4(),"capturePackage":package});
    let submission = parse_producer_submission(&submission.to_string()).unwrap();
    submit_producer_package(database, &submission)
        .await
        .unwrap();
}

pub(super) async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
