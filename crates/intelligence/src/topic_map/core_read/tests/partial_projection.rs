use super::*;

#[test]
fn partial_acceptance_requires_exact_source_identity_roles_and_unicode_ranges() {
    let input = input("unresearched_comment");
    let draft = draft(&input);
    let good = accepted(&input, &draft, 0, 100);
    let (output, core) =
        partial::payload(&draft, &input, &json!([good.clone(), good.clone()])).unwrap();
    assert_eq!(output["discussions"].as_array().unwrap().len(), 1);
    assert_eq!(core["units"].as_array().unwrap().len(), 1);
    assert_eq!(core["units"][0]["evidence"][0]["start"], 10_001);
    assert_eq!(output["angles"], json!([]));
    assert_eq!(output["scenes"], json!([]));
    assert_eq!(output["journey"]["mainStage"], "unclear");
    for changed in [
        "speakerRole",
        "statement",
        "evidence",
        "unitId",
        "unitRef",
        "resolutionRef",
    ] {
        let mut bad = good.clone();
        bad[changed] = if changed == "evidence" {
            json!([{"fragmentId":input.fragments[0].fragment_id,"start":10_001,"end":10_002}])
        } else {
            json!("foreign-or-altered")
        };
        assert!(
            partial::payload(&draft, &input, &json!([bad])).is_none(),
            "{changed}"
        );
    }
    assert!(partial::payload(&draft, &input, &json!([])).is_none());
    let mut contradictory = good;
    contradictory["status"] = json!("uncertain");
    assert!(partial::payload(&draft, &input, &json!([contradictory.clone()])).is_none());
    contradictory["assignments"] = json!([]);
    assert!(partial::payload(&draft, &input, &json!([contradictory])).is_some());
}

#[test]
fn comment_only_partial_is_readable_without_forging_a_result_or_author_journey() {
    let mut input = input("unresearched_comment");
    let mut parent = input.fragments[0].clone();
    parent.field = "parent_comment_context".into();
    parent.fragment_id = "synthetic-parent-context".into();
    parent.source_ref = Uuid::from_u128(9);
    input.fragments.push(parent);
    let draft = draft(&input);
    let accepted = accepted(&input, &draft, 0, 100);
    let (output, core) = partial::payload(&draft, &input, &json!([accepted])).unwrap();
    let mut work = work(1, false);
    let mut topics = vec![topic(100)];
    projection::project_work(
        &mut work,
        vec![window(input, output, core, None, "2026-10-10")],
        &mut topics,
        &HashSet::new(),
    );
    let research = work.research.as_ref().unwrap();
    assert!(!work.readable);
    assert_eq!(research["readable"], true);
    assert_eq!(research["authorSourceState"], "source_unavailable");
    assert_eq!(research["commentSourceState"], "available");
    assert_eq!(research["state"], "partial");
    assert!(research["resultRef"].is_null());
    assert_eq!(research["resultRefs"], json!([]));
    assert_eq!(research["partialTaskRefs"], json!([Uuid::from_u128(99)]));
    assert_eq!(research["output"]["angles"], json!([]));
    assert!(research["core"]["units"][0]["researchResultRef"].is_null());
    assert_eq!(research["core"]["coverage"]["state"], "partial");
    assert_eq!(research["core"]["coverage"]["completedWindows"], 0);
    assert_eq!(
        research["core"]["coverage"]["coveredChars"],
        research["core"]["coverage"]["sourceChars"]
    );
    assert_eq!(research["fragments"].as_array().unwrap().len(), 2);
    assert_eq!(work.main_stage, "pending");
    assert_eq!(work.path, "unknown");
    assert_eq!(topics[0].direct_work_refs, vec![work.work_ref]);
}

#[test]
fn current_partial_does_not_inherit_completion_from_previous_window_revision() {
    let input = input("body");
    let draft = draft(&input);
    let accepted = accepted(&input, &draft, 0, 100);
    let (output, core) = partial::payload(&draft, &input, &json!([accepted.clone()])).unwrap();
    let partial = window(input.clone(), output, core, None, "2026-10-10");
    let completed = window(
        input,
        serde_json::to_value(draft).unwrap(),
        json!({"units":[accepted]}),
        Some(50),
        "2026-10-09",
    );
    let mut coverage = work_summary::Coverage::default();
    coverage.add(&partial);
    coverage.add(&completed);
    let coverage = coverage.value();
    assert_eq!(coverage["coveredChars"], coverage["sourceChars"]);
    assert_eq!(coverage["completedWindows"], 0);
    assert_eq!(coverage["state"], "partial");
    let mut newer_completed = work_summary::Coverage::default();
    newer_completed.add(&completed);
    newer_completed.add(&partial);
    assert_eq!(newer_completed.value()["state"], "complete");
}

#[test]
fn completed_result_keeps_real_save_indexes_and_comments_never_classify_author_stage() {
    let input = input("studied_comment");
    let draft = draft(&input);
    let unit = accepted(&input, &draft, 0, 100);
    let mut work = work(1, false);
    projection::project_work(
        &mut work,
        vec![window(
            input,
            serde_json::to_value(draft).unwrap(),
            json!({"units":[unit]}),
            Some(50),
            "today",
        )],
        &mut [topic(100)],
        &HashSet::new(),
    );
    let research = work.research.as_ref().unwrap();
    assert_eq!(
        research["output"]["angles"][0]["researchResultRef"],
        json!(Uuid::from_u128(50))
    );
    assert_eq!(research["output"]["angles"][0]["researchAngleIndex"], 0);
    assert_eq!(research["output"]["journey"]["mainStage"], "unclear");
    assert_eq!(work.main_stage, "pending");
}

#[test]
fn comparison_does_not_count_primary_memberships_coverage_or_author_stages() {
    let input = input("body");
    let draft = draft(&input);
    let unit = accepted(&input, &draft, 0, 100);
    let mut comparison = window(
        input,
        serde_json::to_value(draft).unwrap(),
        json!({"units":[unit]}),
        Some(50),
        "today",
    );
    comparison.manifest["coverage"]["kind"] = json!("comparison");
    let mut work = work(1, true);
    let mut topics = vec![topic(100)];
    projection::project_work(&mut work, vec![comparison], &mut topics, &HashSet::new());
    assert!(topics[0].direct_work_refs.is_empty());
    let research = work.research.as_ref().unwrap();
    assert_eq!(research["core"]["units"], json!([]));
    assert_eq!(research["core"]["coverage"]["completedWindows"], 0);
    assert_eq!(work.main_stage, "pending");
}

#[test]
fn repeated_configs_cannot_complete_unread_parent_context_pages() {
    let mut source = input("unresearched_comment");
    source.coverage["windowCount"] = json!(3);
    let mut parent = source.fragments[0].clone();
    parent.field = "parent_comment_context".into();
    parent.fragment_id = "logical-parent".into();
    parent.start = 0;
    parent.end = 3;
    parent.text = "上下文".into();
    source.fragments.push(parent);
    let mut coverage = work_summary::Coverage::default();
    for config in 1..=3 {
        let mut revision = source.clone();
        revision.coverage["windowKey"] = json!(format!("config-{config}-first-page"));
        revision.coverage["configRef"] = json!(Uuid::from_u128(config));
        revision.fragments[0].source_ref = Uuid::from_u128(100 + config);
        coverage.add(&window(
            revision,
            json!({}),
            json!({}),
            Some(config),
            "today",
        ));
    }
    let result = coverage.value();
    assert_eq!(result["coveredChars"], result["sourceChars"]);
    assert_eq!(result["completedWindows"], 1);
    assert_eq!(result["totalWindows"], 3);
    assert_eq!(result["state"], "partial");
}

#[test]
fn physical_recapture_keeps_discussion_identity_and_partial_character_union() {
    let source = input("body");
    let mut first_draft = draft(&source);
    first_draft.discussions.truncate(1);
    first_draft.discussions[0].evidence[0].end = source.fragments[0].start + 2;
    let mut second = source.clone();
    second.fragments[0].fragment_id = format!("{}.body.recapture", source.work.work_ref);
    second.fragments[0].source_ref = Uuid::from_u128(700);
    second.coverage["windowKey"] = json!("config-B-new-observation");
    let mut second_draft = first_draft.clone();
    second_draft.discussions[0].evidence[0].fragment_id = second.fragments[0].fragment_id.clone();
    assert_eq!(
        crate::topic_map_core::units(&source, &first_draft)[0].0,
        crate::topic_map_core::units(&second, &second_draft)[0].0
    );
    let mut coverage = work_summary::Coverage::default();
    for (input, draft) in [(source, first_draft), (second, second_draft)] {
        let unit = accepted(&input, &draft, 0, 100);
        coverage.add(&window(
            input,
            json!({}),
            json!({"units":[unit]}),
            None,
            "today",
        ));
    }
    let result = coverage.value();
    assert_eq!(result["coveredChars"], 2);
    assert_eq!(result["partialWindows"], 1);
    assert_eq!(result["state"], "partial");
}

#[test]
fn latest_source_revision_replaces_paraphrases_and_keeps_partial_truth() {
    let source = input("body");
    let old_draft = draft(&source);
    let old_units = json!([
        accepted(&source, &old_draft, 0, 100),
        accepted(&source, &old_draft, 1, 100)
    ]);
    let mut new_draft = old_draft.clone();
    new_draft.discussions[0].statement = "先把开始任务的动作缩小".into();
    new_draft.discussions[1].statement = "开始以后还需要维持注意".into();
    let new_units = json!([
        accepted(&source, &new_draft, 0, 100),
        accepted(&source, &new_draft, 1, 100)
    ]);
    for is_partial in [false, true] {
        let old = window(
            source.clone(),
            serde_json::to_value(&old_draft).unwrap(),
            json!({"units":old_units}),
            Some(50),
            "2026-10-09",
        );
        let (output, core, result) = if is_partial {
            let (output, core) =
                partial::payload(&new_draft, &source, &json!([new_units[0]])).unwrap();
            (output, core, None)
        } else {
            (
                serde_json::to_value(&new_draft).unwrap(),
                json!({"units":new_units}),
                Some(51),
            )
        };
        let mut new_input = source.clone();
        new_input.coverage["configRef"] = json!(Uuid::from_u128(900));
        new_input.coverage["windowKey"] = json!("different-config-same-source");
        let newer = window(new_input, output, core, result, "2026-10-10");
        let mut work = work(1, true);
        projection::project_work(
            &mut work,
            vec![old, newer],
            &mut [topic(100)],
            &HashSet::new(),
        );
        let research = work.research.unwrap();
        assert_eq!(
            research["core"]["units"].as_array().unwrap().len(),
            if is_partial { 1 } else { 2 }
        );
        assert_eq!(
            research["core"]["units"][0]["statement"],
            "先把开始任务的动作缩小"
        );
        if is_partial {
            assert_eq!(research["resultRefs"], json!([]));
            assert_eq!(research["output"]["angles"], json!([]));
            assert_eq!(research["core"]["coverage"]["completedWindows"], 0);
            assert_eq!(research["state"], "partial");
        } else {
            assert_eq!(research["resultRefs"], json!([Uuid::from_u128(51)]));
            assert_eq!(research["output"]["angles"].as_array().unwrap().len(), 1);
        }
    }
}
