//! A recorded model capability bounds a configuration; global ceilings never fill unknowns.
use super::RuleError;
use linggan_contracts::ai_limits::AI_LIMITS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub context_tokens: u64,
    pub max_output_tokens: u64,
    pub max_timeout_seconds: u64,
    pub thinking_levels: BTreeSet<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelLimits {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub timeout_seconds: u64,
    pub attempts: u32,
    pub thinking_level: String,
}
impl Capability {
    pub fn validate(&self) -> Result<(), RuleError> {
        let valid_levels = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
        if self.context_tokens < 1024
            || self.context_tokens > AI_LIMITS.max_context_tokens
            || self.max_output_tokens == 0
            || self.max_output_tokens > self.context_tokens
            || self.max_output_tokens > AI_LIMITS.max_output_tokens
            || self.max_timeout_seconds == 0
            || self.max_timeout_seconds > AI_LIMITS.max_timeout_seconds
            || self.thinking_levels.is_empty()
            || self
                .thinking_levels
                .iter()
                .any(|l| !valid_levels.contains(&l.as_str()))
        {
            return Err(RuleError::Capability);
        }
        Ok(())
    }
    pub fn permits(&self, config: &ModelLimits) -> Result<(), RuleError> {
        self.validate()?;
        if config.input_tokens < 1024
            || config.output_tokens < 16
            || config.attempts == 0
            || config.attempts > AI_LIMITS.max_attempts
            || config
                .input_tokens
                .checked_add(config.output_tokens)
                .is_none_or(|sum| sum > self.context_tokens)
            || config.output_tokens > self.max_output_tokens
            || config.timeout_seconds == 0
            || config.timeout_seconds > self.max_timeout_seconds
        {
            return Err(RuleError::Capacity);
        }
        if !self.thinking_levels.contains(&config.thinking_level) {
            return Err(RuleError::Thinking);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combined_context_output_timeout_and_thinking_all_bind_the_actual_model() {
        let cap = Capability {
            context_tokens: 1_000_000,
            max_output_tokens: 384_000,
            max_timeout_seconds: 600,
            thinking_levels: ["off", "high", "max"].map(str::to_owned).into(),
        };
        let mut config = ModelLimits {
            input_tokens: 500_000,
            output_tokens: 384_000,
            timeout_seconds: 300,
            attempts: 1,
            thinking_level: "high".into(),
        };
        assert!(cap.permits(&config).is_ok());
        config.input_tokens = 900_000;
        assert_eq!(cap.permits(&config), Err(RuleError::Capacity));
        config.input_tokens = 500_000;
        config.thinking_level = "low".into();
        assert_eq!(cap.permits(&config), Err(RuleError::Thinking));
        config.thinking_level = "off".into();
        config.timeout_seconds = 601;
        assert_eq!(cap.permits(&config), Err(RuleError::Capacity));
        config.timeout_seconds = 300;
        config.output_tokens = 384_001;
        assert_eq!(cap.permits(&config), Err(RuleError::Capacity));
    }
}
