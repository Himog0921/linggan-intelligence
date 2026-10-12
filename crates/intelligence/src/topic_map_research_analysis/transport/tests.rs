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
        "responseMatches":[{"status":"unknown","unanswered":["缺少双方材料"],"evidence":[]}],
        "angles":[{"label":"启动支持","title":"如何开始第一步","answerTask":"解释如何降低启动难度","evidence":evidence}],
        "productOpportunities":[{"need":"开始任务的支持","hypothesis":"较小的第一步可能有帮助",
            "verificationQuestion":"是否更容易开始","alternativeExplanation":"可能是任务本身较容易","evidence":evidence}],
        "limitations":["少量合成材料，无代表性"]})
}

fn wire(value: &Value, fragments: &[Fragment]) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, child)| {
                    let child = if key == "fragmentId" {
                        json!(
                            fragments
                                .iter()
                                .position(|f| child.as_str() == Some(f.fragment_id.as_str()))
                                .unwrap()
                        )
                    } else {
                        wire(child, fragments)
                    };
                    (wire_key(key).to_owned(), child)
                })
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
fn compact_roundtrip_preserves_every_field_and_absolute_unicode_tail() {
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
    assert!(compact.to_string().len() < original.to_string().len() * 3 / 4);
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
    let restored = decode_compact_output(&invalid.to_string(), &fragments).unwrap();
    assert!(validate_output(&restored, &fragments, &[]).is_err());
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
