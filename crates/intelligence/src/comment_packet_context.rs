//! Deterministic, bounded context excerpts. Metadata resolves excerpts back to their sources;
//! the model only receives excerpt handles and text, never unrelated complete documents.
use crate::comment_cleaning::{clean, outbound};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub const SELECTOR_VERSION: &str = "comment-context.lexical-excerpts.v1";
const WINDOW: usize = 180;
const MAX_TEXT: usize = 1200;
fn terms(text: &str) -> BTreeSet<String> {
    let chars: Vec<char> = text.chars().filter(|c| c.is_alphanumeric()).collect();
    chars.windows(2).map(|w| w.iter().collect()).collect()
}
struct Source<'a> {
    kind: &'a str,
    path: String,
    text: &'a str,
    reference: Value,
    limit: usize,
}
pub(super) fn fragments(context: &Value, comment: &str) -> Value {
    let mut sources = Vec::new();
    if let Some(text) = context.pointer("/work/title/value").and_then(Value::as_str) {
        sources.push(Source {
            kind: "title",
            path: "/work/title/value".into(),
            text,
            reference: context
                .pointer("/work/title/source")
                .cloned()
                .unwrap_or(Value::Null),
            limit: 1,
        });
    }
    if let Some(text) = context.pointer("/parent/body").and_then(Value::as_str) {
        sources.push(Source {
            kind: "parent",
            path: "/parent/body".into(),
            text,
            reference: context
                .pointer("/parent/sourceRef")
                .cloned()
                .unwrap_or(Value::Null),
            limit: 2,
        });
    }
    if let Some(text) = context.pointer("/work/body/value").and_then(Value::as_str) {
        sources.push(Source {
            kind: "body",
            path: "/work/body/value".into(),
            text,
            reference: context
                .pointer("/work/body/source")
                .cloned()
                .unwrap_or(Value::Null),
            limit: 2,
        });
    }
    for (i, d) in context["derivatives"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if d["state"] == "ACQUIRED" {
            if let Some(text) = d["displayText"].as_str() {
                sources.push(Source {
                    kind: d["kind"].as_str().unwrap_or("media"),
                    path: format!("/derivatives/{i}/displayText"),
                    text,
                    reference: d["jobRef"].clone(),
                    limit: 2,
                });
            }
        }
    }
    let query = terms(comment);
    let mut result = Vec::new();
    let mut used = 0;
    for source in sources {
        if used >= MAX_TEXT {
            break;
        }
        let cleaned = outbound(clean(source.text));
        let chars: Vec<char> = cleaned.text.chars().collect();
        let mut windows = Vec::new();
        for (n, chunk) in chars.chunks(WINDOW).enumerate() {
            let text: String = chunk.iter().collect();
            let score = terms(&text).intersection(&query).count();
            // Title and immediate parent are relational context. Body/media require lexical support;
            // absence is reported, never replaced with a generated summary.
            if score > 0 || matches!(source.kind, "title" | "parent") {
                windows.push((score, n, text));
            }
        }
        windows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        windows.truncate(source.limit);
        windows.sort_by_key(|w| w.1);
        for (_, window_index, mut text) in windows {
            text = text.chars().take(MAX_TEXT - used).collect();
            if text.is_empty() {
                break;
            }
            let clean_start = window_index * WINDOW;
            let clean_end = clean_start + text.chars().count();
            let (Some(first), Some(last)) = (
                cleaned.offsets.get(clean_start),
                cleaned.offsets.get(clean_end - 1),
            ) else {
                continue;
            };
            let (start, end) = (first.0, last.1);
            let original_span: String = source.text.chars().skip(start).take(end - start).collect();
            used += text.chars().count();
            result.push(json!({"fragmentRef":format!("F{:03}",result.len()+1),"kind":source.kind,"text":text,"cleanStartChar":clean_start,"sourcePath":source.path,"sourceRef":source.reference,"startChar":start,"endChar":end,"sourceSpanHash":crate::comment_research::comment_source_hash(&original_span)}));
        }
    }
    json!(result)
}
/// Location and text changes invalidate raw evidence coordinates. Re-observation identifiers
/// and engagement metadata do not change meaning; old evidence still requires readable guards.
pub(super) fn evidence_identity(context: &Value, comment: &str) -> Value {
    let mut selected = fragments(context, comment);
    for f in selected.as_array_mut().into_iter().flatten() {
        let stable = match f["kind"].as_str() {
            Some("parent") => context.pointer("/parent/stableIdentity"),
            Some("title" | "body") => context.get("workIdentity"),
            _ => None,
        }
        .filter(|v| !v.is_null());
        if let Some(stable) = stable {
            if let Some(object) = f.as_object_mut() {
                object.remove("sourceRef");
                object.insert("stableIdentity".into(), stable.clone());
            }
        }
    }
    selected
}
pub fn semantic_context_for_comment(context: &Value, comment: &str) -> Value {
    // Recompute from source context so a stale cached fragment cannot change acceptance/fingerprints.
    let fragments = fragments(context, comment);
    let mut available_kinds: Vec<String> = fragments
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["kind"].as_str().map(str::to_owned))
        .collect();
    available_kinds.sort();
    available_kinds.dedup();
    json!({"selectorVersion":SELECTOR_VERSION,"role":context.get("role").and_then(Value::as_str).unwrap_or("unknown"),"parentState":context["parentState"],"availableKinds":available_kinds,"sourceTruncation":{"body":context.pointer("/work/body/truncated"),"parent":context.pointer("/parent/bodyTruncated")},
      "limitations":["LEXICAL_EXCERPTS_MAY_MISS_RELEVANT_CONTEXT","MISSING_EXCERPTS_DO_NOT_PROVE_ABSENCE"],
      "fragments":fragments.as_array().into_iter().flatten().map(|f|json!({"fragmentRef":f["fragmentRef"],"kind":f["kind"],"text":f["text"]})).collect::<Vec<_>>()})
}
