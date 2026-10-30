//! Separate, bounded local media processor.
//!
//! This process never visits a platform. It only consumes blobs that the Browser Producer has
//! already admitted and materialized under `LINGGAN_LOCAL_MEDIA_ROOT`.

use linggan_evidence::{
    ClaimGateReadiness, MediaProcessingClaim, MediaProcessingClaimOutcome, OcrCompletionInput,
    OcrLayeringInput, OcrLineInput, READINESS_RETRY_START, claim_media_processing_work,
    complete_media_processing_derivative, complete_media_processing_ocr,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work, next_readiness_retry,
    read_claim_gate_readiness,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

mod media_worker_support;

use media_worker_support::{
    apply_ffmpeg_path, command_available, emit_startup_event, ensure_local_input, ffmpeg_command,
    first_json_file, local_media_root, paddle_ocr_python, paddle_ocr_script, prepare_output,
    require_success, run_command, sha256_hex, whisper_command, whisper_model,
};

const TICK_INTERVAL: Duration = Duration::from_secs(5);
const MAX_JOBS_PER_TICK: usize = 4;
const MAX_DISPLAY_CHARS: usize = 600;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// 转录单独一份超时，不跟上面那 15 分钟共用。
///
/// 其余处理器处理的是「一张图」「一段抽帧」，15 分钟已经是「它卡住了」的可靠信号；转录不是——
/// 它的耗时随音频时长线性增长，实测 **1.70 倍实时**（109.9 秒音频跑 187 秒）。按 `derivatives/audio`
/// 下现存的 195 条算，最长一条 7.23 分钟，约 12.3 分钟跑完，15 分钟只剩两成余量；机器上有别的活时
/// 就不够。
///
/// 取 30 分钟：对已见过的最长视频留 **2.4 倍余量**。**边界说在明处**——按这个倍率，
/// 超过约 17.6 分钟的音频会撞上它。真出现那么长的视频，要改的是这里，不是让它静默失败。
///
/// 代价：并发闸把 asr 限成同时 1 条，所以真卡住时，asr 这一路会被占住 30 分钟。
const ASR_PROCESS_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// 转录的开场提示，只为了让输出落到简体。
///
/// **whisper 的中文输出默认是繁体**——它的中文训练语料以繁体为主，不给提示就一路吐繁体。
/// 2026-09-16 用同一段 110 秒中文音频量过：不给提示得 78 个繁体专有字（家長、分鐘、從、說、
/// 變……），给这一句得 **0 个**。这正是 Mog 报的「繁体字」，与 OCR 那边是同一类症状、不同的
/// 成因：OCR 是语言表里多带了 `chi_tra`，转录是模型本身的默认。
///
/// 只加提示、**不加 `--language zh`**：实测两者产出一字不差（都是 1888 字节），而强制语种
/// 会让一段英文音频被硬当成中文转写，提示则保留了 whisper 自己的语种判断。
///
/// 提示本身不进入结果，只是解码的起点。
const ASR_INITIAL_PROMPT: &str = "以下是普通话的句子。";

const OCR_RAW_DIRECTORY: &str = "ocr-raw";
const OCR_LAYOUT_DIRECTORY: &str = "ocr-layout";

/// Wire contract of the local Python bridge.  `bbox_norm` means the worker never has to infer
/// image resolution from a line or recover coordinates from unstructured OCR text.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaddleOcrOutput {
    schema_version: u8,
    ok: bool,
    engine: Option<String>,
    engine_version: Option<String>,
    image_width: Option<i32>,
    image_height: Option<i32>,
    lines: Option<Vec<PaddleOcrLine>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaddleOcrLine {
    text: String,
    confidence: f64,
    bbox_norm: [f64; 4],
}

#[tokio::main]
async fn main() -> ExitCode {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        // 与巡检 worker 同一条判据（COLLECTION-UPGRADE-001 · T29）：没有地址是配置错误，
        // 不是「没有活干」。以失败退出，让 supervisor 看见；用成功退出等于把故障说成正常。
        eprintln!("linggan media worker: LINGGAN_LOCAL_DATABASE_URL is not set; refusing to start");
        return ExitCode::FAILURE;
    };
    // 与巡检 worker 同一个启动事件（同一张字段表）：日志里带上 service 与 revision，
    // 一次故障才能被钉到某一次部署上。
    emit_startup_event();
    let database = connect_when_reachable(&url).await;
    let worker_instance_ref = Uuid::new_v4();
    let enabled_processors = enabled_processors();
    println!(
        "linggan media worker: local processors [{}] every {}s",
        enabled_processors.join(","),
        TICK_INTERVAL.as_secs()
    );
    // 认领闸的就绪状态，启动时报一次。
    //
    // **不能只把「应该停下来被人看见」写在注释里。** 未迁移或未登记时 `claim` 一律返回
    // `Ok(None)`，下面的循环把它读成「这一轮没有活」继续转，一行日志都不打——界面照常，
    // 进程照常，而它永远领不到任何一条。这正是本包要消灭的失败形态，闸门自己不能也这样藏起来。
    //
    // 只在启动时报：两个触发条件（迁移没跑、新处理器没登记）都在进程启动那一刻就已经确定，
    // 而每 tick 都报会把日志淹掉，反而没人看。
    match read_claim_gate_readiness(&database, &enabled_processors).await {
        Ok(ClaimGateReadiness::Ready) => {}
        Ok(ClaimGateReadiness::NotMigrated) => println!(
            "linggan media worker: the claim-gate table is missing (migration 0088 not applied): \
             no processor can be claimed, this process will idle forever"
        ),
        Ok(ClaimGateReadiness::Unregistered(kinds)) => println!(
            "linggan media worker: these processors are enabled here but have no concurrency row, \
             so they will never be claimed: {}",
            kinds.join(",")
        ),
        // 读不出来就交给下面的认领去报错，别在这里制造第二个错误源。
        Err(_) => {}
    }
    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    loop {
        ticker.tick().await;
        if let Err(error) = ensure_media_processing_work(&database).await {
            println!("linggan media worker: cannot project processing work: {error}");
            continue;
        }
        for _ in 0..MAX_JOBS_PER_TICK {
            let claim = match claim_media_processing_work(
                &database,
                worker_instance_ref,
                &enabled_processors,
            )
            .await
            {
                Ok(MediaProcessingClaimOutcome::Claimed(claim)) => claim,
                // 退休也要出声：一条一行，日志里数与库里数对得上。**不能**并进下面的
                // `Idle` 分支——那样这条永远不会有输入的作业会安静地消失，正是本包要消灭的形态。
                Ok(MediaProcessingClaimOutcome::Retired {
                    job_ref,
                    processor_kind,
                    reason,
                }) => {
                    println!(
                        "linggan media worker: {processor_kind} {job_ref} -> not_applicable ({reason})"
                    );
                    continue;
                }
                Ok(MediaProcessingClaimOutcome::Idle) => break,
                Err(error) => {
                    println!("linggan media worker: claim failed: {error}");
                    break;
                }
            };
            if let Err(error) = execute_claim(&database, worker_instance_ref, &claim).await {
                let state =
                    fail_media_processing_work(&database, &claim, worker_instance_ref, &error)
                        .await
                        .unwrap_or_else(|failure| format!("failure_record_error:{failure}"));
                println!(
                    "linggan media worker: {} {} -> {} ({})",
                    claim.processor_kind, claim.job_ref, state, error
                );
            }
        }
    }
}

/// 一次连接判定最多等多久。与巡检 worker 同一条道理：连接池有自己的耐心（sqlx 默认
/// `acquire_timeout` 30 秒，期间反复重试），那是连接池该有的，不是操作员该等的——本机数据库
/// 是毫秒级的，十秒还没有结果就是「这一轮没问到」，如实记下来、退避后再问。
///
/// 这道上限只影响**故障多久被说出来**，不影响恢复速度：数据库一回来，连接尝试马上成功。
const CONNECT_ATTEMPT_BUDGET: Duration = Duration::from_secs(10);

/// 连不上数据库时留在原地等它回来。
///
/// 以前这条路是「打印一行然后 return」——main 返回 `()` 就是退出码 0，对 supervisor 来说是
/// 一次干净退出，对看日志的人来说是这台机器什么都没说。改成退避重试：数据库回来这一轮就
/// 接上，进程不重启、日志不重灌。只在进入这个状态时说一次话。
///
/// 开场先说一句、再开始判定：没有这句，连不上时前 30 秒日志是空的，而空日志与「一台没事
/// 可做的机器」是同一个样子。打印的错误用 `StorageError` 的固定说法（「the database refused
/// a connection」），**不带连接串**——那里面有密码。
///
/// 这里不装信号处理器：这个进程没有在途预留要收，SIGTERM 的默认动作就是立即结束。
async fn connect_when_reachable(url: &str) -> linggan_storage_postgres::Database {
    println!("linggan media worker: started; checking whether the local database is reachable");
    let mut retry_after = READINESS_RETRY_START;
    let mut reported = false;
    loop {
        let attempt = tokio::time::timeout(
            CONNECT_ATTEMPT_BUDGET,
            linggan_storage_postgres::Database::connect(url),
        );
        match attempt.await {
            Ok(Ok(database)) => return database,
            Ok(Err(error)) => {
                if !reported {
                    println!(
                        "linggan media worker: cannot reach the local database ({error}); \
                         retrying with backoff instead of exiting"
                    );
                    reported = true;
                }
            }
            Err(_) => {
                if !reported {
                    println!(
                        "linggan media worker: the local database did not answer within {}s; \
                         retrying with backoff instead of exiting",
                        CONNECT_ATTEMPT_BUDGET.as_secs()
                    );
                    reported = true;
                }
            }
        }
        tokio::time::sleep(retry_after).await;
        retry_after = next_readiness_retry(retry_after);
    }
}

fn enabled_processors() -> Vec<String> {
    let mut processors = Vec::new();
    let paddle_ocr = command_available(&paddle_ocr_python()) && paddle_ocr_script().is_file();
    let ffmpeg = command_available(&ffmpeg_command());
    let whisper = command_available(&whisper_command());
    if paddle_ocr {
        processors.push("image_ocr".to_owned());
    }
    if ffmpeg {
        processors.push("thumbnail".to_owned());
        processors.push("audio_extract".to_owned());
    }
    if ffmpeg && whisper {
        processors.push("asr".to_owned());
    }
    if ffmpeg && paddle_ocr {
        processors.push("video_frame_ocr".to_owned());
    }
    processors
}

async fn execute_claim(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    match claim.processor_kind.as_str() {
        "image_ocr" => execute_image_ocr(database, worker_instance_ref, claim).await,
        "thumbnail" => execute_thumbnail(database, worker_instance_ref, claim).await,
        "audio_extract" => execute_audio_extract(database, worker_instance_ref, claim).await,
        "asr" => execute_asr(database, worker_instance_ref, claim).await,
        "video_frame_ocr" => execute_video_frame_ocr(database, worker_instance_ref, claim).await,
        other => Err(format!("processor_not_enabled:{other}")),
    }
}

async fn execute_image_ocr(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    if claim.processor_version == linggan_evidence::ocr_adoption_persistence::PROCESSOR_VERSION {
        return media_worker_support::ocr_vl::execute(database, worker_instance_ref, claim).await;
    }
    if !claim.mime_type.starts_with("image/") {
        return Err("image_ocr_mime_mismatch".to_owned());
    }
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    let output = run_paddle_ocr(&input, &claim.mime_type).await?;
    let Some(lines) = output.lines.as_ref() else {
        return Err("paddle_ocr_result_lines_missing".to_owned());
    };
    if lines.is_empty() {
        complete_media_processing_without_output(
            database,
            claim,
            worker_instance_ref,
            "no_text_observed",
        )
        .await
        .map_err(|error| error.to_string())?;
        println!(
            "linggan media worker: OCR observed no text {}",
            claim.job_ref
        );
        return Ok(());
    }
    let raw_text = lines
        .iter()
        .map(|line| line.text.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if raw_text.is_empty() {
        return Err("paddle_ocr_result_text_empty".to_owned());
    }
    let raw_bytes = raw_text.as_bytes();
    let raw_content_hash = sha256_hex(raw_bytes);
    let raw_storage_key = format!(
        "derivatives/{OCR_RAW_DIRECTORY}/{}/{raw_content_hash}.txt",
        claim.job_ref
    );
    let raw_output_path = prepare_output(&raw_storage_key)?;
    std::fs::write(&raw_output_path, raw_bytes)
        .map_err(|error| format!("ocr_raw_output_write_failed:{error}"))?;
    let layout_bytes = serde_json::to_vec(&output)
        .map_err(|error| format!("paddle_ocr_layout_serialize_failed:{error}"))?;
    let layout_content_hash = sha256_hex(&layout_bytes);
    let layout_storage_key = format!(
        "derivatives/{OCR_LAYOUT_DIRECTORY}/{}/{layout_content_hash}.json",
        claim.job_ref
    );
    let layout_output_path = prepare_output(&layout_storage_key)?;
    std::fs::write(&layout_output_path, &layout_bytes)
        .map_err(|error| format!("ocr_layout_output_write_failed:{error}"))?;
    let engine_version = output
        .engine_version
        .as_deref()
        .ok_or_else(|| "paddle_ocr_engine_version_missing".to_owned())?;
    let image_width = output
        .image_width
        .ok_or_else(|| "paddle_ocr_image_width_missing".to_owned())?;
    let image_height = output
        .image_height
        .ok_or_else(|| "paddle_ocr_image_height_missing".to_owned())?;
    let layering = layer_paddle_lines(lines);
    let completion = OcrCompletionInput {
        engine_version: engine_version.to_owned(),
        image_width,
        image_height,
        raw_text,
        raw_content_hash: raw_content_hash.clone(),
        raw_storage_key,
        layout_content_hash,
        layout_byte_size: i64::try_from(layout_bytes.len()).map_err(|error| error.to_string())?,
        layout_storage_key,
        lines: lines
            .iter()
            .map(|line| OcrLineInput {
                text: line.text.clone(),
                confidence: line.confidence,
                bbox_norm: line.bbox_norm,
            })
            .collect(),
        layering,
    };
    complete_media_processing_ocr(database, claim, worker_instance_ref, &completion)
        .await
        .map_err(|error| error.to_string())?;
    println!(
        "linggan media worker: Paddle OCR completed {} -> {}",
        claim.job_ref, raw_content_hash
    );
    Ok(())
}

/// PaddleOCR uses the filename suffix to decide whether an input is an image.  Linggan stores
/// blobs under their content hash, deliberately without a filename extension, so pass Paddle a
/// short-lived local link/copy whose suffix is derived from the admitted MIME fact.  The original
/// Blob remains the authority and is never renamed or rewritten.
async fn run_paddle_ocr(input: &Path, mime_type: &str) -> Result<PaddleOcrOutput, String> {
    let extension = paddle_image_extension(mime_type)
        .ok_or_else(|| format!("paddle_ocr_unsupported_mime:{mime_type}"))?;
    let staged_input = stage_paddle_input(input, extension)?;
    let result = run_paddle_ocr_staged(&staged_input).await;
    // A staging file is an execution scratch artifact, not a media materialization.  It must not
    // survive the attempt (whether Paddle accepts, rejects, or times out on the input).
    let _ = std::fs::remove_file(&staged_input);
    result
}

fn stage_paddle_input(input: &Path, extension: &str) -> Result<std::path::PathBuf, String> {
    let staged_input =
        std::env::temp_dir().join(format!("linggan-paddle-{}.{}", Uuid::new_v4(), extension));
    stage_paddle_input_at(input, &staged_input)?;
    Ok(staged_input)
}

fn stage_paddle_input_at(input: &Path, staged_input: &Path) -> Result<(), String> {
    if let Err(link_error) = std::fs::hard_link(input, &staged_input) {
        if let Err(copy_error) = std::fs::copy(input, &staged_input) {
            // A failed copy may have created a partial file.  Delete it before returning so a
            // retry never leaves raw media in the system temporary directory.
            let _ = std::fs::remove_file(staged_input);
            return Err(format!(
                "paddle_ocr_input_stage_failed:{link_error};{copy_error}"
            ));
        }
    }
    Ok(())
}

fn paddle_image_extension(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/bmp" | "image/x-ms-bmp" => Some("bmp"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/tiff" => Some("tiff"),
        "image/x-portable-bitmap" => Some("pbm"),
        "image/x-portable-graymap" => Some("pgm"),
        "image/x-portable-pixmap" => Some("ppm"),
        "image/x-portable-anymap" => Some("pnm"),
        _ => None,
    }
}

async fn run_paddle_ocr_staged(input: &Path) -> Result<PaddleOcrOutput, String> {
    let script = paddle_ocr_script();
    if !script.is_file() {
        return Err("paddle_ocr_script_missing".to_owned());
    }
    let mut command = Command::new(paddle_ocr_python());
    command.arg(script).arg(input);
    let output = run_command(&mut command, PROCESS_TIMEOUT).await?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(500)
            .collect::<String>();
        return Err(format!("paddle_ocr_failed:{}", detail.trim()));
    }
    if output.stdout.len() > 2_000_000 {
        return Err("paddle_ocr_result_too_large".to_owned());
    }
    let parsed: PaddleOcrOutput = serde_json::from_slice(&output.stdout)
        .map_err(|_| "paddle_ocr_result_invalid_json".to_owned())?;
    if parsed.schema_version != 1 || !parsed.ok || parsed.engine.as_deref() != Some("paddleocr") {
        return Err(format!(
            "paddle_ocr_result_rejected:{}",
            parsed
                .error
                .unwrap_or_else(|| "schema_or_engine".to_owned())
        ));
    }
    Ok(parsed)
}

fn layer_paddle_lines(lines: &[PaddleOcrLine]) -> OcrLayeringInput {
    linggan_evidence::ocr_layering::layer_lines(
        &lines
            .iter()
            .map(|line| OcrLineInput {
                text: line.text.clone(),
                confidence: line.confidence,
                bbox_norm: line.bbox_norm,
            })
            .collect::<Vec<_>>(),
    )
}

async fn execute_thumbnail(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    let storage_key = format!("derivatives/thumbnail/{}/preview.jpg", claim.job_ref);
    let output_path = prepare_output(&storage_key)?;
    let mut command = Command::new(ffmpeg_command());
    command
        .args(["-y", "-i"])
        .arg(&input)
        .args(["-vf", "thumbnail,scale='min(960,iw)':-2", "-frames:v", "1"])
        .arg(&output_path);
    require_success(
        run_command(&mut command, PROCESS_TIMEOUT).await?,
        "thumbnail",
    )?;
    complete_binary_derivative(
        database,
        worker_instance_ref,
        claim,
        "thumbnail",
        &storage_key,
        &output_path,
    )
    .await
}

async fn execute_audio_extract(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    if !claim.mime_type.starts_with("video/") {
        return Err("audio_extract_mime_mismatch".to_owned());
    }
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    let storage_key = format!("derivatives/audio/{}/audio.wav", claim.job_ref);
    let output_path = prepare_output(&storage_key)?;
    let mut command = Command::new(ffmpeg_command());
    command
        .args(["-y", "-i"])
        .arg(&input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(&output_path);
    require_success(
        run_command(&mut command, PROCESS_TIMEOUT).await?,
        "audio_extract",
    )?;
    complete_binary_derivative(
        database,
        worker_instance_ref,
        claim,
        "audio",
        &storage_key,
        &output_path,
    )
    .await
}

async fn execute_asr(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    if !claim.mime_type.starts_with("video/") && !claim.mime_type.starts_with("audio/") {
        return Err("asr_mime_mismatch".to_owned());
    }
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    if !has_audio_stream(&input).await? {
        complete_media_processing_without_output(
            database,
            claim,
            worker_instance_ref,
            "no_audio_stream_observed",
        )
        .await
        .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let output_dir = local_media_root().join(format!("derivatives/asr/{}", claim.job_ref));
    // The job's previous timed-out attempt must not supply this attempt's transcript.
    let _ = std::fs::remove_dir_all(&output_dir);
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| format!("asr_output_directory_failed:{error}"))?;
    let mut command = Command::new(whisper_command());
    apply_ffmpeg_path(&mut command)?;
    command
        .arg(&input)
        .args([
            "--model",
            &whisper_model(),
            "--output_format",
            "json",
            "--output_dir",
        ])
        .arg(&output_dir)
        .args(["--fp16", "False"])
        .args(["--initial_prompt", ASR_INITIAL_PROMPT]);
    require_success(run_command(&mut command, ASR_PROCESS_TIMEOUT).await?, "asr")?;
    let transcript = first_json_file(&output_dir)?;
    let raw = std::fs::read_to_string(&transcript)
        .map_err(|error| format!("asr_output_read_failed:{error}"))?;
    let blocks = media_worker_support::timed_text::asr_blocks(&raw)?;
    let text = blocks
        .iter()
        .map(linggan_evidence::research_dossier::content::block_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        complete_media_processing_without_output(
            database,
            claim,
            worker_instance_ref,
            "no_speech_observed",
        )
        .await
        .map_err(|error| error.to_string())?;
        return Ok(());
    }
    complete_text_derivative(
        database,
        worker_instance_ref,
        claim,
        "asr_text",
        "asr",
        &text,
        Some("zh-Hans"),
        linggan_evidence::material_reading::producers::ReadingContent { blocks,issues:vec![],source_state:"present".into(),
            source_location:serde_json::json!({"rawTranscriptSha256":sha256_hex(raw.as_bytes()),"timing":"whisper_segments"}) },
    )
    .await
}

async fn execute_video_frame_ocr(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    if !claim.mime_type.starts_with("video/") {
        return Err("video_frame_ocr_mime_mismatch".to_owned());
    }
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    let frame_dir =
        local_media_root().join(format!("derivatives/frame-ocr/{}/frames", claim.job_ref));
    if frame_dir.exists() {
        std::fs::remove_dir_all(&frame_dir).map_err(|e| format!("frame_cleanup_failed:{e}"))?;
    }
    std::fs::create_dir_all(&frame_dir).map_err(|e| format!("frame_directory_failed:{e}"))?;
    let pattern = frame_dir.join("frame-%09d.jpg");
    let mut ffmpeg = Command::new(ffmpeg_command());
    ffmpeg.args(["-y","-i"]).arg(&input)
        .args(["-vf","select='isnan(prev_selected_t)+gte(t-prev_selected_t,10)',showinfo,scale='min(1280,iw)':-2","-vsync","vfr"])
        .arg(&pattern);
    let extracted = run_command(&mut ffmpeg, PROCESS_TIMEOUT).await?;
    let timestamps = media_worker_support::timed_text::sampled_pts(&extracted.stderr)?;
    require_success(extracted, "video_frame_extract")?;
    let mut frames = std::fs::read_dir(&frame_dir)
        .map_err(|e| format!("frame_read_failed:{e}"))?
        .filter_map(Result::ok)
        .map(|f| f.path())
        .filter(|f| f.extension().is_some_and(|e| e == "jpg"))
        .collect::<Vec<_>>();
    frames.sort();
    if frames.len() != timestamps.len() {
        return Err("frame_timestamp_count_mismatch".into());
    }
    let mut blocks = vec![];
    for (frame_index, (frame, pts)) in frames.iter().zip(&timestamps).enumerate() {
        let output = run_paddle_ocr(frame, "image/jpeg").await?;
        let lines:Vec<_>=output.lines.unwrap_or_default().iter().map(|l|serde_json::json!({"text":l.text,"confidence":l.confidence,
            "bbox":[l.bbox_norm[0],l.bbox_norm[1],l.bbox_norm[2]-l.bbox_norm[0],l.bbox_norm[3]-l.bbox_norm[1]]})).collect();
        let mut frame_blocks = linggan_evidence::research_dossier::content::ocr_blocks(&lines);
        if frame_blocks.is_empty() {
            frame_blocks.push(serde_json::json!({"kind":"gap","speaker":"unconfirmed","text":"〔此抽样帧未返回文字〕","issues":[{"key":"recognition_empty","code":"recognition_empty","message":"未返回文字不证明此帧没有文字","resolution":"reading_edit"}]}));
        }
        for block in &mut frame_blocks {
            block["time"] = serde_json::json!({"startMs":pts,"endMs":pts});
            if let Some(issues) = block
                .get_mut("issues")
                .and_then(serde_json::Value::as_array_mut)
            {
                for issue in issues {
                    issue["key"] = serde_json::json!(format!(
                        "frame_{frame_index}:{}",
                        issue["key"].as_str().expect("OCR issue key")
                    ));
                }
            }
        }
        blocks.extend(frame_blocks);
    }
    if blocks.is_empty() {
        return Err("video_frames_not_acquired".into());
    }
    let text = blocks
        .iter()
        .map(linggan_evidence::research_dossier::content::block_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    complete_text_derivative(database,worker_instance_ref,claim,"frame_ocr_text","frame-ocr",&text,Some("zh-Hans"),
        linggan_evidence::material_reading::producers::ReadingContent {blocks,issues:vec![serde_json::json!({"key":"sampled_frames","code":"sampling_incomplete","message":"仅覆盖实际抽样帧，未逐帧检查完整视频","resolution":"source_refresh"})],source_state:"present".into(),
            source_location:serde_json::json!({"sampledPtsMs":timestamps,"frameCount":frames.len(),"sampling":"at_least_10_seconds_between_selected_source_frames"})}).await
}

async fn has_audio_stream(input: &Path) -> Result<bool, String> {
    let command = std::env::var("LINGGAN_FFPROBE_COMMAND").unwrap_or_else(|_| {
        std::path::PathBuf::from(ffmpeg_command())
            .with_file_name("ffprobe")
            .to_string_lossy()
            .into_owned()
    });
    let output = run_command(
        Command::new(command)
            .args([
                "-v",
                "error",
                "-select_streams",
                "a",
                "-show_entries",
                "stream=index",
                "-of",
                "json",
            ])
            .arg(input),
        PROCESS_TIMEOUT,
    )
    .await?;
    if !output.status.success() {
        return Err("audio_stream_probe_failed".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|_| "audio_stream_probe_invalid")?;
    Ok(!value["streams"]
        .as_array()
        .ok_or("audio_stream_probe_invalid")?
        .is_empty())
}

async fn complete_binary_derivative(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
    derivative_kind: &str,
    storage_key: &str,
    output_path: &Path,
) -> Result<(), String> {
    let bytes =
        std::fs::read(output_path).map_err(|error| format!("derivative_read_failed:{error}"))?;
    if bytes.is_empty() {
        return Err("derivative_empty".to_owned());
    }
    complete_media_processing_derivative(
        database,
        claim,
        worker_instance_ref,
        derivative_kind,
        &sha256_hex(&bytes),
        i64::try_from(bytes.len()).map_err(|error| error.to_string())?,
        storage_key,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

async fn complete_text_derivative(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
    derivative_kind: &str,
    directory: &str,
    text: &str,
    language_tag: Option<&str>,
    reading: linggan_evidence::material_reading::producers::ReadingContent,
) -> Result<(), String> {
    let bytes = text.as_bytes();
    let content_hash = sha256_hex(bytes);
    let storage_key = format!(
        "derivatives/{directory}/{}/{content_hash}.txt",
        claim.job_ref
    );
    let output_path = prepare_output(&storage_key)?;
    std::fs::write(&output_path, bytes)
        .map_err(|error| format!("text_output_write_failed:{error}"))?;
    let display_text: String = text.chars().take(MAX_DISPLAY_CHARS).collect();
    complete_media_processing_text(
        database,
        claim,
        worker_instance_ref,
        derivative_kind,
        &content_hash,
        i64::try_from(bytes.len()).map_err(|error| error.to_string())?,
        &storage_key,
        text,
        &display_text,
        language_tag,
        reading,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PaddleOcrLine, layer_paddle_lines, paddle_image_extension, stage_paddle_input_at};
    use uuid::Uuid;

    fn line(text: &str, confidence: f64, bbox_norm: [f64; 4]) -> PaddleOcrLine {
        PaddleOcrLine {
            text: text.to_owned(),
            confidence,
            bbox_norm,
        }
    }

    #[test]
    fn cover_headline_keeps_author_text_and_excludes_an_edge_watermark() {
        let output = layer_paddle_lines(&[
            line("不要带 A 娃", 0.99, [0.12, 0.68, 0.81, 0.75]),
            line("吊在一棵树上！", 0.98, [0.14, 0.76, 0.84, 0.84]),
            line("小红书", 0.99, [0.84, 0.92, 0.98, 0.98]),
        ]);
        assert_eq!(
            output.cover_headline.as_deref(),
            Some("不要带 A 娃 吊在一棵树上！")
        );
        assert_eq!(output.state, "ACCEPTED");
        assert_eq!(output.excluded_lines.len(), 1);
        assert_eq!(
            output.excluded_lines[0].classification,
            "platform_watermark"
        );
    }

    #[test]
    fn platform_comment_chrome_is_excluded_without_erasing_the_comment_text() {
        let output = layer_paddle_lines(&[
            line(
                "大猫给乐乐输血，说大猫很勇敢",
                0.96,
                [0.10, 0.28, 0.88, 0.31],
            ),
            line("但是猫没有选择的权利", 0.97, [0.10, 0.37, 0.82, 0.40]),
            line("2天前", 0.99, [0.08, 0.84, 0.20, 0.86]),
            line("作者赞过", 0.99, [0.73, 0.84, 0.94, 0.86]),
        ]);
        assert_eq!(output.cover_headline, None);
        assert_eq!(output.image_substantive_text, None);
        assert_eq!(output.state, "PARTIAL");
        assert_eq!(output.retained_ordinals, vec![0, 1]);
        assert_eq!(output.excluded_lines.len(), 2);
        assert!(
            output
                .excluded_lines
                .iter()
                .all(|line| line.classification == "platform_ui")
        );
    }

    #[test]
    fn dense_competing_text_is_marked_partial_instead_of_being_silently_deleted() {
        let output = layer_paddle_lines(&[
            line("不要带 A 娃", 0.99, [0.12, 0.68, 0.81, 0.75]),
            line("吊在一棵树上！", 0.98, [0.14, 0.76, 0.84, 0.84]),
            line("Read the text and answer", 0.93, [0.08, 0.16, 0.78, 0.20]),
            line(
                "You must stop at a red light",
                0.94,
                [0.08, 0.24, 0.80, 0.28],
            ),
            line("On foot", 0.96, [0.08, 0.32, 0.28, 0.36]),
            line("小红书", 0.99, [0.84, 0.92, 0.98, 0.98]),
        ]);
        assert_eq!(output.state, "PARTIAL");
        assert_eq!(
            output.cover_headline.as_deref(),
            Some("不要带 A 娃 吊在一棵树上！")
        );
        assert!(output.image_substantive_text.is_none());
    }

    #[test]
    fn paddle_input_suffix_is_derived_only_from_supported_admitted_image_mime() {
        assert_eq!(paddle_image_extension("image/webp"), Some("webp"));
        assert_eq!(paddle_image_extension("image/jpeg"), Some("jpg"));
        assert_eq!(paddle_image_extension("image/png"), Some("png"));
        assert_eq!(paddle_image_extension("image/gif"), None);
        assert_eq!(paddle_image_extension("video/mp4"), None);
    }

    #[test]
    fn failed_paddle_input_copy_removes_a_partial_staging_file() {
        let root = std::env::temp_dir().join(format!("linggan-paddle-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("test root");
        let source_directory = root.join("source-directory");
        std::fs::create_dir(&source_directory).expect("source directory");
        let staged_input = root.join("partial.webp");
        std::fs::write(&staged_input, b"partial").expect("partial staging file");

        let result = stage_paddle_input_at(&source_directory, &staged_input);

        assert!(result.is_err());
        assert!(
            !staged_input.exists(),
            "failed staging must not leave a partial temporary file"
        );
        std::fs::remove_dir_all(&root).expect("test cleanup");
    }
}
