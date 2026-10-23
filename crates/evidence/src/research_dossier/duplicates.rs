//! Byte identity can establish page equivalence, never erase a differing human decision.
use serde_json::{Value, json};

pub(super) fn collapse(mut slots: Vec<Value>, readings: &[Value]) -> Vec<Value> {
    let Some(first) = slots
        .iter()
        .find(|s| {
            s["component"] == "image"
                && s["purpose"] == "body_image"
                && s["page"] == 1
                && s["sourceState"] == "present"
        })
        .cloned()
    else {
        return slots;
    };
    let Some(hash) = first["blobSha256"].as_str() else {
        return slots;
    };
    let group: Vec<_> = slots
        .iter()
        .filter(|s| {
            s["component"] == "image"
                && s["sourceState"] == "present"
                && s["blobSha256"] == hash
                && (s["slotKey"] == first["slotKey"] || s["purpose"] == "cover")
        })
        .collect();
    if group.len() < 2 {
        return slots;
    }
    let available: Vec<_> = group
        .iter()
        .filter_map(|slot| reading(slot, readings).map(|r| (*slot, r)))
        .collect();
    let human: Vec<_> = available
        .iter()
        .copied()
        .filter(|(_, r)| r["reading"]["adoption"] != "automatic")
        .collect();
    let choices = if human.is_empty() { &available } else { &human };
    let compatible = choices
        .first()
        .is_none_or(|(_, first)| choices.iter().all(|(_, other)| equivalent(first, other)));
    if !compatible {
        let keys: Vec<_> = group.iter().map(|s| s["slotKey"].clone()).collect();
        for slot in &mut slots {
            if keys.contains(&slot["slotKey"]) {
                slot["duplicateReadingConflict"] = json!(true);
            }
        }
        return slots;
    }
    let chosen = choices
        .iter()
        .find(|(s, _)| s["slotKey"] == first["slotKey"])
        .or_else(|| choices.first())
        .map_or(first.clone(), |(s, _)| (*s).clone());
    let keys: Vec<_> = group.iter().map(|s| s["slotKey"].clone()).collect();
    slots.retain(|s| {
        s["component"] != "image"
            || !keys.contains(&s["slotKey"])
            || s["slotKey"] == chosen["slotKey"]
    });
    if chosen["slotKey"] != first["slotKey"] {
        if let Some(slot) = slots
            .iter_mut()
            .find(|s| s["slotKey"] == chosen["slotKey"] && s["component"] == "image")
        {
            slot["page"] = json!(1);
            slot["displayOrderState"] = json!("KNOWN");
            slot["pageSource"] = json!({"basis":"same_verified_bytes","sourceRef":first["sourceRef"],"sourceVersion":first["sourceVersion"],"slotKey":first["slotKey"]});
        }
    }
    slots
}

fn reading<'a>(slot: &Value, readings: &'a [Value]) -> Option<&'a Value> {
    readings.iter().find(|r| {
        r["kind"] == "image"
            && r["slotKey"] == slot["slotKey"]
            && r["sourceVersion"] == slot["sourceVersion"]
            && r["currentRevisionRef"].is_string()
            && r["sourceState"] != "restricted"
    })
}
fn equivalent(a: &Value, b: &Value) -> bool {
    let content = |r: &Value| {
        r["blocks"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|block| {
                let mut block = block.clone();
                if let Some(object) = block.as_object_mut() {
                    object.remove("ref");
                }
                block
            })
            .collect::<Vec<_>>()
    };
    content(a) == content(b)
        && a["issues"] == b["issues"]
        && a["reading"]["adoption"] == b["reading"]["adoption"]
        && a["reading"]["verification"] == b["reading"]["verification"]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slot(key: &str, purpose: &str, page: Value) -> Value {
        json!({"slotKey":key,"purpose":purpose,"page":page,"component":"image","sourceState":"present","blobSha256":"same","sourceRef":key,"sourceVersion":key,"displayOrderState":"KNOWN"})
    }
    fn read(key: &str, text: &str, adoption: &str) -> Value {
        json!({"kind":"image","slotKey":key,"sourceVersion":key,"sourceState":"present","currentRevisionRef":key,"blocks":[{"kind":"paragraph","text":text}],"issues":[],"reading":{"adoption":adoption,"verification":"unchecked"}})
    }
    #[test]
    fn existing_cover_text_and_human_adoption_survive_duplicate_page_arrival() {
        let slots = vec![
            slot("cover", "cover", Value::Null),
            slot("first", "body_image", json!(1)),
            slot("repeat", "body_image", json!(3)),
        ];
        for readings in [
            vec![read("cover", "已完成全文", "automatic")],
            vec![
                read("cover", "人工补全全文", "human_edited"),
                read("first", "机器读本", "automatic"),
            ],
        ] {
            let result = collapse(slots.clone(), &readings);
            assert_eq!(result.len(), 2);
            let cover = result.iter().find(|s| s["slotKey"] == "cover").unwrap();
            assert_eq!(cover["page"], 1);
            assert_eq!(cover["sourceVersion"], "cover");
            assert_eq!(cover["pageSource"]["sourceRef"], "first");
            assert!(result.iter().any(|s| s["page"] == 3));
        }
    }
    #[test]
    fn contradictory_human_versions_require_resolution_without_hidden_selection() {
        let slots = vec![
            slot("cover", "cover", Value::Null),
            slot("first", "body_image", json!(1)),
        ];
        let readings = vec![
            read("cover", "人工版本一", "human_edited"),
            read("first", "人工版本二", "human_edited"),
        ];
        let result = collapse(slots, &readings);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|s| s["duplicateReadingConflict"] == true));
    }
}
