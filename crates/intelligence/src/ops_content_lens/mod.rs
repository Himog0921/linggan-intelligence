//! Definitions shared by title labelling, retrospective analysis and snapshot readers.
mod adjudication;
pub use adjudication::{Adjudication, adjudicate};
mod agreement;
mod analysis;
pub use agreement::{Agreement, agreement};
mod example_titles;
pub mod label;
mod taxonomy;
pub use example_titles::load_taxonomy_file;

pub use analysis::*;
use linggan_storage_postgres::Database;
use serde::Deserialize;
pub use taxonomy::{TaxonomyReceipt, load_taxonomy};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LensDefinitionError {
    #[error("ops_lens_taxonomy_invalid: {0}")]
    InvalidTaxonomy(&'static str),
    #[error("ops_lens_definition_invalid")]
    InvalidDefinition,
    #[error("ops_lens_missing_agreement")]
    MissingAgreement,
    #[error("ops_lens_calculation_unavailable")]
    CalculationUnavailable,
    #[error("ops_lens_scope_invalid")]
    InvalidScope,
    #[error("ops_lens_no_current_method")]
    NoMethod,
    #[error("ops_lens_snapshot_not_found")]
    SnapshotNotFound,
    #[error("ops_lens_run_unavailable")]
    RunUnavailable,
    #[error("ops_lens_adjudication_conflict")]
    AdjudicationConflict,
    #[error("ops_lens_database_error")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Deserialize)]
pub struct MetricDefinition {
    pub views_threshold_high: u64,
    pub views_threshold_mid: u64,
    pub comments_threshold_active: u64,
    pub min_n_rate: u32,
    pub min_account_posts: u32,
    pub wilson_z: f64,
    pub permutation_count: u32,
    pub permutation_seed: u64,
    pub label_batch_size: u32,
    pub audit_sample_size: u32,
    pub audit_sample_seed: u64,
    pub min_kappa: f64,
    pub min_agreement: f64,
    pub stale_after_days: u32,
}

pub async fn metric_definition(
    database: &Database,
    version: i32,
) -> Result<(Uuid, MetricDefinition), LensDefinitionError> {
    let (reference, sqlx::types::Json(metric)) = sqlx::query_as(
        "SELECT definition_ref,body FROM linggan_ops_lens_definition WHERE kind='metric' AND version=$1",
    )
    .bind(version)
    .fetch_one(database.pool())
    .await?;
    Ok((reference, metric))
}

mod read;
pub use read::{
    BoardStatus, PageRequest, SnapshotLookup, board_status, publications, read_snapshot, spot_check,
};
mod sample;
mod statistics;
mod variance;
pub use sample::{LabelFilter, SampleMetric, Subset, select_publications};

pub fn enabled_from_env() -> bool {
    std::env::var("LINGGAN_OPS_CONTENT_LENS").as_deref() == Ok("enabled")
}
mod snapshot;
mod source;
pub use snapshot::{SnapshotReceipt, SnapshotScope, build_snapshot};
pub use source::TITLED_PUBLICATIONS_CTE;
pub use variance::shuffle_with_seed;

#[cfg(test)]
mod golden_tests;
