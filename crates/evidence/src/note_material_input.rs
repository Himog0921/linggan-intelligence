//! A versioned input projection over the shared dossier; never dispatches an AI or media job.
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

mod backfill;
mod batch;
mod identity;
mod read;
mod readiness;
mod snapshot;
pub use batch::{BatchOperation, BatchPlan, BatchReceipt, apply_plan, plan};
pub use read::{read, read_at};
pub use snapshot::{SnapshotReceipt, save_snapshot};

pub const VERSION: &str = "note-material-input.v1";
pub const FINGERPRINT_VERSION: &str = "note-material-fingerprint.v1";
pub const MAX_WORKS: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("material_not_found")]
    NotFound,
    #[error("material_changed")]
    Conflict,
    #[error("material_storage_unavailable")]
    Storage(#[from] sqlx::Error),
    #[error("material_relayer_failed")]
    Relayer(#[from] crate::ProducerRuntimeError),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColumnState {
    Available,
    NotProvided,
    NotAcquired,
    Waiting,
    KnownEmpty,
    TerminalFailure,
    Restricted,
    NotAdopted,
    NotConnected,
    NeedsRelayer,
    InvalidSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Column {
    pub field: String,
    pub state: ColumnState,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fragment {
    pub citation_id: String,
    pub field: String,
    pub logical_position: String,
    pub source_ref: Uuid,
    pub source_version: String,
    pub start: usize,
    pub end: usize,
    pub text_hash: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Material {
    pub contract: String,
    pub fingerprint_contract: String,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub as_of: String,
    pub dossier_revision: String,
    pub material_fingerprint: String,
    pub provenance_hash: String,
    pub snapshot_identity: String,
    pub ready: bool,
    pub not_ready_reasons: Vec<String>,
    pub columns: Vec<Column>,
    pub fragments: Vec<Fragment>,
    pub layer_version: String,
    pub processing_scope: ProcessingScope,
}

pub(crate) async fn cutoff(
    tx: &mut Transaction<'_, Postgres>,
    value: Option<&str>,
) -> Result<String, Error> {
    let valid: bool =
        sqlx::query_scalar("SELECT COALESCE($1::timestamptz,scope_001_now())<=scope_001_now()")
            .bind(value)
            .fetch_one(&mut **tx)
            .await?;
    if !valid {
        return Err(Error::Invalid("future_cutoff"));
    }
    Ok(sqlx::query_scalar("SELECT to_char(COALESCE($1::timestamptz,scope_001_now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')")
        .bind(value).fetch_one(&mut **tx).await?)
}

pub(crate) fn validate_works(works: &[Uuid]) -> Result<(), Error> {
    if works.is_empty() || works.len() > MAX_WORKS {
        return Err(Error::Invalid("scope_must_contain_1_to_200_works"));
    }
    let unique: std::collections::BTreeSet<_> = works.iter().collect();
    if unique.len() != works.len() {
        return Err(Error::Invalid("duplicate_work"));
    }
    Ok(())
}

/// Fixed interpretation boundary: omitted processing is visible, never inferred complete.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessingScope {
    pub included: Vec<String>,
    pub context_only: Vec<String>,
    pub not_connected: Vec<String>,
}
