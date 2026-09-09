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

pub mod model_secrets;
pub mod model_settings;
pub mod pi_adapter;

pub mod model_invocation;

pub mod model_plans;

pub mod model_runner;

pub mod model_worker_drain;

pub mod model_settings_read;

pub(crate) mod comment_cleaning;
pub mod comment_daily;
mod comment_daily_read;
mod comment_daily_runner;
mod comment_packet;
pub mod comment_research_atoms;
pub mod comment_research_kernel;

pub mod comment_intelligence;
pub mod comment_intelligence_actions;
pub mod comment_intelligence_problems;
pub mod comment_intelligence_statistics;
mod comment_preflight;

mod comment_legacy_reuse;
pub mod comment_replay_metrics;
pub mod comment_research_fingerprint;
pub mod comment_research_rule_builder;
pub mod comment_research_rules;
pub mod comment_runtime;

pub mod embedding_settings;

mod comment_execution_budget;

mod comment_semantic_reservation;

mod comment_research_reconcile;

mod comment_eligibility_projection;

pub mod comment_semantic_compute;

pub mod comment_replay;

mod comment_semantic_atoms;

mod comment_atom_vectors;

mod comment_field_repair;
mod comment_local_recovery;
pub mod comment_research_sampling;

mod comment_auto_backlog;

mod comment_auto_status;

pub mod comment_semantic_organization;

mod comment_topic_associations;

mod comment_field_repair_read;

pub mod comment_replay_continuity;
