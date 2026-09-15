#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use linggan_intelligence::comment_research_atoms::{
    AtomBasis, AtomKind, CommentResearchAtomError, FrameBasis, ProblemFrameFieldProposal,
    ProblemFrameProposal, SemanticAtomProposal, SemanticExtractionOutput,
    SemanticQuoteAtomProposal, SemanticQuoteExtractionOutput, accept_semantic_output,
    accept_semantic_quote_output,
};
use linggan_intelligence::comment_research_embeddings::{
    AtomEmbeddingResult, CommentResearchEmbeddingError, EmbeddingSpaceReceipt,
    accept_atom_embedding, activate_configured_embedding_space, claim_next_embedding_work,
    queue_atom_embedding, recall_problem_candidates, record_embedding_failure,
    recover_expired_embedding_work,
};
use linggan_intelligence::comment_research_kernel::{
    CommentResearchKernelError, DERIVATION_VERSION, ResearchRunReceipt, RunItemFailureClass,
    SaveResearchPolicy, claim_next_run_item, derive_current_sources,
    fail_active_runs_without_embedding_config, preview_run, prewarm_current_sources,
    record_run_item_failure, recover_expired_run_items, refresh_run_completion_for_atom,
    reset_development_derived, save_active_policy, start_run,
};
use linggan_intelligence::comment_research_problems::{
    CommentResearchProblemError, ExistingProblemAdmission, NewProblemAdmission,
    NewProblemPairAdmission, ProblemAdmissionReceipt, ProblemDefinitionProposal, ProblemMembershipBasis,
    StableProblemDefinitionProposal, admit_existing_problem, admit_new_problem,
    admit_new_problem_pair,
};
use linggan_intelligence::comment_research_read_v1::{
    CommentResearchV1ReadError, CommentResearchV1ReadQuery, ProblemReadView, read_changes,
    read_overview, read_problems, read_runs, read_voices,
};
use linggan_intelligence::comment_research_results::publish_result_revision;
use linggan_intelligence::comment_research_worker::{
    recover_problem_resolution_leases, run_once, schema_ready,
};
use linggan_intelligence::model_invocation::{ProbeModel, probe_model};
use linggan_intelligence::model_secrets::SyntheticModelSecrets;
use linggan_intelligence::model_settings::{
    ModelError, SaveModelConfig, reserve_research_model_semantic_dispatch_permit, save_model_config,
};
use linggan_intelligence::model_settings_read::read_model_settings;
use linggan_intelligence::model_worker_drain::ModelWorkerDrain;
use linggan_intelligence::pi_adapter::PiAdapter;
use linggan_storage_postgres::Database;
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use serde_json::json;
use sqlx::Row;
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;

const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Production V2 policies must never use the legacy one-Atom create path. Existing regression
/// fixtures still exercise historical packet repair, so they explicitly fork a legacy policy
/// revision after the Run has frozen rather than weakening the production admission guard.
async fn admit_legacy_new_problem(
    database: &Database,
    admission: NewProblemAdmission,
) -> Result<ProblemAdmissionReceipt, CommentResearchProblemError> {
    let legacy_policy_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_research_policy_revision( \
             policy_revision_ref,config_ref,contract_version,derivation_version,extraction_rule_hash, \
             membership_policy_hash,source_limit,token_limit \
         ) SELECT $2,policy.config_ref,policy.contract_version,policy.derivation_version, \
                  policy.extraction_rule_hash,policy.membership_policy_hash,policy.source_limit,policy.token_limit \
           FROM linggan_comment_research_atom atom \
           JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
           JOIN linggan_comment_research_policy_revision policy \
             ON policy.policy_revision_ref=run.policy_revision_ref \
          WHERE atom.atom_ref=$1",
    )
    .bind(admission.atom_ref)
    .bind(legacy_policy_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_run run SET policy_revision_ref=$2 \
         FROM linggan_comment_research_atom atom \
         WHERE atom.atom_ref=$1 AND run.run_ref=atom.run_ref",
    )
    .bind(admission.atom_ref)
    .bind(legacy_policy_ref)
    .execute(database.pool())
    .await
    .unwrap();
    admit_new_problem(database, admission).await
}

fn valid_v2_problem_frame() -> ProblemFrameProposal {
    let supported = |value: &str| ProblemFrameFieldProposal {
        value: Some(value.to_owned()),
        basis: FrameBasis::ContextResolved,
        evidence_refs: vec!["atom_evidence".into()],
    };
    ProblemFrameProposal {
        scope_relation: linggan_intelligence::comment_research_problem_resolution_v2::ScopeRelation::InScope,
        subject: supported("孩子"),
        goal: supported("自主开始家庭作业"),
        barrier: supported("家庭作业启动困难"),
        context: supported("家庭作业"),
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn worker_refuses_to_run_without_cross_run_execution_history_schema() {
    let database = fixture::proof_database("comment_research_execution_history_schema_guard").await;
    assert!(schema_ready(&database).await.unwrap());
    sqlx::query("DROP TABLE linggan_comment_research_problem_resolution_execution")
        .execute(database.pool())
        .await
        .unwrap();
    assert!(!schema_ready(&database).await.unwrap());
}

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
async fn local_embedding_cutover_removes_definition_embeddings_without_touching_raw_evidence() {
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
    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    start_ready_run(&database).await.unwrap();

    let retired_relation: Option<String> = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_research_problem_definition_embedding')::text",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(retired_relation.is_none());
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

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn reply_derivation_freezes_clean_parent_text_in_semantic_context_not_audit_context() {
    let database = fixture::proof_database("comment_research_parent_context_input").await;
    detail_with_author(
        &database,
        "parent-context-note",
        "SYNTHETIC parent context note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "parent-context-note",
        "parent",
        "我真的习惯性熬夜，有时候会熬通宵",
        Some("reader-parent"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let reply = reply_with_author(
        &database,
        "parent-context-note",
        "reply",
        "parent",
        "我也是",
        Some("reader-reply"),
        "2026-09-01T08:00:01Z",
    )
    .await;

    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 2);
    let manifest: serde_json::Value = sqlx::query_scalar(
        "SELECT context_manifest FROM linggan_comment_research_derivation \
         WHERE source_ref=$1 AND derivation_version=$2",
    )
    .bind(reply)
    .bind(DERIVATION_VERSION)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(manifest["contract"], "comment-research.context.v2");
    assert_eq!(manifest["semantic"]["parent"]["state"], "available");
    assert_eq!(
        manifest["semantic"]["parent"]["researchText"],
        "我真的习惯性熬夜,有时候会熬通宵"
    );
    assert!(manifest["audit"].get("researchText").is_none());
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn context_dependent_reply_without_readable_parent_is_terminal_without_a_model_call() {
    let database = fixture::proof_database("comment_research_missing_parent_context").await;
    detail_with_author(
        &database,
        "missing-parent-note",
        "SYNTHETIC missing parent context note",
        Some("creator-1"),
    )
    .await;
    reply_with_author(
        &database,
        "missing-parent-note",
        "orphan-reply",
        "missing-parent",
        "我也是",
        Some("reader-reply"),
        "2026-09-01T08:00:01Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    assert_eq!(run.selected_sources, 1);

    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from("/bin/sh"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_research_semantic_settlement_adapter.sh"),
    );
    assert!(
        run_once(
            &database,
            &SyntheticModelSecrets,
            &adapter,
            &ModelWorkerDrain::new(),
        )
        .await
        .unwrap()
    );
    let item: (String, Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT state,failure_code,invocation_ref FROM linggan_comment_research_run_item \
         WHERE run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(item.0, "incompatible");
    assert_eq!(
        item.1.as_deref(),
        Some("context_insufficient_parent_unavailable")
    );
    assert!(
        item.2.is_none(),
        "missing context must not reserve a model call"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn saved_policy_with_outdated_output_rule_hashes_must_be_replaced_before_a_v6_run_is_created()
{
    let database = fixture::proof_database("comment_research_stale_policy_contract").await;
    detail_with_author(
        &database,
        "stale-policy-note",
        "SYNTHETIC stale policy note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "stale-policy-note",
        "ordinary",
        "孩子写作业总是拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let stale_policy_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_research_policy_revision( \
             policy_revision_ref,config_ref,contract_version,derivation_version,extraction_rule_hash, \
             membership_policy_hash,source_limit,token_limit \
         ) VALUES($1,$2,'comment-research.semantic.v1','comment-research.derivation.v2',$3,$3,10,10000)",
    )
    .bind(stale_policy_ref)
    .bind(config_ref)
    .bind(HASH)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_policy_active \
         SET policy_revision_ref=$1,revision=revision+1,updated_at=scope_001_now() WHERE singleton",
    )
    .bind(stale_policy_ref)
    .execute(database.pool())
    .await
    .unwrap();

    assert!(matches!(
        start_ready_run(&database).await,
        Err(CommentResearchKernelError::PolicyInputContractStale)
    ));
    let run_count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_run")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(run_count, 0, "stale policy must not freeze a V6 Run");
}

async fn insert_historical_derivation_version(database: &Database, source_ref: Uuid) {
    sqlx::query(
        "INSERT INTO linggan_comment_research_derivation( \
             derivation_ref,source_ref,derivation_version,source_sha256,cleaner_version,clean_state, \
             research_text,research_sha256,derivation_input_hash,research_offsets,normalization_reasons, \
             author_role,attribution_source,attribution_observed_at,eligibility,eligibility_reason,context_manifest \
         ) SELECT $1,source_ref,'comment-research.derivation.v0',source_sha256,cleaner_version,clean_state, \
                  research_text,research_sha256,$2,research_offsets,normalization_reasons, \
                  author_role,attribution_source,attribution_observed_at,eligibility,eligibility_reason,context_manifest \
           FROM linggan_comment_research_derivation \
          WHERE source_ref=$3 AND derivation_version=$4",
    )
    .bind(Uuid::new_v4())
    .bind("b".repeat(64))
    .bind(source_ref)
    .bind(DERIVATION_VERSION)
    .execute(database.pool())
    .await
    .unwrap();
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
    activate_configured_embedding_space(database).await.unwrap()
}

fn unit_vector_512() -> Vec<f64> {
    let mut values = vec![0.0; 512];
    values[0] = 1.0;
    values
}

async fn start_ready_run(
    database: &Database,
) -> Result<ResearchRunReceipt, CommentResearchKernelError> {
    let ready: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_research_embedding_profile WHERE singleton AND enabled)",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    if !ready {
        sqlx::query(
            "UPDATE linggan_comment_research_embedding_profile SET enabled=true WHERE singleton",
        )
        .execute(database.pool())
        .await
        .unwrap();
    }
    start_run(database).await
}

async fn synthetic_research_model(
    database: &Database,
    semantic_qualified: bool,
) -> (Uuid, Uuid, Uuid) {
    let connection_ref = Uuid::new_v4();
    let version_ref = Uuid::new_v4();
    let model_ref = Uuid::new_v4();
    let config_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)",
    )
    .bind(connection_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_connection_version( \
             version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref \
         ) VALUES($1,$2,1,'SYNTHETIC V1 research','openai-completions','http://127.0.0.1:18080',true,$3)",
    )
    .bind(version_ref)
    .bind(connection_ref)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) \
         VALUES($1,$2,'synthetic-comment-research-v1','manual')",
    )
    .bind(model_ref)
    .bind(version_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_config( \
             config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts \
         ) VALUES($1,$2,16000,2000,30,1)",
    )
    .bind(config_ref)
    .bind(model_ref)
    .execute(database.pool())
    .await
    .unwrap();
    record_synthetic_research_probe(
        database,
        version_ref,
        model_ref,
        config_ref,
        "succeeded",
        semantic_qualified,
    )
    .await;
    (config_ref, model_ref, version_ref)
}

async fn record_synthetic_research_probe(
    database: &Database,
    version_ref: Uuid,
    model_ref: Uuid,
    config_ref: Uuid,
    state: &str,
    semantic_qualified: bool,
) {
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
             invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state, \
             reserved_tokens,charged_tokens,result,finished_at \
         ) VALUES($1,$2,$3,$4,'probe',$5,$6,0,0,$7,scope_001_now())",
    )
    .bind(Uuid::new_v4())
    .bind(version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(HASH)
    .bind(state)
    .bind(json!({
        "ok": state == "succeeded",
        "modelCallable": state == "succeeded",
        "semanticQualified": semantic_qualified,
    }))
    .execute(database.pool())
    .await
    .unwrap();
}

async fn qualified_research_config(database: &Database) -> Uuid {
    synthetic_research_model(database, true).await.0
}

/// A bad offsets payload can only exist through historical corruption because derivations are
/// append-only. The isolated proof temporarily disables that one guard, restores it before the
/// worker runs, then verifies the production settlement path reports the corruption safely.
async fn corrupt_isolated_derivation_offsets(database: &Database, source_ref: Uuid) {
    sqlx::raw_sql(
        "ALTER TABLE linggan_comment_research_derivation \
         DISABLE TRIGGER linggan_comment_research_derivation_immutable",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let updated = sqlx::query(
        "UPDATE linggan_comment_research_derivation \
         SET research_offsets='[]'::jsonb WHERE source_ref=$1",
    )
    .bind(source_ref)
    .execute(database.pool())
    .await;
    sqlx::raw_sql(
        "ALTER TABLE linggan_comment_research_derivation \
         ENABLE TRIGGER linggan_comment_research_derivation_immutable",
    )
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(updated.unwrap().rows_affected(), 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn v1_model_admission_rejects_an_unqualified_default_and_policy_before_any_run() {
    let database = fixture::proof_database("comment_research_v1_model_admission").await;
    assert!(matches!(
        save_active_policy(
            &database,
            SaveResearchPolicy {
                config_ref: None,
                source_limit: 10,
                token_limit: 10_000,
            },
        )
        .await,
        Err(CommentResearchKernelError::ModelNotReady)
    ));

    let (_, unqualified_model_ref, _) = synthetic_research_model(&database, false).await;
    assert!(matches!(
        save_model_config(
            &database,
            &SaveModelConfig {
                config_ref: Uuid::new_v4(),
                expected_config_ref: None,
                model_ref: unqualified_model_ref,
                input_token_limit: 16_000,
                output_token_limit: 2_000,
                timeout_seconds: 30,
                max_attempts: 1,
            },
        )
        .await,
        Err(ModelError::NotQualified)
    ));
    let default_config: Option<Uuid> = sqlx::query_scalar(
        "SELECT default_config_ref FROM linggan_model_workspace WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(default_config.is_none());

    let qualified_config_ref = qualified_research_config(&database).await;
    let policy = save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    assert_eq!(policy.active_revision, 1);
    let contract_version: String = sqlx::query_scalar(
        "SELECT contract_version FROM linggan_comment_research_policy_revision \
         WHERE policy_revision_ref=$1",
    )
    .bind(policy.policy_revision_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(contract_version, "comment-research.semantic.v1");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn latest_failed_v1_probe_blocks_start_before_a_run_is_created() {
    let database = fixture::proof_database("comment_research_v1_start_preflight").await;
    detail_with_author(
        &database,
        "semantic-preflight-note",
        "SYNTHETIC semantic preflight note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "semantic-preflight-note",
        "reader",
        "孩子写作业时总是拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let (config_ref, model_ref, version_ref) = synthetic_research_model(&database, true).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    record_synthetic_research_probe(
        &database,
        version_ref,
        model_ref,
        config_ref,
        "failed",
        false,
    )
    .await;

    assert!(matches!(
        start_ready_run(&database).await,
        Err(CommentResearchKernelError::ModelNotReady)
    ));
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_run")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(runs, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn probe_write_and_failure_cannot_cross_a_reserved_generation_dispatch_permit() {
    let database = fixture::proof_database("comment_research_semantic_probe_permit").await;
    let (config_ref, model_ref, version_ref) = synthetic_research_model(&database, true).await;
    let permit = reserve_research_model_semantic_dispatch_permit(&database, config_ref)
        .await
        .unwrap();
    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from("/bin/sh"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_research_semantic_settlement_adapter.sh"),
    );
    let probe_database = database.clone();
    let mut probe = tokio::spawn(async move {
        probe_model(
            &probe_database,
            &SyntheticModelSecrets,
            &adapter,
            &ProbeModel {
                invocation_ref: Uuid::new_v4(),
                connection_version_ref: version_ref,
                model_ref: Some(model_ref),
                operation: "probe".into(),
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut probe)
            .await
            .is_err(),
        "probe insertion must wait until the generation dispatch permit is released"
    );

    permit.release().await.unwrap();
    probe.await.unwrap().unwrap();
    assert!(matches!(
        reserve_research_model_semantic_dispatch_permit(&database, config_ref).await,
        Err(ModelError::NotQualified)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn model_settings_projection_reads_the_model_alias_used_by_callability() {
    let database = fixture::proof_database("comment_research_model_settings_projection").await;
    let (config_ref, model_ref, _) = synthetic_research_model(&database, true).await;
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=$1 WHERE singleton")
        .bind(config_ref)
        .execute(database.pool())
        .await
        .unwrap();

    let settings = read_model_settings(&database, true).await.unwrap();
    let model = settings["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["modelRef"] == model_ref.to_string())
        .unwrap();
    assert_eq!(model["modelCallable"], true);
    assert_eq!(model["semanticQualified"], true);
    assert_eq!(model["testState"], "succeeded");
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

    assert_eq!(prewarm_current_sources(&database).await.unwrap(), 3);
    assert_eq!(prewarm_current_sources(&database).await.unwrap(), 0);

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
async fn voices_read_current_ordinary_user_evidence_without_a_published_result() {
    let database = fixture::proof_database("comment_research_current_voices").await;
    detail_with_author(
        &database,
        "current-voices-note",
        "SYNTHETIC current voices note",
        Some("creator-1"),
    )
    .await;
    let ordinary = comment_with_author(
        &database,
        "current-voices-note",
        "ordinary-reader",
        "孩子总是在写作业前拖延，我不知道怎么帮他开始。",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "current-voices-note",
        "author-reply",
        "作者 我会继续补充方法。",
        Some("creator-1"),
        "2026-09-01T08:01:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "current-voices-note",
        "unknown-reader",
        "我家也是这种情况。",
        None,
        "2026-09-01T08:02:00Z",
    )
    .await;
    assert_eq!(derive_current_sources(&database, 100).await.unwrap(), 3);
    insert_historical_derivation_version(&database, ordinary).await;
    let derivation_heads: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_derivation_current WHERE source_ref=$1",
    )
    .bind(ordinary)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(derivation_heads, 2);

    let latest_status_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
         WHERE schemaname=current_schema() \
           AND indexname='linggan_comment_research_run_item_derivation_latest_idx'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(latest_status_index.contains("derivation_ref, updated_at DESC, run_ref DESC"));

    let invocations_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(database.pool())
            .await
            .unwrap();
    let query = CommentResearchV1ReadQuery {
        // A stale result reference belongs to the published-result views only.  It cannot turn
        // an evidence browse into a `ResultUnavailable` response.
        result_revision_ref: Some(Uuid::new_v4()),
        limit: Some(1),
        offset: Some(0),
        problem_view: None,
    };
    let voices = read_voices(&database, &query).await.unwrap();
    assert_eq!(voices["view"], "voices");
    assert_eq!(voices["source"]["kind"], "current_readable_ordinary_user");
    assert!(voices.get("result").is_none());
    assert_eq!(voices["page"]["total"], 1);
    assert_eq!(voices["page"]["limit"], 1);
    assert_eq!(voices["page"]["offset"], 0);
    assert_eq!(
        voices["page"]["items"][0]["sourceRef"],
        ordinary.to_string()
    );
    assert_eq!(
        voices["page"]["items"][0]["researchText"],
        "孩子总是在写作业前拖延,我不知道怎么帮他开始。"
    );
    assert_eq!(voices["page"]["items"][0]["researchStatus"], "unresearched");
    assert!(
        voices["page"]["items"][0]
            .get("authorDisplayName")
            .is_none()
    );
    assert!(voices["page"]["items"][0].get("atomKinds").is_none());
    let invocations_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(invocations_after, invocations_before);

    let second_page = read_voices(
        &database,
        &CommentResearchV1ReadQuery {
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(second_page["page"]["total"], 1);
    assert!(second_page["page"]["items"].as_array().unwrap().is_empty());

    assert!(matches!(
        read_overview(&database, &query).await,
        Err(CommentResearchV1ReadError::ResultUnavailable)
    ));
    assert!(matches!(
        read_problems(&database, &query).await,
        Err(CommentResearchV1ReadError::ResultUnavailable)
    ));
    assert!(matches!(
        read_changes(&database, &query).await,
        Err(CommentResearchV1ReadError::ResultUnavailable)
    ));

    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    assert_eq!(run.external_calls_started, 0);
    let queued_voices = read_voices(&database, &query).await.unwrap();
    assert_eq!(
        queued_voices["page"]["items"][0]["researchStatus"],
        "pending"
    );
    assert!(queued_voices.get("result").is_none());
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
            config_ref: Some(qualified_research_config(&database).await),
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
            config_ref: Some(qualified_research_config(&database).await),
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
            config_ref: Some(qualified_research_config(&database).await),
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
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(calls, 0, "manifest freezing must not invoke a model");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn automatic_run_preview_recovers_cancelled_items_without_repeating_effective_or_active_research()
 {
    let database = fixture::proof_database("comment_research_automatic_run_preview").await;
    detail_with_author(
        &database,
        "preview-note",
        "SYNTHETIC automatic preview note",
        Some("creator-1"),
    )
    .await;
    for comment_id in ["no-signal", "succeeded", "retryable", "cancelled"] {
        comment_with_author(
            &database,
            "preview-note",
            comment_id,
            "孩子写作业很困难，需要帮助",
            Some("reader-1"),
            "2026-09-01T08:00:00Z",
        )
        .await;
    }
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let first_run = start_ready_run(&database).await.unwrap();

    let no_signal = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &no_signal,
        SemanticExtractionOutput::NoSignal {
            reason: "礼貌性回复之外没有明确研究信号".into(),
        },
        None,
    )
    .await
    .unwrap();
    let succeeded = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &succeeded,
        SemanticExtractionOutput::Atoms {
            atoms: vec![SemanticAtomProposal {
                kind: AtomKind::Need,
                proposition: "希望得到作业支持".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 1,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let retryable = claim_next_run_item(&database).await.unwrap().unwrap();
    record_run_item_failure(
        &database,
        &retryable,
        RunItemFailureClass::Retryable,
        "provider_timeout",
    )
    .await
    .unwrap();
    let cancelled = claim_next_run_item(&database).await.unwrap().unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_run_item \
         SET state='cancelled',failure_code='manual_cancelled',finished_at=scope_001_now(), \
             lease_until=NULL,updated_at=scope_001_now() \
         WHERE run_ref=$1 AND derivation_ref=$2 AND state='running'",
    )
    .bind(cancelled.run_ref)
    .bind(cancelled.derivation_ref)
    .execute(database.pool())
    .await
    .unwrap();

    comment_with_author(
        &database,
        "preview-note",
        "new-automatic-input",
        "我想知道怎样开始陪孩子写作业",
        Some("reader-1"),
        "2026-09-01T08:01:00Z",
    )
    .await;
    let calls_before_preview: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation IN ('analyze','embed')",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let preview = preview_run(&database).await.unwrap();
    assert!(preview.policy_configured);
    assert_eq!(preview.source_limit, Some(10));
    assert_eq!(preview.eligible_sources, 5);
    assert_eq!(preview.unprocessed_sources, 1);
    assert_eq!(preview.recoverable_sources, 1);
    assert_eq!(preview.selected_sources, 2);
    assert_eq!(preview.succeeded_sources, 1);
    assert_eq!(preview.no_signal_sources, 1);
    assert_eq!(preview.retryable_sources, 1);
    assert!(preview.terminal_item_states.get("cancelled").is_none());
    let public_preview = serde_json::to_value(&preview).unwrap();
    assert!(public_preview.get("derivationRefs").is_none());
    assert!(public_preview.get("commentText").is_none());
    let calls_after_preview: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation IN ('analyze','embed')",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(calls_after_preview, calls_before_preview);

    let second_run = start_ready_run(&database).await.unwrap();
    assert_ne!(second_run.run_ref, first_run.run_ref);
    assert_eq!(second_run.selected_sources, 2);
    let selected: Vec<Uuid> = sqlx::query_scalar(
        "SELECT derivation_ref FROM linggan_comment_research_run_item WHERE run_ref=$1",
    )
    .bind(second_run.run_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(selected.len(), 2);
    assert!(!selected.contains(&no_signal.derivation_ref));
    assert!(!selected.contains(&succeeded.derivation_ref));
    assert!(!selected.contains(&retryable.derivation_ref));
    assert!(selected.contains(&cancelled.derivation_ref));
    assert!(
        selected
            .iter()
            .any(|derivation_ref| *derivation_ref != cancelled.derivation_ref)
    );
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
            config_ref: Some(qualified_research_config(&database).await),
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
async fn run_read_exposes_safe_semantic_failure_counts_without_model_or_comment_text() {
    let database = fixture::proof_database("comment_research_run_failure_summary").await;
    detail_with_author(
        &database,
        "failure-summary-note",
        "SYNTHETIC failure summary note",
        Some("creator-1"),
    )
    .await;
    let json_source = comment_with_author(
        &database,
        "failure-summary-note",
        "json-failure",
        "孩子写作业总是拖延。SETTLEMENT_JSON_UNPARSEABLE",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let contract_source = comment_with_author(
        &database,
        "failure-summary-note",
        "contract-failure",
        "孩子一写应用题就不知道怎么开始。SETTLEMENT_CONTRACT_REJECTED",
        Some("reader-1"),
        "2026-09-01T08:00:01Z",
    )
    .await;
    let offset_source = comment_with_author(
        &database,
        "failure-summary-note",
        "offset-failure",
        "孩子总说自己不会做题。SETTLEMENT_OFFSET_UNMAPPABLE",
        Some("reader-1"),
        "2026-09-01T08:00:02Z",
    )
    .await;
    let quote_source = comment_with_author(
        &database,
        "failure-summary-note",
        "quote-valid",
        "孩子写作业总是拖延。SETTLEMENT_QUOTE_VALID",
        Some("reader-1"),
        "2026-09-01T08:00:03Z",
    )
    .await;
    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 100_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    reserve_research_model_semantic_dispatch_permit(&database, config_ref)
        .await
        .unwrap()
        .release()
        .await
        .unwrap();
    assert!(
        linggan_intelligence::local_embedding_profile::active(&database)
            .await
            .unwrap()
            .is_some()
    );
    corrupt_isolated_derivation_offsets(&database, offset_source).await;
    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from("/bin/sh"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_research_semantic_settlement_adapter.sh"),
    );
    let secrets = SyntheticModelSecrets;
    let drain = ModelWorkerDrain::new();
    for _ in 0..4 {
        assert!(
            run_once(&database, &secrets, &adapter, &drain)
                .await
                .unwrap()
        );
    }
    let item_receipts: Vec<(String, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT state,failure_code,invocation_ref FROM linggan_comment_research_run_item \
         WHERE run_ref=$1 ORDER BY derivation_ref",
    )
    .bind(run.run_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert!(
        item_receipts
            .iter()
            .all(|(_, _, invocation_ref)| invocation_ref.is_some()),
        "worker must reserve an invocation before semantic settlement: {item_receipts:?}"
    );

    let outcomes = sqlx::query(
        "SELECT derivation.source_ref,item.failure_code,invocation.result->>'failureStage' AS failure_stage, \
                NOT (invocation.result ? 'text') AS safe_result \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation derivation USING(derivation_ref) \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE item.run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        outcomes.len(),
        4,
        "each synthetic semantic settlement must retain one invocation receipt"
    );
    let outcome_for = |source_ref| {
        outcomes
            .iter()
            .find(|row| row.get::<Uuid, _>("source_ref") == source_ref)
            .unwrap()
    };
    let json_outcome = outcome_for(json_source);
    assert_eq!(
        json_outcome
            .get::<Option<String>, _>("failure_code")
            .as_deref(),
        Some("semantic_json_unparseable")
    );
    assert_eq!(
        json_outcome
            .get::<Option<String>, _>("failure_stage")
            .as_deref(),
        Some("semantic_json_parse")
    );
    let contract_outcome = outcome_for(contract_source);
    assert_eq!(
        contract_outcome
            .get::<Option<String>, _>("failure_code")
            .as_deref(),
        Some("semantic_contract_rejected")
    );
    assert_eq!(
        contract_outcome
            .get::<Option<String>, _>("failure_stage")
            .as_deref(),
        Some("semantic_contract_acceptance")
    );
    let quote_outcome = outcome_for(quote_source);
    assert!(
        quote_outcome
            .get::<Option<String>, _>("failure_code")
            .is_none()
    );
    assert!(
        quote_outcome
            .get::<Option<String>, _>("failure_stage")
            .is_none()
    );

    let offset_outcome = outcome_for(offset_source);
    assert_eq!(
        offset_outcome
            .get::<Option<String>, _>("failure_code")
            .as_deref(),
        Some("semantic_evidence_offset_unmappable")
    );
    assert_eq!(
        offset_outcome
            .get::<Option<String>, _>("failure_stage")
            .as_deref(),
        Some("semantic_evidence_offset_mapping")
    );
    assert!(outcomes.iter().all(|row| row.get::<bool, _>("safe_result")));

    let runs = read_runs(
        &database,
        &CommentResearchV1ReadQuery {
            result_revision_ref: None,
            limit: Some(20),
            offset: Some(0),
            problem_view: None,
        },
    )
    .await
    .unwrap();
    let item = runs["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["runRef"] == run.run_ref.to_string())
        .unwrap();
    assert_eq!(item["itemFailureCounts"]["semantic_json_unparseable"], 1);
    assert_eq!(item["itemFailureCounts"]["semantic_contract_rejected"], 1);
    assert_eq!(
        item["itemFailureCounts"]["semantic_evidence_offset_unmappable"],
        1
    );
    assert_eq!(item["modelExecution"]["callCount"], 4);
    assert_eq!(item["modelExecution"]["startedCallCount"], 4);
    assert_eq!(item["modelExecution"]["failedCallCount"], 3);
    assert_eq!(item["modelExecution"]["elapsedMs"], 4);
    assert_eq!(
        item["modelExecution"]["stageCounts"]["semantic_extraction"],
        4
    );
    assert_eq!(
        item["modelExecution"]["failureCounts"]["semantic_json_unparseable"],
        1
    );
    assert!(item["modelExecution"].get("invocationRef").is_none());
    assert!(item.get("modelResponse").is_none());
    assert!(item.get("commentText").is_none());
    let public_run_payload = runs.to_string();
    assert!(!public_run_payload.contains("SETTLEMENT_JSON_UNPARSEABLE"));
    assert!(!public_run_payload.contains("SETTLEMENT_CONTRACT_REJECTED"));
    assert!(!public_run_payload.contains("SETTLEMENT_OFFSET_UNMAPPABLE"));
    assert!(!public_run_payload.contains("SETTLEMENT_QUOTE_VALID"));
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
            config_ref: Some(qualified_research_config(&database).await),
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
            config_ref: Some(qualified_research_config(&database).await),
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
            atoms: vec![
                SemanticAtomProposal {
                    kind: AtomKind::Problem,
                    proposition: "孩子写作业时存在持续拖延".into(),
                    basis: AtomBasis::Explicit,
                    evidence_start: 0,
                    evidence_end: 5,
                },
                SemanticAtomProposal {
                    kind: AtomKind::Need,
                    proposition: " ".into(),
                    basis: AtomBasis::Explicit,
                    evidence_start: 0,
                    evidence_end: 5,
                },
            ],
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "succeeded");
    assert_eq!(receipt.accepted_atoms, 1);
    assert_eq!(receipt.rejected_atoms, 1);

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
async fn development_reset_removes_only_comment_research_derivatives() {
    let database = fixture::proof_database("comment_research_development_reset").await;
    detail_with_author(
        &database,
        "development-reset-note",
        "SYNTHETIC development reset note",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "development-reset-note",
        "development-reset-comment",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
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
                proposition: "孩子写作业持续拖延".into(),
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
    let space = synthetic_qualified_embedding_space(&database).await;
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at \
         ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','provider_timeout',$4,scope_001_now())",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution_execution( \
             atom_ref,run_ref,state,failure_code,finished_at \
         ) VALUES($1,$2,'model_failed','provider_timeout',scope_001_now())",
    )
    .bind(atom)
    .bind(run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let raw_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_comment WHERE material_ref=$1")
            .bind(source_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    let policy_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_policy_active WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let invocation_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(database.pool())
            .await
            .unwrap();
    let profile_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_embedding_profile WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();

    let receipt = reset_development_derived(&database).await.unwrap();
    assert!(receipt.deleted_derivations >= 1);
    assert!(receipt.deleted_runs >= 1);
    assert!(receipt.deleted_run_items >= 1);
    assert!(receipt.deleted_atoms >= 1);
    for (relation, statement) in [
        (
            "linggan_comment_research_derivation",
            "SELECT count(*) FROM linggan_comment_research_derivation",
        ),
        (
            "linggan_comment_research_run",
            "SELECT count(*) FROM linggan_comment_research_run",
        ),
        (
            "linggan_comment_research_run_item",
            "SELECT count(*) FROM linggan_comment_research_run_item",
        ),
        (
            "linggan_comment_research_atom",
            "SELECT count(*) FROM linggan_comment_research_atom",
        ),
        (
            "linggan_comment_research_atom_embedding",
            "SELECT count(*) FROM linggan_comment_research_atom_embedding",
        ),
        (
            "linggan_comment_research_problem",
            "SELECT count(*) FROM linggan_comment_research_problem",
        ),
        (
            "linggan_comment_research_problem_definition",
            "SELECT count(*) FROM linggan_comment_research_problem_definition",
        ),
        (
            "linggan_comment_research_problem_resolution",
            "SELECT count(*) FROM linggan_comment_research_problem_resolution",
        ),
        (
            "linggan_comment_research_problem_resolution_execution",
            "SELECT count(*) FROM linggan_comment_research_problem_resolution_execution",
        ),
        (
            "linggan_comment_research_result_revision",
            "SELECT count(*) FROM linggan_comment_research_result_revision",
        ),
        (
            "linggan_comment_research_problem_window_stat",
            "SELECT count(*) FROM linggan_comment_research_problem_window_stat",
        ),
        (
            "linggan_comment_research_change_observation",
            "SELECT count(*) FROM linggan_comment_research_change_observation",
        ),
        (
            "linggan_comment_research_embedding_space",
            "SELECT count(*) FROM linggan_comment_research_embedding_space",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(statement)
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{relation} should be regenerated after reset");
    }
    let raw_after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_comment WHERE material_ref=$1")
            .bind(source_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    let policy_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_policy_active WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let invocation_after: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(database.pool())
        .await
        .unwrap();
    let profile_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_embedding_profile WHERE singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(raw_after, raw_before);
    assert_eq!(policy_after, policy_before);
    assert_eq!(invocation_after, invocation_before);
    assert_eq!(profile_after, profile_before);
    assert_eq!(derive_current_sources(&database, 10).await.unwrap(), 1);
    let preview = preview_run(&database).await.unwrap();
    assert_eq!(preview.selected_sources, 1);
    let after_reset_run = start_run(&database).await.unwrap();
    assert_eq!(after_reset_run.selected_sources, 1);
    assert_ne!(after_reset_run.run_ref, run.run_ref);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_stale_run_item_attempt_cannot_write_the_recovered_item() {
    let database = fixture::proof_database("comment_research_run_item_attempt_fence").await;
    detail_with_author(
        &database,
        "run-item-attempt-note",
        "SYNTHETIC run item attempt note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "run-item-attempt-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    start_ready_run(&database).await.unwrap();
    let stale = claim_next_run_item(&database).await.unwrap().unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_run_item SET attempts=attempts+1 \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(stale.run_ref)
    .bind(stale.derivation_ref)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(matches!(
        record_run_item_failure(
            &database,
            &stale,
            RunItemFailureClass::ModelFailed,
            "synthetic_stale_owner",
        )
        .await,
        Err(CommentResearchKernelError::ClaimLost)
    ));
    let current: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT state,attempts,failure_code FROM linggan_comment_research_run_item \
         WHERE run_ref=$1 AND derivation_ref=$2",
    )
    .bind(stale.run_ref)
    .bind(stale.derivation_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(current.0, "running");
    assert_eq!(current.1, stale.attempt + 1);
    assert!(current.2.is_none());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_expired_embedding_claim_is_retried_and_cannot_be_settled_by_its_old_owner() {
    let database = fixture::proof_database("comment_research_embedding_claim_recovery").await;
    detail_with_author(
        &database,
        "embedding-claim-note",
        "SYNTHETIC embedding claim note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "embedding-claim-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let semantic_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &semantic_claim,
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
    let atom = atom_ref(&database, run.run_ref, semantic_claim.derivation_ref).await;
    let space = synthetic_qualified_embedding_space(&database).await;
    let input = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    let first = claim_next_embedding_work(&database).await.unwrap().unwrap();
    assert_eq!(first.attempt, 1);
    // Model the durable state immediately after the worker has atomically reserved and attached
    // the local embedding invocation, then dies before it can receive a provider response.
    let (connection_version_ref, model_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT entry.connection_version_ref,profile.model_ref \
         FROM linggan_comment_research_embedding_profile profile \
         JOIN linggan_model_entry entry USING(model_ref) WHERE profile.singleton",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let invocation_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
             invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,'embed',$4,'running',64,64,$5)",
    )
    .bind(invocation_ref)
    .bind(connection_version_ref)
    .bind(model_ref)
    .bind(HASH)
    .bind(json!({"runRef":run.run_ref,"stage":"embedding","callStarted":false}))
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding SET invocation_ref=$4 \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND attempts=$5 AND state='running'",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .bind(invocation_ref)
    .bind(first.attempt)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET lease_until=scope_001_now()-interval '1 second' \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(recover_expired_embedding_work(&database).await.unwrap(), 1);
    let invocation_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_model_invocation WHERE invocation_ref=$1")
            .bind(invocation_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        invocation_state, "failed",
        "lease recovery must terminalize the atomically attached provider reservation"
    );
    let recovered: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT state,attempts,next_attempt_at::text FROM linggan_comment_research_atom_embedding \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(recovered.0, "retryable");
    assert_eq!(recovered.1, 1);
    assert!(recovered.2.is_some());
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET next_attempt_at=scope_001_now()-interval '1 second' \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await
    .unwrap();
    let second = claim_next_embedding_work(&database).await.unwrap().unwrap();
    assert_eq!(second.attempt, 2);
    assert!(matches!(
        accept_atom_embedding(
            &database,
            &first,
            AtomEmbeddingResult {
                atom_ref: atom,
                space_ref: space.space_ref,
                input_hash: input.input_hash.clone(),
                values: unit_vector_512(),
                invocation_ref: None,
            },
        )
        .await,
        Err(CommentResearchEmbeddingError::WorkUnavailable)
    ));
    record_embedding_failure(&database, &second, None, "synthetic_retryable_failure")
        .await
        .unwrap();
    let state: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT state,attempts,lease_until::text FROM linggan_comment_research_atom_embedding \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state.0, "retryable");
    assert_eq!(state.1, 2);
    assert!(state.2.is_none());
}

/// **认领了却写不下到期时间的行，也必须被回收。**
///
/// `lease_until` 是可空列：在它存在之前就已经被认领、停在 `running` 的行没有这个值。而恢复
/// 判据若只写 `lease_until<=scope_001_now()`，NULL 参与比较得 NULL、不为真，这些行就恰好落在
/// 两个判据之间：认领只挑 `pending`/`retryable`，回收只挑「已过期的 running」——它既不再被
/// 认领，也永不被回收，所属 run 永远不完成，而且**一声不吭**。
///
/// 这条钉住的是「没有任何到期时间的 running 行按已过期处理」，而不是某一次迁移有没有
/// 把存量补齐：迁移只在它自己那一刻生效，之后新增的行仍然要靠判据本身兜住。
#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_embedding_claim_without_a_lease_deadline_is_still_recovered() {
    let database = fixture::proof_database("comment_research_embedding_null_lease").await;
    detail_with_author(
        &database,
        "embedding-null-lease-note",
        "SYNTHETIC embedding null lease note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "embedding-null-lease-note",
        "reader",
        "孩子写作业总拖延，有什么办法吗",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let semantic_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &semantic_claim,
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
    let atom = atom_ref(&database, run.run_ref, semantic_claim.derivation_ref).await;
    let space = synthetic_qualified_embedding_space(&database).await;
    let input = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    let first = claim_next_embedding_work(&database).await.unwrap().unwrap();
    assert_eq!(first.attempt, 1);
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding SET lease_until=NULL \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        recover_expired_embedding_work(&database).await.unwrap(),
        1,
        "没有到期时间的认领行不能落在认领与回收之间的缝里"
    );
    let recovered: (String, i32, Option<String>) = sqlx::query_as(
        "SELECT state,attempts,next_attempt_at::text FROM linggan_comment_research_atom_embedding \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(recovered.0, "retryable");
    assert_eq!(recovered.1, 1);
    assert!(recovered.2.is_some());
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET next_attempt_at=scope_001_now()-interval '1 second' \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await
    .unwrap();
    let second = claim_next_embedding_work(&database).await.unwrap().unwrap();
    assert_eq!(second.attempt, 2, "回收之后必须真的能再被认领一次");
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
            config_ref: Some(qualified_research_config(&database).await),
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
    sqlx::query(
        "UPDATE linggan_comment_research_embedding_profile SET enabled=false WHERE singleton",
    )
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
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_embedding_profile SET enabled=false WHERE singleton",
    )
    .execute(database.pool())
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
    sqlx::query(
        "UPDATE linggan_comment_research_embedding_profile SET enabled=false WHERE singleton",
    )
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
                config_ref: Some(qualified_research_config(&database).await),
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
            let embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
            accept_atom_embedding(
                &database,
                &embedding_claim,
                AtomEmbeddingResult {
                    atom_ref: atom,
                    space_ref: space.space_ref,
                    input_hash: input.input_hash,
                    values: unit_vector_512(),
                    invocation_ref: None,
                },
            )
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO linggan_comment_research_problem_resolution( \
                     atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at \
                 ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','synthetic_resolution_failure',$4,scope_001_now())",
            )
            .bind(atom)
            .bind(space.space_ref)
            .bind(HASH)
            .bind(run.run_ref)
            .execute(database.pool())
            .await
            .unwrap();
        } else {
            let claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
            sqlx::query(
                "UPDATE linggan_comment_research_atom_embedding SET attempts=3 \
                 WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3",
            )
            .bind(claim.input.atom_ref)
            .bind(claim.input.space_ref)
            .bind(&claim.input.input_hash)
            .execute(database.pool())
            .await
            .unwrap();
            let exhausted_claim =
                linggan_intelligence::comment_research_embeddings::EmbeddingWorkClaim {
                    attempt: 3,
                    ..claim
                };
            record_embedding_failure(
                &database,
                &exhausted_claim,
                None,
                "synthetic_embedding_failure",
            )
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
        if resolution_failure {
            assert!(matches!(
                publish_result_revision(&database, run.run_ref).await,
                Err(linggan_intelligence::comment_research_results::CommentResearchResultError::InsufficientPublicationCoverage)
            ));
        }
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn problem_read_keeps_a_deferred_signal_out_of_confirmed_problem_counts() {
    let database = fixture::proof_database("comment_research_v2_deferred_problem_read").await;
    detail_with_author(
        &database,
        "v2-deferred-read-note",
        "SYNTHETIC deferred read note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "v2-deferred-read-note",
        "deferred-reader",
        "孩子每天写作业都要催，不催就不开始",
        Some("deferred-reader"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
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
                proposition: "孩子在家庭作业中难以自主启动".into(),
                basis: AtomBasis::Explicit,
                evidence_start: 0,
                evidence_end: 7,
            }],
        },
        None,
    )
    .await
    .unwrap();
    let atom = atom_ref(&database, run.run_ref, claim.derivation_ref).await;
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at, \
             decision_kind,decision_payload,recheck_conditions,resolution_input_hash,catalog_revision_at_recall \
         ) VALUES($1,$2,'[]'::jsonb,$3,'succeeded','deferred_novel',$4,scope_001_now(), \
                  'deferred_novel',$5,'[\"independent_same_frame_signal\"]'::jsonb,$3,0)",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(run.run_ref)
    .bind(json!({"decision":"deferred_novel","comparisons":[]}))
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution_execution( \
             atom_ref,run_ref,state,attempts,failure_code,decision_kind,decision_payload,recheck_conditions,finished_at \
         ) VALUES($1,$2,'succeeded',1,'deferred_novel','deferred_novel',$3, \
                  '[\"independent_same_frame_signal\"]'::jsonb,scope_001_now())",
    )
    .bind(atom)
    .bind(run.run_ref)
    .bind(json!({"decision":"deferred_novel","comparisons":[]}))
    .execute(database.pool())
    .await
    .unwrap();

    let all = read_problems(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    assert_eq!(all["cumulative"]["problemCount"], 0);
    assert_eq!(all["facets"]["confirmed"], 0);
    assert_eq!(all["facets"]["deferred"], 1);
    assert_eq!(all["page"]["total"], 1);
    assert_eq!(all["page"]["items"][0]["itemKind"], "deferred");
    assert_eq!(all["page"]["items"][0]["decisionKind"], "deferred_novel");
    for restricted_key in [
        "atomRef",
        "sourceRef",
        "workRef",
        "originalVoice",
        "researchText",
    ] {
        assert!(
            all["page"]["items"][0].get(restricted_key).is_none(),
            "the ordinary Problem read must not expose {restricted_key} from restricted comment material"
        );
    }
    assert!(
        all["page"]["items"][0]["executionHistory"][0]
            .get("runRef")
            .is_none(),
        "the ordinary Problem read must not expose internal execution identifiers"
    );
    assert_eq!(
        all["page"]["items"][0]["candidateSnapshot"],
        json!([]),
        "the frozen empty catalog stays explicit rather than becoming an implicit create"
    );
    assert_eq!(
        all["page"]["items"][0]["executionHistory"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let confirmed = read_problems(
        &database,
        &CommentResearchV1ReadQuery {
            problem_view: Some(ProblemReadView::Confirmed),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed["page"]["total"], 0);
    let deferred = read_problems(
        &database,
        &CommentResearchV1ReadQuery {
            problem_view: Some(ProblemReadView::Deferred),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(deferred["page"]["total"], 1);
    assert_eq!(deferred["page"]["items"][0]["itemKind"], "deferred");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn v2_pair_admission_requires_independent_deferred_signals_and_creates_two_memberships() {
    let database = fixture::proof_database("comment_research_v2_pair_admission").await;
    detail_with_author(
        &database,
        "v2-pair-note",
        "SYNTHETIC V2 pair admission note",
        Some("creator-1"),
    )
    .await;
    for (id, author, body) in [
        ("first", "parent-a", "孩子每天写作业都要催，不催就不开始"),
        ("second", "parent-b", "孩子面对家庭作业总拖着，难以自己开始"),
        ("same-author-a", "parent-c", "孩子写作业前一直拖延"),
        ("same-author-b", "parent-c", "孩子做作业总要家长反复催促"),
        ("out-of-scope", "parent-e", "孩子写家庭作业时总是拖着不开始"),
        ("missing-frame", "parent-d", "孩子写家庭作业时总是拖着不开始"),
    ] {
        comment_with_author(
            &database,
            "v2-pair-note",
            id,
            body,
            Some(author),
            "2026-09-01T08:00:00Z",
        )
        .await;
    }
    let space = synthetic_qualified_embedding_space(&database).await;
    let config_ref = qualified_research_config(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(config_ref),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();
    let mut atoms = Vec::new();
    for ordinal in 0..6 {
        let claim = claim_next_run_item(&database).await.unwrap().unwrap();
        if ordinal == 5 {
            accept_semantic_output(
                &database,
                &claim,
                SemanticExtractionOutput::Atoms {
                    atoms: vec![SemanticAtomProposal {
                        kind: AtomKind::Problem,
                        proposition: "孩子在家庭作业中难以自主启动".into(),
                        basis: AtomBasis::Explicit,
                        evidence_start: 0,
                        evidence_end: 7,
                    }],
                },
                None,
            )
            .await
            .unwrap();
        } else {
            let problem_frame = if ordinal == 4 {
                ProblemFrameProposal {
                    scope_relation: linggan_intelligence::comment_research_problem_resolution_v2::ScopeRelation::OutOfScope,
                    ..valid_v2_problem_frame()
                }
            } else {
                valid_v2_problem_frame()
            };
            accept_semantic_quote_output(
                &database,
                &claim,
                SemanticQuoteExtractionOutput::Atoms {
                    atoms: vec![SemanticQuoteAtomProposal {
                        kind: AtomKind::Problem,
                        proposition: "孩子在家庭作业中难以自主启动".into(),
                        basis: AtomBasis::Explicit,
                        evidence: "孩子".into(),
                        problem_frame: Some(problem_frame),
                    }],
                },
                None,
            )
            .await
            .unwrap();
        }
        atoms.push(atom_ref(&database, run.run_ref, claim.derivation_ref).await);
    }
    let initial_revision: i64 = sqlx::query_scalar(
        "SELECT guard.revision FROM linggan_comment_research_problem_catalog_guard guard \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.problem_scope_domain_ref=guard.scope_domain_ref \
         JOIN linggan_comment_research_run run ON run.policy_revision_ref=policy.policy_revision_ref \
         WHERE run.run_ref=$1",
    )
    .bind(run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    for atom in &atoms {
        sqlx::query(
            "INSERT INTO linggan_comment_research_problem_resolution( \
                 atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at, \
                 decision_kind,decision_payload,recheck_conditions,resolution_input_hash,catalog_revision_at_recall \
             ) VALUES($1,$2,'[]'::jsonb,$3,'succeeded','deferred_novel',$4,scope_001_now(), \
                      'deferred_novel',$5,'[\"independent_same_frame_signal\"]'::jsonb,$3,$6)",
        )
        .bind(*atom)
        .bind(space.space_ref)
        .bind(HASH)
        .bind(run.run_ref)
        .bind(json!({"decision":"deferred_novel","comparisons":[]}))
        .bind(initial_revision)
        .execute(database.pool())
        .await
        .unwrap();
    }
    let (connection_version_ref, model_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT entry.connection_version_ref,config.model_ref \
         FROM linggan_model_config config JOIN linggan_model_entry entry USING(model_ref) \
         WHERE config.config_ref=$1",
    )
    .bind(config_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let invocation_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
             invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state, \
             reserved_tokens,charged_tokens,result,finished_at \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'succeeded',0,0,$6,scope_001_now())",
    )
    .bind(invocation_ref)
    .bind(connection_version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(HASH)
    .bind(json!({"ok":true,"stage":"problem_pair_resolution"}))
    .execute(database.pool())
    .await
    .unwrap();
    let definition = StableProblemDefinitionProposal {
        name: "家庭作业自主启动困难".into(),
        meaning: "孩子在家庭作业场景难以在没有外部催促时自行开始行动".into(),
        include: vec!["需要反复催促才开始写作业".into()],
        exclude: vec!["只是不愿意完成某一道具体题目".into()],
    };
    assert!(matches!(
        admit_new_problem(
            &database,
            NewProblemAdmission {
                atom_ref: atoms[0],
                definition: ProblemDefinitionProposal {
                    name: "不得单 Atom 新建".into(),
                    meaning: "V2 范围内的 Atom 必须经过双独立证据建题入口".into(),
                },
                basis: ProblemMembershipBasis::Deterministic,
                decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
                invocation_ref: None,
            },
        )
        .await,
        Err(CommentResearchProblemError::PairRequiredForV2)
    ));
    let created = admit_new_problem_pair(
        &database,
        NewProblemPairAdmission {
            first_atom_ref: atoms[0],
            second_atom_ref: atoms[1],
            expected_catalog_revision: initial_revision,
            definition: definition.clone(),
            decision_evidence: json!({"decision":"v2_pair_equivalent","pair":{"synthetic":true}}),
            invocation_ref: Some(invocation_ref),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.catalog_revision, initial_revision + 1);
    let memberships: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_atom_problem_membership \
         WHERE problem_ref=$1 AND current",
    )
    .bind(created.problem_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(memberships, 2);
    assert!(matches!(
        admit_new_problem_pair(
            &database,
            NewProblemPairAdmission {
                first_atom_ref: atoms[2],
                second_atom_ref: atoms[3],
                expected_catalog_revision: initial_revision,
                definition,
                decision_evidence: json!({"decision":"v2_pair_equivalent","pair":{"synthetic":true}}),
                invocation_ref: Some(invocation_ref),
            },
        )
        .await,
        Err(CommentResearchProblemError::PairSourcesNotIndependent)
    ));
    assert!(matches!(
        admit_new_problem_pair(
            &database,
            NewProblemPairAdmission {
                first_atom_ref: atoms[2],
                second_atom_ref: atoms[4],
                expected_catalog_revision: initial_revision,
                definition: StableProblemDefinitionProposal {
                    name: "域外 frame 不得建题".into(),
                    meaning: "out_of_scope Atom 即使被错误标记为 deferred 也必须拒绝 pair admission".into(),
                    include: vec!["测试".into()],
                    exclude: vec!["测试".into()],
                },
                decision_evidence: json!({"decision":"v2_pair_equivalent","pair":{"synthetic":true}}),
                invocation_ref: Some(invocation_ref),
            },
        )
        .await,
        Err(CommentResearchProblemError::PairNotEligible)
    ));
    assert!(matches!(
        admit_new_problem_pair(
            &database,
            NewProblemPairAdmission {
                first_atom_ref: atoms[2],
                second_atom_ref: atoms[5],
                expected_catalog_revision: initial_revision,
                definition: StableProblemDefinitionProposal {
                    name: "不应由缺少 frame 的 Atom 创建".into(),
                    meaning: "缺少 V2 Problem Frame 时必须拒绝 pair admission".into(),
                    include: vec!["测试".into()],
                    exclude: vec!["测试".into()],
                },
                decision_evidence: json!({"decision":"v2_pair_equivalent","pair":{"synthetic":true}}),
                invocation_ref: Some(invocation_ref),
            },
        )
        .await,
        Err(CommentResearchProblemError::PairNotEligible)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn confirmed_memberships_remain_readable_when_their_run_cannot_publish_statistics() {
    let database = fixture::proof_database("comment_research_cumulative_low_coverage").await;
    detail_with_author(
        &database,
        "cumulative-low-coverage-note",
        "SYNTHETIC cumulative low coverage note",
        Some("creator-1"),
    )
    .await;
    for ordinal in 0..26 {
        comment_with_author(
            &database,
            "cumulative-low-coverage-note",
            &format!("reader-{ordinal}"),
            &format!("孩子写作业前总是拖延，第 {ordinal} 次不知道怎么开始"),
            Some(&format!("reader-{ordinal}")),
            "2026-09-01T08:00:00Z",
        )
        .await;
    }
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 30,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let run = start_ready_run(&database).await.unwrap();

    for ordinal in 0..26 {
        let claim = claim_next_run_item(&database).await.unwrap().unwrap();
        accept_semantic_output(
            &database,
            &claim,
            SemanticExtractionOutput::Atoms {
                atoms: vec![SemanticAtomProposal {
                    kind: AtomKind::Problem,
                    proposition: format!("孩子难以启动第 {ordinal} 次写作业"),
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
        if ordinal < 2 {
            admit_legacy_new_problem(
                &database,
                NewProblemAdmission {
                    atom_ref: atom,
                    definition: ProblemDefinitionProposal {
                        name: format!("第 {ordinal} 类写作业启动困难"),
                        meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
                    },
                    basis: ProblemMembershipBasis::Deterministic,
                    decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
                    invocation_ref: None,
                },
            )
            .await
            .unwrap();
        } else {
            sqlx::query(
                "INSERT INTO linggan_comment_research_problem_resolution( \
                     atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at \
                 ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','synthetic_resolution_failure',$4,scope_001_now())",
            )
            .bind(atom)
            .bind(space.space_ref)
            .bind(HASH)
            .bind(run.run_ref)
            .execute(database.pool())
            .await
            .unwrap();
            refresh_run_completion_for_atom(&database, atom)
                .await
                .unwrap();
        }
    }

    assert!(matches!(
        publish_result_revision(&database, run.run_ref).await,
        Err(linggan_intelligence::comment_research_results::CommentResearchResultError::InsufficientPublicationCoverage)
    ));

    let overview = read_overview(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    assert_eq!(overview["cumulative"]["problemCount"], 2);
    assert_eq!(overview["cumulative"]["confirmedAtomCount"], 2);
    assert_eq!(overview["cumulative"]["confirmedCommentCount"], 2);
    assert_eq!(overview["currentProblems"].as_array().unwrap().len(), 2);
    assert!(overview["statisticsResult"].is_null());

    let problems = read_problems(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    assert_eq!(problems["page"]["total"], 2);
    let names: Vec<&str> = problems["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"第 0 类写作业启动困难"));
    assert!(names.contains(&"第 1 类写作业启动困难"));
    assert!(
        problems["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|item| item["confirmedAtomCount"] == 1)
    );
    assert!(problems["statisticsResult"].is_null());

    let runs = read_runs(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    let health = &runs["page"]["items"][0]["researchHealth"];
    assert_eq!(health["researchSignalCount"], 26);
    assert_eq!(health["problemBearingAtomCount"], 26);
    assert_eq!(health["organizedProblemAtomCount"], 2);
    assert_eq!(health["backlogProblemAtomCount"], 24);
    assert_eq!(health["organizationCoverage"]["numerator"], 2);
    assert_eq!(health["organizationCoverage"]["denominator"], 26);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_later_run_boundedly_continues_an_eligible_historical_resolution() {
    let database = fixture::proof_database("comment_research_cross_run_resolution_retry").await;
    detail_with_author(
        &database,
        "cross-run-resolution-note",
        "SYNTHETIC cross-run resolution note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "cross-run-resolution-note",
        "cross-run-resolution-comment",
        "孩子写作业前总是拖延，不知道怎么开始",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let source_run = start_ready_run(&database).await.unwrap();
    let semantic_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &semantic_claim,
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
    let atom = atom_ref(&database, source_run.run_ref, semantic_claim.derivation_ref).await;
    let embedding = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    let embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    accept_atom_embedding(
        &database,
        &embedding_claim,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: embedding.input_hash,
            values: unit_vector_512(),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,attempts,failure_code,execution_run_ref,finished_at \
         ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed',1,'problem_resolution_admission_rejected',$4,scope_001_now())",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(source_run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    refresh_run_completion_for_atom(&database, atom)
        .await
        .unwrap();

    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution \
         SET failure_code='problem_resolution_json_schema_rejected' WHERE atom_ref=$1",
    )
    .bind(atom)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        preview_run(&database).await.unwrap().eligible_backlog_atoms,
        0,
        "a schema failure is not retried under the unchanged model and membership contract"
    );
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution \
         SET failure_code='problem_resolution_admission_rejected',attempts=3 WHERE atom_ref=$1",
    )
    .bind(atom)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        preview_run(&database).await.unwrap().eligible_backlog_atoms,
        0,
        "semantic admission ambiguity stops after its bounded attempts"
    );
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution SET attempts=1 WHERE atom_ref=$1",
    )
    .bind(atom)
    .execute(database.pool())
    .await
    .unwrap();
    let preview = preview_run(&database).await.unwrap();
    assert_eq!(preview.selected_sources, 0);
    assert_eq!(preview.eligible_backlog_atoms, 1);
    let retry_run = start_ready_run(&database).await.unwrap();
    assert_eq!(retry_run.selected_sources, 0);
    assert_eq!(retry_run.selected_backlog_atoms, 1);
    let retry_state: (String, i32, Uuid) = sqlx::query_as(
        "SELECT state,attempts,execution_run_ref \
         FROM linggan_comment_research_problem_resolution WHERE atom_ref=$1",
    )
    .bind(atom)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(retry_state, ("pending".into(), 1, retry_run.run_ref));

    // Simulate B exhausting a transient provider attempt. C must get a fresh execution history
    // row rather than overwriting B when it later completes the same Atom.
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution \
         SET state='model_failed',failure_code='provider_timeout',finished_at=scope_001_now(), \
             next_attempt_at=NULL,lease_until=NULL,updated_at=scope_001_now() WHERE atom_ref=$1",
    )
    .bind(atom)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution_execution \
         SET state='model_failed',failure_code='provider_timeout',finished_at=scope_001_now(), \
             next_attempt_at=NULL,updated_at=scope_001_now() WHERE atom_ref=$1 AND run_ref=$2",
    )
    .bind(atom)
    .bind(retry_run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    refresh_run_completion_for_atom(&database, atom)
        .await
        .unwrap();
    let follow_up_run = start_ready_run(&database).await.unwrap();
    assert_eq!(follow_up_run.selected_sources, 0);
    assert_eq!(follow_up_run.selected_backlog_atoms, 1);

    // The worker changes pending to running before it can accept a model decision. This test
    // does not dispatch a provider, so it records that exact lease state before exercising the
    // same guarded admission and recovery path.
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution \
         SET state='running',lease_until=scope_001_now()+interval '120 seconds' WHERE atom_ref=$1",
    )
    .bind(atom)
    .execute(database.pool())
    .await
    .unwrap();

    admit_legacy_new_problem(
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
    assert_eq!(
        recover_problem_resolution_leases(&database).await.unwrap(),
        1
    );
    let runs = read_runs(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    let items = runs["page"]["items"].as_array().unwrap();
    let follow_up = items
        .iter()
        .find(|item| item["runRef"] == follow_up_run.run_ref.to_string())
        .unwrap();
    let follow_up_health = &follow_up["researchHealth"];
    assert_eq!(follow_up_health["activatedBacklogAtomCount"], 1);
    assert_eq!(follow_up_health["activatedBacklogResolvedAtomCount"], 1);
    assert_eq!(follow_up_health["activatedBacklogPendingAtomCount"], 0);
    assert_eq!(follow_up_health["activatedBacklogFailedAtomCount"], 0);
    let failed_retry = items
        .iter()
        .find(|item| item["runRef"] == retry_run.run_ref.to_string())
        .unwrap();
    assert_eq!(
        failed_retry["researchHealth"]["activatedBacklogFailedAtomCount"], 1,
        "B remains a failed execution record after C resolves the same Atom"
    );
    assert!(
        publish_result_revision(&database, source_run.run_ref)
            .await
            .is_ok()
    );
    assert!(matches!(
        publish_result_revision(&database, follow_up_run.run_ref).await,
        Err(linggan_intelligence::comment_research_results::CommentResearchResultError::InsufficientPublicationCoverage)
    ));
    let retry_result_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_result_revision WHERE run_ref=$1",
    )
    .bind(follow_up_run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        retry_result_count, 0,
        "retry-only Runs never create a new statistical window"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn disabling_embedding_terminalizes_a_historical_resolution_under_its_retry_run() {
    let database = fixture::proof_database("comment_research_cross_run_embedding_disabled").await;
    detail_with_author(
        &database,
        "cross-run-embedding-note",
        "SYNTHETIC cross-run embedding note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "cross-run-embedding-note",
        "cross-run-embedding-comment",
        "孩子写作业前总是拖延，不知道怎么开始",
        Some("reader-1"),
        "2026-09-01T08:00:00Z",
    )
    .await;
    let space = synthetic_qualified_embedding_space(&database).await;
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 10,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let source_run = start_ready_run(&database).await.unwrap();
    let semantic_claim = claim_next_run_item(&database).await.unwrap().unwrap();
    accept_semantic_output(
        &database,
        &semantic_claim,
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
    let atom = atom_ref(&database, source_run.run_ref, semantic_claim.derivation_ref).await;
    let embedding = queue_atom_embedding(&database, atom, space.space_ref)
        .await
        .unwrap();
    let embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    accept_atom_embedding(
        &database,
        &embedding_claim,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: embedding.input_hash,
            values: unit_vector_512(),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at \
         ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','provider_timeout',$4,scope_001_now())",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(source_run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    refresh_run_completion_for_atom(&database, atom)
        .await
        .unwrap();
    let retry_run = start_ready_run(&database).await.unwrap();
    assert_eq!(retry_run.selected_backlog_atoms, 1);

    sqlx::query(
        "UPDATE linggan_comment_research_embedding_profile SET enabled=false WHERE singleton",
    )
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        fail_active_runs_without_embedding_config(&database)
            .await
            .unwrap()
            > 0
    );
    let resolution: (String, String, Uuid) = sqlx::query_as(
        "SELECT state,failure_code,execution_run_ref \
         FROM linggan_comment_research_problem_resolution WHERE atom_ref=$1",
    )
    .bind(atom)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        resolution,
        (
            "incompatible".into(),
            "embedding_configuration_unavailable".into(),
            retry_run.run_ref
        )
    );
    let execution_history_state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_research_problem_resolution_execution \
         WHERE atom_ref=$1 AND run_ref=$2",
    )
    .bind(atom)
    .bind(retry_run.run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(execution_history_state, "incompatible");
    let retry_state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_research_run WHERE run_ref=$1")
            .bind(retry_run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(retry_state, "completed_with_failures");
    let runs = read_runs(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    let retry_health = runs["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["runRef"] == retry_run.run_ref.to_string())
        .unwrap()["researchHealth"]
        .clone();
    assert_eq!(retry_health["activatedBacklogPendingAtomCount"], 0);
    assert_eq!(retry_health["activatedBacklogFailedAtomCount"], 1);
    let invocation_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE result->>'runRef'=$1",
    )
    .bind(retry_run.run_ref.to_string())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(invocation_count, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn sufficiently_covered_terminal_run_publishes_a_partial_result_without_erasing_failures() {
    let database = fixture::proof_database("comment_research_partial_publication").await;
    detail_with_author(
        &database,
        "partial-publication-note",
        "SYNTHETIC partial publication note",
        Some("creator-1"),
    )
    .await;
    for ordinal in 0..10 {
        comment_with_author(
            &database,
            "partial-publication-note",
            &format!("reader-{ordinal}"),
            &format!("孩子写作业总拖延，第 {ordinal} 次想知道怎么办"),
            Some(&format!("reader-{ordinal}")),
            "2026-09-01T08:00:00Z",
        )
        .await;
    }
    save_active_policy(
        &database,
        SaveResearchPolicy {
            config_ref: Some(qualified_research_config(&database).await),
            source_limit: 20,
            token_limit: 10_000,
        },
    )
    .await
    .unwrap();
    let space = synthetic_qualified_embedding_space(&database).await;
    let run = start_ready_run(&database).await.unwrap();

    for ordinal in 0..10 {
        let claim = claim_next_run_item(&database).await.unwrap().unwrap();
        if ordinal == 9 {
            record_run_item_failure(
                &database,
                &claim,
                RunItemFailureClass::ModelFailed,
                "synthetic_semantic_schema_failure",
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
                    proposition: format!("孩子难以启动第 {ordinal} 次写作业"),
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
        if ordinal == 8 {
            sqlx::query(
                "INSERT INTO linggan_comment_research_problem_resolution( \
                     atom_ref,space_ref,candidate_set,candidate_hash,state,failure_code,execution_run_ref,finished_at \
                 ) VALUES($1,$2,'[]'::jsonb,$3,'model_failed','synthetic_resolution_failure',$4,scope_001_now())",
            )
            .bind(atom)
            .bind(space.space_ref)
            .bind(HASH)
            .bind(run.run_ref)
            .execute(database.pool())
            .await
            .unwrap();
            refresh_run_completion_for_atom(&database, atom)
                .await
                .unwrap();
            continue;
        }
        admit_legacy_new_problem(
            &database,
            NewProblemAdmission {
                atom_ref: atom,
                definition: ProblemDefinitionProposal {
                    name: format!("第 {ordinal} 次作业启动困难"),
                    meaning: "孩子在开始完成作业前持续拖延或难以行动".into(),
                },
                basis: ProblemMembershipBasis::Deterministic,
                decision_evidence: json!({"decision":"new_problem","candidateRefs":[]}),
                invocation_ref: None,
            },
        )
        .await
        .unwrap();
    }

    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_research_run WHERE run_ref=$1")
            .bind(run.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(state, "completed_with_failures");
    let publication = publish_result_revision(&database, run.run_ref)
        .await
        .unwrap();
    let input_counts: serde_json::Value = sqlx::query_scalar(
        "SELECT input_counts FROM linggan_comment_research_result_revision WHERE result_revision_ref=$1",
    )
    .bind(publication.result_revision_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(input_counts["publicationCoverage"], "partial");
    assert_eq!(input_counts["selectedCommentCount"], 10);
    assert_eq!(input_counts["includedCommentCount"], 9);
    assert_eq!(input_counts["excludedTerminalCommentCount"], 1);
    assert_eq!(input_counts["organizedProblemAtomCount"], 8);
    assert_eq!(input_counts["unorganizedProblemAtomCount"], 1);
    let runs = read_runs(&database, &CommentResearchV1ReadQuery::default())
        .await
        .unwrap();
    assert_eq!(runs["page"]["items"][0]["state"], "completed_with_failures");
    assert_eq!(
        runs["page"]["items"][0]["publishedResult"]["inputCounts"]["publicationCoverage"],
        "partial"
    );
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
            config_ref: Some(qualified_research_config(&database).await),
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
    let embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    accept_atom_embedding(
        &database,
        &embedding_claim,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: input.input_hash,
            values: unit_vector_512(),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,execution_run_ref,lease_until \
         ) VALUES($1,$2,'[]'::jsonb,$3,'running',$4,scope_001_now()+interval '120 seconds')",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();

    admit_legacy_new_problem(
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
            config_ref: Some(qualified_research_config(&database).await),
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
    let embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    accept_atom_embedding(
        &database,
        &embedding_claim,
        AtomEmbeddingResult {
            atom_ref: atom,
            space_ref: space.space_ref,
            input_hash: input.input_hash,
            values: unit_vector_512(),
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_resolution( \
             atom_ref,space_ref,candidate_set,candidate_hash,state,execution_run_ref \
         ) VALUES($1,$2,'[]'::jsonb,$3,'pending',$4)",
    )
    .bind(atom)
    .bind(space.space_ref)
    .bind(HASH)
    .bind(run.run_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_research_embedding_profile SET enabled=false WHERE singleton",
    )
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
            config_ref: Some(qualified_research_config(&database).await),
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
            config_ref: Some(qualified_research_config(&database).await),
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
    let created = admit_legacy_new_problem(
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
        admit_legacy_new_problem(
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
async fn exact_vector_recall_excludes_unscoped_legacy_definitions_and_invalid_vectors_never_write() {
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
            config_ref: Some(qualified_research_config(&database).await),
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
    let problem = admit_legacy_new_problem(
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
    let first_input = queue_atom_embedding(&database, first_atom, space.space_ref)
        .await
        .unwrap();
    let first_embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    let mut first_values = vec![0.0; 512];
    first_values[0] = 1.0;
    accept_atom_embedding(
        &database,
        &first_embedding_claim,
        AtomEmbeddingResult {
            atom_ref: first_atom,
            space_ref: space.space_ref,
            input_hash: first_input.input_hash,
            values: first_values,
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
    let second_embedding_claim = claim_next_embedding_work(&database).await.unwrap().unwrap();
    assert!(matches!(
        accept_atom_embedding(
            &database,
            &second_embedding_claim,
            AtomEmbeddingResult {
                atom_ref: second_atom,
                space_ref: space.space_ref,
                input_hash: atom_input.input_hash.clone(),
                values: vec![f64::NAN; 512],
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
    assert_eq!(atom_work_state, "running");
    accept_atom_embedding(
        &database,
        &second_embedding_claim,
        AtomEmbeddingResult {
            atom_ref: second_atom,
            space_ref: space.space_ref,
            input_hash: atom_input.input_hash,
            values: {
                let mut values = vec![0.0; 512];
                values[0] = 0.99;
                values[1] = 0.1;
                values
            },
            invocation_ref: None,
        },
    )
    .await
    .unwrap();
    let candidates = recall_problem_candidates(&database, second_atom, space.space_ref)
        .await
        .unwrap();
    assert!(
        candidates.is_empty(),
        "V2 retrieval must not offer a legacy definition without a stable identity as a merge candidate"
    );
    let stable_identity: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT stable_identity FROM linggan_comment_research_problem_definition WHERE problem_ref=$1",
    )
    .bind(problem.problem_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(stable_identity.is_none());
    let memberships: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_atom_problem_membership \
         WHERE atom_ref=$1 AND current",
    )
    .bind(second_atom)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(memberships, 0, "candidate recall must not assign a Problem");
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
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
            config_ref: Some(qualified_research_config(&database).await),
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
            let created = admit_legacy_new_problem(
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
        problem_view: None,
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
    assert_eq!(problems["page"]["items"][0]["confirmedAtomCount"], 3);
    assert_eq!(problems["page"]["items"][0]["confirmedCommentCount"], 3);
    assert_eq!(problems["page"]["items"][0]["confirmedWorkCount"], 2);

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
