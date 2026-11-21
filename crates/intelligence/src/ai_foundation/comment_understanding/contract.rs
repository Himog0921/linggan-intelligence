//! `comment-understanding.v1`: the model-facing request payload and response schema of a
//! research unit. Aliases (`p1…` catalog entries, `t1…` targets) keep UUIDs out of the model's
//! reach; callers map them back from the order frozen at dispatch.
use crate::comment_study_semantic::SignalKind;
use serde_json::{Value, json};

pub const CONTRACT: &str = "comment-understanding.v1";
/// Stored per-target result contract. It keeps every field Topic Map and the catalog already
/// read from `comment-study.semantic.v1` and adds `problemRef`.
pub const RESULT_CONTRACT: &str = "comment-study.semantic.v2";
pub const MAX_SIGNALS_PER_TARGET: usize = 12;

/// Hard bound on a new Problem label. The prompt asks for 20 characters or fewer.
pub(super) const LABEL_MAX_CHARS: usize = 30;
pub(super) const FIELD_MAX_CHARS: usize = 500;
pub(super) const TEXT_MAX_CHARS: usize = 1000;
pub(super) const REASON_MAX_CHARS: usize = 500;
pub(super) const KEY_MAX_CHARS: usize = 20;

pub fn catalog_alias(index: usize) -> String {
    format!("p{}", index + 1)
}

pub fn target_alias(index: usize) -> String {
    format!("t{}", index + 1)
}

/// One catalog Problem as shown to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub alias: String,
    pub label: String,
    pub actor: Option<String>,
    pub goal: String,
    pub barrier: String,
    pub context: Option<String>,
}

/// One target comment as shown to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetInput {
    pub alias: String,
    pub research_text: String,
    /// `{"state":"none"|"available"|"unavailable", "text": …}`; context only, never evidence.
    pub parent_context: Value,
}

const RESPONSE_REQUIREMENTS: &str = r#"严格输出 JSON。每个目标填写 reasonCode。仅当评论主动宣传并引导他人联系、咨询或购买，且 outcome 为 no_signal 时填写 "promotion"。其余情况填写 null。提到推广、询问链接、索取资料或按提示只回复一个口令，本身都不构成引流。
每条 signal 只写一个能由 evidence 完整支撑的命题。evidence 须逐字来自本条 researchText 的唯一连续片段。证据只支撑前半句时，删除后半句，或把有自己证据的内容拆成另一条。父评论和作品只解释指代。自述的诊断、疗效或因果写成评论者的说法。
询问查看位置等事实是 question。明确请求资源、帮助或协作是 need。目标和障碍都清楚才用 problem。只回复口令时，依据该评论能确定的意图判断；不能从作品的邀请自行推断具体个人需求。
problem 和 need 的 problem 归属只能选择其一：已有目录用 {"ref":"p1","newKey":null}；新需求用 {"ref":null,"newKey":"n1"} 并在 newProblems 中为 n1 写且只写一份完整定义；信息不足时用 null，保留信号。不得省略所引用的新定义，不得同时填写两个引用。定稿前核对所有引用。其他信号的 problem 写 null。
比如不同评论明确请求同一份学习记录表，且资源缺口相同，应共用一个定义；障碍可以写“未拿到这份学习记录表”。不要替评论者编造额外障碍。定义只写可复用的主体、目标和障碍，具体年级或场景只有会改变障碍时才进入定义。"#;

/// The request payload. The task's editable instruction text is added by the executor.
pub fn payload(work_context: &Value, catalog: &[CatalogEntry], targets: &[TargetInput]) -> Value {
    json!({
        "contract": CONTRACT,
        "responseRequirements": RESPONSE_REQUIREMENTS,
        "workContext": work_context,
        "catalog": catalog.iter().map(|e| json!({
            "ref": e.alias, "label": e.label, "actor": e.actor,
            "goal": e.goal, "barrier": e.barrier, "context": e.context,
        })).collect::<Vec<_>>(),
        "targets": targets.iter().map(|t| json!({
            "targetKey": t.alias, "researchText": t.research_text, "parentContext": t.parent_context,
        })).collect::<Vec<_>>(),
    })
}

/// Trimmed text of 1..=max characters.
pub(super) fn trimmed(value: &str, max: usize) -> Option<String> {
    let text = value.trim();
    (!text.is_empty() && text.chars().count() <= max).then(|| text.to_owned())
}

/// Blank optional text is absent; text over the bound is a defect (`Err`).
pub(super) fn optional_text(value: Option<&str>, max: usize) -> Result<Option<String>, ()> {
    match value.map(str::trim).filter(|text| !text.is_empty()) {
        None => Ok(None),
        Some(text) if text.chars().count() <= max => Ok(Some(text.to_owned())),
        Some(_) => Err(()),
    }
}

fn object(properties: Value) -> Value {
    let required: Vec<String> = properties.as_object().unwrap().keys().cloned().collect();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}

fn nullable_text() -> Value {
    json!({"type":["string","null"]})
}

pub fn response_schema() -> Value {
    let kinds = [
        SignalKind::Problem,
        SignalKind::Need,
        SignalKind::Belief,
        SignalKind::Emotion,
        SignalKind::Experience,
        SignalKind::Solution,
        SignalKind::Quote,
        SignalKind::Context,
        SignalKind::Question,
    ]
    .map(|kind| kind.as_str());
    // Exactly one of ref/newKey is non-null; the acceptor enforces it, because provider-side
    // structured outputs are not relied on for anyOf.
    let mut problem = object(json!({"ref":nullable_text(),"newKey":nullable_text()}));
    problem["type"] = json!(["object", "null"]);
    let signal = object(json!({
        "kind": {"type":"string","enum":kinds},
        "proposition": {"type":"string"},
        "evidence": {"type":"string"},
        "problem": problem,
    }));
    let target = object(json!({
        "targetKey": {"type":"string"},
        "outcome": {"type":"string","enum":["signals","no_signal","needs_context"]},
        "reason": nullable_text(),
        "reasonCode": {"type":["string","null"],"enum":["promotion",null]},
        "signals": {"type":"array","maxItems":MAX_SIGNALS_PER_TARGET,"items":signal},
    }));
    let new_problem = object(json!({
        "newKey": {"type":"string"},
        "label": {"type":"string","maxLength":LABEL_MAX_CHARS},
        "actor": nullable_text(),
        "goal": {"type":"string"},
        "barrier": {"type":"string"},
        "context": nullable_text(),
    }));
    object(json!({
        "contract": {"type":"string","enum":[CONTRACT]},
        "newProblems": {"type":"array","items":new_problem},
        "targets": {"type":"array","items":target},
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_names_catalog_and_targets_by_alias_only() {
        let catalog = [CatalogEntry {
            alias: catalog_alias(0),
            label: "孩子写作业启动困难".into(),
            actor: Some("小学生家长".into()),
            goal: "孩子按时开始写作业".into(),
            barrier: "需要反复催促才动笔".into(),
            context: None,
        }];
        let targets = [TargetInput {
            alias: target_alias(1),
            research_text: "我家娃也是".into(),
            parent_context: json!({"state":"none"}),
        }];
        let value = payload(&json!({"summary":"x"}), &catalog, &targets);
        assert_eq!(value["catalog"][0]["ref"], "p1");
        assert_eq!(value["targets"][0]["targetKey"], "t2");
        assert_eq!(value["catalog"][0]["context"], Value::Null);
        assert_eq!(value["contract"], CONTRACT);
    }

    #[test]
    fn schema_lists_the_nine_kinds_and_requires_every_field() {
        let schema = response_schema();
        let signal = &schema["properties"]["targets"]["items"]["properties"]["signals"]["items"];
        assert_eq!(
            signal["properties"]["kind"]["enum"]
                .as_array()
                .unwrap()
                .len(),
            9
        );
        assert_eq!(signal["required"].as_array().unwrap().len(), 4);
        assert_eq!(schema["required"].as_array().unwrap().len(), 3);
        assert_eq!(
            signal["properties"]["problem"]["type"],
            json!(["object", "null"])
        );
    }
}
