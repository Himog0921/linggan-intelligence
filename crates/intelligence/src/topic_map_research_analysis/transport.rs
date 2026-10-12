//! Lossless extraction wire projection. Stored evidence keeps its original identity.
use super::{EXTRACT_CONTRACT, ResearchOutput, output_schema};
use linggan_evidence::creator_discovery::Fragment;
use serde_json::{Value, json};

pub const COMPACT_EXTRACT_CONTRACT: &str = "topic-map.extract.quotes.v1";

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
        "scenes" | "statement" | "involvedStages" => "s",
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
        "evidence" => "e",
        "fragmentId" => "f",
        "mainStage" => "m",
        "status" => "st",
        "unanswered" => "u",
        "answerTask" | "quote" => "q",
        "need" => "n",
        "hypothesis" => "h",
        "verificationQuestion" => "q",
        "alternativeExplanation" => "x",
        _ => key,
    }
}

fn citation_schema(schema: &Value) -> bool {
    schema["properties"].as_object().is_some_and(|fields| {
        fields.len() == 3
            && ["fragmentId", "start", "end"]
                .iter()
                .all(|key| fields.contains_key(*key))
    })
}

fn compact_schema(schema: &Value) -> Value {
    if citation_schema(schema) {
        return json!({"type":"object","properties":{
            "f":{"type":"integer","minimum":0},
            "q":{"type":"string","minLength":1,"maxLength":1000}},
            "required":["f","q"],"additionalProperties":false});
    }
    let mut projected = schema.clone();
    if let Some(properties) = schema["properties"].as_object() {
        let fields: serde_json::Map<_, _> = properties
            .iter()
            .map(|(key, child)| (wire_key(key).to_owned(), compact_schema(child)))
            .collect();
        projected["required"] = json!(fields.keys().collect::<Vec<_>>());
        projected["properties"] = json!(fields);
    }
    if schema["type"] == "array" {
        projected["items"] = compact_schema(&schema["items"]);
    }
    projected
}

/// Every business field remains required; evidence quotes replace model-produced offsets.
pub fn compact_output_schema() -> Value {
    let mut schema = compact_schema(&output_schema());
    schema["properties"]["contract"]["enum"] = json!([COMPACT_EXTRACT_CONTRACT]);
    schema
}

pub fn compact_field_names() -> Value {
    fn collect(schema: &Value, fields: &mut serde_json::Map<String, Value>) {
        if citation_schema(schema) {
            fields.insert("fragmentId".into(), json!("f"));
            fields.insert("quote".into(), json!("q"));
            return;
        }
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

fn expand_citation(
    value: &Value,
    fragments: &[Fragment],
    path: &str,
) -> Result<Value, CompactDecodeError> {
    let values = value
        .as_object()
        .ok_or_else(|| rejection("output_schema_invalid", path, "type"))?;
    if values.keys().any(|key| !["f", "q"].contains(&key.as_str())) {
        return Err(rejection("output_schema_invalid", path, "unknown_property"));
    }
    let fragment_path = format!("{path}.fragmentId");
    let index = values
        .get("f")
        .ok_or_else(|| rejection("output_schema_invalid", &fragment_path, "required_property"))?
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| {
            rejection(
                "invalid_citation_fragment_index",
                &fragment_path,
                "fragment_index",
            )
        })?;
    let fragment = fragments.get(index).ok_or_else(|| {
        rejection(
            "invalid_citation_fragment_index",
            &fragment_path,
            "fragment_index",
        )
    })?;
    let quote_path = format!("{path}.quote");
    let quote = values
        .get("q")
        .ok_or_else(|| rejection("output_schema_invalid", &quote_path, "required_property"))?
        .as_str()
        .ok_or_else(|| rejection("output_schema_invalid", &quote_path, "type"))?;
    let length = quote.chars().count();
    if quote.trim().is_empty() || length > 1000 {
        return Err(rejection(
            "invalid_citation_quote",
            &quote_path,
            "quote_nonblank_bounded",
        ));
    }
    if fragment.end.checked_sub(fragment.start) != Some(fragment.text.chars().count()) {
        return Err(rejection(
            "invalid_unicode_fragment",
            path,
            "fragment_range",
        ));
    }
    // Scan scalar boundaries, including overlapping matches. Normalizing or
    // choosing one repeated occurrence would invent a position the model did not cite.
    let mut matches = fragment
        .text
        .char_indices()
        .filter_map(|(byte, _)| fragment.text[byte..].starts_with(quote).then_some(byte));
    let byte = matches
        .next()
        .ok_or_else(|| rejection("citation_quote_not_found", &quote_path, "verbatim_quote"))?;
    if matches.next().is_some() {
        return Err(rejection(
            "citation_quote_ambiguous",
            &quote_path,
            "unique_quote",
        ));
    }
    let start = fragment
        .start
        .checked_add(fragment.text[..byte].chars().count())
        .ok_or_else(|| rejection("invalid_unicode_fragment", path, "fragment_range"))?;
    let end = start
        .checked_add(length)
        .filter(|end| *end <= fragment.end)
        .ok_or_else(|| rejection("invalid_unicode_fragment", path, "fragment_range"))?;
    Ok(json!({"fragmentId":fragment.fragment_id,"start":start,"end":end}))
}

fn expand(
    value: &Value,
    schema: &Value,
    fragments: &[Fragment],
    path: &str,
) -> Result<Value, CompactDecodeError> {
    if citation_schema(schema) {
        return expand_citation(value, fragments, path);
    }
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
            let field = expand(field, child, fragments, &field_path)?;
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

/// Strict quote transport decoding; no JSON repair, normalization, defaults, or position guessing.
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
