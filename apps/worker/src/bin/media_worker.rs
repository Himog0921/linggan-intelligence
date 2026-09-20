//! Separate, bounded local media processor.
//!
//! This process never visits a platform. It only consumes blobs that the Browser Producer has
//! already admitted and materialized under `LINGGAN_LOCAL_MEDIA_ROOT`.

use linggan_evidence::{
    ClaimGateReadiness, MediaProcessingClaim, MediaProcessingClaimOutcome, OcrCompletionInput,
    OcrExcludedLineInput, OcrLayeringInput, OcrLineInput, claim_media_processing_work,
    complete_media_processing_derivative, complete_media_processing_ocr,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work, read_claim_gate_readiness,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

mod media_worker_support;

use media_worker_support::{
    apply_ffmpeg_path, command_available, ensure_local_input, ffmpeg_command, first_text_file,
    local_media_root, normalize_text, paddle_ocr_python, paddle_ocr_script, prepare_output,
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
async fn main() {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        println!("linggan media worker: LINGGAN_LOCAL_DATABASE_URL is not set");
        return;
    };
    let database = match linggan_storage_postgres::Database::connect(&url).await {
        Ok(database) => database,
        Err(error) => {
            println!("linggan media worker: cannot reach the local database: {error}");
            return;
        }
    };
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
    if !claim.mime_type.starts_with("image/") {
        return Err("image_ocr_mime_mismatch".to_owned());
    }
    let input = local_media_root().join(&claim.storage_key);
    ensure_local_input(&input)?;
    let output = run_paddle_ocr(&input, &claim.mime_type)?;
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
fn run_paddle_ocr(input: &Path, mime_type: &str) -> Result<PaddleOcrOutput, String> {
    let extension = paddle_image_extension(mime_type)
        .ok_or_else(|| format!("paddle_ocr_unsupported_mime:{mime_type}"))?;
    let staged_input =
        std::env::temp_dir().join(format!("linggan-paddle-{}.{}", Uuid::new_v4(), extension));
    if let Err(link_error) = std::fs::hard_link(input, &staged_input) {
        std::fs::copy(input, &staged_input).map_err(|copy_error| {
            format!("paddle_ocr_input_stage_failed:{link_error};{copy_error}")
        })?;
    }
    let result = run_paddle_ocr_staged(&staged_input);
    // A staging file is an execution scratch artifact, not a media materialization.  It must not
    // survive the attempt (whether Paddle accepts, rejects, or times out on the input).
    let _ = std::fs::remove_file(&staged_input);
    result
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

fn run_paddle_ocr_staged(input: &Path) -> Result<PaddleOcrOutput, String> {
    let script = paddle_ocr_script();
    if !script.is_file() {
        return Err("paddle_ocr_script_missing".to_owned());
    }
    let mut command = Command::new(paddle_ocr_python());
    command.arg(script).arg(input);
    let output = run_command(&mut command, PROCESS_TIMEOUT)?;
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

/// The first local pass only removes narrowly-identifiable platform chrome.  It deliberately
/// does not try to decide whether a photographed page, a logo, or a quote card is substantive;
/// those ambiguous lines remain raw evidence and put the item in `PARTIAL` for the later vision
/// selector.  This avoids silently treating a visual guess as a fact.
fn layer_paddle_lines(lines: &[PaddleOcrLine]) -> OcrLayeringInput {
    let mut retained = Vec::new();
    let mut excluded = Vec::new();
    for (ordinal, line) in lines.iter().enumerate() {
        let compact = line
            .text
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        let at_edge = line.bbox_norm[1] <= 0.14
            || line.bbox_norm[3] >= 0.86
            || line.bbox_norm[0] <= 0.08
            || line.bbox_norm[2] >= 0.92;
        let classification =
            if (compact == "小红书" || compact.eq_ignore_ascii_case("xiaohongshu")) && at_edge {
                Some(("platform_watermark", "edge_platform_watermark"))
            } else if is_platform_ui_line(&compact) {
                Some(("platform_ui", "platform_ui_metadata"))
            } else if line.confidence < 0.35 {
                Some(("low_confidence", "below_local_confidence_floor"))
            } else {
                None
            };
        if let Some((classification, reason)) = classification {
            excluded.push(OcrExcludedLineInput {
                ordinal,
                classification: classification.to_owned(),
                reason: reason.to_owned(),
            });
        } else {
            retained.push(ordinal);
        }
    }

    let headline_ordinals = select_local_headline(lines, &retained);
    let cover_headline = (!headline_ordinals.is_empty()).then(|| {
        headline_ordinals
            .iter()
            .map(|ordinal| lines[*ordinal].text.trim())
            .collect::<Vec<_>>()
            .join(" ")
    });
    let non_headline_count = retained
        .iter()
        .filter(|ordinal| !headline_ordinals.contains(ordinal))
        .count();
    let single_unambiguous_line = headline_ordinals.is_empty()
        && retained.len() == 1
        && lines[retained[0]]
            .text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count()
            >= 6
        && lines[retained[0]].confidence >= 0.75
        && lines[retained[0]].bbox_norm[1] >= 0.08
        && lines[retained[0]].bbox_norm[3] <= 0.92;
    let image_substantive_text = if single_unambiguous_line {
        Some(
            retained
                .iter()
                .map(|ordinal| lines[*ordinal].text.trim())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    } else {
        None
    };
    let state = if retained.is_empty() {
        "NEEDS_REVIEW"
    } else if single_unambiguous_line || (!headline_ordinals.is_empty() && non_headline_count <= 2)
    {
        "ACCEPTED"
    } else {
        "PARTIAL"
    };
    OcrLayeringInput {
        state: state.to_owned(),
        cover_headline,
        image_substantive_text,
        retained_ordinals: retained,
        headline_ordinals,
        excluded_lines: excluded,
    }
}

fn is_platform_ui_line(compact: &str) -> bool {
    matches!(
        compact,
        "作者赞过" | "回复" | "展开更多回复" | "发布于" | "编辑于"
    ) || (compact.ends_with("分钟前") || compact.ends_with("小时前") || compact.ends_with("天前"))
        || (compact.starts_with("回复") && compact.chars().count() <= 12)
}

fn select_local_headline(lines: &[PaddleOcrLine], retained: &[usize]) -> Vec<usize> {
    let candidate = retained
        .iter()
        .copied()
        .filter(|ordinal| {
            let line = &lines[*ordinal];
            let width = line.bbox_norm[2] - line.bbox_norm[0];
            let height = line.bbox_norm[3] - line.bbox_norm[1];
            line.confidence >= 0.50 && width >= 0.24 && height >= 0.045
        })
        .max_by(|left, right| {
            headline_score(&lines[*left]).total_cmp(&headline_score(&lines[*right]))
        });
    let Some(anchor) = candidate else {
        return Vec::new();
    };
    let anchor_line = &lines[anchor];
    let anchor_height = anchor_line.bbox_norm[3] - anchor_line.bbox_norm[1];
    let mut selected = retained
        .iter()
        .copied()
        .filter(|ordinal| {
            let line = &lines[*ordinal];
            let height = line.bbox_norm[3] - line.bbox_norm[1];
            let close_vertically = line.bbox_norm[1] >= anchor_line.bbox_norm[1] - 0.09
                && line.bbox_norm[3] <= anchor_line.bbox_norm[3] + 0.13;
            let similar_scale = height >= anchor_height * 0.60 && height <= anchor_height * 1.55;
            let broad_enough = line.bbox_norm[2] - line.bbox_norm[0] >= 0.20;
            close_vertically && similar_scale && broad_enough
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        lines[*left].bbox_norm[1]
            .total_cmp(&lines[*right].bbox_norm[1])
            .then_with(|| lines[*left].bbox_norm[0].total_cmp(&lines[*right].bbox_norm[0]))
    });
    selected.truncate(4);
    selected
}

fn headline_score(line: &PaddleOcrLine) -> f64 {
    let width = line.bbox_norm[2] - line.bbox_norm[0];
    let height = line.bbox_norm[3] - line.bbox_norm[1];
    height * 0.75 + width * 0.20 + line.confidence * 0.05
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
    require_success(run_command(&mut command, PROCESS_TIMEOUT)?, "thumbnail")?;
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
    require_success(run_command(&mut command, PROCESS_TIMEOUT)?, "audio_extract")?;
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
    let output_dir = local_media_root().join(format!("derivatives/asr/{}", claim.job_ref));
    // 先清空再建。`first_text_file` 取的是目录里**第一个** txt，所以上一次被超时杀掉留下的
    // 半截转写，会被这一次当成完整结果读走。此前从没撞上，是因为 whisper 一次都没跑成过；
    // 换成 medium 之后单条耗时涨到十几分钟、逼近超时，这条路径才真正可达。
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
            "txt",
            "--output_dir",
        ])
        .arg(&output_dir)
        .args(["--fp16", "False"])
        .args(["--initial_prompt", ASR_INITIAL_PROMPT]);
    require_success(run_command(&mut command, ASR_PROCESS_TIMEOUT)?, "asr")?;
    let transcript = first_text_file(&output_dir)?;
    let text = normalize_text(
        &std::fs::read_to_string(&transcript)
            .map_err(|error| format!("asr_output_read_failed:{error}"))?,
    );
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
    std::fs::create_dir_all(&frame_dir)
        .map_err(|error| format!("frame_output_directory_failed:{error}"))?;
    let pattern = frame_dir.join("frame-%03d.jpg");
    let mut ffmpeg = Command::new(ffmpeg_command());
    ffmpeg
        .args(["-y", "-i"])
        .arg(&input)
        .args(["-vf", "fps=1/10,scale='min(1280,iw)':-2", "-frames:v", "12"])
        .arg(&pattern);
    require_success(
        run_command(&mut ffmpeg, PROCESS_TIMEOUT)?,
        "video_frame_extract",
    )?;
    let mut texts = Vec::new();
    let mut frames = std::fs::read_dir(&frame_dir)
        .map_err(|error| format!("frame_directory_read_failed:{error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    frames.sort();
    for frame in frames {
        let output = run_paddle_ocr(&frame, "image/jpeg")?;
        let text = output
            .lines
            .unwrap_or_default()
            .iter()
            .map(|line| line.text.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() && !texts.contains(&text) {
            texts.push(text);
        }
    }
    let text = texts.join("\n");
    if text.is_empty() {
        complete_media_processing_without_output(
            database,
            claim,
            worker_instance_ref,
            "no_frame_text_observed",
        )
        .await
        .map_err(|error| error.to_string())?;
        return Ok(());
    }
    complete_text_derivative(
        database,
        worker_instance_ref,
        claim,
        "frame_ocr_text",
        "frame-ocr",
        &text,
        Some("zh-Hans"),
    )
    .await
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
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PaddleOcrLine, layer_paddle_lines, paddle_image_extension};

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
}
