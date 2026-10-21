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
        if self.operation == "analyze_image" && self.images.len() != 1 {
            return Err(ModelError::Invalid);
        }
        if !self.images.is_empty() {
            use base64::Engine;
            if self.execution.is_none()
                || self.images.len() != 1
                || self.operation != "analyze_image"
            {
                return Err(ModelError::Invalid);
            }
            let image = &self.images[0];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&image.data)
                .map_err(|_| ModelError::Invalid)?;
            let magic = match image.mime_type.as_str() {
                "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
                "image/jpeg" => bytes.starts_with(&[255, 216, 255]),
                "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
                _ => false,
            };
            if !magic
                || bytes.len() > 8_000_000
                || super::ai_foundation::transport::estimate_input_tokens(
                    &self.system,
                    &self.prompt,
                ) + 1024
                    > self.execution.as_ref().unwrap().input_token_limit
            {
                return Err(ModelError::InputLimit);
            }
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
