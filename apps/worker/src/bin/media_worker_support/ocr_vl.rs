//! Explicit full-local OCR job version. Reuses the existing claim and completion transaction.
use super::{local_media_root, paddle_ocr_python, prepare_output, sha256_hex};
use linggan_evidence::{MediaProcessingClaim, ocr_adoption_persistence as store};
use linggan_storage_postgres::Database;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use uuid::Uuid;

const OUTPUT_LIMIT: usize = 8_000_000;
const IMAGE_BUDGET: Duration = Duration::from_secs(180);

pub(crate) async fn execute(
    db: &Database,
    worker: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    if claim.processor_version != store::PROCESSOR_VERSION || !claim.mime_type.starts_with("image/")
    {
        return Err("local_vl_job_mismatch".into());
    }
    if !store::is_safe_input_key(&claim.storage_key) {
        return Err("local_vl_input_key_invalid".into());
    }
    let input = local_media_root().join(&claim.storage_key);
    let bytes = std::fs::read(&input).map_err(|_| "local_vl_input_unavailable")?;
    if sha256_hex(&bytes) != claim.blob_sha256 {
        return Err("local_vl_input_identity_mismatch".into());
    }
    let metadata = store::source_metadata(db, claim)
        .await
        .map_err(|_| "local_vl_source_unavailable")?;
    let key = format!(
        "derivatives/ocr-vl/{}/input-{}.json",
        claim.job_ref, claim.claim_generation
    );
    let meta_path = prepare_output(&key)?;
    let _cleanup = RemoveFile(meta_path.clone());
    private_write(
        &meta_path,
        &serde_json::to_vec(&metadata).map_err(|_| "local_vl_metadata_invalid")?,
    )?;
    let script = std::env::var_os("LINGGAN_PADDLE_VL_SCRIPT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src/bin/media_worker_support/paddle_ocr_vl.py")
        });
    let mut command = Command::new(paddle_ocr_python());
    command
        .arg("-B")
        .arg(script)
        .arg(input)
        .arg(&meta_path)
        .env_clear();
    // Do not forward runtime credentials, database URLs, provider keys or proxies.
    for key in [
        "HOME",
        "PATH",
        "LANG",
        "TMPDIR",
        "HF_HOME",
        "PADDLE_PDX_CACHE_HOME",
        "XDG_CACHE_HOME",
        "LINGGAN_PADDLE_VL_ENDPOINT",
        "LINGGAN_OCR_MODEL_ROOT",
    ] {
        if let Some(v) = std::env::var_os(key) {
            command.env(key, v);
        }
    }
    for (k, v) in [
        ("HF_HUB_OFFLINE", "1"),
        ("TRANSFORMERS_OFFLINE", "1"),
        ("HF_HUB_DISABLE_TELEMETRY", "1"),
        ("DO_NOT_TRACK", "1"),
        ("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True"),
        ("OMP_NUM_THREADS", "4"),
        ("PYTHONDONTWRITEBYTECODE", "1"),
    ] {
        command.env(k, v);
    }
    let raw = bounded_output(&mut command, IMAGE_BUDGET)?;
    // Preserve bounded raw output even when qualification fails; it is not a corpus result.
    let key = format!(
        "derivatives/ocr-vl/{}/{}.json",
        claim.job_ref,
        sha256_hex(&raw)
    );
    private_write(&prepare_output(&key)?, &raw)?;
    store::prepare(&raw).map_err(|kind| format!("local_vl_result_invalid:{kind:?}"))?;
    store::complete(db, claim, worker, &raw, &key)
        .await
        .map_err(|_| "local_vl_completion_failed")?;
    Ok(())
}
struct RemoveFile(PathBuf);
impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn private_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
                Ok(())
            } else {
                Err("local_vl_output_identity_conflict".into())
            };
        }
        Err(_) => return Err("local_vl_output_create_failed".into()),
    };
    f.write_all(bytes)
        .map_err(|_| "local_vl_output_write_failed".into())
}

fn drain(mut source: impl Read, limit: usize) -> Result<Vec<u8>, ()> {
    let mut saved = Vec::new();
    let mut chunk = [0; 8192];
    let mut exceeded = false;
    loop {
        let n = source.read(&mut chunk).map_err(|_| ())?;
        if n == 0 {
            break;
        }
        if saved.len() + n <= limit {
            saved.extend_from_slice(&chunk[..n]);
        } else {
            exceeded = true;
        }
    }
    if exceeded { Err(()) } else { Ok(saved) }
}
fn bounded_output(command: &mut Command, budget: Duration) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "local_vl_spawn_failed")?;
    let out = child.stdout.take().ok_or("local_vl_stdout_missing")?;
    let err = child.stderr.take().ok_or("local_vl_stderr_missing")?;
    let stdout = std::thread::spawn(move || drain(out, OUTPUT_LIMIT));
    // Drain diagnostics while retaining no image text or arbitrary exception content.
    let stderr = std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::BufReader::new(err), &mut std::io::sink());
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| "local_vl_wait_failed")? {
            break status;
        }
        if start.elapsed() >= budget {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout.join();
            let _ = stderr.join();
            return Err("local_vl_image_budget_exceeded".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = stdout
        .join()
        .map_err(|_| "local_vl_read_failed")?
        .map_err(|_| "local_vl_output_limit_or_read_failed")?;
    let _ = stderr.join();
    if !status.success() {
        return Err("local_vl_bridge_failed".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_bound_and_timeout_are_fixed_errors() {
        let mut c = Command::new("sh");
        c.args(["-c", "printf canary; printf private >&2; sleep 1"]);
        assert_eq!(
            bounded_output(&mut c, Duration::from_millis(30)).unwrap_err(),
            "local_vl_image_budget_exceeded"
        );
        assert!(drain(&b"12345"[..], 4).is_err());
    }
}
