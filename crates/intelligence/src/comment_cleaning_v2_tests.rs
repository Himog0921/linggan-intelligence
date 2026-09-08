use super::*;

#[test]
fn definite_noise_has_no_research_text() {
    for text in [
        "😂😂😂",
        "❤️❤️",
        "👨‍👩‍👧",
        "🀄🃏",
        "🫩",
        "#️⃣*️⃣",
        "🇨🇳",
        "👍🏽",
        "1️⃣",
        "@小明",
        "@小明 😂😂",
        "！！！",
        "......",
        "\t \n",
        "\u{200b}",
    ] {
        let value = clean(text);
        assert_eq!(value.state, "dropped", "{text:?}");
        assert!(value.text.is_empty());
        assert!(value.offsets.is_empty());
    }
}

#[test]
fn meaningful_short_text_and_mixed_comments_survive() {
    for (raw, expected) in [
        ("@小明 有效果吗？", "有效果吗?"),
        ("😂 我家也是，每天写作业都这样", "我家也是,每天写作业都这样"),
        ("@作者 求分享具体方法", "求分享具体方法"),
        ("@小明 😂😂 同问", "同问"),
        ("👍🏽 不可以！！！", "不可以!!!"),
        ("1 2 3 ＡＢＣ 数学𝑥² = 4", "1 2 3 ABC 数学𝑥² = 4"),
        ("∞", "∞"),
    ] {
        let value = clean(raw);
        assert_ne!(value.state, "dropped", "{raw:?}");
        assert_eq!(value.text, expected);
        assert_eq!(value.text.chars().count(), value.offsets.len());
    }
    for raw in [
        "蹲",
        "同问",
        "求分享",
        "有效果吗",
        "我家也是",
        "ADHD",
        "24",
        "-1",
        "𝑥",
    ] {
        assert!(
            matches!(clean(raw).state.as_str(), "direct" | "context"),
            "{raw:?}"
        );
    }
}

#[test]
fn mention_fallback_does_not_consume_email_or_attached_prose() {
    for raw in [
        "联系 test@example.com",
        "@小明有效果吗？",
        "句中@小明 有效",
        "@something.example",
        "@",
        "a@b",
    ] {
        assert!(clean(raw).text.contains('@') || raw == "@", "{raw:?}");
    }
    let raw = "@名字带 空格分享有效方法";
    let value = clean_with_mentions(raw, &[MentionSpan { start: 0, end: 7 }]);
    assert_eq!(value.text, "分享有效方法");
    assert_eq!(
        value.resolve("分享有效方法", raw).unwrap(),
        (7, 13, "分享有效方法".into())
    );
    let invalid = clean_with_mentions("分享有效方法", &[MentionSpan { start: 0, end: 99 }]);
    assert_eq!(invalid.text, "分享有效方法");
    assert!(
        invalid
            .reasons
            .iter()
            .any(|r| r == "mention_boundary_invalid")
    );
}

#[test]
fn original_unicode_spans_survive_removed_noise_and_normalization() {
    let raw = " @小明 👨‍👩‍👧我不想\n  催促 &amp; 监督";
    let value = clean(raw);
    assert_eq!(value.text, "我不想 催促 & 监督");
    assert_eq!(value.resolve("不想 催促", raw).unwrap().2, "不想\n  催促");
    assert_eq!(value.resolve("&", raw).unwrap().2, "&amp;");
    for (i, (a, b)) in value.offsets.iter().copied().enumerate() {
        assert!(a < b && b <= raw.chars().count(), "{i}");
        if i > 0 {
            assert!(value.offsets[i - 1].1 <= a);
        }
    }
    let inline = clean("我😂不想");
    assert_eq!(inline.resolve("我不想", "我😂不想").unwrap().2, "我😂不想");
}

#[test]
fn encoding_damage_is_not_reported_as_valid_empty_analysis() {
    assert_eq!(clean("��").state, "anomaly");
    assert_eq!(clean("�� !!!").state, "anomaly");
    assert_eq!(clean_optional(None).state, "anomaly");
    assert_eq!(clean_optional(Some("")).state, "dropped");
    assert_eq!(clean(&"长".repeat(16001)).state, "anomaly");
    assert_eq!(CLEANER_VERSION, "comment-clean.v2");
    assert_eq!(unicode_properties::UNICODE_VERSION, (17, 0, 0));
}

#[test]
fn outbound_urls_are_redacted_without_changing_source_offsets() {
    let raw = "有效方法 https://example.test/a?q=1 更多信息";
    let local = clean(raw);
    assert!(local.text.contains("example.test"));
    let sent = outbound(local.clone());
    assert!(!sent.text.contains("example.test"));
    assert_eq!(sent.offsets, local.offsets);
    assert!(sent.resolve("有效方法", raw).is_ok());
    assert!(sent.reasons.iter().any(|r| r == "url_masked"));
    let adjacent = outbound(clean("参考https://example.test/a?q=1，依旧不知道怎么办"));
    assert!(adjacent.text.ends_with(",依旧不知道怎么办"));
    assert!(!adjacent.text.contains("example.test"));
}
