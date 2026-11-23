//! Local content authorship, immutable revisions and frozen exports; never publication.
mod assets;
mod experience_fragments;
mod exports;
mod idea_drafts;
pub mod recovery;
mod requests;
mod source_usage;
mod store;
pub mod topic_handoff;
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
pub use idea_drafts::{open_idea_drafts, open_idea_drafts_as_in};
pub use requests::read_request;
pub(crate) use requests::{RequestOwner, ensure_request_owner_in, lock as work_lock};
use sha2::{Digest, Sha256};
pub use source_usage::{
    propagate_adopted_sources_in, redact_revision_in, redact_revision_with_statuses_in,
};
pub use store::{
    IDEA_ACCOUNT_LIMIT, adopt_in, create_work, create_work_as_in, desk_card_positions,
    desk_positions, list_idea_boxes, list_idea_boxes_in, list_works, list_works_in, lock_save_in,
    place, read_revision_in, read_work, reset_withdrawn, restore, revision, revision_index,
    revisions, save, save_as_in, save_desk_card_position, save_desk_position,
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
/// Ordinary team members read authored copy without raw comments linked to source excerpts.
pub fn strip_member_comment_quotes(document: &mut WorkDocument) -> bool {
    let mut restricted = false;
    for excerpt in &document.excerpts {
        if excerpt.citation.source_kind != SourceKind::Comment {
            continue;
        }
        let Some(text) = excerpt
            .selected_text
            .as_deref()
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        for field in std::iter::once(&mut document.title)
            .chain(std::iter::once(&mut document.angle))
            .chain(document.idea_name.iter_mut())
            .chain(
                document
                    .paragraphs
                    .iter_mut()
                    .map(|paragraph| &mut paragraph.text),
            )
            .chain(&mut document.tags)
        {
            if field.contains(text) {
                *field = field.replace(text, "[受限原话]");
                restricted = true;
            }
        }
    }
    restricted
}

pub(crate) fn hash(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "content_workbench/topic_handoff_tests.rs"]
mod topic_handoff_tests;
