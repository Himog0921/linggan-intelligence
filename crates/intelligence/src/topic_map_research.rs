//! Topic-map research commands and immutable canonical input identity.
use linggan_evidence::creator_discovery::{DiscoveryWork, Fragment};
use serde_json::Value;
use uuid::Uuid;

#[path = "topic_map_research/identity.rs"]
mod identity;
pub(crate) use identity::semantic_source_identity;
#[path = "topic_map_research/queue.rs"]
mod queue;
#[path = "topic_map_research/restoration.rs"]
mod restoration;
#[path = "topic_map_research/source.rs"]
mod source;
#[path = "topic_map_research/windows.rs"]
mod windows;
pub(crate) use queue::overlaps_unknown_dispatch;
pub(crate) use restoration::{research_result_sources_current, restore_scoped_window};
#[cfg(test)]
use restoration::{restore_window, sources_current};
pub(crate) use source::{
    load_inputs_for_works, load_inputs_for_works_at, manifest_as_of, manifest_source_work_refs,
};
#[cfg(test)]
pub(crate) use windows::{input_identity, with_comparison_context};
pub(crate) use windows::{
    is_research_evidence, reference_manifest, research_window_count, research_windows,
};
#[cfg(test)]
#[path = "topic_map_research/tests.rs"]
mod input_tests;

#[cfg(test)]
#[path = "topic_map_research/tests/saved_source_postgres.rs"]
mod saved_source_postgres;

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("research_not_found")]
    NotFound,
    #[error("request_identity_conflict")]
    Conflict,
}
impl ResearchError {
    pub fn code(&self) -> &str {
        match self {
            Self::Database(_) => "research_read_failed",
            Self::Invalid(c) => c,
            Self::NotFound => "research_not_found",
            Self::Conflict => "request_identity_conflict",
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ResearchInput {
    pub work: DiscoveryWork,
    pub fragments: Vec<Fragment>,
    pub topics: Value,
    pub domain: Value,
    pub hash: String,
    pub context_work_refs: Vec<Uuid>,
    pub comment_study: Value,
    pub role_metadata: Value,
    pub coverage: Value,
}

pub(crate) const INPUT_CONTRACT: &str = "topic-map.source-windows.v3";
const PARENT_CONTEXT_FIELD: &str = "parent_comment_context";
