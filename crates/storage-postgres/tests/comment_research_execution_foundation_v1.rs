use linggan_contracts::CapturePackageV0;
use linggan_domain::comment_research::{
    COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1, COMMENT_RESEARCH_EXECUTION_CONTRACT_V1,
    CommentResearchContextPackInputV1, ContextPackReadinessV1, ContextPackSourceTextV1,
    ResearchFingerprintInputV1, ResearchModelStrategyV1, build_comment_research_context_pack_v1,
    freeze_comment_research_context_pack_v1, research_fingerprint_v1,
};
use linggan_storage_postgres::{CommentFactStore, CommentProjectionDispositionV0};
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls, error::SqlState};
use uuid::Uuid;

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "comment-research-execution-foundation-v1-proof";

/// This proof requires the companion script's disposable PostgreSQL instance.
/// It has no configured runtime fallback and makes no provider, worker, queue,
/// browser, or external-source call.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-research-execution-foundation-v1.sh"]
async fn proves_immutable_research_input_foundation_against_isolated_postgres() {
    let database_url =
        std::env::var("LINGGAN_COMMENT_RESEARCH_EXECUTION_FOUNDATION_TEST_DATABASE_URL")
            .expect("proof script must provide isolated PostgreSQL URL");
    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated PostgreSQL connects");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact migration applies");
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration applies");
    store
        .apply_comment_research_execution_foundation_v1_migration()
        .await
        .expect("execution-foundation migration applies");
    let inspector = connect_inspector(&database_url).await;

    let (detail, mut comments) = fixture_packages();
    comments.records[0].payload["text"] = json!("我想知道具体方法");
    comments
        .extensions
        .insert("executionFoundation".to_owned(), json!("v1"));
    let admitted = store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
        .await
        .expect("current comment and immutable derivation admit");
    let observation_id = match admitted.comments[0].disposition {
        CommentProjectionDispositionV0::Created { observation_id }
        | CommentProjectionDispositionV0::Advanced { observation_id, .. } => observation_id,
        ref other => panic!("expected a fresh current observation, got {other:?}"),
    };
    let derivation_id = inspector
        .query_one(
            "SELECT id FROM comment_derivation_v1 WHERE comment_observation_id = $1",
            &[&observation_id],
        )
        .await
        .expect("derivation query")
        .get::<_, Uuid>(0);

    let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
        research_expression: "我想知道具体方法".to_owned(),
        readiness: ContextPackReadinessV1::Ready,
        has_source_backed_context: false,
        related_discussion: Vec::new(),
        work_title: ContextPackSourceTextV1::Unavailable,
        work_body: ContextPackSourceTextV1::Unavailable,
    });
    let frozen = freeze_comment_research_context_pack_v1(&pack);
    let fingerprint = research_fingerprint_v1(&ResearchFingerprintInputV1 {
        cleaned_research_text: "我想知道具体方法".to_owned(),
        frozen_context_pack: frozen.clone(),
        cleaning_contract: "comment-cleaning.v1".to_owned(),
        research_contract: COMMENT_RESEARCH_EXECUTION_CONTRACT_V1.to_owned(),
        output_schema: COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1.to_owned(),
        model_strategy: ResearchModelStrategyV1 {
            strategy_id: "contract-only".to_owned(),
            strategy_version: "v1".to_owned(),
        },
    })
    .expect("frozen V1 input fingerprints");

    let run_id = Uuid::new_v4();
    let run_item_id = Uuid::new_v4();
    inspector
        .execute(
            "INSERT INTO comment_research_run_v1 (id, workspace_id, execution_state) \
             VALUES ($1, $2, 'prepared')",
            &[&run_id, &WORKSPACE],
        )
        .await
        .expect("run snapshot inserts");
    inspector
        .execute(
            "INSERT INTO comment_research_run_item_v1 \
             (id, run_id, workspace_id, source_evidence_id, source_record_index, \
              comment_observation_id, comment_derivation_id, cleaned_research_text, \
              cleaning_contract, research_contract, output_schema, model_strategy_id, \
              model_strategy_version, research_fingerprint, context_pack_version, \
              context_pack_integrity_sha256, frozen_context_pack_text, \
              context_sufficiency_state, execution_state, initial_failure_code) \
             VALUES ($1, $2, $3, $4, 0, $5, $6, $7, 'comment-cleaning.v1', \
              'comment-research-execution.v1', 'comment-analysis-structured-output.v1', \
              'contract-only', 'v1', $8, 'comment-research-input-snapshot.v1', $9, $10, \
              'sufficient', 'prepared', NULL)",
            &[
                &run_item_id,
                &run_id,
                &WORKSPACE,
                &admitted.comments_evidence.id,
                &observation_id,
                &derivation_id,
                &"我想知道具体方法",
                &fingerprint,
                &frozen.integrity_sha256,
                &frozen.text,
            ],
        )
        .await
        .expect("only a source-backed immutable input snapshot inserts");

    let stored = inspector
        .query_one(
            "SELECT source_evidence_id, source_record_index, comment_observation_id, \
                    comment_derivation_id, research_fingerprint, context_pack_version, \
                    context_pack_integrity_sha256, frozen_context_pack_text, execution_state, \
                    output_validation_state, initial_failure_code \
               FROM comment_research_run_item_v1 WHERE id = $1",
            &[&run_item_id],
        )
        .await
        .expect("stored snapshot reads");
    assert_eq!(stored.get::<_, Uuid>(0), admitted.comments_evidence.id);
    assert_eq!(stored.get::<_, i32>(1), 0);
    assert_eq!(stored.get::<_, Uuid>(2), observation_id);
    assert_eq!(stored.get::<_, Uuid>(3), derivation_id);
    assert_eq!(stored.get::<_, String>(4), fingerprint);
    assert_eq!(stored.get::<_, String>(5), frozen.version);
    assert_eq!(stored.get::<_, String>(6), frozen.integrity_sha256);
    assert_eq!(stored.get::<_, String>(7), frozen.text);
    assert_eq!(stored.get::<_, String>(8), "prepared");
    assert_eq!(stored.get::<_, String>(9), "not_submitted");
    assert_eq!(stored.get::<_, Option<String>>(10), None);

    let mutation = inspector
        .execute(
            "UPDATE comment_research_run_item_v1 SET frozen_context_pack_text = 'tampered' \
             WHERE id = $1",
            &[&run_item_id],
        )
        .await
        .expect_err("frozen research inputs must be append-only");
    assert_eq!(
        mutation
            .as_db_error()
            .expect("append-only trigger error")
            .code(),
        &SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE
    );

    // The comment conclusion is stored separately from operational execution
    // state. This is a no-signal result, not a fake `success` run state.
    inspector
        .execute(
            "INSERT INTO comment_analysis_v1 \
             (id, run_item_id, comment_observation_id, comment_derivation_id, \
              research_fingerprint, conclusion_state, structured_output) \
             VALUES ($1, $2, $3, $4, $5, 'no_signal', '{\"outcome\":\"no_signal\"}'::jsonb)",
            &[
                &Uuid::new_v4(),
                &run_item_id,
                &observation_id,
                &derivation_id,
                &fingerprint,
            ],
        )
        .await
        .expect("no-signal conclusion is distinct from run state");
    let conclusion = inspector
        .query_one(
            "SELECT conclusion_state FROM comment_analysis_v1 WHERE run_item_id = $1",
            &[&run_item_id],
        )
        .await
        .expect("conclusion reads")
        .get::<_, String>(0);
    assert_eq!(conclusion, "no_signal");

    let mixed_state = inspector
        .execute(
            "INSERT INTO comment_research_run_item_event_v1 \
             (id, run_item_id, execution_state, output_validation_state) \
             VALUES ($1, $2, 'success', 'accepted')",
            &[&Uuid::new_v4(), &run_item_id],
        )
        .await
        .expect_err("comment conclusion words must not be run execution states");
    assert_eq!(
        mixed_state.as_db_error().expect("state check error").code(),
        &SqlState::CHECK_VIOLATION
    );
}

fn fixture_packages() -> (CapturePackageV0, CapturePackageV0) {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    (
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail package"),
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package"),
    )
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector connects");
    tokio::spawn(async move {
        connection.await.expect("inspector stays connected");
    });
    client
}
