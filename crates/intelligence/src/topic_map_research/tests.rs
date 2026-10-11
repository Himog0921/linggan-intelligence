//! Input qualification, stable identity and lossless window regression proofs.
use super::queue::{overlaps_unknown_dispatch, should_queue_input};
use super::windows::{is_research_evidence, update_source_coverage};
use super::*;

fn source(field: &str, text: &str, id: u128) -> Fragment {
    let work = Uuid::from_u128(1);
    let source = Uuid::from_u128(id);
    Fragment {
        fragment_id: if matches!(field, "title" | "body") {
            format!("{work}.{field}")
        } else {
            format!("{work}.{field}.{id}")
        },
        source_ref: source,
        field: field.into(),
        source_version: creator_discovery::hash(text),
        start: 0,
        end: text.chars().count(),
        text: text.into(),
    }
}

fn input(fragments: Vec<Fragment>) -> ResearchInput {
    let work_ref = Uuid::from_u128(1);
    let mut input = ResearchInput {
        work: DiscoveryWork {
            work_ref,
            usage_roles: vec!["primary".into()],
            creator_key: None,
            platform: "xhs".into(),
            author_external_id: Some("synthetic-author".into()),
            display_name: None,
            title: None,
            published_date: None,
            published_epoch: None,
            likes: None,
            likes_observed_at: None,
            likes_observed_display: None,
            comments: None,
            collects: None,
            first_added: String::new(),
            first_added_display: String::new(),
            fingerprint: String::new(),
            search_text: String::new(),
            fragments: fragments
                .iter()
                .filter(|f| matches!(f.field.as_str(), "title" | "body" | "ocr" | "transcript"))
                .cloned()
                .collect(),
            relevance: "unknown".into(),
            traits: Vec::new(),
            topic_hints: Vec::new(),
            analysis: Value::Null,
            analysis_state: "not_analyzed".into(),
            observation: Value::Null,
            profile: Value::Null,
            focus: Value::Null,
            author_analysis: Value::Null,
            acquisition_kind: "synthetic".into(),
        },
        fragments,
        topics: json!([]),
        domain: json!({"domainRef":Uuid::from_u128(3),"name":"合成领域","description":"仅测试","researchGoal":"明确归属边界"}),
        hash: String::new(),
        context_work_refs: Vec::new(),
        comment_study: json!([]),
        role_metadata: json!({"workRef":work_ref,"authorExternalId":"synthetic-author","own":false,"ownScopeConfigured":false}),
        coverage: json!({"inputContract":INPUT_CONTRACT,"configRef":Uuid::from_u128(4)}),
    };
    update_source_coverage(&mut input);
    input.hash = input_identity(&input);
    input
}

fn comment_metadata(comment: &Fragment, parent: Option<&Fragment>, signals: Value) -> Value {
    json!({"workRef":Uuid::from_u128(1),"sourceRef":comment.source_ref,"fragmentId":comment.fragment_id,
        "sourceFragmentId":comment.fragment_id,"role":"eligible_user_comment","contextOnly":false,
        "researchState":if signals.as_array().is_some_and(|s|!s.is_empty()){ "accepted" }else{ "unresearched" },
        "observationRole":"primary","parentSourceRef":parent.map(|p|p.source_ref),
        "parentFragmentId":parent.map(|p|p.fragment_id.clone()),"parentContextOnly":true,"signals":signals})
}

#[test]
fn source_windows_cover_long_unicode_text_without_prefix_loss() {
    let text = format!(
        "{}最后才出现的限制：这一办法对我没有效果。",
        "开始一个任务。👩‍👧\n保留完整段落。\n".repeat(430)
    );
    let body = source("body", &text, 10);
    let input = input(vec![body.clone()]);
    let windows = research_windows(&input, 3000);
    assert!(windows.len() > 2);
    assert_eq!(
        windows
            .iter()
            .map(|w| w.coverage["windowChars"].as_u64().unwrap())
            .sum::<u64>(),
        text.chars().count() as u64
    );
    let rebuilt: String = windows
        .iter()
        .flat_map(|w| w.fragments.iter())
        .map(|f| f.text.as_str())
        .collect();
    assert_eq!(rebuilt, text);
    for (index, window) in windows.iter().enumerate() {
        assert!(
            window
                .fragments
                .iter()
                .all(|f| f.text.chars().count() <= 1200)
        );
        assert!(
            window
                .fragments
                .iter()
                .map(|f| f.text.chars().count())
                .sum::<usize>()
                <= 3000
        );
        assert_eq!(window.coverage["windowIndex"], json!(index));
        assert_eq!(window.coverage["windowCount"], json!(windows.len()));
        for f in &window.fragments {
            assert_eq!(
                f.text,
                body.text
                    .chars()
                    .skip(f.start)
                    .take(f.end - f.start)
                    .collect::<String>()
            );
        }
        let restored = restore_window(&input, &reference_manifest(window)).unwrap();
        assert_eq!(restored.hash, window.hash);
    }
}

#[test]
fn unrelated_topic_and_new_source_do_not_change_a_frozen_window() {
    let body = source("body", "同一场景中先弄清楚下一步做什么。", 10);
    let original = input(vec![body]);
    let window = research_windows(&original, 3000).remove(0);
    let manifest = reference_manifest(&window);
    let mut expanded = original.clone();
    expanded.topics = json!([{"topicRef":Uuid::from_u128(80),"definition":"完全不同的新主题"}]);
    expanded
        .fragments
        .push(source("unresearched_comment", "后补采的一条独立评论。", 20));
    update_source_coverage(&mut expanded);
    assert_eq!(research_windows(&expanded, 3000)[0].hash, window.hash);
    assert!(sources_current(&expanded, &manifest));
    assert_eq!(
        restore_window(&expanded, &manifest)
            .unwrap()
            .fragments
            .len(),
        window.fragments.len()
    );
}

#[test]
fn physical_refresh_preserves_semantics_and_audit_but_text_owner_and_range_changes_do_not() {
    let original = input(vec![source("body", "你好🙂这里有明确的文本证据。", 10)]);
    let window = research_windows(&original, 7).remove(0);
    let manifest = reference_manifest(&window);
    let mut changed = original.clone();
    changed.fragments[0].source_ref = Uuid::from_u128(900);
    changed.fragments[0].source_version = "a-different-source-version".into();
    changed.work.likes = Some(900);
    changed.role_metadata["likes"] = json!(900);
    let restored = restore_window(&changed, &manifest).unwrap();
    assert_eq!(research_windows(&changed, 7)[0].hash, window.hash);
    assert_eq!(
        restored.fragments[0].source_ref,
        original.fragments[0].source_ref
    );
    assert_eq!(
        restored.coverage["currentSources"][&restored.fragments[0].fragment_id]["sourceRef"],
        json!(Uuid::from_u128(900))
    );
    changed.fragments[0]
        .text
        .push_str("追加与已选窗口无关的尾段");
    changed.fragments[0].end = changed.fragments[0].text.chars().count();
    assert!(sources_current(&changed, &manifest));
    changed.fragments[0].text = changed.fragments[0].text.replacen("你好", "再见", 1);
    assert!(!sources_current(&changed, &manifest));
    changed = original.clone();
    changed.fragments.clear();
    assert!(restore_window(&changed, &manifest).is_none());
    changed = original.clone();
    changed.role_metadata["own"] = json!(true);
    assert!(restore_window(&changed, &manifest).is_none());
    changed = original.clone();
    changed.work.author_external_id = Some("different-author".into());
    assert!(!sources_current(&changed, &manifest));
    let mut corrupt = manifest.clone();
    corrupt["fragments"][0]["end"] = json!(999);
    assert!(restore_window(&original, &corrupt).is_none());
    corrupt = manifest.clone();
    corrupt["fragments"][0]["textHash"] = json!("0".repeat(64));
    assert!(restore_window(&original, &corrupt).is_none());
}

#[test]
fn long_parent_is_context_only_and_does_not_duplicate_comment_coverage() {
    let child = source(
        "unresearched_comment",
        "我已经这样试过，还是不知道先做哪一步。",
        20,
    );
    let parent_text = "请具体说说你从哪一个任务开始尝试。\n".repeat(350);
    let parent = source(PARENT_CONTEXT_FIELD, &parent_text, 21);
    let mut original = input(vec![child.clone(), parent.clone()]);
    original.comment_study = json!([comment_metadata(&child, Some(&parent), json!([]))]);
    update_source_coverage(&mut original);
    let windows = research_windows(&original, 3000);
    assert!(windows.len() > 1);
    assert_eq!(
        windows
            .iter()
            .map(|w| w.coverage["windowChars"].as_u64().unwrap())
            .sum::<u64>(),
        child.text.chars().count() as u64
    );
    let reconstructed_parent: String = windows
        .iter()
        .flat_map(|w| w.fragments.iter())
        .filter(|f| !is_research_evidence(f))
        .map(|f| f.text.as_str())
        .collect();
    assert_eq!(reconstructed_parent, parent_text);
    for window in &windows {
        assert_eq!(
            window.coverage["sourceChars"],
            json!(child.text.chars().count())
        );
        assert!(window.work.fragments.is_empty());
        assert_eq!(window.role_metadata["commentOnly"], json!(true));
        assert!(
            !window.comment_study[0]["parentFragmentIds"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            window
                .fragments
                .iter()
                .map(|f| f.text.chars().count())
                .sum::<usize>()
                <= 3000
        );
        assert!(sources_current(&original, &reference_manifest(window)));
    }
    original.fragments.retain(is_research_evidence);
    assert!(restore_window(&original, &reference_manifest(&windows[0])).is_none());
}

#[test]
fn signal_dependency_is_limited_to_comments_in_this_group() {
    let comments: Vec<_> = (20..27)
        .map(|id| source("studied_comment", "独立评论中的具体行动障碍。", id))
        .collect();
    let mut original = input(comments.clone());
    original.comment_study = json!(comments.iter().enumerate().map(|(index,c)|
        comment_metadata(c,None,json!([{ "signalRef":Uuid::from_u128(40+index as u128),"kind":"need","proposition":"需要明确第一步"}]))
    ).collect::<Vec<_>>());
    let window = research_windows(&original, 3000).remove(0);
    assert_eq!(window.comment_study.as_array().unwrap().len(), 6);
    let manifest = reference_manifest(&window);
    let mut changed = original.clone();
    changed.comment_study[6]["signals"][0]["proposition"] = json!("未选中评论自己的研究变化");
    assert!(sources_current(&changed, &manifest));
    changed.comment_study[0]["signals"][0]["proposition"] = json!("实际依赖发生变化");
    assert!(!sources_current(&changed, &manifest));
}

#[test]
fn comment_reobservation_retains_effective_signal_and_frozen_provenance() {
    let comment = source("studied_comment", "我试过这一步还是不知怎样继续。", 20);
    let parent = source(PARENT_CONTEXT_FIELD, "先从一个很小的任务开始试。", 21);
    let mut original = input(vec![comment.clone(), parent.clone()]);
    original.comment_study = json!([comment_metadata(
        &comment,
        Some(&parent),
        json!([
        {"signalRef":Uuid::from_u128(50),"kind":"need","proposition":"下一步行动不明确"}])
    )]);
    let window = research_windows(&original, 3000).remove(0);
    let mut current = original.clone();
    current.fragments[0].source_ref = Uuid::from_u128(500);
    current.fragments[1].source_ref = Uuid::from_u128(501);
    current.comment_study[0]["sourceRef"] = json!(Uuid::from_u128(500));
    current.comment_study[0]["parentSourceRef"] = json!(Uuid::from_u128(501));
    assert_eq!(research_windows(&current, 3000)[0].hash, window.hash);
    let restored = restore_window(&current, &reference_manifest(&window)).unwrap();
    assert_eq!(
        restored.comment_study[0]["sourceRef"],
        json!(comment.source_ref)
    );
    assert_eq!(restored.fragments[0].source_ref, comment.source_ref);
    current.comment_study[0]["signals"][0]["signalRef"] = json!(Uuid::from_u128(502));
    assert!(!sources_current(&current, &reference_manifest(&window)));
}

#[test]
fn explicit_retry_keeps_unknown_dispatch_and_successes_closed() {
    let states = |values: &[&str]| values.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    assert!(should_queue_input(&[], "incremental"));
    assert!(!should_queue_input(&states(&["failed"]), "incremental"));
    assert!(should_queue_input(&states(&["failed"]), "on_demand"));
    assert!(should_queue_input(&states(&["stopped"]), "on_demand"));
    assert!(!should_queue_input(
        &states(&["unknown_dispatch"]),
        "on_demand"
    ));
    assert!(!should_queue_input(
        &states(&["failed", "unknown_dispatch"]),
        "on_demand"
    ));
    assert!(!should_queue_input(
        &states(&["failed", "succeeded"]),
        "on_demand"
    ));
    assert!(!should_queue_input(&states(&["running"]), "on_demand"));
}

#[test]
fn unknown_dispatch_cannot_be_replayed_through_another_window_hash() {
    let original = input(vec![source(
        "body",
        "前十个字保持原样。后面是尚未发送的更多内容。",
        10,
    )]);
    let unknown = research_windows(&original, 10).remove(0);
    let refs = reference_manifest(&unknown)["fragments"]
        .as_array()
        .unwrap()
        .clone();
    let mut changed_config = original.clone();
    changed_config.fragments[0].source_ref = Uuid::from_u128(901);
    changed_config.fragments[0].source_version = "later physical observation".into();
    changed_config.coverage["configRef"] = json!(Uuid::from_u128(999));
    let new_windows = research_windows(&changed_config, 4);
    assert_ne!(new_windows[0].hash, unknown.hash);
    assert!(overlaps_unknown_dispatch(
        &new_windows[0],
        &changed_config,
        &refs
    ));
    assert!(!overlaps_unknown_dispatch(
        &new_windows[3],
        &changed_config,
        &refs
    ));
    let changed_text = input(vec![source(
        "body",
        "这里真的换了完整的新来源内容，不是同一份发送。",
        10,
    )]);
    assert!(!overlaps_unknown_dispatch(
        &research_windows(&changed_text, 4)[0],
        &changed_text,
        &refs
    ));
    let mut legacy = refs;
    legacy[0]["sourceFragmentId"] = Value::Null;
    legacy[0]["fragmentId"] = json!("old-prefix-format");
    legacy[0]["sourceVersion"] = json!("old-preview-version");
    assert!(overlaps_unknown_dispatch(
        &new_windows[0],
        &changed_config,
        &legacy
    ));
}

fn two_works() -> Vec<ResearchInput> {
    let first = input(vec![source("body", "作者说先确定下一步。", 10)]);
    let mut second = input(vec![source(
        "unresearched_comment",
        "读者说执行中还是会走神。",
        20,
    )]);
    second.work.work_ref = Uuid::from_u128(2);
    second.role_metadata["workRef"] = json!(second.work.work_ref);
    for fragment in &mut second.fragments {
        fragment.fragment_id = fragment.fragment_id.replacen(
            &Uuid::from_u128(1).to_string(),
            &second.work.work_ref.to_string(),
            1,
        );
    }
    second.work.fragments.clear();
    vec![first, second]
}

#[test]
fn comparison_restores_each_work_and_checks_other_works_roles_and_restrictions() {
    let all = two_works();
    let comparison = with_comparison_context(all[0].clone(), &all, &[all[1].work.work_ref]);
    let manifest = reference_manifest(&comparison);
    assert_eq!(manifest["topics"], json!([]));
    assert_eq!(
        manifest["fragments"][1]["workRef"],
        json!(all[1].work.work_ref)
    );
    let restored = restore_scoped_window(
        &all,
        all[0].work.work_ref,
        &json!({"source":manifest.clone()}),
    )
    .unwrap();
    assert_eq!(restored.fragments.len(), 2);
    assert_eq!(restored.work.fragments.len(), 1);
    assert_eq!(restored.hash, comparison.hash);
    let mut changed = all.clone();
    changed[1].role_metadata["own"] = json!(true);
    assert!(restore_scoped_window(&changed, all[0].work.work_ref, &manifest).is_none());
    assert!(restore_scoped_window(&all[..1], all[0].work.work_ref, &manifest).is_none());
    let mut source = research_windows(&all[0], 3000).remove(0);
    source.context_work_refs.push(all[1].work.work_ref);
    assert!(
        restore_scoped_window(
            &all[..1],
            all[0].work.work_ref,
            &reference_manifest(&source)
        )
        .is_some()
    );
}

#[test]
fn comparison_tracks_only_its_selected_discussion_definition_dependencies() {
    let mut all = two_works();
    let definition = Uuid::from_u128(80);
    for work in &mut all {
        work.topics = json!([{"definitionRef":definition}]);
    }
    let mut comparison = with_comparison_context(all[0].clone(), &all, &[all[1].work.work_ref]);
    let source_hash = comparison.hash.clone();
    comparison.coverage["selectedDiscussions"] =
        json!([{"unitId":"one","assignments":[{"definitionRef":definition}]}]);
    comparison.hash = input_identity(&comparison);
    assert_ne!(source_hash, comparison.hash);
    let manifest = reference_manifest(&comparison);
    assert!(restore_scoped_window(&all, all[0].work.work_ref, &manifest).is_some());
    all[0].topics = json!([{"definitionRef":Uuid::from_u128(81)}]);
    assert!(restore_scoped_window(&all, all[0].work.work_ref, &manifest).is_none());
}

#[test]
fn comparison_ignores_physical_task_relinks_and_checks_unassigned_compared_definitions() {
    let mut all = two_works();
    let definition = Uuid::from_u128(90);
    for input in &mut all {
        input.topics = json!([{"definitionRef":definition}]);
    }
    let mut comparison = with_comparison_context(all[0].clone(), &all, &[all[1].work.work_ref]);
    comparison.coverage["selectedDiscussions"] = json!([{"unitId":"same-unit","taskRef":Uuid::from_u128(91),
        "assignments":[],"comparedDefinitionRefs":[definition]}]);
    comparison.hash = input_identity(&comparison);
    let manifest = reference_manifest(&comparison);
    comparison.coverage["selectedDiscussions"][0]["taskRef"] = json!(Uuid::from_u128(92));
    assert_eq!(comparison.hash, input_identity(&comparison));
    assert!(restore_scoped_window(&all, all[0].work.work_ref, &manifest).is_some());
    all[0].topics = json!([]);
    assert!(restore_scoped_window(&all, all[0].work.work_ref, &manifest).is_none());
}

#[path = "tests/grouped.rs"]
mod grouped;
