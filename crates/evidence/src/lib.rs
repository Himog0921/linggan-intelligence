//! Immutable admission preparation for externally observed facts.
//!
//! This crate does not persist Evidence. It turns a source package pair that
//! has passed the external contract into a narrow, deterministic admission
//! command: immutable package payloads plus stable comment facts. PostgreSQL
//! adapters own transactions and storage constraints.

use core::fmt;

use linggan_contracts::{
    CapturePackageV0, SourceContractError, ValidatedXhsCommentCapturePairV0,
    XHS_COMMENT_SOURCE_CONTRACT_V0, validate_xhs_comment_capture_pair_v0,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The only platform admitted by the V0 comment source contract.
pub const XHS_PLATFORM: &str = "xhs";

/// A stable external comment identity in the V0 source boundary.
///
/// The fields remain source identifiers. They are neither database primary
/// keys nor user-facing display values.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CommentSourceIdentityV0 {
    pub workspace_id: String,
    pub platform: String,
    pub note_id: String,
    pub comment_id: String,
}

/// An immutable package to be stored as source Evidence.
#[derive(Clone, Debug)]
pub struct PreparedSourceEvidenceV0 {
    pub package_kind: String,
    pub payload: Value,
    pub payload_sha256: String,
}

/// A comment record admitted from the producer boundary.
#[derive(Clone, Debug)]
pub struct PreparedCommentRecordV0 {
    pub source_record_index: usize,
    pub identity: CommentSourceIdentityV0,
    /// Exact producer text. Cleaning and AI must never mutate this fact.
    pub text: String,
    pub text_sha256: String,
}

/// The pure result of validating and preparing one producer package pair.
///
/// It contains no database IDs and has no side effects. A storage adapter must
/// persist both packages and all comment records in one transaction.
#[derive(Clone, Debug)]
pub struct PreparedCommentEvidenceAdmissionV0 {
    pub source_contract: &'static str,
    pub workspace_id: String,
    pub detail_evidence: PreparedSourceEvidenceV0,
    pub comments_evidence: PreparedSourceEvidenceV0,
    pub comments: Vec<PreparedCommentRecordV0>,
}

/// Closed preparation failures. Source errors retain their structural details
/// but never echo external comment text or identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidencePreparationError {
    BlankWorkspaceId,
    Source(SourceContractError),
    Serialization,
    /// PostgreSQL JSONB cannot encode U+0000. The source package is rejected
    /// as a whole, including when the character appears in an unknown extension.
    PostgresJsonbNul,
}

impl fmt::Display for EvidencePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankWorkspaceId => write!(formatter, "workspace identifier must not be blank"),
            Self::Source(error) => write!(
                formatter,
                "source contract rejected the package pair: {error}"
            ),
            Self::Serialization => write!(
                formatter,
                "validated source package could not be represented as JSON"
            ),
            Self::PostgresJsonbNul => write!(
                formatter,
                "validated source package contains a character PostgreSQL JSONB cannot store"
            ),
        }
    }
}

impl std::error::Error for EvidencePreparationError {}

/// Validates a producer pair, then prepares immutable evidence and comment
/// facts. This is the mandatory boundary before V0 PostgreSQL admission.
pub fn prepare_xhs_comment_evidence_admission_v0(
    workspace_id: impl AsRef<str>,
    detail_package: CapturePackageV0,
    comments_package: CapturePackageV0,
) -> Result<PreparedCommentEvidenceAdmissionV0, EvidencePreparationError> {
    let workspace_id = workspace_id.as_ref().trim();
    if workspace_id.is_empty() {
        return Err(EvidencePreparationError::BlankWorkspaceId);
    }

    let validated = validate_xhs_comment_capture_pair_v0(detail_package, comments_package)
        .map_err(EvidencePreparationError::Source)?;
    prepare_validated_xhs_pair(workspace_id.to_owned(), validated)
}

fn prepare_validated_xhs_pair(
    workspace_id: String,
    validated: ValidatedXhsCommentCapturePairV0,
) -> Result<PreparedCommentEvidenceAdmissionV0, EvidencePreparationError> {
    let detail_payload = serde_json::to_value(&validated.detail.package)
        .map_err(|_| EvidencePreparationError::Serialization)?;
    let comments_payload = serde_json::to_value(&validated.comments.package)
        .map_err(|_| EvidencePreparationError::Serialization)?;
    if json_contains_postgres_nul(&detail_payload) || json_contains_postgres_nul(&comments_payload)
    {
        return Err(EvidencePreparationError::PostgresJsonbNul);
    }

    let comments = validated
        .comments
        .comments
        .into_iter()
        .map(|comment| PreparedCommentRecordV0 {
            source_record_index: comment.record_index,
            identity: CommentSourceIdentityV0 {
                workspace_id: workspace_id.clone(),
                platform: XHS_PLATFORM.to_owned(),
                note_id: comment.note_id,
                comment_id: comment.comment_id,
            },
            text_sha256: sha256_text(&comment.text),
            text: comment.text,
        })
        .collect();

    Ok(PreparedCommentEvidenceAdmissionV0 {
        source_contract: XHS_COMMENT_SOURCE_CONTRACT_V0,
        workspace_id,
        detail_evidence: PreparedSourceEvidenceV0 {
            package_kind: validated.detail.package.package_kind,
            payload_sha256: sha256_json(&detail_payload),
            payload: detail_payload,
        },
        comments_evidence: PreparedSourceEvidenceV0 {
            package_kind: validated.comments.package.package_kind,
            payload_sha256: sha256_json(&comments_payload),
            payload: comments_payload,
        },
        comments,
    })
}

/// Deterministic SHA-256 over the logical JSON value. Object keys are sorted
/// recursively, so producer whitespace and key order do not create a different
/// immutable Evidence identity.
pub fn sha256_json(value: &Value) -> String {
    let mut canonical = String::new();
    append_canonical_json(value, &mut canonical);
    sha256_bytes(canonical.as_bytes())
}

/// SHA-256 of exact UTF-8 source text. There is deliberately no trim,
/// normalization, cleaning, or AI-derived representation in this hash.
pub fn sha256_text(value: &str) -> String {
    sha256_bytes(value.as_bytes())
}

fn sha256_bytes(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    format!("{digest:x}")
}

fn append_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => output.push_str(
            &serde_json::to_string(value)
                .expect("primitive serde_json::Value must serialize canonically"),
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                append_canonical_json(item, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key)
                        .expect("JSON object key must serialize canonically"),
                );
                output.push(':');
                append_canonical_json(&values[*key], output);
            }
            output.push('}');
        }
    }
}

fn json_contains_postgres_nul(value: &Value) -> bool {
    match value {
        Value::String(value) => value.contains('\0'),
        Value::Array(values) => values.iter().any(json_contains_postgres_nul),
        Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.contains('\0') || json_contains_postgres_nul(value)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use linggan_contracts::CapturePackageV0;
    use serde_json::Value;

    use super::*;

    const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");

    fn fixture_packages() -> (CapturePackageV0, CapturePackageV0) {
        let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        let packages = fixture["packages"].as_object().expect("packages object");
        (
            serde_json::from_value(packages["detailPackage"].clone()).expect("detail package"),
            serde_json::from_value(packages["commentsPackage"].clone()).expect("comments package"),
        )
    }

    #[test]
    fn prepares_only_a_validated_source_pair() {
        let (detail, comments) = fixture_packages();
        let prepared = prepare_xhs_comment_evidence_admission_v0("workspace-a", detail, comments)
            .expect("fixture must validate before preparation");

        assert_eq!(prepared.source_contract, XHS_COMMENT_SOURCE_CONTRACT_V0);
        assert_eq!(prepared.comments.len(), 1);
        assert_eq!(prepared.comments[0].identity.workspace_id, "workspace-a");
        assert_eq!(prepared.comments[0].identity.platform, XHS_PLATFORM);
        assert_eq!(prepared.detail_evidence.payload_sha256.len(), 64);
        assert_eq!(prepared.comments_evidence.payload_sha256.len(), 64);
    }

    #[test]
    fn rejects_a_blank_workspace_before_any_admission_command_exists() {
        let (detail, comments) = fixture_packages();
        assert_eq!(
            prepare_xhs_comment_evidence_admission_v0("  ", detail, comments)
                .expect_err("blank workspace must not be admitted"),
            EvidencePreparationError::BlankWorkspaceId
        );
    }

    #[test]
    fn hashes_logically_equal_json_with_different_object_key_orders_once() {
        let left = serde_json::json!({"z": [2, 1], "a": {"beta": true, "alpha": null}});
        let right = serde_json::json!({"a": {"alpha": null, "beta": true}, "z": [2, 1]});
        assert_eq!(sha256_json(&left), sha256_json(&right));
    }

    #[test]
    fn rejects_a_postgres_jsonb_nul_inside_an_unknown_source_extension() {
        let (mut detail, comments) = fixture_packages();
        detail.extensions.insert(
            "unmodeledExtension".into(),
            serde_json::json!("unsafe\u{0000}value"),
        );

        assert_eq!(
            prepare_xhs_comment_evidence_admission_v0("workspace-a", detail, comments)
                .expect_err("unknown extensions are still persisted as source JSON"),
            EvidencePreparationError::PostgresJsonbNul
        );
    }
}
