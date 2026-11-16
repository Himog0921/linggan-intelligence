//! Separate, bounded local media processor.
//!
//! This process never visits a platform. It only consumes blobs that the Browser Producer has
//! already admitted and materialized under `LINGGAN_LOCAL_MEDIA_ROOT`.

use linggan_evidence::{
    ClaimGateReadiness, MediaProcessingClaim, MediaProcessingClaimOutcome, READINESS_RETRY_START,
    claim_media_processing_work, complete_media_processing_derivative,
    complete_media_processing_text, complete_media_processing_without_output,
    ensure_media_processing_work, fail_media_processing_work, next_readiness_retry,
    read_claim_gate_readiness,
};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

mod media_worker_support;

use media_worker_support::{
    apply_ffmpeg_path, command_available, emit_startup_event, ensure_local_input, ffmpeg_command,
    first_json_file, local_media_root, prepare_output, require_success, run_command, sha256_hex,
    whisper_command, whisper_model,
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

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let batch = match args.as_slice() {
        [] => None,
        [flag, path] if flag == "--jobs-file" => match std::fs::read(path)
            .ok()
            .and_then(|raw| serde_json::from_slice::<Vec<Uuid>>(&raw).ok())
        {
            Some(jobs) if !jobs.is_empty() => Some(jobs),
            _ => {
                eprintln!("jobs file must contain a nonempty JSON array of job UUIDs");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("usage: linggan-media-worker [--jobs-file path]");
            return ExitCode::from(2);
        }
    };
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
    if let Some(jobs) = batch {
        return run_job_batch(&database, worker_instance_ref, &enabled_processors, jobs).await;
    }
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

/// Consume each named job at most once. No projection, polling or automatic retries.
async fn run_job_batch(
    database: &linggan_storage_postgres::Database,
    worker: Uuid,
    processors: &[String],
    mut jobs: Vec<Uuid>,
) -> ExitCode {
    jobs.sort_unstable();
    jobs.dedup();
    let mut failed = false;
    while !jobs.is_empty() {
        let outcome =
            linggan_evidence::claim_media_processing_jobs(database, worker, processors, &jobs)
                .await;
        match outcome {
            Ok(MediaProcessingClaimOutcome::Claimed(claim)) => {
                jobs.retain(|job| *job != claim.job_ref);
                match execute_claim(database, worker, &claim).await {
                    Ok(()) => {
                        println!("batch {} {} succeeded", claim.processor_kind, claim.job_ref)
                    }
                    Err(error) => {
                        failed = true;
                        let recorded =
                            fail_media_processing_work(database, &claim, worker, &error).await;
                        eprintln!(
                            "batch {} failed: {error}; recorded={recorded:?}",
                            claim.job_ref
                        );
                    }
                }
            }
            Ok(MediaProcessingClaimOutcome::Retired {
                job_ref, reason, ..
            }) => {
                jobs.retain(|job| *job != job_ref);
                failed = true;
                eprintln!("batch {job_ref} unavailable: {reason}");
            }
            Ok(MediaProcessingClaimOutcome::Idle) => {
                eprintln!("batch stopped: {} named jobs not claimable", jobs.len());
                return ExitCode::FAILURE;
            }
            Err(error) => {
                eprintln!("batch claim failed: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
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
    let ffmpeg = command_available(&ffmpeg_command());
    let whisper = command_available(&whisper_command());
    if ffmpeg {
        processors.push("thumbnail".to_owned());
        processors.push("audio_extract".to_owned());
    }
    if ffmpeg && whisper {
        processors.push("asr".to_owned());
    }
    processors
}

async fn execute_claim(
    database: &linggan_storage_postgres::Database,
    worker_instance_ref: Uuid,
    claim: &MediaProcessingClaim,
) -> Result<(), String> {
    match claim.processor_kind.as_str() {
        "thumbnail" => execute_thumbnail(database, worker_instance_ref, claim).await,
        "audio_extract" => execute_audio_extract(database, worker_instance_ref, claim).await,
        "asr" => execute_asr(database, worker_instance_ref, claim).await,
        other => Err(format!("processor_not_enabled:{other}")),
    }
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
