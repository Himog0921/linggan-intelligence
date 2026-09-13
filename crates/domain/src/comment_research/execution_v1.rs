//! Pure V1 execution-input semantics for Comment Research.
//!
//! This module has no database, queue, provider, model, or prompt dependency.
//! It creates a deterministic, inspectable snapshot boundary and the semantic
//! fingerprint used by later storage and execution work.

use core::fmt;

use sha2::{Digest, Sha256};

use super::{
    CommentResearchContextPackV1, ContextPackOmissionV1, ContextPackPreviewSourceTextV1,
    ContextPackReadinessV1,
};

/// Version of the semantic input-hash algorithm.
pub const COMMENT_RESEARCH_FINGERPRINT_V1: &str = "comment-research-fingerprint.v1";
/// The V1 research behavior contract whose changes must create a new input.
pub const COMMENT_RESEARCH_EXECUTION_CONTRACT_V1: &str = "comment-research-execution.v1";
/// The structured output contract that later execution must validate.
pub const COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1: &str = "comment-analysis-structured-output.v1";
/// The explicit format of the stored source-backed input snapshot.
pub const COMMENT_RESEARCH_INPUT_SNAPSHOT_VERSION_V1: &str = "comment-research-input-snapshot.v1";

/// A strategy is an input semantic, even when no provider is configured or
/// called. It deliberately identifies behavior rather than an account, URL,
/// token, or provider request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchModelStrategyV1 {
    pub strategy_id: String,
    pub strategy_version: String,
}

/// Whether the frozen source-backed pack may proceed to a later execution
/// adapter. A `NeedsContext` cleaning outcome is never silently upgraded merely
/// because the preview happened to contain some surrounding text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextSufficiencyV1 {
    SufficientForExecution,
    BlockedNeedsContext,
}

impl ContextSufficiencyV1 {
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::SufficientForExecution => "sufficient",
            Self::BlockedNeedsContext => "insufficient_needs_context",
        }
    }

    pub const fn initial_execution_state(self) -> &'static str {
        match self {
            Self::SufficientForExecution => "prepared",
            Self::BlockedNeedsContext => "blocked",
        }
    }

    pub const fn initial_failure_code(self) -> Option<&'static str> {
        match self {
            Self::SufficientForExecution => None,
            Self::BlockedNeedsContext => Some("needs_context_insufficient"),
        }
    }
}

/// An immutable source-backed input snapshot. `text` is not a provider prompt:
/// it is a length-delimited audit representation of the exact V1 pack content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrozenCommentResearchContextPackV1 {
    pub version: &'static str,
    pub text: String,
    pub integrity_sha256: String,
    pub context_sufficiency: ContextSufficiencyV1,
}

/// Inputs whose every byte has semantic meaning for V1 research reuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchFingerprintInputV1 {
    pub cleaned_research_text: String,
    pub frozen_context_pack: FrozenCommentResearchContextPackV1,
    pub cleaning_contract: String,
    pub research_contract: String,
    pub output_schema: String,
    pub model_strategy: ResearchModelStrategyV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResearchFingerprintErrorV1 {
    BlankField { field: &'static str },
    InvalidFrozenPackVersion,
    InvalidFrozenPackIntegrity,
}

impl fmt::Display for ResearchFingerprintErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankField { field } => write!(formatter, "{field} must not be blank"),
            Self::InvalidFrozenPackVersion => {
                write!(
                    formatter,
                    "frozen Context Pack does not use the V1 snapshot version"
                )
            }
            Self::InvalidFrozenPackIntegrity => {
                write!(
                    formatter,
                    "frozen Context Pack integrity hash does not match its text"
                )
            }
        }
    }
}

impl std::error::Error for ResearchFingerprintErrorV1 {}

/// Freezes the currently assembled source-backed Context Pack. The snapshot is
/// retained even for a blocked `needs_context` item so the block remains
/// explainable, but only `SufficientForExecution` may enter a later queue.
pub fn freeze_comment_research_context_pack_v1(
    pack: &CommentResearchContextPackV1,
) -> FrozenCommentResearchContextPackV1 {
    let text = render_frozen_context_pack_v1(pack);
    let integrity_sha256 = sha256_hex(&text);
    let context_sufficiency = match pack.readiness {
        ContextPackReadinessV1::Ready => ContextSufficiencyV1::SufficientForExecution,
        ContextPackReadinessV1::NeedsContext => ContextSufficiencyV1::BlockedNeedsContext,
    };
    FrozenCommentResearchContextPackV1 {
        version: COMMENT_RESEARCH_INPUT_SNAPSHOT_VERSION_V1,
        text,
        integrity_sha256,
        context_sufficiency,
    }
}

/// Produces a SHA-256 V1 research fingerprint from the semantic input only.
/// Observation and Evidence identifiers are intentionally excluded: equal,
/// valid semantic inputs can be reused across source records.
pub fn research_fingerprint_v1(
    input: &ResearchFingerprintInputV1,
) -> Result<String, ResearchFingerprintErrorV1> {
    ensure_not_blank("cleaned_research_text", &input.cleaned_research_text)?;
    ensure_not_blank("cleaning_contract", &input.cleaning_contract)?;
    ensure_not_blank("research_contract", &input.research_contract)?;
    ensure_not_blank("output_schema", &input.output_schema)?;
    ensure_not_blank(
        "model_strategy.strategy_id",
        &input.model_strategy.strategy_id,
    )?;
    ensure_not_blank(
        "model_strategy.strategy_version",
        &input.model_strategy.strategy_version,
    )?;
    if input.frozen_context_pack.version != COMMENT_RESEARCH_INPUT_SNAPSHOT_VERSION_V1 {
        return Err(ResearchFingerprintErrorV1::InvalidFrozenPackVersion);
    }
    if sha256_hex(&input.frozen_context_pack.text) != input.frozen_context_pack.integrity_sha256 {
        return Err(ResearchFingerprintErrorV1::InvalidFrozenPackIntegrity);
    }

    let mut hasher = Sha256::new();
    write_semantic_field(
        &mut hasher,
        "fingerprint_version",
        COMMENT_RESEARCH_FINGERPRINT_V1,
    );
    write_semantic_field(
        &mut hasher,
        "cleaned_research_text",
        &input.cleaned_research_text,
    );
    write_semantic_field(
        &mut hasher,
        "context_pack_version",
        input.frozen_context_pack.version,
    );
    write_semantic_field(
        &mut hasher,
        "context_pack_text",
        &input.frozen_context_pack.text,
    );
    write_semantic_field(&mut hasher, "cleaning_contract", &input.cleaning_contract);
    write_semantic_field(&mut hasher, "research_contract", &input.research_contract);
    write_semantic_field(&mut hasher, "output_schema", &input.output_schema);
    write_semantic_field(
        &mut hasher,
        "model_strategy_id",
        &input.model_strategy.strategy_id,
    );
    write_semantic_field(
        &mut hasher,
        "model_strategy_version",
        &input.model_strategy.strategy_version,
    );
    Ok(format!("{:x}", hasher.finalize()))
}

fn ensure_not_blank(field: &'static str, value: &str) -> Result<(), ResearchFingerprintErrorV1> {
    if value.trim().is_empty() {
        return Err(ResearchFingerprintErrorV1::BlankField { field });
    }
    Ok(())
}

fn write_semantic_field(hasher: &mut Sha256, name: &str, value: &str) {
    hasher.update(name.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes().len().to_string().as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    hasher.update([0]);
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn render_frozen_context_pack_v1(pack: &CommentResearchContextPackV1) -> String {
    let mut rendered = String::from("COMMENT_RESEARCH_INPUT_SNAPSHOT\n");
    append_field(
        &mut rendered,
        "version",
        COMMENT_RESEARCH_INPUT_SNAPSHOT_VERSION_V1,
    );
    append_field(
        &mut rendered,
        "readiness",
        match pack.readiness {
            ContextPackReadinessV1::Ready => "ready",
            ContextPackReadinessV1::NeedsContext => "needs_context",
        },
    );
    append_field(
        &mut rendered,
        "source_backed_context",
        if pack.has_source_backed_context {
            "true"
        } else {
            "false"
        },
    );
    append_field(
        &mut rendered,
        "direct_comment_evidence",
        &pack.direct_comment_evidence.text,
    );
    append_field(
        &mut rendered,
        "discussion_count",
        &pack.discussion_context.len().to_string(),
    );
    for (index, discussion) in pack.discussion_context.iter().enumerate() {
        append_field(
            &mut rendered,
            &format!("discussion.{index}.text"),
            &discussion.text.text,
        );
        append_field(
            &mut rendered,
            &format!("discussion.{index}.truncated"),
            if discussion.text.truncated {
                "true"
            } else {
                "false"
            },
        );
        append_field(
            &mut rendered,
            &format!("discussion.{index}.root_comment"),
            if discussion.root_comment {
                "true"
            } else {
                "false"
            },
        );
        append_field(
            &mut rendered,
            &format!("discussion.{index}.parent_comment"),
            if discussion.parent_comment {
                "true"
            } else {
                "false"
            },
        );
        append_field(
            &mut rendered,
            &format!("discussion.{index}.reply_to_comment"),
            if discussion.reply_to_comment {
                "true"
            } else {
                "false"
            },
        );
    }
    append_source_text(&mut rendered, "work_title", &pack.work_title);
    append_source_text(&mut rendered, "work_body", &pack.work_body);
    append_field(
        &mut rendered,
        "omission_count",
        &pack.omissions.len().to_string(),
    );
    for (index, omission) in pack.omissions.iter().enumerate() {
        append_field(
            &mut rendered,
            &format!("omission.{index}"),
            omission_name(*omission),
        );
    }
    rendered
}

fn append_source_text(rendered: &mut String, name: &str, value: &ContextPackPreviewSourceTextV1) {
    match value {
        ContextPackPreviewSourceTextV1::Observed(value) => {
            append_field(rendered, &format!("{name}.availability"), "observed");
            append_field(rendered, &format!("{name}.text"), &value.text);
            append_field(
                rendered,
                &format!("{name}.truncated"),
                if value.truncated { "true" } else { "false" },
            );
        }
        ContextPackPreviewSourceTextV1::Blank => {
            append_field(rendered, &format!("{name}.availability"), "blank");
        }
        ContextPackPreviewSourceTextV1::Unavailable => {
            append_field(rendered, &format!("{name}.availability"), "unavailable");
        }
    }
}

fn append_field(rendered: &mut String, name: &str, value: &str) {
    rendered.push_str(name);
    rendered.push('=');
    rendered.push_str(&value.as_bytes().len().to_string());
    rendered.push('\n');
    rendered.push_str(value);
    rendered.push('\n');
}

fn omission_name(value: ContextPackOmissionV1) -> &'static str {
    match value {
        ContextPackOmissionV1::DirectEvidenceTruncated => "direct_evidence_truncated",
        ContextPackOmissionV1::DiscussionExcerptTruncated => "discussion_excerpt_truncated",
        ContextPackOmissionV1::DiscussionItemLimitReached => "discussion_item_limit_reached",
        ContextPackOmissionV1::WorkTitleTruncated => "work_title_truncated",
        ContextPackOmissionV1::WorkBodyTruncated => "work_body_truncated",
        ContextPackOmissionV1::SourceBackedContextUnavailable => {
            "source_backed_context_unavailable"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1, COMMENT_RESEARCH_EXECUTION_CONTRACT_V1,
        ResearchFingerprintInputV1, ResearchModelStrategyV1,
        freeze_comment_research_context_pack_v1, research_fingerprint_v1,
    };
    use crate::comment_research::{
        CommentResearchContextPackInputV1, ContextPackReadinessV1, ContextPackSourceTextV1,
        build_comment_research_context_pack_v1,
    };

    fn fingerprint_for(expression: &str, work_body: &str) -> String {
        let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
            research_expression: expression.to_owned(),
            readiness: ContextPackReadinessV1::Ready,
            has_source_backed_context: true,
            related_discussion: Vec::new(),
            work_title: ContextPackSourceTextV1::Observed("标题".to_owned()),
            work_body: ContextPackSourceTextV1::Observed(work_body.to_owned()),
        });
        let frozen = freeze_comment_research_context_pack_v1(&pack);
        research_fingerprint_v1(&ResearchFingerprintInputV1 {
            cleaned_research_text: expression.to_owned(),
            frozen_context_pack: frozen,
            cleaning_contract: "comment-cleaning.v1".to_owned(),
            research_contract: COMMENT_RESEARCH_EXECUTION_CONTRACT_V1.to_owned(),
            output_schema: COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1.to_owned(),
            model_strategy: ResearchModelStrategyV1 {
                strategy_id: "deterministic-contract-test".to_owned(),
                strategy_version: "v1".to_owned(),
            },
        })
        .expect("valid semantic input fingerprints")
    }

    #[test]
    fn same_effective_input_has_the_same_fingerprint_without_an_observation_id() {
        assert_eq!(
            fingerprint_for("我想知道具体方法", "作品正文"),
            fingerprint_for("我想知道具体方法", "作品正文")
        );
    }

    #[test]
    fn changed_research_expression_or_context_changes_the_fingerprint() {
        let original = fingerprint_for("我想知道具体方法", "作品正文");
        assert_ne!(original, fingerprint_for("我想知道具体方法吗", "作品正文"));
        assert_ne!(
            original,
            fingerprint_for("我想知道具体方法", "不同作品正文")
        );
    }

    #[test]
    fn needs_context_remains_frozen_but_is_blocked_from_execution() {
        let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
            research_expression: "同问".to_owned(),
            readiness: ContextPackReadinessV1::NeedsContext,
            has_source_backed_context: true,
            related_discussion: Vec::new(),
            work_title: ContextPackSourceTextV1::Observed("作品标题".to_owned()),
            work_body: ContextPackSourceTextV1::Unavailable,
        });
        let frozen = freeze_comment_research_context_pack_v1(&pack);
        assert_eq!(
            frozen.context_sufficiency.as_storage_value(),
            "insufficient_needs_context"
        );
        assert_eq!(
            frozen.context_sufficiency.initial_execution_state(),
            "blocked"
        );
        assert_eq!(
            frozen.context_sufficiency.initial_failure_code(),
            Some("needs_context_insufficient")
        );
        assert!(
            frozen.text.contains("direct_comment_evidence=6\n同问"),
            "the source-backed snapshot is still retained to explain the block"
        );
    }
}
