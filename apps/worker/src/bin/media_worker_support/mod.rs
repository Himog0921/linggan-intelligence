use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub(crate) struct CommandOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(crate) fn run_command(
    command: &mut Command,
    timeout: Duration,
) -> Result<CommandOutput, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("process_spawn_failed:{error}"))?;
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "process_stdout_unavailable".to_owned())?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "process_stderr_unavailable".to_owned())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        child_stdout.read_to_end(&mut output).map(|_| output)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        child_stderr.read_to_end(&mut output).map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("process_wait_failed:{error}"))?
        {
            break status;
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("process_timeout".to_owned());
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "process_stdout_reader_panicked".to_owned())?
        .map_err(|error| format!("process_stdout_read_failed:{error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "process_stderr_reader_panicked".to_owned())?
        .map_err(|error| format!("process_stderr_read_failed:{error}"))?;
    Ok(CommandOutput {
        status,
        stdout,
        stderr,
    })
}

pub(crate) fn require_success(output: CommandOutput, prefix: &str) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{prefix}_failed:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub(crate) fn prepare_output(storage_key: &str) -> Result<PathBuf, String> {
    let path = local_media_root().join(storage_key);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("output_directory_failed:{error}"))?;
    }
    Ok(path)
}

pub(crate) fn first_text_file(directory: &Path) -> Result<PathBuf, String> {
    std::fs::read_dir(directory)
        .map_err(|error| format!("output_directory_read_failed:{error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("txt"))
        .ok_or_else(|| "asr_output_missing".to_owned())
}

pub(crate) fn local_media_root() -> PathBuf {
    std::env::var("LINGGAN_LOCAL_MEDIA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".linggan-local/media"))
}

pub(crate) fn tesseract_command() -> String {
    std::env::var("LINGGAN_TESSERACT_COMMAND")
        .unwrap_or_else(|_| "/opt/homebrew/bin/tesseract".to_owned())
}

pub(crate) fn ffmpeg_command() -> String {
    std::env::var("LINGGAN_FFMPEG_COMMAND")
        .unwrap_or_else(|_| "/opt/homebrew/bin/ffmpeg".to_owned())
}

pub(crate) fn whisper_command() -> String {
    std::env::var("LINGGAN_WHISPER_COMMAND")
        .unwrap_or_else(|_| "/Users/moglenny/Library/Python/3.9/bin/whisper".to_owned())
}

pub(crate) fn whisper_model() -> String {
    std::env::var("LINGGAN_WHISPER_MODEL").unwrap_or_else(|_| "tiny".to_owned())
}

pub(crate) fn command_available(command: &str) -> bool {
    Path::new(command).is_file()
        || Command::new(command)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
}

pub(crate) fn ensure_local_input(path: &Path) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("local_media_input_unavailable:{error}"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("local_media_input_invalid".to_owned());
    }
    Ok(())
}

pub(crate) fn normalize_text(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn sha256_hex(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
