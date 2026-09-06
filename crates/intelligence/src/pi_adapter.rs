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
    serde_json::json!({"ok":response.ok,"failureCode":response.failure_code,"usage":response.usage,"elapsedMs":response.elapsed_ms,"modelIds":response.model_ids,"modelListOrigin":response.model_list_origin})
}
