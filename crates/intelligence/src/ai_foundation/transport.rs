//! Frozen adapter metadata, never permission. Both processes validate this envelope.
use super::{
    capability::{Capability, ModelLimits},
    provider_store::OutputMode,
};
use crate::model_settings::ModelError;
use linggan_contracts::{
    ai_limits::AI_LIMITS,
    ai_protocol::{PROTOCOL, valid_compat, valid_output_mode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseSchema {
    pub name: String,
    pub schema: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiExecution {
    pub capability_ref: Uuid,
    pub limits: Capability,
    pub input_token_limit: u64,
    pub thinking_level: String,
    pub compat: Value,
    pub thinking_level_map: BTreeMap<String, Option<String>>,
    pub output_mode: OutputMode,
    pub strict_schema: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<ResponseSchema>,
}
impl PiExecution {
    pub fn validate(
        &self,
        api: &str,
        output: u64,
        timeout_ms: u64,
        system: &str,
        prompt: &str,
    ) -> Result<(), ModelError> {
        self.limits
            .permits(&ModelLimits {
                input_tokens: self.input_token_limit,
                output_tokens: output,
                timeout_seconds: timeout_ms.div_ceil(1000),
                attempts: 1,
                thinking_level: self.thinking_level.clone(),
            })
            .map_err(|_| ModelError::Invalid)?;
        if !valid_compat(api, &self.compat)
            || !valid_output_mode(api, self.output_mode.as_str(), self.strict_schema)
            || self.thinking_level_map.iter().any(|(level, mapped)| {
                !PROTOCOL["thinkingLevels"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_str() == Some(level))
                    || mapped
                        .as_ref()
                        .is_some_and(|v| v.trim().is_empty() || v.len() > 80)
                    || (mapped.is_none() && self.limits.thinking_levels.contains(level))
            })
            || (self.thinking_level == "off" && self.compat["supportsMidConvoEffort"] == true)
            || (self.thinking_level != "off"
                && api == "openai-completions"
                && self.compat["thinkingFormat"].as_str().unwrap_or("openai") == "openai"
                && !self.compat["supportsReasoningEffort"]
                    .as_bool()
                    .unwrap_or_else(|| {
                        PROTOCOL["apis"][api]["defaults"]["supportsReasoningEffort"]
                            .as_bool()
                            .unwrap_or(false)
                    }))
        {
            return Err(ModelError::Invalid);
        }
        if (api == "openai-responses" && self.compat["supportsMaxOutputTokens"] == false)
            || (api == "anthropic-messages"
                && self.thinking_level != "off"
                && self.compat["forceAdaptiveThinking"] != true
                && self.compat["supportsMidConvoEffort"] != true
                && output
                    < PROTOCOL["apis"][api]["minBudgetThinkingTokens"]
                        .as_u64()
                        .unwrap()
                        + PROTOCOL["apis"][api]["minThinkingAnswerTokens"]
                            .as_u64()
                            .unwrap())
        {
            return Err(ModelError::Invalid);
        }
        if let Some(response) = &self.response_schema {
            let valid_name = !response.name.is_empty()
                && response.name.len() <= 64
                && response
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
            if !valid_name
                || response.schema["type"] != "object"
                || !response.schema["properties"].is_object()
                || response.schema["additionalProperties"] != false
                || response.schema.to_string().len() as u64 > AI_LIMITS.max_response_schema_bytes
                || (self.strict_schema && !complete_schema(&response.schema))
            {
                return Err(ModelError::Invalid);
            }
        }
        if estimate_input_tokens(system, prompt) > self.input_token_limit {
            return Err(ModelError::InputLimit);
        }
        Ok(())
    }
}
/// Conservative request estimate, not a tokenizer or a provider capacity claim.
pub fn estimate_input_tokens(system: &str, prompt: &str) -> u64 {
    fn estimate(value: &str) -> u64 {
        let (mut total, mut ascii_run) = (0_u64, 0_u64);
        for ch in value.chars() {
            if ch.is_ascii_alphanumeric() {
                ascii_run += 1;
            } else {
                total += ascii_run.div_ceil(3) + 1;
                ascii_run = 0;
            }
        }
        total + ascii_run.div_ceil(3)
    }
    estimate(system) + estimate(prompt) + 128
}
fn complete_schema(schema: &Value) -> bool {
    fn visit(s: &Value, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if !s.is_object() || depth > 20 || *nodes > 256 || s.get("$ref").is_some() {
            return false;
        }
        if let Some(any) = s.get("anyOf").and_then(Value::as_array) {
            return !any.is_empty() && any.iter().all(|v| visit(v, depth + 1, nodes));
        }
        let types = match s.get("type") {
            Some(Value::String(t)) => vec![t.as_str()],
            Some(Value::Array(a)) => {
                let Some(types) = a.iter().map(Value::as_str).collect::<Option<Vec<_>>>() else {
                    return false;
                };
                types
            }
            _ => return false,
        };
        if types.is_empty()
            || types.iter().any(|t| {
                ![
                    "object", "array", "string", "integer", "number", "boolean", "null",
                ]
                .contains(t)
            })
        {
            return false;
        }
        if types.contains(&"object") {
            let (Some(properties), Some(required)) = (
                s.get("properties").and_then(Value::as_object),
                s.get("required").and_then(Value::as_array),
            ) else {
                return false;
            };
            if s["additionalProperties"] != false
                || required.len() != properties.len()
                || required
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != properties.len()
                || !properties.iter().all(|(k, v)| {
                    required.iter().any(|r| r.as_str() == Some(k)) && visit(v, depth + 1, nodes)
                })
            {
                return false;
            }
        }
        !types.contains(&"array") || s.get("items").is_some_and(|v| visit(v, depth + 1, nodes))
    }
    visit(schema, 0, &mut 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn capacity_and_metadata_bind_input_output_timeout_and_actual_thinking_without_model_name_rules()
     {
        let mut e = PiExecution {
            capability_ref: Uuid::new_v4(),
            limits: Capability {
                context_tokens: 1_000_000,
                max_output_tokens: 384_000,
                max_timeout_seconds: 3600,
                thinking_levels: ["off", "high", "max"].map(str::to_owned).into(),
            },
            input_token_limit: 600_000,
            thinking_level: "max".into(),
            compat: json!({"thinkingFormat":"deepseek","supportsReasoningEffort":true}),
            thinking_level_map: BTreeMap::new(),
            output_mode: OutputMode::PromptOnly,
            strict_schema: false,
            response_schema: None,
        };
        assert!(
            e.validate("openai-completions", 384_000, 3_600_000, "Synthetic", "材")
                .is_ok()
        );
        assert!(matches!(
            e.validate("openai-completions", 400_001, 3000, "", ""),
            Err(ModelError::Invalid)
        ));
        assert!(matches!(
            e.validate("openai-completions", 384_000, 3_600_001, "", ""),
            Err(ModelError::Invalid)
        ));
        e.input_token_limit = 1024;
        assert!(matches!(
            e.validate("openai-completions", 1024, 3000, "", &"材".repeat(1025)),
            Err(ModelError::InputLimit)
        ));
        e.compat = json!({"headers":{"Authorization":"not-a-capability"}});
        assert!(matches!(
            e.validate("openai-completions", 1024, 3000, "", ""),
            Err(ModelError::Invalid)
        ));
    }
    #[test]
    fn strict_schema_rejects_incomplete_nested_objects_and_external_references() {
        let valid = json!({"type":"object","properties":{"label":{"type":["string","null"]}},"required":["label"],"additionalProperties":false});
        assert!(complete_schema(&valid));
        let mut bad = valid.clone();
        bad["required"] = json!([]);
        assert!(!complete_schema(&bad));
        bad = valid;
        bad["properties"]["label"] = json!({"$ref":"https://not-a-resource"});
        assert!(!complete_schema(&bad));
    }
}
