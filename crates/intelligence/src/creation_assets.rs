//! CREATION-ASSETS-001 S1a: account identity, product fact cards, the bridge sentence library,
//! three-level manuscript checks and the Xiaohongshu copy format. Nothing here publishes.
mod bridge_sentences;
mod checks;
mod copy_format;
mod drafts;
pub mod experience_policy;
mod experiences;
mod fact_cards;
mod identity;
mod persona_facts;
mod requests;
mod subject;
mod text;
pub mod types;
mod words;

pub use bridge_sentences::{
    candidates_for_account, check_sentence_text, create_sentence, edit_sentence, list_sentences,
    review_sentence,
};
pub use checks::{blocks_entry, check_manuscript, check_sentence};
pub use copy_format::{CopyText, SPACER, format_copy, normalize_tags};
pub use drafts::{check_draft, check_draft_for_work, confirm_copy, copy_status, prepare_copy};
pub use experiences::{
    cancel_experience_request, confirm_experience, create_experience, edit_experience,
    experience_request_status, list_experiences, read_experience, read_usable_experience,
    withdraw_experience,
};
pub use fact_cards::{confirm_fact_card, list_fact_cards, put_fact_card};
pub use identity::{list_accounts, put_identity, read_identity};
pub use persona_facts::{
    cancel_persona_request, confirm_persona_fact, list_persona_facts, persona_fact_history, persona_request_status, put_persona_fact, withdraw_persona_fact,
};
pub use subject::{CardTerms, Context, Subject};
pub use types::*;
pub use words::*;

use crate::content_workbench::WorkbenchError;

#[derive(Debug, thiserror::Error)]
pub enum AssetsError {
    #[error("creation_assets_invalid:{0}")]
    Invalid(&'static str),
    #[error("creation_assets_not_found")]
    NotFound,
    /// The same request id was used for a different command.
    #[error("creation_assets_request_conflict")]
    RequestConflict,
    /// The editor started from a revision that is no longer the latest.
    #[error("creation_assets_stale")]
    Stale,
    #[error("creation_assets_source_unavailable")]
    SourceUnavailable,
    /// Rules refuse the command; the findings say why.
    #[error("creation_assets_blocked")]
    Blocked(Vec<Finding>),
    #[error(transparent)]
    Workbench(WorkbenchError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
