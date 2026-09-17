//! Separate, bounded local media processor.
//!
//! This process never visits a platform. It only consumes blobs that the Browser Producer has
//! already admitted and materialized under `LINGGAN_LOCAL_MEDIA_ROOT`.

use linggan_evidence::{
    ClaimGateReadiness, MediaProcessingClaim, MediaProcessingClaimOutcome,
    claim_media_processing_work, complete_media_processing_derivative,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work, read_claim_gate_readiness,
};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

mod media_worker_support;

use media_worker_support::{
    apply_ffmpeg_path, command_available, ensure_local_input, ffmpeg_command, first_text_file,
    local_media_root, normalize_text, prepare_output, require_success, run_command, sha256_hex,
    tesseract_command, whisper_command, whisper_model,
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

/// Tesseract 的识别语言与分页方式。两处调用（单图、视频抽帧）共用一份。
///
/// **`chi_tra` 不能加回来。**原先写作 `chi_sim+chi_tra+eng`，理由是「万一是繁体」；实际效果
/// 是 Tesseract 会**按区域自己挑一种**，于是一张简体封面被逐行拆成简繁混排——同一张图上
/// 「家长」和「家長」并存。2026-09-16 在生产数据上量过：441 条 image_ocr 产出含繁体专有字，
/// 随机 40 条用 `chi_sim+eng` 复跑，**39 条繁体减少、0 条增加**，繁体字总数 222 → 27；
/// 典型一条 `在進行單位換算時` → `在进行单位换算时`。
///
/// **这组数字量的是繁体字数，不是识别准不准，别读成「OCR 变准了」。**换语言表会换掉整段的
/// 识别结果，不只是把繁体折成简体：抽样里有一张电影海报，加 `chi_tra` 时读出「央視推 / 導演: 李」，
/// 去掉后读出「RTE / 主演:」，繁体确实少了，但那两处谁也没读对。所以这里能据以说的是
/// 「繁体串扰消失了」，不是「字认得更多了」。
///
/// 代价先说清楚：**内容本身是繁体的图，会被转写成简体**。来源语料是大陆平台，简体的占绝
/// 大多数，所以这个方向是对的；但 `chi_sim` 本身仍会零星吐繁体（那 27 个字就是），这是改善
/// 而非根治。真要彻底消掉，得在识别之后加一道繁转简，那是另一次决定。
///
/// `--psm 11`（稀疏文本）保持基线取值不动，本包没碰它。**这一条没有量过**：`--psm 6` 在这个
/// 仓库的历史里从没出现过，本包也没做 6/3/11 的对照，所以不要从这里读出一个「11 更好」的结论。
const TESSERACT_LANGUAGE_ARGS: [&str; 4] = ["-l", "chi_sim+eng", "--psm", "11"];

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
    let tesseract = command_available(&tesseract_command());
    let ffmpeg = command_available(&ffmpeg_command());
    let whisper = command_available(&whisper_command());
    if tesseract {
        processors.push("image_ocr".to_owned());
    }
    if ffmpeg {
        processors.push("thumbnail".to_owned());
        processors.push("audio_extract".to_owned());
    }
    if ffmpeg && whisper {
        processors.push("asr".to_owned());
    }
    if ffmpeg && tesseract {
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
    let mut command = Command::new(tesseract_command());
    command
        .arg(&input)
        .arg("stdout")
        .args(TESSERACT_LANGUAGE_ARGS);
    let output = run_command(&mut command, PROCESS_TIMEOUT)?;
    if !output.status.success() {
        return Err(format!(
            "tesseract_failed:{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = normalize_text(&String::from_utf8_lossy(&output.stdout));
    if text.is_empty() {
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
    let bytes = text.as_bytes();
    let content_hash = sha256_hex(bytes);
    let storage_key = format!("derivatives/ocr/{}/{content_hash}.txt", claim.job_ref);
    let output_path = local_media_root().join(&storage_key);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("ocr_output_directory_failed:{error}"))?;
    }
    std::fs::write(&output_path, bytes)
        .map_err(|error| format!("ocr_output_write_failed:{error}"))?;
    let display_text: String = text.chars().take(MAX_DISPLAY_CHARS).collect();
    complete_media_processing_text(
        database,
        claim,
        worker_instance_ref,
        "ocr_text",
        &content_hash,
        i64::try_from(bytes.len()).map_err(|error| error.to_string())?,
        &storage_key,
        &text,
        &display_text,
        Some("zh-Hans"),
    )
    .await
    .map_err(|error| error.to_string())?;
    println!(
        "linggan media worker: OCR completed {} -> {}",
        claim.job_ref, content_hash
    );
    Ok(())
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
        let mut tesseract = Command::new(tesseract_command());
        tesseract
            .arg(&frame)
            .arg("stdout")
            .args(TESSERACT_LANGUAGE_ARGS);
        let output = run_command(&mut tesseract, PROCESS_TIMEOUT)?;
        if output.status.success() {
            let text = normalize_text(&String::from_utf8_lossy(&output.stdout));
            if !text.is_empty() && !texts.contains(&text) {
                texts.push(text);
            }
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
