use super::material_asset_route_fixture::assert_asset_response;
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_evidence::{
    MaterialMediaDisposition, record_blob_disposition, record_derivative_disposition,
    record_materialization_disposition, record_media_derivative_completion,
};
use tower::ServiceExt;

const PROOF_BLOB_SHA256: &str = "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62";
const OCR_PROOF_SHA256: &str = "1320b046a60f7c39a3480dea50b655ca92ce61db269ea07e4037e7a6f0788e5a";

pub(super) async fn assert_disposition_precedence(
    database: Database,
    materialization_ref: uuid::Uuid,
    materialization_url: &str,
    shared_materialization_url: &str,
    derivative_ref: uuid::Uuid,
    derivative_url: &str,
) {
    record_materialization_disposition(
        &database,
        materialization_ref,
        MaterialMediaDisposition::BytesCleaned,
        "proof-authority",
        "retention-proof",
    )
    .await
    .unwrap();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?restriction=BYTES_CLEANED")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cleaned: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        cleaned
            .pointer("/items/0/preview/bytesState")
            .and_then(Value::as_str),
        Some("BYTES_CLEANED")
    );
    assert_eq!(
        cleaned.pointer("/items/0/preview/localAssetUrl"),
        Some(&Value::Null)
    );
    let gated = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(materialization_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let gated: Value =
        serde_json::from_slice(&to_bytes(gated.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        gated.pointer("/code").and_then(Value::as_str),
        Some("local_media_not_found")
    );
    assert_asset_response(&database, shared_materialization_url, b"proof-bytes!").await;
    assert_derivative_disposition(&database, derivative_ref, derivative_url).await;
    record_materialization_disposition(
        &database,
        materialization_ref,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "proof-authority",
        "rights-proof",
    )
    .await
    .unwrap();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?restriction=WITHDRAWN_OR_RESTRICTED")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let restricted: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        restricted
            .pointer("/items/0/preview/bytesState")
            .and_then(Value::as_str),
        Some("WITHDRAWN_OR_RESTRICTED")
    );
    assert_eq!(
        restricted.pointer("/items/0/preview/localAssetUrl"),
        Some(&Value::Null)
    );
    assert_asset_response(&database, shared_materialization_url, b"proof-bytes!").await;
    record_blob_disposition(
        &database,
        PROOF_BLOB_SHA256,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "proof-authority",
        "global-blob-rights",
    )
    .await
    .unwrap();
    for uri in [materialization_url, shared_materialization_url] {
        let gated = app_with_database(database.clone())
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(gated.status(), StatusCode::NOT_FOUND);
    }
}

async fn assert_derivative_disposition(
    database: &Database,
    derivative_ref: uuid::Uuid,
    derivative_url: &str,
) {
    record_derivative_disposition(
        database,
        derivative_ref,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "proof-authority",
        "derivative-rights",
    )
    .await
    .unwrap();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=ocr&restriction=WITHDRAWN_OR_RESTRICTED")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let derivative = payload
        .pointer("/items/0/inspector/derivatives")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|value| value.get("kind").and_then(Value::as_str) == Some("ocr_text"))
        .unwrap();
    assert_eq!(
        derivative.get("state").and_then(Value::as_str),
        Some("WITHDRAWN_OR_RESTRICTED")
    );
    assert_eq!(derivative.get("sourceLocation"), Some(&Value::Null));
    let gated = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(derivative_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gated.status(), StatusCode::NOT_FOUND);
}

pub(super) async fn seed_media(database: &Database) -> uuid::Uuid {
    let task_id = uuid::Uuid::new_v4();
    let producer_instance_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let observation_ref = uuid::Uuid::new_v4();
    let task = json!({"contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual","platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"note-api-media"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]});
    let task = parse_producer_task_spec(&task.to_string()).unwrap();
    create_producer_task(database, &task).await.unwrap();
    let attempt = json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id});
    start_producer_attempt(
        database,
        &parse_producer_attempt(&attempt.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":uuid::Uuid::new_v4(),"packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-api-media"},"layers":[{"capability":"media_slots","observed":1,"attempted":0,"acquired":0,"verified":0,"failed":0,"notAttempted":1,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-api-media:image:1","observationRef":observation_ref,"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/primary","candidateUris":["https://media.example/primary","https://media.example/backup"],"observedAt":"2026-08-28T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-api-media"}}]});
    submit(database, producer_instance_id, task_id, attempt_id, package).await;
    observation_ref
}

pub(super) async fn seed_shared_media(database: &Database) -> uuid::Uuid {
    let task_id = uuid::Uuid::new_v4();
    let producer_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let observation_ref = uuid::Uuid::new_v4();
    let task = json!({"contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual","platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"note-api-media-shared"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]});
    create_producer_task(
        database,
        &parse_producer_task_spec(&task.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let attempt = json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_id,"taskId":task_id,"attemptId":attempt_id});
    start_producer_attempt(
        database,
        &parse_producer_attempt(&attempt.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":uuid::Uuid::new_v4(),"packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-28T10:00:00Z","capturedAt":"2026-08-28T10:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-api-media-shared"},"layers":[{"capability":"media_slots","observed":1,"attempted":0,"acquired":0,"verified":0,"failed":0,"notAttempted":1,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-api-media-shared:image:1","observationRef":observation_ref,"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/shared","candidateUris":["https://media.example/shared"],"observedAt":"2026-08-28T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-api-media-shared"}}]});
    submit(database, producer_id, task_id, attempt_id, package).await;
    observation_ref
}

pub(super) async fn complete_ocr_derivative(
    database: &Database,
    job_ref: uuid::Uuid,
) -> uuid::Uuid {
    record_media_derivative_completion(
        database,
        job_ref,
        "ocr_text",
        OCR_PROOF_SHA256,
        Some("derivatives/ocr/proof"),
    )
    .await
    .unwrap()
}

pub(super) async fn seed_media_refresh(database: &Database) {
    let task_id = uuid::Uuid::new_v4();
    let producer_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let observation_ref = uuid::Uuid::new_v4();
    let task = json!({"contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual","platform":"xhs","pageType":"synthetic_material_proof","target":{"contentExternalId":"note-api-media"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]});
    create_producer_task(
        database,
        &parse_producer_task_spec(&task.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let attempt = json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_id,"taskId":task_id,"attemptId":attempt_id});
    start_producer_attempt(
        database,
        &parse_producer_attempt(&attempt.to_string()).unwrap(),
    )
    .await
    .unwrap();
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":uuid::Uuid::new_v4(),"packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-28T10:05:00Z","capturedAt":"2026-08-28T10:05:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-api-media"},"layers":[{"capability":"media_slots","observed":1,"attempted":0,"acquired":0,"verified":0,"failed":0,"notAttempted":1,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-api-media:image:1","observationRef":observation_ref,"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/refreshed","candidateUris":["https://media.example/refreshed","https://media.example/refreshed-backup"],"observedAt":"2026-08-28T10:05:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"note-api-media"}}]});
    submit(database, producer_id, task_id, attempt_id, package).await;
}

async fn submit(
    database: &Database,
    producer_id: uuid::Uuid,
    task_id: uuid::Uuid,
    attempt_id: uuid::Uuid,
    package: Value,
) {
    let submission = json!({"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":producer_id,"taskId":task_id,"attemptId":attempt_id,"submissionId":uuid::Uuid::new_v4(),"capturePackage":package});
    submit_producer_package(
        database,
        &parse_producer_submission(&submission.to_string()).unwrap(),
    )
    .await
    .unwrap();
}
