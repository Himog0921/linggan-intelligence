use serde_json::{Value, json};
fn obj(fields: Vec<(&str, Value)>) -> Value {
    let required: Vec<_> = fields.iter().map(|(k, _)| *k).collect();
    let properties: serde_json::Map<_, _> = fields
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    json!({"type":"object","additionalProperties":false,"required":required,"properties":properties})
}
fn string() -> Value {
    json!({"type":"string","minLength":1,"maxLength":12000})
}
fn optional(v: Value) -> Value {
    json!({"anyOf":[v,{"type":"null"}]})
}
fn array(v: Value) -> Value {
    json!({"type":"array","items":v,"maxItems":64})
}
fn nonempty_array(v: Value) -> Value {
    let mut schema = array(v);
    schema["minItems"] = json!(1);
    schema
}
fn described(mut schema: Value, description: &str) -> Value {
    schema["description"] = json!(description);
    schema
}
fn choices(v: &[&str]) -> Value {
    json!({"type":"string","enum":v})
}
fn statement_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("id", string()),
        ("text", string()),
        (
            "kind",
            choices(&["observation", "author_claim", "interpretation"]),
        ),
        ("evidenceRefs", nonempty_array(string())),
    ]
}
pub fn response_schema() -> Value {
    let statement = obj(statement_fields());
    let mut f = statement_fields();
    f.extend([
        ("label", string()),
        (
            "category",
            choices(&["answer_form", "expression", "participation"]),
        ),
    ]);
    obj(vec![
        ("contract", choices(&[super::CONTRACT])),
        (
            "summary",
            described(
                string(),
                "压缩summaryBasisIds所指判断，保留主体、对象、程度、说话者和未知，不另作分析。",
            ),
        ),
        (
            "summaryBasisIds",
            described(
                nonempty_array(string()),
                "仅引用understanding、features、feedback中的判断ID。",
            ),
        ),
        (
            "understanding",
            obj(vec![
                ("discussion", statement.clone()),
                (
                    "audience",
                    described(
                        optional(statement.clone()),
                        "内容面向谁；有依据推断标interpretation，无依据填null；评论者姿态归反馈。",
                    ),
                ),
                ("situation", optional(statement.clone())),
                ("answer", statement.clone()),
                (
                    "offeredValue",
                    described(
                        optional(statement.clone()),
                        "材料提供的规则、步骤、例子等内容提供物；读者可能的理解或参与收益写入explanations。",
                    ),
                ),
            ]),
        ),
        (
            "features",
            described(
                array(obj(f)),
                "记录材料中的表达与组织做法，观察条目不夹读者作用假设。",
            ),
        ),
        (
            "feedback",
            described(
                array(statement),
                "记录评论者或作者表达了什么，保留已确认的角色和身份范围；有人说X不等于X已被确认。",
            ),
        ),
        (
            "explanations",
            array(obj(vec![
                ("id", string()),
                (
                    "text",
                    described(
                        string(),
                        "具体内容特征与读者可能的理解、兴趣或参与作用之间的联系；假设所用的事实前提也须有据。",
                    ),
                ),
                ("kind", choices(&["interpretation"])),
                (
                    "featureIds",
                    described(
                        nonempty_array(string()),
                        "至少一个与该解释相关的内容特征ID。",
                    ),
                ),
                ("feedbackIds", array(string())),
                ("evidenceRefs", nonempty_array(string())),
                (
                    "support",
                    described(string(), "已确认的前提，以及该前提与作用假设的联系理由。"),
                ),
                (
                    "uncertainty",
                    described(
                        optional(string()),
                        "这个作用假设尚缺的支持，不用其他材料缺口代替。",
                    ),
                ),
            ])),
        ),
        (
            "tags",
            array(obj(vec![
                (
                    "label",
                    described(
                        string(),
                        "压缩所引基础判断，保留说话者、对象、程度及推断性质；不以未知状态占位。",
                    ),
                ),
                (
                    "dimension",
                    choices(&[
                        "discussion",
                        "audience",
                        "situation",
                        "offered_value",
                        "answer_form",
                        "expression",
                        "participation",
                        "response",
                    ]),
                ),
                (
                    "judgmentIds",
                    described(
                        nonempty_array(string()),
                        "仅引用understanding、features、feedback中的判断ID，不引用explanations或methodCandidates。",
                    ),
                ),
                (
                    "retrievalUse",
                    described(
                        string(),
                        "按依据中可见的表达说明筛选用途；用途未知时保留未知，不把回应写成已确认的参与行为。",
                    ),
                ),
            ])),
        ),
        (
            "methodCandidates",
            array(obj(vec![
                ("id", string()),
                (
                    "text",
                    described(
                        string(),
                        "可迁移的内容表达、内容比较、证据判断或选题方法；现实行为建议留在作者回答/反馈。",
                    ),
                ),
                ("kind", choices(&["reuse_idea"])),
                ("basisIds", nonempty_array(string())),
                ("conditions", string()),
                ("openQuestion", optional(string())),
            ])),
        ),
        ("limitations", array(string())),
        (
            "evidence",
            array(obj(vec![
                ("id", string()),
                ("inputCitationId", string()),
                (
                    "quote",
                    described(
                        string(),
                        "从指定输入逐字复制唯一匹配的短句，保留错别字、拼音替字及标点，不纠正或同义替换；相邻片段分别引用。",
                    ),
                ),
            ])),
        ),
    ])
}
