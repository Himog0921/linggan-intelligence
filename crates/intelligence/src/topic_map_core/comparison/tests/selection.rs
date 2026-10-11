use super::*;

#[test]
fn shared_topic_counterexample_and_another_speaker_are_selected_before_extra_labels() {
    let (mut inputs, mut tasks, requested) = pair();
    let comment = source(1, 12, "unresearched_comment", "合成评论包含不同经历反例");
    inputs[0] = input(1, vec![inputs[0].fragments[0].clone(), comment.clone()]);
    let fragment = &inputs[0].fragments[0];
    tasks[0] = task(
        &inputs[0],
        1001,
        vec![
            unit("support", fragment, 0, 3, "author", "support", 100),
            unit(
                "counterexample",
                &comment,
                3,
                7,
                "commenter",
                "challenge",
                100,
            ),
            unit("other-label", &comment, 7, 9, "commenter", "challenge", 200),
        ],
    );
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    let rows = comparison.coverage["selectedDiscussions"]
        .as_array()
        .unwrap();
    assert_eq!(rows[0]["unitId"], "counterexample");
    assert!(rows.iter().any(|u| u["unitId"] == "support"));
    assert!(!rows.iter().any(|u| u["unitId"] == "other-label"));
    assert_eq!(comparison.coverage["availableDiscussionCount"], 4);
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 3);
}

#[test]
fn parent_context_is_bounded_and_never_counts_as_a_second_discussion_source() {
    let (mut inputs, mut tasks, requested) = pair();
    let parent = source(2, 23, "parent_comment_context", &"父语境".repeat(100));
    inputs[1].comment_study[0]["parentFragmentId"] = json!(parent.fragment_id);
    inputs[1].comment_study[0]["parentSourceRef"] = json!(parent.source_ref);
    inputs[1].fragments.push(parent.clone());
    inputs[1].hash = research::input_identity(&inputs[1]);
    let windows = research::research_windows(&inputs[1], 3000);
    let window = &windows[0];
    let child = window
        .fragments
        .iter()
        .find(|f| f.field == "unresearched_comment")
        .unwrap();
    let parent_window = window
        .fragments
        .iter()
        .find(|f| f.field == "parent_comment_context")
        .unwrap();
    tasks[1] = task(
        window,
        1002,
        vec![
            unit("b", child, 0, 6, "commenter", "challenge", 100),
            unit("b-support", child, 0, 6, "commenter", "support", 100),
            unit(
                "parent-alone",
                parent_window,
                0,
                5,
                "quoted",
                "context",
                100,
            ),
        ],
    );
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(comparison.coverage["availableDiscussionCount"], 3);
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 3);
    assert_eq!(comparison.coverage["evidenceChars"], 11);
    assert_eq!(comparison.coverage["contextChars"], 160);
    assert_eq!(comparison.coverage["inputChars"], 171);
    assert_eq!(
        comparison.comment_study[0]["parentFragmentIds"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        comparison
            .fragments
            .iter()
            .filter(|f| f.field == "unresearched_comment")
            .count(),
        1
    );
    assert!(
        !comparison.coverage["selectedDiscussions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["unitId"] == "parent-alone")
    );
    // Removing only required parent material invalidates the accepted child window.
    inputs[1]
        .fragments
        .retain(|f| f.field != "parent_comment_context");
    assert!(build_comparison(&inputs, &tasks, &requested, &topics()).is_err());
}

#[test]
fn coverage_reports_selected_subset_and_whole_scope_metadata_under_source_budget() {
    let mut inputs = Vec::new();
    let mut tasks = Vec::new();
    for work in 1..=10 {
        let input = input(
            work,
            vec![source(work, work + 20, "body", &"字".repeat(900))],
        );
        tasks.push(task(
            &input,
            work + 1000,
            vec![unit(
                &format!("unit-{work}"),
                &input.fragments[0],
                0,
                900,
                "author",
                "support",
                100,
            )],
        ));
        inputs.push(input);
    }
    let requested: Vec<_> = inputs.iter().map(|i| i.work.work_ref).collect();
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(
        comparison.coverage["scopeWorkRefs"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert_eq!(
        comparison.coverage["availableWorkRefs"]
            .as_array()
            .unwrap()
            .len(),
        10
    );
    assert_eq!(
        comparison.coverage["selectedWorkRefs"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(comparison.coverage["sourceChars"], 9000);
    assert_eq!(comparison.coverage["coveredChars"], 2700);
    assert_eq!(comparison.coverage["inputChars"], 2700);
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 3);
    assert_eq!(
        comparison.role_metadata["works"].as_array().unwrap().len(),
        10
    );
    assert_eq!(comparison.context_work_refs.len(), 9);
    let manifest = research::reference_manifest(&comparison);
    assert!(research::restore_scoped_window(&inputs, requested[0], &manifest).is_some());
    assert!(research::restore_scoped_window(&inputs[..9], requested[0], &manifest).is_none());
    assert_ne!(
        comparison.hash,
        build_comparison(&inputs, &tasks, &requested[..3], &topics())
            .unwrap()
            .hash
    );
}

#[test]
fn duplicate_units_and_overlapping_citations_do_not_inflate_input_or_discussion_counts() {
    let (inputs, mut tasks, requested) = pair();
    let fragment = &inputs[0].fragments[0];
    tasks[0].resolutions[0]["evidence"] = json!([
        {"fragmentId":fragment.fragment_id,"start":0,"end":5},
        {"fragmentId":fragment.fragment_id,"start":3,"end":7}
    ]);
    tasks.push(tasks[0].clone());
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(comparison.coverage["availableDiscussionCount"], 2);
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 2);
    assert_eq!(comparison.coverage["coveredChars"], 13);
    assert_eq!(comparison.fragments.len(), 2);
    let unit = &comparison.coverage["selectedDiscussions"][0];
    assert_eq!(
        unit["evidence"][0]["fragmentId"],
        unit["evidence"][1]["fragmentId"]
    );
    assert_eq!(unit["evidence"][1]["start"], 3);
    assert_eq!(unit["evidence"][1]["end"], 7);
}

#[test]
fn single_work_comparison_keeps_author_and_commenter_with_real_source_roles() {
    let author = source(1, 11, "body", "作者的具体经验观点");
    let comment = source(1, 12, "unresearched_comment", "评论原声提出了不同经历");
    let input = input(1, vec![author.clone(), comment.clone()]);
    let tasks = vec![task(
        &input,
        1001,
        vec![
            unit("author", &author, 0, 6, "author", "support", 100),
            unit(
                "comment-support",
                &comment,
                0,
                4,
                "commenter",
                "support",
                100,
            ),
            unit(
                "comment-challenge",
                &comment,
                4,
                8,
                "commenter",
                "challenge",
                100,
            ),
        ],
    )];
    let requested = vec![input.work.work_ref];
    let inputs = vec![input];
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 2);
    assert_eq!(
        comparison.coverage["selectedDiscussions"][0]["speakerRole"],
        "author"
    );
    assert_eq!(
        comparison.coverage["selectedDiscussions"][1]["unitId"],
        "comment-challenge"
    );
    assert_eq!(comparison.context_work_refs, Vec::<Uuid>::new());
    assert_eq!(
        comparison.role_metadata["works"].as_array().unwrap().len(),
        1
    );
    assert_eq!(comparison.work.fragments.len(), 1);
    assert_eq!(comparison.comment_study.as_array().unwrap().len(), 1);
    assert!(
        research::restore_scoped_window(
            &inputs,
            requested[0],
            &research::reference_manifest(&comparison)
        )
        .is_some()
    );
    assert_ne!(comparison.hash, inputs[0].hash);
}

#[test]
fn single_work_without_both_current_source_roles_cannot_generate_a_response_comparison() {
    let body = source(1, 11, "body", "只有作者自己的观点");
    let author_input = input(1, vec![body.clone()]);
    let requested = vec![author_input.work.work_ref];
    let tasks = vec![task(
        &author_input,
        1001,
        vec![
            unit("author", &body, 0, 4, "author", "support", 100),
            // A claimed commenter role cannot turn author text into comment evidence.
            unit("wrong-role", &body, 4, 8, "commenter", "challenge", 100),
        ],
    )];
    let comparison = build_comparison(&[author_input], &tasks, &requested, &topics());
    assert_eq!(
        comparison.unwrap_err(),
        "comparison_requires_author_and_commenter_discussions"
    );
    let comment = source(2, 22, "unresearched_comment", "只有合格评论原声");
    let input = input(2, vec![comment.clone()]);
    let tasks = vec![task(
        &input,
        1002,
        vec![unit(
            "unresearched_comment",
            &comment,
            0,
            4,
            "commenter",
            "challenge",
            100,
        )],
    )];
    assert_eq!(
        build_comparison(&[input.clone()], &tasks, &[input.work.work_ref], &topics()).unwrap_err(),
        "comparison_requires_author_and_commenter_discussions"
    );
}
