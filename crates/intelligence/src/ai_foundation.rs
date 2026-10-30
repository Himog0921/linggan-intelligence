//! AI-GEN1 execution boundaries: pure decisions and explicit persisted control.
//! A configured task is not permission to send material, nor proof its handler is connected.
use serde::{Deserialize, Serialize};
pub(crate) mod admission;
pub mod batch_control;
pub mod batch_store;
pub mod capability;
pub mod catalog;
pub mod comment_problem;
pub mod comment_research;
pub mod comment_research_overview;
pub mod control;
pub mod control_store;
mod creator_account_input;
pub mod creator_account_research;
pub mod creator_research;
pub mod executor;
mod number;
pub mod prompt;
pub mod provider_store;
pub mod rounds;
pub mod settings_inputs;
pub mod settings_read;
pub mod skill_binding;
pub mod skill_package;
pub mod skill_store;
pub mod store;
pub mod task_config;
pub mod topic_research;
mod topic_research_packet;
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
    CreationTitle,
    ModelProbe,
    OcrRecognition,
    WorkComparison,
    CommentStudy,
    CommentProblemResolution,
    CommentProblemPairReview,
    TopicResearch,
    TopicResearchExtractReview,
    TopicResearchResolve,
    TopicResearchResolveReview,
    CreatorResearch,
    CreatorAccountResearch,
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
            Self::CreationTitle => "creation_title",
            Self::ModelProbe => "model_probe",
            Self::OcrRecognition => "ocr_recognition",
            Self::WorkComparison => "work_comparison",
            Self::CommentStudy => "comment_study",
            Self::CommentProblemResolution => "comment_problem_resolution",
            Self::CommentProblemPairReview => "comment_problem_pair_review",
            Self::TopicResearch => "topic_research",
            Self::TopicResearchExtractReview => "topic_research_extract_review",
            Self::TopicResearchResolve => "topic_research_resolve",
            Self::TopicResearchResolveReview => "topic_research_resolve_review",
            Self::CreatorResearch => "creator_research",
            Self::CreatorAccountResearch => "creator_account_research",
            Self::CommentEmbedding => "comment_embedding",
            Self::LegacyCreatorDiscovery => "legacy_creator_discovery",
            Self::LegacyTopicMap => "legacy_topic_map",
            Self::LegacyUnclassified => "legacy_unclassified",
        }
    }
    pub(crate) fn comment_stage(self) -> Option<u8> {
        match self {
            Self::CommentStudy => Some(0),
            Self::CommentProblemResolution => Some(1),
            Self::CommentProblemPairReview => Some(2),
            _ => None,
        }
    }
    pub(crate) fn topic_stage(self) -> Option<u8> {
        match self {
            Self::TopicResearch => Some(0),
            Self::TopicResearchExtractReview => Some(1),
            Self::TopicResearchResolve => Some(2),
            Self::TopicResearchResolveReview => Some(3),
            _ => None,
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
