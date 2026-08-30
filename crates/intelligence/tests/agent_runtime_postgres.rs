use linggan_intelligence::{
    AgentBudget, AgentInvocationState, AgentRuntime, AgentRuntimeError,
    CandidateAnalysisInvocation, CandidateAnalysisOutput, CandidateExplanation,
    DeterministicCandidateAdapter, ExecutionOutcome, FrozenTopicMaterial, FrozenTopicMaterialPack,
    GrantedAgentTool, ModelToolLoopError, ModelToolLoopPort, PostgresAgentRuntime,
    TopicMaterialMemberImport, TopicMaterialRole, TopicWorkspaceImport, import_topic_workspace,
    read_topic_workspace,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use uuid::Uuid;

const AGENT_PROOF_SCHEMA: &str = concat!(
    "CREATE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT clock_timestamp() $$;\n",
    "CREATE FUNCTION linggan_plugin_runtime_forbid_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'append-only relation'; END $$;\n",
    "CREATE TABLE linggan_material_content (public_ref uuid NOT NULL UNIQUE);\n",
    include_str!("../../../database/migrations/0027_topic_workspace.sql"),
    include_str!("../../../database/migrations/0028_agent_candidate_analysis.sql")
);

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("isolated proof database URL is supplied");
    isolated_proof_schema(&url, schema, AGENT_PROOF_SCHEMA)
        .await
        .expect("Agent proof schema applies")
}

async fn fixture_invocation(database: &Database, key: &str) -> CandidateAnalysisInvocation {
    let support_ref = Uuid::new_v4();
    let challenge_ref = Uuid::new_v4();
    for work_ref in [support_ref, challenge_ref] {
        sqlx::query("INSERT INTO linggan_material_content(public_ref) VALUES ($1)")
            .bind(work_ref)
            .execute(database.pool())
            .await
            .expect("fixture Work Resource identity is admitted");
    }
    let topic = TopicWorkspaceImport {
        idempotency_key: format!("topic-{key}"),
        domain_key: "adhd-family".to_owned(),
        canonical_key: format!("agent-fixture-{}", Uuid::new_v4().simple()),
        display_name: "任务启动困难".to_owned(),
        definition_text: "有明确意图时仍难以开始第一步。".to_owned(),
        expected_version: None,
        adjudication_note: "synthetic human adjudication".to_owned(),
        source_boundary: "two synthetic, explicitly de-identified fixtures".to_owned(),
        members: vec![
            TopicMaterialMemberImport {
                work_public_ref: support_ref,
                role: TopicMaterialRole::Support,
                rationale: "describes difficulty starting the first step".to_owned(),
            },
            TopicMaterialMemberImport {
                work_public_ref: challenge_ref,
                role: TopicMaterialRole::Challenge,
                rationale: "fatigue may explain this instance".to_owned(),
            },
        ],
    };
    let canonical_key = topic.canonical_key.clone();
    import_topic_workspace(database, &topic)
        .await
        .expect("fixture Topic imports");
    let workspace = read_topic_workspace(database, &canonical_key)
        .await
        .expect("Topic reads")
        .expect("Topic exists");
    CandidateAnalysisInvocation {
        idempotency_key: key.to_owned(),
        actor_ref: Uuid::new_v4(),
        delegation_revision: "local-research-owner:v1".to_owned(),
        purpose: "produce a bounded candidate explanation".to_owned(),
        profile_key: "topic-candidate-analysis".to_owned(),
        profile_version: 1,
        material_pack: FrozenTopicMaterialPack {
            topic_ref: workspace.topic_ref,
            definition_ref: workspace.definition.definition_ref,
            definition_version: workspace.definition.version,
            classification_run_ref: workspace.classification_run.classification_run_ref,
            material_pack_ref: workspace.material_pack.material_pack_ref,
            source_boundary: workspace.material_pack.source_boundary,
            members: workspace
                .members
                .into_iter()
                .map(|member| FrozenTopicMaterial {
                    work_public_ref: member.work_public_ref,
                    role: member.role,
                    rationale: member.rationale,
                    controlled_excerpt: "synthetic de-identified excerpt".to_owned(),
                })
                .collect(),
        },
        budget: AgentBudget {
            max_steps: 2,
            max_tool_calls: 1,
            max_output_candidates: 2,
        },
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn exact_pack_replays_and_persists_a_candidate_only_terminal_result() {
    let database = proof_database("agent_exact_pack").await;
    let invocation = fixture_invocation(&database, "agent-proof:exact-pack:0001").await;
    let runtime = PostgresAgentRuntime::new(&database, DeterministicCandidateAdapter);
    let accepted = runtime.submit(&invocation).await.expect("submit succeeds");
    assert_eq!(accepted.state, AgentInvocationState::Accepted);
    let replay = runtime
        .submit(&invocation)
        .await
        .expect("identical replay succeeds");
    assert_eq!(replay.invocation_ref, accepted.invocation_ref);

    let completed = runtime
        .execute(accepted.invocation_ref)
        .await
        .expect("execution succeeds");
    assert_eq!(completed.state, AgentInvocationState::Succeeded);
    let output = completed.output.expect("candidate output is frozen");
    assert_eq!(output.claim_ceiling, "candidate_only");
    assert!(!output.unknowns.is_empty());
    let receipt_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_agent_tool_receipt WHERE invocation_ref=$1",
    )
    .bind(accepted.invocation_ref)
    .fetch_one(database.pool())
    .await
    .expect("tool receipts read");
    assert_eq!(receipt_count, 1);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn idempotency_key_rejects_changed_frozen_input() {
    let database = proof_database("agent_idempotency_conflict").await;
    let invocation = fixture_invocation(&database, "agent-proof:idempotency:0001").await;
    let runtime = PostgresAgentRuntime::new(&database, DeterministicCandidateAdapter);
    runtime
        .submit(&invocation)
        .await
        .expect("first submit succeeds");
    let mut changed = invocation;
    changed.purpose.push_str(" changed");
    assert_eq!(
        runtime
            .submit(&changed)
            .await
            .expect_err("changed replay conflicts"),
        AgentRuntimeError::IdempotencyConflict
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn mismatched_pack_is_rejected_before_the_invocation_ledger_changes() {
    let database = proof_database("agent_pack_mismatch").await;
    let mut invocation = fixture_invocation(&database, "agent-proof:pack-mismatch:0001").await;
    invocation
        .material_pack
        .source_boundary
        .push_str(" changed");
    let runtime = PostgresAgentRuntime::new(&database, DeterministicCandidateAdapter);
    let error = runtime
        .submit(&invocation)
        .await
        .expect_err("changed frozen pack is rejected");
    assert_eq!(error, AgentRuntimeError::MaterialPackMismatch);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_agent_invocation")
        .fetch_one(database.pool())
        .await
        .expect("ledger count reads");
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn cancellation_and_interrupted_recovery_fail_closed() {
    let database = proof_database("agent_cancel_recover").await;
    let first = fixture_invocation(&database, "agent-proof:cancel:0001").await;
    let runtime = PostgresAgentRuntime::new(&database, DeterministicCandidateAdapter);
    let accepted = runtime.submit(&first).await.expect("submit succeeds");
    let cancelled = runtime
        .cancel(
            accepted.invocation_ref,
            first.actor_ref,
            "owner stopped the analysis",
        )
        .await
        .expect("accepted invocation cancels");
    assert_eq!(cancelled.state, AgentInvocationState::Cancelled);
    assert!(runtime.execute(accepted.invocation_ref).await.is_err());

    let second = fixture_invocation(&database, "agent-proof:recover:0001").await;
    let accepted = runtime
        .submit(&second)
        .await
        .expect("second submit succeeds");
    sqlx::query("UPDATE linggan_agent_invocation SET state='running' WHERE invocation_ref=$1")
        .bind(accepted.invocation_ref)
        .execute(database.pool())
        .await
        .expect("fixture simulates interrupted execution");
    let recovered = runtime
        .recover_interrupted(accepted.invocation_ref)
        .await
        .expect("interrupted execution closes explicitly");
    assert_eq!(recovered.state, AgentInvocationState::Failed);
    assert_eq!(
        recovered.failure_code.as_deref(),
        Some("interrupted_before_finalize")
    );
}

struct ForeignOutputAdapter;

impl ModelToolLoopPort for ForeignOutputAdapter {
    fn adapter_identity(&self) -> &'static str {
        "foreign-output-fixture"
    }

    fn run(
        &self,
        request: &linggan_intelligence::ExecutionRequest,
    ) -> Result<ExecutionOutcome, ModelToolLoopError> {
        Ok(ExecutionOutcome {
            tool_calls: vec![GrantedAgentTool::ReadTopicMaterialPack],
            output: CandidateAnalysisOutput {
                claim_ceiling: "candidate_only".to_owned(),
                observed_references: vec![request.material_pack.members[0].work_public_ref],
                candidate_explanations: vec![CandidateExplanation {
                    statement: "malformed citation".to_owned(),
                    supporting_refs: vec![Uuid::new_v4()],
                    challenging_refs: vec![request.material_pack.members[1].work_public_ref],
                    limitations: "synthetic".to_owned(),
                    falsification_condition: "contradiction".to_owned(),
                }],
                alternative_explanations: vec!["alternative".to_owned()],
                unknowns: vec!["unknown".to_owned()],
                information_gaps: vec!["gap".to_owned()],
                user_summary: "candidate only".to_owned(),
            },
        })
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn invalid_adapter_output_is_rejected_and_never_persisted_as_a_candidate() {
    let database = proof_database("agent_invalid_output").await;
    let invocation = fixture_invocation(&database, "agent-proof:invalid-output:0001").await;
    let runtime = PostgresAgentRuntime::new(&database, ForeignOutputAdapter);
    let accepted = runtime.submit(&invocation).await.expect("submit succeeds");
    runtime
        .execute(accepted.invocation_ref)
        .await
        .expect_err("foreign citation fails validation");
    let rejected = runtime
        .status(accepted.invocation_ref)
        .await
        .expect("authoritative terminal status remains readable");
    assert_eq!(rejected.state, AgentInvocationState::Rejected);
    assert!(rejected.output.is_none());
    let receipt_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_agent_tool_receipt WHERE invocation_ref=$1",
    )
    .bind(accepted.invocation_ref)
    .fetch_one(database.pool())
    .await
    .expect("the completed A1 read remains auditable");
    assert_eq!(receipt_count, 1);
}

struct ProviderFailureAdapter;

impl ModelToolLoopPort for ProviderFailureAdapter {
    fn adapter_identity(&self) -> &'static str {
        "provider-failure-fixture"
    }

    fn run(
        &self,
        _request: &linggan_intelligence::ExecutionRequest,
    ) -> Result<ExecutionOutcome, ModelToolLoopError> {
        Err(ModelToolLoopError::Provider(
            "synthetic provider outage".to_owned(),
        ))
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn provider_failure_closes_the_authoritative_invocation_without_a_candidate() {
    let database = proof_database("agent_provider_failure").await;
    let invocation = fixture_invocation(&database, "agent-proof:provider-failure:0001").await;
    let runtime = PostgresAgentRuntime::new(&database, ProviderFailureAdapter);
    let accepted = runtime.submit(&invocation).await.expect("submit succeeds");
    let failed = runtime
        .execute(accepted.invocation_ref)
        .await
        .expect("provider failure is represented as a terminal receipt");
    assert_eq!(failed.state, AgentInvocationState::Failed);
    assert_eq!(failed.failure_code.as_deref(), Some("adapter_failed"));
    assert!(failed.output.is_none());
    let receipt_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_agent_tool_receipt WHERE invocation_ref=$1",
    )
    .bind(accepted.invocation_ref)
    .fetch_one(database.pool())
    .await
    .expect("the completed A1 read remains auditable");
    assert_eq!(receipt_count, 1);
}
