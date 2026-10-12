//! One bounded corrective attempt carries rules, never a previous model response.
use super::*;
#[path = "feedback/review.rs"]
mod review;
pub(super) use review::{previous_review_feedback, review_response_eligible, safe_review_feedback};

pub(super) fn safe_feedback(code: &str, diagnostic: &Value) -> Option<Value> {
    let (rule, path, instruction) = match code {
        "output_schema_invalid" => {
            let rule = diagnostic["rule"].as_str()?;
            if ![
                "type",
                "type_or_format",
                "required_property",
                "enum",
                "min_items",
            ]
            .contains(&rule)
            {
                return None;
            }
            let path = diagnostic["schemaPath"].as_str()?;
            if !safe_path(path)
                || path.split(['.', '[', ']']).any(|field| {
                    [
                        "quote",
                        "checks",
                        "checkId",
                        "verdict",
                        "fragmentId",
                        "start",
                        "end",
                        "topicRef",
                        "definitionRef",
                        "unitId",
                    ]
                    .contains(&field)
                })
            {
                return None;
            }
            (
                rule,
                path,
                "按 outputSchema 修正此字段的类型、必填、允许值或最少条数；纳入与排除条件各至少一条，标题必须是字符串。",
            )
        }
        "new_topic_candidate_coverage_missing" if diagnostic["rule"] == code => (
            "new_topic_candidate_coverage_missing",
            "decisions[].relations",
            "new 必须逐一比较当前 candidateComparisonChecklist 的全部精确候选，不能只比较关键词相近的几项；每项都给关系和简短依据。equivalent 必须明确 matched，不能创建重复新主题。",
        ),
        "journey_evidence_missing" if diagnostic["rule"] == code => (
            "journey_evidence_missing",
            "journey.evidence",
            "明确旅程阶段必须引用实际支持该阶段的有效材料；没有依据应保持 unclear，不补造引用。",
        ),
        _ => return None,
    };
    Some(
        json!({"code":code,"rule":rule,"fieldPath":path,"instruction":instruction,
        "boundary":"只依据本次输入重新生成完整严格输出。不提供旧模型输出，不补默认，不放松引用和语义守门。"}),
    )
}

fn safe_path(path: &str) -> bool {
    const FIELDS: &[&str] = &[
        "contract",
        "outcome",
        "discussions",
        "scenes",
        "journey",
        "responseMatches",
        "angles",
        "productOpportunities",
        "limitations",
        "label",
        "statement",
        "definition",
        "inclusionCriteria",
        "exclusionCriteria",
        "speakerRole",
        "evidenceRole",
        "rationale",
        "topicRef",
        "evidence",
        "fragmentId",
        "start",
        "end",
        "mainStage",
        "involvedStages",
        "overlays",
        "path",
        "status",
        "unanswered",
        "title",
        "answerTask",
        "need",
        "hypothesis",
        "verificationQuestion",
        "alternativeExplanation",
        "decisions",
        "unitId",
        "matches",
        "definitionRef",
        "reason",
        "proposedTopic",
        "relations",
        "relation",
        "domainFit",
        "domainReason",
        "abstractionReason",
        "parent",
        "kind",
        "proposal",
    ];
    path.len() <= 300
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".[]".contains(c))
        && path
            .split(['.', '[', ']'])
            .filter(|s| !s.is_empty())
            .all(|part| {
                FIELDS.contains(&part)
                    || (part.len() <= 6 && part.chars().all(|c| c.is_ascii_digit()))
            })
}

pub(super) fn correction_allowed(attempts: i32, limit: i32) -> bool {
    // One extra model call, even when a synthetic configuration permits three.
    attempts == 1 && attempts < limit
}

pub(super) enum RuleFeedback {
    NotRequired,
    Available(Value),
    Unavailable,
}

fn correctable_code(code: Option<&str>) -> bool {
    matches!(
        code,
        Some(
            "output_schema_invalid"
                | "new_topic_candidate_coverage_missing"
                | "journey_evidence_missing"
        )
    )
}

/// Only the live restoration map is derived metadata. Hashes, ranges, roles,
/// context participants and exact definition dependencies retain full equality.
pub(super) fn source_stamp_equal(left: &Value, right: &Value) -> bool {
    fn stamp(value: &Value) -> Option<Value> {
        let object = value.as_object()?;
        let mut source = if let Some(source) = object.get("source") {
            // A bare source wrapper is equivalent; other wrapper provenance is
            // not silently discarded and cannot certify a matching source.
            if object.len() != 1 {
                return None;
            }
            source.as_object()?.clone()
        } else {
            object.clone()
        };
        source
            .get_mut("coverage")?
            .as_object_mut()?
            .remove("currentSources");
        Some(Value::Object(source))
    }
    stamp(left)
        .zip(stamp(right))
        .is_some_and(|(left, right)| left == right)
}

pub(super) async fn previous_feedback(
    db: &Database,
    row: &sqlx::postgres::PgRow,
    input: &ResearchInput,
) -> Result<RuleFeedback, ModelError> {
    let attempts: i32 = row.get("phase_attempt_count");
    if attempts == 0 {
        return Ok(RuleFeedback::NotRequired);
    }
    let reason: Option<String> = row.get("last_reason");
    let latest = sqlx::query("SELECT i.invocation_ref,i.state,i.failure_code,i.result,q.phase,q.request_manifest FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.task_ref=$1 ORDER BY q.attempt_ordinal DESC LIMIT 1")
        .bind(row.get::<Uuid,_>("task_ref")).fetch_optional(db.pool()).await?;
    let Some(latest) = latest else {
        return Ok(if correctable_code(reason.as_deref()) {
            RuleFeedback::Unavailable
        } else {
            RuleFeedback::NotRequired
        });
    };
    let result: Value = latest.get("result");
    let manifest: Value = latest.get("request_manifest");
    let failure: Option<String> = latest.get("failure_code");
    let needed = correctable_code(reason.as_deref())
        || correctable_code(failure.as_deref())
        || manifest
            .get("validationFeedback")
            .is_some_and(|value| !value.is_null());
    if !needed {
        return Ok(RuleFeedback::NotRequired);
    }
    let recall: Value = row.get("recall_manifest");
    let frozen: Value = row.get("input_refs");
    let source = &manifest["source"];
    if attempts != 1
        || recall.get("backfill").is_some()
        || row.get::<String, _>("input_hash") != input.hash
        || latest.get::<String, _>("state") != "failed"
        || latest.get::<String, _>("phase") != row.get::<String, _>("phase")
        || failure.is_none()
        || failure != reason
        || result["accepted"] != false
        || result["dispatchUnknown"] != false
        || source["inputHash"].as_str() != Some(input.hash.as_str())
        || !source_stamp_equal(source, &frozen)
        || !source_stamp_equal(source, &topic_map_research::reference_manifest(input))
    {
        return Ok(RuleFeedback::Unavailable);
    }
    let Some(mut feedback) =
        safe_feedback(failure.as_deref().unwrap(), &result["validationDiagnostic"])
    else {
        return Ok(RuleFeedback::Unavailable);
    };
    feedback["sourceInvocationRef"] = json!(latest.get::<Uuid, _>("invocation_ref"));
    Ok(RuleFeedback::Available(feedback))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frozen_source_stamp() -> Value {
        json!({
            "inputContract":"topic-map.source-windows.v2", "inputHash":"frozen-input",
            "workRef":"work-1", "contextWorkRefs":["context-1"], "scopeWorkRefs":["work-1","context-1"],
            "methodVersion":"topic-map.research.v4", "configRef":"config-1", "domain":"domain-1",
            "domainDefinitionHash":"domain-hash", "roleIdentity":{"workRole":"author_work"},
            "roleMetadata":{"sourceRole":"author_work","commentOnly":false},
            "commentStudy":[{"signalRef":"signal-1","version":1}], "topics":[],
            "coverage":{
                "kind":"comparison", "sourceHashes":{"source-1":"text-hash"},
                "fragmentOrigins":{"fragment-1":"source-1"}, "scopeWorkRefs":["work-1","context-1"],
                "selectedDiscussions":[{"unitRef":"unit-1","definitionRef":"definition-1"}],
                "comparedDefinitionRefs":["definition-1"],
                "hierarchyDependencies":[{"topicRef":"parent-1","definitionRef":"parent-definition-1"}]
            },
            "fragments":[{"fragmentId":"fragment-1","start":0,"end":20,
                "textHash":"text-hash","sourceVersion":1,"role":"author_work"}]
        })
    }

    #[test]
    fn source_stamp_accepts_only_the_derived_live_map_difference() {
        let frozen = frozen_source_stamp();
        let mut live = frozen.clone();
        live["coverage"]["currentSources"] =
            json!({"fragment-1":{"sourceRef":"restored-source-1"}});
        assert!(source_stamp_equal(&frozen, &live));
        assert!(source_stamp_equal(&json!({"source":frozen}), &live));
        assert!(source_stamp_equal(&live, &json!({"source":frozen})));
        let mut another_live_map = live.clone();
        another_live_map["coverage"]["currentSources"] =
            json!({"fragment-1":{"sourceRef":"physical-restoration-2"}});
        assert!(source_stamp_equal(&live, &another_live_map));
    }

    #[test]
    fn source_stamp_preserves_every_frozen_identity_and_dependency() {
        let frozen = frozen_source_stamp();
        let mut live = frozen.clone();
        live["coverage"]["currentSources"] = json!({});
        for path in [
            "/inputHash",
            "/inputContract",
            "/workRef",
            "/contextWorkRefs/0",
            "/scopeWorkRefs/0",
            "/methodVersion",
            "/configRef",
            "/domain",
            "/domainDefinitionHash",
            "/roleIdentity/workRole",
            "/roleMetadata/sourceRole",
            "/roleMetadata/commentOnly",
            "/commentStudy/0/signalRef",
            "/commentStudy/0/version",
            "/coverage/kind",
            "/coverage/sourceHashes/source-1",
            "/coverage/fragmentOrigins/fragment-1",
            "/coverage/scopeWorkRefs/0",
            "/coverage/selectedDiscussions/0/unitRef",
            "/coverage/selectedDiscussions/0/definitionRef",
            "/coverage/comparedDefinitionRefs/0",
            "/coverage/hierarchyDependencies/0/topicRef",
            "/coverage/hierarchyDependencies/0/definitionRef",
            "/fragments/0/fragmentId",
            "/fragments/0/start",
            "/fragments/0/end",
            "/fragments/0/textHash",
            "/fragments/0/sourceVersion",
            "/fragments/0/role",
        ] {
            let mut changed = live.clone();
            *changed.pointer_mut(path).expect("fixture path exists") = json!("changed");
            assert!(!source_stamp_equal(&frozen, &changed), "changed {path}");
        }
        let mut extra_dependency = live.clone();
        extra_dependency["coverage"]["newDependency"] = json!("definition-2");
        assert!(!source_stamp_equal(&frozen, &extra_dependency));
        assert!(!source_stamp_equal(
            &json!({"source":frozen,"dependency":"outer-provenance"}),
            &live
        ));
        for invalid in [json!(null), json!([]), json!({}), json!({"coverage":[]})] {
            assert!(!source_stamp_equal(&invalid, &invalid));
            assert!(!source_stamp_equal(&live, &invalid));
        }
    }

    #[test]
    fn only_static_correctable_rules_get_one_extra_attempt() {
        assert!(
            safe_feedback(
                "output_schema_invalid",
                &json!({"rule":"min_items","schemaPath":"discussions[0].exclusionCriteria"})
            )
            .is_some()
        );
        assert!(
            safe_feedback(
                "journey_evidence_missing",
                &json!({"rule":"journey_evidence_missing"})
            )
            .is_some()
        );
        for code in [
            "unknown_dispatch",
            "invalid_discussion_evidence",
            "journey_evidence_invalid",
            "source_changed",
            "topic_parent_version_mismatch",
            "output_limit",
        ] {
            assert!(safe_feedback(code, &json!({"rule":code})).is_none());
        }
        assert!(correction_allowed(1, 2));
        for (attempts, limit) in [(0, 2), (1, 1), (2, 3), (3, 3)] {
            assert!(!correction_allowed(attempts, limit));
        }
    }
    #[test]
    fn foreign_fields_values_and_unallowlisted_rules_never_enter_feedback() {
        for diagnostic in [
            json!({"rule":"unknown_property","schemaPath":"private_source"}),
            json!({"rule":"type","schemaPath":"discussions[0].PRIVATE_SOURCE"}),
            json!({"rule":"PRIVATE_PROVIDER_PROSE","schemaPath":"title"}),
        ] {
            assert!(safe_feedback("output_schema_invalid", &diagnostic).is_none());
        }
        for field in [
            "fragmentId",
            "start",
            "end",
            "topicRef",
            "definitionRef",
            "unitId",
        ] {
            for rule in ["type", "type_or_format", "required_property"] {
                assert!(
                    safe_feedback(
                        "output_schema_invalid",
                        &json!({"rule":rule,"schemaPath":format!("decisions[0].{field}")})
                    )
                    .is_none()
                );
            }
        }
        let feedback = safe_feedback(
            "output_schema_invalid",
            &json!({"rule":"type","schemaPath":"angles[0].title","raw":"PRIVATE_SOURCE_VALUE"}),
        )
        .unwrap();
        assert!(!feedback.to_string().contains("PRIVATE_SOURCE_VALUE"));
    }
}
