use super::*;

#[test]
fn comparison_scope_is_a_bounded_set_and_reordering_reuses_identity() {
    let a = Uuid::from_u128(1);
    let b = Uuid::from_u128(2);
    assert_eq!(scope(&json!({"workRefs":[b,a]})), Some(vec![a, b]));
    assert_eq!(scope(&json!({"workRefs":[a]})), Some(vec![a]));
    for invalid in [
        json!({}),
        json!({"workRefs":[]}),
        json!({"workRefs":[a,a]}),
        json!({"workRefs":[a,"invalid"]}),
        json!({"workRefs":(1..=11).map(Uuid::from_u128).collect::<Vec<_>>()}),
    ] {
        assert!(scope(&invalid).is_none());
    }
    let (inputs, tasks, requested) = pair();
    let first = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let second = build_comparison(&inputs, &tasks, &[b, a], &topics()).unwrap();
    assert_eq!(first.hash, second.hash);
    assert_eq!(first.coverage["scopeWorkRefs"], json!([a, b]));
}

#[test]
fn comparison_uses_real_work_metadata_and_absolute_unicode_citations_without_raw_storage() {
    let a_source = source(
        1,
        11,
        "body",
        &format!("{}甲🐈正文片段尾声未选入秘密", "占".repeat(2500)),
    );
    let a = input(1, vec![a_source.clone()]);
    let a_window = research::research_windows(&a, 3000)
        .into_iter()
        .find(|w| w.fragments.iter().any(|f| f.start <= 2500 && f.end >= 2508))
        .unwrap();
    let a_fragment = a_window
        .fragments
        .iter()
        .find(|f| f.start <= 2500 && f.end >= 2508)
        .unwrap();
    let b = input(
        2,
        vec![source(
            2,
            22,
            "unresearched_comment",
            "乙🐙评论原声提供反例未选入秘密",
        )],
    );
    let tasks = vec![
        task(
            &a_window,
            1001,
            vec![unit("a", a_fragment, 2500, 2508, "author", "support", 100)],
        ),
        task(
            &b,
            1002,
            vec![unit(
                "b",
                &b.fragments[0],
                0,
                7,
                "commenter",
                "challenge",
                100,
            )],
        ),
    ];
    let inputs = vec![a, b];
    let requested = vec![inputs[0].work.work_ref, inputs[1].work.work_ref];
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(comparison.work.work_ref, requested[0]);
    assert_eq!(comparison.context_work_refs, vec![requested[1]]);
    assert_eq!(comparison.work.fragments.len(), 1);
    assert_eq!(comparison.work.fragments[0].start, 2500);
    assert_eq!(comparison.work.fragments[0].text, "甲🐈正文片段尾声");
    assert_eq!(comparison.coverage["coveredChars"], 15);
    assert_eq!(comparison.coverage["state"], "partial");
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 2);
    assert_eq!(comparison.coverage["countsArePeople"], false);
    assert!(comparison.coverage["sourceChars"].as_u64().unwrap() > 2500);
    let roles = comparison.role_metadata["works"].as_array().unwrap();
    assert!(roles.iter().any(|r| r["workRef"] == json!(requested[0])
        && r["platform"] == "xhs"
        && r["own"] == true));
    assert!(roles.iter().any(|r| r["workRef"] == json!(requested[1])
        && r["platform"] == "douyin"
        && r["own"] == false
        && r["likes"] == 20));
    assert_eq!(comparison.comment_study[0]["workRef"], json!(requested[1]));
    let manifest = research::reference_manifest(&comparison);
    assert!(research::restore_scoped_window(&inputs, requested[0], &manifest).is_some());
    assert!(!manifest.to_string().contains("甲🐈正文片段尾声"));
    assert!(!manifest.to_string().contains("乙🐙评论原声"));
    assert!(!manifest.to_string().contains("未选入秘密"));
    for reference in manifest["fragments"].as_array().unwrap() {
        let id = reference["fragmentId"].as_str().unwrap();
        assert_eq!(
            reference["workRef"],
            comparison.coverage["fragmentWorkRefs"][id]
        );
        assert_eq!(
            reference["sourceFragmentId"],
            comparison.coverage["fragmentOrigins"][id]
        );
        assert!(reference.get("text").is_none());
    }
}

#[test]
fn withdrawal_changed_text_role_or_used_definition_invalidates_the_whole_comparison() {
    let (inputs, tasks, requested) = pair();
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let manifest = research::reference_manifest(&comparison);
    assert!(research::restore_scoped_window(&inputs[..1], requested[0], &manifest).is_none());
    let mut withdrawn = inputs.clone();
    withdrawn[1].fragments.clear();
    assert!(research::restore_scoped_window(&withdrawn, requested[0], &manifest).is_none());
    assert_eq!(
        build_comparison(&withdrawn, &tasks, &requested, &topics()).unwrap_err(),
        "comparison_requires_two_current_discussion_sources"
    );
    let mut version = inputs.clone();
    version[1].fragments[0].source_version = "changed".into();
    version[1].fragments[0].text = version[1].fragments[0].text.replacen('评', "变", 1);
    assert!(research::restore_scoped_window(&version, requested[0], &manifest).is_none());
    let mut role = inputs.clone();
    role[1].role_metadata["own"] = json!(true);
    assert!(research::restore_scoped_window(&role, requested[0], &manifest).is_none());
    let mut definition = inputs.clone();
    for input in &mut definition {
        input.topics[0]["definitionRef"] = json!(Uuid::from_u128(199));
    }
    assert!(research::restore_scoped_window(&definition, requested[0], &manifest).is_none());
    let mut unrelated = inputs.clone();
    for input in &mut unrelated {
        input.topics[1]["definitionRef"] = json!(Uuid::from_u128(299));
    }
    assert!(research::restore_scoped_window(&unrelated, requested[0], &manifest).is_some());
}

#[test]
fn equal_text_reobservation_keeps_semantic_comparison_and_frozen_audit_origin() {
    let (inputs, tasks, requested) = pair();
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let manifest = research::reference_manifest(&comparison);
    let mut reobserved = inputs.clone();
    reobserved[1].fragments[0].source_ref = Uuid::from_u128(999);
    reobserved[1].fragments[0].source_version = "new-physical-observation".into();
    reobserved[1].comment_study[0]["sourceRef"] = json!(Uuid::from_u128(999));
    let current = research::restore_scoped_window(&reobserved, requested[0], &manifest).unwrap();
    assert_eq!(current.hash, comparison.hash);
    let fragment = current
        .fragments
        .iter()
        .find(|f| f.field == "unresearched_comment")
        .unwrap();
    assert_eq!(fragment.source_ref, inputs[1].fragments[0].source_ref);
    assert_eq!(
        current.coverage["currentSources"][&fragment.fragment_id]["sourceRef"],
        json!(Uuid::from_u128(999))
    );
    assert_eq!(
        build_comparison(&reobserved, &tasks, &requested, &topics())
            .unwrap()
            .hash,
        comparison.hash
    );
    let mut reassigned = tasks.clone();
    reassigned[0].task = Uuid::from_u128(9999);
    let same_discussions =
        build_comparison(&reobserved, &reassigned, &requested, &topics()).unwrap();
    assert_eq!(same_discussions.hash, comparison.hash);
    assert_eq!(
        same_discussions.coverage["selectedDiscussions"][0]["taskRef"],
        json!(Uuid::from_u128(9999))
    );
}

#[test]
fn unknown_comparison_ranges_block_other_configs_and_work_wrappers() {
    let (inputs, tasks, requested) = pair();
    let first = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let mut other_config = inputs.clone();
    for input in &mut other_config {
        input.coverage["configRef"] = json!(Uuid::from_u128(400));
        input.hash = research::input_identity(input);
    }
    // The accepted source tasks already exist for this second configuration.
    let later_tasks: Vec<_> = other_config
        .iter()
        .zip(&tasks)
        .map(|(input, old)| {
            task(
                input,
                old.task.as_u128() + 10,
                old.resolutions.as_array().unwrap().clone(),
            )
        })
        .collect();
    let second = build_comparison(&other_config, &later_tasks, &requested, &topics()).unwrap();
    assert_ne!(first.hash, second.hash);
    let full =
        research::with_comparison_context(other_config[0].clone(), &other_config, &requested);
    let refs = research::reference_manifest(&first)["fragments"]
        .as_array()
        .unwrap()
        .clone();
    assert!(research::overlaps_unknown_dispatch(&second, &full, &refs));
    // A reference owned by the other work alone must also block a comparison.
    let other_refs: Vec<_> = refs
        .iter()
        .filter(|r| r["workRef"] == json!(requested[1]))
        .cloned()
        .collect();
    assert!(research::overlaps_unknown_dispatch(
        &second,
        &full,
        &other_refs
    ));
    let mut changed = full.clone();
    for fragment in &mut changed.fragments {
        fragment.text = "新".repeat(fragment.text.chars().count());
    }
    assert!(!research::overlaps_unknown_dispatch(
        &second, &changed, &refs
    ));
}

#[test]
fn unassigned_compared_definition_is_frozen_and_required_by_the_comparison() {
    let (inputs, mut tasks, requested) = pair();
    tasks[0].resolutions[0]["comparedDefinitionRefs"] = json!([
        Uuid::from_u128(201),
        Uuid::from_u128(101),
        Uuid::from_u128(201)
    ]);
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let discussion = &comparison.coverage["selectedDiscussions"][0];
    assert_eq!(
        discussion["comparedDefinitionRefs"],
        json!([Uuid::from_u128(101), Uuid::from_u128(201)])
    );
    assert_eq!(discussion["assignments"].as_array().unwrap().len(), 1);
    let mut changed = inputs.clone();
    for input in &mut changed {
        input.topics[1]["definitionRef"] = json!(Uuid::from_u128(299));
    }
    assert!(
        research::restore_scoped_window(
            &changed,
            requested[0],
            &research::reference_manifest(&comparison)
        )
        .is_none()
    );
    let mut catalog = topics();
    catalog[1]["definitionRef"] = json!(Uuid::from_u128(299));
    assert_eq!(
        build_comparison(&inputs, &tasks, &requested, &catalog).unwrap_err(),
        "comparison_requires_two_current_discussion_sources"
    );
}

#[test]
fn semantic_identity_ignores_equal_labels_and_clears_outdated_assignments() {
    let (inputs, mut tasks, requested) = pair();
    tasks[1].resolutions[0]["assignments"] = json!([assignment(200)]);
    let different = current_candidates(&inputs, &tasks, &requested, &topics());
    assert!(shared_topics(&different).is_empty());
    tasks[1].resolutions[0]["assignments"] = json!([assignment(100)]);
    tasks[1].resolutions[0]["assignments"][0]["label"] = json!("另一个标题");
    assert_eq!(
        shared_topics(&current_candidates(&inputs, &tasks, &requested, &topics())),
        BTreeSet::from([Uuid::from_u128(100).to_string()])
    );
    let mut changed = topics();
    changed[0]["definitionRef"] = json!(Uuid::from_u128(199));
    let candidates = current_candidates(&inputs, &tasks, &requested, &changed);
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|c| c.unit["status"] == "uncertain" && c.unit["assignments"] == json!([]))
    );
    assert!(shared_topics(&candidates).is_empty());
}

#[test]
fn hierarchy_parent_must_be_current_even_when_it_was_not_a_compared_candidate() {
    let (mut inputs, mut tasks, requested) = pair();
    let parent = Uuid::from_u128(300);
    let parent_definition = Uuid::from_u128(301);
    tasks[0].resolutions[0]["hierarchy"] =
        json!({"parentTopicRef":parent,"parentDefinitionRef":parent_definition,"state":"attached"});
    tasks[0].resolutions[0]["comparedDefinitionRefs"] = json!([]);
    tasks[0].resolutions[0]["relations"] = json!([]);
    let mut catalog = topics();
    catalog
        .as_array_mut()
        .unwrap()
        .push(json!({"topicRef":parent,"definitionRef":parent_definition,"label":"合成父概念"}));
    for input in &mut inputs {
        input.topics = catalog.clone();
    }
    let comparison = build_comparison(&inputs, &tasks, &requested, &catalog).unwrap();
    let manifest = research::reference_manifest(&comparison);
    assert!(research::restore_scoped_window(&inputs, requested[0], &manifest).is_some());
    assert_eq!(
        crate::topic_map_core::definition_dependencies(&manifest),
        BTreeSet::from([Uuid::from_u128(101), parent_definition])
            .into_iter()
            .collect()
    );
    let mut changed = inputs.clone();
    for input in &mut changed {
        input.topics[2]["definitionRef"] = json!(Uuid::from_u128(302));
    }
    assert!(research::restore_scoped_window(&changed, requested[0], &manifest).is_none());
    for input in &mut changed {
        input.topics = topics();
    }
    assert!(research::restore_scoped_window(&changed, requested[0], &manifest).is_none());
    catalog[2]["definitionRef"] = json!(Uuid::from_u128(302));
    assert!(build_comparison(&inputs, &tasks, &requested, &catalog).is_err());
    assert!(build_comparison(&inputs, &tasks, &requested, &topics()).is_err());
}
