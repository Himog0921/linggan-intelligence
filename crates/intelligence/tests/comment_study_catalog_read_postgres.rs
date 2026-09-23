//! Isolated catalog proofs. These exercise real SQL, not front-end fixture filtering.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::comment_study_catalog::{
    CatalogStudyState, CatalogSummaryQuery, CatalogVoiceRole, CommentCatalogQuery,
    StudyCatalogError, read_catalog_summary, read_comment_catalog, refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::Uuid;

const BASE: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");
const DELTA: &str = include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql");

async fn database(name: &str) -> Database {
    let db = proof_database(name).await;
    sqlx::raw_sql(BASE).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(DELTA).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    db
}

fn query() -> CommentCatalogQuery {
    serde_json::from_value(json!({"domain": ADHD_DOMAIN_REF})).unwrap()
}

async fn add_comment(db: &Database, note: &str, id: &str, text: &str, author: Option<&str>) -> Uuid {
    comment_with_author(db, note, id, text, author, "2026-09-21T08:00:00Z").await
}

async fn cache(db: &Database) {
    // Bounded test helper, never an unbounded production backfill loop.
    for _ in 0..4 {
        let report = refresh_clean_cache(db, query().domain, 200).await.unwrap();
        if report.examined_count == 0 { return; }
    }
    panic!("synthetic test cache did not finish within four bounded passes");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_paginates_more_than_100_comments_without_a_run_and_searches_literally() {
    let db = database("catalog_read_pagination").await;
    detail_with_author(&db, "catalog-page", "SYNTHETIC 分页作品", Some("creator")).await;
    for index in 0..123 {
        add_comment(&db, "catalog-page", &format!("c-{index:03}"),
            &format!("SYNTHETIC 药物观察 {index}；仅为测试"), Some("reader")).await;
    }
    add_comment(&db, "catalog-page", "literal", r"SYNTHETIC 药物 50%_\ 原样匹配", Some("reader")).await;
    cache(&db).await;
    let mut request = query();
    request.limit = Some(31);
    let mut seen = BTreeSet::new();
    let mut original_as_of = None;
    for page_number in 0..8 {
        let response = read_comment_catalog(&db, &request).await.unwrap();
        let as_of = response["page"]["asOf"].as_str().unwrap().to_owned();
        if let Some(expected) = &original_as_of { assert_eq!(&as_of, expected); }
        else { original_as_of = Some(as_of); }
        assert_eq!(response["indexCoverage"]["indexedCount"], 124);
        for row in response["items"].as_array().unwrap() {
            assert!(seen.insert(row["commentKey"]["commentExternalId"].as_str().unwrap().to_owned()));
            assert!(row["latestStudy"].is_null());
            assert!(row["likeCount"].is_null());
        }
        request.cursor = response["page"]["nextCursor"].as_str().map(str::to_owned);
        if request.cursor.is_none() { break; }
        assert!(page_number < 7, "pagination did not terminate");
        request.limit = Some(27); // Page size is deliberately not part of scopeHash.
    }
    assert_eq!(seen.len(), 124);
    request.cursor = None;
    request.q = Some(r"50%_\".to_owned());
    let result = read_comment_catalog(&db, &request).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["commentKey"]["commentExternalId"], "literal");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_run")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(count, 0, "directory reads must never create a Run");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn catalog_distinguishes_visibility_eligibility_and_pending_index() {
    let db = database("catalog_read_voices").await;
    detail_with_author(&db, "catalog-voices", "SYNTHETIC 声音角色", Some("creator")).await;
    add_comment(&db, "catalog-voices", "reader", "SYNTHETIC 用户原声", Some("reader")).await;
    add_comment(&db, "catalog-voices", "creator", "SYNTHETIC 作者说明", Some("creator")).await;
    add_comment(&db, "catalog-voices", "unknown", "SYNTHETIC 身份未知但有语义", None).await;
    add_comment(&db, "catalog-voices", "emoji", "😀😀", Some("reader")).await;
    let before = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(before["items"].as_array().unwrap().is_empty());
    assert_eq!(before["indexCoverage"]["state"], "partial");
    assert_eq!(before["indexCoverage"]["pendingCount"], 3); // reader + unknown + emoji
    cache(&db).await;
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["indexCoverage"]["indexedCount"], 3); // dropped cache still indexed
    let unknown = result["items"].as_array().unwrap().iter()
        .find(|row| row["voiceRole"] == "unknown").unwrap();
    assert_eq!(unknown["studyEligibility"]["eligible"], false);
    let mut creator = query();
    creator.voice_role = CatalogVoiceRole::Creator;
    let result = read_comment_catalog(&db, &creator).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["studyEligibility"]["eligible"], false);
    add_comment(&db, "catalog-voices", "new", "SYNTHETIC 尚未清洗", Some("reader")).await;
    let mut search = query();
    search.q = Some("用户".to_owned());
    let result = read_comment_catalog(&db, &search).await.unwrap();
    assert_eq!(result["indexCoverage"]["state"], "partial");
    assert!(result["indexCoverage"]["pendingCount"].is_null());
    // A text query cannot claim whether the unindexed source would match after cleaning.
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn newer_unknown_or_restricted_source_never_resurrects_old_cached_text() {
    let db = database("catalog_read_restriction").await;
    detail_with_author(&db, "catalog-safe", "SYNTHETIC 来源边界", Some("creator")).await;
    let old = add_comment(&db, "catalog-safe", "changed", "SYNTHETIC 旧可读正文", Some("reader")).await;
    let restricted = add_comment(&db, "catalog-safe", "restricted", "SYNTHETIC 后来受限正文", Some("reader")).await;
    cache(&db).await;
    submit_package_at(&db, "comments", json!({"contentExternalId":"catalog-safe"}), json!({
        "kind":"comment", "sourceObject":{"platform":"xhs","type":"content","externalId":"catalog-safe"},
        "payload":{"commentId":"changed","noteId":"catalog-safe","authorId":"reader"}
    }), "2026-09-22T08:00:00Z").await;
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
        SELECT content_public_ref,comment_external_id,'synthetic restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(restricted).execute(db.pool()).await.unwrap();
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert!(result["items"].as_array().unwrap().is_empty());
    let retained: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_clean_cache WHERE source_ref=$1")
        .bind(old).fetch_one(db.pool()).await.unwrap();
    assert_eq!(retained, 1, "read restrictions must not delete retained cache or raw history");
}

async fn record_target(db: &Database, source: Uuid, state: &str, at: &str) -> Uuid {
    let run = Uuid::new_v4();
    let policy = Uuid::new_v4();
    let work: Uuid = sqlx::query_scalar("SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1")
        .bind(source).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,'comment-study.v1',100,6000)")
        .bind(policy).bind(query().domain).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_run(run_ref,policy_ref,as_of,state,selection_manifest,selection_hash,created_at) \
        VALUES($1,$2,$3::timestamptz,'prepared','{}',repeat('0',64),$3::timestamptz)")
        .bind(run).bind(policy).bind(at).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_work \
        (run_ref,content_public_ref,domain_ref,selection_reason,context_state,context_manifest,context_hash) \
        VALUES($1,$2,$3,'user_selected','missing',$4,repeat('0',64))")
        .bind(run).bind(work).bind(query().domain).bind(json!({"workRef":work,"sources":[]}))
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_target \
        (target_ref,run_ref,content_public_ref,source_ref,research_text,research_sha256,dependency_state,state,input_manifest,input_hash,created_at) \
        VALUES($1,$2,$3,$4,'SYNTHETIC history',repeat('0',64),'self_contained',$5,'{}',repeat('0',64),$6::timestamptz)")
        .bind(Uuid::new_v4()).bind(run).bind(work).bind(source).bind(state).bind(at)
        .execute(db.pool()).await.unwrap();
    run
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn stable_comment_history_separates_last_attempt_from_success_head() {
    let db = database("catalog_read_history").await;
    detail_with_author(&db, "catalog-history", "SYNTHETIC 研究历史", Some("creator")).await;
    let source = add_comment(&db, "catalog-history", "one", "SYNTHETIC 一条原声", Some("reader")).await;
    let success = record_target(&db, source, "succeeded", "2026-09-21T10:00:00Z").await;
    let failed = record_target(&db, source, "failed", "2026-09-21T11:00:00Z").await;
    cache(&db).await;
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(result["items"][0]["latestStudy"]["runRef"], failed.to_string());
    assert_eq!(result["items"][0]["effectiveStudy"]["runRef"], success.to_string());
    assert_eq!(result["items"][0]["effectiveState"], "effective");
    assert_eq!(result["items"][0]["effectiveStudy"]["inputComparison"], "unknown");
    let no_signal = record_target(&db, source, "no_signal", "2026-09-21T12:00:00Z").await;
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(result["items"][0]["effectiveStudy"]["runRef"], no_signal.to_string());
    assert_eq!(result["items"][0]["effectiveStudy"]["state"], "no_signal");
    comment_with_author(&db, "catalog-history", "one", "SYNTHETIC 修改后的正文", Some("reader"),
        "2026-09-22T08:00:00Z").await;
    cache(&db).await;
    let result = read_comment_catalog(&db, &query()).await.unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["items"][0]["effectiveState"], "source_changed");
    assert_eq!(result["items"][0]["effectiveStudy"]["runRef"], no_signal.to_string());
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn cursor_rejects_changed_filters_and_summary_does_not_count_only_the_first_page() {
    let db = database("catalog_read_cursor_scope").await;
    detail_with_author(&db, "catalog-cursor", "SYNTHETIC 游标", Some("creator")).await;
    for index in 0..3 {
        add_comment(&db, "catalog-cursor", &format!("cursor-{index}"), "SYNTHETIC 搜索证据", Some("reader")).await;
    }
    cache(&db).await;
    let mut request = query();
    request.limit = Some(1);
    let first = read_comment_catalog(&db, &request).await.unwrap();
    request.cursor = first["page"]["nextCursor"].as_str().map(str::to_owned);
    request.q = Some("搜索".to_owned());
    assert!(matches!(read_comment_catalog(&db, &request).await,
        Err(StudyCatalogError::CursorScopeMismatch)));
    let summary: CatalogSummaryQuery = serde_json::from_value(json!({"domain":ADHD_DOMAIN_REF})).unwrap();
    let result = read_catalog_summary(&db, &summary).await.unwrap();
    assert_eq!(result["summary"]["displayableCommentCount"], 3);
    assert_eq!(result["summary"]["eligibleCommentCount"], 3);
    request.cursor = None;
    request.q = None;
    request.study_state = CatalogStudyState::NeverStudied;
    assert_eq!(read_comment_catalog(&db, &request).await.unwrap()["items"].as_array().unwrap().len(), 1);
}
