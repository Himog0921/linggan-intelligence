use super::*;
use crate::topic_map_research_analysis::{validate_context_links, validate_output};
use uuid::Uuid;

fn fragment(id: &str, field: &str, start: usize, text: &str) -> Fragment {
    Fragment {
        fragment_id: id.into(),
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

fn semantic(fragments: &[Fragment]) -> Value {
    let evidence: Vec<_> = fragments
        .iter()
        .map(|f| json!({"fragmentId":f.fragment_id,"start":f.start,"end":f.end}))
        .collect();
    json!({"contract":EXTRACT_CONTRACT,"outcome":"analyzed",
        "discussions":[{"label":"开始任务困难","statement":"已决定执行，但不知道第一步如何开始。",
            "definition":"开始一项已决定执行的任务时的困难与支持", "inclusionCriteria":["已决定执行而难以开始"],
            "exclusionCriteria":["开始之后的持续注意困难"], "speakerRole":"author", "evidenceRole":"support",
            "rationale":"原文描述启动阶段", "topicRef":null,"evidence":evidence}],
        "scenes":[{"label":"家庭任务场景","evidence":evidence}],
        "journey":{"mainStage":"begin_practice","involvedStages":["begin_practice"],"overlays":[],
            "path":"unknown","rationale":"有执行任务依据，身份未知","evidence":evidence},
        "responseMatches":[{"status":"unknown","unanswered":["缺少双方材料"],"evidence":evidence}],
        "angles":[{"label":"启动支持","title":"如何开始第一步","answerTask":"解释如何降低启动难度","evidence":evidence}],
        "productOpportunities":[{"need":"开始任务的支持","hypothesis":"较小的第一步可能有帮助",
            "verificationQuestion":"是否更容易开始","alternativeExplanation":"可能是任务本身较容易","evidence":evidence}],
        "limitations":["少量合成材料，无代表性"]})
}

fn wire(value: &Value, fragments: &[Fragment]) -> Value {
    match value {
        Value::Object(fields) if fields.contains_key("fragmentId") => {
            let index = fragments
                .iter()
                .position(|fragment| {
                    value["fragmentId"].as_str() == Some(fragment.fragment_id.as_str())
                })
                .unwrap();
            let fragment = &fragments[index];
            let start = value["start"].as_u64().unwrap() as usize;
            let end = value["end"].as_u64().unwrap() as usize;
            let quote: String = fragment
                .text
                .chars()
                .skip(start - fragment.start)
                .take(end - start)
                .collect();
            json!({"f":index,"q":quote})
        }
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, child)| (wire_key(key).to_owned(), wire(child, fragments)))
                .collect(),
        ),
        Value::Array(values) => json!(
            values
                .iter()
                .map(|value| wire(value, fragments))
                .collect::<Vec<_>>()
        ),
        _ => value.clone(),
    }
}

fn packet(value: &Value, fragments: &[Fragment]) -> Value {
    let mut wire = wire(value, fragments);
    wire["contract"] = json!(COMPACT_EXTRACT_CONTRACT);
    wire
}

#[test]
fn quote_roundtrip_preserves_every_field_and_absolute_unicode_tail() {
    let fragments = vec![
        fragment(
            "00000000-0000-4000-8000-000000000001.body.chars.4200.4230",
            "body",
            4200,
            "😀开始任务很困难，最后的反例仍要保留。",
        ),
        fragment(
            "00000000-0000-4000-8000-000000000001.ocr.page.11.chars.1990.2010",
            "ocr",
            1990,
            "这一方法对我没有作用。",
        ),
    ];
    let original = semantic(&fragments);
    let compact = packet(&original, &fragments);
    let restored = decode_compact_output(&compact.to_string(), &fragments).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), original);
    assert_eq!(validate_output(&restored, &fragments, &[]), Ok(()));
    assert_eq!(
        validate_context_links(&restored, &fragments, &json!([])),
        Ok(())
    );
    assert!(compact.to_string().len() < original.to_string().len());
    eprintln!(
        "synthetic output bytes: canonical={}, compact={} (not provider tokens)",
        original.to_string().len(),
        compact.to_string().len()
    );
}

#[test]
fn compact_decoder_rejects_unknown_missing_and_wrong_types_at_static_paths() {
    let fragments = vec![fragment("synthetic.body", "body", 0, "开始任务很困难")];
    let valid = packet(&semantic(&fragments), &fragments);
    let mut missing = valid.clone();
    missing["d"][0].as_object_mut().unwrap().remove("x");
    let error = decode_compact_output(&missing.to_string(), &fragments).unwrap_err();
    assert_eq!(
        (error.code, error.path.as_str(), error.rule),
        (
            "output_schema_invalid",
            "discussions[0].exclusionCriteria",
            "required_property"
        )
    );
    let mut unknown = valid.clone();
    unknown["d"][0]["PRIVATE-MATERIAL-MUST-NOT-LEAK"] = json!("private");
    let error = decode_compact_output(&unknown.to_string(), &fragments).unwrap_err();
    assert_eq!(
        (error.path.as_str(), error.rule),
        ("discussions[0]", "unknown_property")
    );
    assert!(!format!("{error:?}").contains("PRIVATE"));
    let mut wrong = valid;
    wrong["d"][0]["i"] = json!([0]);
    let error = decode_compact_output(&wrong.to_string(), &fragments).unwrap_err();
    assert_eq!(
        (error.path.as_str(), error.rule),
        ("discussions[0].inclusionCriteria[0]", "type")
    );
}

#[test]
fn compact_citation_index_never_guesses_or_admits_outside_frozen_input() {
    let fragments = vec![fragment("synthetic.body", "body", 30, "开始任务很困难")];
    let valid = packet(&semantic(&fragments), &fragments);
    for index in [json!(-1), json!(1), json!(0.5), json!("synthetic.body")] {
        let mut invalid = valid.clone();
        invalid["d"][0]["e"][0]["f"] = index;
        let error = decode_compact_output(&invalid.to_string(), &fragments).unwrap_err();
        assert_eq!(
            (error.code, error.path.as_str(), error.rule),
            (
                "invalid_citation_fragment_index",
                "discussions[0].evidence[0].fragmentId",
                "fragment_index"
            )
        );
    }
    let mut invalid = valid;
    invalid["d"][0]["e"][0]["s"] = json!(0);
    let error = decode_compact_output(&invalid.to_string(), &fragments).unwrap_err();
    assert_eq!(
        (error.code, error.rule),
        ("output_schema_invalid", "unknown_property")
    );
}

#[test]
fn restored_comment_roles_and_parent_links_still_require_real_main_evidence() {
    let fragments = vec![
        fragment(
            "synthetic.reply",
            "unresearched_comment",
            100,
            "已开始但又中断",
        ),
        fragment(
            "synthetic.parent",
            "parent_comment_context",
            8,
            "你已经开始了吗",
        ),
    ];
    let mut original = semantic(&fragments);
    original["discussions"][0]["speakerRole"] = json!("commenter");
    original["scenes"] = json!([]);
    original["journey"]["mainStage"] = json!("unclear");
    original["journey"]["involvedStages"] = json!([]);
    original["journey"]["evidence"] = json!([]);
    original["angles"] = json!([]);
    original["productOpportunities"] = json!([]);
    let restored =
        decode_compact_output(&packet(&original, &fragments).to_string(), &fragments).unwrap();
    assert_eq!(validate_output(&restored, &fragments, &[]), Ok(()));
    assert_eq!(
        validate_context_links(
            &restored,
            &fragments,
            &json!([{"fragmentId":"synthetic.reply","parentFragmentIds":["synthetic.parent"]}])
        ),
        Ok(())
    );
    assert!(
        validate_context_links(
            &restored,
            &fragments,
            &json!([{"fragmentId":"other.reply","parentFragmentIds":["synthetic.parent"]}])
        )
        .is_err()
    );
    let mut author = restored.clone();
    author.discussions[0].speaker_role = "author".into();
    assert!(validate_output(&author, &fragments, &[]).is_err());
}

#[test]
fn compact_schema_has_no_collisions_and_preserves_complete_required_objects() {
    fn check(schema: &Value) {
        if let Some(fields) = schema["properties"].as_object() {
            assert_eq!(fields.len(), schema["required"].as_array().unwrap().len());
            assert_eq!(schema["additionalProperties"], false);
            for child in fields.values() {
                check(child);
            }
        }
        if schema["type"] == "array" {
            check(&schema["items"]);
        }
    }
    let schema = compact_output_schema();
    check(&schema);
    assert_eq!(
        schema["properties"]["d"]["items"]["properties"]["e"]["items"]["properties"]["f"]["type"],
        "integer"
    );
    assert_eq!(
        schema["properties"]["d"]["items"]["properties"]
            .as_object()
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn short_quote_offsets_are_computed_from_absolute_unicode_scalar_boundaries() {
    let fragments = vec![fragment(
        "synthetic.body",
        "body",
        4200,
        "前文😀e\u{301}B后文；尾段",
    )];
    let mut valid = packet(&semantic(&fragments), &fragments);
    valid["d"][0]["e"] = json!([{"f":0,"q":"😀e\u{301}B"},{"f":0,"q":"尾段"}]);
    let restored = decode_compact_output(&valid.to_string(), &fragments).unwrap();
    let quotes = &restored.discussions[0].evidence;
    assert_eq!((quotes[0].start, quotes[0].end), (4202, 4206));
    assert_eq!((quotes[1].start, quotes[1].end), (4209, 4211));
    assert_eq!(validate_output(&restored, &fragments, &[]), Ok(()));
}

#[test]
fn one_claim_can_keep_multiple_complete_verbatim_quotes_from_the_same_source() {
    let fragments = vec![fragment(
        "synthetic.comment",
        "unresearched_comment",
        91,
        "已决定执行，中间还有其他背景，但第一步无法开始。",
    )];
    let mut valid = packet(&semantic(&fragments), &fragments);
    valid["d"][0]["sp"] = json!("commenter");
    valid["d"][0]["e"] = json!([{"f":0,"q":"已决定执行"},{"f":0,"q":"第一步无法开始"}]);
    let restored = decode_compact_output(&valid.to_string(), &fragments).unwrap();
    let quotes: Vec<String> = restored.discussions[0]
        .evidence
        .iter()
        .map(|citation| {
            fragments[0]
                .text
                .chars()
                .skip(citation.start - fragments[0].start)
                .take(citation.end - citation.start)
                .collect()
        })
        .collect();
    assert_eq!(quotes, ["已决定执行", "第一步无法开始"]);
    assert_eq!(validate_output(&restored, &fragments, &[]), Ok(()));
}

#[test]
fn ambiguous_missing_blank_or_normalized_quotes_never_guess_an_occurrence() {
    for (text, quote, code, rule) in [
        (
            "开始后再开始",
            "开始",
            "citation_quote_ambiguous",
            "unique_quote",
        ),
        ("哈哈哈", "哈哈", "citation_quote_ambiguous", "unique_quote"),
        (
            "真实原文",
            "PRIVATE-CLAIM-NOT-IN-SOURCE",
            "citation_quote_not_found",
            "verbatim_quote",
        ),
        (
            "真实原文",
            "",
            "invalid_citation_quote",
            "quote_nonblank_bounded",
        ),
        (
            "真实原文",
            " \t\n",
            "invalid_citation_quote",
            "quote_nonblank_bounded",
        ),
        (
            "开始",
            " 开始",
            "citation_quote_not_found",
            "verbatim_quote",
        ),
        (
            "e\u{301}",
            "é",
            "citation_quote_not_found",
            "verbatim_quote",
        ),
    ] {
        let fragments = vec![fragment("synthetic.body", "body", 90, text)];
        let mut invalid = packet(&semantic(&fragments), &fragments);
        invalid["d"][0]["e"][0]["q"] = json!(quote);
        let error = decode_compact_output(&invalid.to_string(), &fragments).unwrap_err();
        assert_eq!((error.code, error.rule), (code, rule));
        assert_eq!(error.path, "discussions[0].evidence[0].quote");
        assert!(!format!("{error:?}").contains("PRIVATE"));
    }
}

#[test]
fn quote_bounds_and_field_types_are_strict_without_legacy_offset_fallback() {
    let fragments = vec![fragment("synthetic.body", "body", 5, "原文支持")];
    let valid = packet(&semantic(&fragments), &fragments);
    for quote in [json!(null), json!(0), json!(["原文"])] {
        let mut invalid = valid.clone();
        invalid["d"][0]["e"][0]["q"] = quote;
        let error = decode_compact_output(&invalid.to_string(), &fragments).unwrap_err();
        assert_eq!((error.code, error.rule), ("output_schema_invalid", "type"));
    }
    let mut missing = valid.clone();
    missing["d"][0]["e"][0].as_object_mut().unwrap().remove("q");
    let error = decode_compact_output(&missing.to_string(), &fragments).unwrap_err();
    assert_eq!(
        (error.path.as_str(), error.rule),
        ("discussions[0].evidence[0].quote", "required_property")
    );
    let mut huge = valid.clone();
    huge["d"][0]["e"][0]["q"] = json!("字".repeat(1001));
    assert_eq!(
        decode_compact_output(&huge.to_string(), &fragments)
            .unwrap_err()
            .code,
        "invalid_citation_quote"
    );
    for old_offsets in [
        json!({"f":0,"s":5,"e":9}),
        json!({"f":0,"q":"原文","s":5}),
        json!({"f":0,"q":"原文","e":9}),
    ] {
        let mut invalid = valid.clone();
        invalid["d"][0]["e"][0] = old_offsets;
        assert_eq!(
            decode_compact_output(&invalid.to_string(), &fragments)
                .unwrap_err()
                .rule,
            "unknown_property"
        );
    }
    let mut historical = valid;
    historical["contract"] = json!("topic-map.extract.compact.v1");
    assert_eq!(
        decode_compact_output(&historical.to_string(), &fragments)
            .unwrap_err()
            .code,
        "output_contract_mismatch"
    );
}

#[test]
fn quotes_are_scoped_to_one_frozen_fragment_and_invalid_fragment_bounds_are_rejected() {
    let fragments = vec![
        fragment("synthetic.a", "body", 20, "共有原文"),
        fragment("synthetic.b", "ocr", 3000, "共有原文"),
    ];
    let valid = packet(&semantic(&fragments), &fragments);
    let restored = decode_compact_output(&valid.to_string(), &fragments).unwrap();
    assert_eq!(
        restored.discussions[0].evidence[1].fragment_id,
        "synthetic.b"
    );
    assert_eq!(
        (
            restored.discussions[0].evidence[1].start,
            restored.discussions[0].evidence[1].end
        ),
        (3000, 3004)
    );
    let mut corrupt = fragments.clone();
    corrupt[0].end += 1;
    assert_eq!(
        decode_compact_output(&valid.to_string(), &corrupt)
            .unwrap_err()
            .code,
        "invalid_unicode_fragment"
    );
}

#[test]
fn every_evidence_category_uses_only_required_index_and_quote_fields() {
    let schema = compact_output_schema();
    let categories = [
        &schema["properties"]["d"]["items"]["properties"]["e"],
        &schema["properties"]["s"]["items"]["properties"]["e"],
        &schema["properties"]["j"]["properties"]["e"],
        &schema["properties"]["rm"]["items"]["properties"]["e"],
        &schema["properties"]["a"]["items"]["properties"]["e"],
        &schema["properties"]["p"]["items"]["properties"]["e"],
    ];
    for category in categories {
        let citation = &category["items"];
        assert_eq!(citation["properties"].as_object().unwrap().len(), 2);
        assert_eq!(citation["required"], json!(["f", "q"]));
        assert_eq!(citation["additionalProperties"], false);
        assert_eq!(citation["properties"]["q"]["maxLength"], 1000);
    }
    let fields = compact_field_names();
    assert!(fields.get("start").is_none());
    assert!(fields.get("end").is_none());
    assert_eq!(fields["fragmentId"], "f");
    assert_eq!(fields["quote"], "q");
}
