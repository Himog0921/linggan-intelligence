//! Deterministic transport fixture for `comment-understanding.v1`, not an analysis model or a
//! quality oracle. The comment text decides the reply, so a test controls it by what it writes:
//! "提醒" and "磨蹭" are two different needs, "MISSING" is left out of the reply, and "BADEVIDENCE"
//! quotes text that is not in the comment.
use serde_json::{Value, json};

fn assignment(catalog: &[Value], needle: &str, new_key: &str) -> Value {
    match catalog.iter().find(|entry| {
        entry["label"]
            .as_str()
            .is_some_and(|label| label.contains(needle))
    }) {
        Some(entry) => json!({"ref": entry["ref"], "newKey": null}),
        None => json!({"ref": null, "newKey": new_key}),
    }
}

pub fn response(request: &Value) -> Value {
    let catalog = request["catalog"].as_array().cloned().unwrap_or_default();
    let targets: Vec<Value> = request["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|target| !target["researchText"].as_str().unwrap_or("").contains("MISSING"))
        .map(|target| {
            let text = target["researchText"].as_str().unwrap_or("");
            let signals = if text.contains("BADEVIDENCE") {
                vec![json!({"kind":"belief","proposition":"评论者发表了看法","evidence":"原文里并没有这句话","problem":null})]
            } else if text.contains("提醒") {
                vec![json!({"kind":"need","proposition":"评论者的孩子需要有人提醒","evidence":"提醒","problem":assignment(&catalog,"提醒","n1")})]
            } else if text.contains("磨蹭") {
                vec![json!({"kind":"problem","proposition":"评论者的孩子写作业磨蹭","evidence":"磨蹭","problem":assignment(&catalog,"磨蹭","n2")})]
            } else {
                vec![]
            };
            json!({
                "targetKey": target["targetKey"],
                "outcome": if signals.is_empty() { "no_signal" } else { "signals" },
                "reason": if signals.is_empty() { json!("没有可提取的内容") } else { Value::Null },
                "signals": signals,
            })
        })
        .collect();
    json!({
        "contract": "comment-understanding.v1",
        "newProblems": [
            {"newKey":"n1","label":"孩子需要每天有人提醒","actor":"家长","goal":"孩子能自己开始做事","barrier":"需要每天有人提醒才会开始","context":null},
            {"newKey":"n2","label":"孩子写作业磨蹭","actor":"家长","goal":"孩子能按时写完作业","barrier":"写作业时拖拖拉拉","context":null},
        ],
        "targets": targets,
    })
}
