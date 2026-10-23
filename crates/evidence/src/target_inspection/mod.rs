//! Object-scoped inspection over existing immutable material records.
//! No new Work identity, scheduler, capture or model dispatch is created by these reads.
mod scope;
mod rounds;
mod comparison;
mod works;
mod trace;

pub use comparison::compare_rounds;
pub use rounds::read_rounds;
pub use trace::read_trace;
pub use works::compare_works;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const METHOD_VERSION: &str = "target-inspection.v1";
pub const PAGE_SIZE: usize = 50;
pub const ROUND_WORK_LIMIT: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("target_inspection_not_found")]
    NotFound,
    #[error("target_inspection_scope_unavailable")]
    Scope,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("target_inspection_material_unavailable")]
    Material,
}

#[derive(Clone, Debug)]
pub struct Scope {
    pub target_ref: Uuid,
    pub domain_ref: Option<Uuid>,
    pub knowledge_cutoff: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metric {
    pub value: Option<i64>,
    pub state: String,
    pub observation_ref: Option<String>,
    pub observed_at: Option<String>,
    pub observed_epoch_ms: Option<i64>,
    pub source_kind: Option<String>,
    pub precision: String,
}
impl Metric {
    pub(crate) fn unknown() -> Self {
        Self { value: None, state: "unknown".into(), observation_ref: None,
            observed_at: None, observed_epoch_ms: None, source_kind: None,
            precision: "unknown".into() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metrics {
    pub likes: Metric,
    pub comments: Metric,
    pub collects: Metric,
    pub shares: Metric,
}
impl Metrics {
    pub(crate) fn unknown() -> Self {
        Self { likes: Metric::unknown(), comments: Metric::unknown(),
            collects: Metric::unknown(), shares: Metric::unknown() }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDifference {
    pub left: Metric,
    pub right: Metric,
    pub delta: Option<i64>,
    pub elapsed_seconds: Option<f64>,
    pub percent: Option<f64>,
    pub comparable: bool,
    pub reason: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDifferences {
    pub likes: MetricDifference,
    pub comments: MetricDifference,
    pub collects: MetricDifference,
    pub shares: MetricDifference,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    /// Runtime admits one terminal package per Attempt. A retry is another Attempt.
    pub round_ref: Uuid,
    pub package_ref: Uuid,
    pub rule_ref: Option<Uuid>,
    pub rule_revision_ref: Option<Uuid>,
    pub slot: Option<String>,
    pub observed_at: String,
    pub observed_epoch_ms: i64,
    pub accepted_at: String,
    pub coverage: String,
    pub hits: i64,
    pub conditions: serde_json::Value,
    #[serde(skip)]
    pub(crate) account_ref: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundPage {
    pub target_ref: Uuid,
    pub domain_ref: Option<Uuid>,
    pub knowledge_cutoff: String,
    pub method_version: &'static str,
    pub rounds: Vec<Round>,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundWork {
    pub work_ref: Uuid,
    pub title: Option<String>,
    pub creator: Option<String>,
    pub position: Option<i64>,
    pub metrics: Metrics,
    pub first_seen_in_scope: bool,
    pub library_presence: String,
    pub seen_before_left: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparedRow {
    pub work_ref: Uuid,
    pub membership: &'static str,
    pub discovery_label: String,
    pub left: Option<RoundWork>,
    pub right: Option<RoundWork>,
    pub metrics: MetricDifferences,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundComparison {
    pub target_ref: Uuid,
    pub knowledge_cutoff: String,
    pub method_version: &'static str,
    pub left: Round,
    pub right: Round,
    pub comparison_state: &'static str,
    pub reasons: Vec<&'static str>,
    pub rows: Vec<ComparedRow>,
    pub common_count: usize,
    pub entered_count: usize,
    pub absent_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEntry {
    pub observation_ref: String,
    pub package_ref: Uuid,
    pub round_ref: Uuid,
    pub target_ref: Option<Uuid>,
    pub target_name: Option<String>,
    pub rule_slot: Option<String>,
    pub source_kind: String,
    pub observed_at: String,
    pub observed_epoch_ms: i64,
    pub accepted_at: String,
    pub metrics: Metrics,
    pub position: Option<i64>,
    pub late_arrival: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePage {
    pub work_ref: Uuid,
    pub knowledge_cutoff: String,
    pub observations: Vec<TraceEntry>,
    pub next_offset: Option<usize>,
    pub method_version: &'static str,
}
