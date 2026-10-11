//! Concept induction, source-context boundaries, and parent contract regressions.
use super::*;

pub(super) fn new_proposal() -> ProposedTopic {
    ProposedTopic {
        label: "合成持续注意".into(),
        definition: "任务开始以后维持注意的困难".into(),
        inclusion_criteria: vec!["开始后注意中断".into()],
        exclusion_criteria: vec!["还未开始第一步".into()],
        domain_fit: "in_scope".into(),
        domain_reason: "领域内的执行任务困难".into(),
        abstraction_reason: "持续阶段跨作业与工作可复用，场景不改变障碍".into(),
        parent: ProposedParent {
            kind: "root".into(),
            topic_ref: None,
            definition_ref: None,
            proposal: None,
            reason: "没有足够依据说明现有启动主题包含持续注意".into(),
        },
    }
}

#[test]
fn new_concept_requires_domain_fit_and_cannot_copy_a_specific_statement() {
    let (units, _, mut output) = resolution_case();
    let decision = &mut output.decisions[0];
    decision.status = "new".into();
    decision.matches.clear();
    decision.proposed_topic = Some(new_proposal());
    assert!(validate_resolution(&output, &units, &json!([])).is_ok());
    output.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .domain_fit = "uncertain".into();
    assert_eq!(
        validate_resolution(&output, &units, &json!([])),
        Err("invalid_new_topic_boundary")
    );
    output.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .domain_fit = "in_scope".into();
    output.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .definition = units[0].1.statement.clone();
    assert_eq!(
        validate_resolution(&output, &units, &json!([])),
        Err("invalid_new_topic_boundary")
    );
}

#[test]
fn existing_parent_requires_exact_version_and_scope_containment() {
    let (units, topics, mut output) = resolution_case();
    let target = output.decisions[0].matches.pop().unwrap();
    output.decisions[0].status = "new".into();
    let mut child = new_proposal();
    child.parent = ProposedParent {
        kind: "existing".into(),
        topic_ref: Some(target.topic_ref),
        definition_ref: Some(target.definition_ref),
        proposal: None,
        reason: "子概念的任务维持边界包含于执行任务过程，非共现".into(),
    };
    output.decisions[0].proposed_topic = Some(child);
    output.decisions[0].relations = vec![TopicRelation {
        topic_ref: target.topic_ref,
        definition_ref: target.definition_ref,
        relation: "narrower".into(),
        reason: "任务维持只是执行过程的一个阶段".into(),
    }];
    assert!(validate_resolution(&output, &units, &topics).is_ok());
    output.decisions[0].relations[0].relation = "related".into();
    assert_eq!(
        validate_resolution(&output, &units, &topics),
        Err("invalid_topic_parent")
    );
    output.decisions[0].relations[0].relation = "narrower".into();
    output.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .parent
        .definition_ref = Some(Uuid::new_v4());
    assert_eq!(
        validate_resolution(&output, &units, &topics),
        Err("invalid_topic_parent")
    );
}

#[test]
fn inferred_parent_keeps_boundaries_and_no_recursive_tree_or_count_gate() {
    let (units, _, mut output) = resolution_case();
    output.decisions[0].matches.clear();
    output.decisions[0].status = "new".into();
    let mut child = new_proposal();
    child.parent = ProposedParent {
        kind: "proposed".into(),
        topic_ref: None,
        definition_ref: None,
        proposal: Some(ConceptDefinition {
            label: "任务执行支持".into(),
            definition: "帮助开始并维持已选定任务的支持过程".into(),
            inclusion_criteria: vec!["任务开始或执行中的具体障碍与支持".into()],
            exclusion_criteria: vec!["仅讨论选择哪些任务".into()],
            domain_fit: "in_scope".into(),
            domain_reason: "当前领域讨论执行支持".into(),
            abstraction_reason: "持续注意实例支持执行过程这一上位范围，未声明其他实例已有证据"
                .into(),
        }),
        reason: "持续注意是任务执行支持的一个具体阶段".into(),
    };
    output.decisions[0].proposed_topic = Some(child);
    assert!(validate_resolution(&output, &units, &json!([])).is_ok());
    assert_serialized_shape(
        &serde_json::to_value(&output).unwrap(),
        &resolution_schema(),
    );
    let mut raw = serde_json::to_value(&output).unwrap();
    raw["decisions"][0]["proposedTopic"]["parent"]["proposal"]["parent"] = json!({"kind":"root"});
    assert!(serde_json::from_value::<ResolutionOutput>(raw).is_err());
    let label = output.decisions[0]
        .proposed_topic
        .as_ref()
        .unwrap()
        .label
        .clone();
    output.decisions[0]
        .proposed_topic
        .as_mut()
        .unwrap()
        .parent
        .proposal
        .as_mut()
        .unwrap()
        .label = label;
    assert_eq!(
        validate_resolution(&output, &units, &json!([])),
        Err("invalid_topic_parent")
    );
}

#[test]
fn work_context_is_not_an_independent_author_or_response_witness() {
    let (mut output, mut fragments) = sample();
    let context = fragment(
        "synthetic.context",
        "work_context:body",
        0,
        "用于理解评论的正文",
    );
    fragments.push(context.clone());
    output.discussions[0].evidence = vec![citation(&context)];
    assert!(validate_output(&output, &fragments, &[]).is_err());
    output.discussions[0].speaker_role = "unknown".into();
    output.discussions[0].evidence_role = "context".into();
    assert!(validate_output(&output, &fragments, &[]).is_ok());
    let comment = fragment(
        "synthetic.primary",
        "unresearched_comment",
        0,
        "具体评论经历",
    );
    fragments.push(comment.clone());
    output.response_matches = vec![ResponseMatch {
        status: "direct".into(),
        unanswered: vec![],
        evidence: vec![citation(&context), citation(&comment)],
    }];
    assert_eq!(
        validate_output(&output, &fragments, &[]),
        Err("invalid_response_evidence")
    );
}

#[test]
fn historical_v2_output_retains_strict_source_checks_after_v3_upgrade() {
    let (mut output, fragments) = sample();
    output.contract = "topic-map.research.v2".into();
    assert!(validate_output(&output, &fragments, &[]).is_ok());
    output.discussions[0].statement.clear();
    assert_eq!(
        validate_output(&output, &fragments, &[]),
        Err("invalid_discussion_evidence")
    );
}

#[test]
fn a_normalized_self_parent_is_rejected_before_acceptance() {
    let (units, _, mut output) = resolution_case();
    output.decisions[0].matches.clear();
    output.decisions[0].status = "new".into();
    let mut child = new_proposal();
    child.definition = "SYNTHETIC task scope".into();
    child.parent = ProposedParent {
        kind: "proposed".into(),
        topic_ref: None,
        definition_ref: None,
        proposal: Some(ConceptDefinition {
            label: "换名父概念".into(),
            definition: "synthetic   task scope".into(),
            inclusion_criteria: child.inclusion_criteria.clone(),
            exclusion_criteria: child.exclusion_criteria.clone(),
            domain_fit: "in_scope".into(),
            domain_reason: "合成范围相关".into(),
            abstraction_reason: "声称抽象但同一身份".into(),
        }),
        reason: "合成错误自环建议".into(),
    };
    output.decisions[0].proposed_topic = Some(child);
    assert_eq!(
        validate_resolution(&output, &units, &json!([])),
        Err("invalid_topic_parent")
    );
}

#[test]
fn a_grade_specific_legacy_candidate_does_not_block_a_reusable_evidence_based_concept() {
    let (mut units, mut candidates, mut output) = resolution_case();
    units[0].1.statement = "家长询问三年级孩子读写时怎样分步练习".into();
    candidates[0]["qualityState"] = json!("legacy_candidate");
    candidates[0]["inductionMethod"] = json!("topic-map.research.v2");
    candidates[0]["label"] = json!("三年级家长求阅读书写方法分享");
    candidates[0]["definition"] = json!("仅限三年级孩子家长询问读写方法分享");
    let old = output.decisions[0].matches.pop().unwrap();
    output.decisions[0].status = "new".into();
    output.decisions[0].proposed_topic = Some(ProposedTopic {
        label: "读写练习支持".into(),
        definition: "为阅读与书写练习中的具体困难寻找可执行支持".into(),
        inclusion_criteria: vec!["关于读写练习困难与具体支持方式的询问或经历".into()],
        exclusion_criteria: vec!["仅询问学年安排而未涉及读写练习支持".into()],
        domain_fit: "in_scope".into(),
        domain_reason: "原声讨论本领域的读写练习支持问题".into(),
        abstraction_reason:
            "三年级是该原声的实例属性，支持方向并不由年级定义；未宣称其他年级已有样本".into(),
        parent: ProposedParent {
            kind: "root".into(),
            topic_ref: None,
            definition_ref: None,
            proposal: None,
            reason: "没有足够证据确定一个包含读写练习支持的现有上位主题".into(),
        },
    });
    output.decisions[0].relations = vec![TopicRelation {
        topic_ref: old.topic_ref,
        definition_ref: old.definition_ref,
        relation: "broader".into(),
        reason: "稳定方向包含三年级这个场景，但不把年级作为纳入的必要条件".into(),
    }];
    assert!(validate_resolution(&output, &units, &candidates).is_ok());
    assert_eq!(
        output.decisions[0].relations[0].topic_ref, old.topic_ref,
        "the old candidate remains a separate comparison identity"
    );
    assert!(
        output.decisions[0]
            .proposed_topic
            .as_ref()
            .unwrap()
            .parent
            .topic_ref
            .is_none()
    );
    // This proves the strict contract admits the deliberate business decision;
    // the deterministic example cannot establish a provider's semantic quality.
}

#[test]
fn grouped_comments_cannot_attach_another_childs_parent_context() {
    let (mut output, _) = sample();
    let a = fragment("synthetic.a", "unresearched_comment", 0, "我也是");
    let b = fragment("synthetic.b", "studied_comment", 0, "这不同");
    let pa = fragment(
        "synthetic.parent-a",
        "parent_comment_context",
        3,
        "关于开始任务的讨论",
    );
    let pb = fragment(
        "synthetic.parent-b",
        "parent_comment_context",
        12,
        "关于持续执行的讨论",
    );
    let fragments = vec![a.clone(), b.clone(), pa.clone(), pb.clone()];
    let mapping = json!([
        {"fragmentId":a.fragment_id,"parentFragmentIds":[pa.fragment_id]},
        {"fragmentId":b.fragment_id,"parentFragmentIds":[pb.fragment_id]},
    ]);
    output.discussions[0].speaker_role = "commenter".into();
    output.discussions[0].evidence = vec![citation(&a), citation(&pa)];
    assert!(validate_output(&output, &fragments, &[]).is_ok());
    assert!(validate_context_links(&output, &fragments, &mapping).is_ok());
    output.discussions[0].evidence[1] = citation(&pb);
    assert!(
        validate_output(&output, &fragments, &[]).is_ok(),
        "valid text alone cannot establish parenthood"
    );
    assert_eq!(
        validate_context_links(&output, &fragments, &mapping),
        Err("invalid_comment_parent_context")
    );
    output.discussions[0].evidence.push(citation(&b));
    assert!(
        validate_context_links(&output, &fragments, &mapping).is_ok(),
        "each parent now has its own cited child"
    );
    assert_eq!(
        validate_context_links(&output, &fragments, &json!([])),
        Err("invalid_comment_parent_context")
    );
}

#[test]
fn all_explicit_parent_ranges_remain_valid_but_unrelated_analysis_is_rejected() {
    let (mut output, _) = sample();
    let child = fragment("synthetic.primary", "unresearched_comment", 0, "我也是");
    let p1 = fragment(
        "synthetic.parent.chars.0.10",
        "parent_comment_context",
        0,
        "家长准备开始练习的步骤",
    );
    let p2 = fragment(
        "synthetic.parent.chars.10.20",
        "parent_comment_context",
        10,
        "随后描述提醒与支持方法",
    );
    let wrong = fragment(
        "synthetic.foreign-parent",
        "parent_comment_context",
        0,
        "其他人的无关父文",
    );
    let fragments = vec![child.clone(), p1.clone(), p2.clone(), wrong.clone()];
    let mapping = json!([{"fragmentId":child.fragment_id,"parentFragmentIds":[p1.fragment_id,p2.fragment_id]}]);
    output.discussions[0].speaker_role = "commenter".into();
    output.discussions[0].evidence = vec![citation(&child), citation(&p1), citation(&p2)];
    assert!(validate_context_links(&output, &fragments, &mapping).is_ok());
    output.angles.push(Angle {
        label: "合成角度".into(),
        title: "合成题目".into(),
        answer_task: "解释已引用的原声".into(),
        evidence: vec![citation(&child), citation(&wrong)],
    });
    assert_eq!(
        validate_context_links(&output, &fragments, &mapping),
        Err("invalid_comment_parent_context")
    );
}
