use super::*;

#[test]
fn final_scope_counts_units_and_roles_without_display_name_merging() {
    let input = input("body");
    let draft = draft(&input);
    let unit = accepted(&input, &draft, 0, 100);
    let mut challenge = unit.clone();
    challenge["evidenceRole"] = json!("challenge");
    challenge["assignments"] = json!([assignment(200)]);
    let mut first = work(1, true);
    first.research = Some(json!({"core":{"units":[unit.clone(),unit]}}));
    let mut second = work(2, true);
    second.research = Some(json!({"core":{"units":[challenge]}}));
    let mut topics = vec![topic(100), topic(200)];
    // This is the exact final work collection handed to the reader after its filters.
    units::summarize_scope(&[second], &mut topics);
    assert_eq!(topics[0].core["discussionCount"], 0);
    assert_eq!(topics[1].core["discussionCount"], 1);
    assert_eq!(topics[1].core["evidenceRoles"]["challenge"], 1);
    units::summarize_scope(&[first], &mut topics);
    assert_eq!(topics[0].core["discussionCount"], 1);
    assert_eq!(topics[0].core["evidenceRoles"]["support"], 1);
    assert_eq!(topics[1].core["discussionCount"], 0);
    assert_eq!(topics[0].display_name, topics[1].display_name);
}

#[test]
fn multilabel_counts_once_per_topic_and_work_but_never_counts_unknown_membership() {
    let input = input("body");
    let draft = draft(&input);
    let mut unit = accepted(&input, &draft, 0, 100);
    unit["assignments"] = json!([assignment(100), assignment(100), assignment(200)]);
    let mut first = work(1, true);
    first.research = Some(json!({"core":{"units":[unit.clone()]}}));
    let mut second = work(2, true);
    second.research = first.research.clone();
    let mut unknown = work(3, true);
    unit["status"] = json!("uncertain");
    unknown.research = Some(json!({"core":{"units":[unit]}}));
    let mut topics = vec![topic(100), topic(200)];
    units::summarize_scope(&[first, second, unknown], &mut topics);
    assert_eq!(topics[0].core["discussionCount"], 2);
    assert_eq!(topics[1].core["discussionCount"], 2);
}

#[test]
fn withdrawn_compared_definition_clears_decision_but_preserves_source_discussion() {
    let input = input("body");
    let draft = draft(&input);
    let mut unit = accepted(&input, &draft, 0, 100);
    unit["comparedDefinitionRefs"] = json!([Uuid::from_u128(101), Uuid::from_u128(201)]);
    unit["relations"] = json!([{"topicRef":Uuid::from_u128(200),"definitionRef":Uuid::from_u128(201),
        "relation":"distinct","reason":"已撤回的机密主题定义"}]);
    unit["reason"] = json!("比较后排除了已撤回的机密主题定义");
    unit["recall"] = json!({"candidateLabel":"已撤回的机密主题定义"});
    let mut topics = vec![topic(100), topic(200)];
    units::apply_rule(
        &mut topics[1],
        Some((vec!["机密纳入".into()], vec!["机密排除".into()])),
        true,
    );
    let current = units::current_unit(
        &unit,
        Some(Uuid::from_u128(7)),
        &topics,
        &HashSet::from([Uuid::from_u128(201)]),
    );
    assert_eq!(current["status"], "uncertain");
    assert_eq!(current["assignments"], json!([]));
    assert_eq!(current["relations"], json!([]));
    assert!(current.get("recall").is_none());
    assert_eq!(current["statement"], unit["statement"]);
    assert_eq!(current["evidence"], unit["evidence"]);
    assert!(!current.to_string().contains("机密主题定义"));
    assert_eq!(topics[1].display_name, "来源受限主题");
    assert_eq!(topics[1].core["sourceState"], "source_unavailable");
    assert_eq!(topics[1].core["inclusionCriteria"], json!([]));
    assert_eq!(topics[1].core["exclusionCriteria"], json!([]));
    assert_eq!(topics[0].core["sourceState"], "available");
}

#[test]
fn old_definition_does_not_inherit_current_version_even_with_the_same_label() {
    let input = input("body");
    let mut draft = draft(&input);
    draft.discussions[0].topic_ref = Some(Uuid::from_u128(100));
    let manifest =
        json!({"topics":[{"topicRef":Uuid::from_u128(100),"definitionRef":Uuid::from_u128(101)}]});
    let legacy = legacy::legacy_units(Uuid::from_u128(6), &draft, &manifest);
    let mut topics = vec![topic(100)];
    topics[0].definition_ref = Uuid::from_u128(102);
    let unit = units::current_unit(
        &legacy["units"][0],
        Some(Uuid::from_u128(6)),
        &topics,
        &HashSet::new(),
    );
    assert_eq!(
        legacy["units"][0]["assignments"][0]["definitionRef"],
        json!(Uuid::from_u128(101))
    );
    assert_eq!(unit["status"], "uncertain");
    assert_eq!(unit["assignments"], json!([]));
    let missing = legacy::legacy_units(Uuid::from_u128(6), &draft, &json!({}));
    assert!(missing["units"][0]["assignments"][0]["definitionRef"].is_null());
}

#[test]
fn comparison_definition_dependency_withdrawal_hides_only_comparison_result() {
    let unavailable = HashSet::from([Uuid::from_u128(201)]);
    let mut manifest = json!({"coverage":{"kind":"comparison","selectedDiscussions":[{
        "assignments":[assignment(100)],"comparedDefinitionRefs":[Uuid::from_u128(201)]}]}});
    assert!(!load::comparison_definitions_current(
        &manifest,
        &unavailable
    ));
    manifest["coverage"]["kind"] = json!("source");
    assert!(load::comparison_definitions_current(
        &manifest,
        &unavailable
    ));
}

#[test]
fn withdrawn_definition_annotation_cannot_restore_membership_or_old_reason() {
    let mut work = work(1, true);
    work.annotation = Some(
        json!({"topic_ref":Uuid::from_u128(100),"definition_ref":Uuid::from_u128(101),
        "rationale":"旧定义的敏感说明"}),
    );
    work.main_stage = "begin_practice".into();
    work.path = "adult".into();
    units::invalidate_annotation(&mut work, &HashSet::from([Uuid::from_u128(201)]));
    assert!(work.annotation.is_some());
    units::invalidate_annotation(&mut work, &HashSet::from([Uuid::from_u128(101)]));
    assert!(work.annotation.is_none());
    assert_eq!(work.main_stage, "pending");
    assert_eq!(work.path, "unknown");
}
