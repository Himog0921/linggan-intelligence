//! Isolated P1 schema/cache proofs. No real data, provider, scheduler or shared migration.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::comment_study_catalog::refresh_clean_cache;
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::{Value, json};
use uuid::Uuid;

async fn base_database(name: &str) -> Database {
    proof_database(name).await
}
async fn migrate(_database: &Database) {}
fn domain() -> Uuid {
    Uuid::parse_str(ADHD_DOMAIN_REF).unwrap()
}

#[tokio::test]
#[ignore = "requires the random isolated productization proof database"]
async fn cache_is_bounded_idempotent_and_does_not_create_research() {
    let database = base_database("productization_clean_cache").await;
    migrate(&database).await;
    detail_with_author(&database, "cache-note", "SYNTHETIC cache", Some("creator")).await;
    let reader = comment_with_author(
        &database,
        "cache-note",
        "reader",
        "SYNTHETIC / NOT EVIDENCE · 写作业很困难😀",
        Some("reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "cache-note",
        "emoji",
        "😀😀",
        Some("reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "cache-note",
        "creator",
        "SYNTHETIC / NOT EVIDENCE · 作者提供语境",
        Some("creator"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let first = refresh_clean_cache(&database, domain(), 1).await.unwrap();
    assert_eq!(first.examined_count, 1);
    assert_eq!(first.inserted_count, 1);
    assert_eq!(
        refresh_clean_cache(&database, domain(), 200)
            .await
            .unwrap()
            .inserted_count,
        2
    );
    assert_eq!(
        refresh_clean_cache(&database, domain(), 200)
            .await
            .unwrap()
            .examined_count,
        0
    );
    let projection: Value = sqlx::query_scalar("SELECT jsonb_build_object( \
        'rawHashMatches',cache.raw_sha256=encode(sha256(convert_to(source.body_text,'UTF8')),'hex'), \
        'state',cache.clean_state) FROM linggan_comment_clean_cache cache \
        JOIN linggan_material_comment source ON source.material_ref=cache.source_ref WHERE cache.source_ref=$1")
        .bind(reader).fetch_one(database.pool()).await.unwrap();
    assert_eq!(projection["rawHashMatches"], true);
    assert_eq!(projection["state"], "direct");
    let dropped: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_clean_cache WHERE clean_state='dropped'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(dropped, 1);
    let writes: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM linggan_comment_research_request) \
        + (SELECT count(*) FROM linggan_model_invocation) + (SELECT count(*) FROM linggan_ai_command)")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(writes, 0);
    assert!(refresh_clean_cache(&database, domain(), 201).await.is_err());
    assert!(refresh_clean_cache(&database, domain(), 0).await.is_err());
}

#[tokio::test]
#[ignore = "requires the random isolated productization proof database"]
async fn latest_unknown_and_restricted_sources_never_revert_to_older_readable_text() {
    let database = base_database("productization_cache_latest_gate").await;
    migrate(&database).await;
    detail_with_author(&database, "gate-note", "SYNTHETIC gate", Some("creator")).await;
    comment_with_author(
        &database,
        "gate-note",
        "unknown-latest",
        "SYNTHETIC old body",
        Some("reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    submit_package_at(&database, "comments", json!({"contentExternalId":"gate-note"}), json!({
        "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"gate-note"},
        "payload":{"commentId":"unknown-latest","noteId":"gate-note","authorId":"reader"}
    }), "2026-09-16T09:00:00Z").await;
    let restricted = comment_with_author(
        &database,
        "gate-note",
        "restricted",
        "SYNTHETIC restricted text",
        Some("reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
        SELECT content_public_ref,comment_external_id,'SYNTHETIC restriction' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(restricted).execute(database.pool()).await.unwrap();
    assert_eq!(
        refresh_clean_cache(&database, domain(), 200)
            .await
            .unwrap()
            .examined_count,
        0
    );
}

#[tokio::test]
#[ignore = "requires the random isolated productization proof database"]
async fn concurrent_refresh_does_not_duplicate_cache_entries() {
    let database = base_database("productization_cache_concurrency").await;
    migrate(&database).await;
    detail_with_author(
        &database,
        "concurrent-note",
        "SYNTHETIC concurrency",
        Some("creator"),
    )
    .await;
    comment_with_author(
        &database,
        "concurrent-note",
        "one",
        "SYNTHETIC one comment",
        Some("reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let (left, right) = tokio::join!(
        refresh_clean_cache(&database, domain(), 10),
        refresh_clean_cache(&database, domain(), 10),
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.inserted_count + right.inserted_count, 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_clean_cache")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}
