//! Handwritten adversarial inputs test the review packet and acceptance gate,
//! not whether a real model will issue the correct semantic verdict.
use super::super::{
    Citation, Discussion, EXTRACT_CONTRACT, Journey, ProposedParent, ProposedTopic,
    RESOLVE_CONTRACT, ResearchOutput, ResolutionOutput, TopicMatch, UnitDecision,
};
use super::*;
use linggan_evidence::creator_discovery::Fragment;
use uuid::Uuid;

mod calibration;

fn source(text: &str) -> Fragment {
    Fragment {
        fragment_id: "synthetic.body".into(),
        source_ref: Uuid::new_v4(),
        source_version: "synthetic-v1".into(),
        field: "body".into(),
        start: 7,
        end: 7 + text.chars().count(),
        text: text.into(),
    }
}

fn discussion(f: &Fragment, statement: &str) -> Discussion {
    Discussion {
        label: "合成练习支持".into(),
        topic_ref: None,
        evidence: vec![Citation {
            fragment_id: f.fragment_id.clone(),
            start: f.start,
            end: f.end,
        }],
        statement: statement.into(),
        definition: "合成的日常练习执行困难".into(),
        inclusion_criteria: vec!["已选定练习的执行困难".into()],
        exclusion_criteria: vec!["仅讨论选择练习内容".into()],
        speaker_role: "author".into(),
        evidence_role: "support".into(),
        rationale: "手写合成样例".into(),
    }
}

fn extraction(d: Discussion) -> ResearchOutput {
    ResearchOutput {
        contract: EXTRACT_CONTRACT.into(),
        outcome: "analyzed".into(),
        discussions: vec![d],
        scenes: vec![],
        journey: Journey {
            main_stage: "unclear".into(),
            involved_stages: vec![],
            overlays: vec![],
            path: "unknown".into(),
            rationale: "无阶段依据".into(),
            evidence: vec![],
        },
        response_matches: vec![],
        angles: vec![],
        product_opportunities: vec![],
        limitations: vec!["合成，不是现实样本".into()],
    }
}

fn all_pass(packet: &Value) -> VerificationOutput {
    VerificationOutput {
        contract: VERIFY_CONTRACT.into(),
        checks: packet["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| VerificationCheck {
                check_id: c["checkId"].as_str().unwrap().into(),
                verdict: "pass".into(),
                reason: "相符".into(),
            })
            .collect(),
    }
}

fn reject(packet: &Value, rule: &str, verdict: &str) -> VerificationOutput {
    let id = packet["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["rule"] == rule)
        .unwrap()["checkId"]
        .as_str()
        .unwrap();
    let mut reviewed = all_pass(packet);
    let check = reviewed
        .checks
        .iter_mut()
        .find(|c| c.check_id == id)
        .unwrap();
    check.verdict = verdict.into();
    check.reason = "合成反例不满足边界".into();
    reviewed
}

#[test]
fn a_partial_quote_never_exposes_adjacent_text_to_complete_its_claim() {
    let f = source("我计划试做练习。后来尚未开始，是否有效还不知道。😀");
    let mut d = discussion(&f, "已经坚持练习并观察到有效");
    d.evidence[0].end = f.start + 8;
    let packet = extraction_review(&extraction(d), std::slice::from_ref(&f)).unwrap();
    assert_eq!(packet["evidence"][0]["text"], "我计划试做练习。");
    assert!(packet["evidence"][0].get("context").is_none());
    assert!(!packet.to_string().contains("后来尚未开始"));
    assert_eq!(packet["outcomeContext"], json!([]));
    assert_eq!(
        packet["proposal"]["discussions"][0]["evidenceIds"],
        json!(["e0"])
    );
    assert!(
        packet["proposal"]["discussions"][0]
            .get("evidence")
            .is_none()
    );
    let failed = reject(&packet, "semantic_discussion_unsupported", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_discussion_unsupported")
    );
}

#[test]
fn a_reported_view_is_not_first_person_experience_and_a_plan_is_not_effect() {
    for (raw, claim) in [
        ("朋友说很难开始，我转述他的看法。", "作者本人经历开始困难"),
        ("下周准备试着讲题，效果尚不知道。", "讲题已经提高坚持程度"),
    ] {
        let f = source(raw);
        let packet = extraction_review(&extraction(discussion(&f, claim)), &[f]).unwrap();
        assert_eq!(packet["evidence"][0]["text"], raw);
        assert_eq!(packet["proposal"]["discussions"][0]["statement"], claim);
        let failed = reject(&packet, "semantic_discussion_unsupported", "fail");
        assert_eq!(
            validate_verification(&failed, &packet["checks"]),
            Err("semantic_discussion_unsupported")
        );
    }
}

#[test]
fn independent_starting_and_sustaining_obstacles_have_a_separate_atomic_obligation() {
    let f = source("有时无法开始练习，另外开始之后又会中断。");
    let packet = extraction_review(
        &extraction(discussion(&f, "无法开始和开始后中断合为一个障碍")),
        &[f],
    )
    .unwrap();
    assert!(
        packet["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["checkId"] == "d0.a")
    );
    let failed = reject(&packet, "semantic_discussion_non_atomic", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_discussion_non_atomic")
    );
}

#[test]
fn empty_no_signal_still_requires_review_of_the_actual_source() {
    let f = source("这是一个清楚而低频的练习困难。");
    let mut output = extraction(discussion(&f, "有一个练习困难"));
    output.outcome = "no_signal".into();
    output.discussions.clear();
    let packet = extraction_review(&output, std::slice::from_ref(&f)).unwrap();
    assert_eq!(packet["checks"][0]["checkId"], "o0");
    assert_eq!(packet["outcomeContext"][0]["text"], f.text);
    assert_eq!(packet["outcomeContext"][0]["contextOnly"], true);
    let failed = reject(&packet, "semantic_outcome_unsupported", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_outcome_unsupported")
    );
}

#[test]
fn repeated_exact_evidence_is_shared_but_parent_context_cannot_support_alone() {
    let mut f = source("用于理解评论的合成父文");
    f.field = "parent_comment_context".into();
    let d = discussion(&f, "合成上下文陈述");
    let mut output = extraction(d.clone());
    output.discussions.push(d);
    let packet = extraction_review(&output, &[f]).unwrap();
    assert_eq!(packet["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(packet["evidence"][0]["contextOnly"], true);
    assert_eq!(packet["checks"].as_array().unwrap().len(), 6);
}

#[test]
fn every_derived_field_has_its_own_obligation_without_repeating_the_window() {
    use super::super::{Angle, ProductOpportunity, ResponseMatch, Scene};
    let f = source("合成评论讨论一次练习中的障碍");
    let d = discussion(&f, "合成评论讨论练习障碍");
    let mut output = extraction(d.clone());
    output.scenes.push(Scene {
        label: "合成现场".into(),
        evidence: d.evidence.clone(),
    });
    output.response_matches.push(ResponseMatch {
        status: "unknown".into(),
        unanswered: vec!["如何降低起步难度".into()],
        evidence: d.evidence.clone(),
    });
    output.angles.push(Angle {
        label: "合成角度".into(),
        title: "起步支持的条件".into(),
        answer_task: "检验哪些条件适用".into(),
        evidence: d.evidence.clone(),
    });
    output.product_opportunities.push(ProductOpportunity {
        need: "起步支持".into(),
        hypothesis: "可能需要提示工具".into(),
        verification_question: "是否真的需要工具".into(),
        alternative_explanation: "可能只需要更多理解".into(),
        evidence: d.evidence.clone(),
    });
    let packet = extraction_review(&output, &[f]).unwrap();
    let ids: Vec<_> = packet["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["checkId"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["o0", "d0.s", "d0.a", "s0", "j0", "r0", "a0", "p0"]);
    assert_eq!(packet["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(packet["outcomeContext"], json!([]));
    for field in [
        "scenes",
        "responseMatches",
        "angles",
        "productOpportunities",
    ] {
        assert_eq!(packet["proposal"][field][0]["evidenceIds"], json!(["e0"]));
    }
    assert_eq!(
        packet["proposal"]["productOpportunities"][0]["alternativeExplanation"],
        "可能只需要更多理解"
    );
}

fn resolution_case() -> (
    Vec<(String, Discussion)>,
    ResolutionOutput,
    Value,
    Vec<Fragment>,
) {
    let f = source("我愿意练习，但是已经开始之后会反复中断。");
    let units = vec![(
        "a".repeat(64),
        discussion(&f, "有练习意愿但开始之后持续困难"),
    )];
    let topic = Uuid::new_v4();
    let definition = Uuid::new_v4();
    let candidates = json!([{"topicRef":topic,"definitionRef":definition,"label":"任务启动支持",
        "definition":"成年者本人在任务尚未开始前的启动困难",
        "inclusionCriteria":["本人描述尚未开始第一步的障碍"],
        "exclusionCriteria":["第三人称支持","开始后的持续困难"],
        "qualityState":"current_candidate","conceptRole":"leaf","embedding":[1,2,3]}]);
    let resolution = ResolutionOutput {
        contract: RESOLVE_CONTRACT.into(),
        decisions: vec![UnitDecision {
            unit_id: units[0].0.clone(),
            status: "matched".into(),
            matches: vec![TopicMatch {
                topic_ref: topic,
                definition_ref: definition,
                reason: "合成错误地把持续当启动".into(),
            }],
            proposed_topic: None,
            relations: vec![],
            reason: "共同涉及练习不足以匹配".into(),
        }],
    };
    (units, resolution, candidates, vec![f])
}

#[test]
fn matching_checks_keep_exact_inclusion_and_exclusion_and_all_candidates() {
    let (units, resolution, candidates, fragments) = resolution_case();
    let packet = resolution_review(&units, &resolution, &candidates, &fragments).unwrap();
    assert_eq!(
        packet["candidates"][0]["inclusionCriteria"],
        candidates[0]["inclusionCriteria"]
    );
    assert_eq!(
        packet["candidates"][0]["exclusionCriteria"],
        candidates[0]["exclusionCriteria"]
    );
    assert!(packet["candidates"][0].get("embedding").is_none());
    let failed = reject(&packet, "semantic_assignment_boundary", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_assignment_boundary")
    );
}

#[test]
fn a_child_that_can_fit_without_fitting_its_parent_cannot_be_accepted() {
    let (units, mut resolution, candidates, fragments) = resolution_case();
    let matched = resolution.decisions[0].matches.remove(0);
    resolution.decisions[0].status = "new".into();
    resolution.decisions[0].proposed_topic = Some(ProposedTopic {
        label: "练习持续支持".into(),
        definition: "已经开始练习后的持续执行困难".into(),
        inclusion_criteria: vec!["已经开始后中断".into()],
        exclusion_criteria: vec!["尚未开始".into()],
        domain_fit: "in_scope".into(),
        domain_reason: "合成领域".into(),
        abstraction_reason: "跨场景持续困难".into(),
        parent: ProposedParent {
            kind: "existing".into(),
            topic_ref: Some(matched.topic_ref),
            definition_ref: Some(matched.definition_ref),
            proposal: None,
            reason: "合成错误地以相关当包含".into(),
        },
    });
    let packet = resolution_review(&units, &resolution, &candidates, &fragments).unwrap();
    assert!(
        packet["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["checkId"] == "u0.p")
    );
    assert_eq!(
        packet["decisions"][0]["proposedTopic"]["parent"]["definitionRef"],
        json!(matched.definition_ref)
    );
    let failed = reject(&packet, "semantic_parent_containment", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_parent_containment")
    );
}

#[test]
fn all_expected_checks_exactly_once_and_only_all_pass_can_be_accepted() {
    let f = source("合成练习障碍");
    let packet = extraction_review(&extraction(discussion(&f, "合成练习障碍")), &[f]).unwrap();
    let good = all_pass(&packet);
    assert!(validate_verification(&good, &packet["checks"]).is_ok());
    let mut reordered = good.clone();
    reordered.checks.reverse();
    assert!(validate_verification(&reordered, &packet["checks"]).is_ok());
    for mode in ["missing", "extra", "duplicate", "unknown", "unsafe"] {
        let mut bad = good.clone();
        match mode {
            "missing" => {
                bad.checks.pop();
            }
            "extra" => bad.checks.push(bad.checks[0].clone()),
            "duplicate" => bad.checks[1] = bad.checks[0].clone(),
            "unknown" => bad.checks[0].check_id = "external0".into(),
            _ => bad.checks[0].check_id = "raw material / injected".into(),
        }
        assert_eq!(
            validate_verification(&bad, &packet["checks"]),
            Err("semantic_review_coverage_invalid")
        );
    }
    assert_eq!(
        validate_verification(
            &reject(&packet, "semantic_discussion_unsupported", "uncertain"),
            &packet["checks"]
        ),
        Err("semantic_review_uncertain")
    );
}

#[test]
fn strict_contract_rejects_provider_edits_unknown_fields_and_invalid_reasons() {
    let f = source("合成练习障碍");
    let packet = extraction_review(&extraction(discussion(&f, "合成练习障碍")), &[f]).unwrap();
    let mut good = all_pass(&packet);
    let mut extra = serde_json::to_value(&good).unwrap();
    extra["proposal"] = json!({"corrected":true});
    assert!(serde_json::from_value::<VerificationOutput>(extra).is_err());
    for key in ["rule", "subject", "thoughts"] {
        let mut copied = serde_json::to_value(&good).unwrap();
        copied["checks"][0][key] = packet["checks"][0]["rule"].clone();
        assert!(serde_json::from_value::<VerificationOutput>(copied).is_err());
    }
    assert_eq!(
        verify_schema()["properties"]["checks"]["items"]["required"],
        json!(["checkId", "verdict", "reason"])
    );
    assert_eq!(
        verify_schema()["properties"]["checks"]["items"]["properties"]
            .as_object()
            .unwrap()
            .len(),
        3
    );
    good.checks[0].verdict = "maybe".into();
    assert_eq!(
        validate_verification(&good, &packet["checks"]),
        Err("semantic_review_check_invalid")
    );
    good.checks[0].verdict = "pass".into();
    good.checks[0].reason = "甲".repeat(61);
    assert_eq!(
        validate_verification(&good, &packet["checks"]),
        Err("semantic_review_check_invalid")
    );
    good.checks[0].reason = " ".into();
    assert_eq!(
        validate_verification(&good, &packet["checks"]),
        Err("semantic_review_check_invalid")
    );
    assert_eq!(
        verify_schema()["properties"]["checks"]["items"]["additionalProperties"],
        false
    );
    assert_eq!(
        verify_schema()["properties"]["checks"]["items"]["properties"]["reason"]["maxLength"],
        60
    );
}

#[test]
fn malformed_expected_checks_sources_and_units_never_create_an_empty_green_review() {
    let f = source("合成练习障碍");
    let mut output = extraction(discussion(&f, "合成练习障碍"));
    let packet = extraction_review(&output, std::slice::from_ref(&f)).unwrap();
    assert_eq!(
        validate_verification(&all_pass(&packet), &json!([])),
        Err("semantic_review_expected_invalid")
    );
    let mut expected = packet["checks"].clone();
    expected[0]["rule"] = json!("provider_chosen_rule");
    assert_eq!(
        validate_verification(&all_pass(&packet), &expected),
        Err("semantic_review_expected_invalid")
    );
    output.discussions[0].evidence[0].end = f.end + 1;
    assert_eq!(
        extraction_review(&output, &[f]).unwrap_err(),
        "semantic_review_source_invalid"
    );
    let (units, mut resolution, candidates, fragments) = resolution_case();
    resolution.decisions.clear();
    assert_eq!(
        resolution_review(&units, &resolution, &candidates, &fragments).unwrap_err(),
        "semantic_review_proposal_invalid"
    );
}
