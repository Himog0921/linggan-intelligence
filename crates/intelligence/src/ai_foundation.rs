//! AI-GEN1 execution boundaries: pure decisions and explicit persisted control.
//! A configured task is not permission to send material, nor proof its handler is connected.
use serde::{Deserialize, Serialize};
pub(crate) mod admission;
pub mod batch_control;
pub mod batch_store;
pub mod capability;
pub mod catalog;
pub mod control;
pub mod control_store;
pub mod executor;
mod number;
pub mod provider_store;
pub mod rounds;
pub mod settings_inputs;
pub mod settings_read;
pub mod skill_binding;
pub mod skill_package;
pub mod skill_store;
pub mod store;
pub mod task_config;
pub mod transport;
pub mod transport_store;
pub mod usage;
pub mod work_comparison;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleError {
    #[error("ai_invalid_usage")]
    Usage,
    #[error("ai_arithmetic_overflow")]
    Overflow,
    #[error("ai_invalid_capability")]
    Capability,
    #[error("ai_capacity_unavailable")]
    Capacity,
    #[error("ai_thinking_level_unsupported")]
    Thinking,
    #[error("ai_invalid_round")]
    Round,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKey {
    NoteInterpretation,
    DirectionListDraft,
    DirectionProposal,
    AuthorOrgIdentity,
    WritingAssistant,
    WritingLegacy,
    ModelProbe,
    OcrQualityAudit,
    OcrRecognition,
    WorkComparison,
    CommentStudy,
    CommentEmbedding,
    LegacyCreatorDiscovery,
    LegacyTopicMap,
    LegacyUnclassified,
}
impl TaskKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoteInterpretation => "note_interpretation",
            Self::DirectionListDraft => "direction_list_draft",
            Self::DirectionProposal => "direction_proposal",
            Self::AuthorOrgIdentity => "author_org_identity",
            Self::WritingAssistant => "writing_assistant",
            Self::WritingLegacy => "writing_legacy",
            Self::ModelProbe => "model_probe",
            Self::OcrQualityAudit => "ocr_quality_audit",
            Self::OcrRecognition => "ocr_recognition",
            Self::WorkComparison => "work_comparison",
            Self::CommentStudy => "comment_study",
            Self::CommentEmbedding => "comment_embedding",
            Self::LegacyCreatorDiscovery => "legacy_creator_discovery",
            Self::LegacyTopicMap => "legacy_topic_map",
            Self::LegacyUnclassified => "legacy_unclassified",
        }
    }
    pub fn uses_automatic_brake(self) -> bool {
        matches!(
            self,
            Self::NoteInterpretation
                | Self::DirectionListDraft
                | Self::DirectionProposal
                | Self::AuthorOrgIdentity
        )
    }
}
pub mod invocation_read;
pub mod ocr_recognition;
