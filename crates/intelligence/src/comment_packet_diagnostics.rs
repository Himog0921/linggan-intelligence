//! Safe explanations of the strict parser's decisions, never a second acceptance path.
use super::{ResearchPacket, normalized_envelope};
use serde_json::{Value, json};

fn object(properties: Value) -> Value {
    let required: Vec<_> = properties
        .as_object()
        .expect("static schema")
        .keys()
        .cloned()
        .collect();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn summary(value: Option<&Value>) -> Value {
    match value {
        None => json!({"type":"missing"}),
        Some(Value::Null) => json!({"type":"null"}),
        Some(Value::Bool(_)) => json!({"type":"boolean"}),
        Some(Value::Number(_)) => json!({"type":"number"}),
        Some(Value::String(s)) => json!({"type":"string","characters":s.chars().count()}),
        Some(Value::Array(a)) => json!({"type":"array","count":a.len()}),
        Some(Value::Object(o)) => json!({"type":"object","fields":o.len()}),
    }
}
fn diagnostic(id: Option<&str>, code: &str, path: &str, expected: &str, actual: Value) -> Value {
    json!({"commentRef":id,"code":code,"path":path,"expected":expected,"actual":actual})
}

fn normalization_diagnostic(code: &str, text: &str) -> Value {
    let expected = match code {
        "json_fence_invalid" => "返回裸JSON，或全文唯一完整的```json fenced block",
        "duplicate_json_key" => "每个JSON对象键只能出现一次",
        _ => "返回完整JSON对象，不夹带Markdown",
    };
    diagnostic(
        None,
        code,
        "$",
        expected,
        json!({"type":"invalid_json","bytes":text.len()}),
    )
}

/// Traverse only server-owned property names. Unknown output keys may themselves contain
/// sensitive content, so their names and values must never enter a diagnostic path.
fn schema_issue(
    schema: &Value,
    value: Option<&Value>,
    path: &str,
) -> Option<(String, String, Value)> {
    let actual = summary(value);
    let Some(value) = value else {
        return Some((path.into(), "必须提供该字段".into(), actual));
    };
    let kind = actual["type"].as_str().unwrap_or("unknown");
    let accepted = schema["type"].as_str().is_some_and(|t| t == kind)
        || schema["type"]
            .as_array()
            .is_some_and(|ts| ts.iter().any(|t| t == kind));
    if !accepted {
        return Some((
            path.into(),
            format!("字段类型应为 {}", schema["type"]),
            actual,
        ));
    }
    if let Some(allowed) = schema["enum"].as_array() {
        if !allowed.contains(value) {
            return Some((
                path.into(),
                format!("取值须属于 {}", schema["enum"]),
                actual,
            ));
        }
    }
    if let Some(properties) = schema["properties"].as_object() {
        for (name, spec) in properties {
            if let Some(issue) = schema_issue(spec, value.get(name), &format!("{path}.{name}")) {
                return Some(issue);
            }
        }
        let unexpected = value
            .as_object()
            .map(|o| o.keys().filter(|k| !properties.contains_key(*k)).count())
            .unwrap_or(0);
        if unexpected > 0 {
            return Some((
                path.into(),
                "不允许增加约定以外的字段".into(),
                json!({"type":"object","unexpectedFields":unexpected}),
            ));
        }
    }
    if let Some(items) = value.as_array() {
        // Packet input is bounded to 100 items; diagnostics must stay bounded even for malformed output.
        for (index, item) in items.iter().take(100).enumerate() {
            if let Some(issue) =
                schema_issue(&schema["items"], Some(item), &format!("{path}[{index}]"))
            {
                return Some(issue);
            }
        }
    }
    None
}

impl ResearchPacket {
    /// Provider schema and server field diagnostics share one v4 contract.
    pub fn output_schema() -> Value {
        let string = json!({"type":"string"});
        let nullable = json!({"type":["string","null"]});
        let evidence = array(object(json!({"quote":string})));
        let basis = json!({"type":"string","enum":["explicit","context_resolved","uncertain"]});
        let context = array(object(json!({"fragmentRef":string,"quote":string})));
        object(json!({"comments":array(object(json!({
            "commentRef":string,"outcome":{"type":"string","enum":["interpretable","uncertain","no_signal"]},
            "labels":array(object(json!({"label":{"type":"string","enum":["need","solution","story","quote"]},"basis":basis,"contextEvidence":context,"evidence":evidence}))),
            "problems":array(object(json!({"name":string,"meaning":string,"basis":basis,"contextEvidence":context,"evidence":evidence}))),
            "stances":array(object(json!({"target":string,"position":{"type":"string","enum":["support","oppose","concern","mixed"]},"basis":basis,"contextEvidence":context,"evidence":evidence}))),
            "contextMissing":array(string.clone()),"uncertaintyReason":nullable,"limitations":array(string)
        })))}))
    }
    fn rejected_field_location<'a>(
        &self,
        index: usize,
        field: &'a Value,
        path: &str,
        code: &str,
    ) -> (String, &'a Value) {
        let mut detail_path = path.to_owned();
        let mut detail_value = field;
        if code.starts_with("quote_") {
            for (q, entry) in field["evidence"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                if entry["quote"].as_str().is_some_and(|text| {
                    self.cleaned[index]
                        .resolve(text, &self.inputs[index].body)
                        .err()
                        == Some(code)
                }) {
                    detail_path = format!("{path}.evidence[{q}].quote");
                    detail_value = &entry["quote"];
                    break;
                }
            }
        } else if code == "evidence_bounds" {
            detail_path = format!("{path}.evidence");
            detail_value = &field["evidence"];
        } else if code.starts_with("context_") {
            detail_path = format!("{path}.contextEvidence");
            detail_value = &field["contextEvidence"];
        } else if code == "stance_bounds" {
            detail_path = format!("{path}.target");
            detail_value = &field["target"];
        } else if code == "problem_bounds" {
            let key = if !field["name"]
                .as_str()
                .is_some_and(|s| super::bounded(s, 100))
            {
                "name"
            } else {
                "meaning"
            };
            detail_path = format!("{path}.{key}");
            detail_value = &field[key];
        }
        (detail_path, detail_value)
    }
    fn rejected_field_diagnostic(
        &self,
        index: usize,
        item: &Value,
        base: &str,
        rejected: &Value,
        schema: &Value,
    ) -> Value {
        // Paths here are generated by the parser, never interpolated from provider keys.
        let relative = rejected["path"].as_str().unwrap_or("field");
        let code = rejected["code"].as_str().unwrap_or("field_rejected");
        let (group, field_index) = relative
            .split_once('[')
            .map(|(g, n)| {
                (
                    g,
                    Some(n.trim_end_matches(']').parse::<usize>().unwrap_or(0)),
                )
            })
            .unwrap_or((relative, None));
        let path = format!("{base}.{relative}");
        let null = Value::Null;
        let field = match field_index {
            Some(field_index) => item
                .get(group)
                .and_then(Value::as_array)
                .and_then(|items| items.get(field_index))
                .unwrap_or(&null),
            None => item.get(group).unwrap_or(&null),
        };
        let issue = if code == "field_schema_invalid" {
            let property = &schema["properties"]["comments"]["items"]["properties"][group];
            let field_schema = if field_index.is_some() {
                &property["items"]
            } else {
                property
            };
            schema_issue(field_schema, Some(field), &path)
        } else {
            None
        };
        let (detail_path, detail_value) = self.rejected_field_location(index, field, &path, code);
        let (path, expected, actual) = issue.unwrap_or_else(|| {
            (
                detail_path,
                match code {
                    "context_evidence_required_or_conflicting" => {
                        "直接表达不得借用上下文；上下文消解必须提供辅助片段证据"
                    }
                    "context_fragment_unknown" => "引用本次实际提供给该评论的片段编号",
                    "context_quote_missing_or_ambiguous" => {
                        "辅助引用须精确唯一地命中提供的上下文片段"
                    }
                    "problem_bounds" => "问题名称1至100字，定义1至200字",
                    "stance_bounds" => "立场对象1至100字",
                    "uncertainty_reason_missing" => "不确定解释必须在uncertaintyReason说明原因",
                    "evidence_bounds" => "每项提供1至4条目标评论引用",
                    "auxiliary_text_bounds" => "说明字段须为1至200字",
                    "auxiliary_items_bounds" => "说明字段最多8项",
                    _ => "引用须精确唯一地对应目标评论，不能包含遮盖文字",
                }
                .into(),
                summary(Some(detail_value)),
            )
        });
        diagnostic(
            Some(&format!("C{:03}", index + 1)),
            code,
            &path,
            &expected,
            actual,
        )
    }
    pub fn validation_diagnostics(&self, text: &str) -> Vec<Value> {
        let envelope = match normalized_envelope(text) {
            Ok(normalized) => normalized.value,
            Err(code) => return vec![normalization_diagnostic(code, text)],
        };
        let parsed = self.parse_envelope(&envelope);
        let schema = Self::output_schema();
        let results = match parsed {
            Ok(results) => results,
            Err(code) => {
                let (path, expected, actual) = schema_issue(&schema, Some(&envelope), "$")
                    .unwrap_or_else(|| {
                        (
                            "$.comments".into(),
                            "每个输入评论恰好一条结果，禁止未知评论编号".into(),
                            summary(envelope.get("comments")),
                        )
                    });
                return vec![diagnostic(None, code, &path, &expected, actual)];
            }
        };
        let items = envelope["comments"].as_array().expect("accepted envelope");
        let mut diagnostics = Vec::new();
        for (index, result) in results.into_iter().enumerate() {
            let id = format!("C{:03}", index + 1);
            let matches: Vec<_> = items
                .iter()
                .enumerate()
                .filter(|(_, i)| i["commentRef"] == id)
                .collect();
            if matches.len() != 1 {
                diagnostics.push(diagnostic(
                    Some(&id),
                    result.err().unwrap_or("missing_comment"),
                    "$.comments",
                    "每条输入评论恰好对应一条结果",
                    json!({"type":"matches","count":matches.len()}),
                ));
                continue;
            }
            let (position, item) = matches[0];
            let base = format!("$.comments[{position}]");
            let evaluated = self.evaluate_item(index, item);
            if let Ok(value) = &evaluated {
                for rejected in value["semantic"]["rejectedFields"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let mut d =
                        self.rejected_field_diagnostic(index, item, &base, rejected, &schema);
                    d["acceptance"] = value["semantic"]["acceptance"].clone();
                    diagnostics.push(d);
                }
                if !value["semantic"]["rejectedFields"]
                    .as_array()
                    .is_none_or(Vec::is_empty)
                {
                    continue;
                }
            }
            if let Err(code) = result {
                let issue = schema_issue(
                    &schema["properties"]["comments"]["items"],
                    Some(item),
                    &base,
                );
                let (path, expected, actual) = issue.unwrap_or_else(|| {
                    (
                        base,
                        "结果状态、字段数量和证据须满足评论合同".into(),
                        summary(Some(item)),
                    )
                });
                diagnostics.push(diagnostic(Some(&id), code, &path, &expected, actual));
            }
        }
        diagnostics
    }
}
