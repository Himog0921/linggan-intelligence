use crate::TopicMaterialRole;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

pub const PI_AGENT_CORE_PACKAGE: &str = "@earendil-works/pi-agent-core";
pub const PI_AGENT_CORE_VERSION: &str = "0.84.2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBudget {
    pub max_steps: u16,
    pub max_tool_calls: u16,
    pub max_output_candidates: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenTopicMaterial {
    pub work_public_ref: Uuid,
    pub role: TopicMaterialRole,
    pub rationale: String,
    pub controlled_excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenTopicMaterialPack {
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
    pub definition_version: i32,
    pub classification_run_ref: Uuid,
    pub material_pack_ref: Uuid,
    pub source_boundary: String,
    pub members: Vec<FrozenTopicMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateAnalysisInvocation {
    pub idempotency_key: String,
    pub actor_ref: Uuid,
    pub delegation_revision: String,
    pub purpose: String,
    pub profile_key: String,
    pub profile_version: i32,
    pub material_pack: FrozenTopicMaterialPack,
    pub budget: AgentBudget,
}

impl CandidateAnalysisInvocation {
    pub fn execution_request(&self) -> Result<ExecutionRequest, AgentContractError> {
        validate_invocation(self)?;
        Ok(ExecutionRequest {
            invocation_ref: Uuid::new_v4(),
            purpose: self.purpose.clone(),
            profile_key: self.profile_key.clone(),
            profile_version: self.profile_version,
            material_pack: self.material_pack.clone(),
            granted_tools: vec![GrantedAgentTool::ReadTopicMaterialPack],
            budget: self.budget.clone(),
        })
    }

    pub(crate) fn execution_request_with_ref(
        &self,
        invocation_ref: Uuid,
    ) -> Result<ExecutionRequest, AgentContractError> {
        let mut request = self.execution_request()?;
        request.invocation_ref = invocation_ref;
        Ok(request)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantedAgentTool {
    ReadTopicMaterialPack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRequest {
    pub invocation_ref: Uuid,
    pub purpose: String,
    pub profile_key: String,
    pub profile_version: i32,
    pub material_pack: FrozenTopicMaterialPack,
    pub granted_tools: Vec<GrantedAgentTool>,
    pub budget: AgentBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateExplanation {
    pub statement: String,
    pub supporting_refs: Vec<Uuid>,
    pub challenging_refs: Vec<Uuid>,
    pub limitations: String,
    pub falsification_condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateAnalysisOutput {
    pub claim_ceiling: String,
    pub observed_references: Vec<Uuid>,
    pub candidate_explanations: Vec<CandidateExplanation>,
    pub alternative_explanations: Vec<String>,
    pub unknowns: Vec<String>,
    pub information_gaps: Vec<String>,
    pub user_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionOutcome {
    pub tool_calls: Vec<GrantedAgentTool>,
    pub output: CandidateAnalysisOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelToolLoopError {
    #[error("model/tool loop provider failed: {0}")]
    Provider(String),
    #[error("Pi adapter version mismatch: expected {expected}, actual {actual}")]
    VersionMismatch {
        expected: &'static str,
        actual: String,
    },
}

pub trait ModelToolLoopPort {
    fn adapter_identity(&self) -> &'static str;
    fn run(&self, request: &ExecutionRequest) -> Result<ExecutionOutcome, ModelToolLoopError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicCandidateAdapter;

impl ModelToolLoopPort for DeterministicCandidateAdapter {
    fn adapter_identity(&self) -> &'static str {
        "deterministic-topic-candidate-v1"
    }

    fn run(&self, request: &ExecutionRequest) -> Result<ExecutionOutcome, ModelToolLoopError> {
        let support_refs = material_refs(request, TopicMaterialRole::Support);
        let challenge_refs = counterweight_refs(request);
        let statement = request
            .material_pack
            .members
            .iter()
            .find(|member| member.role == TopicMaterialRole::Support)
            .map_or_else(
                || "No qualified supporting material was available.".to_owned(),
                |member| format!("Candidate explanation: {}", member.rationale),
            );
        Ok(ExecutionOutcome {
            tool_calls: vec![GrantedAgentTool::ReadTopicMaterialPack],
            output: CandidateAnalysisOutput {
                claim_ceiling: "candidate_only".to_owned(),
                observed_references: request
                    .material_pack
                    .members
                    .iter()
                    .map(|member| member.work_public_ref)
                    .collect(),
                candidate_explanations: vec![CandidateExplanation {
                    statement,
                    supporting_refs: support_refs,
                    challenging_refs: challenge_refs,
                    limitations: request.material_pack.source_boundary.clone(),
                    falsification_condition: "A broader qualified pack supplies a contradictory pattern."
                        .to_owned(),
                }],
                alternative_explanations: vec![
                    "The counterweight may reflect a different causal condition.".to_owned(),
                ],
                unknowns: vec![
                    "The frozen pack cannot establish prevalence or causality.".to_owned(),
                ],
                information_gaps: vec![
                    "Independent qualified observations outside this pack are absent.".to_owned(),
                ],
                user_summary: "A bounded candidate explanation was produced from the exact frozen pack; it is not adopted knowledge."
                    .to_owned(),
            },
        })
    }
}

fn material_refs(request: &ExecutionRequest, role: TopicMaterialRole) -> Vec<Uuid> {
    request
        .material_pack
        .members
        .iter()
        .filter(|member| member.role == role)
        .map(|member| member.work_public_ref)
        .collect()
}

fn counterweight_refs(request: &ExecutionRequest) -> Vec<Uuid> {
    request
        .material_pack
        .members
        .iter()
        .filter(|member| {
            matches!(
                member.role,
                TopicMaterialRole::Challenge | TopicMaterialRole::Boundary
            )
        })
        .map(|member| member.work_public_ref)
        .collect()
}

pub trait PiLoopTransport {
    fn package_version(&self) -> &str;
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionOutcome, ModelToolLoopError>;
}

#[derive(Debug, Clone)]
pub struct PiCandidateAdapter<T> {
    transport: T,
}

impl<T> PiCandidateAdapter<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: PiLoopTransport> ModelToolLoopPort for PiCandidateAdapter<T> {
    fn adapter_identity(&self) -> &'static str {
        "pi-agent-core-0.84.2-topic-candidate-v1"
    }

    fn run(&self, request: &ExecutionRequest) -> Result<ExecutionOutcome, ModelToolLoopError> {
        if self.transport.package_version() != PI_AGENT_CORE_VERSION {
            return Err(ModelToolLoopError::VersionMismatch {
                expected: PI_AGENT_CORE_VERSION,
                actual: self.transport.package_version().to_owned(),
            });
        }
        self.transport.execute(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentContractError {
    #[error("invalid Agent invocation: {0}")]
    InvalidInvocation(&'static str),
    #[error("candidate output references material outside the frozen pack")]
    ForeignReference,
    #[error("candidate output exceeds the frozen execution budget")]
    BudgetExceeded,
    #[error("candidate output is not eligible: {0}")]
    InvalidOutput(&'static str),
}

pub fn validate_candidate_output(
    request: &ExecutionRequest,
    outcome: &ExecutionOutcome,
) -> Result<(), AgentContractError> {
    validate_tool_calls(request, outcome)?;
    let output = &outcome.output;
    if output.claim_ceiling != "candidate_only" {
        return Err(AgentContractError::InvalidOutput(
            "claim ceiling must remain candidate_only",
        ));
    }
    if output.candidate_explanations.is_empty()
        || output.candidate_explanations.len() > usize::from(request.budget.max_output_candidates)
    {
        return Err(AgentContractError::InvalidOutput(
            "invalid candidate explanation count",
        ));
    }
    if output.alternative_explanations.is_empty()
        || output.unknowns.is_empty()
        || output.information_gaps.is_empty()
    {
        return Err(AgentContractError::InvalidOutput(
            "alternatives, unknowns and information gaps are required",
        ));
    }
    validate_output_references(request, output)?;
    validate_output_text(output)
}

fn validate_tool_calls(
    request: &ExecutionRequest,
    outcome: &ExecutionOutcome,
) -> Result<(), AgentContractError> {
    if outcome.tool_calls.len() > usize::from(request.budget.max_tool_calls) {
        return Err(AgentContractError::BudgetExceeded);
    }
    if outcome
        .tool_calls
        .iter()
        .any(|tool| !request.granted_tools.contains(tool))
    {
        return Err(AgentContractError::InvalidOutput("ungranted tool call"));
    }
    if outcome.tool_calls != [GrantedAgentTool::ReadTopicMaterialPack] {
        return Err(AgentContractError::InvalidOutput(
            "exact material pack read is required once",
        ));
    }
    Ok(())
}

fn validate_output_references(
    request: &ExecutionRequest,
    output: &CandidateAnalysisOutput,
) -> Result<(), AgentContractError> {
    let allowed = request
        .material_pack
        .members
        .iter()
        .map(|member| member.work_public_ref)
        .collect::<BTreeSet<_>>();
    let mut cited = output.observed_references.clone();
    for candidate in &output.candidate_explanations {
        if candidate.supporting_refs.is_empty() || candidate.challenging_refs.is_empty() {
            return Err(AgentContractError::InvalidOutput(
                "each candidate requires support and a counterweight",
            ));
        }
        cited.extend(&candidate.supporting_refs);
        cited.extend(&candidate.challenging_refs);
    }
    if cited.iter().any(|reference| !allowed.contains(reference)) {
        return Err(AgentContractError::ForeignReference);
    }
    if output.candidate_explanations.iter().any(|candidate| {
        !candidate
            .supporting_refs
            .iter()
            .all(|reference| has_role(request, *reference, TopicMaterialRole::Support))
            || !candidate.challenging_refs.iter().all(|reference| {
                has_role(request, *reference, TopicMaterialRole::Challenge)
                    || has_role(request, *reference, TopicMaterialRole::Boundary)
            })
    }) {
        return Err(AgentContractError::InvalidOutput(
            "candidate citations do not match their adjudicated roles",
        ));
    }
    Ok(())
}

fn has_role(request: &ExecutionRequest, reference: Uuid, role: TopicMaterialRole) -> bool {
    request
        .material_pack
        .members
        .iter()
        .any(|member| member.work_public_ref == reference && member.role == role)
}

fn validate_output_text(output: &CandidateAnalysisOutput) -> Result<(), AgentContractError> {
    valid_output_text(&output.user_summary, 2000, "invalid user summary")?;
    for candidate in &output.candidate_explanations {
        valid_output_text(&candidate.statement, 2000, "invalid candidate statement")?;
        valid_output_text(
            &candidate.limitations,
            2000,
            "invalid candidate limitations",
        )?;
        valid_output_text(
            &candidate.falsification_condition,
            2000,
            "invalid falsification condition",
        )?;
    }
    for value in output
        .alternative_explanations
        .iter()
        .chain(&output.unknowns)
        .chain(&output.information_gaps)
    {
        valid_output_text(value, 2000, "invalid candidate analysis text")?;
    }
    Ok(())
}

fn validate_invocation(request: &CandidateAnalysisInvocation) -> Result<(), AgentContractError> {
    if !valid_idempotency_key(&request.idempotency_key) {
        return Err(AgentContractError::InvalidInvocation(
            "invalid idempotency key",
        ));
    }
    valid_text(
        &request.delegation_revision,
        160,
        "invalid delegation revision",
    )?;
    valid_text(&request.purpose, 1000, "invalid purpose")?;
    if !valid_key(&request.profile_key, 64) || request.profile_version < 1 {
        return Err(AgentContractError::InvalidInvocation("invalid profile"));
    }
    validate_budget(&request.budget)?;
    validate_pack(&request.material_pack)
}

fn validate_budget(budget: &AgentBudget) -> Result<(), AgentContractError> {
    if budget.max_steps == 0
        || budget.max_steps > 8
        || budget.max_tool_calls != 1
        || budget.max_output_candidates == 0
        || budget.max_output_candidates > 5
    {
        return Err(AgentContractError::InvalidInvocation(
            "budget exceeds the A1-A2 profile",
        ));
    }
    Ok(())
}

fn validate_pack(pack: &FrozenTopicMaterialPack) -> Result<(), AgentContractError> {
    valid_text(&pack.source_boundary, 2000, "invalid source boundary")?;
    if pack.definition_version < 1 || !(2..=100).contains(&pack.members.len()) {
        return Err(AgentContractError::InvalidInvocation(
            "invalid frozen material pack",
        ));
    }
    let mut refs = BTreeSet::new();
    let mut support = false;
    let mut counterweight = false;
    for member in &pack.members {
        if !refs.insert(member.work_public_ref) {
            return Err(AgentContractError::InvalidInvocation(
                "duplicate frozen material",
            ));
        }
        support |= member.role == TopicMaterialRole::Support;
        counterweight |= matches!(
            member.role,
            TopicMaterialRole::Challenge | TopicMaterialRole::Boundary
        );
        valid_text(&member.rationale, 1000, "invalid material rationale")?;
        valid_text(
            &member.controlled_excerpt,
            2000,
            "invalid controlled excerpt",
        )?;
    }
    if !support || !counterweight {
        return Err(AgentContractError::InvalidInvocation(
            "support and counterweight are required",
        ));
    }
    Ok(())
}

fn valid_idempotency_key(value: &str) -> bool {
    (8..=128).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && b"._:-".contains(&byte))
        })
}

fn valid_key(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_text(value: &str, max: usize, error: &'static str) -> Result<(), AgentContractError> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(AgentContractError::InvalidInvocation(error));
    }
    Ok(())
}

fn valid_output_text(
    value: &str,
    max: usize,
    error: &'static str,
) -> Result<(), AgentContractError> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(AgentContractError::InvalidOutput(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_pi_adapter_refuses_a_version_drift() {
        struct DriftedTransport;
        impl PiLoopTransport for DriftedTransport {
            fn package_version(&self) -> &str {
                "0.85.0"
            }
            fn execute(
                &self,
                _request: &ExecutionRequest,
            ) -> Result<ExecutionOutcome, ModelToolLoopError> {
                unreachable!("version drift must fail closed")
            }
        }
        let request = CandidateAnalysisInvocation {
            idempotency_key: "agent-unit:pi-version:0001".to_owned(),
            actor_ref: Uuid::new_v4(),
            delegation_revision: "test:v1".to_owned(),
            purpose: "contract test".to_owned(),
            profile_key: "topic-candidate-analysis".to_owned(),
            profile_version: 1,
            material_pack: FrozenTopicMaterialPack {
                topic_ref: Uuid::new_v4(),
                definition_ref: Uuid::new_v4(),
                definition_version: 1,
                classification_run_ref: Uuid::new_v4(),
                material_pack_ref: Uuid::new_v4(),
                source_boundary: "synthetic".to_owned(),
                members: vec![
                    material(TopicMaterialRole::Support),
                    material(TopicMaterialRole::Boundary),
                ],
            },
            budget: AgentBudget {
                max_steps: 2,
                max_tool_calls: 1,
                max_output_candidates: 1,
            },
        }
        .execution_request()
        .expect("fixture is valid");
        let error = PiCandidateAdapter::new(DriftedTransport)
            .run(&request)
            .expect_err("version drift fails closed");
        assert!(matches!(error, ModelToolLoopError::VersionMismatch { .. }));
    }

    fn material(role: TopicMaterialRole) -> FrozenTopicMaterial {
        FrozenTopicMaterial {
            work_public_ref: Uuid::new_v4(),
            role,
            rationale: "synthetic rationale".to_owned(),
            controlled_excerpt: "synthetic de-identified excerpt".to_owned(),
        }
    }
}
