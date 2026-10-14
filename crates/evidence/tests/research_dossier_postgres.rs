use linggan_evidence::{research_dossier as dossier, *};
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/research_dossier_fixture.rs"]
mod dossier_fixture;
use dossier_fixture::{DOMAIN, fixture, legacy_ocr, work};

async fn counts(db: &linggan_storage_postgres::Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_media_ocr_layout),(SELECT count(*) FROM linggan_media_ocr_line),(SELECT count(*) FROM linggan_runtime_capture_package))").fetch_one(db.pool()).await.unwrap()
}

async fn assert_task_adapters(
    db: &linggan_storage_postgres::Database,
    work: Uuid,
    after: &dossier::ResearchDossier,
) {
    let materials = creator_discovery::load_topic_materials_for_works(db, DOMAIN, &[work])
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&materials.works[0].fragments).unwrap(),
        serde_json::to_value(&after.fragments).unwrap()
    );
    let scope = serde_json::from_value(json!({"domain":DOMAIN})).unwrap();
    let creator = creator_discovery::load(db, &scope).await.unwrap();
    assert!(creator.works[0].search_text.contains("结尾不能丢失🙂"));
    assert!(creator.works[0].search_text.contains("第3行连续"));
    assert_eq!(
        serde_json::to_value(&creator.works[0].fragments).unwrap(),
        serde_json::to_value(dossier::bounded_fragments(&after.fragments, 4000)).unwrap()
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_relayer_preserves_sources_is_idempotent_and_feeds_existing_material_adapters() {
    let db = fixture::proof_database("research_dossier_phase1").await;
    let work = work(&db).await;
    let derivative = legacy_ocr(&db).await;
    let initial = counts(&db).await;
    let before = dossier::read(&db, work).await.unwrap().unwrap();
    assert!(
        before
            .fragments
            .iter()
            .find(|f| f.field == "body")
            .unwrap()
            .text
            .ends_with("结尾不能丢失🙂")
    );
    assert!(!before.fragments.iter().any(|f| f.field == "ocr"));
    assert_eq!(
        before.revision,
        dossier::read(&db, work).await.unwrap().unwrap().revision
    );
    assert!(
        dossier::relayer_work(&db, work, "stale revision")
            .await
            .is_err()
    );
    let receipt = dossier::relayer_work(&db, work, &before.revision)
        .await
        .unwrap();
    assert_eq!(receipt["updated"], 1);
    let after = dossier::read(&db, work).await.unwrap().unwrap();
    assert_ne!(before.revision, after.revision);
    let body = after.fragments.iter().find(|f| f.field == "ocr").unwrap();
    assert_eq!(body.source_ref, derivative);
    assert!(body.text.contains("第3行连续"));
    assert!(
        after
            .fragments
            .iter()
            .any(|f| f.field == "cover_headline" && f.text == "封面大字独立保留")
    );
    assert!(
        after.media[0]["originalAssetUrl"]
            .as_str()
            .unwrap()
            .ends_with(&"a".repeat(64))
    );
    assert!(!body.text.contains("背景"));
    assert_eq!(after.media[0]["ocrLayering"]["state"], "PARTIAL");
    assert_eq!(
        after.media[0]["ocrLayering"]["lines"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        dossier::relayer_work(&db, work, &after.revision)
            .await
            .unwrap()["alreadyCurrent"],
        1
    );
    assert_eq!(
        counts(&db).await,
        initial,
        "no OCR job, layout, raw line or collection package is created"
    );
    let layers: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_media_ocr_layering_result")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(layers, 2, "old and new layer each survive exactly once");
    assert_task_adapters(&db, work, &after).await;
    let mut tx = db.pool().begin().await.unwrap();
    let frozen = dossier::read_in(&mut tx, &[work], &before.as_of)
        .await
        .unwrap();
    assert!(
        !frozen[&work].fragments.iter().any(|f| f.field == "ocr"),
        "new layer never leaks into an earlier cutoff"
    );
    tx.rollback().await.unwrap();
    record_derivative_disposition(
        &db,
        derivative,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "withdraw OCR",
    )
    .await
    .unwrap();
    let restricted = dossier::read(&db, work).await.unwrap().unwrap();
    assert!(!restricted.fragments.iter().any(|f| f.field == "ocr"));
    assert!(
        restricted
            .media
            .iter()
            .all(|m| m["researchText"].is_null() && m["ocrLayering"].is_null())
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn dossier_empty_body_and_missing_media_do_not_turn_unknown_into_zero() {
    let db = fixture::proof_database("research_dossier_missing").await;
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"empty-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"empty-note"},"payload":{"title":"仅有标题"}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='empty-note'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let read = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(read.fragments.len(), 1);
    assert_eq!(read.coverage["body"], "UNKNOWN");
    assert!(read.metadata["likes"]["value"].is_null());
    assert_eq!(
        dossier::relayer_work(&db, work, &read.revision)
            .await
            .unwrap()["scopeCount"],
        0
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn numeric_discovery_count_is_not_a_platform_title_but_numeric_detail_is() {
    let db = fixture::proof_database("dossier_numeric_title").await;
    for (id, title, likes, detail_title, expected) in [
        ("suspected-count", "53", 53, None, None),
        ("numeric-detail", "53", 53, Some("53"), Some("53")),
        ("numeric-card", "1984", 12, None, Some("1984")),
    ] {
        fixture::submit_package(&db,"profile_discovery",json!({"authorExternalId":"synthetic-author"}),
            json!({"kind":"profile_discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":title,"likeCount":likes}})).await;
        fixture::submit_package(&db,"content_detail",json!({"contentExternalId":id}),
            json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":detail_title,"bodyText":"合成正文","likedCount":999}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let dossier = dossier::read(&db, work).await.unwrap().unwrap();
        assert_eq!(
            dossier
                .fragments
                .iter()
                .find(|f| f.field == "title")
                .map(|f| f.text.as_str()),
            expected
        );
        assert_eq!(
            dossier.coverage["title"],
            if expected.is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            }
        );
        assert_eq!(dossier.metadata["likes"]["value"], 999);
        let raw: Option<String> = sqlx::query_scalar(
            "SELECT title FROM linggan_material_discovery_finding WHERE content_public_ref=$1",
        )
        .bind(work)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(raw.as_deref(), Some(title), "source record stays unchanged");
    }
}
