use super::*;

const QUESTION: &str = "样例如何说明练习内容？";

fn card() -> Value {
    json!({
        "core":{"knowledgeUnits":[{"id":"u1","statement":"每日必练。"}]},
        "evidence":[
            {"id":"intro","quote":"先介绍练习。","sourceRef":"body","sourceVersion":"v1","inputCitationId":"b","start":0,"end":6},
            {"id":"example","quote":"再列出题目。","sourceRef":"body","sourceVersion":"v1","inputCitationId":"b","start":6,"end":12},
            {"id":"frame","quote":"图中列出题目。","sourceRef":"frame","sourceVersion":"v1","inputCitationId":"f","start":30,"end":37},
            {"id":"otherBlock","quote":"另列出题目。","sourceRef":"body","sourceVersion":"v1","inputCitationId":"other","start":30,"end":37},
            {"id":"image","modelQuote":"第一课","observation":"页内有练习题。"}
        ]
    })
}

fn finding() -> Value {
    json!({
        "id":"f1","findingType":"structure","statement":"正文先写「先介绍练习。」，再写「再列出题目。」。",
        "basisUnitIds":["u1"],"questionFocus":"说明练习内容",
        "support":[{"evidenceRef":"intro","quote":"先介绍练习。"},{"evidenceRef":"example","quote":"再列出题目。"}],
        "presentationOrder":["intro","example"]
    })
}

fn check(finding: Value) -> Result<Value, &'static str> {
    accept(
        &json!({"findings":[finding]}).to_string(),
        QUESTION,
        &card(),
    )
}

#[test]
fn accepted_grounded_draft_keeps_content_card_finding_contract() {
    let accepted = check(finding()).unwrap();
    assert_eq!(
        accepted,
        json!([{
            "id":"f1","findingType":"structure","statement":finding()["statement"],
            "basisUnitIds":["u1"],"evidenceRefs":["intro","example"]
        }])
    );
    let payload = payload(QUESTION, &card());
    let evidence = payload["evidence"].as_array().unwrap();
    assert_eq!(
        evidence[0]["textPosition"]["block"],
        evidence[1]["textPosition"]["block"]
    );
    assert_ne!(
        evidence[0]["textPosition"]["block"],
        evidence[3]["textPosition"]["block"]
    );
    assert!(evidence[4].get("textPosition").is_none());
    assert!(
        evidence
            .iter()
            .all(|entry| entry.get("sourceRef").is_none())
    );
}

#[test]
fn core_summary_and_other_valid_evidence_ids_cannot_substitute_for_direct_quotes() {
    let mut draft = finding();
    draft["statement"] = json!("作者称「每日必练」。");
    assert_eq!(check(draft.clone()), Err(INVALID));
    draft["support"][0]["quote"] = json!("每日必练。");
    assert_eq!(check(draft), Err(INVALID));
}

#[test]
fn order_requires_forward_nonoverlapping_positions_in_one_text_block() {
    let mut draft = finding();
    draft["presentationOrder"] = json!(["example", "intro"]);
    assert_eq!(check(draft.clone()), Err(INVALID));

    for (id, quote) in [
        ("frame", "图中列出题目。"),
        ("otherBlock", "另列出题目。"),
        ("image", "第一课"),
    ] {
        let mut draft = finding();
        draft["statement"] = json!("先介绍练习，再列题。");
        draft["support"][1] = json!({"evidenceRef":id,"quote":quote});
        draft["presentationOrder"] = json!(["intro", id]);
        assert_eq!(check(draft.clone()), Err(INVALID));
        draft["statement"] = json!("材料包含练习说明与题目样例。");
        draft["presentationOrder"] = json!([]);
        assert!(check(draft).is_ok());
    }
}

#[test]
fn question_focus_must_come_from_the_requested_question() {
    let mut draft = finding();
    draft["questionFocus"] = json!("资料退款条件");
    assert_eq!(check(draft), Err(INVALID));
    assert_eq!(
        accept(r#"{"findings":[]}"#, QUESTION, &card()),
        Err(INSUFFICIENT_EVIDENCE)
    );
}
