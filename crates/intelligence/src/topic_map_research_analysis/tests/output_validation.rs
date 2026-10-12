use super::*;

fn response_case() -> (ResearchOutput, Vec<Fragment>) {
    let (mut output, mut fragments) = sample();
    fragments.push(fragment(
        "synthetic.comment",
        "unresearched_comment",
        30,
        "独立主评论",
    ));
    fragments.push(fragment(
        "synthetic.parent",
        "parent_comment_context",
        5,
        "父评论语境",
    ));
    fragments.push(fragment(
        "synthetic.context",
        "work_context:body",
        9,
        "作品语境",
    ));
    output.response_matches = vec![ResponseMatch {
        status: "direct".into(),
        unanswered: vec![],
        evidence: vec![citation(&fragments[0]), citation(&fragments[1])],
    }];
    (output, fragments)
}

fn assert_rejected_without_edit(
    output: &ResearchOutput,
    fragments: &[Fragment],
    code: &'static str,
) {
    let frozen = serde_json::to_value(output).unwrap();
    assert_eq!(validate_output(output, fragments, &[]), Err(code));
    assert_eq!(serde_json::to_value(output).unwrap(), frozen);
}

#[test]
fn response_status_and_unanswered_failures_are_specific_and_never_defaulted() {
    let (output, fragments) = response_case();
    assert_eq!(validate_output(&output, &fragments, &[]), Ok(()));
    let mut invalid = output.clone();
    invalid.response_matches[0].status = "PRIVATE-INVALID-STATUS".into();
    assert_rejected_without_edit(&invalid, &fragments, "response_status_invalid");
    invalid = output.clone();
    invalid.response_matches[0].unanswered = vec!["待验证问题".into(); 9];
    assert_rejected_without_edit(&invalid, &fragments, "response_unanswered_count_exceeded");
    for value in [" \t\n".into(), "😀".repeat(301)] {
        invalid = output.clone();
        invalid.response_matches[0].unanswered = vec![value];
        assert_rejected_without_edit(&invalid, &fragments, "response_unanswered_text_invalid");
    }
}

#[test]
fn response_citation_failures_keep_exact_source_rules_before_role_checks() {
    let (output, mut fragments) = response_case();
    let original = citation(&fragments[0]);
    for (evidence, code) in [
        (vec![], "citation_missing"),
        (vec![original.clone(); 13], "citation_count_exceeded"),
        (
            vec![Citation {
                fragment_id: "PRIVATE-FOREIGN-SOURCE".into(),
                ..original.clone()
            }],
            "citation_fragment_not_found",
        ),
        (
            vec![Citation {
                start: 6,
                ..original.clone()
            }],
            "citation_range_invalid",
        ),
        (
            vec![Citation {
                end: original.start,
                ..original.clone()
            }],
            "citation_range_invalid",
        ),
        (
            vec![Citation {
                end: original.end + 1,
                ..original.clone()
            }],
            "citation_range_invalid",
        ),
    ] {
        let mut invalid = output.clone();
        invalid.response_matches[0].evidence = evidence;
        assert_rejected_without_edit(&invalid, &fragments, code);
    }
    fragments.push(fragment("synthetic.long", "body", 2000, &"字".repeat(1001)));
    let mut invalid = output;
    invalid.response_matches[0].evidence = vec![citation(fragments.last().unwrap())];
    assert_rejected_without_edit(&invalid, &fragments, "citation_length_exceeded");
}

#[test]
fn response_identifies_which_primary_role_is_missing_and_context_cannot_supply_it() {
    let (output, fragments) = response_case();
    for status in ["direct", "partial", "unmatched"] {
        for (indexes, code) in [
            (vec![1, 2, 3], "response_author_evidence_missing"),
            (vec![0, 2, 3], "response_commenter_evidence_missing"),
            (vec![2, 3], "response_author_evidence_missing"),
        ] {
            let mut invalid = output.clone();
            invalid.response_matches[0].status = status.into();
            invalid.response_matches[0].evidence = indexes
                .into_iter()
                .map(|index| citation(&fragments[index]))
                .collect();
            assert_rejected_without_edit(&invalid, &fragments, code);
        }
        let mut valid = output.clone();
        valid.response_matches[0].status = status.into();
        assert_eq!(validate_output(&valid, &fragments, &[]), Ok(()));
    }
    let mut unknown = output;
    unknown.response_matches[0].status = "unknown".into();
    unknown.response_matches[0].evidence.clear();
    assert_eq!(validate_output(&unknown, &fragments, &[]), Ok(()));
    unknown.response_matches.clear();
    assert_eq!(validate_output(&unknown, &fragments, &[]), Ok(()));
}

#[test]
fn v5_remains_strict_historical_input_and_v1_keeps_its_original_role_contract() {
    let (mut output, fragments) = response_case();
    output.response_matches[0].evidence = vec![citation(&fragments[0])];
    for contract in [
        "topic-map.research.v2",
        "topic-map.research.v3",
        "topic-map.research.v4",
        "topic-map.research.v5",
        EXTRACT_CONTRACT,
    ] {
        output.contract = contract.into();
        assert_rejected_without_edit(&output, &fragments, "response_commenter_evidence_missing");
        assert_eq!(output.contract, contract);
    }
    output.contract = "topic-map.research.v1".into();
    assert_eq!(validate_output(&output, &fragments, &[]), Ok(()));
    assert_eq!(output.contract, "topic-map.research.v1");
}

#[test]
fn angle_fields_and_sources_fail_separately_without_synthesizing_a_replacement() {
    let (mut output, fragments) = sample();
    let angle = Angle {
        label: "任务支持".into(),
        title: "开始第一步".into(),
        answer_task: "解释任务启动支持".into(),
        evidence: vec![citation(&fragments[0])],
    };
    output.angles = vec![angle.clone()];
    assert_eq!(validate_output(&output, &fragments, &[]), Ok(()));
    for (field, max, code) in [
        (0, 120, "angle_label_invalid"),
        (1, 120, "angle_title_invalid"),
        (2, 500, "angle_answer_task_invalid"),
    ] {
        for value in [" ".into(), "😀".repeat(max + 1)] {
            output.angles[0] = angle.clone();
            match field {
                0 => output.angles[0].label = value,
                1 => output.angles[0].title = value,
                _ => output.angles[0].answer_task = value,
            }
            assert_rejected_without_edit(&output, &fragments, code);
        }
    }
    output.angles[0] = angle;
    output.angles[0].evidence.clear();
    assert_rejected_without_edit(&output, &fragments, "citation_missing");
}

#[test]
fn opportunity_fields_and_sources_fail_separately_without_filling_missing_hypotheses() {
    let (mut output, fragments) = sample();
    let opportunity = ProductOpportunity {
        need: "启动支持".into(),
        hypothesis: "小步骤或许有帮助".into(),
        verification_question: "是否更容易开始".into(),
        alternative_explanation: "也许任务较简单".into(),
        evidence: vec![citation(&fragments[0])],
    };
    output.product_opportunities = vec![opportunity.clone()];
    assert_eq!(validate_output(&output, &fragments, &[]), Ok(()));
    for (field, code) in [
        (0, "opportunity_need_invalid"),
        (1, "opportunity_hypothesis_invalid"),
        (2, "opportunity_verification_question_invalid"),
        (3, "opportunity_alternative_explanation_invalid"),
    ] {
        for value in [" ".into(), "😀".repeat(501)] {
            output.product_opportunities[0] = opportunity.clone();
            let opportunity = &mut output.product_opportunities[0];
            match field {
                0 => opportunity.need = value,
                1 => opportunity.hypothesis = value,
                2 => opportunity.verification_question = value,
                _ => opportunity.alternative_explanation = value,
            }
            assert_rejected_without_edit(&output, &fragments, code);
        }
    }
    output.product_opportunities[0] = opportunity;
    output.product_opportunities[0].evidence[0].fragment_id = "PRIVATE-FOREIGN-SOURCE".into();
    assert_rejected_without_edit(&output, &fragments, "citation_fragment_not_found");
}

#[test]
fn one_method_may_preserve_observation_and_future_plan_in_one_explicit_discussion() {
    let (mut output, _) = sample();
    let source = fragment(
        "synthetic.body",
        "body",
        18,
        "作为家长，我这次录视频看到孩子显得兴奋；以后我计划再尝试。",
    );
    let discussion = &mut output.discussions[0];
    discussion.statement = "这次录视频观察到孩子兴奋，家长计划继续尝试该方法。".into();
    discussion.definition = "使用录视频方法支持任务完成的观察与尝试计划".into();
    discussion.inclusion_criteria = vec!["录视频方法的实际反应或明确尝试计划".into()];
    discussion.exclusion_criteria = vec!["其他方法或已证明的长期效果".into()];
    discussion.evidence = vec![citation(&source)];
    let frozen = serde_json::to_value(&output).unwrap();
    assert_eq!(validate_output(&output, &[source], &[]), Ok(()));
    assert_eq!(output.discussions.len(), 1);
    assert_eq!(serde_json::to_value(output).unwrap(), frozen);
    // This proves structural admission only; the independent semantic review still must pass.
}
