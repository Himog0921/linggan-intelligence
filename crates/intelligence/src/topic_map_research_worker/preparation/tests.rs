use super::*;
use linggan_evidence::creator_discovery::Fragment;

fn fragment(id: &str, field: &str, start: usize, text: &str) -> Fragment {
    Fragment {
        fragment_id: id.into(),
        structure: json!({"ref":id,"kind":"paragraph","text":text,"speaker":if field.contains("comment") {"other"} else {"author"}}),
        source_ref: Uuid::from_u128(1),
        field: field.into(),
        source_version: "synthetic.v1".into(),
        start,
        end: start + text.chars().count(),
        text: text.into(),
        speaker: if field.contains("comment") {
            "other"
        } else {
            "author"
        }
        .into(),
        ..Fragment::default()
    }
}

#[test]
fn compact_input_indexes_keep_comment_parent_links_and_absolute_positions() {
    let fragments = vec![
        fragment(
            "synthetic.reply.chars.1200.1220",
            "unresearched_comment",
            1200,
            "😀尾部的新障碍仍需保留",
        ),
        fragment(
            "synthetic.parent.chars.8.20",
            "parent_comment_context",
            8,
            "请说明什么地方卡住",
        ),
        fragment(
            "synthetic.work.context",
            "work_context:body",
            5000,
            "作者实践建议",
        ),
    ];
    let projected = compact_provider_fragments(&fragments, Uuid::from_u128(1), &json!({}));
    assert_eq!(projected.len(), fragments.len());
    for (index, source) in fragments.iter().enumerate() {
        assert_eq!(projected[index]["fragmentIndex"], index);
        assert_eq!(projected[index]["start"], source.start);
        assert_eq!(projected[index]["end"], source.end);
        assert_eq!(projected[index]["content"]["text"], source.text);
        assert!(projected[index].get("text").is_none());
        assert_eq!(projected[index]["field"], source.field);
        assert!(projected[index].get("fragmentId").is_none());
    }
    let comments = json!([{"fragmentId":fragments[0].fragment_id,
        "sourceRef":"private-source","sourceFragmentId":"physical-original",
        "parentFragmentId":"physical-parent", "parentFragmentIds":[fragments[1].fragment_id],
        "parentSourceRef":"private-parent-source", "role":"eligible_user_comment", "contextOnly":false}]);
    let projected = compact_provider_comments(&comments, &fragments);
    assert_eq!(projected[0]["fragmentIndex"], 0);
    assert_eq!(projected[0]["parentFragmentIndexes"], json!([1]));
    assert_eq!(projected[0]["role"], "eligible_user_comment");
    assert_eq!(projected[0]["contextOnly"], false);
    assert!(!projected.to_string().contains("private-source"));
    assert!(!projected.to_string().contains("physical-original"));
    assert!(!projected.to_string().contains("synthetic.reply"));
    assert_eq!(comments[0]["fragmentId"], fragments[0].fragment_id);
}

#[test]
fn compact_comparison_preserves_each_work_owner_and_selected_citation() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let fragments = vec![
        fragment("synthetic.work.a", "body", 900, "甲的建议"),
        fragment("synthetic.work.b", "body", 1400, "乙的不同建议"),
    ];
    let coverage = json!({"kind":"comparison","fragmentWorkRefs":{
        "synthetic.work.a":first,"synthetic.work.b":second},
        "selectedDiscussions":[{"workRef":second,"evidence":[{"fragmentId":"synthetic.work.b","start":1400,"end":1406}]}]});
    let projected = compact_provider_fragments(&fragments, first, &coverage);
    assert_eq!(projected[0]["workRef"], json!(first));
    assert_eq!(projected[1]["workRef"], json!(second));
    let projected = compact_provider_coverage(&coverage, &fragments);
    assert!(projected.get("fragmentWorkRefs").is_none());
    assert_eq!(
        projected["selectedDiscussions"][0]["evidence"][0]["fragmentIndex"],
        1
    );
    assert_eq!(
        projected["selectedDiscussions"][0]["evidence"][0]["start"],
        1400
    );
    assert_eq!(
        coverage["selectedDiscussions"][0]["evidence"][0]["fragmentId"],
        "synthetic.work.b"
    );
}
