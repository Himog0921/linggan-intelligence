//! Definitions shared by title labelling, retrospective analysis and snapshot readers.
mod analysis;
mod taxonomy;

pub use analysis::*;
use linggan_storage_postgres::Database;
use serde::Deserialize;
pub use taxonomy::{TaxonomyReceipt, load_taxonomy};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LensDefinitionError {
    #[error("ops_lens_taxonomy_invalid: {0}")]
    InvalidTaxonomy(&'static str),
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
