use linggan_intelligence::{
    AgentBudget, CandidateAnalysisInvocation, CandidateAnalysisOutput, CandidateExplanation,
    DeterministicCandidateAdapter, ExecutionOutcome, FrozenTopicMaterial, FrozenTopicMaterialPack,
    GrantedAgentTool, ModelToolLoopError, ModelToolLoopPort, PI_AGENT_CORE_VERSION,
    PiCandidateAdapter, PiLoopTransport, TopicMaterialRole, validate_candidate_output,
};
use uuid::Uuid;

fn frozen_pack() -> FrozenTopicMaterialPack {
    FrozenTopicMaterialPack {
        topic_ref: Uuid::new_v4(),
        definition_ref: Uuid::new_v4(),
        definition_version: 1,
        classification_run_ref: Uuid::new_v4(),
        material_pack_ref: Uuid::new_v4(),
        source_boundary: "two synthetic, explicitly de-identified fixtures".to_owned(),
        members: vec![
            FrozenTopicMaterial {
                work_public_ref: Uuid::new_v4(),
                role: TopicMaterialRole::Support,
                rationale: "describes difficulty starting the first step".to_owned(),
                controlled_excerpt: "I know the next step but remain unable to begin.".to_owned(),
            },
            FrozenTopicMaterial {
                work_public_ref: Uuid::new_v4(),
                role: TopicMaterialRole::Challenge,
                rationale: "fatigue may explain this instance".to_owned(),
                controlled_excerpt: "This only happens for me after severe sleep loss.".to_owned(),
            },
        ],
    }
}

fn invocation() -> CandidateAnalysisInvocation {
    CandidateAnalysisInvocation {
        idempotency_key: "agent-contract:synthetic:0001".to_owned(),
        actor_ref: Uuid::new_v4(),
        delegation_revision: "local-research-owner:v1".to_owned(),
        purpose: "find a bounded candidate explanation and its counterweight".to_owned(),
        profile_key: "topic-candidate-analysis".to_owned(),
        profile_version: 1,
        material_pack: frozen_pack(),
        budget: AgentBudget {
            max_steps: 2,
            max_tool_calls: 1,
            max_output_candidates: 2,
        },
    }
}

#[test]
fn deterministic_adapter_uses_only_the_frozen_pack_contract() {
    let request = invocation().execution_request().expect("valid invocation");
    let outcome = DeterministicCandidateAdapter
        .run(&request)
        .expect("deterministic execution succeeds");
    assert_eq!(
        outcome.tool_calls,
        vec![GrantedAgentTool::ReadTopicMaterialPack]
    );
    validate_candidate_output(&request, &outcome).expect("bounded candidate is valid");
    assert_eq!(outcome.output.claim_ceiling, "candidate_only");
    assert!(!outcome.output.unknowns.is_empty());
    assert!(!outcome.output.alternative_explanations.is_empty());
}

struct ForeignCitationAdapter;

impl ModelToolLoopPort for ForeignCitationAdapter {
    fn adapter_identity(&self) -> &'static str {
        "malicious-fixture"
    }

    fn run(
        &self,
        request: &linggan_intelligence::ExecutionRequest,
    ) -> Result<ExecutionOutcome, ModelToolLoopError> {
        let valid_ref = request.material_pack.members[0].work_public_ref;
        Ok(ExecutionOutcome {
            tool_calls: vec![GrantedAgentTool::ReadTopicMaterialPack],
            output: CandidateAnalysisOutput {
                claim_ceiling: "candidate_only".to_owned(),
                observed_references: vec![valid_ref],
                candidate_explanations: vec![CandidateExplanation {
                    statement: "unsupported stronger claim".to_owned(),
                    supporting_refs: vec![Uuid::new_v4()],
                    challenging_refs: vec![request.material_pack.members[1].work_public_ref],
                    limitations: "synthetic fixture only".to_owned(),
                    falsification_condition: "a broader qualified pack contradicts it".to_owned(),
                }],
                alternative_explanations: vec!["fatigue".to_owned()],
                unknowns: vec!["population prevalence".to_owned()],
                information_gaps: vec!["independent observations".to_owned()],
                user_summary: "candidate only".to_owned(),
            },
        })
    }
}

#[test]
fn output_validator_rejects_a_reference_outside_the_pack() {
    let request = invocation().execution_request().expect("valid invocation");
    let outcome = ForeignCitationAdapter
        .run(&request)
        .expect("fixture emits an outcome");
    let error = validate_candidate_output(&request, &outcome)
        .expect_err("foreign citation cannot finalize");
    assert_eq!(
        error.to_string(),
        "candidate output references material outside the frozen pack"
    );
}

struct FixedPiTransport;

impl PiLoopTransport for FixedPiTransport {
    fn package_version(&self) -> &str {
        PI_AGENT_CORE_VERSION
    }

    fn execute(
        &self,
        request: &linggan_intelligence::ExecutionRequest,
    ) -> Result<ExecutionOutcome, ModelToolLoopError> {
        DeterministicCandidateAdapter.run(request)
    }
}

#[test]
fn fixed_pi_transport_and_deterministic_adapter_share_the_output_contract() {
    let request = invocation().execution_request().expect("valid invocation");
    for outcome in [
        DeterministicCandidateAdapter
            .run(&request)
            .expect("deterministic adapter runs"),
        PiCandidateAdapter::new(FixedPiTransport)
            .run(&request)
            .expect("fixed Pi transport runs"),
    ] {
        validate_candidate_output(&request, &outcome).expect("shared contract accepts output");
    }
}

#[test]
fn output_validator_enforces_the_exact_tool_budget() {
    let request = invocation().execution_request().expect("valid invocation");
    let mut outcome = DeterministicCandidateAdapter
        .run(&request)
        .expect("deterministic adapter runs");
    outcome
        .tool_calls
        .push(GrantedAgentTool::ReadTopicMaterialPack);
    assert_eq!(
        validate_candidate_output(&request, &outcome)
            .expect_err("duplicate tool call exceeds budget")
            .to_string(),
        "candidate output exceeds the frozen execution budget"
    );
}

#[test]
fn output_validator_rejects_citations_assigned_to_the_wrong_role() {
    let request = invocation().execution_request().expect("valid invocation");
    let mut outcome = DeterministicCandidateAdapter
        .run(&request)
        .expect("deterministic adapter runs");
    outcome.output.candidate_explanations[0].supporting_refs =
        vec![request.material_pack.members[1].work_public_ref];
    assert_eq!(
        validate_candidate_output(&request, &outcome)
            .expect_err("challenge material cannot masquerade as support")
            .to_string(),
        "candidate output is not eligible: candidate citations do not match their adjudicated roles"
    );
}
