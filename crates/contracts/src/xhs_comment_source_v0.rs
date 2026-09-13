//! XHS comment source contract V0.
//!
//! ## Boundary
//!
//! The producer currently provides `content_detail` and `comments` as
//! independent capture packages. This contract validates their minimum shared
//! identity (`noteId`) before handing a pair to downstream code. It must not be
//! read as a statement that one package contains the other, or as a persistence
//! model for Evidence or Comment entities.
//!
//! Unknown outer, record, and payload fields are retained as JSON. The contract
//! only requires fields directly demonstrated by the deidentified producer
//! fixture and needed to establish a safe source relation.

use core::fmt;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Contract identifier for the producer boundary implemented in this module.
pub const XHS_COMMENT_SOURCE_CONTRACT_V0: &str = "xhs.comment-capture-source.v0";

const XHS_PLATFORM: &str = "xhs";
const CONTENT_DETAIL_KIND: &str = "content_detail";
const COMMENTS_KIND: &str = "comments";
const COMMENT_RECORD_KIND: &str = "comment";

/// A producer capture package with unrecognized extension fields preserved.
///
/// `package_kind`, `platform`, and `records` are intentionally the only outer
/// fields required for V0 validation. The observed fixture has additional
/// fields, but V0 does not claim they are mandatory until the producer contract
/// says so.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CapturePackageV0 {
    #[serde(rename = "packageKind")]
    pub package_kind: String,
    pub platform: String,
    pub records: Vec<CaptureRecordV0>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

/// One source record. Its payload and extensions are preserved rather than
/// narrowed into a guessed product schema. `sourceObject` exists in the
/// observed fixture but is not part of V0's minimum validation boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaptureRecordV0 {
    pub kind: String,
    pub payload: Value,
    #[serde(rename = "sourceObject", default)]
    pub source_object: Option<Value>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

/// The validated role a package plays in an XHS comment source pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageRole {
    ContentDetail,
    Comments,
}

impl PackageRole {
    const fn expected_package_kind(self) -> &'static str {
        match self {
            Self::ContentDetail => CONTENT_DETAIL_KIND,
            Self::Comments => COMMENTS_KIND,
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::ContentDetail => "content detail",
            Self::Comments => "comments",
        }
    }
}

/// A content-detail package that has a verified `noteId`.
#[derive(Clone, Debug)]
pub struct ValidatedContentDetailPackageV0 {
    pub package: CapturePackageV0,
    pub note_id: String,
}

/// A comments package whose records all refer to one verified `noteId`.
#[derive(Clone, Debug)]
pub struct ValidatedCommentsPackageV0 {
    pub package: CapturePackageV0,
    pub note_id: String,
    pub comments: Vec<ValidatedCommentV0>,
}

/// The minimum comment facts validated at the producer boundary.
///
/// `raw_record` preserves the producer data for a later, explicitly designed
/// Evidence admission step. This contract does not persist it.
#[derive(Clone, Debug)]
pub struct ValidatedCommentV0 {
    pub record_index: usize,
    pub note_id: String,
    pub comment_id: String,
    pub text: String,
    pub raw_record: CaptureRecordV0,
}

/// Two independently captured packages with a verified relation through
/// `noteId`. They are not merged into a fabricated composite capture package.
#[derive(Clone, Debug)]
pub struct ValidatedXhsCommentCapturePairV0 {
    pub detail: ValidatedContentDetailPackageV0,
    pub comments: ValidatedCommentsPackageV0,
}

/// Runtime validation errors. Messages intentionally identify only structure
/// and field paths; they never echo captured text or identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceContractError {
    UnsupportedPlatform {
        role: PackageRole,
    },
    UnsupportedPackageKind {
        role: PackageRole,
    },
    EmptyRecords {
        role: PackageRole,
    },
    UnexpectedRecordKind {
        role: PackageRole,
        record_index: usize,
    },
    PayloadMustBeObject {
        role: PackageRole,
        record_index: usize,
    },
    MissingRequiredField {
        role: PackageRole,
        record_index: usize,
        field: &'static str,
    },
    BlankCommentText {
        record_index: usize,
    },
    InconsistentNoteIdWithinPackage {
        role: PackageRole,
        record_index: usize,
    },
    DuplicateCommentIdentity {
        first_record_index: usize,
        record_index: usize,
    },
    CrossPackageNoteIdMismatch,
}

impl fmt::Display for SourceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform { role } => write!(
                formatter,
                "{} package does not declare the XHS platform",
                role.display_name()
            ),
            Self::UnsupportedPackageKind { role } => write!(
                formatter,
                "{} package does not declare its required package kind",
                role.display_name()
            ),
            Self::EmptyRecords { role } => {
                write!(formatter, "{} package has no records", role.display_name())
            }
            Self::UnexpectedRecordKind { role, record_index } => write!(
                formatter,
                "{} package record {} has an unsupported record kind",
                role.display_name(),
                record_index
            ),
            Self::PayloadMustBeObject { role, record_index } => write!(
                formatter,
                "{} package record {} has a non-object payload",
                role.display_name(),
                record_index
            ),
            Self::MissingRequiredField {
                role,
                record_index,
                field,
            } => write!(
                formatter,
                "{} package record {} is missing required field {}",
                role.display_name(),
                record_index,
                field
            ),
            Self::BlankCommentText { record_index } => {
                write!(formatter, "comment record {} has blank text", record_index)
            }
            Self::InconsistentNoteIdWithinPackage { role, record_index } => write!(
                formatter,
                "{} package record {} does not match the package noteId",
                role.display_name(),
                record_index
            ),
            Self::DuplicateCommentIdentity {
                first_record_index,
                record_index,
            } => write!(
                formatter,
                "comment record {} duplicates the identity first seen at record {}",
                record_index, first_record_index
            ),
            Self::CrossPackageNoteIdMismatch => {
                write!(
                    formatter,
                    "content detail and comments packages refer to different notes"
                )
            }
        }
    }
}

impl std::error::Error for SourceContractError {}

/// Validate two independently captured producer packages before any downstream
/// Evidence admission or comment research work begins.
///
/// The sole cross-package relation is equality of their verified `noteId`.
pub fn validate_xhs_comment_capture_pair_v0(
    detail_package: CapturePackageV0,
    comments_package: CapturePackageV0,
) -> Result<ValidatedXhsCommentCapturePairV0, SourceContractError> {
    let detail = validate_content_detail_package(detail_package)?;
    let comments = validate_comments_package(comments_package)?;

    if detail.note_id != comments.note_id {
        return Err(SourceContractError::CrossPackageNoteIdMismatch);
    }

    Ok(ValidatedXhsCommentCapturePairV0 { detail, comments })
}

fn validate_content_detail_package(
    package: CapturePackageV0,
) -> Result<ValidatedContentDetailPackageV0, SourceContractError> {
    validate_package_header(&package, PackageRole::ContentDetail)?;

    let mut note_id: Option<String> = None;
    for (record_index, record) in package.records.iter().enumerate() {
        if record.kind != CONTENT_DETAIL_KIND {
            return Err(SourceContractError::UnexpectedRecordKind {
                role: PackageRole::ContentDetail,
                record_index,
            });
        }

        let payload = payload_object(record, PackageRole::ContentDetail, record_index)?;
        let candidate =
            required_non_blank_string(payload, PackageRole::ContentDetail, record_index, "noteId")?;
        require_consistent_note_id(
            &mut note_id,
            candidate,
            PackageRole::ContentDetail,
            record_index,
        )?;
    }

    // A non-empty records list and required noteId guarantee this branch.
    let note_id = note_id.expect("validated content-detail records must yield a noteId");
    Ok(ValidatedContentDetailPackageV0 { package, note_id })
}

fn validate_comments_package(
    package: CapturePackageV0,
) -> Result<ValidatedCommentsPackageV0, SourceContractError> {
    validate_package_header(&package, PackageRole::Comments)?;

    let mut package_note_id: Option<String> = None;
    let mut comments = Vec::with_capacity(package.records.len());
    let mut first_record_by_identity = HashMap::with_capacity(package.records.len());

    for (record_index, record) in package.records.iter().enumerate() {
        if record.kind != COMMENT_RECORD_KIND {
            return Err(SourceContractError::UnexpectedRecordKind {
                role: PackageRole::Comments,
                record_index,
            });
        }

        let payload = payload_object(record, PackageRole::Comments, record_index)?;
        let note_id =
            required_non_blank_string(payload, PackageRole::Comments, record_index, "noteId")?;
        let comment_id =
            required_non_blank_string(payload, PackageRole::Comments, record_index, "commentId")?;
        let text = required_comment_text(payload, record_index)?;

        require_consistent_note_id(
            &mut package_note_id,
            note_id.clone(),
            PackageRole::Comments,
            record_index,
        )?;

        if let Some(first_record_index) =
            first_record_by_identity.insert((note_id.clone(), comment_id.clone()), record_index)
        {
            return Err(SourceContractError::DuplicateCommentIdentity {
                first_record_index,
                record_index,
            });
        }

        comments.push(ValidatedCommentV0 {
            record_index,
            note_id,
            comment_id,
            text,
            raw_record: record.clone(),
        });
    }

    let note_id = package_note_id.expect("validated comment records must yield a noteId");
    Ok(ValidatedCommentsPackageV0 {
        package,
        note_id,
        comments,
    })
}

fn validate_package_header(
    package: &CapturePackageV0,
    role: PackageRole,
) -> Result<(), SourceContractError> {
    if package.platform != XHS_PLATFORM {
        return Err(SourceContractError::UnsupportedPlatform { role });
    }
    if package.package_kind != role.expected_package_kind() {
        return Err(SourceContractError::UnsupportedPackageKind { role });
    }
    if package.records.is_empty() {
        return Err(SourceContractError::EmptyRecords { role });
    }
    Ok(())
}

fn payload_object(
    record: &CaptureRecordV0,
    role: PackageRole,
    record_index: usize,
) -> Result<&Map<String, Value>, SourceContractError> {
    record
        .payload
        .as_object()
        .ok_or(SourceContractError::PayloadMustBeObject { role, record_index })
}

fn required_non_blank_string(
    payload: &Map<String, Value>,
    role: PackageRole,
    record_index: usize,
    field: &'static str,
) -> Result<String, SourceContractError> {
    let value = payload.get(field).and_then(Value::as_str).ok_or(
        SourceContractError::MissingRequiredField {
            role,
            record_index,
            field,
        },
    )?;
    if value.trim().is_empty() {
        return Err(SourceContractError::MissingRequiredField {
            role,
            record_index,
            field,
        });
    }
    Ok(value.to_owned())
}

fn required_comment_text(
    payload: &Map<String, Value>,
    record_index: usize,
) -> Result<String, SourceContractError> {
    let value = payload.get("text").and_then(Value::as_str).ok_or(
        SourceContractError::MissingRequiredField {
            role: PackageRole::Comments,
            record_index,
            field: "text",
        },
    )?;

    if value.trim().is_empty() {
        return Err(SourceContractError::BlankCommentText { record_index });
    }
    Ok(value.to_owned())
}

fn require_consistent_note_id(
    expected: &mut Option<String>,
    candidate: String,
    role: PackageRole,
    record_index: usize,
) -> Result<(), SourceContractError> {
    match expected {
        Some(current) if current != &candidate => {
            Err(SourceContractError::InconsistentNoteIdWithinPackage { role, record_index })
        }
        Some(_) => Ok(()),
        None => {
            *expected = Some(candidate);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");

    fn fixture_packages() -> (CapturePackageV0, CapturePackageV0) {
        let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture must be valid JSON");
        let packages = fixture
            .get("packages")
            .and_then(Value::as_object)
            .expect("fixture packages object");
        let detail = serde_json::from_value(packages["detailPackage"].clone())
            .expect("detail package must deserialize");
        let comments = serde_json::from_value(packages["commentsPackage"].clone())
            .expect("comments package must deserialize");
        (detail, comments)
    }

    fn fixture_value() -> Value {
        serde_json::from_str(FIXTURE).expect("fixture must be valid JSON")
    }

    fn parse_fixture_value(value: Value) -> (CapturePackageV0, CapturePackageV0) {
        let packages = value["packages"].as_object().expect("packages object");
        let detail = serde_json::from_value(packages["detailPackage"].clone())
            .expect("modified detail package must deserialize");
        let comments = serde_json::from_value(packages["commentsPackage"].clone())
            .expect("modified comments package must deserialize");
        (detail, comments)
    }

    #[test]
    fn validates_the_deidentified_producer_fixture_as_two_related_packages() {
        let (detail, comments) = fixture_packages();

        let validated = validate_xhs_comment_capture_pair_v0(detail, comments)
            .expect("fixture must meet the source contract");

        assert_eq!(validated.detail.note_id, validated.comments.note_id);
        assert!(!validated.comments.comments.is_empty());
        assert!(
            validated
                .comments
                .comments
                .iter()
                .all(|comment| comment.note_id == validated.comments.note_id)
        );
    }

    #[test]
    fn permits_missing_source_object_and_preserves_semantic_text_exactly() {
        let mut fixture = fixture_value();
        fixture["packages"]["detailPackage"]["records"][0]
            .as_object_mut()
            .expect("detail record object")
            .remove("sourceObject");
        fixture["packages"]["commentsPackage"]["records"][0]
            .as_object_mut()
            .expect("comment record object")
            .remove("sourceObject");
        fixture["packages"]["commentsPackage"]["records"][0]["payload"]["text"] =
            json!("  semantic source text  ");
        let (detail, comments) = parse_fixture_value(fixture);

        let validated = validate_xhs_comment_capture_pair_v0(detail, comments)
            .expect("sourceObject is optional and text whitespace is retained");

        assert_eq!(
            validated.comments.comments[0].text,
            "  semantic source text  "
        );
        assert!(validated.detail.package.records[0].source_object.is_none());
        assert!(
            validated.comments.comments[0]
                .raw_record
                .source_object
                .is_none()
        );
    }

    #[test]
    fn rejects_wrong_outer_package_kind() {
        let mut fixture = fixture_value();
        fixture["packages"]["detailPackage"]["packageKind"] = json!("comments");
        let (detail, comments) = parse_fixture_value(fixture);

        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::UnsupportedPackageKind {
                role: PackageRole::ContentDetail
            })
        ));
    }

    #[test]
    fn rejects_wrong_outer_platform_or_empty_records() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["platform"] = json!("not-xhs");
        let (detail, comments) = parse_fixture_value(fixture);
        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::UnsupportedPlatform {
                role: PackageRole::Comments
            })
        ));

        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"] = json!([]);
        let (detail, comments) = parse_fixture_value(fixture);
        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::EmptyRecords {
                role: PackageRole::Comments
            })
        ));
    }

    #[test]
    fn rejects_comment_missing_required_identity_or_text() {
        for field in ["noteId", "commentId", "text"] {
            let mut fixture = fixture_value();
            fixture["packages"]["commentsPackage"]["records"][0]["payload"]
                .as_object_mut()
                .expect("comment payload object")
                .remove(field);
            let (detail, comments) = parse_fixture_value(fixture);

            assert!(matches!(
                validate_xhs_comment_capture_pair_v0(detail, comments),
                Err(SourceContractError::MissingRequiredField {
                    role: PackageRole::Comments,
                    field: actual,
                    ..
                }) if actual == field
            ));
        }
    }

    #[test]
    fn rejects_blank_comment_text() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["payload"]["text"] = json!("  \n\t ");
        let (detail, comments) = parse_fixture_value(fixture);

        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::BlankCommentText { .. })
        ));
    }

    #[test]
    fn rejects_detail_without_note_id() {
        let mut fixture = fixture_value();
        fixture["packages"]["detailPackage"]["records"][0]["payload"]
            .as_object_mut()
            .expect("detail payload object")
            .remove("noteId");
        let (detail, comments) = parse_fixture_value(fixture);

        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::MissingRequiredField {
                role: PackageRole::ContentDetail,
                field: "noteId",
                ..
            })
        ));
    }

    #[test]
    fn rejects_cross_package_note_id_mismatch() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["payload"]["noteId"] =
            json!("note-id-mismatch");
        let (detail, comments) = parse_fixture_value(fixture);

        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::CrossPackageNoteIdMismatch)
        ));
    }

    #[test]
    fn rejects_duplicate_comment_identity_within_one_comments_package() {
        let mut fixture = fixture_value();
        let duplicate = fixture["packages"]["commentsPackage"]["records"][0].clone();
        fixture["packages"]["commentsPackage"]["records"]
            .as_array_mut()
            .expect("comments records array")
            .push(duplicate);
        let (detail, comments) = parse_fixture_value(fixture);

        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::DuplicateCommentIdentity {
                first_record_index: 0,
                record_index: 1,
            })
        ));
    }

    #[test]
    fn rejects_structurally_invalid_records_and_mixed_note_ids() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["kind"] = json!("reply");
        let (detail, comments) = parse_fixture_value(fixture);
        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::UnexpectedRecordKind {
                role: PackageRole::Comments,
                record_index: 0,
            })
        ));

        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["payload"] = json!(null);
        let (detail, comments) = parse_fixture_value(fixture);
        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::PayloadMustBeObject {
                role: PackageRole::Comments,
                record_index: 0,
            })
        ));

        let mut fixture = fixture_value();
        let mut second_record = fixture["packages"]["commentsPackage"]["records"][0].clone();
        second_record["payload"]["noteId"] = json!("another-note-id");
        second_record["payload"]["commentId"] = json!("another-comment-id");
        fixture["packages"]["commentsPackage"]["records"]
            .as_array_mut()
            .expect("comments records array")
            .push(second_record);
        let (detail, comments) = parse_fixture_value(fixture);
        assert!(matches!(
            validate_xhs_comment_capture_pair_v0(detail, comments),
            Err(SourceContractError::InconsistentNoteIdWithinPackage {
                role: PackageRole::Comments,
                record_index: 1,
            })
        ));
    }
}
