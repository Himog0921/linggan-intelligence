use super::material_asset_route_fixture::{
    LocalFixtureFiles, assert_asset_response, assert_derivative_integrity_is_checked,
    assert_integrity_and_symlink_attacks_are_unavailable, assert_materialization_read_contract,
};
use super::material_projection_media_fixture::{
    assert_disposition_precedence, complete_ocr_derivative, seed_media, seed_media_refresh,
    seed_shared_media,
};
use super::material_projection_tests::{fetch_detail, proof_database};
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_evidence::admit_media_blob;
use tower::ServiceExt;

const PROOF_BLOB_SHA256: &str = "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62";

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn loopback_media_projection_exposes_only_local_replica_and_honest_processor_candidate_states()
 {
    let database = proof_database("material_loopback_media").await;
    let observation_ref = seed_media(&database).await;
    let admission = admit_media_blob(
        &database,
        observation_ref,
        PROOF_BLOB_SHA256,
        "image/jpeg",
        12,
        "blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
    )
    .await
    .expect("synthetic verified blob materializes");
    let shared_observation_ref = seed_shared_media(&database).await;
    let shared_admission = admit_media_blob(
        &database,
        shared_observation_ref,
        PROOF_BLOB_SHA256,
        "image/jpeg",
        12,
        "blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
    )
    .await
    .expect("a second qualified materialization may share the content-addressed blob");
    let derivative_ref = complete_ocr_derivative(&database, admission.processing_jobs[1]).await;
    let blob_path = local_media_root()
        .join("blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62");
    std::fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
    std::fs::write(&blob_path, b"proof-bytes!").unwrap();
    let derivative_path = local_media_root().join("derivatives/ocr/proof");
    let _fixture_files = LocalFixtureFiles::new(vec![blob_path.clone(), derivative_path.clone()]);
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
    let list_payload: Value = serde_json::from_slice(&body).unwrap();
    assert!(list_payload.pointer("/items/0/inspector").is_none());
    let detail = fetch_detail(&database, &list_payload).await;
    let payload = detail.get("item").cloned().unwrap();
    assert_media_payload(&payload);
    assert_eq!(
        payload
            .pointer("/preview/localAssetUrl")
            .and_then(Value::as_str),
        Some(admission.local_asset_path.as_str()),
        "the list and direct read use the same materialization-bound qualification handle"
    );
    assert_asr_absent(&database).await;
    assert_materialization_read_contract(
        &database,
        &admission.local_asset_path,
        admission.materialization_ref,
        &shared_admission.local_asset_path,
    )
    .await;
    assert_integrity_and_symlink_attacks_are_unavailable(
        &database,
        &admission.local_asset_path,
        &blob_path,
    )
    .await;
    let derivative_url = payload
        .pointer("/inspector/derivatives")
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
    assert_derivative_integrity_is_checked(&database, derivative_url, &derivative_path).await;
    assert_disposition_precedence(
        database,
        admission.materialization_ref,
        &admission.local_asset_path,
        &shared_admission.local_asset_path,
        derivative_ref,
        derivative_url,
    )
    .await;
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn unknown_declared_media_is_preserved_but_never_delivered_inline() {
    const HASH: &str = "60f7a502f497dd02d7ac8be759a99f1ad7f712eacbad842880b3d3afab4770db";
    let database = proof_database("material_unknown_mime_delivery").await;
    let observation_ref = seed_media(&database).await;
    let admission = admit_media_blob(
        &database,
        observation_ref,
        HASH,
        "image/svg+xml",
        16,
        "blobs/60/60f7a502f497dd02d7ac8be759a99f1ad7f712eacbad842880b3d3afab4770db",
    )
    .await
    .expect("unknown or unsafe declared MIME is retained");
    let path = local_media_root()
        .join("blobs/60/60f7a502f497dd02d7ac8be759a99f1ad7f712eacbad842880b3d3afab4770db");
    let _fixture_files = LocalFixtureFiles::new(vec![path.clone()]);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"<svg>proof</svg>").unwrap();
    let response = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri(admission.local_asset_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/octet-stream")
    );
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .and_then(|value| value.to_str().ok()),
        Some("attachment")
    );
    assert_eq!(
        response
            .headers()
            .get("x-linggan-media-state")
            .and_then(|value| value.to_str().ok()),
        Some("UNSUPPORTED_MEDIA_TYPE")
    );
    assert_eq!(
        &to_bytes(response.into_body(), usize::MAX).await.unwrap()[..],
        b"<svg>proof</svg>"
    );
}

async fn assert_asr_absent(database: &Database) {
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=asr")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload
            .pointer("/items")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

fn assert_media_payload(payload: &Value) {
    assert_eq!(
        payload
            .pointer("/inspector/mediaSlots/0/origin/candidateUriCount")
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        payload
            .pointer("/inspector/mediaSlots/0/origin/candidateSetState")
            .and_then(Value::as_str),
        Some("OBSERVED_SET")
    );
    assert_eq!(
        payload
            .pointer("/inspector/mediaSlots/0/origin/actualDownloadCandidateState")
            .and_then(Value::as_str),
        Some("UNKNOWN")
    );
    assert_eq!(
        payload
            .pointer("/inspector/mediaSlots/0/origin/sourceGeneration")
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        payload
            .pointer("/inspector/mediaSlots/0/replica/isCurrentOrigin")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        payload
            .pointer("/preview/bytesState")
            .and_then(Value::as_str),
        Some("ACQUIRED")
    );
    let ocr = payload
        .pointer("/inspector/derivatives")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|value| value.get("kind").and_then(Value::as_str) == Some("ocr_text"))
        .unwrap();
    assert_eq!(ocr.get("state").and_then(Value::as_str), Some("ACQUIRED"));
    assert!(ocr.get("sourceLocation").is_some_and(Value::is_object));
    assert_eq!(
        payload
            .pointer("/laneSummaries/7/state")
            .and_then(Value::as_str),
        Some("ACQUIRED")
    );
    assert!(
        payload
            .pointer("/preview/localAssetUrl")
            .and_then(Value::as_str)
            .is_some_and(|url| url.starts_with("/api/local/media/"))
    );
    assert!(
        !payload.to_string().contains("media.example"),
        "remote candidate URIs stay out of the ordinary read API"
    );
}
