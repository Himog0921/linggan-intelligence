//! Domain intelligence interfaces. Topic workspaces are explicit, versioned human research
//! decisions over Work Resource identities; they never manufacture Evidence or source facts.

pub mod comment_analysis;
pub mod comment_research;
pub mod comment_research_projection;
mod topic_workspace;

pub use topic_workspace::{
    TopicClassificationRun, TopicDefinition, TopicMaterialMember, TopicMaterialMemberImport,
    TopicMaterialPack, TopicMaterialRole, TopicWorkspace, TopicWorkspaceError,
    TopicWorkspaceImport, TopicWorkspaceReceipt, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};

pub const INTELLIGENCE_IMPLEMENTED: bool = true;

pub mod comment_research_management;
