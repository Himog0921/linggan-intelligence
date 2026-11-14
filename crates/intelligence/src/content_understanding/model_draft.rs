//! The model sees snapshot-local text IDs. The stored Card always uses the canonical IDs.
use super::{Generation, Snapshot, accept_draft};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const PROTOCOL: &str = "content-understanding.draft.short-sources.v1";

pub fn material(snapshot: &Snapshot) -> Value {
    let mut material = json!(snapshot);
    if let Some(texts) = material["texts"].as_array_mut() {
        for (index, text) in texts.iter_mut().enumerate() {
            text["citationId"] = json!(format!("s{}", index + 1));
            if let Some(structure) = text["structure"].as_object_mut() {
                structure.remove("ref");
            }
        }
    }
    material
}

pub fn accept_model_draft(
    draft: Value,
    snapshot: &Snapshot,
    generation: &Generation,
) -> Result<Value, &'static str> {
    let result = accept_draft(draft, snapshot, generation, true)?;
    verify_literal_support(&result, snapshot)?;
    Ok(result)
}

fn verify_literal_support(result: &Value, snapshot: &Snapshot) -> Result<(), &'static str> {
    let card = &result["card"];
    let Some(evidence) = card["evidence"].as_array() else {
        return Err("invalid_draft");
    };
    let by_id: BTreeMap<_, _> = evidence
        .iter()
        .filter_map(|entry| Some((entry["id"].as_str()?, entry)))
        .collect();
    let source_kinds: BTreeMap<_, _> = snapshot
        .texts
        .iter()
        .map(|entry| (entry.citation_id.as_str(), entry.kind.as_str()))
        .collect();
    let Some(units) = card["core"]["knowledgeUnits"].as_array() else {
        return Err("invalid_draft");
    };
    let supported = |statement: &str, refs: &BTreeSet<&str>| {
        let referenced: Vec<_> = refs.iter().filter_map(|id| by_id.get(id)).collect();
        // Ordinary quotation marks can also flag a topic or a term.
        for marker in ["《", "原文", "原话"] {
            for (offset, _) in statement.match_indices(marker) {
                let tail = &statement[offset + marker.len()..];
                let (open, close, text, required_kind) = if marker == "《" {
                    ('《', '》', tail, None)
                } else {
                    let after_marker = tail.trim_start();
                    let after_marker = after_marker
                        .strip_prefix(['：', ':'])
                        .unwrap_or(after_marker)
                        .trim_start();
                    let Some(open) = after_marker.chars().next() else {
                        continue;
                    };
                    let close = match open {
                        '「' => '」',
                        '“' => '”',
                        _ => continue,
                    };
                    let context = statement[..offset].trim_end();
                    let required_kind = if context.ends_with("画面") {
                        Some("frame_text")
                    } else if context.ends_with("口播") {
                        Some("transcript")
                    } else if context.ends_with("图片") {
                        Some("image")
                    } else {
                        None
                    };
                    (open, close, &after_marker[open.len_utf8()..], required_kind)
                };
                let (literal, _) = text.split_once(close).ok_or("unsupported_literal")?;
                if literal.trim().is_empty() || literal.contains(open) {
                    return Err("unsupported_literal");
                }
                let verified = referenced.iter().any(|entry| {
                    entry["quote"]
                        .as_str()
                        .is_some_and(|quote| quote.contains(literal))
                        && required_kind.is_none_or(|kind| {
                            entry["inputCitationId"]
                                .as_str()
                                .and_then(|id| source_kinds.get(id))
                                == Some(&kind)
                        })
                });
                if verified {
                    continue;
                }
                let image_read = referenced.iter().any(|entry| {
                    entry["modelQuote"]
                        .as_str()
                        .is_some_and(|quote| quote.contains(literal))
                }) && (required_kind == Some("image")
                    || required_kind.is_none()
                        && !snapshot.texts.iter().any(|source| {
                            source.kind == "frame_text" && source.text.contains(literal)
                        }));
                if !image_read {
                    return Err("unsupported_literal");
                }
            }
        }
        Ok(())
    };
    let summary = &card["core"]["summary"];
    let summary_units: BTreeSet<_> = summary["unitIds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut summary_refs = BTreeSet::new();
    for unit in units {
        let refs: BTreeSet<_> = unit["evidenceRefs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        supported(unit["statement"].as_str().unwrap_or_default(), &refs)?;
        if unit["id"]
            .as_str()
            .is_some_and(|id| summary_units.contains(id))
        {
            summary_refs.extend(refs);
        }
    }
    supported(summary["text"].as_str().unwrap_or_default(), &summary_refs)
}
