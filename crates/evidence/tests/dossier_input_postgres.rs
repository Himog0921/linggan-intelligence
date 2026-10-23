//! Explicit dossier preparation and immutable reference snapshots; never a second content contract.
use linggan_evidence::{dossier_input as input, research_dossier as dossier};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/research_dossier_fixture.rs"]
mod support;
use support::{DOMAIN, fixture};

async fn seed(db: &Database, id: &str, body: &str) -> Uuid {
    let package=fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"合成快照","bodyText":body,"hashtags":["#A","#B","#A"]}})).await;
    let work = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4) ON CONFLICT DO NOTHING")
        .bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    work
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn snapshots_store_exact_block_references_and_do_not_copy_source_text() {
    let db = fixture::proof_database("dossier_input_identity").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let first = input::read(&db, DOMAIN, work).await.unwrap();
    let canonical = dossier::read_at(&db, work, Some(&first.as_of))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.contract, dossier::VERSION);
    assert_eq!(first.fragments.len(), canonical.fragments.len());
    for (fragment, block) in first.fragments.iter().zip(&canonical.fragments) {
        assert_eq!(fragment.logical_position, block.fragment_id);
        assert_eq!(fragment.text, block.text);
        assert_eq!(fragment.structure, block.structure);
        assert_eq!(fragment.speaker, block.speaker);
        assert_eq!(fragment.issues, block.issues);
    }
    let saved = input::save_snapshot(&db, DOMAIN, work, &first.as_of, &first.dossier_revision)
        .await
        .unwrap();
    let again = input::save_snapshot(&db, DOMAIN, work, &first.as_of, &first.dossier_revision)
        .await
        .unwrap();
    assert!(saved.inserted);
    assert!(!again.inserted);
    assert_eq!(saved.snapshot_ref, again.snapshot_ref);
    let columns:Vec<String>=sqlx::query_scalar("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='linggan_dossier_fragment'").fetch_all(db.pool()).await.unwrap();
    assert!(
        !columns
            .iter()
            .any(|c| matches!(c.as_str(), "text" | "body" | "blocks" | "raw_text"))
    );
    let references:Vec<String>=sqlx::query_scalar("SELECT logical_position FROM linggan_dossier_fragment WHERE snapshot_ref=$1 ORDER BY ordinal").bind(saved.snapshot_ref).fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        references,
        first
            .fragments
            .iter()
            .map(|f| f.logical_position.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        sqlx::query("DELETE FROM linggan_dossier_snapshot WHERE snapshot_ref=$1")
            .bind(saved.snapshot_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    seed(
        &db,
        "dossier-note",
        "改变正文，不能继续保存旧 current revision",
    )
    .await;
    assert!(
        input::save_snapshot(&db, DOMAIN, work, &first.as_of, &first.dossier_revision)
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn explicit_native_rebuild_is_atomic_idempotent_and_not_a_read_side_effect() {
    let db = fixture::proof_database("dossier_input_native").await;
    let body = format!("{}最终原文", "长段落🙂".repeat(26000));
    let work = seed(&db, "native-rebuild", &body).await;
    // Simulate the intentional development cutover in this isolated synthetic schema.
    sqlx::query("TRUNCATE linggan_material_unit CASCADE")
        .execute(db.pool())
        .await
        .unwrap();
    let before = dossier::read(&db, work).await.unwrap().unwrap();
    assert!(before.fragments.iter().all(|f| f.field != "body"));
    assert!(
        before
            .materials
            .iter()
            .find(|m| m["kind"] == "body")
            .unwrap()["currentRevisionRef"]
            .is_null()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "GET cannot backfill");
    let plan = input::plan(
        &db,
        DOMAIN,
        &[work],
        None,
        input::BatchOperation::NativeBody,
    )
    .await
    .unwrap();
    let receipt = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(receipt.updated, 1);
    assert!(receipt.complete());
    let replay = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(replay.already_current, 1);
    let after = dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        after
            .fragments
            .iter()
            .find(|f| f.field == "body")
            .unwrap()
            .text,
        body
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn frozen_batch_conflicts_per_work_and_rejects_tampered_scope() {
    let db = fixture::proof_database("dossier_input_batch").await;
    let first = seed(&db, "first", "第一篇").await;
    let second = seed(&db, "second", "第二篇").await;
    let plan = input::plan(
        &db,
        DOMAIN,
        &[first, second],
        None,
        input::BatchOperation::Snapshot,
    )
    .await
    .unwrap();
    seed(&db, "first", "第一篇已改变").await;
    let receipt = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(receipt.failed, 1);
    assert_eq!(receipt.updated, 1);
    assert_eq!(receipt.results[0].work_ref, first);
    assert_eq!(receipt.results[1].work_ref, second);
    assert_eq!(receipt.results[0].outcome, "failed");
    let mut tampered = serde_json::to_value(&plan).unwrap();
    tampered["entries"][0]["workRef"] = json!(Uuid::new_v4());
    let tampered = serde_json::from_value(tampered).unwrap();
    assert!(input::apply_plan(&db, &tampered).await.is_err());
    assert!(input::read(&db, Uuid::new_v4(), second).await.is_err());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL only"]
async fn explicit_hashtag_projection_preserves_source_order_without_narrative_fallback() {
    let db = fixture::proof_database("dossier_input_tags").await;
    let work = seed(&db, "tags", "正文中 #不会自动当成平台标签").await;
    let original: Value = sqlx::query_scalar(
        "SELECT payload FROM linggan_runtime_capture_package WHERE package_kind='content_detail'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("TRUNCATE linggan_material_content_hashtag,linggan_material_hashtag_projection")
        .execute(db.pool())
        .await
        .unwrap();
    let before = dossier::read(&db, work).await.unwrap().unwrap();
    assert!(!before.materials.iter().any(|m| m["kind"] == "tags"));
    let plan = input::plan(&db, DOMAIN, &[work], None, input::BatchOperation::Hashtags)
        .await
        .unwrap();
    assert_eq!(input::apply_plan(&db, &plan).await.unwrap().updated, 1);
    assert_eq!(
        input::apply_plan(&db, &plan).await.unwrap().already_current,
        1
    );
    let after = dossier::read(&db, work).await.unwrap().unwrap();
    let tags = after
        .materials
        .iter()
        .find(|m| m["kind"] == "tags")
        .unwrap();
    assert_eq!(tags["blocks"][0]["items"], json!(["#A", "#B", "#A"]));
    let unchanged: Value = sqlx::query_scalar(
        "SELECT payload FROM linggan_runtime_capture_package WHERE package_kind='content_detail'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(original, unchanged);
    seed(&db, "tags", "新的详情来源不属于原计划").await;
    let changed = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(changed.failed, 1);
    assert_eq!(changed.already_current, 0);
    assert_eq!(changed.results[0].result["reason"], "material_changed");
}
