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
                || !["analyze", "analyze_image"].contains(&self.operation.as_str())
            {
                return Err(ModelError::Invalid);
            }
            for image in &self.images {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&image.data)
                    .map_err(|_| ModelError::Invalid)?;
                let magic = match image.mime_type.as_str() {
                    "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
                    "image/jpeg" => bytes.starts_with(&[255, 216, 255]),
                    "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
                    _ => false,
                };
                if !magic || bytes.len() > 8_000_000 {
                    return Err(ModelError::Invalid);
                }
            }
            if super::ai_foundation::transport::estimate_input_tokens(&self.system, &self.prompt)
                + self.images.len() as u64 * 1024
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ai_foundation::{
            capability::Capability, provider_store::OutputMode, transport::PiExecution,
        },
        pi_adapter::PiImage,
    };
    use base64::Engine;
    use serde_json::json;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn request(count: usize) -> PiRequest {
        PiRequest {
            version: PI_PROTOCOL,
            operation: "analyze".into(),
            api: "openai-responses".into(),
            base_url: "http://127.0.0.1:1".into(),
            local_endpoint: true,
            api_key: "synthetic".into(),
            model_id: "synthetic".into(),
            timeout_ms: 1000,
            max_output_tokens: 2000,
            system: String::new(),
            prompt: String::new(),
            images: (0..count)
                .map(|i| PiImage {
                    mime_type: "image/png".into(),
                    data: base64::engine::general_purpose::STANDARD
                        .encode([b"\x89PNG\r\n\x1a\n".as_slice(), &[i as u8]].concat()),
                })
                .collect(),
            execution: Some(PiExecution {
                capability_ref: Uuid::new_v4(),
                limits: Capability {
                    context_tokens: 32768,
                    max_output_tokens: 8192,
                    max_timeout_seconds: 60,
                    thinking_levels: ["high".into()].into(),
                },
                input_token_limit: 24576,
                thinking_level: "high".into(),
                compat: json!({}),
                thinking_level_map: BTreeMap::new(),
                output_mode: OutputMode::PromptOnly,
                strict_schema: false,
                response_schema: None,
            }),
        }
    }
    #[test]
    fn qualified_multimage_wire_keeps_all_bytes_and_order() {
        let r = request(11);
        let wire: Value = serde_json::from_slice(&r.wire_bytes().unwrap()).unwrap();
        assert_eq!(wire["images"].as_array().unwrap().len(), 11);
        for (i, image) in r.images.iter().enumerate() {
            assert_eq!(wire["images"][i]["data"], image.data);
        }
        let mut r = request(11);
        r.images[10].data = "not-base64".into();
        assert!(matches!(r.wire_bytes(), Err(ModelError::Invalid)));
    }
    #[test]
    fn images_require_qualified_operation_and_single_image_mode_is_unchanged() {
        let mut r = request(2);
        r.operation = "analyze_image".into();
        assert!(matches!(r.wire_bytes(), Err(ModelError::Invalid)));
        r.images.pop();
        assert!(r.wire_bytes().is_ok());
        r.images.clear();
        assert!(matches!(r.wire_bytes(), Err(ModelError::Invalid)));
        let mut r = request(1);
        r.execution = None;
        assert!(matches!(r.wire_bytes(), Err(ModelError::Invalid)));
        let mut r = request(1);
        r.operation = "probe".into();
        assert!(matches!(r.wire_bytes(), Err(ModelError::Invalid)));
    }
    #[test]
    fn image_input_reserve_counts_every_image_at_exact_boundary() {
        let mut r = request(11);
        r.execution.as_mut().unwrap().input_token_limit =
            super::super::ai_foundation::transport::estimate_input_tokens(&r.system, &r.prompt)
                + 11 * 1024;
        assert!(r.wire_bytes().is_ok());
        r.execution.as_mut().unwrap().input_token_limit -= 1;
        assert!(matches!(r.wire_bytes(), Err(ModelError::InputLimit)));
        r.images.pop();
        assert!(r.wire_bytes().is_ok());
    }
}
