//! Safe explanations of the strict parser's decisions, never a second acceptance path.
use super::{ResearchPacket, bounded};
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
    /// Shared wire shape for provider constrained output and safe field diagnostics.
    /// Length, outcome consistency, identity and exact evidence remain server validations.
    pub fn output_schema() -> Value {
        let string = json!({"type":"string"});
        let nullable = json!({"type":["string","null"]});
        let evidence = array(object(json!({"quote":string})));
        object(json!({"comments":array(object(json!({
            "commentRef":string,"outcome":{"type":"string","enum":["interpretable","uncertain","no_signal"]},
            "labels":array(object(json!({"label":{"type":"string","enum":["need","solution","story","quote"]},"evidence":evidence}))),
            "problems":array(object(json!({"candidateRef":nullable,"equivalenceReason":string,"boundaryMatch":{"type":"boolean"},"name":string,"meaning":string,"evidence":evidence}))),
            "stances":array(object(json!({"target":string,"position":{"type":"string","enum":["support","oppose","concern","mixed"]},"evidence":evidence}))),
            "contextMissing":array(string.clone()),"uncertaintyReason":nullable,"limitations":array(string)
        })))}))
    }

    /// One safe explanation for each rejected comment, or one packet-level explanation.
    /// Acceptance always comes from `parse`; these diagnostics cannot make output valid.
    pub fn validation_diagnostics(&self, text: &str) -> Vec<Value> {
        let parsed = self.parse(text);
        let decoded = serde_json::from_str::<Value>(text);
        let json_failure = decoded.as_ref().err().map(|error| {
            json!({"type":"invalid_json","bytes":text.len(),"line":error.line(),"column":error.column(),
                "category":if error.is_eof() {"incomplete"} else {"syntax"}})
        });
        let envelope = decoded.ok();
        let results = match parsed {
            Err(code) => {
                let (path, expected, actual) = match (code, envelope.as_ref()) {
                    ("json_invalid", _) => (
                        "$".into(),
                        "返回完整 JSON 对象，不夹带说明文字或 Markdown".into(),
                        json_failure
                            .unwrap_or_else(|| json!({"type":"invalid_json","bytes":text.len()})),
                    ),
                    ("output_bounds", Some(v)) => (
                        "$.comments".into(),
                        "最多 100 条结果".into(),
                        summary(v.get("comments")),
                    ),
                    (_, Some(v)) => schema_issue(&Self::output_schema(), Some(v), "$")
                        .unwrap_or_else(|| {
                            (
                                "$".into(),
                                "仅包含 comments 的结果对象".into(),
                                summary(Some(v)),
                            )
                        }),
                    _ => ("$".into(), "完整的评论研究结果".into(), summary(None)),
                };
                return vec![diagnostic(None, code, &path, &expected, actual)];
            }
            Ok(results) => results,
        };
        let schema = Self::output_schema();
        let items = envelope
            .as_ref()
            .and_then(|v| v["comments"].as_array())
            .expect("parser accepted envelope");
        results
            .into_iter()
            .enumerate()
            .filter_map(|(index, result)| {
                let code = result.err()?;
                let id = format!("C{:03}", index + 1);
                let matches: Vec<_> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| item["commentRef"] == id)
                    .collect();
                if matches.len() != 1 {
                    return Some(diagnostic(
                        Some(&id),
                        code,
                        "$.comments",
                        "每条评论必须恰好对应一条结果",
                        json!({"type":"matches","count":matches.len()}),
                    ));
                }
                let (position, item) = matches[0];
                let base = format!("$.comments[{position}]");
                let issue = if code == "item_schema_invalid" {
                    schema_issue(
                        &schema["properties"]["comments"]["items"],
                        Some(item),
                        &base,
                    )
                } else {
                    self.business_issue(index, item, &base, code)
                };
                let (path, expected, actual) = issue.unwrap_or_else(|| {
                    (
                        base,
                        "字段合格，证据可定位到当前评论".into(),
                        summary(Some(item)),
                    )
                });
                Some(diagnostic(Some(&id), code, &path, &expected, actual))
            })
            .collect()
    }

    fn business_issue(
        &self,
        index: usize,
        item: &Value,
        base: &str,
        code: &str,
    ) -> Option<(String, String, Value)> {
        if code == "output_bounds" {
            for field in [
                "labels",
                "problems",
                "stances",
                "contextMissing",
                "limitations",
            ] {
                if item[field].as_array().is_some_and(|a| a.len() > 8) {
                    return Some((
                        format!("{base}.{field}"),
                        "最多 8 项".into(),
                        summary(Some(&item[field])),
                    ));
                }
            }
            for field in ["contextMissing", "limitations"] {
                for (n, value) in item[field].as_array()?.iter().enumerate() {
                    if !bounded(value.as_str()?, 200) {
                        return Some((
                            format!("{base}.{field}[{n}]"),
                            "1 至 200 个字符，不能只有空白".into(),
                            summary(Some(value)),
                        ));
                    }
                }
            }
            return Some((
                format!("{base}.uncertaintyReason"),
                "空值或 1 至 200 个非空字符".into(),
                summary(item.get("uncertaintyReason")),
            ));
        }
        if code == "uncertainty_reason_missing" {
            return Some((
                format!("{base}.uncertaintyReason"),
                "无法理解时必须填写原因".into(),
                summary(item.get("uncertaintyReason")),
            ));
        }
        if code == "outcome_conflict" || code == "interpretable_without_evidence" {
            return Some((
                format!("{base}.outcome"),
                if code == "outcome_conflict" {
                    "只有可解释的结果可以携带标签、问题或立场"
                } else {
                    "可解释的结果至少包含一个有证据的标签、问题或立场"
                }
                .into(),
                json!({"type":"object","labels":item["labels"].as_array()?.len(),"problems":item["problems"].as_array()?.len(),"stances":item["stances"].as_array()?.len()}),
            ));
        }
        for group in ["labels", "problems", "stances"] {
            for (n, entry) in item[group].as_array()?.iter().enumerate() {
                let path = format!("{base}.{group}[{n}]");
                if code == "problem_bounds" && group == "problems" {
                    for (field, max, blank_allowed) in [
                        ("name", 100, false),
                        ("meaning", 200, false),
                        ("equivalenceReason", 200, true),
                    ] {
                        let s = entry[field].as_str()?;
                        if s.chars().count() > max || (!blank_allowed && s.trim().is_empty()) {
                            return Some((
                                format!("{path}.{field}"),
                                format!("{} 至 {max} 个字符", if blank_allowed { 0 } else { 1 }),
                                summary(Some(&entry[field])),
                            ));
                        }
                    }
                }
                if code == "stance_bounds"
                    && group == "stances"
                    && !bounded(entry["target"].as_str()?, 100)
                {
                    return Some((
                        format!("{path}.target"),
                        "1 至 100 个字符，不能只有空白".into(),
                        summary(Some(&entry["target"])),
                    ));
                }
                let quotes = entry["evidence"].as_array()?;
                if code == "evidence_bounds" && (quotes.is_empty() || quotes.len() > 4) {
                    return Some((
                        format!("{path}.evidence"),
                        "提供 1 至 4 条证据引用".into(),
                        summary(Some(&entry["evidence"])),
                    ));
                }
                for (q, quote) in quotes.iter().enumerate() {
                    let quoted = quote["quote"].as_str()?;
                    if self.cleaned[index]
                        .resolve(quoted, &self.inputs[index].body)
                        .err()
                        == Some(code)
                    {
                        return Some((
                            format!("{path}.evidence[{q}].quote"),
                            "引用须精确、唯一地对应本条原声，且不包含被遮盖的文字".into(),
                            summary(Some(&quote["quote"])),
                        ));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment_packet::synthetic_packet;
    fn item(packet: &ResearchPacket) -> Value {
        json!({"commentRef":"C001","outcome":"interpretable","labels":[{"label":"need","evidence":[{"quote":packet.cleaned[0].text}]}],"problems":[],"stances":[],"contextMissing":[],"uncertaintyReason":null,"limitations":[]})
    }
    fn check(packet: &ResearchPacket, value: Value, code: &str, path: &str) -> Value {
        let text = json!({"comments":[value]}).to_string();
        assert_eq!(packet.parse(&text).unwrap()[0], Err(code));
        let diagnostics = packet.validation_diagnostics(&text);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["code"], code);
        assert_eq!(diagnostics[0]["path"], path);
        diagnostics[0].clone()
    }
    #[test]
    fn schema_diagnostics_locate_missing_nested_types_without_echoes() {
        let packet = synthetic_packet();
        let mut value = item(&packet);
        value.as_object_mut().unwrap().remove("uncertaintyReason");
        check(
            &packet,
            value,
            "item_schema_invalid",
            "$.comments[0].uncertaintyReason",
        );
        let mut value = item(&packet);
        value["labels"][0]["evidence"][0]["quote"] =
            json!({"PRIVATE-SYNTHETIC-KEY":"PRIVATE-SYNTHETIC-VALUE"});
        let d = check(
            &packet,
            value,
            "item_schema_invalid",
            "$.comments[0].labels[0].evidence[0].quote",
        );
        assert!(!d.to_string().contains("PRIVATE"));
        let mut value = item(&packet);
        value["labels"][0]["PRIVATE-SYNTHETIC-KEY"] = json!("PRIVATE-SYNTHETIC-VALUE");
        let d = check(
            &packet,
            value,
            "item_schema_invalid",
            "$.comments[0].labels[0]",
        );
        assert!(!d.to_string().contains("PRIVATE"));
        assert_eq!(d["actual"]["unexpectedFields"], 1);
    }
    #[test]
    fn evidence_and_business_failures_point_to_the_responsible_field() {
        let packet = synthetic_packet();
        let mut value = item(&packet);
        value["labels"][0]["evidence"] = json!([]);
        check(
            &packet,
            value,
            "evidence_bounds",
            "$.comments[0].labels[0].evidence",
        );
        let mut value = item(&packet);
        value["labels"][0]["evidence"][0]["quote"] = json!("不存在的合成原话");
        check(
            &packet,
            value,
            "quote_missing_or_ambiguous",
            "$.comments[0].labels[0].evidence[0].quote",
        );
        let mut value = item(&packet);
        value["labels"] = json!([]);
        check(
            &packet,
            value,
            "interpretable_without_evidence",
            "$.comments[0].outcome",
        );
        let mut value = item(&packet);
        value["limitations"] = json!([""]);
        check(
            &packet,
            value,
            "output_bounds",
            "$.comments[0].limitations[0]",
        );
    }
    #[test]
    fn independent_successes_are_omitted_and_envelope_is_never_repaired() {
        let mut packet = synthetic_packet();
        let good = item(&packet);
        assert!(
            packet
                .validation_diagnostics(&json!({"comments":[good.clone()]}).to_string())
                .is_empty()
        );
        packet
            .inputs
            .push(crate::model_invocation::synthetic_input());
        packet.cleaned.push(packet.cleaned[0].clone());
        let mut bad = good.clone();
        bad["commentRef"] = json!("C002");
        bad["labels"][0]["label"] = json!("not-a-label");
        let d = packet.validation_diagnostics(&json!({"comments":[good,bad]}).to_string());
        assert_eq!(d.len(), 1);
        assert_eq!(d[0]["commentRef"], "C002");
        let d = packet.validation_diagnostics("```json\n{\"comments\":[]}\n```");
        assert_eq!(d[0]["code"], "json_invalid");
        assert!(d[0]["commentRef"].is_null());
        let incomplete = packet.validation_diagnostics("{\"comments\":");
        assert_eq!(incomplete[0]["code"], "json_invalid");
        assert_eq!(incomplete[0]["actual"]["category"], "incomplete");
        assert!(incomplete[0]["actual"]["column"].is_number());
    }
    #[test]
    fn wire_schema_matches_required_comment_shape_and_stays_within_prompt_budget() {
        let packet = synthetic_packet();
        let schema = ResearchPacket::output_schema();
        assert!(schema.to_string().len() < 6144);
        let value = json!({"comments":[item(&packet)]});
        assert!(schema_issue(&schema, Some(&value), "$").is_none());
        let properties = schema["properties"]["comments"]["items"]["properties"]
            .as_object()
            .unwrap();
        assert_eq!(properties.len(), 8);
        for field in properties.keys() {
            let mut value = item(&packet);
            value.as_object_mut().unwrap().remove(field);
            let raw = json!({"comments":[value]}).to_string();
            assert!(packet.parse(&raw).unwrap()[0].is_err(), "required {field}");
            assert!(!packet.validation_diagnostics(&raw).is_empty());
        }
        let mut value = item(&packet);
        value["problems"] = json!([{"equivalenceReason":"","boundaryMatch":false,"name":"合成问题","meaning":"合成含义","evidence":[{"quote":packet.cleaned[0].text}]}]);
        check(
            &packet,
            value,
            "item_schema_invalid",
            "$.comments[0].problems[0].candidateRef",
        );
    }
}
