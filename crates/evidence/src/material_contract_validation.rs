//! Task/package/record qualification before any typed material is projected.

use linggan_contracts::ProducerCapturePackage;
use serde_json::Value;

/// 这个包是不是这张任务单要的东西。
///
/// target 的比对只问**身份**（搜的是不是这个词、这个博主、这篇内容），不问采样口径。
/// 口径是下发给插件的**执行指令**（怎么排序、滚几次、取赞前几名、限几天内），插件照做，
/// 但回执的 `coverage.target` 只回显身份，不会把指令抄回来——它本来也没有义务抄：
/// 「插件有没有守住口径」由 `coverage` 的 `attempted/acquired` 如实记录，不靠回显一份
/// 指令副本来证明。
///
/// 2026-09-08 的 `45c04f2` 给关键词任务加上口径下发后，这条比对就必然为假，
/// 此后**每一次**带口径的关键词采集都被整包判为 `task_package_contract_mismatch`
/// 并隔离：09-08 至 09-10 共 120 条搜索结果采回来却全部进不了库。
/// 创作者/内容任务的 target 只有一个身份键，不受影响，所以问题只在关键词这一条通道。
///
/// 同一条错误判定此前在 `creator_lifecycle` 的巡检采样 SQL 里也有一份，09-08 的
/// `58fce3f` 已经把那条 `@>` 比对整条删掉、改用 `package.task_id=task.task_id` 定归属；
/// 那里现在只剩一段记录经过的注释。**当时没有一并改这里**——而这里才是决定材料
/// 能不能入库的那道闸。
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
                    .filter(|(key, _)| !is_exempt_sampling_directive(package, key))
                    .all(|(key, value)| actual.get(key) == Some(value))
            })
}

/// 采样口径只在**关键词搜索**这一条通道上豁免身份比对。
///
/// 豁免按 `package_kind` 而不是只按键名生效：只有 `discovery_search` 会下发口径，
/// 别的通道即使将来出现同名字段（比如给创作者作品排序也叫 `ranking`），那也是它自己的
/// 身份口径，必须照常逐键比对，不该被这里顺手放过。名单的唯一真源在
/// [`crate::work_order_lease::SAMPLING_DIRECTIVE_KEYS`]。
fn is_exempt_sampling_directive(package: &ProducerCapturePackage, key: &str) -> bool {
    package.package_kind() == "discovery_search"
        && crate::work_order_lease::SAMPLING_DIRECTIVE_KEYS.contains(&key)
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

pub(crate) fn duplicate_value(
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

#[cfg(test)]
mod task_package_binding_tests {
    use super::*;
    use serde_json::json;

    /// 按真实链路构造：包只能由合同解析产生，不能在测试里直接拼结构体。
    fn package(package_kind: &str, coverage_target: Value) -> ProducerCapturePackage {
        let raw = json!({
            "contractVersion": "linggan.producer.capture-package.v1",
            "packageRef": "5a6563c6-b962-4916-a17c-04c660cb7313",
            "packageKind": package_kind,
            "platform": "xhs",
            "observedAt": "2026-09-10T10:57:26.801Z",
            "capturedAt": "2026-09-10T10:57:26.801Z",
            "coverage": {
                "target": coverage_target,
                "layers": [{
                    "capability": package_kind,
                    "observed": 20,
                    "attempted": 20,
                    "acquired": 20,
                    "verified": 0,
                    "failed": 0,
                    "notAttempted": 0,
                    "unknown": 4,
                    "stoppedReason": "surface_read_complete"
                }]
            },
            "records": []
        });
        linggan_contracts::parse_producer_capture_package(&raw.to_string())
            .expect("fixture is a valid capture package")
    }

    fn task_spec(capability: &str, target: Value) -> Value {
        json!({
            "platform": "xhs",
            "capabilitiesRequested": [capability],
            "target": target,
        })
    }

    /// 插件回执里的 `coverage.target`，只回显身份，不回显口径——这是真实形状。
    fn keyword_coverage_target() -> Value {
        json!({
            "basis": "current_visible_surface",
            "surface": "target_driven_surface",
            "query": "考研自习",
        })
    }

    /// 回归：2026-09-08 起带口径的关键词任务被整包判为不匹配，120 条搜索结果因此隔离。
    #[test]
    fn a_keyword_task_carrying_sampling_directives_still_binds() {
        let spec = task_spec(
            "discovery_search",
            json!({
                "query": "考研自习",
                "ranking": "most_liked",
                "topByLikes": 20,
                "scrollRounds": 3,
                "publishedWithinDays": 7,
            }),
        );
        assert!(task_package_binding_valid(
            &spec,
            &package("discovery_search", keyword_coverage_target())
        ));
    }

    /// 豁免的只有口径。搜索词本身对不上，仍然必须判为不匹配——
    /// 否则这次修改就成了「关键词包一律放行」。
    #[test]
    fn a_keyword_task_whose_term_differs_does_not_bind() {
        let spec = task_spec(
            "discovery_search",
            json!({ "query": "考研自习", "ranking": "most_liked" }),
        );
        let mut elsewhere = keyword_coverage_target();
        elsewhere["query"] = json!("adhd");
        assert!(!task_package_binding_valid(
            &spec,
            &package("discovery_search", elsewhere)
        ));
    }

    /// 创作者通道原样不变：target 只有身份键，逐键全比对。
    #[test]
    fn a_creator_task_binds_exactly_as_before() {
        let spec = task_spec(
            "profile_discovery",
            json!({ "authorExternalId": "69aad16e000000003201b172" }),
        );
        assert!(task_package_binding_valid(
            &spec,
            &package(
                "profile_discovery",
                json!({
                    "basis": "current_visible_surface",
                    "surface": "target_driven_surface",
                    "authorExternalId": "69aad16e000000003201b172",
                })
            )
        ));
    }

    /// 创作者通道的负例同样不变：身份对不上就是不匹配。
    #[test]
    fn a_creator_task_pointing_at_another_author_does_not_bind() {
        let spec = task_spec(
            "profile_discovery",
            json!({ "authorExternalId": "69aad16e000000003201b172" }),
        );
        assert!(!task_package_binding_valid(
            &spec,
            &package(
                "profile_discovery",
                json!({
                    "basis": "current_visible_surface",
                    "surface": "target_driven_surface",
                    "authorExternalId": "578d8413bd0da503d085793c",
                })
            )
        ));
    }

    /// 口径键的豁免不跨通道生效：创作者任务的 target 里根本不会出现这些键，
    /// 真出现了也不该被当成口径放过——这里锁住「只改了关键词」这件事。
    #[test]
    fn the_exemption_does_not_loosen_identity_keys_of_other_channels() {
        let spec = task_spec(
            "content_detail",
            json!({ "contentExternalId": "6aa1670c0000000029017b8f" }),
        );
        assert!(!task_package_binding_valid(
            &spec,
            &package(
                "content_detail",
                json!({ "basis": "known_set", "contentExternalId": "6a8e86de00000000240043b0" })
            )
        ));
    }

    /// 豁免按通道生效，不只按键名：别的通道将来若出现同名字段（例如给创作者作品
    /// 排序也叫 `ranking`），那是它自己的身份口径，必须照常参与比对。
    /// 少了这一条，这次豁免就成了一条按裸键名全局放宽身份校验的暗门。
    #[test]
    fn a_same_named_key_on_another_channel_is_not_exempt() {
        let spec = task_spec(
            "profile_discovery",
            json!({
                "authorExternalId": "69aad16e000000003201b172",
                "ranking": "most_liked",
            }),
        );
        assert!(!task_package_binding_valid(
            &spec,
            &package(
                "profile_discovery",
                json!({
                    "basis": "current_visible_surface",
                    "surface": "target_driven_surface",
                    "authorExternalId": "69aad16e000000003201b172",
                })
            )
        ));
    }

    /// 平台、能力、层唯一性这三道检查不受本次改动影响。
    #[test]
    fn the_other_three_checks_still_hold() {
        let mismatched_capability = task_spec("profile_discovery", json!({ "query": "考研自习" }));
        assert!(!task_package_binding_valid(
            &mismatched_capability,
            &package("discovery_search", keyword_coverage_target())
        ));

        let mut mismatched_platform = task_spec("discovery_search", json!({ "query": "考研自习" }));
        mismatched_platform["platform"] = json!("douyin");
        assert!(!task_package_binding_valid(
            &mismatched_platform,
            &package("discovery_search", keyword_coverage_target())
        ));
    }
}
