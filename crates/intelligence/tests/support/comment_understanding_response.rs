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
            } else if text.contains("想领学习记录表") {
                vec![json!({"kind":"need","proposition":"评论者想领取学习记录表","evidence":"想领学习记录表","problem":assignment(&catalog,"学习记录表","n4")})]
            } else if text.contains("催促") {
                vec![json!({"kind":"need","proposition":"评论者的孩子需要催促才开始","evidence":"催促","problem":assignment(&catalog,"催促","n3")})]
            } else if text.contains("提醒") {
                vec![json!({"kind":"need","proposition":"评论者的孩子需要有人提醒","evidence":"提醒","problem":assignment(&catalog,"提醒","n1")})]
            } else if text.contains("磨蹭") {
                vec![json!({"kind":"problem","proposition":"评论者的孩子写作业磨蹭","evidence":"磨蹭","problem":assignment(&catalog,"磨蹭","n2")})]
            } else {
                vec![]
            };
            let reason = match (signals.is_empty(), text.contains("合成咨询入口")) {
                (true, true) => json!("在邀请其他读者联系助理"),
                (true, false) => json!("没有可提取的内容"),
                (false, _) => Value::Null,
            };
            let mut reply = json!({
                "targetKey": target["targetKey"],
                "outcome": if signals.is_empty() { "no_signal" } else { "signals" },
                "reason": reason,
                "reasonCode": if signals.is_empty() && text.contains("合成咨询入口") { json!("promotion") } else { Value::Null },
                "signals": signals,
            });
            if text.contains("旧式引流夹具") || text.contains("显式空值夹具") {
                reply["reason"] = json!("引流话术");
            }
            if text.contains("旧式引流夹具") {
                reply.as_object_mut().unwrap().remove("reasonCode");
            }
            reply
        })
        .collect();
    json!({
        "contract": "comment-understanding.v1",
        "newProblems": [
            {"newKey":"n1","label":"孩子需要每天有人提醒","actor":"家长","goal":"孩子能自己开始做事","barrier":"需要每天有人提醒才会开始","context":null},
            {"newKey":"n2","label":"孩子写作业磨蹭","actor":"家长","goal":"孩子能按时写完作业","barrier":"写作业时拖拖拉拉","context":null},
            {"newKey":"n3","label":"孩子需要家长催促","actor":"家长","goal":"孩子能自己开始做事","barrier":"需要家长催促才会开始","context":null},
            {"newKey":"n4","label":"获取学习记录表","actor":"资料使用者","goal":"获取学习记录表","barrier":"尚未拿到学习记录表","context":null},
        ],
        "targets": targets,
    })
}

pub fn consolidation(request: &Value) -> Value {
    let catalog = request["catalog"]
        .as_array()
        .expect("consolidation catalog");
    let mut pairs = Vec::new();
    for (index, left) in catalog.iter().enumerate() {
        for (right_index, right) in catalog.iter().enumerate().skip(index + 1) {
            let labels = [
                left["label"].as_str().unwrap(),
                right["label"].as_str().unwrap(),
            ];
            let matches = |term| labels.iter().any(|label| label.contains(term));
            let verdict = if matches("提醒") && matches("催促") {
                "same"
            } else if matches("提醒") {
                "different"
            } else {
                "narrower"
            };
            pairs.push(json!({
                "pairKey":format!("q{}-{}", index + 1, right_index + 1),
                "verdict":verdict,"reason":"合成夹具按需求标签判定",
            }));
        }
    }
    json!({"contract":"comment-problem-consolidation.v1","pairs":pairs})
}
