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
    activate_configured_embedding_space, queue_atom_embedding, queue_problem_definition_embedding,
    recall_problem_candidates,
};
use linggan_intelligence::comment_research_kernel::{
    DERIVATION_VERSION, RunItemFailureClass, SaveResearchPolicy, claim_next_run_item,
    derive_current_sources, record_run_item_failure, save_active_policy, start_run,
};
use linggan_intelligence::comment_research_problems::{
    CommentResearchProblemError, ExistingProblemAdmission, NewProblemAdmission,
    ProblemDefinitionProposal, ProblemMembershipBasis, admit_existing_problem, admit_new_problem,
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
    let run = start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();
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
    assert_eq!(run_state, "completed");
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
    let run = start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();
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
    let run = start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();

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
    let run = start_run(&database, json!({"initiatedBy":"synthetic-test"}))
        .await
        .unwrap();
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
