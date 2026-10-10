//! A comparison keeps one result identity while every actual participant can read it.
use super::*;

fn secondary_source() -> ResearchInput {
    let mut source = input("studied_comment");
    source.work.work_ref = Uuid::from_u128(2);
    source.fragments[0].fragment_id = format!("{}.studied_comment.synthetic", source.work.work_ref);
    source.fragments[0].source_ref = Uuid::from_u128(6);
    source.role_metadata["workRef"] = json!(source.work.work_ref);
    source
}

fn comparison() -> (Window, Vec<ResearchInput>) {
    let sources = vec![input("body"), secondary_source()];
    let selected = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
    let mut source =
        topic_map_research::with_comparison_context(sources[0].clone(), &sources, &selected);
    source.coverage["scopeWorkRefs"] =
        json!([Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)]);
    source.coverage["selectedWorkRefs"] = json!(selected);
    source.coverage["state"] = json!("partial");
    source.coverage["boundary"] = json!("只比较两篇作品的所选讨论，第三篇未入选。 ");
    source.coverage["fragmentWorkRefs"] = json!({
        &source.fragments[0].fragment_id: selected[0],
        &source.fragments[1].fragment_id: selected[1],
    });
    source.coverage["selectedDiscussions"] = json!(
        sources
            .iter()
            .map(|input| json!({
                "workRef":input.work.work_ref,"taskRef":Uuid::from_u128(60),
                "assignments":[],"comparedDefinitionRefs":[],
            }))
            .collect::<Vec<_>>()
    );
    source.hash = topic_map_research::input_identity(&source);
    let mut output = draft(&source);
    output.discussions[1] = draft(&sources[1]).discussions[1].clone();
    let evidence: Vec<_> = source
        .fragments
        .iter()
        .map(|fragment| Citation {
            fragment_id: fragment.fragment_id.clone(),
            start: fragment.start,
            end: fragment.end,
        })
        .collect();
    output.angles[0].evidence = evidence.clone();
    let mut second_angle = output.angles[0].clone();
    second_angle.label = "持续练习".into();
    second_angle.title = "练习以后如何维持注意".into();
    second_angle.answer_task = "说明如何在开始以后继续练习".into();
    output.angles.push(second_angle);
    output.response_matches = serde_json::from_value(json!([{
        "status":"partial","unanswered":["持续练习仍未知"],"evidence":evidence,
    }]))
    .unwrap();
    output.product_opportunities = serde_json::from_value(json!([{
        "need":"维持练习","hypothesis":"较短练习可能更适合", "verificationQuestion":"是否更容易持续？",
        "alternativeExplanation":"也可能取决于任务种类","evidence":evidence,
    },{
        "need":"开始练习","hypothesis":"明确第一步可能有帮助", "verificationQuestion":"是否更容易开始？",
        "alternativeExplanation":"也可能只需要休息","evidence":evidence,
    }])).unwrap();
    crate::topic_map_research_analysis::validate_output(&output, &source.fragments, &[]).unwrap();
    let core = json!({"units":[accepted(&source, &output, 0, 100)]});
    let manifest = topic_map_research::reference_manifest(&source);
    let mut comparison = window(source, json!(output), core, Some(50), "2026-10-10");
    comparison.manifest = manifest;
    (comparison, sources)
}

#[test]
fn comparison_targets_only_actual_participants_even_when_primary_is_filtered_out() {
    let (comparison, sources) = comparison();
    let (a, b, requested_only) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    for wrapper in [
        comparison.manifest.clone(),
        json!({"source":comparison.manifest}),
    ] {
        assert_eq!(
            load::projection_targets(a, &wrapper, &HashSet::from([a, b, requested_only])),
            vec![a, b],
        );
        assert_eq!(
            load::projection_targets(a, &wrapper, &HashSet::from([b, requested_only])),
            vec![b],
        );
        assert!(load::projection_targets(a, &wrapper, &HashSet::from([requested_only])).is_empty());
        // Display filters never change the primary used to qualify the complete result.
        let restored = topic_map_research::restore_scoped_window(&sources, a, &wrapper).unwrap();
        assert_eq!(restored.work.work_ref, a);
        assert_eq!(restored.fragments.len(), 2);
        assert!(topic_map_research::restore_scoped_window(&sources[1..], a, &wrapper).is_none());
        assert!(topic_map_research::restore_scoped_window(&sources[..1], a, &wrapper).is_none());
    }
}

#[test]
fn all_participants_share_frozen_findings_without_single_work_claims() {
    let (comparison, _) = comparison();
    let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let expected_scope = json!({"scopeWorkRefs":[a,b,Uuid::from_u128(3)],
        "selectedWorkRefs":[a,b],"state":"partial","boundary":comparison.manifest["coverage"]["boundary"],
        "resultRef":Uuid::from_u128(50)});
    let mut topics = vec![topic(100)];
    let mut results = Vec::new();
    for target in load::projection_targets(a, &comparison.manifest, &HashSet::from([a, b])) {
        let mut work = work(target.as_u128(), false);
        assert!(
            projection::project_work(
                &mut work,
                vec![comparison.clone()],
                &mut topics,
                &HashSet::new(),
            )
            .is_empty()
        );
        let research = work.research.as_ref().unwrap();
        assert_eq!(research["resultRef"], json!(Uuid::from_u128(50)));
        assert_eq!(research["core"]["units"], json!([]));
        for field in [
            "sourceChars",
            "coveredChars",
            "totalWindows",
            "completedWindows",
            "partialWindows",
        ] {
            assert_eq!(research["core"]["coverage"][field], 0);
        }
        assert_eq!(work.main_stage, "pending");
        assert_eq!(work.path, "unknown");
        assert_eq!(research["output"]["journey"]["mainStage"], "unclear");
        assert_eq!(
            research["authorSourceState"],
            if target == a {
                "available"
            } else {
                "source_unavailable"
            }
        );
        assert_eq!(
            research["commentSourceState"],
            if target == b {
                "available"
            } else {
                "source_unavailable"
            }
        );
        assert_eq!(
            research["output"]["responseMatches"][0]["status"],
            "partial"
        );
        for key in [
            "discussions",
            "scenes",
            "responseMatches",
            "angles",
            "productOpportunities",
        ] {
            for item in research["output"][key].as_array().unwrap() {
                assert_eq!(item["comparisonScope"], expected_scope);
            }
        }
        for (key, index_field) in [
            ("angles", "researchAngleIndex"),
            ("productOpportunities", "researchOpportunityIndex"),
        ] {
            for (index, item) in research["output"][key]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                assert_eq!(item["researchResultRef"], json!(Uuid::from_u128(50)));
                assert_eq!(item["researchMethodVersion"], "topic-map.research.v2");
                assert_eq!(item[index_field], json!(index));
                assert_eq!(item["evidenceWorkRefs"], json!([a, b]));
            }
        }
        for (fragment, owner) in research["fragments"].as_array().unwrap().iter().zip([a, b]) {
            assert_eq!(fragment["workRef"], json!(owner));
        }
        assert_eq!(
            research["output"]["limitations"],
            comparison.output["limitations"]
        );
        results.push(research["output"].clone());
    }
    assert_eq!(results.len(), 2);
    assert_eq!(results[0], results[1]);
    assert!(topics[0].direct_work_refs.is_empty());
}

#[test]
fn mixed_versions_keep_each_result_method_index_and_evidence_scope() {
    let source = input("body");
    let mut output = draft(&source);
    output.contract = "topic-map.research.v1".into();
    let core = legacy::legacy_units(Uuid::from_u128(40), &output, &json!({}));
    let mut single = window(source, json!(output), core, Some(40), "2026-10-09");
    single.method = "topic-map.research.v1".into();
    let mut baseline = work(1, false);
    let baseline_candidates = projection::project_work(
        &mut baseline,
        vec![single.clone()],
        &mut [topic(100)],
        &HashSet::new(),
    );
    let (comparison, _) = comparison();
    let mut mixed = work(1, false);
    let candidates = projection::project_work(
        &mut mixed,
        vec![single, comparison],
        &mut [topic(100)],
        &HashSet::new(),
    );
    assert_eq!(candidates, baseline_candidates);
    assert_eq!(mixed.main_stage, baseline.main_stage);
    assert_eq!(mixed.path, baseline.path);
    let research = mixed.research.as_ref().unwrap();
    assert_eq!(
        research["core"],
        baseline.research.as_ref().unwrap()["core"]
    );
    let angles = research["output"]["angles"].as_array().unwrap();
    assert_eq!(angles.len(), 3);
    assert_eq!(angles[0]["researchMethodVersion"], "topic-map.research.v2");
    assert_eq!(angles[1]["researchAngleIndex"], 1);
    assert_eq!(angles[2]["researchResultRef"], json!(Uuid::from_u128(40)));
    assert_eq!(angles[2]["researchMethodVersion"], "topic-map.research.v1");
    assert_eq!(angles[2]["researchAngleIndex"], 0);
    assert_eq!(angles[2]["evidenceWorkRefs"], json!([Uuid::from_u128(1)]));
    assert!(angles[2].get("comparisonScope").is_none());
    let single_fragment = &baseline.research.as_ref().unwrap()["fragments"][0];
    assert_eq!(single_fragment["workRef"], json!(Uuid::from_u128(1)));
}

#[test]
fn requested_or_malformed_scope_cannot_create_additional_participants() {
    let (comparison, _) = comparison();
    let (a, b, c) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let visible = HashSet::from([a, b, c]);
    for selected in [
        json!([a, c]),
        json!([a, a, b]),
        json!([b]),
        json!([a, "unknown"]),
    ] {
        let mut manifest = comparison.manifest.clone();
        manifest["coverage"]["selectedWorkRefs"] = selected;
        assert!(load::projection_targets(a, &manifest, &visible).is_empty());
    }
    let mut outside_scope = comparison.manifest.clone();
    outside_scope["coverage"]["scopeWorkRefs"] = json!([a, c]);
    assert!(load::projection_targets(a, &outside_scope, &visible).is_empty());
    let mut single = comparison.manifest;
    single["coverage"]["kind"] = json!("source");
    assert_eq!(load::projection_targets(a, &single, &visible), vec![a]);
    assert!(load::projection_targets(a, &single, &HashSet::from([b, c])).is_empty());
}

#[test]
fn missing_comparison_owner_is_unknown_in_both_readers() {
    let (comparison, _) = comparison();
    let id = comparison.input.fragments[0].fragment_id.clone();
    for owner in [None, Some(Value::Null), Some(json!("not-a-work-ref"))] {
        let mut comparison = comparison.clone();
        for coverage in [
            &mut comparison.input.coverage,
            &mut comparison.manifest["coverage"],
        ] {
            if let Some(owner) = &owner {
                coverage["fragmentWorkRefs"][&id] = owner.clone();
            } else {
                coverage["fragmentWorkRefs"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&id);
            }
        }
        for target in [1, 2] {
            let mut work = work(target, false);
            projection::project_work(
                &mut work,
                vec![comparison.clone()],
                &mut [],
                &HashSet::new(),
            );
            let research = work.research.as_ref().unwrap();
            assert_eq!(research["fragments"][0].get("workRef"), Some(&Value::Null));
            assert_eq!(research["authorSourceState"], "source_unavailable");
        }
    }
}
