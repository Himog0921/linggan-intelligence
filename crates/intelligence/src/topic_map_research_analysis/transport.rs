//! Lossless extraction wire projection. Stored evidence keeps its original identity.
use super::{EXTRACT_CONTRACT, ResearchOutput, output_schema};
use linggan_evidence::creator_discovery::Fragment;
use serde_json::{Value, json};

pub const COMPACT_EXTRACT_CONTRACT: &str = "topic-map.extract.compact.v1";

#[derive(Debug)]
pub struct CompactDecodeError {
    pub code: &'static str,
    pub path: String,
    pub rule: &'static str,
}

fn rejection(code: &'static str, path: &str, rule: &'static str) -> CompactDecodeError {
    CompactDecodeError {
        code,
        path: path.to_owned(),
        rule,
    }
}

fn wire_key(key: &str) -> &str {
    match key {
        "outcome" | "overlays" => "o",
        "discussions" | "definition" => "d",
        "scenes" | "statement" | "involvedStages" | "start" => "s",
        "journey" => "j",
        "responseMatches" => "rm",
        "angles" => "a",
        "productOpportunities" | "path" => "p",
        "limitations" | "label" => "l",
        "inclusionCriteria" => "i",
        "exclusionCriteria" => "x",
        "speakerRole" => "sp",
        "evidenceRole" => "er",
        "rationale" => "r",
        "topicRef" | "title" => "t",
        "evidence" | "end" => "e",
        "fragmentId" => "f",
        "mainStage" => "m",
        "status" => "st",
        "unanswered" => "u",
        "answerTask" => "q",
        "need" => "n",
        "hypothesis" => "h",
        "verificationQuestion" => "q",
        "alternativeExplanation" => "x",
        _ => key,
    }
}

fn compact_schema(schema: &Value) -> Value {
    let mut projected = schema.clone();
    if let Some(properties) = schema["properties"].as_object() {
        let fields: serde_json::Map<_, _> = properties
            .iter()
            .map(|(key, child)| {
                let child = if key == "fragmentId" {
                    json!({"type":"integer","minimum":0})
                } else {
                    compact_schema(child)
                };
                (wire_key(key).to_owned(), child)
            })
            .collect();
        projected["required"] = json!(fields.keys().collect::<Vec<_>>());
        projected["properties"] = json!(fields);
    }
    if schema["type"] == "array" {
        projected["items"] = compact_schema(&schema["items"]);
    }
    projected
}

/// Only wire keys and local citation indexes change; every business field remains required.
pub fn compact_output_schema() -> Value {
    let mut schema = compact_schema(&output_schema());
    schema["properties"]["contract"]["enum"] = json!([COMPACT_EXTRACT_CONTRACT]);
    schema
}

pub fn compact_field_names() -> Value {
    fn collect(schema: &Value, fields: &mut serde_json::Map<String, Value>) {
        for (key, child) in schema["properties"].as_object().into_iter().flatten() {
            fields.insert(key.clone(), json!(wire_key(key)));
            collect(child, fields);
        }
        if schema["type"] == "array" {
            collect(&schema["items"], fields);
        }
    }
    let mut fields = serde_json::Map::new();
    collect(&output_schema(), &mut fields);
    Value::Object(fields)
}

fn expand(
    value: &Value,
    schema: &Value,
    fragments: &[Fragment],
    path: &str,
) -> Result<Value, CompactDecodeError> {
    if let Some(properties) = schema["properties"].as_object() {
        let values = value
            .as_object()
            .ok_or_else(|| rejection("output_schema_invalid", path, "type"))?;
        if values
            .keys()
            .any(|key| !properties.keys().any(|expected| wire_key(expected) == key))
        {
            return Err(rejection("output_schema_invalid", path, "unknown_property"));
        }
        let mut restored = serde_json::Map::new();
        for (key, child) in properties {
            let field_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            let field = values.get(wire_key(key)).ok_or_else(|| {
                rejection("output_schema_invalid", &field_path, "required_property")
            })?;
            let field = if key == "fragmentId" {
                let index = field.as_u64().ok_or_else(|| {
                    rejection(
                        "invalid_citation_fragment_index",
                        &field_path,
                        "fragment_index",
                    )
                })?;
                let fragment = usize::try_from(index)
                    .ok()
                    .and_then(|index| fragments.get(index))
                    .ok_or_else(|| {
                        rejection(
                            "invalid_citation_fragment_index",
                            &field_path,
                            "fragment_index",
                        )
                    })?;
                json!(fragment.fragment_id)
            } else {
                expand(field, child, fragments, &field_path)?
            };
            restored.insert(key.clone(), field);
        }
        return Ok(Value::Object(restored));
    }
    if schema["type"] == "array" {
        let items = value
            .as_array()
            .ok_or_else(|| rejection("output_schema_invalid", path, "type"))?;
        for (bound, rule, violated) in [
            ("minItems", "min_items", true),
            ("maxItems", "max_items", false),
        ] {
            if let Some(limit) = schema[bound].as_u64()
                && if violated {
                    (items.len() as u64) < limit
                } else {
                    (items.len() as u64) > limit
                }
            {
                return Err(rejection("output_schema_invalid", path, rule));
            }
        }
        return items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                expand(
                    item,
                    &schema["items"],
                    fragments,
                    &format!("{path}[{index}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    let valid_type = match schema["type"].as_str() {
        Some("string") => value.is_string(),
        Some("integer") => value.is_i64() || value.is_u64(),
        Some("null") => value.is_null(),
        _ => false,
    };
    if !valid_type {
        return Err(rejection("output_schema_invalid", path, "type"));
    }
    if let Some(choices) = schema["enum"].as_array()
        && !choices.contains(value)
    {
        return Err(rejection("output_schema_invalid", path, "enum"));
    }
    if let Some(text) = value.as_str() {
        for (bound, rule, lower) in [
            ("minLength", "min_length", true),
            ("maxLength", "max_length", false),
        ] {
            if let Some(limit) = schema[bound].as_u64()
                && if lower {
                    (text.chars().count() as u64) < limit
                } else {
                    (text.chars().count() as u64) > limit
                }
            {
                return Err(rejection("output_schema_invalid", path, rule));
            }
        }
    }
    if let Some(limit) = schema["minimum"].as_i64()
        && value.as_i64().is_some_and(|number| number < limit)
    {
        return Err(rejection("output_schema_invalid", path, "minimum"));
    }
    Ok(value.clone())
}

/// Strict transport decoding; no JSON repair, missing-field defaults, or citation guessing.
/// Callers must then run the ordinary semantic/source/context validators on the restored output.
pub fn decode_compact_output(
    text: &str,
    fragments: &[Fragment],
) -> Result<ResearchOutput, CompactDecodeError> {
    let wire: Value =
        serde_json::from_str(text).map_err(|_| rejection("output_json_invalid", "", "json"))?;
    if wire["contract"] != COMPACT_EXTRACT_CONTRACT {
        return Err(rejection(
            "output_contract_mismatch",
            "contract",
            "contract",
        ));
    }
    let mut schema = output_schema();
    schema["properties"]["contract"]["enum"] = json!([COMPACT_EXTRACT_CONTRACT]);
    let mut output = expand(&wire, &schema, fragments, "")?;
    output["contract"] = json!(EXTRACT_CONTRACT);
    serde_json::from_value(output).map_err(|_| rejection("output_schema_invalid", "", "type"))
}

#[cfg(test)]
mod tests;
