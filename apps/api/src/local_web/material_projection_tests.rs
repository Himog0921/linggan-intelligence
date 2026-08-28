use super::material_projection_media_fixture::{
    assert_asset_response, assert_disposition_precedence, assert_materialization_read_contract,
    complete_ocr_derivative, seed_media, seed_media_refresh, seed_shared_media,
};
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_evidence::admit_media_blob;
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
    "\n",
    include_str!("../../../../database/migrations/0016_material_social_lanes.sql"),
    "\n",
    include_str!("../../../../database/migrations/0017_material_media_projection.sql"),
    "\n",
    include_str!("../../../../database/migrations/0018_material_discovery_lane.sql"),
    "\n",
    "INSERT",
    " INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES \
      ('0001_scope_001_capture_evidence','1'),('0002_local_001_discovery','2'), \
      ('0003_local_trusted_producer','3'),('0004_plugin_runtime_all_capabilities','4'), \
      ('0015_material_projection','15'),('0016_material_social_lanes','16'),('0017_material_media_projection','17'),('0018_material_discovery_lane','18');\n",
);

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_material_query_applies_lane_filter_instead_of_returning_unrelated_detail() {
    let database = proof_database("material_loopback_lane_filter").await;
    seed_detail(&database).await;
    let response = app_with_database(database.clone())
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
    let unfiltered = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library")
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
    let response = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=comments")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.pointer("/items/0/inspector/commentThreads"),
        Some(&json!([]))
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/commentsAccess/accessLevel")
            .and_then(Value::as_str),
        Some("RESTRICTED_SOURCE")
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/commentsAccess/bodyReturned")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/commentsAccess/externalIdentityReturned")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/commentsCoverage/countState")
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
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_media_projection_exposes_only_local_replica_and_honest_processor_candidate_states()
 {
    let database = proof_database("material_loopback_media").await;
    let observation_ref = seed_media(&database).await;
    let admission = admit_media_blob(
        &database,
        observation_ref,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/jpeg",
        12,
        "blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await
    .expect("synthetic verified blob materializes");
    let shared_observation_ref = seed_shared_media(&database).await;
    let shared_admission = admit_media_blob(
        &database,
        shared_observation_ref,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/jpeg",
        12,
        "blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await
    .expect("a second qualified materialization may share the content-addressed blob");
    let derivative_ref = complete_ocr_derivative(&database, admission.processing_jobs[1]).await;
    let blob_path = local_media_root()
        .join("blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    std::fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
    std::fs::write(&blob_path, b"proof-bytes!").unwrap();
    let derivative_path = local_media_root().join("derivatives/ocr/proof");
    std::fs::create_dir_all(derivative_path.parent().unwrap()).unwrap();
    std::fs::write(&derivative_path, b"ocr-proof-bytes").unwrap();
    seed_media_refresh(&database).await;
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=media_slots&mediaKind=image")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_media_payload(&payload);
    assert_eq!(
        payload
            .pointer("/items/0/preview/localAssetUrl")
            .and_then(Value::as_str),
        Some(admission.local_asset_path.as_str()),
        "the list and direct read use the same materialization-bound qualification handle"
    );
    let asr = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=asr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let asr: Value =
        serde_json::from_slice(&to_bytes(asr.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        asr.pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert_materialization_read_contract(
        &database,
        &admission.local_asset_path,
        admission.materialization_ref,
        &shared_admission.local_asset_path,
    )
    .await;
    let derivative_url = payload
        .pointer("/items/0/inspector/derivatives")
        .and_then(Value::as_array)
        .and_then(|values| {
            values
                .iter()
                .find(|value| value.get("kind").and_then(Value::as_str) == Some("ocr_text"))
        })
        .and_then(|value| value.pointer("/sourceLocation/localAssetUrl"))
        .and_then(Value::as_str)
        .expect("acquired OCR exposes a controlled derivative handle");
    assert_asset_response(&database, derivative_url, b"ocr-proof-bytes").await;
    assert_disposition_precedence(
        database,
        admission.materialization_ref,
        &admission.local_asset_path,
        &shared_admission.local_asset_path,
        derivative_ref,
        derivative_url,
    )
    .await;
    std::fs::remove_file(blob_path).unwrap();
    std::fs::remove_file(derivative_path).unwrap();
}

fn assert_media_payload(payload: &Value) {
    assert_eq!(
        payload
            .pointer("/items/0/inspector/mediaSlots/0/origin/candidateUriCount")
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/mediaSlots/0/origin/candidateSetState")
            .and_then(Value::as_str),
        Some("OBSERVED_SET")
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/mediaSlots/0/origin/actualDownloadCandidateState")
            .and_then(Value::as_str),
        Some("UNKNOWN")
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/mediaSlots/0/origin/sourceGeneration")
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        payload
            .pointer("/items/0/inspector/mediaSlots/0/replica/isCurrentOrigin")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        payload
            .pointer("/items/0/preview/bytesState")
            .and_then(Value::as_str),
        Some("ACQUIRED")
    );
    let ocr = payload
        .pointer("/items/0/inspector/derivatives")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|value| value.get("kind").and_then(Value::as_str) == Some("ocr_text"))
        .unwrap();
    assert_eq!(ocr.get("state").and_then(Value::as_str), Some("ACQUIRED"));
    assert!(ocr.get("sourceLocation").is_some_and(Value::is_object));
    assert_eq!(
        payload
            .pointer("/items/0/laneSummaries/7/state")
            .and_then(Value::as_str),
        Some("ACQUIRED")
    );
    assert!(
        payload
            .pointer("/items/0/preview/localAssetUrl")
            .and_then(Value::as_str)
            .is_some_and(|url| url.starts_with("/api/local/media/"))
    );
    assert!(
        !payload.to_string().contains("media.example"),
        "remote candidate URIs stay out of the ordinary read API"
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

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
