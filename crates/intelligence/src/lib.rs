//! Domain intelligence interfaces. Topic workspaces are explicit, versioned human research
//! decisions over Work Resource identities; they never manufacture Evidence or source facts.

pub mod research_text;
mod topic_workspace;

pub use topic_workspace::{
    TopicClassificationRun, TopicDefinition, TopicMaterialMember, TopicMaterialMemberImport,
    TopicMaterialPack, TopicMaterialRole, TopicWorkspace, TopicWorkspaceError,
    TopicWorkspaceImport, TopicWorkspaceReceipt, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};

pub const INTELLIGENCE_IMPLEMENTED: bool = true;

pub mod model_secrets;
pub mod model_settings;
pub mod pi_adapter;

pub mod model_invocation;

pub mod model_runner;

pub mod model_worker_drain;

pub mod model_settings_read;

pub(crate) mod comment_cleaning;
pub mod comment_research_atoms;
pub mod comment_research_embeddings;
pub mod comment_research_kernel;
pub mod comment_research_problems;
pub mod comment_research_read_v1;
pub mod comment_research_results;
pub mod comment_research_worker;

pub mod embedding_settings;
pub mod local_embedding_profile;
