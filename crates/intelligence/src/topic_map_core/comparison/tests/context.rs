use super::*;
use crate::topic_map_research_analysis::{ResearchOutput, validate_output};

fn comment_window(source: &ResearchInput) -> ResearchInput {
    research::research_windows(source, 3000)
        .into_iter()
        .find(|window| window.work.fragments.is_empty())
        .unwrap()
}

fn contextual_unit(window: &ResearchInput, id: &str) -> Value {
    let comment = window
        .fragments
        .iter()
        .find(|f| f.field == "unresearched_comment")
        .unwrap();
    let context = window
        .fragments
        .iter()
        .find(|f| f.field == "work_context:body")
        .unwrap();
    let mut discussion = unit(id, comment, 0, 4, "commenter", "challenge", 100);
    discussion["evidence"].as_array_mut().unwrap().push(json!({
        "fragmentId":context.fragment_id,"start":2,"end":6
    }));
    discussion
}

fn response_output(comparison: &ResearchInput, status: &str) -> ResearchOutput {
    serde_json::from_value(json!({
        "contract":crate::topic_map_research_analysis::EXTRACT_CONTRACT,"outcome":"analyzed",
        "discussions":[],"scenes":[],"angles":[],"productOpportunities":[],"limitations":[],
        "journey":{"mainStage":"unclear","involvedStages":[],"overlays":[],"path":"unknown",
            "rationale":"没有实际作者回应的双侧依据","evidence":[]},
        "responseMatches":[{"status":status,"unanswered":[],"evidence":comparison.fragments.iter().map(|f|
            json!({"fragmentId":f.fragment_id,"start":f.start,"end":f.end})).collect::<Vec<_>>()}]
    })).unwrap()
}

#[test]
fn comment_only_comparison_keeps_work_context_without_manufacturing_author_evidence() {
    let inputs: Vec<_> = (1..=2)
        .map(|work| {
            input(
                work,
                vec![
                    source(work, work * 10 + 1, "body", "A中😀e\u{301}B作者语境"),
                    source(
                        work,
                        work * 10 + 2,
                        "unresearched_comment",
                        "评论表达自己的困惑",
                    ),
                ],
            )
        })
        .collect();
    let tasks: Vec<_> = inputs
        .iter()
        .map(|source| {
            let window = comment_window(source);
            let context = window
                .fragments
                .iter()
                .find(|f| f.field == "work_context:body")
                .unwrap();
            task(
                &window,
                source.work.work_ref.as_u128() + 1000,
                vec![
                    contextual_unit(&window, "comment"),
                    // Neither a claimed author nor an unknown context-only discussion is independent evidence.
                    unit("fake-author", context, 2, 6, "author", "support", 100),
                    unit("context-alone", context, 2, 6, "unknown", "context", 100),
                ],
            )
        })
        .collect();
    let requested: Vec<_> = inputs.iter().map(|input| input.work.work_ref).collect();
    let comparison = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert_eq!(comparison.coverage["availableDiscussionCount"], 2);
    assert_eq!(comparison.coverage["selectedDiscussionCount"], 2);
    assert_eq!(comparison.coverage["evidenceChars"], 8);
    assert_eq!(comparison.coverage["coveredChars"], 8);
    assert_eq!(comparison.coverage["contextChars"], 8);
    assert_eq!(comparison.coverage["inputChars"], 16);
    assert!(comparison.work.fragments.is_empty());
    assert_eq!(comparison.fragments.len(), 4);
    for work in comparison.coverage["perWork"].as_array().unwrap() {
        assert_eq!(work["selectedChars"], 4);
        assert_eq!(work["parentContextChars"], 0);
    }
    for context in comparison
        .fragments
        .iter()
        .filter(|f| f.field == "work_context:body")
    {
        assert!(context.fragment_id.ends_with(".context.chars.2.6"));
        assert_eq!(context.text, "😀e\u{301}B");
        assert_eq!((context.start, context.end), (2, 6));
    }
    let manifest = research::reference_manifest(&comparison);
    let restored = research::restore_scoped_window(&inputs, requested[0], &manifest).unwrap();
    assert_eq!(restored.hash, comparison.hash);
    assert!(restored.work.fragments.is_empty());
    assert_eq!(
        serde_json::to_value(&restored.fragments).unwrap(),
        serde_json::to_value(&comparison.fragments).unwrap()
    );
    assert!(
        manifest["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["field"] == "work_context:body")
            .all(|f| f["contextOnly"] == true)
    );
    assert!(
        validate_output(
            &response_output(&comparison, "unknown"),
            &comparison.fragments,
            &[]
        )
        .is_ok()
    );
    for status in ["direct", "partial", "unmatched"] {
        assert_eq!(
            validate_output(
                &response_output(&comparison, status),
                &comparison.fragments,
                &[]
            ),
            Err("invalid_response_evidence")
        );
    }
    let mut changed = inputs.clone();
    changed[0].fragments[0].text = "A中错误的新作品上下文".into();
    changed[0].fragments[0].end = changed[0].fragments[0].text.chars().count();
    assert!(research::restore_scoped_window(&changed, requested[0], &manifest).is_none());
    let mut withdrawn = inputs.clone();
    withdrawn[1].fragments.retain(|f| f.field != "body");
    withdrawn[1].work.fragments.clear();
    assert!(research::restore_scoped_window(&withdrawn, requested[0], &manifest).is_none());
    assert!(research::restore_scoped_window(&inputs[..1], requested[0], &manifest).is_none());
    let mut wrong_owner = manifest.clone();
    let context = wrong_owner["fragments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["field"] == "work_context:body")
        .unwrap();
    context["workRef"] = json!(Uuid::from_u128(999));
    assert!(research::restore_scoped_window(&inputs, requested[0], &wrong_owner).is_none());
    let mut disguised_comment = manifest.clone();
    let comment = disguised_comment["fragments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["field"] == "unresearched_comment")
        .unwrap();
    comment["field"] = json!("work_context:unresearched_comment");
    comment["contextOnly"] = json!(true);
    comment["fragmentId"] = json!(format!(
        "{}.context.chars.0.4",
        comment["sourceFragmentId"].as_str().unwrap()
    ));
    assert!(research::restore_scoped_window(&inputs, requested[0], &disguised_comment).is_none());
    // A single work still needs a real author discussion; its copied context cannot stand in.
    assert_eq!(
        build_comparison(&inputs[..1], &tasks[..1], &requested[..1], &topics()).unwrap_err(),
        "comparison_requires_author_and_commenter_discussions"
    );
}

#[test]
fn overlapping_author_evidence_and_work_context_retain_distinct_roles_ids_and_citations() {
    let source = input(
        1,
        vec![
            source(1, 11, "body", "A中😀e\u{301}B作者语境"),
            source(1, 12, "unresearched_comment", "评论表达自己的困惑"),
        ],
    );
    let author_window = research::research_windows(&source, 3000)
        .into_iter()
        .find(|window| !window.work.fragments.is_empty())
        .unwrap();
    let comment_window = comment_window(&source);
    let author = &author_window.work.fragments[0];
    let tasks = vec![
        task(
            &author_window,
            1001,
            vec![unit("author", author, 2, 6, "author", "support", 100)],
        ),
        task(
            &comment_window,
            1002,
            vec![contextual_unit(&comment_window, "comment")],
        ),
    ];
    let comparison = build_comparison(
        &[source.clone()],
        &tasks,
        &[source.work.work_ref],
        &topics(),
    )
    .unwrap();
    assert_eq!(comparison.fragments.len(), 3);
    assert_eq!(comparison.work.fragments.len(), 1);
    assert_eq!(comparison.coverage["evidenceChars"], 8);
    assert_eq!(comparison.coverage["contextChars"], 4);
    assert_eq!(comparison.coverage["inputChars"], 12);
    let primary = comparison
        .fragments
        .iter()
        .find(|f| f.field == "body")
        .unwrap();
    let context = comparison
        .fragments
        .iter()
        .find(|f| f.field == "work_context:body")
        .unwrap();
    assert_eq!(primary.text, context.text);
    assert_eq!((primary.start, primary.end), (context.start, context.end));
    assert_ne!(primary.fragment_id, context.fragment_id);
    let rows = comparison.coverage["selectedDiscussions"]
        .as_array()
        .unwrap();
    let author = rows.iter().find(|u| u["unitId"] == "author").unwrap();
    let comment = rows.iter().find(|u| u["unitId"] == "comment").unwrap();
    assert_eq!(author["evidence"][0]["fragmentId"], primary.fragment_id);
    assert_eq!(comment["evidence"][1]["fragmentId"], context.fragment_id);
    assert!(
        validate_output(
            &response_output(&comparison, "direct"),
            &comparison.fragments,
            &[]
        )
        .is_ok()
    );
    assert!(
        research::restore_scoped_window(
            &[source.clone()],
            source.work.work_ref,
            &research::reference_manifest(&comparison)
        )
        .is_some()
    );
}
