//! Grouped-source, context, historical identity and unknown-send regressions.
use super::*;

#[test]
fn one_hundred_short_comments_pack_independent_voices_and_restore_precise_sources() {
    let comments: Vec<_> = (20..120)
        .map(|id| {
            source(
                "unresearched_comment",
                &format!("合成用户{id}：我仍然不知道该先做哪一步。"),
                id,
            )
        })
        .collect();
    let title = source("title", "任务启动的分步支持", 10);
    let body = source("body", &"这是作者提供的分步建议。".repeat(100), 11);
    let mut original = input(
        std::iter::once(title.clone())
            .chain(std::iter::once(body.clone()))
            .chain(comments.clone())
            .collect(),
    );
    original.comment_study = json!(
        comments
            .iter()
            .map(|c| comment_metadata(c, None, json!([])))
            .collect::<Vec<_>>()
    );
    update_source_coverage(&mut original);
    let windows = research_windows(&original, 3000);
    let groups: Vec<_> = windows
        .iter()
        .filter(|w| w.role_metadata["commentOnly"] == true)
        .collect();
    assert_eq!(groups.len(), 17);
    assert_eq!(windows.len(), 18); // author title and body share one request
    assert_eq!(
        windows[0]
            .work
            .fragments
            .iter()
            .map(|f| f.field.as_str())
            .collect::<Vec<_>>(),
        vec!["title", "body"]
    );
    let mut seen = std::collections::BTreeSet::new();
    for window in groups {
        assert!(window.comment_study.as_array().unwrap().len() <= 6);
        assert!(window.coverage["inputChars"].as_u64().unwrap() <= 3000);
        assert!(window.coverage["workContextPartial"].as_bool().unwrap());
        assert!(window.work.fragments.is_empty());
        let manifest = reference_manifest(window);
        let context: Vec<_> = manifest["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["contextOnly"] == true)
            .collect();
        assert!(!context.is_empty());
        assert!(
            context
                .iter()
                .all(|r| r["field"].as_str().unwrap().starts_with("work_context:"))
        );
        assert_eq!(
            restore_window(&original, &manifest).unwrap().hash,
            window.hash
        );
        for fragment in window.fragments.iter().filter(|f| is_research_evidence(f)) {
            let origin = window.coverage["fragmentOrigins"][&fragment.fragment_id]
                .as_str()
                .unwrap();
            assert!(seen.insert(origin.to_owned()));
            assert_eq!(fragment.start, 0);
            assert_eq!(
                fragment.text,
                comments
                    .iter()
                    .find(|c| c.fragment_id == origin)
                    .unwrap()
                    .text
            );
        }
    }
    assert_eq!(seen.len(), 100);
}

#[test]
fn author_context_is_source_qualified_and_never_turns_into_independent_evidence() {
    let comment = source("unresearched_comment", "照着做了仍然很难开始。", 20);
    let title = source("title", "小任务如何开始", 10);
    let body = source("body", "先把任务拆成小步骤，降低第一步的难度。", 11);
    let mut original = input(vec![title, body, comment.clone()]);
    original.comment_study = json!([comment_metadata(&comment, None, json!([]))]);
    let all_windows = research_windows(&original, 3000);
    let author_ids: std::collections::BTreeSet<_> = all_windows[0]
        .fragments
        .iter()
        .map(|f| f.fragment_id.as_str())
        .collect();
    let window = all_windows
        .iter()
        .find(|w| w.role_metadata["commentOnly"] == true)
        .unwrap();
    assert!(
        window
            .fragments
            .iter()
            .filter(|f| !is_research_evidence(f))
            .all(|f| !author_ids.contains(f.fragment_id.as_str()))
    );
    assert_eq!(
        window.coverage["windowChars"],
        json!(comment.text.chars().count())
    );
    assert_eq!(
        window
            .fragments
            .iter()
            .filter(|f| is_research_evidence(f))
            .count(),
        1
    );
    let manifest = reference_manifest(window);
    assert!(sources_current(&original, &manifest));
    let mut changed = original.clone();
    changed.fragments[1].text = changed.fragments[1].text.replacen("降低", "提高", 1);
    assert!(!sources_current(&changed, &manifest));
    let mut corrupt = manifest.clone();
    let context = corrupt["fragments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "work_context:body")
        .unwrap();
    context["contextOnly"] = json!(false);
    assert!(restore_window(&original, &corrupt).is_none());
    corrupt = manifest.clone();
    let context = corrupt["fragments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["field"] == "work_context:body")
        .unwrap();
    context["field"] = json!("work_context:transcript");
    assert!(restore_window(&original, &corrupt).is_none());
}

#[test]
fn grouped_children_share_parent_context_and_keep_the_parent_tail() {
    let parent = source(
        PARENT_CONTEXT_FIELD,
        &format!("{}父评论的最后限制。", "既有父评论语境。".repeat(700)),
        21,
    );
    let children: Vec<_> = (30..34)
        .map(|id| source("unresearched_comment", "我也是这样，下一步怎么办？", id))
        .collect();
    let mut original = input(
        children
            .iter()
            .cloned()
            .chain(std::iter::once(parent.clone()))
            .collect(),
    );
    original.comment_study = json!(
        children
            .iter()
            .map(|c| comment_metadata(c, Some(&parent), json!([])))
            .collect::<Vec<_>>()
    );
    update_source_coverage(&mut original);
    let windows = research_windows(&original, 3000);
    assert!(windows.len() > 1);
    let text: String = windows
        .iter()
        .flat_map(|w| w.fragments.iter())
        .filter(|f| f.field == PARENT_CONTEXT_FIELD)
        .map(|f| f.text.as_str())
        .collect();
    assert_eq!(text, parent.text);
    assert_eq!(
        windows
            .iter()
            .map(|w| w.coverage["windowChars"].as_u64().unwrap())
            .sum::<u64>(),
        children
            .iter()
            .map(|f| f.text.chars().count() as u64)
            .sum::<u64>()
    );
    for window in &windows {
        assert_eq!(window.comment_study.as_array().unwrap().len(), 4);
        assert!(
            window
                .comment_study
                .as_array()
                .unwrap()
                .iter()
                .all(|c| !c["parentFragmentIds"].as_array().unwrap().is_empty())
        );
        assert!(sources_current(&original, &reference_manifest(window)));
    }
}

#[test]
fn historical_v2_frozen_window_keeps_read_qualification_without_new_dispatch_identity() {
    let original = input(vec![source("body", "历史窗口的合格原声。", 10)]);
    let mut window = research_windows(&original, 3000).remove(0);
    window.hash = super::super::identity::input_identity_for(
        &window,
        "topic-map.source-windows.v1",
        "topic-map.research.v2",
    );
    let mut manifest = reference_manifest(&window);
    manifest["inputContract"] = json!("topic-map.source-windows.v1");
    manifest["methodVersion"] = json!("topic-map.research.v2");
    assert_eq!(
        restore_window(&original, &manifest).unwrap().hash,
        window.hash
    );
    assert_ne!(research_windows(&original, 3000)[0].hash, window.hash);
    manifest["methodVersion"] = json!("unrecognized-method");
    assert!(restore_window(&original, &manifest).is_none());
}

#[test]
fn missing_parent_pages_never_reopen_unknown_child_when_short_voices_are_packed() {
    let child = source("unresearched_comment", "已发出的未知回复。", 20);
    let parent = source(
        PARENT_CONTEXT_FIELD,
        &"尚未处理完的父评论语境。".repeat(500),
        21,
    );
    let mut old = input(vec![child.clone(), parent.clone()]);
    old.comment_study = json!([comment_metadata(&child, Some(&parent), json!([]))]);
    let unknown = research_windows(&old, 3000).remove(0);
    let refs = reference_manifest(&unknown)["fragments"]
        .as_array()
        .unwrap()
        .clone();
    let mut expanded = old.clone();
    let voices: Vec<_> = (30..35)
        .map(|id| {
            source(
                "unresearched_comment",
                &format!("独立新用户{id}的练习启动问题。"),
                id,
            )
        })
        .collect();
    expanded.fragments.extend(voices.clone());
    expanded
        .comment_study
        .as_array_mut()
        .unwrap()
        .extend(voices.iter().map(|c| comment_metadata(c, None, json!([]))));
    update_source_coverage(&mut expanded);
    let pending = super::super::admission::pending_input(&expanded, &[], &refs);
    assert!(
        !pending
            .fragments
            .iter()
            .any(|f| f.fragment_id == child.fragment_id)
    );
    let windows = research_windows(&pending, 3000);
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].comment_study.as_array().unwrap().len(), 5);
    assert!(!overlaps_unknown_dispatch(&windows[0], &expanded, &refs));
}
