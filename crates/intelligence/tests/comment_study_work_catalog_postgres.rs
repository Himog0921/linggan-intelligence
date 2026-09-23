//! P1 work-directory proofs over isolated synthetic material. No shared DB or model calls.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::proof_database;
use linggan_intelligence::comment_study_catalog::{
    CatalogSummaryQuery, StudyCatalogError, WorkCatalogQuery, read_catalog_summary,
    read_work_catalog, refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::Uuid;

const BASE: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");
const DELTA: &str =
    include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql");

async fn database(name: &str) -> Database {
    let db = proof_database(name).await;
    sqlx::raw_sql(BASE).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(DELTA).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    db
}

fn query() -> WorkCatalogQuery {
    serde_json::from_value(json!({"domain": ADHD_DOMAIN_REF})).unwrap()
}

async fn work_ref(db: &Database, note: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(note)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

fn find_work(response: &Value, work: Uuid) -> &Value {
    let id = work.to_string();
    response["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["workRef"].as_str() == Some(id.as_str()))
        .unwrap()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; never connect to a shared database"]
async fn all_125_works_are_pageable_and_titles_after_the_first_100_are_searchable() {
    let db = database("study_work_paging").await;
    for index in 0..125 {
        detail_with_author(
            &db,
            &format!("work-page-{index:03}"),
            &format!("SYNTHETIC-WORK-{index:03}"),
            Some("creator"),
        )
        .await;
    }
    let mut request = query();
    request.limit = Some(37);
    let mut ids = Vec::new();
    let mut unique = BTreeSet::new();
    let mut first_as_of = None;
    let mut last_work = None;
    for page_index in 0..8 {
        let response = read_work_catalog(&db, &request).await.unwrap();
        assert_eq!(response["totalWorkCount"], 125);
        let as_of = response["page"]["asOf"].as_str().unwrap().to_owned();
        if let Some(expected) = &first_as_of {
            assert_eq!(&as_of, expected);
        } else {
            first_as_of = Some(as_of);
        }
        for item in response["items"].as_array().unwrap() {
            let id = item["workRef"].as_str().unwrap().to_owned();
            assert!(unique.insert(id.clone()), "duplicate work across pages");
            ids.push(id);
            assert_eq!(item["eligibleCommentCount"], 0);
            last_work = Some(item.clone());
        }
        request.cursor = response["page"]["nextCursor"].as_str().map(str::to_owned);
        if request.cursor.is_none() {
            break;
        }
        assert!(page_index < 7, "work pagination must terminate");
        request.limit = Some(29);
    }
    assert_eq!(ids.len(), 125);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    let last_work = last_work.unwrap();
    request.cursor = None;
    request.q = Some(last_work["displayTitle"].as_str().unwrap().to_owned());
    request.limit = Some(1);
    let found = read_work_catalog(&db, &request).await.unwrap();
    assert_eq!(found["totalWorkCount"], 1);
    assert_eq!(found["items"][0]["workRef"], last_work["workRef"]);
    assert_eq!(found["page"]["hasMore"], false);
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_run")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(runs, 0, "reading works cannot create research");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; never connect to a shared database"]
async fn work_counts_share_comment_qualification_and_keep_zero_comment_works() {
    let db = database("study_work_counts").await;
    detail_with_author(&db, "work-counts", "SYNTHETIC 数量", Some("creator")).await;
    detail_with_author(&db, "work-empty", "SYNTHETIC 零评论", Some("creator")).await;
    let work = work_ref(&db, "work-counts").await;
    let empty = work_ref(&db, "work-empty").await;
    let mut restricted = Uuid::nil();
    for (id, text, author) in [
        ("reader1", "SYNTHETIC 用户表达一", Some("reader1")),
        ("reader2", "SYNTHETIC 用户表达二", Some("reader2")),
        ("creator", "SYNTHETIC 作者语境", Some("creator")),
        ("unknown", "SYNTHETIC 未知作者", None),
        ("emoji", "😀😀", Some("reader3")),
    ] {
        let source = comment_with_author(
            &db, "work-counts", id, text, author, "2026-09-22T08:00:00Z",
        ).await;
        if id == "reader1" {
            restricted = source;
        }
    }
    let before = read_work_catalog(&db, &query()).await.unwrap();
    assert_eq!(before["totalWorkCount"], 2);
    assert_eq!(find_work(&before, empty)["indexedCommentCount"], 0);
    assert_eq!(find_work(&before, work)["pendingIndexCount"], 5);
    assert_eq!(before["indexCoverage"]["state"], "partial");
    assert_eq!(refresh_clean_cache(&db, query().domain, 200).await.unwrap().inserted_count, 5);
    let after = read_work_catalog(&db, &query()).await.unwrap();
    let row = find_work(&after, work);
    assert_eq!(row["eligibleCommentCount"], 2);
    assert_eq!(row["indexedCommentCount"], 5); // dropped still has a deterministic cache entry
    assert_eq!(row["pendingIndexCount"], 0);
    let summary: CatalogSummaryQuery = serde_json::from_value(json!({
        "domain": ADHD_DOMAIN_REF, "workRef": work, "voiceRole": "all"
    })).unwrap();
    let comments = read_catalog_summary(&db, &summary).await.unwrap();
    assert_eq!(comments["summary"]["eligibleCommentCount"], row["eligibleCommentCount"]);
    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
         SELECT content_public_ref,comment_external_id,'SYNTHETIC restriction' \
         FROM linggan_material_comment WHERE material_ref=$1",
    ).bind(restricted).execute(db.pool()).await.unwrap();
    let restricted_view = read_work_catalog(&db, &query()).await.unwrap();
    assert_eq!(find_work(&restricted_view, work)["eligibleCommentCount"], 1);
    assert_eq!(restricted_view["totalWorkCount"], 2);
    let cached: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_clean_cache")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(cached, 5, "reading must not erase retained cache");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; never connect to a shared database"]
async fn native_title_and_literal_search_match_the_shared_evidence_display() {
    let db = database("study_work_title_owner").await;
    let title = r"SYNTHETIC 药物50%_\终点";
    detail_with_author(&db, "work-title-literal", title, Some("creator")).await;
    detail_with_author(&db, "work-title-decoy", "SYNTHETIC 药物50ABX终点", Some("creator")).await;
    let work = work_ref(&db, "work-title-literal").await;
    let evidence = linggan_evidence::read_work_resource(&db, work).await.unwrap().unwrap();
    let mut request = query();
    request.q = Some(r"50%_\".to_owned());
    let result = read_work_catalog(&db, &request).await.unwrap();
    assert_eq!(result["totalWorkCount"], 1);
    let row = find_work(&result, work);
    assert_eq!(row["displayTitle"].as_str(), evidence.display.title.as_deref());
    assert_eq!(row["displayTitleSource"], evidence.display.title_source);
    request.q = Some("药".to_owned());
    assert_eq!(read_work_catalog(&db, &request).await.unwrap()["totalWorkCount"], 2);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; never connect to a shared database"]
async fn work_cursor_cannot_be_reused_after_query_scope_changes() {
    let db = database("study_work_cursor").await;
    for index in 0..2 {
        detail_with_author(
            &db, &format!("work-cursor-{index}"), "SYNTHETIC 游标", Some("creator"),
        ).await;
    }
    let mut request = query();
    request.limit = Some(1);
    let page = read_work_catalog(&db, &request).await.unwrap();
    request.cursor = page["page"]["nextCursor"].as_str().map(str::to_owned);
    assert!(request.cursor.is_some());
    request.q = Some("游标".to_owned());
    assert!(matches!(read_work_catalog(&db, &request).await, Err(StudyCatalogError::CursorScopeMismatch)));
    request.cursor = None;
    request.domain = Uuid::nil();
    assert!(matches!(read_work_catalog(&db, &request).await, Err(StudyCatalogError::UnsupportedDomain)));
}
