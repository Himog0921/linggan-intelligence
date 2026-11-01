//! Byte identity can establish page equivalence, never erase a differing human decision.
use serde_json::{Value, json};

pub(super) fn collapse(slots: Vec<Value>, readings: &[Value]) -> Vec<Value> {
    let mut slots = collapse_cover(slots, readings);
    // Unknown page numbers do not prevent byte-proven duplicate static materials
    // from sharing a reading. Distinct known platform pages retain their positions.
    let hashes: std::collections::BTreeSet<String> = slots
        .iter()
        .filter(|s| {
            s["component"] == "image" && s["sourceState"] == "present" && s["page"].is_null()
        })
        .filter_map(|s| {
            s["blobSha256"]
                .as_str()
                .filter(|hash| !hash.is_empty())
                .map(str::to_owned)
        })
        .collect();
    for hash in hashes {
        let group: Vec<Value> = slots
            .iter()
            .filter(|s| {
                s["component"] == "image"
                    && s["sourceState"] == "present"
                    && s["page"].is_null()
                    && s["blobSha256"] == hash
            })
            .cloned()
            .collect();
        if group.len() < 2 {
            continue;
        }
        let available: Vec<_> = group
            .iter()
            .filter_map(|s| reading(s, readings).map(|r| (s, r)))
            .collect();
        let human: Vec<_> = available
            .iter()
            .copied()
            .filter(|(_, r)| r["reading"]["adoption"] != "automatic")
            .collect();
        let choices = if human.is_empty() { &available } else { &human };
        if choices
            .first()
            .is_some_and(|(_, first)| choices.iter().any(|(_, other)| !equivalent(first, other)))
        {
            for slot in &mut slots {
                if group.iter().any(|s| s["slotKey"] == slot["slotKey"])
                    && slot["component"] == "image"
                {
                    slot["duplicateReadingConflict"] = json!(true);
                }
            }
            continue;
        }
        let chosen = choices.first().map_or(&group[0], |(slot, _)| *slot);
        let mut aliases = chosen["sameBytesSources"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for slot in &group {
            if slot["slotKey"] == chosen["slotKey"] {
                continue;
            }
            aliases.push(json!({"slotKey":slot["slotKey"],"sourceVersion":slot["sourceVersion"],"purpose":slot["purpose"]}));
            aliases.extend(
                slot["sameBytesSources"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        let mut retained = chosen.clone();
        retained["sameBytesSources"] = json!(aliases);
        let mut inserted = false;
        slots = slots
            .into_iter()
            .filter_map(|slot| {
                let duplicate = slot["component"] == "image"
                    && group.iter().any(|s| s["slotKey"] == slot["slotKey"]);
                if !duplicate {
                    return Some(slot);
                }
                if inserted {
                    return None;
                }
                inserted = true;
                Some(retained.clone())
            })
            .collect();
    }
    slots
}

fn collapse_cover(mut slots: Vec<Value>, readings: &[Value]) -> Vec<Value> {
    let Some(first) = slots
        .iter()
        .find(|s| {
            s["component"] == "image"
                && s["purpose"] == "body_image"
                && (s["page"] == 1 || (s["page"].is_null() && s["producerOrdinal"] == 1))
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
    let preferred = if first["page"].is_null() {
        group
            .iter()
            .find(|s| s["purpose"] == "cover")
            .copied()
            .unwrap_or(&first)
    } else {
        &first
    };
    let chosen = choices
        .iter()
        .find(|(s, _)| s["slotKey"] == preferred["slotKey"])
        .or_else(|| choices.first())
        .map_or(preferred.clone(), |(s, _)| (*s).clone());
    let same_bytes_sources: Vec<_> = group
        .iter()
        .filter(|s| s["slotKey"] != chosen["slotKey"])
        .map(|s| json!({"slotKey":s["slotKey"],"sourceVersion":s["sourceVersion"],"purpose":s["purpose"]}))
        .collect();
    let keys: Vec<_> = group.iter().map(|s| s["slotKey"].clone()).collect();
    slots.retain(|s| {
        s["component"] != "image"
            || !keys.contains(&s["slotKey"])
            || s["slotKey"] == chosen["slotKey"]
    });
    if let Some(slot) = slots
        .iter_mut()
        .find(|s| s["slotKey"] == chosen["slotKey"] && s["component"] == "image")
    {
        // Only metadata crosses the proven cover/first-page equivalence; reading identities stay put.
        slot["sameBytesSources"] = json!(same_bytes_sources);
        if chosen["slotKey"] != first["slotKey"] && first["page"] == 1 {
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
            && (r["sourceVersion"] == slot["sourceVersion"]
                || (slot["blobSha256"]
                    .as_str()
                    .is_some_and(|hash| !hash.is_empty())
                    && r["sourceLocation"]["blobSha256"] == slot["blobSha256"]))
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
    fn first_slot() -> Value {
        let mut value = slot("first", "body_image", Value::Null);
        value["producerOrdinal"] = json!(1);
        value["displayOrderState"] = json!("UNKNOWN");
        value
    }
    #[test]
    fn observed_first_alias_collapses_without_inventing_platform_page() {
        let mut first = slot("first", "body_image", Value::Null);
        first["producerOrdinal"] = json!(1);
        first["displayOrderState"] = json!("UNKNOWN");
        let mut cover = slot("cover", "cover", Value::Null);
        cover["displayOrderState"] = json!("UNKNOWN");
        let result = collapse(
            vec![cover, first, slot("repeat", "body_image", json!(3))],
            &[
                read("cover", "同一份文字", "automatic"),
                read("first", "同一份文字", "automatic"),
            ],
        );
        assert_eq!(result.len(), 2);
        let cover = result.iter().find(|s| s["slotKey"] == "cover").unwrap();
        assert!(cover["page"].is_null());
        assert_eq!(cover["displayOrderState"], "UNKNOWN");
        assert_eq!(cover["sameBytesSources"][0]["slotKey"], "first");
        assert!(result.iter().any(|s| s["slotKey"] == "repeat"));
        let human = collapse(
            result.into_iter().chain([first_slot()]).collect(),
            &[
                read("cover", "自动标题", "automatic"),
                read("first", "人工标题", "human_edited"),
            ],
        );
        let first = human.iter().find(|s| s["slotKey"] == "first").unwrap();
        assert!(first["page"].is_null());
        assert_eq!(first["purpose"], "body_image");
        assert_eq!(first["sameBytesSources"][0]["purpose"], "cover");
    }
    #[test]
    fn repeated_static_bytes_share_content_but_motion_and_human_differences_survive() {
        let mut first = first_slot();
        first["producerOrdinal"] = json!(2);
        let mut live = slot("live", "live_photo", Value::Null);
        live["displayOrderState"] = json!("UNKNOWN");
        let mut motion = live.clone();
        motion["component"] = json!("motion");
        let slots = vec![first, live, motion];
        let result = collapse(slots.clone(), &[read("first", "真实文字", "automatic")]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["sameBytesSources"][0]["slotKey"], "live");
        assert_eq!(result[1]["component"], "motion");
        let conflicts = collapse(
            slots,
            &[
                read("first", "人工甲", "human_edited"),
                read("live", "人工乙", "human_edited"),
            ],
        );
        assert_eq!(conflicts.len(), 3);
        assert!(
            conflicts
                .iter()
                .filter(|s| s["component"] == "image")
                .all(|s| s["duplicateReadingConflict"] == true)
        );
    }
}
