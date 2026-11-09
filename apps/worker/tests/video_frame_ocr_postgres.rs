//! Video frame OCR reading publication through the current lease and dossier readers.
#[path = "../../../crates/evidence/tests/support/material_fixture.rs"]
#[allow(dead_code)]

mod fixture;
#[path = "../src/bin/media_worker_support/mod.rs"]
#[allow(dead_code)]
mod support;

use linggan_evidence::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

async fn admit_video(db: &Database, work: &str, ordinal: u32, bytes: &[u8]) -> Uuid {
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_content WHERE content_external_id=$1)",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    if !present {
        fixture::submit_package(db,"content_detail",json!({"contentExternalId":work}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":work},"payload":{"title":"Synthetic replay metadata","bodyText":"Native body must remain independent."}})).await;
    }
    let observation = Uuid::new_v4();
    let slot = format!("xhs:{work}:video:{ordinal}");
    let uri = format!("https://media.example/{work}/{ordinal}.jpg");
    let record = json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"video","ordinal":ordinal},"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-21T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":work}});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["media_slots"],
        json!({"contentExternalId":work}),
        "media_slots",
        "xhs",
        json!({"target":{"contentExternalId":work},"layers":[fixture::coverage_layer("media_slots",1)]}),
        vec![record],
    )
    .await;
    let hash = support::sha256_hex(bytes);
    let key = format!("blobs/{hash}");
    let path = support::prepare_output(&key).unwrap();
    std::fs::write(path, bytes).unwrap();
    admit_media_blob(
        db,
        observation,
        &hash,
        "video/mp4",
        bytes.len() as i64,
        &key,
    )
    .await
    .unwrap();
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL runner only"]
async fn latest_frame_completion_and_empty_outcome_publish_current_readings() {
    use linggan_evidence::material_reading::producers::ReadingContent;
    let db = fixture::proof_database("frame_current_completion").await;
    for (name, no_text) in [("frame-text", false), ("frame-empty", true)] {
        let work = admit_video(&db, name, 1, name.as_bytes()).await;
        let job: Uuid = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE processor_kind='video_frame_ocr' AND blob_sha256=$1 AND processor_version='local-v5'").bind(support::sha256_hex(name.as_bytes())).fetch_one(db.pool()).await.unwrap();
        ensure_media_processing_work(&db).await.unwrap();
        let worker = Uuid::new_v4();
        let MediaProcessingClaimOutcome::Claimed(claim) =
            claim_media_processing_work(&db, worker, &["video_frame_ocr".into()])
                .await
                .unwrap()
        else {
            panic!("current frame job must be claimable")
        };
        assert_eq!(claim.job_ref, job);
        if no_text {
            complete_media_processing_without_output(&db, &claim, worker, "no_frame_text_observed")
                .await
                .unwrap();
        } else {
            let text = "synthetic literal frame text";
            complete_media_processing_text(&db, &claim, worker, "frame_ocr_text", &support::sha256_hex(text.as_bytes()), text.len() as i64, "derivatives/frame-ocr/synthetic.txt", text, text, None, ReadingContent { blocks: vec![json!({"ref":"synthetic","kind":"paragraph","text":text,"speaker":"unconfirmed"})], issues: vec![], source_state:"present".into(), source_location:json!({"layerVersion":"rules-v3"}) }).await.unwrap();
        }
        let dossier = research_dossier::read(&db, work).await.unwrap().unwrap();
        let frame = dossier
            .materials
            .iter()
            .find(|m| m["kind"] == "video_frame")
            .unwrap();
        assert!(frame["currentRevisionRef"].is_string());
        assert_eq!(frame["reading"]["generatedBy"], "frame_ocr");
        assert_eq!(frame["blocks"].as_array().unwrap().is_empty(), no_text);
        assert_eq!(frame["processing"]["state"], "completed");
        if no_text {
            assert_eq!(frame["processing"]["reason"], "no_frame_text_observed");
        }
    }
}
