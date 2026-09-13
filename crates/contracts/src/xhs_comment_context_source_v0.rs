//! XHS comment context source contract V0.
//!
//! ## Boundary
//!
//! This contract validates three *independent* producer packages:
//! `content_detail`, `comments`, and `replies`. The only relation it establishes
//! across them is a shared `noteId`. It deliberately does not fabricate a
//! complete comment tree, infer missing parent links, or turn work context into
//! evidence asserted by the current commenter.
//!
//! `title` and `bodyText` communicate a three-state source fact: not supplied,
//! supplied but blank, or observed text. Consumers must preserve that distinction
//! rather than turning unavailable context into an empty-string default.

use core::fmt;
use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::{CapturePackageV0, CaptureRecordV0};

/// Contract identifier for the bounded context producer boundary.
pub const XHS_COMMENT_CONTEXT_SOURCE_CONTRACT_V0: &str = "xhs.comment-context-source.v0";

const XHS_PLATFORM: &str = "xhs";
const CONTENT_DETAIL_PACKAGE_KIND: &str = "content_detail";
const COMMENTS_PACKAGE_KIND: &str = "comments";
const REPLIES_PACKAGE_KIND: &str = "replies";
const CONTENT_DETAIL_RECORD_KIND: &str = "content_detail";
const COMMENT_RECORD_KIND: &str = "comment";
const REPLY_RECORD_KIND: &str = "reply";

/// The role an independently captured package plays in this context set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPackageRoleV0 {
    ContentDetail,
    Comments,
    Replies,
}

impl ContextPackageRoleV0 {
    const fn expected_package_kind(self) -> &'static str {
        match self {
            Self::ContentDetail => CONTENT_DETAIL_PACKAGE_KIND,
            Self::Comments => COMMENTS_PACKAGE_KIND,
            Self::Replies => REPLIES_PACKAGE_KIND,
        }
    }

    const fn expected_record_kind(self) -> &'static str {
        match self {
            Self::ContentDetail => CONTENT_DETAIL_RECORD_KIND,
            Self::Comments => COMMENT_RECORD_KIND,
            Self::Replies => REPLY_RECORD_KIND,
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::ContentDetail => "content detail",
            Self::Comments => "comments",
            Self::Replies => "replies",
        }
    }
}

/// The provenance-aware availability of optional text from a work detail.
///
/// This is not a generic text normalization type. In particular, an observed
/// whitespace-only string is distinct from a producer omitting the field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextTextAvailabilityV0 {
    /// The producer did not provide a text value (field absent or JSON null).
    Unavailable,
    /// The producer provided a string, but it contains no non-whitespace text.
    Blank,
    /// Exact observed source text. No trimming or cleaning has been applied.
    Observed(String),
}

/// Optional work context extracted from exactly one `content_detail` record.
///
/// This context can help downstream code understand referents in a comment. It
/// must never be recorded as a direct claim made by that comment's author.
#[derive(Clone, Debug)]
pub struct ValidatedWorkContextV0 {
    pub record_index: usize,
    pub note_id: String,
    pub title: ContextTextAvailabilityV0,
    pub body_text: ContextTextAvailabilityV0,
    pub author_id: Option<String>,
    pub raw_record: CaptureRecordV0,
}

/// A validated content-detail package with one bounded work context record.
#[derive(Clone, Debug)]
pub struct ValidatedContentDetailContextPackageV0 {
    pub package: CapturePackageV0,
    pub work_context: ValidatedWorkContextV0,
}

/// A comment extracted from a `comments` package.
#[derive(Clone, Debug)]
pub struct ValidatedContextCommentV0 {
    pub record_index: usize,
    pub note_id: String,
    pub comment_id: String,
    /// Exact source text. This contract performs no cleaning.
    pub text: String,
    pub author_id: Option<String>,
    pub raw_record: CaptureRecordV0,
}

/// A validated comments package.
#[derive(Clone, Debug)]
pub struct ValidatedContextCommentsPackageV0 {
    pub package: CapturePackageV0,
    pub note_id: String,
    pub comments: Vec<ValidatedContextCommentV0>,
}

/// A reply extracted from a `replies` package.
///
/// `parent_comment_id` and `reply_to_comment_id` intentionally remain separate.
/// Neither is a fallback for the other, and both may be supplied on one reply.
#[derive(Clone, Debug)]
pub struct ValidatedReplyV0 {
    pub record_index: usize,
    pub note_id: String,
    pub comment_id: String,
    /// Exact source text. This contract performs no cleaning.
    pub text: String,
    pub author_id: Option<String>,
    pub root_comment_id: Option<String>,
    pub parent_comment_id: Option<String>,
    pub reply_to_comment_id: Option<String>,
    pub raw_record: CaptureRecordV0,
}

/// A validated replies package.
#[derive(Clone, Debug)]
pub struct ValidatedRepliesPackageV0 {
    pub package: CapturePackageV0,
    pub note_id: String,
    pub replies: Vec<ValidatedReplyV0>,
}

/// Three independent producer packages linked only by their verified `noteId`.
#[derive(Clone, Debug)]
pub struct ValidatedXhsCommentContextCaptureSetV0 {
    pub detail: ValidatedContentDetailContextPackageV0,
    pub comments: ValidatedContextCommentsPackageV0,
    pub replies: ValidatedRepliesPackageV0,
}

/// Closed runtime errors for the bounded source contract.
///
/// Messages deliberately describe structure only. They never echo source text,
/// source identifiers, or raw producer payload values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextSourceContractErrorV0 {
    MalformedPackage {
        role: ContextPackageRoleV0,
    },
    UnsupportedPlatform {
        role: ContextPackageRoleV0,
    },
    UnsupportedPackageKind {
        role: ContextPackageRoleV0,
    },
    EmptyRecords {
        role: ContextPackageRoleV0,
    },
    MultipleContentDetailRecords,
    UnexpectedRecordKind {
        role: ContextPackageRoleV0,
        record_index: usize,
    },
    PayloadMustBeObject {
        role: ContextPackageRoleV0,
        record_index: usize,
    },
    MissingRequiredField {
        role: ContextPackageRoleV0,
        record_index: usize,
        field: &'static str,
    },
    BlankRequiredText {
        role: ContextPackageRoleV0,
        record_index: usize,
        field: &'static str,
    },
    OptionalFieldMustBeStringOrNull {
        role: ContextPackageRoleV0,
        record_index: usize,
        field: &'static str,
    },
    SourceObjectMustBeObject {
        role: ContextPackageRoleV0,
        record_index: usize,
    },
    SourceObjectMissingRequiredField {
        role: ContextPackageRoleV0,
        record_index: usize,
        field: &'static str,
    },
    SourceObjectPlatformMismatch {
        role: ContextPackageRoleV0,
        record_index: usize,
    },
    InconsistentNoteIdWithinPackage {
        role: ContextPackageRoleV0,
        record_index: usize,
    },
    DuplicateCommentIdentity {
        role: ContextPackageRoleV0,
        first_record_index: usize,
        record_index: usize,
    },
    CrossPackageNoteIdMismatch {
        role: ContextPackageRoleV0,
    },
}

impl fmt::Display for ContextSourceContractErrorV0 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPackage { role } => write!(
                formatter,
                "{} package cannot be parsed as the source contract",
                role.display_name()
            ),
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
            Self::MultipleContentDetailRecords => write!(
                formatter,
                "content detail package has more than one record and cannot yield one bounded work context"
            ),
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
            Self::BlankRequiredText {
                role,
                record_index,
                field,
            } => write!(
                formatter,
                "{} package record {} has blank required text field {}",
                role.display_name(),
                record_index,
                field
            ),
            Self::OptionalFieldMustBeStringOrNull {
                role,
                record_index,
                field,
            } => write!(
                formatter,
                "{} package record {} has an invalid optional field {}",
                role.display_name(),
                record_index,
                field
            ),
            Self::SourceObjectMustBeObject { role, record_index } => write!(
                formatter,
                "{} package record {} has a non-object sourceObject",
                role.display_name(),
                record_index
            ),
            Self::SourceObjectMissingRequiredField {
                role,
                record_index,
                field,
            } => write!(
                formatter,
                "{} package record {} sourceObject is missing required field {}",
                role.display_name(),
                record_index,
                field
            ),
            Self::SourceObjectPlatformMismatch { role, record_index } => write!(
                formatter,
                "{} package record {} sourceObject does not declare the XHS platform",
                role.display_name(),
                record_index
            ),
            Self::InconsistentNoteIdWithinPackage { role, record_index } => write!(
                formatter,
                "{} package record {} does not match the package noteId",
                role.display_name(),
                record_index
            ),
            Self::DuplicateCommentIdentity {
                role,
                first_record_index,
                record_index,
            } => write!(
                formatter,
                "{} package record {} duplicates the identity first seen at record {}",
                role.display_name(),
                record_index,
                first_record_index
            ),
            Self::CrossPackageNoteIdMismatch { role } => write!(
                formatter,
                "{} package does not match the content detail package noteId",
                role.display_name()
            ),
        }
    }
}

impl std::error::Error for ContextSourceContractErrorV0 {}

/// Parse three individual producer package JSON values and then validate them.
///
/// The function intentionally does not parse the fixture envelope. That
/// envelope is test material, not an asserted producer wire protocol.
pub fn parse_and_validate_xhs_comment_context_capture_set_v0(
    detail_package: Value,
    comments_package: Value,
    replies_package: Value,
) -> Result<ValidatedXhsCommentContextCaptureSetV0, ContextSourceContractErrorV0> {
    let detail = serde_json::from_value(detail_package).map_err(|_| {
        ContextSourceContractErrorV0::MalformedPackage {
            role: ContextPackageRoleV0::ContentDetail,
        }
    })?;
    let comments = serde_json::from_value(comments_package).map_err(|_| {
        ContextSourceContractErrorV0::MalformedPackage {
            role: ContextPackageRoleV0::Comments,
        }
    })?;
    let replies = serde_json::from_value(replies_package).map_err(|_| {
        ContextSourceContractErrorV0::MalformedPackage {
            role: ContextPackageRoleV0::Replies,
        }
    })?;

    validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
}

/// Validate three independently captured packages before any downstream
/// context packing or research work begins.
pub fn validate_xhs_comment_context_capture_set_v0(
    detail_package: CapturePackageV0,
    comments_package: CapturePackageV0,
    replies_package: CapturePackageV0,
) -> Result<ValidatedXhsCommentContextCaptureSetV0, ContextSourceContractErrorV0> {
    let detail = validate_detail_package(detail_package)?;
    let comments = validate_comments_package(comments_package)?;
    let replies = validate_replies_package(replies_package)?;

    if detail.work_context.note_id != comments.note_id {
        return Err(ContextSourceContractErrorV0::CrossPackageNoteIdMismatch {
            role: ContextPackageRoleV0::Comments,
        });
    }
    if detail.work_context.note_id != replies.note_id {
        return Err(ContextSourceContractErrorV0::CrossPackageNoteIdMismatch {
            role: ContextPackageRoleV0::Replies,
        });
    }

    Ok(ValidatedXhsCommentContextCaptureSetV0 {
        detail,
        comments,
        replies,
    })
}

fn validate_detail_package(
    package: CapturePackageV0,
) -> Result<ValidatedContentDetailContextPackageV0, ContextSourceContractErrorV0> {
    let role = ContextPackageRoleV0::ContentDetail;
    validate_package_header(&package, role)?;
    if package.records.len() != 1 {
        return Err(ContextSourceContractErrorV0::MultipleContentDetailRecords);
    }

    let work_context = {
        let record = &package.records[0];
        validate_record_kind(record, role, 0)?;
        validate_source_object(record, role, 0)?;
        let payload = payload_object(record, role, 0)?;
        let note_id = required_non_blank_string(payload, role, 0, "noteId")?;

        ValidatedWorkContextV0 {
            record_index: 0,
            note_id,
            title: optional_context_text(payload, role, 0, "title")?,
            body_text: optional_context_text(payload, role, 0, "bodyText")?,
            author_id: optional_identifier(payload, role, 0, "authorId")?,
            raw_record: record.clone(),
        }
    };

    Ok(ValidatedContentDetailContextPackageV0 {
        package,
        work_context,
    })
}

fn validate_comments_package(
    package: CapturePackageV0,
) -> Result<ValidatedContextCommentsPackageV0, ContextSourceContractErrorV0> {
    let role = ContextPackageRoleV0::Comments;
    validate_package_header(&package, role)?;

    let mut note_id = None;
    let mut first_record_by_identity = HashMap::with_capacity(package.records.len());
    let mut comments = Vec::with_capacity(package.records.len());

    for (record_index, record) in package.records.iter().enumerate() {
        validate_record_kind(record, role, record_index)?;
        validate_source_object(record, role, record_index)?;
        let payload = payload_object(record, role, record_index)?;
        let current_note_id = required_non_blank_string(payload, role, record_index, "noteId")?;
        let comment_id = required_non_blank_string(payload, role, record_index, "commentId")?;
        let text = required_non_blank_text(payload, role, record_index, "text")?;
        require_consistent_note_id(&mut note_id, current_note_id.clone(), role, record_index)?;

        if let Some(first_record_index) = first_record_by_identity
            .insert((current_note_id.clone(), comment_id.clone()), record_index)
        {
            return Err(ContextSourceContractErrorV0::DuplicateCommentIdentity {
                role,
                first_record_index,
                record_index,
            });
        }

        comments.push(ValidatedContextCommentV0 {
            record_index,
            note_id: current_note_id,
            comment_id,
            text,
            author_id: optional_identifier(payload, role, record_index, "authorId")?,
            raw_record: record.clone(),
        });
    }

    Ok(ValidatedContextCommentsPackageV0 {
        package,
        note_id: note_id.expect("non-empty validated comments package must produce a noteId"),
        comments,
    })
}

fn validate_replies_package(
    package: CapturePackageV0,
) -> Result<ValidatedRepliesPackageV0, ContextSourceContractErrorV0> {
    let role = ContextPackageRoleV0::Replies;
    validate_package_header(&package, role)?;

    let mut note_id = None;
    let mut first_record_by_identity = HashMap::with_capacity(package.records.len());
    let mut replies = Vec::with_capacity(package.records.len());

    for (record_index, record) in package.records.iter().enumerate() {
        validate_record_kind(record, role, record_index)?;
        validate_source_object(record, role, record_index)?;
        let payload = payload_object(record, role, record_index)?;
        let current_note_id = required_non_blank_string(payload, role, record_index, "noteId")?;
        let comment_id = required_non_blank_string(payload, role, record_index, "commentId")?;
        let text = required_non_blank_text(payload, role, record_index, "text")?;
        require_consistent_note_id(&mut note_id, current_note_id.clone(), role, record_index)?;

        if let Some(first_record_index) = first_record_by_identity
            .insert((current_note_id.clone(), comment_id.clone()), record_index)
        {
            return Err(ContextSourceContractErrorV0::DuplicateCommentIdentity {
                role,
                first_record_index,
                record_index,
            });
        }

        replies.push(ValidatedReplyV0 {
            record_index,
            note_id: current_note_id,
            comment_id,
            text,
            author_id: optional_identifier(payload, role, record_index, "authorId")?,
            root_comment_id: optional_identifier(payload, role, record_index, "rootCommentId")?,
            parent_comment_id: optional_identifier(payload, role, record_index, "parentCommentId")?,
            reply_to_comment_id: optional_identifier(
                payload,
                role,
                record_index,
                "replyToCommentId",
            )?,
            raw_record: record.clone(),
        });
    }

    Ok(ValidatedRepliesPackageV0 {
        package,
        note_id: note_id.expect("non-empty validated replies package must produce a noteId"),
        replies,
    })
}

fn validate_package_header(
    package: &CapturePackageV0,
    role: ContextPackageRoleV0,
) -> Result<(), ContextSourceContractErrorV0> {
    if package.platform != XHS_PLATFORM {
        return Err(ContextSourceContractErrorV0::UnsupportedPlatform { role });
    }
    if package.package_kind != role.expected_package_kind() {
        return Err(ContextSourceContractErrorV0::UnsupportedPackageKind { role });
    }
    if package.records.is_empty() {
        return Err(ContextSourceContractErrorV0::EmptyRecords { role });
    }
    Ok(())
}

fn validate_record_kind(
    record: &CaptureRecordV0,
    role: ContextPackageRoleV0,
    record_index: usize,
) -> Result<(), ContextSourceContractErrorV0> {
    if record.kind != role.expected_record_kind() {
        return Err(ContextSourceContractErrorV0::UnexpectedRecordKind { role, record_index });
    }
    Ok(())
}

fn validate_source_object(
    record: &CaptureRecordV0,
    role: ContextPackageRoleV0,
    record_index: usize,
) -> Result<(), ContextSourceContractErrorV0> {
    let Some(source_object) = &record.source_object else {
        return Ok(());
    };
    let source_object = source_object
        .as_object()
        .ok_or(ContextSourceContractErrorV0::SourceObjectMustBeObject { role, record_index })?;

    for field in ["type", "platform", "externalId"] {
        let value = source_object.get(field).and_then(Value::as_str).ok_or(
            ContextSourceContractErrorV0::SourceObjectMissingRequiredField {
                role,
                record_index,
                field,
            },
        )?;
        if value.trim().is_empty() {
            return Err(
                ContextSourceContractErrorV0::SourceObjectMissingRequiredField {
                    role,
                    record_index,
                    field,
                },
            );
        }
    }

    if source_object.get("platform").and_then(Value::as_str) != Some(XHS_PLATFORM) {
        return Err(ContextSourceContractErrorV0::SourceObjectPlatformMismatch {
            role,
            record_index,
        });
    }
    Ok(())
}

fn payload_object(
    record: &CaptureRecordV0,
    role: ContextPackageRoleV0,
    record_index: usize,
) -> Result<&Map<String, Value>, ContextSourceContractErrorV0> {
    record
        .payload
        .as_object()
        .ok_or(ContextSourceContractErrorV0::PayloadMustBeObject { role, record_index })
}

fn required_non_blank_string(
    payload: &Map<String, Value>,
    role: ContextPackageRoleV0,
    record_index: usize,
    field: &'static str,
) -> Result<String, ContextSourceContractErrorV0> {
    let value = payload.get(field).and_then(Value::as_str).ok_or(
        ContextSourceContractErrorV0::MissingRequiredField {
            role,
            record_index,
            field,
        },
    )?;
    if value.trim().is_empty() {
        return Err(ContextSourceContractErrorV0::MissingRequiredField {
            role,
            record_index,
            field,
        });
    }
    Ok(value.to_owned())
}

fn required_non_blank_text(
    payload: &Map<String, Value>,
    role: ContextPackageRoleV0,
    record_index: usize,
    field: &'static str,
) -> Result<String, ContextSourceContractErrorV0> {
    let value = payload.get(field).and_then(Value::as_str).ok_or(
        ContextSourceContractErrorV0::MissingRequiredField {
            role,
            record_index,
            field,
        },
    )?;
    if value.trim().is_empty() {
        return Err(ContextSourceContractErrorV0::BlankRequiredText {
            role,
            record_index,
            field,
        });
    }
    Ok(value.to_owned())
}

fn optional_context_text(
    payload: &Map<String, Value>,
    role: ContextPackageRoleV0,
    record_index: usize,
    field: &'static str,
) -> Result<ContextTextAvailabilityV0, ContextSourceContractErrorV0> {
    match payload.get(field) {
        None | Some(Value::Null) => Ok(ContextTextAvailabilityV0::Unavailable),
        Some(Value::String(value)) if value.trim().is_empty() => {
            Ok(ContextTextAvailabilityV0::Blank)
        }
        Some(Value::String(value)) => Ok(ContextTextAvailabilityV0::Observed(value.to_owned())),
        Some(_) => Err(
            ContextSourceContractErrorV0::OptionalFieldMustBeStringOrNull {
                role,
                record_index,
                field,
            },
        ),
    }
}

fn optional_identifier(
    payload: &Map<String, Value>,
    role: ContextPackageRoleV0,
    record_index: usize,
    field: &'static str,
) -> Result<Option<String>, ContextSourceContractErrorV0> {
    match payload.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.to_owned())),
        Some(_) => Err(
            ContextSourceContractErrorV0::OptionalFieldMustBeStringOrNull {
                role,
                record_index,
                field,
            },
        ),
    }
}

fn require_consistent_note_id(
    expected: &mut Option<String>,
    candidate: String,
    role: ContextPackageRoleV0,
    record_index: usize,
) -> Result<(), ContextSourceContractErrorV0> {
    match expected {
        Some(current) if current != &candidate => Err(
            ContextSourceContractErrorV0::InconsistentNoteIdWithinPackage { role, record_index },
        ),
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

    const FIXTURE: &str =
        include_str!("../../../fixtures/xhs/comment-context-evidence-set-v0.json");

    fn fixture_value() -> Value {
        serde_json::from_str(FIXTURE).expect("context fixture must be valid JSON")
    }

    fn package_values(value: &Value) -> (Value, Value, Value) {
        let packages = value["packages"]
            .as_object()
            .expect("fixture packages object");
        (
            packages["detailPackage"].clone(),
            packages["commentsPackage"].clone(),
            packages["repliesPackage"].clone(),
        )
    }

    fn fixture_packages() -> (CapturePackageV0, CapturePackageV0, CapturePackageV0) {
        let fixture = fixture_value();
        let (detail, comments, replies) = package_values(&fixture);
        (
            serde_json::from_value(detail).expect("detail must deserialize"),
            serde_json::from_value(comments).expect("comments must deserialize"),
            serde_json::from_value(replies).expect("replies must deserialize"),
        )
    }

    #[test]
    fn validates_real_deidentified_context_fixture_without_collapsing_reply_pointers() {
        let (detail, comments, replies) = fixture_packages();
        let validated = validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
            .expect("fixture must meet the context source contract");

        assert_eq!(
            validated.detail.work_context.note_id,
            validated.comments.note_id
        );
        assert_eq!(
            validated.detail.work_context.note_id,
            validated.replies.note_id
        );
        assert!(matches!(
            validated.detail.work_context.title,
            ContextTextAvailabilityV0::Observed(_)
        ));
        assert!(matches!(
            validated.detail.work_context.body_text,
            ContextTextAvailabilityV0::Observed(_)
        ));
        assert!(validated.replies.replies.iter().any(|reply| {
            reply.parent_comment_id.is_some() && reply.reply_to_comment_id.is_some()
        }));
    }

    #[test]
    fn parses_individual_packages_without_treating_fixture_envelope_as_wire_contract() {
        let fixture = fixture_value();
        let (detail, comments, replies) = package_values(&fixture);
        let validated =
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
                .expect("individual source packages must parse and validate");

        assert_eq!(validated.comments.comments.len(), 1);
        assert_eq!(validated.replies.replies.len(), 2);
    }

    #[test]
    fn preserves_reply_to_only_without_falling_back_to_parent() {
        let mut fixture = fixture_value();
        fixture["packages"]["repliesPackage"]["records"][0]["payload"]
            .as_object_mut()
            .expect("reply payload object")
            .remove("parentCommentId");
        let (detail, comments, replies) = package_values(&fixture);
        let validated =
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
                .expect("replyTo-only reply remains a valid, distinct relation");

        let reply = &validated.replies.replies[0];
        assert!(reply.parent_comment_id.is_none());
        assert!(reply.reply_to_comment_id.is_some());
    }

    #[test]
    fn distinguishes_observed_blank_and_unavailable_work_text() {
        let mut fixture = fixture_value();
        let payload = fixture["packages"]["detailPackage"]["records"][0]["payload"]
            .as_object_mut()
            .expect("detail payload object");
        payload["title"] = json!("  ");
        payload.remove("bodyText");
        let (detail, comments, replies) = package_values(&fixture);
        let validated =
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
                .expect("blank and unavailable optional work text are distinct valid states");

        assert_eq!(
            validated.detail.work_context.title,
            ContextTextAvailabilityV0::Blank
        );
        assert_eq!(
            validated.detail.work_context.body_text,
            ContextTextAvailabilityV0::Unavailable
        );
    }

    #[test]
    fn permits_missing_source_object_but_validates_it_when_present() {
        let mut fixture = fixture_value();
        for package in ["detailPackage", "commentsPackage", "repliesPackage"] {
            for record in fixture["packages"][package]["records"]
                .as_array_mut()
                .expect("records array")
            {
                record
                    .as_object_mut()
                    .expect("record object")
                    .remove("sourceObject");
            }
        }
        let (detail, comments, replies) = package_values(&fixture);
        parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies)
            .expect("sourceObject is optional");

        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["sourceObject"] = json!([]);
        let (detail, comments, replies) = package_values(&fixture);
        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::SourceObjectMustBeObject {
                role: ContextPackageRoleV0::Comments,
                record_index: 0
            })
        ));

        let mut fixture = fixture_value();
        fixture["packages"]["repliesPackage"]["records"][0]["sourceObject"]["platform"] =
            json!("not-xhs");
        let (detail, comments, replies) = package_values(&fixture);
        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::SourceObjectPlatformMismatch {
                role: ContextPackageRoleV0::Replies,
                record_index: 0
            })
        ));
    }

    #[test]
    fn rejects_card_as_work_context() {
        let mut fixture = fixture_value();
        fixture["packages"]["detailPackage"]["records"][0]["kind"] = json!("card");
        let (detail, comments, replies) = package_values(&fixture);

        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::UnexpectedRecordKind {
                role: ContextPackageRoleV0::ContentDetail,
                record_index: 0
            })
        ));
    }

    #[test]
    fn rejects_note_mismatch_between_current_comments_and_work_context() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["payload"]["noteId"] =
            json!("different-note-fixture-001");
        let (detail, comments, replies) = package_values(&fixture);

        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::CrossPackageNoteIdMismatch {
                role: ContextPackageRoleV0::Comments
            })
        ));
    }

    #[test]
    fn rejects_blank_comment_or_reply_text() {
        let mut fixture = fixture_value();
        fixture["packages"]["commentsPackage"]["records"][0]["payload"]["text"] = json!(" \t ");
        let (detail, comments, replies) = package_values(&fixture);
        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::BlankRequiredText {
                role: ContextPackageRoleV0::Comments,
                record_index: 0,
                field: "text"
            })
        ));

        let mut fixture = fixture_value();
        fixture["packages"]["repliesPackage"]["records"][0]["payload"]["text"] = json!("\n");
        let (detail, comments, replies) = package_values(&fixture);
        assert!(matches!(
            parse_and_validate_xhs_comment_context_capture_set_v0(detail, comments, replies),
            Err(ContextSourceContractErrorV0::BlankRequiredText {
                role: ContextPackageRoleV0::Replies,
                record_index: 0,
                field: "text"
            })
        ));
    }
}
