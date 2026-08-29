//! Separate, bounded local media processor.
//!
//! This process never visits a platform. It only consumes blobs that the Browser Producer has
//! already admitted and materialized under `LINGGAN_LOCAL_MEDIA_ROOT`.

use linggan_evidence::{
    MediaProcessingClaim, claim_media_processing_work, complete_media_processing_derivative,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work,
};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

mod media_worker_support;

use media_worker_support::{
    command_available, ensure_local_input, ffmpeg_command, first_text_file, local_media_root,
    normalize_text, prepare_output, require_success, run_command, sha256_hex, tesseract_command,
    whisper_command, whisper_model,
};

const TICK_INTERVAL: Duration = Duration::from_secs(5);
const MAX_JOBS_PER_TICK: usize = 4;
const MAX_DISPLAY_CHARS: usize = 600;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(15 * 60);

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
                Ok(Some(claim)) => claim,
                Ok(None) => break,
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
        .args(["-l", "chi_sim+chi_tra+eng", "--psm", "11"]);
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
    std::fs::create_dir_all(&output_dir)
        .map_err(|error| format!("asr_output_directory_failed:{error}"))?;
    let mut command = Command::new(whisper_command());
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
        .args(["--fp16", "False"]);
    require_success(run_command(&mut command, PROCESS_TIMEOUT)?, "asr")?;
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
            .args(["-l", "chi_sim+chi_tra+eng", "--psm", "11"]);
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
