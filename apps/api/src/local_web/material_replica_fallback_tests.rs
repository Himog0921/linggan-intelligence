use super::material_asset_route_fixture::{LocalFixtureFiles, assert_asset_response};
use super::material_projection_media_fixture::seed_media;
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_evidence::{
    MaterialMediaDisposition, admit_media_blob, record_blob_disposition,
    record_materialization_disposition, record_slot_disposition,
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

const M1_SHA256: &str = "0a8c8287ec12c5cebead27e5212d079358a60d6735f855f5083fe5390d33ca77";
const M2_SHA256: &str = "852a8bcbfad35ef908c0a4e05c637223146bd4fe5d6caf269af23879a057ae93";
const M3_SHA256: &str = "09be326f576bbc385eedacc9fd29a7b33fb5262b075dd12521cd2c31e5269e1c";
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
    include_str!("../../../../database/migrations/0020_observation_runtime_automation.sql"),
    "\n",
    "INSERT",
    " INTO linggan_local_schema_migration (migration_id,migration_sha256) VALUES ('0001_scope_001_capture_evidence','1'),('0002_local_001_discovery','2'),('0003_local_trusted_producer','3'),('0004_plugin_runtime_all_capabilities','4'),('0015_material_projection','15'),('0016_material_social_lanes','16'),('0017_material_media_projection','17'),('0018_material_discovery_lane','18'),('0020_observation_runtime_automation','20');\n",
);

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn latest_disposed_replica_falls_back_to_the_newest_qualified_materialization() {
    let database = proof_database("material_replica_fallback").await;
    let observation_ref = seed_media(&database).await;
    let m1 = admit_media_blob(
        &database,
        observation_ref,
        M1_SHA256,
        "image/jpeg",
        18,
        "blobs/0a/0a8c8287ec12c5cebead27e5212d079358a60d6735f855f5083fe5390d33ca77",
    )
    .await
    .unwrap();
    let _fixture_files = LocalFixtureFiles::new(vec![
        fixture_path(M1_SHA256, "0a"),
        fixture_path(M2_SHA256, "85"),
        fixture_path(M3_SHA256, "09"),
    ]);
    let m2 = admit_media_blob(
        &database,
        observation_ref,
        M2_SHA256,
        "image/jpeg",
        17,
        "blobs/85/852a8bcbfad35ef908c0a4e05c637223146bd4fe5d6caf269af23879a057ae93",
    )
    .await
    .unwrap();
    write_fixture(M1_SHA256, "0a", b"fallback-old-bytes");
    write_fixture(M2_SHA256, "85", b"newer-proof-bytes");

    let before = library(&database).await;
    assert_eq!(asset_url(&before), Some(m2.local_asset_path.as_str()));
    assert_asset_response(&database, &m2.local_asset_path, b"newer-proof-bytes").await;

    record_materialization_disposition(
        &database,
        m2.materialization_ref,
        MaterialMediaDisposition::BytesCleaned,
        "fallback-proof",
        "newest-replica-cleaned",
    )
    .await
    .unwrap();
    let after = library(&database).await;
    assert_eq!(asset_url(&after), Some(m1.local_asset_path.as_str()));
    assert_eq!(
        after.pointer("/preview/bytesState").and_then(Value::as_str),
        Some("ACQUIRED")
    );
    assert!(
        after
            .pointer("/inspector/mediaSlots/0/replicaSelection/limitations")
            .and_then(Value::as_array)
            .is_some_and(|values| values
                .iter()
                .any(|value| value == "NEWER_MATERIALIZATION_DISPOSED"))
    );
    assert_asset_response(&database, &m1.local_asset_path, b"fallback-old-bytes").await;
    let denied = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(&m2.local_asset_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    assert_blob_disposition_falls_back(&database, observation_ref, &m1.local_asset_path).await;
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn slot_disposition_blocks_every_materialization_without_fallback() {
    let database = proof_database("material_slot_disposition").await;
    let observation_ref = seed_media(&database).await;
    let m1 = admit_media_blob(
        &database,
        observation_ref,
        M1_SHA256,
        "image/jpeg",
        18,
        "blobs/0a/0a8c8287ec12c5cebead27e5212d079358a60d6735f855f5083fe5390d33ca77",
    )
    .await
    .unwrap();
    let m2 = admit_media_blob(
        &database,
        observation_ref,
        M2_SHA256,
        "image/jpeg",
        17,
        "blobs/85/852a8bcbfad35ef908c0a4e05c637223146bd4fe5d6caf269af23879a057ae93",
    )
    .await
    .unwrap();
    let _fixture_files = LocalFixtureFiles::new(vec![
        fixture_path(M1_SHA256, "0a"),
        fixture_path(M2_SHA256, "85"),
    ]);
    write_fixture(M1_SHA256, "0a", b"fallback-old-bytes");
    write_fixture(M2_SHA256, "85", b"newer-proof-bytes");
    record_slot_disposition(
        &database,
        "xhs:note-api-media:image:1",
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "slot-proof",
        "whole-slot-withdrawn",
    )
    .await
    .unwrap();
    let item = library(&database).await;
    assert_eq!(asset_url(&item), None);
    assert!(
        item.pointer("/inspector/limitations")
            .and_then(Value::as_array)
            .is_some_and(|values| values
                .iter()
                .any(|value| value == "SLOT_WITHDRAWN_OR_RESTRICTED"))
    );
    for url in [&m1.local_asset_path, &m2.local_asset_path] {
        let response = app_with_database(database.clone())
            .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

async fn assert_blob_disposition_falls_back(
    database: &Database,
    observation_ref: uuid::Uuid,
    healthy_url: &str,
) {
    let m3 = admit_media_blob(
        database,
        observation_ref,
        M3_SHA256,
        "image/jpeg",
        17,
        "blobs/09/09be326f576bbc385eedacc9fd29a7b33fb5262b075dd12521cd2c31e5269e1c",
    )
    .await
    .unwrap();
    write_fixture(M3_SHA256, "09", b"newest-blob-proof");
    assert_eq!(
        asset_url(&library(database).await),
        Some(m3.local_asset_path.as_str())
    );
    record_blob_disposition(
        database,
        M3_SHA256,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "fallback-proof",
        "newest-blob-restricted",
    )
    .await
    .unwrap();
    let blob_fallback = library(database).await;
    assert_eq!(asset_url(&blob_fallback), Some(healthy_url));
    assert!(
        blob_fallback
            .pointer("/inspector/mediaSlots/0/replicaSelection/limitations")
            .and_then(Value::as_array)
            .is_some_and(|values| values
                .iter()
                .any(|value| value == "NEWER_MATERIALIZATION_DISPOSED"))
    );
    assert_asset_response(database, healthy_url, b"fallback-old-bytes").await;
    let denied = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(&m3.local_asset_path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
}

async fn library(database: &Database) -> Value {
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri("/api/local/evidence-library?lane=media_slots")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let list: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let detail_url = list
        .pointer("/items/0/detailUrl")
        .and_then(Value::as_str)
        .unwrap();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(detail_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    detail.get("item").cloned().unwrap()
}

fn asset_url(value: &Value) -> Option<&str> {
    value
        .pointer("/preview/localAssetUrl")
        .and_then(Value::as_str)
}

fn write_fixture(hash: &str, prefix: &str, bytes: &[u8]) {
    let path = fixture_path(hash, prefix);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn fixture_path(hash: &str, prefix: &str) -> std::path::PathBuf {
    local_media_root().join(format!("blobs/{prefix}/{hash}"))
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("migrations apply")
}
