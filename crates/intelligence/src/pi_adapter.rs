//! One fixed child process, one bounded request; credentials are stdin-only and never Debug.
use crate::model_settings::ModelError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};

pub const PI_PROTOCOL: &str = "linggan.pi.v1/0.85.1";
pub const WEMM_PROTOCOL: &str = "linggan.wemm.v1";
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
    wemm_python: PathBuf,
    wemm_script: PathBuf,
    wemm_model: PathBuf,
    wemm_revision: String,
    wemm_process: Arc<Mutex<Option<WeMMProcess>>>,
}

struct WeMMProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeMMResponse {
    pub version: String,
    #[serde(default)]
    pub id: Option<String>,
    pub ok: bool,
    #[serde(default)]
    pub values: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    pub failure_code: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<i64>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub dimension: Option<usize>,
    #[serde(default, rename = "type")]
    pub response_type: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub model_revision: Option<String>,
    #[serde(default)]
    pub encoding_mode: Option<String>,
}

impl WeMMResponse {
    fn valid_ready(&self, revision: &str) -> bool {
        self.version == WEMM_PROTOCOL
            && self.ok
            && self.response_type.as_deref() == Some("ready")
            && self.model_id.as_deref() == Some("Tencent/WeMM-Embedding-2B")
            && self.model_revision.as_deref() == Some(revision)
            && self.encoding_mode.as_deref() == Some("document")
            && self.dimension == Some(512)
            && matches!(self.backend.as_deref(), Some("mps") | Some("cpu"))
    }

    pub fn single_document_values(&self, request_id: &str) -> Option<Vec<f64>> {
        if self.version != WEMM_PROTOCOL
            || !self.ok
            || self.id.as_deref() != Some(request_id)
            || self.dimension != Some(512)
            || !matches!(self.backend.as_deref(), Some("mps") | Some("cpu"))
        {
            return None;
        }
        let vectors = self.values.as_ref()?;
        (vectors.len() == 1).then(|| vectors[0].clone())
    }
}
impl PiAdapter {
    pub fn configured() -> Self {
        let support_dir = std::env::var_os("LINGGAN_SUPPORT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library/Application Support/Linggan Intelligence")
            });
        let wemm_runtime = std::env::var_os("LINGGAN_WEMM_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| support_dir.join("wemm-embedding-2b"));
        Self {
            node: std::env::var_os("LINGGAN_PI_NODE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                        .join(".nvm/versions/node/v24.13.0/bin/node")
                }),
            script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/pi-adapter/src/adapter.mjs"),
            wemm_python: std::env::var_os("LINGGAN_WEMM_PYTHON")
                .map(PathBuf::from)
                .unwrap_or_else(|| wemm_runtime.join("venv/bin/python")),
            wemm_script: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../apps/pi-adapter/src/wemm_runtime.py"),
            wemm_model: std::env::var_os("LINGGAN_WEMM_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| wemm_runtime.join("model")),
            wemm_revision: "bbd6cd4bf52cfc6716f752a2df80b2706720bd95".into(),
            wemm_process: Arc::new(Mutex::new(None)),
        }
    }

    /// Isolated PostgreSQL proofs use a checked-in local process instead of a provider. This
    /// keeps worker settlement tests on the production adapter boundary without loading a model
    /// or changing the configured runtime.
    #[doc(hidden)]
    pub fn configured_with_test_command(node: PathBuf, script: PathBuf) -> Self {
        let mut adapter = Self::configured();
        adapter.node = node;
        adapter.script = script;
        adapter
    }

    /// Calls the one local WeMM document runtime.  Its process remains owned by this adapter
    /// (which in turn is owned by the existing research worker), so model weights load once and
    /// no port, independent queue or new lifecycle is introduced.
    pub async fn embed_wemm_document(&self, text: &str) -> Result<WeMMResponse, ModelError> {
        if text.is_empty() || text.len() > 16_000 {
            return Err(ModelError::InputLimit);
        }
        let mut guard = self.wemm_process.lock().await;
        if guard.is_none() {
            *guard = Some(self.start_wemm().await?);
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::to_vec(&serde_json::json!({
            "id": request_id,
            "encodingMode": "document",
            "texts": [text],
        }))
        .map_err(|_| ModelError::Invalid)?;
        let outcome = async {
            let process = guard.as_mut().ok_or(ModelError::AdapterUnavailable)?;
            process
                .stdin
                .write_all(&payload)
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            process
                .stdin
                .write_all(b"\n")
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            process
                .stdin
                .flush()
                .await
                .map_err(|_| ModelError::AdapterUnavailable)?;
            read_wemm_line(&mut process.stdout).await
        };
        match tokio::time::timeout(Duration::from_secs(120), outcome).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(error)) => {
                if let Some(mut process) = guard.take() {
                    let _ = process.child.kill().await;
                }
                Err(error)
            }
            Err(_) => {
                if let Some(mut process) = guard.take() {
                    let _ = process.child.kill().await;
                }
                Err(ModelError::Timeout)
            }
        }
    }

    async fn start_wemm(&self) -> Result<WeMMProcess, ModelError> {
        let mut child = tokio::process::Command::new(&self.wemm_python)
            .arg(&self.wemm_script)
            .arg("--model-path")
            .arg(&self.wemm_model)
            .arg("--model-revision")
            .arg(&self.wemm_revision)
            .env_clear()
            .env("PYTHONNOUSERSITE", "1")
            .env("TOKENIZERS_PARALLELISM", "false")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| ModelError::AdapterUnavailable)?;
        let stdin = child.stdin.take().ok_or(ModelError::AdapterUnavailable)?;
        let stdout = child.stdout.take().ok_or(ModelError::AdapterUnavailable)?;
        let mut process = WeMMProcess {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        };
        let ready =
            tokio::time::timeout(Duration::from_secs(90), read_wemm_line(&mut process.stdout))
                .await
                .map_err(|_| ModelError::Timeout)??;
        if !ready.valid_ready(&self.wemm_revision) {
            let _ = process.child.kill().await;
            return Err(ModelError::AdapterUnavailable);
        }
        Ok(process)
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

async fn read_wemm_line(reader: &mut BufReader<ChildStdout>) -> Result<WeMMResponse, ModelError> {
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .await
        .map_err(|_| ModelError::AdapterUnavailable)?;
    if bytes == 0 || bytes > 262_144 {
        return Err(ModelError::InvalidOutput);
    }
    serde_json::from_str(&line).map_err(|_| ModelError::InvalidOutput)
}
pub fn safe_result(response: &PiResponse) -> Value {
    serde_json::json!({"ok":response.ok,"failureCode":response.failure_code,"usage":response.usage,"elapsedMs":response.elapsed_ms,"modelIds":response.model_ids,"modelListOrigin":response.model_list_origin,"diagnostic":response.diagnostic,"textShape":safe_text_shape(response.text.as_deref())})
}

/// A model response can be diagnosed without retaining its text, prompt, or hidden reasoning.
/// The categories describe only the outer transport shape; the worker remains the authority for
/// the semantic contract and never upgrades a failed response on the basis of this receipt.
fn safe_text_shape(text: Option<&str>) -> Value {
    let Some(text) = text else {
        return serde_json::json!({"presence":"absent"});
    };
    let trimmed = text.trim().trim_start_matches('\u{feff}');
    let shape = if trimmed.is_empty() {
        "empty"
    } else if serde_json::from_str::<Value>(trimmed).is_ok() {
        "direct_json"
    } else if complete_json_fence(trimmed).is_some() {
        "json_fence"
    } else {
        "non_json"
    };
    let size_bucket = match text.len() {
        0..=256 => "0_256",
        257..=1024 => "257_1024",
        1025..=4096 => "1025_4096",
        4097..=16384 => "4097_16384",
        _ => "over_16384",
    };
    serde_json::json!({"presence":"present","shape":shape,"sizeBucket":size_bucket})
}

fn complete_json_fence(text: &str) -> Option<&str> {
    let fenced = text.strip_prefix("```json")?;
    let body = fenced
        .strip_prefix("\r\n")
        .or_else(|| fenced.strip_prefix('\n'))?;
    let body = body.trim_end().strip_suffix("```")?.trim();
    (!body.is_empty()).then_some(body)
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

    #[test]
    fn safe_text_shape_exposes_only_outer_shape_and_bucket() {
        let raw = "```json\n{\"comment\":\"SYNTHETIC-SENSITIVE-TEXT\"}\n```";
        let receipt = safe_text_shape(Some(raw));
        assert_eq!(receipt["presence"], "present");
        assert_eq!(receipt["shape"], "json_fence");
        assert_eq!(receipt["sizeBucket"], "0_256");
        assert!(!receipt.to_string().contains("SYNTHETIC-SENSITIVE-TEXT"));
        assert_eq!(safe_text_shape(None)["presence"], "absent");
    }
}
