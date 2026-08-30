//! Domain intelligence interfaces. Topic workspaces are explicit, versioned human research
//! decisions over Work Resource identities; they never manufacture Evidence or source facts.

mod agent_candidate;
mod agent_runtime;
mod topic_workspace;

pub use agent_candidate::{
    AgentBudget, AgentContractError, CandidateAnalysisInvocation, CandidateAnalysisOutput,
    CandidateExplanation, DeterministicCandidateAdapter, ExecutionOutcome, ExecutionRequest,
    FrozenTopicMaterial, FrozenTopicMaterialPack, GrantedAgentTool, ModelToolLoopError,
    ModelToolLoopPort, PI_AGENT_CORE_PACKAGE, PI_AGENT_CORE_VERSION, PiCandidateAdapter,
    PiLoopTransport, validate_candidate_output,
};
pub use agent_runtime::{
    AgentInvocationReceipt, AgentInvocationState, AgentRuntime, AgentRuntimeError,
    AgentRuntimeFuture, PostgresAgentRuntime,
};

pub use topic_workspace::{
    TopicClassificationRun, TopicDefinition, TopicMaterialMember, TopicMaterialMemberImport,
    TopicMaterialPack, TopicMaterialRole, TopicWorkspace, TopicWorkspaceError,
    TopicWorkspaceImport, TopicWorkspaceReceipt, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};

pub const INTELLIGENCE_IMPLEMENTED: bool = true;
