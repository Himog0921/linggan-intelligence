//! Synthetic P1 inspection proofs. Never connect this target to a shared/real-data database.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::comment_study_catalog::{
    CommentDetailQuery, CommentHistoryQuery, StudyCatalogError,
    read_comment_detail, read_comment_history, read_comment_versions, refresh_clean_cache,
};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::Uuid;

const BASE: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");
const DELTA: &str = include_str!("../../../database/migrations/0103_comment_study_productization_schema.sql");

fn domain() -> Uuid { Uuid::parse_str(ADHD_DOMAIN_REF).unwrap() }

async fn database(name: &str) -> Database {
    let db = proof_database(name).await;
    sqlx::raw_sql(BASE).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(DELTA).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    db
}

async fn key(db: &Database, source: Uuid, id: &str) -> CommentDetailQuery {
    let work_ref = sqlx::query_scalar("SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1")
        .bind(source).fetch_one(db.pool()).await.unwrap();
    CommentDetailQuery { domain: domain(), work_ref, comment_external_id: id.into() }
}

fn history_query(key: &CommentDetailQuery, limit: i64) -> CommentHistoryQuery {
    CommentHistoryQuery { domain: key.domain, work_ref: key.work_ref,
        comment_external_id: key.comment_external_id.clone(), cursor: None, limit: Some(limit) }
}

async fn cache(db: &Database) {
    for _ in 0..4 {
        if refresh_clean_cache(db, domain(), 200).await.unwrap().examined_count == 0 { return; }
    }
    panic!("synthetic fixture did not finish within four bounded cache passes");
}

async fn restrict(db: &Database, key: &CommentDetailQuery) {
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
        VALUES($1,$2,'SYNTHETIC restriction')")
        .bind(key.work_ref).bind(&key.comment_external_id).execute(db.pool()).await.unwrap();
}

async fn unknown(db: &Database, note: &str, id: &str) {
    submit_package_at(db, "comments", json!({"contentExternalId":note}), json!({
        "kind":"comment", "sourceObject":{"platform":"xhs","type":"content","externalId":note},
        "payload":{"noteId":note,"commentId":id,"authorId":"reader"}
    }), "2026-09-22T20:00:00Z").await;
}

async fn retained_counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array( \
        (SELECT count(*) FROM linggan_material_comment), \
        (SELECT count(*) FROM linggan_comment_study_clean_cache), \
        (SELECT count(*) FROM linggan_comment_study_run), \
        (SELECT count(*) FROM linggan_comment_study_target), \
        (SELECT count(*) FROM linggan_model_invocation))")
        .fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn detail_keeps_current_comment_parent_and_cleaning_separate_without_writes() {
    let db = database("inspection_parent_readonly").await;
    detail_with_author(&db, "inspect-parent", "SYNTHETIC 父语境", Some("creator")).await;
    comment_with_author(&db, "inspect-parent", "parent", "SYNTHETIC 作者解释，不是用户痛点", Some("creator"),
        "2026-09-21T08:00:00Z").await;
    let child = reply_with_author(&db, "inspect-parent", "child", "parent", "我也是", Some("reader"),
        "2026-09-21T09:00:00Z").await;
    let request = key(&db, child, "child").await;
    let pending = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(pending["source"]["displayState"], "index_pending");
    assert!(pending["comment"].is_null());
    cache(&db).await;
    let before = retained_counts(&db).await;
    let detail = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(detail["comment"]["commentText"], "我也是");
    assert_eq!(detail["parentContext"]["commentText"], "SYNTHETIC 作者解释，不是用户痛点");
    assert_eq!(detail["parentContext"]["contextOnly"], true);
    assert_eq!(detail["comment"]["voiceRole"], "reader");
    assert_eq!(detail["studyHistory"]["totalCount"], 0);
    assert_eq!(detail["materialVersions"]["totalCount"], 1);
    assert_eq!(retained_counts(&db).await, before, "GET must not materialize cache or create study work");
    assert_eq!(detail["studyHistory"]["page"]["asOf"], detail["asOf"]);
    assert_eq!(detail["materialVersions"]["page"]["asOf"], detail["asOf"]);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn parent_latest_unknown_and_restriction_never_revive_the_old_known_body() {
    let db = database("inspection_parent_boundaries").await;
    detail_with_author(&db, "inspect-latest", "SYNTHETIC 最新父评论", Some("creator")).await;
    let parent = comment_with_author(&db, "inspect-latest", "parent", "SYNTHETIC PARENT_SECRET_OLD", Some("creator"),
        "2026-09-21T08:00:00Z").await;
    let child = reply_with_author(&db, "inspect-latest", "child", "parent", "我也是", Some("reader"),
        "2026-09-21T09:00:00Z").await;
    cache(&db).await;
    let request = key(&db, child, "child").await;
    unknown(&db, "inspect-latest", "parent").await;
    let detail = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(detail["parentContext"]["sourceState"], "unknown");
    assert!(detail["parentContext"]["commentText"].is_null());
    assert!(!detail.to_string().contains("PARENT_SECRET_OLD"));
    restrict(&db, &key(&db, parent, "parent").await).await;
    let detail = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(detail["parentContext"]["sourceState"], "restricted");
    assert!(detail["parentContext"]["researchText"].is_null());
    assert!(!detail.to_string().contains("PARENT_SECRET_OLD"));
    assert_eq!(detail["comment"]["commentText"], "我也是");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn restricted_unknown_and_dropped_current_sources_do_not_leak_history_text() {
    let db = database("inspection_source_boundaries").await;
    detail_with_author(&db, "inspect-state", "SYNTHETIC 来源状态", Some("creator")).await;
    let source = comment_with_author(&db, "inspect-state", "one", "SYNTHETIC COMMENT_SECRET_OLD", Some("reader"),
        "2026-09-21T08:00:00Z").await;
    let emoji = comment_with_author(&db, "inspect-state", "emoji", "😀😀", Some("reader"),
        "2026-09-21T08:00:00Z").await;
    let unknown_author = comment_with_author(&db, "inspect-state", "unknown-author", "SYNTHETIC 身份未知有效原声", None,
        "2026-09-21T08:00:00Z").await;
    cache(&db).await;
    let request = key(&db, source, "one").await;
    unknown(&db, "inspect-state", "one").await;
    let result = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(result["source"]["sourceState"], "unknown");
    assert!(result["comment"].is_null());
    assert_eq!(result["materialVersions"]["totalCount"], 2);
    assert!(!result.to_string().contains("COMMENT_SECRET_OLD"));
    restrict(&db, &request).await;
    let result = read_comment_detail(&db, &request).await.unwrap();
    assert_eq!(result["source"]["sourceState"], "restricted");
    assert!(result["comment"].is_null());
    for row in result["materialVersions"]["items"].as_array().unwrap() {
        assert_eq!(row["sourceState"], "restricted");
        assert!(row.get("commentText").is_none());
    }
    let dropped = read_comment_detail(&db, &key(&db, emoji, "emoji").await).await.unwrap();
    assert_eq!(dropped["source"]["displayState"], "not_displayable");
    assert!(dropped["comment"].is_null());
    let readable = read_comment_detail(&db, &key(&db, unknown_author, "unknown-author").await).await.unwrap();
    assert_eq!(readable["comment"]["voiceRole"], "unknown");
    assert_eq!(readable["comment"]["studyEligibility"]["eligible"], false);
}

async fn record_attempt(db: &Database, key: &CommentDetailQuery, source: Uuid, ordinal: u128, state: &str) -> Uuid {
    let policy = Uuid::new_v4();
    let run = Uuid::new_v4();
    let target = Uuid::from_u128(1000 + ordinal);
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,contract,comment_budget,context_character_budget) \
        VALUES($1,$2,'comment-study.v1',100,6000)")
        .bind(policy).bind(domain()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_run(run_ref,policy_ref,as_of,state,selection_manifest,selection_hash) \
        VALUES($1,$2,scope_001_now(),'prepared','{}',repeat('0',64))")
        .bind(run).bind(policy).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_work(run_ref,content_public_ref,domain_ref,selection_reason,context_state,context_manifest,context_hash) \
        VALUES($1,$2,$3,'user_selected','missing',$4,repeat('0',64))")
        .bind(run).bind(key.work_ref).bind(domain()).bind(json!({"workRef":key.work_ref,"sources":[]}))
        .execute(db.pool()).await.unwrap();
    // Equal timestamps deliberately exercise the UUID tie-breaker across every page.
    sqlx::query("INSERT INTO linggan_comment_study_target(target_ref,run_ref,content_public_ref,source_ref, \
        research_text,research_sha256,dependency_state,state,input_manifest,input_hash,created_at) \
        VALUES($1,$2,$3,$4,'SYNTHETIC TARGET_SECRET',repeat('0',64),'self_contained',$5,'{}',repeat('0',64),'2026-09-22T10:00:00Z')")
        .bind(target).bind(run).bind(key.work_ref).bind(source).bind(state).execute(db.pool()).await.unwrap();
    target
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn history_paginates_all_attempts_of_one_stable_comment_and_keeps_success_head() {
    let db = database("inspection_complete_history").await;
    detail_with_author(&db, "inspect-history", "SYNTHETIC 全部研究历史", Some("creator")).await;
    let old = comment_with_author(&db, "inspect-history", "same", "SYNTHETIC COMMENT_HISTORY_SECRET", Some("reader"),
        "2026-09-21T08:00:00Z").await;
    let new = comment_with_author(&db, "inspect-history", "same", "SYNTHETIC COMMENT_HISTORY_SECRET", Some("reader"),
        "2026-09-22T08:00:00Z").await;
    let key = key(&db, new, "same").await;
    for index in 0..124 {
        let state = if index == 122 { "no_signal" } else if index == 123 { "failed" } else { "succeeded" };
        record_attempt(&db, &key, if index % 2 == 0 { old } else { new }, index, state).await;
    }
    cache(&db).await;
    let before = retained_counts(&db).await;
    let initial = read_comment_detail(&db, &key).await.unwrap();
    assert_eq!(initial["studyHistory"]["items"].as_array().unwrap().len(), 50);
    assert_eq!(initial["studyHistory"]["totalCount"], 124);
    assert_eq!(initial["comment"]["latestStudy"]["state"], "failed");
    assert_eq!(initial["comment"]["effectiveStudy"]["state"], "no_signal");
    let mut query = history_query(&key, 31);
    let mut seen = BTreeSet::new();
    let mut first_as_of = None;
    for number in 0..8 {
        let result = read_comment_history(&db, &query).await.unwrap();
        assert_eq!(result["totalCount"], 124);
        if let Some(as_of) = &first_as_of { assert_eq!(&result["page"]["asOf"], as_of); }
        else { first_as_of = Some(result["page"]["asOf"].clone()); }
        assert!(!result.to_string().contains("TARGET_SECRET"));
        assert!(!result.to_string().contains("COMMENT_HISTORY_SECRET"));
        for row in result["items"].as_array().unwrap() {
            assert!(seen.insert(row["targetRef"].as_str().unwrap().to_owned()));
            assert_eq!(row["method"]["recordingState"], "legacy_unrecorded");
            assert!(row.get("problemFrame").is_none());
        }
        query.cursor = result["page"]["nextCursor"].as_str().map(str::to_owned);
        if query.cursor.is_none() { break; }
        assert!(number < 7, "history pagination did not terminate");
        query.limit = Some(27);
    }
    assert_eq!(seen.len(), 124);
    assert_eq!(retained_counts(&db).await, before);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn versions_pagination_is_not_comment_count_and_cursors_cannot_cross_resources() {
    let db = database("inspection_versions_cursors").await;
    detail_with_author(&db, "inspect-version", "SYNTHETIC 材料版本", Some("creator")).await;
    let mut last = Uuid::nil();
    for index in 0..105 {
        last = comment_with_author(&db, "inspect-version", "same", "SYNTHETIC VERSION_SECRET", Some("reader"),
            &format!("2026-09-21T{:02}:{:02}:00Z", 8 + index / 60, index % 60)).await;
    }
    let key = key(&db, last, "same").await;
    let mut query = history_query(&key, 24);
    let mut seen = BTreeSet::new();
    let mut current_count = 0;
    let first = read_comment_versions(&db, &query).await.unwrap();
    query.cursor = first["page"]["nextCursor"].as_str().map(str::to_owned);
    assert!(matches!(read_comment_history(&db, &query).await, Err(StudyCatalogError::CursorScopeMismatch)));
    let mut other = query.clone();
    other.comment_external_id = "other".into();
    assert!(matches!(read_comment_versions(&db, &other).await, Err(StudyCatalogError::CursorScopeMismatch)));
    query.cursor = None;
    for number in 0..8 {
        let result = read_comment_versions(&db, &query).await.unwrap();
        assert_eq!(result["totalCount"], 105);
        assert!(!result.to_string().contains("VERSION_SECRET"));
        for row in result["items"].as_array().unwrap() {
            assert!(seen.insert(row["sourceRef"].as_str().unwrap().to_owned()));
            if row["isCurrent"] == true { current_count += 1; }
        }
        query.cursor = result["page"]["nextCursor"].as_str().map(str::to_owned);
        if query.cursor.is_none() { break; }
        assert!(number < 7, "version pagination did not terminate");
    }
    assert_eq!(seen.len(), 105);
    assert_eq!(current_count, 1);
    let mut absent = key.clone();
    absent.work_ref = Uuid::new_v4();
    assert!(matches!(read_comment_detail(&db, &absent).await, Err(StudyCatalogError::ResourceNotFound)));
    query.cursor = None;
    query.limit = Some(101);
    assert!(matches!(read_comment_history(&db, &query).await, Err(StudyCatalogError::InvalidLimit)));
}
