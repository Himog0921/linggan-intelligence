//! One fixed child process, one bounded request; credentials are stdin-only and never Debug.
use crate::model_settings::ModelError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const PI_PROTOCOL: &str = "linggan.pi.v1/0.85.1";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiRequest {
    pub version: &'static str,
    pub operation: String,
    pub api: String,
    pub base_url: String,
    pub local_endpoint: bool,
    pub api_key: String,
    pub model_id: String,
    pub timeout_ms: u64,
    pub max_output_tokens: i32,
    pub system: String,
    pub prompt: String,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiResponse {
    pub version: String,
    pub ok: bool,
    pub text: Option<String>,
    pub failure_code: Option<String>,
    pub model_ids: Option<Vec<String>>,
    pub model_list_origin: Option<String>,
    pub usage: PiUsage,
    pub elapsed_ms: Option<i64>,
    #[serde(default)]
    pub diagnostic: Option<PiDiagnostic>,
}
/// Only transport metadata crosses the child boundary; upstream error prose never does.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PiDiagnostic {
    pub schema_version: u8,
    pub stage: String,
    pub http_status: Option<u16>,
    pub response_started: Option<bool>,
    pub terminal_received: Option<bool>,
    pub received_bytes: Option<u64>,
    pub finish_reason: Option<String>,
    pub elapsed_ms: u64,
    pub usage_known: bool,
    pub sdk_error_type: Option<String>,
    pub retry_class: String,
}
impl PiDiagnostic {
    pub fn valid(&self) -> bool {
        self.schema_version == 1
            && matches!(
                self.stage.as_str(),
                "request_not_started"
                    | "request_sent"
                    | "response_started"
                    | "streaming"
                    | "terminal"
                    | "failed"
                    | "timed_out"
                    | "rejected"
            )
            && self.http_status.is_none_or(|v| (100..=599).contains(&v))
            && self.received_bytes.is_none_or(|v| v <= 100_000_000)
            && self.elapsed_ms <= 86_400_000
            && self.finish_reason.as_deref().is_none_or(|v| {
                matches!(
                    v,
                    "stop" | "length" | "content_filter" | "tool_calls" | "cancelled" | "unknown"
                )
            })
            && self.sdk_error_type.as_deref().is_none_or(|v| {
                matches!(
                    v,
                    "timeout"
                        | "abort"
                        | "network"
                        | "http"
                        | "stream_interrupted"
                        | "invalid_response"
                        | "sdk"
                )
            })
            && matches!(
                self.retry_class.as_str(),
                "never" | "after_cooldown" | "manual_review" | "unknown"
            )
    }
    pub fn process_timeout(elapsed_ms: u64) -> Self {
        Self {
            schema_version: 1,
            stage: "timed_out".into(),
            http_status: None,
            response_started: None,
            terminal_received: None,
            received_bytes: None,
            finish_reason: None,
            elapsed_ms,
            usage_known: false,
            sdk_error_type: Some("timeout".into()),
            retry_class: "manual_review".into(),
        }
    }
}
#[derive(Clone)]
pub struct PiAdapter {
    node: PathBuf,
    script: PathBuf,
}
impl PiAdapter {
    pub fn configured() -> Self {
        Self {
            node: std::env::var_os("LINGGAN_PI_NODE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                        .join(".nvm/versions/node/v24.13.0/bin/node")
                }),
            script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/pi-adapter/src/adapter.mjs"),
        }
    }
    pub async fn call(&self, request: &PiRequest) -> Result<PiResponse, ModelError> {
        let input = serde_json::to_vec(request).map_err(|_| ModelError::Invalid)?;
        if input.len() > 131072 {
            return Err(ModelError::InputLimit);
        }
        let mut child = tokio::process::Command::new(&self.node)
            .arg(&self.script)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| ModelError::AdapterUnavailable)?;
        let mut stdin = child.stdin.take().ok_or(ModelError::AdapterUnavailable)?;
        let stdout = child.stdout.take().ok_or(ModelError::AdapterUnavailable)?;
        let deadline = Duration::from_millis(request.timeout_ms + 1000);
        let outcome = tokio::time::timeout(deadline, async {
            stdin
                .write_all(&input)
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            stdin
                .shutdown()
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            drop(stdin);
            let mut bytes = Vec::new();
            stdout
                .take(262145)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            if bytes.len() > 262144 {
                return Err(ModelError::InvalidOutput);
            }
            let status = child
                .wait()
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            if !status.success() {
                return Err(ModelError::AdapterUnavailable);
            }
            let response: PiResponse =
                serde_json::from_slice(&bytes).map_err(|_| ModelError::InvalidOutput)?;
            if response.version != PI_PROTOCOL
                || response.diagnostic.as_ref().is_some_and(|d| {
                    !d.valid()
                        || d.usage_known
                            != (response.usage.input_tokens.is_some()
                                && response.usage.output_tokens.is_some())
                })
                || response
                    .usage
                    .input_tokens
                    .is_some_and(|v| !(0..=100_000_000).contains(&v))
                || response
                    .usage
                    .output_tokens
                    .is_some_and(|v| !(0..=100_000_000).contains(&v))
                || response.usage.cost_usd.is_some()
            {
                return Err(ModelError::InvalidOutput);
            }
            Ok(response)
        })
        .await;
        match outcome {
            Ok(result) => result,
            Err(_) => {
                let _ = child.kill().await;
                Err(ModelError::Timeout)
            }
        }
    }
}
pub fn safe_result(response: &PiResponse) -> Value {
    serde_json::json!({"ok":response.ok,"failureCode":response.failure_code,"usage":response.usage,"elapsedMs":response.elapsed_ms,"modelIds":response.model_ids,"modelListOrigin":response.model_list_origin,"diagnostic":response.diagnostic})
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn missing_transport_observation_stays_unknown() {
        let d = PiDiagnostic::process_timeout(31_000);
        assert!(d.valid());
        let value = serde_json::to_value(&d).unwrap();
        assert!(value["httpStatus"].is_null());
        assert!(value["responseStarted"].is_null());
        assert!(value["receivedBytes"].is_null());
        assert_eq!(value["usageKnown"], false);
    }
    #[test]
    fn diagnostic_rejects_free_form_metadata() {
        let mut d = PiDiagnostic::process_timeout(1);
        d.sdk_error_type = Some("upstream error body with private material".into());
        assert!(!d.valid());
        let mut value = serde_json::to_value(PiDiagnostic::process_timeout(1)).unwrap();
        value["headers"] = serde_json::json!({"authorization":"synthetic"});
        assert!(serde_json::from_value::<PiDiagnostic>(value).is_err());
    }
}
