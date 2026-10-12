//! Review correction admits one known shape defect, never a semantic rewrite.
use super::super::semantic::ReviewProposal;
use super::*;
use std::collections::HashSet;

pub(crate) fn safe_review_feedback(code: &str, diagnostic: &Value) -> Option<Value> {
    if code != "output_schema_invalid" || diagnostic["rule"] != "unknown_property" {
        return None;
    }
    let path = diagnostic["schemaPath"].as_str()?;
    let index: usize = path
        .strip_prefix("checks[")?
        .strip_suffix(']')?
        .parse()
        .ok()?;
    if index >= analysis::verification::MAX_VERIFY_CHECKS || path != format!("checks[{index}]") {
        return None;
    }
    Some(
        json!({"contract":analysis::verification::VERIFY_CONTRACT,"code":code,
        "rule":"unknown_property","fieldPath":path,
        "instruction":"重新生成完整严格复核JSON：顶层仅contract/checks，每个checks项仅checkId/verdict/reason。输入的rule/subject只用于理解，不复制到输出。每个既定checkId恰好出现一次；不改写冻结提案，不把fail或uncertain改成pass。",
        "boundary":"不提供旧模型输出，不删除未知字段后接纳，不补默认，不扩大任何检查或调用权限。"}),
    )
}

/// Inspect a complete response without editing or accepting it. An unknown
/// property must not hide invalid IDs, missing checks, bad reasons or non-pass verdicts.
pub(crate) fn review_response_eligible(response: Option<&PiResponse>, expected: &Value) -> bool {
    let Some(response) = response else {
        return false;
    };
    if !response.ok
        || response.failure_code.is_some()
        || !response.diagnostic.as_ref().is_some_and(|d| {
            d.stage == "terminal"
                && d.terminal_received == Some(true)
                && d.response_started == Some(true)
                && d.finish_reason.as_deref() == Some("stop")
                && d.limit_kind.is_none()
        })
    {
        return false;
    }
    let Some(text) = response.text.as_ref().filter(|text| text.len() <= 65536) else {
        return false;
    };
    let Ok(raw) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let Some(object) = raw.as_object() else {
        return false;
    };
    if object.len() != 2 || raw["contract"] != analysis::verification::VERIFY_CONTRACT {
        return false;
    }
    let Some(expected) = expected.as_array().filter(|checks| {
        !checks.is_empty() && checks.len() <= analysis::verification::MAX_VERIFY_CHECKS
    }) else {
        return false;
    };
    let Some(expected_ids) = expected
        .iter()
        .map(|check| check["checkId"].as_str())
        .collect::<Option<HashSet<_>>>()
    else {
        return false;
    };
    let Some(checks) = raw["checks"].as_array() else {
        return false;
    };
    if expected_ids.len() != expected.len() || checks.len() != expected.len() {
        return false;
    }
    let mut seen = HashSet::new();
    let mut has_extra = false;
    for check in checks {
        let Some(fields) = check.as_object() else {
            return false;
        };
        let Some(id) = check["checkId"].as_str() else {
            return false;
        };
        let Some(reason) = check["reason"].as_str() else {
            return false;
        };
        if !expected_ids.contains(id)
            || !seen.insert(id)
            || check["verdict"] != "pass"
            || reason.trim().is_empty()
            || reason.chars().count() > 60
        {
            return false;
        }
        has_extra |= fields
            .keys()
            .any(|key| !["checkId", "verdict", "reason"].contains(&key.as_str()));
    }
    has_extra
}

fn feedback_binding_matches(
    manifest: &Value,
    result: &Value,
    source: &Value,
    proposal: &ReviewProposal,
    hash: &str,
) -> bool {
    manifest["semanticStage"] == "review"
        && manifest["phase"] == proposal.phase
        && manifest["proposalHash"] == hash
        && manifest["sourceInvocationRef"] == json!(proposal.source_invocation_ref)
        && result["accepted"] == false
        && result["dispatchUnknown"] == false
        && result["reviewCorrectionEligible"] == true
        && result["semanticReview"]["contract"] == analysis::verification::VERIFY_CONTRACT
        && result["semanticReview"]["passed"] == false
        && result["semanticReview"]["proposalHash"] == hash
        && result["semanticReview"]["sourceInvocationRef"] == json!(proposal.source_invocation_ref)
        && result["diagnostic"]["stage"] == "terminal"
        && result["diagnostic"]["terminalReceived"] == true
        && result["diagnostic"]["responseStarted"] == true
        && result["diagnostic"]["finishReason"] == "stop"
        && result["diagnostic"]["limitKind"].is_null()
        && source_stamp_equal(&manifest["source"], source)
        && source_stamp_equal(&manifest["source"], &proposal.source)
}

pub(crate) async fn previous_review_feedback(
    db: &Database,
    row: &sqlx::postgres::PgRow,
    input: &ResearchInput,
    proposal: &ReviewProposal,
) -> Result<RuleFeedback, ModelError> {
    let attempts: i32 = row.get("phase_attempt_count");
    let reason: Option<String> = row.get("last_reason");
    let latest = sqlx::query("SELECT i.invocation_ref,i.state,i.failure_code,i.result,q.phase,q.request_manifest,q.dispatch_started_at IS NOT NULL AS sent FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.task_ref=$1 ORDER BY q.attempt_ordinal DESC LIMIT 1")
        .bind(row.get::<Uuid,_>("task_ref")).fetch_optional(db.pool()).await?;
    let Some(latest) = latest else {
        return Ok(RuleFeedback::Unavailable);
    };
    let failure: Option<String> = latest.get("failure_code");
    let manifest: Value = latest.get("request_manifest");
    let needed = manifest["semanticStage"] == "review"
        && (reason.as_deref() == Some("output_schema_invalid")
            || failure.as_deref() == Some("output_schema_invalid")
            || manifest
                .get("validationFeedback")
                .is_some_and(|v| !v.is_null()));
    if !needed {
        return Ok(RuleFeedback::NotRequired);
    }
    let result: Value = latest.get("result");
    let source = topic_map_research::reference_manifest(input);
    let hash = proposal.fingerprint()?;
    if attempts != 1
        || row
            .get::<Value, _>("recall_manifest")
            .get("backfill")
            .is_some()
        || latest.get::<String, _>("state") != "failed"
        || !latest.get::<bool, _>("sent")
        || latest.get::<String, _>("phase") != proposal.phase
        || failure != reason
        || !source_stamp_equal(&manifest["source"], &row.get::<Value, _>("input_refs"))
        || !feedback_binding_matches(&manifest, &result, &source, proposal, &hash)
    {
        return Ok(RuleFeedback::Unavailable);
    }
    let Some(mut feedback) = safe_review_feedback(
        failure.as_deref().unwrap_or(""),
        &result["validationDiagnostic"],
    ) else {
        return Ok(RuleFeedback::Unavailable);
    };
    feedback["sourceInvocationRef"] = json!(proposal.source_invocation_ref);
    feedback["failedReviewInvocationRef"] = json!(latest.get::<Uuid, _>("invocation_ref"));
    feedback["proposalHash"] = json!(hash);
    Ok(RuleFeedback::Available(feedback))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(raw: Value) -> PiResponse {
        serde_json::from_value(json!({"version":"pi-adapter.v1","ok":true,"text":raw.to_string(),
            "failureCode":null,"modelIds":null,"modelListOrigin":null,
            "usage":{"inputTokens":199,"outputTokens":50,"costUsd":null},"elapsedMs":1,
            "diagnostic":{"schemaVersion":1,"stage":"terminal","httpStatus":200,"responseStarted":true,
                "terminalReceived":true,"receivedBytes":100,"limitKind":null,"finishReason":"stop","elapsedMs":1,
                "usageKnown":true,"sdkErrorType":null,"retryClass":"never"}})).unwrap()
    }

    #[test]
    fn review_feedback_carries_only_static_object_path() {
        let feedback = safe_review_feedback("output_schema_invalid",&json!({"rule":"unknown_property","schemaPath":"checks[0]","unknownKey":"PRIVATE_KEY","value":"PRIVATE_VALUE"})).unwrap();
        assert!(!feedback.to_string().contains("PRIVATE"));
        for path in [
            "",
            "checks",
            "checks[]",
            "checks[-1]",
            "checks[96]",
            "checks[01]",
            "checks[0].checkId",
            "checks[0].PRIVATE_KEY",
            "decisions[0]",
        ] {
            assert!(
                safe_review_feedback(
                    "output_schema_invalid",
                    &json!({"rule":"unknown_property","schemaPath":path})
                )
                .is_none(),
                "{path}"
            );
        }
        for rule in [
            "type",
            "enum",
            "required_property",
            "json",
            "semantic_review_uncertain",
        ] {
            assert!(
                safe_review_feedback(
                    "output_schema_invalid",
                    &json!({"rule":rule,"schemaPath":"checks[0]"})
                )
                .is_none()
            );
        }
        assert!(
            safe_feedback(
                "output_schema_invalid",
                &json!({"rule":"unknown_property","schemaPath":"checks[0]"})
            )
            .is_none()
        );
    }

    #[test]
    fn unknown_properties_cannot_hide_identity_semantic_or_transport_failures() {
        let expected = json!([{"checkId":"o0"},{"checkId":"d0.s"}]);
        let good = json!({"contract":analysis::verification::VERIFY_CONTRACT,"checks":[
            {"checkId":"o0","verdict":"pass","reason":"合成依据","PRIVATE_KEY":"PRIVATE_VALUE"},
            {"checkId":"d0.s","verdict":"pass","reason":"合成依据"}]});
        assert!(review_response_eligible(
            Some(&response(good.clone())),
            &expected
        ));
        for (pointer, value) in [
            ("/checks/0/checkId", json!("foreign0")),
            ("/checks/1/checkId", json!("o0")),
            ("/checks/0/verdict", json!("fail")),
            ("/checks/0/verdict", json!("uncertain")),
            ("/checks/0/reason", json!(null)),
            ("/checks/0/reason", json!(" ")),
            ("/checks/0/reason", json!("字".repeat(61))),
            ("/contract", json!("foreign")),
        ] {
            let mut invalid = good.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                !review_response_eligible(Some(&response(invalid)), &expected),
                "{pointer}"
            );
        }
        let mut missing = good.clone();
        missing["checks"].as_array_mut().unwrap().pop();
        assert!(!review_response_eligible(
            Some(&response(missing)),
            &expected
        ));
        let mut incomplete = response(good);
        incomplete.diagnostic.as_mut().unwrap().terminal_received = Some(false);
        assert!(!review_response_eligible(Some(&incomplete), &expected));
        incomplete.diagnostic.as_mut().unwrap().terminal_received = Some(true);
        incomplete.diagnostic.as_mut().unwrap().finish_reason = Some("length".into());
        assert!(!review_response_eligible(Some(&incomplete), &expected));
    }

    #[test]
    fn corrective_review_is_bound_to_the_same_frozen_proposal_and_source() {
        let source = json!({"coverage":{},"inputHash":"frozen-source"});
        let proposal = ReviewProposal {
            source_invocation_ref: Uuid::new_v4(),
            phase: "extract".into(),
            source: source.clone(),
            proposal: json!({}),
            candidates: json!([]),
            recall: json!({}),
            units: vec![],
            cursor: 0,
            total_units: 0,
        };
        let hash = proposal.fingerprint().unwrap();
        let manifest = json!({"semanticStage":"review","phase":"extract","source":source,
            "sourceInvocationRef":proposal.source_invocation_ref,"proposalHash":hash});
        let result = json!({"accepted":false,"dispatchUnknown":false,"reviewCorrectionEligible":true,
            "semanticReview":{"contract":analysis::verification::VERIFY_CONTRACT,"passed":false,
                "sourceInvocationRef":proposal.source_invocation_ref,"proposalHash":hash},
            "diagnostic":{"stage":"terminal","terminalReceived":true,"responseStarted":true,
                "finishReason":"stop","limitKind":null}});
        assert!(feedback_binding_matches(
            &manifest, &result, &source, &proposal, &hash
        ));
        for path in [
            "/semanticStage",
            "/phase",
            "/source/inputHash",
            "/sourceInvocationRef",
            "/proposalHash",
        ] {
            let mut invalid = manifest.clone();
            *invalid.pointer_mut(path).unwrap() = json!("changed");
            assert!(
                !feedback_binding_matches(&invalid, &result, &source, &proposal, &hash),
                "{path}"
            );
        }
        for path in [
            "/accepted",
            "/dispatchUnknown",
            "/reviewCorrectionEligible",
            "/semanticReview/contract",
            "/semanticReview/passed",
            "/semanticReview/sourceInvocationRef",
            "/semanticReview/proposalHash",
            "/diagnostic/stage",
            "/diagnostic/terminalReceived",
            "/diagnostic/responseStarted",
            "/diagnostic/finishReason",
            "/diagnostic/limitKind",
        ] {
            let mut invalid = result.clone();
            *invalid.pointer_mut(path).unwrap() = json!("changed");
            assert!(
                !feedback_binding_matches(&manifest, &invalid, &source, &proposal, &hash),
                "{path}"
            );
        }
    }
}
