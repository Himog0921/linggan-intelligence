#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_intelligence::comment_research_atoms::{
    AtomBasis, AtomKind, CommentResearchAtomError, SemanticAtomProposal, SemanticExtractionOutput,
    accept_semantic_output,
};
use linggan_intelligence::comment_research_embeddings::{
    AtomEmbeddingResult, CommentResearchEmbeddingError, EmbeddingSpaceReceipt,
    ProblemDefinitionEmbeddingResult, accept_atom_embedding, accept_problem_definition_embedding,
    activate_configured_embedding_space, claim_next_embedding_work, queue_atom_embedding,
    queue_problem_definition_embedding, recall_problem_candidates, record_embedding_failure,
};
use linggan_intelligence::comment_research_kernel::{
    CommentResearchKernelError, DERIVATION_VERSION, ResearchRunReceipt, RunItemFailureClass,
    SaveResearchPolicy, claim_next_run_item, derive_current_sources,
    fail_active_runs_without_embedding_config, record_run_item_failure, recover_expired_run_items,
    refresh_run_completion_for_atom, save_active_policy, start_run,
};
use linggan_intelligence::comment_research_problems::{
    CommentResearchProblemError, ExistingProblemAdmission, NewProblemAdmission,
    ProblemDefinitionProposal, ProblemMembershipBasis, admit_existing_problem, admit_new_problem,
};
use linggan_intelligence::comment_research_read_v1::{
    CommentResearchV1ReadQuery, read_changes, read_overview, read_problems, read_runs, read_voices,
};
use linggan_intelligence::comment_research_results::publish_result_revision;
use linggan_intelligence::comment_research_worker::recover_problem_resolution_leases;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn terminal_cutover_removes_every_retired_research_relation_and_keeps_v1_inputs() {
    let database = fixture::proof_database("comment_research_terminal_cutover").await;
    for relation in [
        "linggan_ci_source_revision",
        "linggan_ci_source_research",
        "linggan_ci_problem",
        "linggan_ci_semantic_atom",
        "linggan_comment_daily_batch",
        "linggan_comment_replay_run",
        "linggan_comment_analysis_work",
    ] {
        let present: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(relation)
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert!(
            present.is_none(),
            "retired relation survived terminal cutover: {relation}"
        );
    }
    for relation in [
        "linggan_material_comment",
        "linggan_material_content_author",
        "linggan_comment_research_restriction",
        "linggan_embedding_settings",
        "linggan_model_invocation",
        "linggan_comment_research_derivation",
    ] {
        let present: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
            .bind(relation)
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert!(
            present.is_some(),
            "required V1 input disappeared: {relation}"
        );
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn terminal_cutover_clears_preliminary_v1_results_without_touching_raw_evidence() {
    let database = fixture::proof_database("comment_research_terminal_reset").await;
    detail_with_author(
        &database,
        "terminal-reset-note",
        "SYNTHETIC terminal reset note",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "terminal-reset-note",
        "terminal-reset-comment",
        "我想知道怎样让孩子愿意写作业",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    start_ready_run(&database).await.unwrap();

    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0069_comment_research_v1_cutover.sql"
    ))
    .execute(database.pool())
    .await
    .unwrap();

    let counts = sqlx::query(
        "SELECT \
           (SELECT count(*) FROM linggan_comment_research_policy_revision) AS policy_revisions, \
           (SELECT count(*) FROM linggan_comment_research_derivation) AS derivations, \
           (SELECT count(*) FROM linggan_comment_research_run) AS runs, \
           (SELECT count(*) FROM linggan_comment_research_run_item) AS run_items, \
           (SELECT count(*) FROM linggan_comment_research_atom) AS atoms, \
           (SELECT count(*) FROM linggan_comment_research_embedding_space) AS spaces, \
           (SELECT count(*) FROM linggan_comment_research_problem) AS problems, \
           (SELECT count(*) FROM linggan_comment_research_result_revision) AS result_revisions",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    for column in [
        "policy_revisions",
        "derivations",
        "runs",
        "run_items",
        "atoms",
        "spaces",
        "problems",
        "result_revisions",
    ] {
        assert_eq!(
            counts.get::<i64, _>(column),
            0,
            "preliminary V1 data survived reset in {column}"
        );
    }
    let active_policy_is_empty: bool = sqlx::query_scalar(
        "SELECT policy_revision_ref IS NULL FROM linggan_comment_research_policy_active WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(active_policy_is_empty);
    let raw_body: String =
        sqlx::query_scalar("SELECT body_text FROM linggan_material_comment WHERE material_ref=$1")
            .bind(source_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(raw_body, "我想知道怎样让孩子愿意写作业");
}

async fn derivation_ref(database: &Database, source_ref: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT derivation_ref FROM linggan_comment_research_derivation WHERE source_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

async fn atom_ref(database: &Database, run_ref: Uuid, derivation_ref: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT atom_ref FROM linggan_comment_research_atom \
         WHERE run_ref=$1 AND derivation_ref=$2 ORDER BY ordinal",
    )
    .bind(run_ref)
    .bind(derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

async fn synthetic_qualified_embedding_space(database: &Database) -> EmbeddingSpaceReceipt {
    let connection_ref = Uuid::new_v4();
    let version_ref = Uuid::new_v4();
    let model_ref = Uuid::new_v4();
    let config_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,enabled) VALUES($1,true)")
        .bind(connection_ref)
        .execute(database.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_connection_version( \
             version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref \
         ) VALUES($1,$2,1,'SYNTHETIC embedding connection','openai-completions', \
             'http://localhost:9',true,$3)",
    )
    .bind(version_ref)
    .bind(connection_ref)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) \
         VALUES($1,$2,'synthetic-embedding','manual')",
    )
    .bind(model_ref)
    .bind(version_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_embedding_config( \
             config_ref,model_ref,dimensions,qualified,enabled \
         ) VALUES($1,$2,2,true,true)",
    )
    .bind(config_ref)
    .bind(model_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_embedding_settings SET config_ref=$1 WHERE singleton")
        .bind(config_ref)
        .execute(database.pool())
        .await
        .unwrap();
    activate_configured_embedding_space(database).await.unwrap()
}

async fn start_ready_run(
    database: &Database,
) -> Result<ResearchRunReceipt, CommentResearchKernelError> {
    let ready: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_embedding_settings settings \
         JOIN linggan_embedding_config config USING(config_ref) \
         JOIN linggan_model_entry model USING(model_ref) \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE settings.singleton AND config.enabled AND config.qualified AND connection.enabled)",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    if !ready {
        synthetic_qualified_embedding_space(database).await;
    }
    start_run(database).await
}

async fn freeze_research_clock(database: &Database, now_at: &str) {
    sqlx::raw_sql(
        "CREATE TABLE research_result_test_clock( \
             singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),now_at timestamptz NOT NULL \
         ); \
         INSERT INTO research_result_test_clock(singleton,now_at) VALUES(true,'2026-09-09T12:00:00Z'); \
         CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ \
             SELECT (SELECT now_at FROM research_result_test_clock WHERE singleton) \
         $$;",
    )
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE research_result_test_clock SET now_at=$1::timestamptz WHERE singleton")
        .bind(now_at)
        .execute(database.pool())
        .await
        .unwrap();
}

async fn claim_is_in_current_result_window(
    database: &Database,
    run_ref: Uuid,
    derivation_ref: Uuid,
) -> bool {
    sqlx::query_scalar(
        "SELECT source.observed_at::timestamptz >= '2026-09-01T16:00:00Z'::timestamptz \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation derivation USING(derivation_ref) \
         JOIN linggan_material_comment source ON source.material_ref=derivation.source_ref \
         WHERE item.run_ref=$1 AND item.derivation_ref=$2",
    )
    .bind(run_ref)
    .bind(derivation_ref)
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
async fn later_author_attribution_creates_a_new_current_derivation() {
    let database = fixture::proof_database("comment_research_derivation_attribution_head").await;
    detail_with_author(
        &database,
        "attribution-note",
        "SYNTHETIC attribution note",
        None,
    )
    .await;
    let comment = comment_with_author(
        &database,
        "attribution-note",
        "reader-comment",
        "作者 说得很对，但我还是不知道怎么做",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
    let first: (String, String) = sqlx::query_as(
        "SELECT author_role,eligibility FROM linggan_comment_research_derivation_readable \
         WHERE source_ref=$1",
    )
    .bind(comment)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        first,
        (
            "author_identity_unknown".into(),
            "author_identity_unknown".into()
        )
    );

    detail_with_author(
        &database,
        "attribution-note",
        "SYNTHETIC attribution note",
        Some("creator-1"),
    )
    .await;
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
    let history_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_derivation WHERE source_ref=$1",
    )
    .bind(comment)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(history_count, 2);
    let readable_history: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_derivation_readable WHERE source_ref=$1",
    )
    .bind(comment)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(readable_history, 2);
    let current: (String, String, String) = sqlx::query_as(
        "SELECT author_role,eligibility,research_text \
         FROM linggan_comment_research_derivation_current WHERE source_ref=$1",
    )
    .bind(comment)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        current,
        (
            "ordinary_user".into(),
            "eligible".into(),
            "作者 说得很对,但我还是不知道怎么做".into()
        )
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn unchanged_unknown_author_does_not_starve_older_ordinary_derivation() {
    let database = fixture::proof_database("comment_research_unknown_author_selection").await;
    detail_with_author(
        &database,
        "unknown-author-selection-note",
        "SYNTHETIC selection note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "unknown-author-selection-note",
        "ordinary-older",
        "孩子总是拖着不写作业",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "unknown-author-selection-note",
        "unknown-newer",
        "我家也是这种情况",
        None,
        "2026-09-01T08:01:00Z",
    )
    .await;

    assert_eq!(derive_current_sources(&database, 1).await.unwrap(), 1);
    assert_eq!(derive_current_sources(&database, 1).await.unwrap(), 1);
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT count(*),count(*) FILTER (WHERE eligibility='eligible') \
         FROM linggan_comment_research_derivation",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(counts, (2, 1));
    assert_eq!(derive_current_sources(&database, 1).await.unwrap(), 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn frozen_run_remains_executable_but_stale_result_is_hidden_after_author_correction() {
    let database = fixture::proof_database("comment_research_frozen_derivation_readability").await;
    detail_with_author(
        &database,
        "frozen-input-note",
        "SYNTHETIC frozen input note",
        Some("creator-1"),
    )
    .await;
    let source = comment_with_author(
        &database,
        "frozen-input-note",
        "reader-comment",
        "这个方法很有用",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();

    detail_with_author(
        &database,
        "frozen-input-note",
        "SYNTHETIC frozen input note",
        Some("reader-1"),
    )
    .await;
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
    let current_role: String = sqlx::query_scalar(
        "SELECT author_role FROM linggan_comment_research_derivation_current WHERE source_ref=$1",
    )
    .bind(source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(current_role, "content_author_reply");

    let claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::NoSignal {
            reason: "礼貌表达，不包含可归并的问题".into(),
        },
        None,
    )
    .await
    .unwrap();
    let result = publish_result_revision(&database, run.run_ref)
        .await
        .unwrap();
    let published: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_result_revision WHERE result_revision_ref=$1",
    )
    .bind(result.result_revision_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let readable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_result_revision_readable WHERE result_revision_ref=$1",
    )
    .bind(result.result_revision_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(published, 1);
    assert_eq!(
        readable, 0,
        "a changed input must hide the entire stale result"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn later_comments_are_derived_and_frozen_before_older_unprocessed_input() {
    let database = fixture::proof_database("comment_research_incremental_selection").await;
    detail_with_author(
        &database,
        "incremental-note",
        "SYNTHETIC incremental note",
        Some("creator-1"),
    )
    .await;
    let older = comment_with_author(
        &database,
        "incremental-note",
        "older",
        "旧评论：孩子写作业总拖延",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 1).await.unwrap(), 1);
    let newer = comment_with_author(
        &database,
        "incremental-note",
        "newer",
        "新评论：孩子一写作业就回避",
        Some("reader-2"),
        "2026-09-02T08:00:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 1).await.unwrap(), 1);
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 1,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();

    let first_run = start_ready_run(&database).await.unwrap();
    let first_source: Uuid = sqlx::query_scalar(
        "SELECT derivation.source_ref FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation derivation USING(derivation_ref) \
         WHERE item.run_ref=$1",
    )
    .bind(first_run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(first_source, newer);

    let second_run = start_ready_run(&database).await.unwrap();
    let second_source: Uuid = sqlx::query_scalar(
        "SELECT derivation.source_ref FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation derivation USING(derivation_ref) \
         WHERE item.run_ref=$1",
    )
    .bind(second_run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(second_source, older);
    assert!(matches!(
        start_ready_run(&database).await,
        Err(linggan_intelligence::comment_research_kernel::CommentResearchKernelError::NoEligibleDerivations)
    ));
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
    let receipt = start_ready_run(&database).await.unwrap();

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
    start_ready_run(&database).await.unwrap();

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

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn expired_v1_lease_recovers_without_stalling_the_run() {
    let database = fixture::proof_database("comment_research_v1_lease_recovery").await;
    detail_with_author(
        &database,
        "lease-note",
        "SYNTHETIC lease recovery note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "lease-note",
        "reader",
        "孩子写作业时总是拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
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
    let run = start_ready_run(&database).await.unwrap();
    let first = claim_next_run_item(&database).await.unwrap().unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_run_item \
         SET lease_until=scope_001_now()-interval '1 second' \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(first.run_ref)
    .bind(first.derivation_ref)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(recover_expired_run_items(&database).await.unwrap(), 1);
    let state: (String, Option<String>) = sqlx::query_as(
        "SELECT state,lease_until::text FROM linggan_comment_research_run_item \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(first.run_ref)
    .bind(first.derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state, ("retryable".into(), None));
    sqlx::query(
        "UPDATE linggan_comment_research_run_item \
         SET next_attempt_at=scope_001_now()-interval '1 second' \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(first.run_ref)
    .bind(first.derivation_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let second = claim_next_run_item(&database).await.unwrap().unwrap();
    assert_eq!(second.run_ref, run.run_ref);
    assert_eq!(second.derivation_ref, first.derivation_ref);
    assert_eq!(second.attempt, 2);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn accepted_atoms_are_evidence_bound_and_invalid_model_output_writes_nothing() {
    let database = fixture::proof_database("comment_research_atom_acceptance").await;
    detail_with_author(
        &database,
        "atom-note",
        "SYNTHETIC atom acceptance note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "atom-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
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
    let run = start_ready_run(&database).await.unwrap();
    let claim = claim_next_run_item(&database).await.unwrap().unwrap();

    let invalid = accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: " ".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await;
    assert!(matches!(
        invalid,
        Err(CommentResearchAtomError::InvalidOutput)
    ));
    let after_invalid: (String, i64) = sqlx::query_as(
        "SELECT item.state,count(atom.atom_ref) \
         FROM linggan_comment_research_run_item item \
         LEFT JOIN linggan_comment_research_atom atom \
           ON atom.run_ref=item.run_ref AND atom.derivation_ref=item.derivation_ref \
         WHERE item.run_ref=$1 AND item.derivation_ref=$2 \
         GROUP BY item.state",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(after_invalid, ("running".into(), 0));

    let receipt = accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子写作业时存在持续拖延".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "succeeded");
    assert_eq!(receipt.accepted_atoms, 1);

    let atom: (String, i32, i32, i32, i32) = sqlx::query_as(
        "SELECT kind,research_start,research_end,source_start,source_end \
         FROM linggan_comment_research_atom \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(atom, ("problem".into(), 0, 5, 0, 5));
    let run_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_research_run WHERE run_ref=$1")
            .bind(run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(run_state, "running");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn unavailable_embedding_settles_a_run_as_failure_instead_of_completed_unpublished() {
    let database = fixture::proof_database("comment_research_embedding_unavailable").await;
    detail_with_author(
        &database,
        "embedding-unavailable-note",
        "SYNTHETIC embedding unavailable note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "embedding-unavailable-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_embedding_config SET enabled=false")
        .execute(database.pool())
        .await
        .unwrap();
    assert_eq!(
        fail_active_runs_without_embedding_config(&database)
            .await
            .unwrap(),
        1
    );
    let state: (String, serde_json::Value) = sqlx::query_as(
        "SELECT state,failure_counts FROM linggan_comment_research_run WHERE run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state.0, "completed_with_failures");
    assert_eq!(state.1["embedding"], 1);
    let published: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_result_revision WHERE run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(published, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn unavailable_embedding_prevents_run_creation_and_terminalizes_queued_work_without_calls() {
    let database = fixture::proof_database("comment_research_embedding_start_gate").await;
    detail_with_author(
        &database,
        "embedding-start-gate-note",
        "SYNTHETIC embedding start gate note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "embedding-start-gate-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        start_run(&database).await,
        Err(CommentResearchKernelError::EmbeddingNotReady)
    ));
    let no_runs: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_run")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(no_runs, 0);

    let run = start_ready_run(&database).await.unwrap();
    sqlx::query("UPDATE linggan_embedding_config SET enabled=false")
        .execute(database.pool())
        .await
        .unwrap();
    assert_eq!(
        fail_active_runs_without_embedding_config(&database)
            .await
            .unwrap(),
        1
    );
    let outcome: (String, String, i64) = sqlx::query_as(
        "SELECT run.state,item.state,(SELECT count(*) FROM linggan_model_invocation invocation \
         WHERE invocation.result->>'runRef'=run.run_ref::text) \
         FROM linggan_comment_research_run run \
         JOIN linggan_comment_research_run_item item USING(run_ref) WHERE run.run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        outcome,
        ("completed_with_failures".into(), "incompatible".into(), 0)
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn terminal_embedding_and_resolution_failures_are_visible_on_the_run() {
    for (suffix, resolution_failure) in [("embedding", false), ("resolution", true)] {
        let database = fixture::proof_database(&format!("comment_research_{suffix}_failure")).await;
        detail_with_author(
            &database,
            "terminal-failure-note",
            "SYNTHETIC terminal failure note",
            Some("creator-1"),
        )
        .await;
        comment_with_author(
            &database,
            "terminal-failure-note",
            suffix,
            "孩子写作业总拖延，有什么办法吗",
            Some("reader-1"),
            "2026-09-01T08:00:00Z",
        )
        .await;
        let space = synthetic_qualified_embedding_space(&database).await;
        save_active_policy(
            &database,
            SaveResearchPolicy {
                config_ref: None,
                source_limit: 10,
                token_limit: 10_000,
            },
        )
        .await
        .unwrap();
        let run = start_ready_run(&database).await.unwrap();
        let claim = claim_next_run_item(&database).await.unwrap().unwrap();
        accept_semantic_output(
            &database,
            &claim,
            SemanticExtractionOutput::Atoms {
                atoms: vec![SemanticAtomProposal {
                    kind: AtomKind::Problem,
                    proposition: "孩子难以启动写作业".into(),
                    basis: AtomBasis::Explicit,
                    evidence_start: 0,
                    evidence_end: 5,
                }],
            },
            None,
        )
        .await
        .unwrap();
        let atom = atom_ref(&database, run.run_ref, claim.derivation_ref).await;
        if resolution_failure {
            let input = queue_atom_embedding(&database, atom, space.space_ref)
                .await
                .unwrap();
            accept_atom_embedding(
                &database,
                AtomEmbeddingResult {
                    atom_ref: atom,
                    space_ref: space.space_ref,
                    input_hash: input.input_hash,
                    values: vec![1.0, 0.0],
                    invocation_ref: None,
                },
            )
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO linggan_comment_research_problem_resolution( \
                     atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,finished_at \
                 ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','synthetic_resolution_failure',scope_001_now())",
            )
            .bind(atom)
            .bind(space.space_ref)
            .bind(HASH)
            .execute(database.pool())
            .await
            .unwrap();
        } else {
            let claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
            record_embedding_failure(&database, &claim, None, "synthetic_embedding_failure")
                .await
                .unwrap();
        }
        refresh_run_completion_for_atom(&database, atom)
            .await
            .unwrap();
        let failures: serde_json::Value = sqlx::query_scalar(
            "SELECT failure_counts FROM linggan_comment_research_run WHERE run_ref=$1 AND state='completed_with_failures'",
        )
        .bind(run.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(
            failures[if resolution_failure {
                "problemResolution"
            } else {
                "embedding"
            }],
            1
        );
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn membership_admission_waits_for_its_running_resolution_to_settle() {
    let database = fixture::proof_database("comment_research_resolution_settlement_boundary").await;
    detail_with_author(
        &database,
        "resolution-boundary-note",
        "SYNTHETIC resolution boundary note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "resolution-boundary-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let atom = atom_ref(&database, run.run_ref, claim.derivation_ref).await;
    let input = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    accept_atom_embedding(
        &database,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: input.input_hash,
            values: vec![1.0, 0.0],
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,lease_until \
         ) VALUES($1,$2,'[]'::jsonb,$3,'running',scope_001_now()+interval '120 seconds')",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();

    admit_new_problem(
        &database,
        NewProblemAdmission {
            atom_ref: atom,
            definition: ProblemDefinitionProposal {
                name: "写作业启动困难".into(),
                meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
            },
            basis: ProblemMembershipBasis::Deterministic,
            decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    let before_settlement: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_research_run WHERE run_ref=$1")
            .bind(run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(before_settlement, "running");

    assert_eq!(
        recover_problem_resolution_leases(&database).await.unwrap(),
        1
    );
    let after_settlement: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_research_run WHERE run_ref=$1")
            .bind(run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(after_settlement, "completed");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn unavailable_embedding_terminalizes_a_frozen_resolution_without_a_new_call() {
    let database = fixture::proof_database("comment_research_embedding_resolution_continues").await;
    detail_with_author(
        &database,
        "embedding-resolution-note",
        "SYNTHETIC embedding resolution note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "embedding-resolution-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: None,
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let atom = atom_ref(&database, run.run_ref, claim.derivation_ref).await;
    let input = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    accept_atom_embedding(
        &database,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: input.input_hash,
            values: vec![1.0, 0.0],
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state \
         ) VALUES($1,$2,'[]'::jsonb,$3,'pending')",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_embedding_config SET enabled=false")
        .execute(database.pool())
        .await
        .unwrap();

    fail_active_runs_without_embedding_config(&database)
        .await
        .unwrap();
    let outcome: (String, String, i64) = sqlx::query_as(
        "SELECT run.state,resolution.state,(SELECT count(*) FROM linggan_model_invocation invocation \
         WHERE invocation.result->>'runRef'=run.run_ref::text) \
         FROM linggan_comment_research_run run \
         JOIN linggan_comment_research_problem_resolution resolution ON resolution.atom_ref=$2 \
         WHERE run.run_ref=$1",
    )
            .bind(run.run_ref)
            .bind(atom)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        outcome,
        ("completed_with_failures".into(), "incompatible".into(), 0)
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn no_signal_is_a_terminal_run_item_outcome_not_an_atom() {
    let database = fixture::proof_database("comment_research_atom_no_signal").await;
    detail_with_author(
        &database,
        "no-signal-note",
        "SYNTHETIC no-signal note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "no-signal-note",
        "reader",
        "谢谢",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
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
    let run = start_ready_run(&database).await.unwrap();
    let claim = claim_next_run_item(&database).await.unwrap().unwrap();

    let receipt = accept_semantic_output(
        &database,
        &claim,
        SemanticExtractionOutput::NoSignal {
            reason: "礼貌性表达，不包含可研究的用户语义".into(),
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "no_signal");
    assert_eq!(receipt.accepted_atoms, 0);
    let item_state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_research_run_item \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(item_state, "no_signal");
    let atoms: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_atom WHERE run_ref=$1")
            .bind(run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(atoms, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn atom_problem_membership_has_one_stable_identity_and_auditable_basis() {
    let database = fixture::proof_database("comment_research_problem_membership").await;
    detail_with_author(
        &database,
        "problem-note",
        "SYNTHETIC Problem membership note",
        Some("creator-1"),
    )
    .await;
    for (id, body) in [
        ("first", "孩子写作业总拖延，有什么办法吗"),
        ("second", "孩子一到写作业就拖着不开始"),
    ] {
        comment_with_author(
            &database,
            "problem-note",
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
    let run = start_ready_run(&database).await.unwrap();

    let first_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &first_claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let first_atom = atom_ref(&database, run.run_ref, first_claim.derivation_ref).await;
    let created = admit_new_problem(
        &database,
        NewProblemAdmission {
            atom_ref: first_atom,
            definition: ProblemDefinitionProposal {
                name: "写作业启动困难".into(),
                meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
            },
            basis: ProblemMembershipBasis::Deterministic,
            decision_evidence: json!({
                "decision":"new_problem",
                "candidateRefs":[],
                "reason":"synthetic bootstrap with no candidate definitions"
            }),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    let definition: (i32, String, String) = sqlx::query_as(
        "SELECT revision,name,meaning \
         FROM linggan_comment_research_problem_definition WHERE problem_ref=$1",
    )
    .bind(created.problem_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        definition,
        (
            1,
            "写作业启动困难".into(),
            "孩子在开始完成作业前持续拖延或难以行动".into()
        )
    );
    assert!(matches!(
        admit_new_problem(
            &database,
            NewProblemAdmission {
                atom_ref: first_atom,
                definition: ProblemDefinitionProposal {
                    name: "重复定义".into(),
                    meaning: "不应创建第二个当前归属".into(),
                },
                basis: ProblemMembershipBasis::Deterministic,
                decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
                invocation_ref: None,
            },
        )
        .await,
        Err(CommentResearchProblemError::AtomAlreadyAssigned)
    ));

    let second_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &second_claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let second_atom = atom_ref(&database, run.run_ref, second_claim.derivation_ref).await;
    let assigned = admit_existing_problem(
        &database,
        ExistingProblemAdmission {
            atom_ref: second_atom,
            problem_ref: created.problem_ref,
            definition_revision: created.definition_revision,
            basis: ProblemMembershipBasis::Manual,
            decision_evidence: json!({
                "decision":"same_problem",
                "reason":"synthetic reviewer confirmed the same problem"
            }),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(assigned.problem_ref, created.problem_ref);
    assert_eq!(assigned.definition_revision, 1);
    assert_eq!(assigned.basis, ProblemMembershipBasis::Manual);
    let memberships: (i64, i64) = sqlx::query_as(
        "SELECT count(*),count(*) FILTER (WHERE current) \
         FROM linggan_comment_research_atom_problem_membership WHERE problem_ref=$1",
    )
    .bind(created.problem_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(memberships, (2, 2));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn exact_vector_recall_only_returns_candidates_and_invalid_vectors_never_write() {
    let database = fixture::proof_database("comment_research_vector_candidates").await;
    detail_with_author(
        &database,
        "vector-note",
        "SYNTHETIC vector candidate note",
        Some("creator-1"),
    )
    .await;
    for (id, body) in [
        ("first", "孩子写作业总拖延，有什么办法吗"),
        ("second", "孩子一到写作业就拖着不开始"),
    ] {
        comment_with_author(
            &database,
            "vector-note",
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
    let run = start_ready_run(&database).await.unwrap();
    let first_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &first_claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let first_atom = atom_ref(&database, run.run_ref, first_claim.derivation_ref).await;
    let problem = admit_new_problem(
        &database,
        NewProblemAdmission {
            atom_ref: first_atom,
            definition: ProblemDefinitionProposal {
                name: "写作业启动困难".into(),
                meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
            },
            basis: ProblemMembershipBasis::Deterministic,
            decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    let space = synthetic_qualified_embedding_space(&database).await;
    let definition_input = queue_problem_definition_embedding(
        &database,
        problem.problem_ref,
        problem.definition_revision,
        space.space_ref,
    )
    .await
    .unwrap();
    assert!(matches!(
        accept_problem_definition_embedding(
            &database,
            ProblemDefinitionEmbeddingResult {
                problem_ref: problem.problem_ref,
                definition_revision: problem.definition_revision,
                space_ref: space.space_ref,
                input_hash: definition_input.input_hash.clone(),
                values: vec![1.0],
                invocation_ref: None,
            },
        )
        .await,
        Err(CommentResearchEmbeddingError::InvalidEmbedding)
    ));
    accept_problem_definition_embedding(
        &database,
        ProblemDefinitionEmbeddingResult {
            problem_ref: problem.problem_ref,
            definition_revision: problem.definition_revision,
            space_ref: space.space_ref,
            input_hash: definition_input.input_hash,
            values: vec![1.0, 0.0],
            invocation_ref: None,
        },
    )
    .await
    .unwrap();

    let second_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &second_claim,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Problem,
                proposition: "孩子难以启动写作业".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 5,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let second_atom = atom_ref(&database, run.run_ref, second_claim.derivation_ref).await;
    let atom_input = queue_atom_embedding(&database, second_atom, space.space_ref)
        .await
        .unwrap();
    assert!(matches!(
        accept_atom_embedding(
            &database,
            AtomEmbeddingResult {
                atom_ref: second_atom,
                space_ref: space.space_ref,
                input_hash: atom_input.input_hash.clone(),
                values: vec![f64::NAN, 0.0],
                invocation_ref: None,
            },
        )
        .await,
        Err(CommentResearchEmbeddingError::InvalidEmbedding)
    ));
    let atom_work_state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_research_atom_embedding \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(second_atom)
    .bind(space.space_ref)
    .bind(&atom_input.input_hash)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(atom_work_state, "pending");
    accept_atom_embedding(
        &database,
        AtomEmbeddingResult {
            atom_ref: second_atom,
            space_ref: space.space_ref,
            input_hash: atom_input.input_hash,
            values: vec![0.99, 0.1],
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    let candidates = recall_problem_candidates(&database, second_atom, space.space_ref)
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].problem_ref, problem.problem_ref);
    assert_eq!(candidates[0].definition_revision, 1);
    assert!(candidates[0].cosine > 0.9);
    let memberships: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_atom_problem_membership \
         WHERE atom_ref=$1 AND current",
    )
    .bind(second_atom)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(memberships, 0, "candidate recall must not assign a Problem");
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0, "synthetic vector acceptance made no model call");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn published_result_has_independent_multi_signal_changes_and_is_idempotent() {
    let database = fixture::proof_database("comment_research_result_revision").await;
    freeze_research_clock(&database, "2026-09-09T12:00:00Z").await;
    for note in ["result-note-a", "result-note-b"] {
        detail_with_author(
            &database,
            note,
            "SYNTHETIC result revision note",
            Some("creator-1"),
        )
        .await;
    }
    for (note, id, body, observed_at) in [
        (
            "result-note-a",
            "baseline-1",
            "谢谢",
            "2026-08-27T08:00:00Z",
        ),
        (
            "result-note-a",
            "baseline-2",
            "谢谢",
            "2026-08-28T08:00:00Z",
        ),
        (
            "result-note-a",
            "baseline-3",
            "谢谢",
            "2026-08-29T08:00:00Z",
        ),
        (
            "result-note-a",
            "current-1",
            "孩子一到写作业就很难开始",
            "2026-09-02T08:00:00Z",
        ),
        (
            "result-note-b",
            "current-2",
            "每天写作业前都拖着不动",
            "2026-09-03T08:00:00Z",
        ),
        (
            "result-note-b",
            "current-3",
            "求一个让孩子开始写作业的方法",
            "2026-09-04T08:00:00Z",
        ),
    ] {
        comment_with_author(&database, note, id, body, Some("reader-1"), observed_at).await;
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
    let run = start_ready_run(&database).await.unwrap();
    let mut created_problem: Option<(Uuid, i32)> = None;
    while let Some(claim) = claim_next_run_item(&database).await.unwrap() {
        if !claim_is_in_current_result_window(&database, run.run_ref, claim.derivation_ref).await {
            accept_semantic_output(
                &database,
                &claim,
                SemanticExtractionOutput::NoSignal {
                    reason: "礼貌表达，不包含可归并的问题".into(),
                },
                None,
            )
            .await
            .unwrap();
            continue;
        }
        accept_semantic_output(
            &database,
            &claim,
            SemanticExtractionOutput::Atoms {
                atoms: vec![SemanticAtomProposal {
                    kind: AtomKind::Problem,
                    proposition: "孩子难以启动写作业".into(),
                    basis: AtomBasis::Explicit,
                    evidence_start: 0,
                    evidence_end: 5,
                }],
            },
            None,
        )
        .await
        .unwrap();
        let atom = atom_ref(&database, run.run_ref, claim.derivation_ref).await;
        if let Some((problem_ref, definition_revision)) = created_problem {
            admit_existing_problem(
                &database,
                ExistingProblemAdmission {
                    atom_ref: atom,
                    problem_ref,
                    definition_revision,
                    basis: ProblemMembershipBasis::Manual,
                    decision_evidence: json!({
                        "decision":"same_problem",
                        "reason":"synthetic reviewer confirmed the same problem"
                    }),
                    invocation_ref: None,
                },
            )
            .await
            .unwrap();
        } else {
            let created = admit_new_problem(
                &database,
                NewProblemAdmission {
                    atom_ref: atom,
                    definition: ProblemDefinitionProposal {
                        name: "写作业启动困难".into(),
                        meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
                    },
                    basis: ProblemMembershipBasis::Deterministic,
                    decision_evidence: json!({
                        "decision":"new_problem",
                        "candidateRefs":[]
                    }),
                    invocation_ref: None,
                },
            )
            .await
            .unwrap();
            created_problem = Some((created.problem_ref, created.definition_revision));
        }
    }
    let (problem_ref, _) = created_problem.unwrap();
    let first = publish_result_revision(&database, run.run_ref)
        .await
        .unwrap();
    assert_eq!(first.published_observations, 3);
    assert_eq!(first.current_window_start, "2026-09-02 00:00:00+08");
    assert_eq!(first.current_window_end, "2026-09-09 00:00:00+08");
    let signals: Vec<String> = sqlx::query_scalar(
        "SELECT kind FROM linggan_comment_research_change_observation \
         WHERE result_revision_ref=$1 AND status='published' ORDER BY kind",
    )
    .bind(first.result_revision_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(signals, vec!["newly_observed", "rising", "spreading"]);
    let stats: Vec<(String, i32, i32, i32, i32)> = sqlx::query_as(
        "SELECT window_kind,comment_count,comment_denominator,work_count,work_denominator \
         FROM linggan_comment_research_problem_window_stat \
         WHERE result_revision_ref=$1 AND problem_ref=$2 ORDER BY window_kind",
    )
    .bind(first.result_revision_ref)
    .bind(problem_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        stats,
        vec![
            ("baseline".into(), 0, 3, 0, 1),
            ("current".into(), 3, 3, 2, 2)
        ]
    );
    let second = publish_result_revision(&database, run.run_ref)
        .await
        .unwrap();
    assert_eq!(second.result_revision_ref, first.result_revision_ref);
    let readable_revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_result_revision_readable WHERE run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(readable_revisions, 1);

    let query = CommentResearchV1ReadQuery {
        result_revision_ref: Some(first.result_revision_ref),
        limit: Some(20),
        offset: Some(0),
    };
    let overview = read_overview(&database, &query).await.unwrap();
    assert_eq!(overview["view"], "overview");
    assert_eq!(overview["currentProblems"].as_array().unwrap().len(), 1);
    assert!(overview.get("observations").is_none());

    let voices = read_voices(&database, &query).await.unwrap();
    assert_eq!(voices["view"], "voices");
    assert_eq!(voices["page"]["total"], 6);
    assert!(
        voices["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|voice| voice.get("embeddingState").is_none())
    );

    let problems = read_problems(&database, &query).await.unwrap();
    assert_eq!(problems["view"], "problems");
    assert_eq!(problems["page"]["total"], 1);
    assert_eq!(problems["page"]["items"][0]["evidenceAtomCount"], 3);

    let changes = read_changes(&database, &query).await.unwrap();
    assert_eq!(changes["view"], "changes");
    assert_eq!(changes["observations"].as_array().unwrap().len(), 3);
    assert!(changes.get("currentProblems").is_none());
    assert!(changes["notComparable"].as_array().unwrap().is_empty());

    let runs = read_runs(&database, &query).await.unwrap();
    assert_eq!(runs["view"], "runs");
    assert_eq!(runs["page"]["total"], 1);
    assert_eq!(
        runs["page"]["items"][0]["publishedResult"]["resultRevisionRef"],
        first.result_revision_ref.to_string()
    );
}
