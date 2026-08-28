//! Task/package/record qualification before any typed material is projected.

use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;

pub(crate) fn task_package_binding_valid(
    task_spec: &Value,
    package: &ProducerCapturePackage,
) -> bool {
    let requested = task_spec
        .get("capabilitiesRequested")
        .and_then(Value::as_array);
    let target = task_spec.get("target").and_then(Value::as_object);
    let coverage_target = package.coverage().get("target").and_then(Value::as_object);
    task_spec.get("platform").and_then(Value::as_str) == Some(package.platform())
        && requested.is_some_and(|values| {
            values
                .iter()
                .any(|value| value.as_str() == Some(package.package_kind()))
        })
        && unique_coverage_layer(package.coverage(), package.package_kind()).is_some()
        && target
            .zip(coverage_target)
            .is_some_and(|(expected, actual)| {
                expected
                    .iter()
                    .all(|(key, value)| actual.get(key) == Some(value))
            })
}

pub(crate) fn record_disposition(
    package: &ProducerCapturePackage,
    record_ordinal: usize,
    record: &Value,
) -> Option<(&'static str, &'static str)> {
    let accepted = "accepted_for_library_content";
    let quarantined = "quarantined";
    match package.package_kind() {
        "discovery_search" | "profile_discovery" => {
            let expected_kind = if package.package_kind() == "profile_discovery" {
                "profile_discovery_card"
            } else {
                "discovery_card"
            };
            if record.get("kind").and_then(Value::as_str) != Some(expected_kind) {
                return Some(("retained_uninterpreted", "typed_discovery_kind_unsupported"));
            }
            Some(if source_is(package, record, "content") {
                (
                    "accepted_for_library_discovery",
                    "typed_discovery_identity_valid",
                )
            } else {
                (quarantined, "typed_discovery_identity_invalid")
            })
        }
        "content_detail" => Some(if content_record_valid(package, record) {
            (accepted, "typed_detail_identity_valid")
        } else {
            (quarantined, "typed_detail_identity_invalid")
        }),
        "comments" | "replies" => {
            let replies = package.package_kind() == "replies";
            let duplicated = duplicate_value(package, record_ordinal, "/payload/commentId", record);
            let valid = !duplicated && discussion_record_valid(package, record, replies);
            Some(if valid {
                (
                    accepted,
                    if replies {
                        "typed_reply_relationship_valid"
                    } else {
                        "typed_comment_identity_valid"
                    },
                )
            } else {
                (
                    quarantined,
                    if duplicated {
                        "typed_comment_identity_duplicate"
                    } else if replies {
                        "typed_reply_relationship_invalid"
                    } else {
                        "typed_comment_identity_invalid"
                    },
                )
            })
        }
        "author_profile" => Some(if author_record_valid(package, record) {
            (accepted, "typed_author_identity_valid")
        } else {
            (quarantined, "typed_author_identity_invalid")
        }),
        "media_slots" => crate::material_media::record_disposition(package, record_ordinal, record),
        _ => None,
    }
}

pub(crate) fn unique_coverage_layer<'a>(
    coverage: &'a Value,
    capability: &str,
) -> Option<&'a Value> {
    let mut matching = coverage
        .get("layers")?
        .as_array()?
        .iter()
        .filter(|layer| layer.get("capability").and_then(Value::as_str) == Some(capability));
    let layer = matching.next()?;
    matching.next().is_none().then_some(layer)
}

fn content_record_valid(package: &ProducerCapturePackage, record: &Value) -> bool {
    record.get("kind").and_then(Value::as_str) == Some("content_detail")
        && source_is(package, record, "content")
        && source_external_id(record) == target_string(package, "contentExternalId")
        && record.get("payload").is_some_and(Value::is_object)
}

fn discussion_record_valid(
    package: &ProducerCapturePackage,
    record: &Value,
    replies: bool,
) -> bool {
    let expected_kind = if replies { "reply" } else { "comment" };
    record.get("kind").and_then(Value::as_str) == Some(expected_kind)
        && source_is(package, record, "content")
        && source_external_id(record) == target_string(package, "contentExternalId")
        && record
            .get("payload")
            .and_then(Value::as_object)
            .is_some_and(|payload| {
                exact_string(payload, "noteId") == target_string(package, "contentExternalId")
                    && exact_string(payload, "commentId").is_some()
                    && if replies {
                        let comment_id = exact_string(payload, "commentId");
                        let root = exact_string(payload, "rootCommentId");
                        let parent = exact_string(payload, "parentCommentId");
                        let reply_to = exact_string(payload, "replyToCommentId");
                        let selected_parent = parent.or(reply_to);
                        root.is_some()
                            && (parent.is_some() ^ reply_to.is_some())
                            && comment_id != root
                            && comment_id != selected_parent
                    } else {
                        ["rootCommentId", "parentCommentId", "replyToCommentId"]
                            .iter()
                            .all(|key| exact_string(payload, key).is_none())
                    }
            })
}

fn author_record_valid(package: &ProducerCapturePackage, record: &Value) -> bool {
    let author_id = target_string(package, "authorExternalId");
    record.get("kind").and_then(Value::as_str) == Some("author_profile")
        && source_is(package, record, "author")
        && source_external_id(record) == author_id
        && record
            .get("payload")
            .and_then(Value::as_object)
            .is_some_and(|payload| {
                exact_string(payload, "userId").or_else(|| exact_string(payload, "id")) == author_id
            })
}

fn duplicate_value(
    package: &ProducerCapturePackage,
    record_ordinal: usize,
    pointer: &str,
    record: &Value,
) -> bool {
    record
        .pointer(pointer)
        .and_then(Value::as_str)
        .is_some_and(|identity| {
            package
                .records()
                .iter()
                .enumerate()
                .any(|(other_ordinal, other)| {
                    other_ordinal != record_ordinal
                        && other.pointer(pointer).and_then(Value::as_str) == Some(identity)
                })
        })
}

fn source_is(package: &ProducerCapturePackage, record: &Value, subject_type: &str) -> bool {
    record
        .pointer("/sourceObject/platform")
        .and_then(Value::as_str)
        == Some(package.platform())
        && record.pointer("/sourceObject/type").and_then(Value::as_str) == Some(subject_type)
        && source_external_id(record).is_some()
}

fn source_external_id(record: &Value) -> Option<&str> {
    record
        .pointer("/sourceObject/externalId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn target_string<'a>(package: &'a ProducerCapturePackage, field: &str) -> Option<&'a str> {
    package
        .coverage()
        .pointer(&format!("/target/{field}"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn exact_string<'a>(payload: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
