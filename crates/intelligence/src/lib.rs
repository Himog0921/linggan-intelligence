//! Domain intelligence interfaces. Topic workspaces are explicit, versioned human research
//! decisions over Work Resource identities; they never manufacture Evidence or source facts.

pub mod ai_foundation;
pub mod creator_discovery_analysis;
pub mod creator_discovery_input;
pub mod note_interpretation_offline;
pub mod research_text;
pub mod topic_map;
pub mod topic_map_collection_search;
pub mod topic_map_core;
pub mod topic_map_research;
pub mod topic_map_research_analysis;
pub mod topic_map_research_collection;
mod topic_workspace;

pub use topic_workspace::{
    TopicClassificationRun, TopicDefinition, TopicMaterialMember, TopicMaterialMemberImport,
    TopicMaterialPack, TopicMaterialRole, TopicWorkspace, TopicWorkspaceError,
    TopicWorkspaceImport, TopicWorkspaceReceipt, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};

pub const INTELLIGENCE_IMPLEMENTED: bool = true;

pub mod ops_ledger;

pub mod content_workbench;
pub mod content_workbench_ai;
pub mod content_workbench_sources;
pub mod creation_assets;

pub mod model_secrets;
pub mod model_settings;
pub mod pi_adapter;
mod pi_offline;
mod pi_request;
mod pi_skills;

pub mod model_invocation;

pub mod model_worker_drain;

pub(crate) mod comment_cleaning;
pub mod comment_study_catalog;
pub mod comment_study_selection;
pub mod comment_study_semantic;
pub mod comment_study_source;

pub mod topic_map_structure;

pub mod ocr_recognition_schema;
