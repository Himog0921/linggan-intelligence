use super::super::{ResearchPacket, synthetic_packet};
use serde_json::{Value, json};

fn label(packet: &ResearchPacket) -> Value {
    json!({
        "label":"need",
        "basis":"explicit",
        "contextEvidence":[],
        "evidence":[{"quote":packet.cleaned[0].text}]
    })
}

fn item(packet: &ResearchPacket) -> Value {
    json!({
        "commentRef":"C001",
        "outcome":"interpretable",
        "labels":[label(packet)],
        "problems":[],
        "stances":[],
        "contextMissing":[],
        "uncertaintyReason":null,
        "limitations":[]
    })
}

fn packet_with_two_inputs() -> ResearchPacket {
    let mut packet = synthetic_packet();
    packet
        .inputs
        .push(crate::model_invocation::synthetic_input());
    packet.cleaned.push(packet.cleaned[0].clone());
    packet
}

#[test]
fn strict_normalization_accepts_only_bare_json_or_one_complete_json_fence() {
    let packet = synthetic_packet();
    let valid = json!({"comments":[item(&packet)]}).to_string();
    let fenced = format!("  ```json\n{valid}\n```  ");

    assert_eq!(
        ResearchPacket::normalization_kind(&valid),
        Some("bare_json")
    );
    assert_eq!(
        ResearchPacket::normalization_kind(&fenced),
        Some("json_fence")
    );
    assert!(packet.parse(&valid).unwrap()[0].is_ok());
    assert!(packet.parse(&fenced).unwrap()[0].is_ok());

    let mut embedded_fence = item(&packet);
    embedded_fence["limitations"] = json!(["原文含有```，仍是合法JSON字符串"]);
    let embedded_fence = json!({"comments":[embedded_fence]}).to_string();
    assert_eq!(
        ResearchPacket::normalization_kind(&embedded_fence),
        Some("bare_json")
    );
    assert!(packet.parse(&embedded_fence).unwrap()[0].is_ok());

    for rejected in [
        format!("解释\n```json\n{valid}\n```"),
        format!("```json\n{valid}\n```\n解释"),
        format!("```json\n{valid}"),
        format!("```JSON\n{valid}\n```"),
        format!("```json\n{valid}\n```\n```json\n{valid}\n```"),
    ] {
        assert_eq!(ResearchPacket::normalization_kind(&rejected), None);
        assert!(packet.parse(&rejected).is_err());
        assert_eq!(packet.validation_diagnostics(&rejected).len(), 1);
    }
}

#[test]
fn strict_normalization_rejects_duplicate_json_keys_before_value_decoding() {
    let packet = synthetic_packet();
    let valid = item(&packet).to_string();
    let duplicate_root = format!(r#"{{"comments":[{valid}],"comments":[{valid}]}}"#);
    let duplicate_item = r#"{
        "comments":[{
            "commentRef":"C001",
            "commentRef":"C001",
            "outcome":"no_signal",
            "labels":[],
            "problems":[],
            "stances":[],
            "contextMissing":[],
            "uncertaintyReason":null,
            "limitations":[]
        }]
    }"#;

    for rejected in [duplicate_root, duplicate_item.to_owned()] {
        assert_eq!(packet.parse(&rejected), Err("duplicate_json_key"));
        let diagnostics = packet.validation_diagnostics(&rejected);
        assert_eq!(diagnostics[0]["code"], "duplicate_json_key");
        assert_eq!(diagnostics[0]["actual"]["type"], "invalid_json");
    }
}

#[test]
fn strict_envelope_rejects_unknown_root_fields_and_comment_identities() {
    let packet = synthetic_packet();
    let unknown_root = json!({
        "comments":[item(&packet)],
        "PRIVATE_UNTRUSTED_ROOT":"PRIVATE_UNTRUSTED_VALUE"
    })
    .to_string();
    assert_eq!(packet.parse(&unknown_root), Err("schema_invalid"));
    assert!(
        !json!(packet.validation_diagnostics(&unknown_root))
            .to_string()
            .contains("PRIVATE_UNTRUSTED")
    );

    let mut unknown = item(&packet);
    unknown["commentRef"] = json!("C999");
    let unknown = json!({"comments":[unknown]}).to_string();
    assert_eq!(packet.parse(&unknown), Err("unknown_comment"));
    assert_eq!(
        packet.validation_diagnostics(&unknown)[0]["code"],
        "unknown_comment"
    );
}

#[test]
fn missing_members_remain_member_failures_while_known_members_are_accepted() {
    let packet = packet_with_two_inputs();
    let parsed = packet
        .parse(&json!({"comments":[item(&packet)]}).to_string())
        .unwrap();

    assert!(parsed[0].is_ok());
    assert_eq!(parsed[1], Err("missing_comment"));
}

#[test]
fn rejected_auxiliary_text_does_not_discard_an_independent_problem() {
    let packet = synthetic_packet();
    let mut output = item(&packet);
    output["labels"] = json!([]);
    output["problems"] = json!([{
        "name":"执行困难",
        "meaning":"评论者描述执行困难",
        "basis":"explicit",
        "contextEvidence":[],
        "evidence":[{"quote":packet.cleaned[0].text}]
    }]);
    output["limitations"] = json!(["说明".repeat(101)]);

    let raw = json!({"comments":[output]}).to_string();
    let result = packet.parse(&raw).unwrap().remove(0).unwrap();
    assert_eq!(result["semantic"]["acceptance"], "partial");
    assert_eq!(result["semantic"]["problems"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["semantic"]["rejectedFields"][0]["path"],
        "limitations[0]"
    );
    assert_eq!(
        result["semantic"]["rejectedFields"][0]["code"],
        "auxiliary_text_bounds"
    );

    let diagnostics = packet.validation_diagnostics(&raw);
    assert_eq!(diagnostics[0]["path"], "$.comments[0].limitations[0]");
    assert_eq!(diagnostics[0]["code"], "auxiliary_text_bounds");
}

#[test]
fn all_rejected_semantic_fields_never_become_no_signal() {
    let packet = synthetic_packet();
    let mut output = item(&packet);
    output["labels"][0]["label"] = json!("not_a_contract_label");
    let raw = json!({"comments":[output]}).to_string();

    assert_eq!(packet.parse(&raw).unwrap()[0], Err("all_fields_rejected"));
    let diagnostics = packet.validation_diagnostics(&raw);
    assert_eq!(diagnostics[0]["code"], "field_schema_invalid");
    assert_ne!(diagnostics[0]["code"], "no_signal");
}
