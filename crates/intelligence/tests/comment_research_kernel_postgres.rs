#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_intelligence::comment_research_kernel::{
    DERIVATION_VERSION, RunItemFailureClass, SaveResearchPolicy, claim_next_run_item,
    derive_current_sources, record_run_item_failure, save_active_policy, start_run,
};
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn derivation_ref(database: &Database, source_ref: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT derivation_ref FROM linggan_comment_research_derivation WHERE source_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn derivation_preserves_raw_text_and_excludes_confirmed_content_author_replies() {
    let database = fixture::proof_database("comment_research_kernel_derivation").await;
    detail_with_author(
        &database,
        "kernel-note",
        "SYNTHETIC kernel note",
        Some("creator-1"),
    )
    .await;
    let ordinary = comment_with_author(
        &database,
        "kernel-note",
        "ordinary",
        "作者 推荐的资料我看了，还是不懂怎么开始",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let author_reply = comment_with_author(
        &database,
        "kernel-note",
        "creator-reply",
        "作者 别再瞎干预啦",
        Some("creator-1"),
        "2026-09-01T08:01:00Z",
    )
    .await;
    let unknown = comment_with_author(
        &database,
        "kernel-note",
        "unknown",
        "我家也是这种情况",
        None,
        "2026-09-01T08:02:00Z",
    )
    .await;

    assert_eq!(derive_current_sources(&database, 100).await.unwrap(), 3);
    assert_eq!(derive_current_sources(&database, 100).await.unwrap(), 0);

    let rows = sqlx::query(
        "SELECT source_ref,research_text,author_role,eligibility,normalization_reasons \
         FROM linggan_comment_research_derivation WHERE derivation_version=$1 ORDER BY source_ref",
    )
    .bind(DERIVATION_VERSION)
    .fetch_all(database.pool())
    .await
    .unwrap();
    let by_source = |source_ref| {
        rows.iter()
            .find(|row| row.get::<Uuid, _>("source_ref") == source_ref)
            .unwrap()
    };
    let ordinary_row = by_source(ordinary);
    assert_eq!(
        ordinary_row.get::<String, _>("research_text"),
        "作者 推荐的资料我看了,还是不懂怎么开始"
    );
    assert_eq!(
        ordinary_row.get::<String, _>("author_role"),
        "ordinary_user"
    );
    assert_eq!(ordinary_row.get::<String, _>("eligibility"), "eligible");

    let author_row = by_source(author_reply);
    assert_eq!(author_row.get::<String, _>("research_text"), "别再瞎干预啦");
    assert_eq!(
        author_row.get::<String, _>("author_role"),
        "content_author_reply"
    );
    assert_eq!(
        author_row.get::<String, _>("eligibility"),
        "excluded_author_reply"
    );
    assert!(
        author_row
            .get::<serde_json::Value, _>("normalization_reasons")
            .as_array()
            .unwrap()
            .contains(&json!("content_author_badge_removed"))
    );

    let unknown_row = by_source(unknown);
    assert_eq!(
        unknown_row.get::<String, _>("author_role"),
        "author_identity_unknown"
    );
    assert_eq!(
        unknown_row.get::<String, _>("eligibility"),
        "author_identity_unknown"
    );

    let raw: String =
        sqlx::query_scalar("SELECT body_text FROM linggan_material_comment WHERE material_ref=$1")
            .bind(author_reply)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(raw, "作者 别再瞎干预啦");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_run_cannot_admit_an_author_reply_or_unknown_identity() {
    let database = fixture::proof_database("comment_research_kernel_eligibility").await;
    detail_with_author(
        &database,
        "eligible-note",
        "SYNTHETIC eligible note",
        Some("creator-1"),
    )
    .await;
    let ordinary = comment_with_author(
        &database,
        "eligible-note",
        "ordinary",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let author_reply = comment_with_author(
        &database,
        "eligible-note",
        "creator-reply",
        "作者 我会补充方法",
        Some("creator-1"),
        "2026-09-01T08:01:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 100).await.unwrap(), 2);

    let policy_ref = Uuid::new_v4();
    let run_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_research_policy_revision( \
             policy_revision_ref,contract_version,derivation_version,extraction_rule_hash, \
             membership_policy_hash,source_limit,token_limit \
         ) VALUES($1,'comment-research.semantic.v1',$2,$3,$3,100,10000)",
    )
    .bind(policy_ref)
    .bind(DERIVATION_VERSION)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_run( \
             run_ref,policy_revision_ref,state,as_of,scope,manifest_hash \
         ) VALUES($1,$2,'queued',scope_001_now(),'{}'::jsonb,$3)",
    )
    .bind(run_ref)
    .bind(policy_ref)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();

    let ordinary_derivation = derivation_ref(&database, ordinary).await;
    let author_derivation = derivation_ref(&database, author_reply).await;
    sqlx::query(
        "INSERT INTO linggan_comment_research_run_item( \
             run_ref,derivation_ref,input_hash,context_hash \
         ) VALUES($1,$2,$3,$3)",
    )
    .bind(run_ref)
    .bind(ordinary_derivation)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();
    let blocked = sqlx::query(
        "INSERT INTO linggan_comment_research_run_item( \
             run_ref,derivation_ref,input_hash,context_hash \
         ) VALUES($1,$2,$3,$3)",
    )
    .bind(run_ref)
    .bind(author_derivation)
    .bind(HASH)
    .execute(database.pool())
    .await;
    assert!(
        blocked.is_err(),
        "author replies must never enter a user research Run"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn saved_policy_is_the_only_authorization_needed_to_queue_a_research_run() {
    let database = fixture::proof_database("comment_research_kernel_direct_run").await;
    detail_with_author(
        &database,
        "direct-note",
        "SYNTHETIC direct-run note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "direct-note",
        "reader",
        "孩子写作业时总是拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "direct-note",
        "creator-reply",
        "作者 我会补充方法",
        Some("creator-1"),
        "2026-09-01T08:01:00Z",
    )
    .await;

    let policy = save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 100,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let receipt = start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();

    assert_eq!(receipt.policy_revision_ref, policy.policy_revision_ref);
    assert_eq!(receipt.selected_sources, 1);
    assert_eq!(receipt.external_calls_started, 0);
    let item_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_run_item WHERE run_ref=$1",
    )
    .bind(receipt.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(item_count, 1);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0, "manifest freezing must not invoke a model");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn incompatible_item_does_not_block_the_next_healthy_v1_item() {
    let database = fixture::proof_database("comment_research_kernel_poison_isolation").await;
    detail_with_author(
        &database,
        "poison-note",
        "SYNTHETIC poison isolation note",
        Some("creator-1"),
    )
    .await;
    for (id, body) in [
        ("first", "孩子写作业时总是拖延，有什么办法吗"),
        ("second", "一写应用题就不知道题目要他做什么"),
    ] {
        comment_with_author(
            &database,
            "poison-note",
            id,
            body,
            Some("reader-1"),
            "2026-09-01T08:00:00Z",
        )
        .await;
    }
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 100,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();

    let poisoned = claim_next_run_item(&database).await.unwrap().unwrap();
    record_run_item_failure(
        &database,
        &poisoned,
        RunItemFailureClass::Incompatible,
        "legacy_contract_incompatible",
    )
    .await
    .unwrap();
    let healthy = claim_next_run_item(&database).await.unwrap().unwrap();
    assert_ne!(healthy.derivation_ref, poisoned.derivation_ref);
    assert_eq!(healthy.attempt, 1);
}
