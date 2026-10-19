//! One frozen request envelope before any child or network work. No logging or secrets lookup.
use crate::{
    model_settings::ModelError,
    pi_adapter::{PI_PROTOCOL, PiRequest},
};
use linggan_contracts::{ai_limits::AI_LIMITS, ai_protocol::PROTOCOL};
use serde_json::Value;
impl PiRequest {
    pub(crate) fn wire_bytes(&self) -> Result<Vec<u8>, ModelError> {
        let limits = &AI_LIMITS;
        if self.version != PI_PROTOCOL
            || !(100..=limits.max_timeout_seconds * 1000).contains(&self.timeout_ms)
            || !(16..=limits.max_output_tokens)
                .contains(&u64::try_from(self.max_output_tokens).map_err(|_| ModelError::Invalid)?)
        {
            return Err(ModelError::Invalid);
        }
        if let Some(execution) = &self.execution {
            execution.validate(
                &self.api,
                self.max_output_tokens as u64,
                self.timeout_ms,
                &self.system,
                &self.prompt,
            )?;
        } else if self.timeout_ms > limits.legacy_timeout_seconds * 1000
            || self.max_output_tokens as u64 > limits.legacy_output_tokens
        {
            return Err(ModelError::Invalid);
        }
        let topic = serde_json::from_str::<Value>(&self.prompt)
            .ok()
            .and_then(|p| p["contract"].as_str().map(str::to_owned))
            .is_some_and(|name| {
                PROTOCOL["legacyTopicContracts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_str() == Some(&name))
            });
        let limit = if self.execution.is_some() {
            limits.max_request_bytes
        } else if topic {
            limits.legacy_topic_request_bytes
        } else {
            limits.legacy_request_bytes
        };
        let input = serde_json::to_vec(self).map_err(|_| ModelError::Invalid)?;
        if input.len() as u64 > limit {
            return Err(ModelError::InputLimit);
        }
        Ok(input)
    }
}
