//! Local content authorship, immutable revisions and frozen exports; never publication.
mod assets;
mod experience_fragments;
mod exports;
pub mod recovery;
mod requests;
mod source_usage;
mod store;
pub mod types;
mod validation;
pub use assets::{asset_file, upload_asset};
pub use experience_fragments::all_available as experience_fragments_available_in;
pub(crate) use experience_fragments::strip as strip_experience_text;
pub(crate) use experience_fragments::withdrawn_history as withdrawn_experience_history_in;
pub use exports::{
    export_file, export_page_file, finalize_export, freeze_export, mark_export_failed, read_export,
    retry_export, upload_export_page,
};
pub use requests::read_request;
pub(crate) use requests::{RequestOwner, ensure_request_owner_in, lock as work_lock};
use sha2::{Digest, Sha256};
pub use source_usage::{
    propagate_adopted_sources_in, redact_revision_in, redact_revision_with_statuses_in,
};
pub use store::{
    adopt_in, create_work, list_idea_boxes, list_works, place, read_revision_in, read_work,
    reset_withdrawn, restore, revision, revision_index, revisions, save,
};
pub use types::*;
pub(crate) use validation::document as validate_document;
#[derive(Debug, thiserror::Error)]
pub enum WorkbenchError {
    #[error("creation_invalid:{0}")]
    Invalid(&'static str),
    #[error("creation_not_found")]
    NotFound,
    #[error("creation_request_conflict")]
    RequestConflict,
    #[error("creation_source_unavailable")]
    Source,
    #[error("creation_export_incomplete")]
    Incomplete,
    #[error("creation_export_conflict")]
    ExportConflict,
    #[error("creation_storage_not_configured")]
    StorageNotConfigured,
    #[error("creation_legacy_reference_unavailable")]
    LegacyReferenceUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
pub(crate) fn hash(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
