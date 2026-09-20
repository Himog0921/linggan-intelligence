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

pub(crate) fn paddle_ocr_python() -> String {
    std::env::var("LINGGAN_PADDLE_OCR_PYTHON").unwrap_or_else(|_| {
        let support_dir = std::env::var_os("LINGGAN_SUPPORT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join("Library/Application Support/Linggan Intelligence")
            });
        support_dir
            .join("paddle-ocr/venv/bin/python")
            .display()
            .to_string()
    })
}

pub(crate) fn paddle_ocr_script() -> PathBuf {
    std::env::var_os("LINGGAN_PADDLE_OCR_SCRIPT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("src/bin/media_worker_support/paddle_ocr.py")
        })
}

pub(crate) fn ffmpeg_command() -> String {
    std::env::var("LINGGAN_FFMPEG_COMMAND")
        .unwrap_or_else(|_| "/opt/homebrew/bin/ffmpeg".to_owned())
}

pub(crate) fn whisper_command() -> String {
    std::env::var("LINGGAN_WHISPER_COMMAND")
        .unwrap_or_else(|_| "/Users/moglenny/Library/Python/3.9/bin/whisper".to_owned())
}

/// whisper 用哪个尺寸的模型。
///
/// `medium` 是 2026-09-16 实测选的，不是「越大越好」的直觉：在 M4 上跑一段 109.9 秒中文音频，
/// `medium` 用时 **187 秒（1.70 倍实时）**——这是**要上线的那个脚本**（带 `--initial_prompt`）
/// 的实测值，不加提示的裸跑是 181 秒，差的 6 秒不值得为它单独记一个数。
///
/// 代价是重跑 195 条视频（`derivatives/audio` 下实测合计 **6.35 小时**音频）需要约 **11 小时**：
/// 并发闸把 asr 限成同时 1 条，所以这就是墙钟时间。换来的是识别质量——`tiny` 此前一条都没跑成过，
/// 所以没有可比的历史质量数据，只能说 `tiny` 对中文严重不足。
///
/// **首次跑要注意：模型本体要现场下载（约 1.5GB），这段下载算在这条 asr 自己的 30 分钟预算里。**
/// `run_command` 是一到点就 `kill` 子进程的，慢网络下第一条可能整个被下载吃掉、被杀在中途。
/// 历史上唯一那一条非 `asr_output_missing` 的失败，正是 `tiny.pt` 校验和对不上、当场重下时被打断。
/// 所以**换模型后先单独把模型拉下来再开跑**，别让第一条去承担这 1.5GB。
pub(crate) fn whisper_model() -> String {
    std::env::var("LINGGAN_WHISPER_MODEL").unwrap_or_else(|_| "medium".to_owned())
}

/// 让 whisper 找得到 ffmpeg。
///
/// **这是 196 条视频转录里 195 条失败的根因。**whisper 自己不解析 `ffmpeg` 的绝对路径——它在
/// `audio.py` 里调裸 `ffmpeg`，交给 PATH。而本进程由 launchd 拉起，PATH 只有
/// `/Users/moglenny/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin`，**没有 `/opt/homebrew/bin`**。
/// 后果比报错更难查：whisper 对每个音频片段打印一行 "Skipping …"，然后**以退出码 0 结束**，
/// 只是输出目录里空空如也。于是我们这边只看到 `asr_output_missing`（195 条），
/// 而它看起来像「视频里没人说话」，不像「工具链断了一节」。
///
/// **剩下那 1 条不是这个原因**，是 `tiny.pt` 校验和对不上、当场重下模型时被打断
/// （`last_error` 是 `asr_failed:` 加一段下载进度条）。它不改变结论——换成 `medium` 会重新
/// 拉一份模型，这条也会跟着重跑——但「196 条全部同一根因」是句假话，记清楚。
///
/// 补的是**我们已经知道的那个 ffmpeg 目录**，不是猜一个通用 PATH：`LINGGAN_FFMPEG_COMMAND`
/// 指向哪里，whisper 就看得见哪里。两处配置只有一个事实来源。
///
/// **拼接失败必须报错，不能就这么算了。** `join_paths` 在 Unix 上遇到带 `:` 的分量会返回
/// `Err`，此时若静默跳过，whisper 就回到 launchd 那个没有 `/opt/homebrew/bin` 的 PATH，
/// 于是「逐段 Skipping、退出码 0、输出目录空空如也、只看到 `asr_output_missing`」——
/// **和真修复之前一模一样，而且从外部完全区分不出来**。这个函数存在的全部理由就是不让人
/// 再经历一次那种排查，所以它自己的失败不能是同一种形态。
///
/// 两处提前返回不是失败：命令名里没有目录（裸 `ffmpeg`）时，子进程本来就要靠 PATH 找它，
/// 而 `command_available` 用的是同一条 PATH——那种情况下 asr 根本不会被启用。
pub(crate) fn apply_ffmpeg_path(command: &mut Command) -> Result<(), String> {
    let ffmpeg = ffmpeg_command();
    let Some(directory) = Path::new(&ffmpeg).parent() else {
        return Ok(());
    };
    if directory.as_os_str().is_empty() {
        return Ok(());
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let entries = std::iter::once(directory.to_path_buf()).chain(std::env::split_paths(&inherited));
    let joined =
        std::env::join_paths(entries).map_err(|error| format!("ffmpeg_path_unusable:{error}"))?;
    command.env("PATH", joined);
    Ok(())
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
