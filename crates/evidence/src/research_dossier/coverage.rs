use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) fn project(materials: &[Value], slots: &[Value], expected: Option<u64>) -> Value {
    let images: Vec<_> = materials.iter().filter(|m| m["kind"] == "image").collect();
    let pages: Vec<_> = images
        .iter()
        .copied()
        .filter(|m| expected.is_none_or(|total| m["page"].as_u64().is_some_and(|p| p <= total)))
        .collect();
    let acquired = pages
        .iter()
        .filter(|m| m["sourceState"] == "present" || m["sourceState"] == "known_empty")
        .count();
    let processed = pages
        .iter()
        .filter(|m| m["currentRevisionRef"].is_string() && m["sourceState"] != "restricted")
        .count();
    let present: BTreeSet<_> = pages
        .iter()
        .filter(|m| m["sourceState"] == "present" || m["sourceState"] == "known_empty")
        .filter_map(|m| m["page"].as_u64())
        .collect();
    let missing: Vec<_> = expected
        .map(|n| (1..=n).filter(|p| !present.contains(p)).collect())
        .unwrap_or_default();
    json!({"imageInventory":{"basis":if expected.is_some(){"platform_image_list"}else{"observed_set"},"expectedPages":expected,
        "acquiredPages":acquired,"processedPages":processed,"missingPages":missing,
        "unknownOrder":slots.iter().any(|s|s["component"]=="image"&&s["displayOrderState"]!="KNOWN")},
        "sources":{"body":source(materials,"body"),"images":source(materials,"image"),"audio":source(materials,"audio"),"videoFrames":source(materials,"video_frame")},
        "issues":if expected.is_none(){vec![super::projection::issue("image_total_unknown","image_total_unknown","仅知道已观察到的图片，平台总页数未知","source_refresh")]}else{vec![]}})
}

fn source(materials: &[Value], kind: &str) -> Value {
    let units: Vec<_> = materials.iter().filter(|m| m["kind"] == kind).collect();
    let readings = units
        .iter()
        .filter(|m| m["currentRevisionRef"].is_string() && m["sourceState"] != "restricted")
        .count();
    let all = |state: &str| !units.is_empty() && units.iter().all(|m| m["sourceState"] == state);
    let state = if all("not_applicable") {
        "not_applicable"
    } else if all("restricted") {
        "restricted"
    } else if all("known_empty") {
        "known_empty"
    } else if units.is_empty() || all("not_acquired") {
        "not_acquired"
    } else if readings == units.len() {
        "available"
    } else if readings > 0 {
        "partial"
    } else {
        "not_processed"
    };
    json!({"state":state,"units":units.len(),"readings":readings})
}
