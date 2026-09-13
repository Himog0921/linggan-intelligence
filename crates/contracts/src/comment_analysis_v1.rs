//! Strict, provider-neutral structured output contract for one comment.
//!
//! The contract intentionally accepts only spans in the current cleaned
//! research expression. Source work and reply context may help a future model
//! interpret text, but have no representation as comment evidence here.

use core::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1: &str =
    "comment-analysis-structured-output.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommentAnalysisReferenceV1 {
    pub comment_observation_id: Uuid,
    pub comment_derivation_id: Uuid,
    pub research_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirectCommentEvidenceSpanV1 {
    /// Unicode scalar-value offsets into the direct cleaned research expression.
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommentAnalysisFindingV1 {
    pub kind: String,
    pub summary: String,
    pub evidence_spans: Vec<DirectCommentEvidenceSpanV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommentAnalysisOutcomeV1 {
    Success {
        findings: Vec<CommentAnalysisFindingV1>,
    },
    NoSignal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommentAnalysisOutputV1 {
    pub contract_version: String,
    pub comment_reference: CommentAnalysisReferenceV1,
    pub outcome: CommentAnalysisOutcomeV1,
}

/// A parsed output whose reference and every evidence span are proven against
/// the one direct comment expression supplied by the execution input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCommentAnalysisOutputV1(pub CommentAnalysisOutputV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommentAnalysisValidationErrorV1 {
    InvalidJson,
    UnsupportedContractVersion,
    CommentReferenceMismatch,
    InvalidResearchFingerprint,
    EmptyFindingSet,
    BlankFindingField,
    MissingFindingEvidence,
    InvalidEvidenceSpan,
    EvidenceSpanTextMismatch,
}

impl fmt::Display for CommentAnalysisValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => write!(formatter, "output is not valid strict V1 JSON"),
            Self::UnsupportedContractVersion => {
                write!(formatter, "output contract version is unsupported")
            }
            Self::CommentReferenceMismatch => {
                write!(
                    formatter,
                    "output comment reference does not match the frozen input"
                )
            }
            Self::InvalidResearchFingerprint => {
                write!(formatter, "research fingerprint is invalid")
            }
            Self::EmptyFindingSet => write!(
                formatter,
                "success output must contain at least one finding"
            ),
            Self::BlankFindingField => {
                write!(formatter, "finding kind and summary must not be blank")
            }
            Self::MissingFindingEvidence => {
                write!(formatter, "finding must cite direct comment evidence")
            }
            Self::InvalidEvidenceSpan => {
                write!(formatter, "evidence span is outside direct comment text")
            }
            Self::EvidenceSpanTextMismatch => write!(
                formatter,
                "evidence span text does not match direct comment text"
            ),
        }
    }
}

impl std::error::Error for CommentAnalysisValidationErrorV1 {}

/// Parses and strictly validates output for exactly one frozen Comment RunItem.
/// Unknown JSON fields are rejected, so a provider cannot sneak discussion or
/// work context in as a second evidence source.
pub fn validate_comment_analysis_json_v1(
    value: &str,
    expected_reference: &CommentAnalysisReferenceV1,
    direct_research_expression: &str,
) -> Result<ValidatedCommentAnalysisOutputV1, CommentAnalysisValidationErrorV1> {
    let output: CommentAnalysisOutputV1 =
        serde_json::from_str(value).map_err(|_| CommentAnalysisValidationErrorV1::InvalidJson)?;
    if output.contract_version != COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1 {
        return Err(CommentAnalysisValidationErrorV1::UnsupportedContractVersion);
    }
    if &output.comment_reference != expected_reference {
        return Err(CommentAnalysisValidationErrorV1::CommentReferenceMismatch);
    }
    if !is_sha256_hex(&output.comment_reference.research_fingerprint) {
        return Err(CommentAnalysisValidationErrorV1::InvalidResearchFingerprint);
    }

    if let CommentAnalysisOutcomeV1::Success { findings } = &output.outcome {
        if findings.is_empty() {
            return Err(CommentAnalysisValidationErrorV1::EmptyFindingSet);
        }
        for finding in findings {
            if finding.kind.trim().is_empty() || finding.summary.trim().is_empty() {
                return Err(CommentAnalysisValidationErrorV1::BlankFindingField);
            }
            if finding.evidence_spans.is_empty() {
                return Err(CommentAnalysisValidationErrorV1::MissingFindingEvidence);
            }
            for span in &finding.evidence_spans {
                validate_direct_span(span, direct_research_expression)?;
            }
        }
    }
    Ok(ValidatedCommentAnalysisOutputV1(output))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_direct_span(
    span: &DirectCommentEvidenceSpanV1,
    direct_research_expression: &str,
) -> Result<(), CommentAnalysisValidationErrorV1> {
    let characters = direct_research_expression.chars().collect::<Vec<_>>();
    if span.start >= span.end || span.end > characters.len() || span.text.trim().is_empty() {
        return Err(CommentAnalysisValidationErrorV1::InvalidEvidenceSpan);
    }
    let expected = characters[span.start..span.end].iter().collect::<String>();
    if expected != span.text {
        return Err(CommentAnalysisValidationErrorV1::EvidenceSpanTextMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::{
        COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1, CommentAnalysisReferenceV1,
        CommentAnalysisValidationErrorV1, validate_comment_analysis_json_v1,
    };

    fn reference() -> CommentAnalysisReferenceV1 {
        CommentAnalysisReferenceV1 {
            comment_observation_id: Uuid::new_v4(),
            comment_derivation_id: Uuid::new_v4(),
            research_fingerprint: "a".repeat(64),
        }
    }

    #[test]
    fn accepts_direct_unicode_span_for_one_matching_comment_reference() {
        let reference = reference();
        let payload = json!({
            "contract_version": COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1,
            "comment_reference": reference,
            "outcome": {"kind": "success", "findings": [{
                "kind": "question",
                "summary": "asks for a method",
                "evidence_spans": [{"start": 4, "end": 8, "text": "具体方法"}]
            }]}
        });
        validate_comment_analysis_json_v1(&payload.to_string(), &reference, "我想知道具体方法")
            .expect("direct evidence span validates");
    }

    #[test]
    fn rejects_context_as_evidence_and_reference_mismatches() {
        let reference = reference();
        let context_payload = json!({
            "contract_version": COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1,
            "comment_reference": reference,
            "outcome": {"kind": "success", "findings": [{
                "kind": "question", "summary": "forged context evidence",
                "evidence_spans": [{"start": 0, "end": 4, "text": "作品正文"}]
            }]}
        });
        assert_eq!(
            validate_comment_analysis_json_v1(&context_payload.to_string(), &reference, "同问"),
            Err(CommentAnalysisValidationErrorV1::InvalidEvidenceSpan)
        );

        let mismatched = CommentAnalysisReferenceV1 {
            comment_observation_id: Uuid::new_v4(),
            ..reference.clone()
        };
        let mismatch_payload = json!({
            "contract_version": COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1,
            "comment_reference": mismatched,
            "outcome": {"kind": "no_signal"}
        });
        assert_eq!(
            validate_comment_analysis_json_v1(&mismatch_payload.to_string(), &reference, "同问"),
            Err(CommentAnalysisValidationErrorV1::CommentReferenceMismatch)
        );
    }

    #[test]
    fn rejects_unknown_context_evidence_field() {
        let reference = reference();
        let payload = json!({
            "contract_version": COMMENT_ANALYSIS_STRUCTURED_OUTPUT_CONTRACT_V1,
            "comment_reference": reference,
            "outcome": {"kind": "no_signal"},
            "context_evidence": "reply text"
        });
        assert_eq!(
            validate_comment_analysis_json_v1(&payload.to_string(), &reference, "同问"),
            Err(CommentAnalysisValidationErrorV1::InvalidJson)
        );
    }
}
