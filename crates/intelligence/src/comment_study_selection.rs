//! Exact canonical comment input identity; no legacy selection policies or schedulers.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod input;
pub use input::{PreparedStudyInput, prepare_study_input};
#[cfg(test)]
mod tests;
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StudySelectionError {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("unsupported_domain")]
    UnsupportedDomain,
    #[error("invalid_limit")]
    InvalidLimit,
    #[error("invalid_selection_snapshot")]
    InvalidSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentKey {
    pub work_ref: Uuid,
    pub comment_external_id: String,
}

impl CommentKey {
    pub fn validate(&self) -> Result<(), StudySelectionError> {
        if self.work_ref.is_nil()
            || self.comment_external_id.trim().is_empty()
            || self.comment_external_id.len() > 512
            || self.comment_external_id.contains('\0')
        {
            return Err(StudySelectionError::InvalidRequest);
        }
        Ok(())
    }
}
