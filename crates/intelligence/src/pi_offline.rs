//! Fixed offline helpers. No user path, network permission, credential, or inherited environment.
use crate::{model_settings::ModelError, pi_adapter::PiAdapter};
use linggan_contracts::ai_limits::AI_LIMITS;
use serde::{Serialize, de::DeserializeOwned};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub(crate) enum OfflineHelper {
    Skill,
    Catalog,
}
impl OfflineHelper {
    fn contract(&self) -> (&'static str, u64, u64) {
        match self {
            Self::Skill => (
                "skills.mjs",
                AI_LIMITS.max_skill_total_bytes * 8,
                AI_LIMITS.max_skill_total_bytes * 8,
            ),
            Self::Catalog => ("catalog.mjs", 4096, 1048576),
        }
    }
}
impl PiAdapter {
    pub(crate) async fn call_offline<T: DeserializeOwned>(
        &self,
        helper: OfflineHelper,
        request: &impl Serialize,
    ) -> Result<T, ModelError> {
        let (script, input_bound, output_bound) = helper.contract();
        let input = serde_json::to_vec(request).map_err(|_| ModelError::Invalid)?;
        if input.len() as u64 > input_bound {
            return Err(ModelError::InputLimit);
        }
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src")
            .join(script);
        let mut child = tokio::process::Command::new(self.node_path())
            .arg(script)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| ModelError::AdapterUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let mut stdin = child.stdin.take().ok_or(ModelError::AdapterUnavailable)?;
            let mut stdout = child
                .stdout
                .take()
                .ok_or(ModelError::AdapterUnavailable)?
                .take(output_bound + 1);
            let write = async {
                stdin.write_all(&input).await?;
                drop(stdin);
                Ok::<(), std::io::Error>(())
            };
            let read = async {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes).await?;
                Ok::<_, std::io::Error>(bytes)
            };
            let (_, bytes) =
                tokio::try_join!(write, read).map_err(|_| ModelError::AdapterUnavailable)?;
            if bytes.len() as u64 > output_bound
                || !child
                    .wait()
                    .await
                    .map_err(|_| ModelError::AdapterUnavailable)?
                    .success()
            {
                return Err(ModelError::InvalidOutput);
            }
            serde_json::from_slice(&bytes).map_err(|_| ModelError::InvalidOutput)
        })
        .await;
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => {
                let _ = child.kill().await;
                Err(error)
            }
            Err(_) => {
                let _ = child.kill().await;
                Err(ModelError::Timeout)
            }
        }
    }
}
