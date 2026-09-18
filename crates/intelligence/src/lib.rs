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
pub mod comment_study_acceptance;
pub mod comment_study_batch;
pub mod comment_study_canonical;
pub mod comment_study_comparison_cache;
pub mod comment_study_embedding;
pub mod comment_study_recall;
pub mod comment_study_batch_acceptance;
pub mod comment_study_batch_worker;
pub mod comment_study_candidate_recall;
pub mod comment_study_model_dispatch;
pub mod comment_study_model_runner;
pub mod comment_study_pair_worker;
pub mod comment_study_problem_resolution;
pub mod comment_study_problem_store;
pub mod comment_study_read;
pub mod comment_study_resolution_worker;
pub mod comment_study_run;
pub mod comment_study_semantic;
pub mod comment_study_source;
