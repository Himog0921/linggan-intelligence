//! Versioned metadata vocabulary for pinned pi. Routing, headers and tool grants are absent.
use serde_json::Value;
use std::sync::LazyLock;
pub const SOURCE: &str = include_str!("ai_protocol.json");
pub static PROTOCOL: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(SOURCE).expect("AI protocol parses"));
fn listed(values: &Value, value: &str) -> bool {
    values
        .as_array()
        .is_some_and(|values| values.iter().any(|v| v.as_str() == Some(value)))
}
fn safe_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 80 && !["__proto__", "prototype", "constructor"].contains(&key)
}
fn template_value(value: &Value) -> bool {
    match value {
        Value::Bool(_) => true,
        Value::Number(n) => n.as_f64().is_some_and(f64::is_finite),
        Value::String(s) => s.len() <= 200,
        Value::Object(map) => {
            map.keys()
                .all(|k| ["$var", "omitWhenOff"].contains(&k.as_str()))
                && map.get("$var").and_then(Value::as_str).is_some_and(|s| {
                    ["thinking.enabled", "thinking.effort", "thinking.budget"].contains(&s)
                })
                && map.get("omitWhenOff").is_none_or(Value::is_boolean)
        }
        _ => false,
    }
}
pub fn valid_compat(api: &str, value: &Value) -> bool {
    let spec = &PROTOCOL["apis"][api];
    let Some(map) = value.as_object() else {
        return false;
    };
    if spec.is_null() || value.to_string().len() > 16384 {
        return false;
    }
    map.iter().all(|(key, value)| {
        if !safe_key(key) {
            return false;
        }
        if listed(&spec["booleans"], key) {
            return value.is_boolean();
        }
        if let Some(s) = value.as_str() {
            return listed(&spec["enums"][key], s);
        }
        if listed(&spec["templates"], key) {
            return value
                .as_object()
                .is_some_and(|map| map.iter().all(|(k, v)| safe_key(k) && template_value(v)));
        }
        let bounds = &spec["numbers"][key];
        value.as_i64().is_some_and(|n| {
            bounds["min"].as_i64().is_some_and(|min| n >= min)
                && bounds["max"].as_i64().is_some_and(|max| n <= max)
        })
    })
}
pub fn valid_output_mode(api: &str, mode: &str, strict: bool) -> bool {
    listed(&PROTOCOL["apis"][api]["outputModes"], mode)
        && (!strict || mode == "json_schema")
        && !(api == "anthropic-messages" && mode == "json_schema" && !strict)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn protocol_matches_both_pinned_sdk_packages() {
        let package: Value =
            serde_json::from_str(include_str!("../../../apps/pi-adapter/package.json")).unwrap();
        assert_eq!(
            package["dependencies"]["@earendil-works/pi-ai"],
            PROTOCOL["sdkVersion"]
        );
        assert_eq!(
            package["dependencies"]["@earendil-works/pi-agent-core"],
            PROTOCOL["sdkVersion"]
        );
    }
    #[test]
    fn metadata_cannot_turn_into_headers_routes_tools_or_unbounded_template_code() {
        assert!(valid_compat(
            "openai-completions",
            &json!({"thinkingFormat":"deepseek","supportsReasoningEffort":true,"maxTokensField":"max_tokens"})
        ));
        assert!(valid_compat(
            "openai-completions",
            &json!({"thinkingFormat":"chat-template","chatTemplateKwargs":{"enable_thinking":{"$var":"thinking.enabled"}}})
        ));
        for bad in [
            json!({"headers":{"Authorization":"secret"}}),
            json!({"openRouterRouting":{"only":["other"]}}),
            json!({"thinkingFormat":"unknown"}),
            json!({"supportsStore":"true"}),
            json!({"chatTemplateKwargs":{"__proto__":true}}),
            json!({"chatTemplateKwargs":{"x":{"$var":"arbitrary.file"}}}),
        ] {
            assert!(!valid_compat("openai-completions", &bad));
        }
        assert!(!valid_output_mode(
            "anthropic-messages",
            "json_object",
            false
        ));
        assert!(!valid_output_mode(
            "anthropic-messages",
            "json_schema",
            false
        ));
        assert!(valid_output_mode("anthropic-messages", "json_schema", true));
    }
}
