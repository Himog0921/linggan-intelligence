use super::sources::{cited_ranges, frozen_slice, is_comment_field};
use serde_json::json;

#[test]
fn saved_unicode_tail_uses_absolute_scalar_ranges() {
    let text = format!("{}尾部👩‍👧只应引用这一段。", "前".repeat(12000));
    let start = 12000;
    let end = text.chars().count();
    let slice: String = text.chars().skip(start).collect();
    let manifest = json!({"fragmentId":"body.tail","start":start,"end":end,
        "textHash":linggan_evidence::creator_discovery::hash(&slice)});
    assert_eq!(frozen_slice(&text, &manifest), Some(slice));
    let citations = json!([
        {"fragmentId":"body.tail","start":12002,"end":end},
        {"fragmentId":"body.tail","start":0,"end":3},
        {"fragmentId":"body.tail","start":end-1,"end":end+1}
    ]);
    let ranges = cited_ranges(&manifest, &citations);
    assert_eq!(ranges.len(), 1);
    assert_eq!((ranges[0].start, ranges[0].end), (12002, end));
    assert!(
        frozen_slice(
            &text,
            &json!({"start":end,"end":end+1,"textHash":"invalid"})
        )
        .is_none()
    );
}

#[test]
fn parent_comment_is_a_comment_source_and_never_an_author_field() {
    assert!(is_comment_field("parent_comment_context"));
    assert!(is_comment_field("unresearched_comment"));
    assert!(is_comment_field("studied_comment"));
    assert!(!is_comment_field("body"));
}
