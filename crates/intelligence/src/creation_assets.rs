//! CREATION-ASSETS-001 S1a: account identity, product fact cards, the bridge sentence library,
//! three-level manuscript checks and the Xiaohongshu copy format. Nothing here publishes.
mod bridge_sentences;
mod checks;
mod copy_format;
mod creative_budget;
mod drafts;
pub mod experience_policy;
mod experiences;
mod fact_cards;
mod identity;
mod persona_facts;
mod publications;
mod requests;
mod subject;
mod templates;
mod text;
pub mod types;
mod words;

pub use bridge_sentences::{
    candidates_for_account, check_sentence_text, create_sentence, edit_sentence, list_sentences,
    review_sentence,
};
pub use checks::{blocks_entry, check_manuscript, check_sentence, title_has_cure_promise};
pub use copy_format::{CopyText, SPACER, format_copy, normalize_tags};
pub use creative_budget::{
    ChangeCreativeBudget, CreativeBudget, CreativeBudgetReceipt, change_creative_budget,
    read_creative_budget,
};
pub use drafts::{
    check_draft, check_draft_for_work, check_saved_drafts, confirm_copy, copy_status, prepare_copy,
};
pub use experiences::{
    cancel_experience_request, confirm_experience, create_experience, edit_experience,
    experience_request_status, list_experiences, read_experience, read_usable_experience,
    withdraw_experience,
};
pub use fact_cards::{confirm_fact_card, list_fact_cards, put_fact_card};
pub use identity::{list_accounts, put_identity, read_identity};
pub use persona_facts::{
    cancel_persona_request, confirm_persona_fact, list_persona_facts, persona_fact_history,
    persona_request_status, put_persona_fact, withdraw_persona_fact,
};
pub use publications::{
    link_publication, publication_candidates, publication_links, unlink_publication,
};
pub use subject::{CardTerms, Context, Subject};
pub use templates::{
    CreateTemplate, ReviewTemplate, ReviseTemplate, StructuralStep, StructuralTemplate,
    TemplateAction, TemplateReceipt, create_structural_template, list_structural_templates,
    review_structural_template, revise_structural_template, structural_template_history,
};
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
