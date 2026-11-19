//! The work context of a unit: a projection of one frozen L1 Core card, shared by all its targets.
//!
//! It carries what helps read a comment about the work and nothing that could be mistaken for
//! comment evidence. Source fragments, candidate labels and the L2 extension stay out; the model
//! never sees evidence references, because it cannot open them.
use serde_json::{Value, json};

/// Knowledge unit types that explain what the work says and for whom.
const CONTEXT_UNIT_TYPES: [&str; 5] = [
    "discussion",
    "author_position",
    "provision",
    "reader_value",
    "audience_context",
];

/// `None` when the card is not a usable Core card.
pub fn work_context(card: &Value) -> Option<Value> {
    let core = card.get("core")?;
    let summary = core.get("summary")?.get("text")?.as_str()?;
    let units: Vec<Value> = core
        .get("knowledgeUnits")?
        .as_array()?
        .iter()
        .filter(|unit| {
            unit["unitType"]
                .as_str()
                .is_some_and(|t| CONTEXT_UNIT_TYPES.contains(&t))
        })
        .map(|unit| {
            json!({
                "id": unit["id"], "unitType": unit["unitType"], "statement": unit["statement"],
                "assertionKind": unit["assertionKind"], "speakerRole": unit["speakerRole"],
            })
        })
        .collect();
    Some(json!({
        "analysisRef": card["analysisRef"],
        "contentType": core["contentType"],
        "summary": summary,
        "mainTopicUnitId": core["mainTopicUnitId"],
        "knowledgeUnits": units,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_context_units_and_drops_evidence_and_the_extension() {
        let card = json!({
            "analysisRef": "a1", "evidence": [{"text":"原文片段"}], "deep": {"x": 1},
            "core": {
                "contentType": "advice", "mainTopicUnitId": "u1",
                "summary": {"text": "讲孩子写作业启动困难", "unitIds": ["u1"]},
                "candidateLabels": ["拖延"],
                "knowledgeUnits": [
                    {"id":"u1","unitType":"discussion","statement":"启动困难","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["e1"]},
                    {"id":"u2","unitType":"example","statement":"某个例子","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["e2"]},
                    {"id":"u3","unitType":"reader_value","statement":"家长能学到方法","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":[]}
                ]
            }
        });
        let context = work_context(&card).unwrap();
        let ids: Vec<_> = context["knowledgeUnits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| u["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["u1", "u3"]);
        assert_eq!(context["summary"], "讲孩子写作业启动困难");
        let text = context.to_string();
        for leaked in ["evidenceRefs", "原文片段", "candidateLabels", "deep"] {
            assert!(!text.contains(leaked), "{leaked}");
        }
    }

    #[test]
    fn a_card_without_a_summary_is_not_a_context() {
        assert!(work_context(&json!({"core":{"knowledgeUnits":[]}})).is_none());
        assert!(work_context(&json!({})).is_none());
    }
}
