//! Durable local execution for already-admitted media processing jobs.
//!
//! Jobs, events and derivatives are append-only facts. `linggan_media_processing_work` is the
//! bounded mutable lease that prevents duplicate processing and infinite pending retries.
//!
//! 本模块另外管一件与「已准入的作业」直接相关的事：**处理器的版本号**。作业的唯一键含
//! `processor_version`，所以改了一个处理器的行为却不抬版本号，改动就只对今后新采的字节生效；
//! 抬了版本号，存量才有一条可被重新排队的路（`requeue_outdated_processor_jobs`）。

use crate::material_processing_validation::{derivative_matches_processor, is_sha256};
use crate::producer_runtime::ProducerRuntimeError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

const MAX_PROCESSING_ATTEMPTS: i32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProcessingClaim {
    pub work_ref: Uuid,
    pub job_ref: Uuid,
    pub claim_generation: i32,
    pub processor_kind: String,
    pub processor_version: String,
    pub blob_sha256: String,
    pub mime_type: String,
    pub storage_key: String,
    pub content_public_ref: Uuid,
    pub lease_expires_at: String,
}

pub async fn ensure_media_processing_work(database: &Database) -> Result<u64, sqlx::Error> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_processing_work') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !ready {
        return Ok(0);
    }
    Ok(sqlx::query(
        "INSERT INTO linggan_media_processing_work(work_ref,job_ref) \
         SELECT gen_random_uuid(),job.job_ref FROM linggan_media_processing_job job \
         LEFT JOIN linggan_media_processing_work work USING(job_ref) \
         WHERE work.job_ref IS NULL ON CONFLICT(job_ref) DO NOTHING",
    )
    .execute(database.pool())
    .await?
    .rows_affected())
}

/// 认领闸的就绪状态：这个 worker 启用的处理器，到底能不能领到活。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimGateReadiness {
    /// 闸门表不存在——`0088` 还没应用。此时**任何**处理器都认领不到。
    NotMigrated,
    /// 闸门表在，但这些已启用的处理器没有登记上限，因此永远认领不到。
    Unregistered(Vec<String>),
    /// 启用的处理器全部登记在册。
    Ready,
}

/// 读一次认领闸的就绪状态。
///
/// **存在的理由是：没登记的表现是「什么都不发生」。** 未迁移或未登记时 `claim` 一律返回
/// `Ok(None)`，worker 把它读成「这一轮没有活」，循环继续，**一行日志都不打**——一个看起来
/// 完全健康的进程永远空转。本包要消灭的正是这种失败形态（196 条 asr 里 195 条静默失败，
/// 就是零日志藏出来的），所以闸门自己不能以同一种方式藏起来。
///
/// 判据与 `claim_media_processing_work` 里那道闸同源：同一张表，同一个「登记了才有资格」的
/// 语义。这里只是把它变成一句可以读给人听的话。
pub async fn read_claim_gate_readiness(
    database: &Database,
    enabled_processors: &[String],
) -> Result<ClaimGateReadiness, sqlx::Error> {
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_media_processing_concurrency') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !ready {
        return Ok(ClaimGateReadiness::NotMigrated);
    }
    let registered: Vec<String> =
        sqlx::query_scalar("SELECT processor_kind FROM linggan_media_processing_concurrency")
            .fetch_all(database.pool())
            .await?;
    let unregistered = enabled_processors
        .iter()
        .filter(|kind| !registered.iter().any(|row| row == *kind))
        .cloned()
        .collect::<Vec<_>>();
    if unregistered.is_empty() {
        Ok(ClaimGateReadiness::Ready)
    } else {
        Ok(ClaimGateReadiness::Unregistered(unregistered))
    }
}

pub async fn claim_media_processing_work(
    database: &Database,
    worker_instance_ref: Uuid,
    enabled_processors: &[String],
) -> Result<Option<MediaProcessingClaim>, sqlx::Error> {
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_media_processing_work') IS NOT NULL \
            AND to_regclass('linggan_media_processing_concurrency') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !ready || enabled_processors.is_empty() {
        return Ok(None);
    }
    let mut tx = database.pool().begin().await?;
    // **认领闸。** 先取锁，再数在途，最后才认领——三步必须在同一把锁下，否则这道闸拦不住任何东西。
    //
    // 光有下面的 `FOR UPDATE ... SKIP LOCKED` 是不够的：它保证两个进程**不会抢到同一条**，
    // 恰恰因此它们会**各拿到一条不同的**，各自数到「在途 0」，然后双双越过上限。这正是
    // 2026-09-16 线上 6 个进程同时在跑的原因——上限是 1，实际是 6，而代码里每一处看都对。
    //
    // 锁按「整张认领动作」取一把，不按 processor_kind 分开：认领事务只做几条 UPDATE 和一次
    // 计数，是毫秒级的，而真正的处理（OCR、转录）发生在事务提交**之后**，不在锁里。
    // 所以这把锁不降低任何实际吞吐，只是让「数一数」和「领一条」之间没有缝。
    //
    // 上限本身住在 `linggan_media_processing_concurrency` 表里，不在各进程的环境变量里：
    // 两个进程各设各的加起来必然超，而且读取点替执行做的决定会随进程重启漂移。
    //
    // **一个必须说清的代价：上限收到 1 之后，一条没到期的租约就占住整条车道。** worker 正常失败
    // 会走 `fail_media_processing_work` 当场释放；但进程被 `kill -9` 时那条租约要等到期才回收，
    // 而 asr 的租约是 **6 小时**（基线取值，本包没改）。此前这不成问题——另外 5 个进程会接着认领，
    // 也就是说「多进程」一直在无意中掩盖租约回收的迟钝。现在要连跑 196 条 asr（约 11 小时），
    // 中途被杀一次就可能白等几小时。这是收紧并发换来的真实代价，不是缺陷，但得写在明处。
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("linggan_media_processing_claim_gate")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE linggan_media_processing_work SET \
           state=CASE WHEN attempt_count >= $1 THEN 'terminal' ELSE 'retry_wait' END, \
           worker_instance_ref=NULL,lease_expires_at=NULL,next_attempt_at=scope_001_now(), \
           last_error='lease_expired',updated_at=scope_001_now() \
         WHERE state='leased' AND lease_expires_at <= scope_001_now()",
    )
    .bind(MAX_PROCESSING_ATTEMPTS)
    .execute(&mut *tx)
    .await?;
    let candidate = sqlx::query(
        // `JOIN ... concurrency` 是 INNER JOIN，且没有兜底行：**表里没登记的 processor_kind
        // 一条都认领不到**（Closed World，与 SCOPE-001 一致）。这是有意的——新增一种处理器
        // 却忘了登记上限时，它应该停下来被人看见，而不是没有上限地跑起来。
        //
        // 「被人看见」这半句此前是假的：停下来是真的，被看见没有实现——`Ok(None)` 在 worker
        // 那里读作「这一轮没活」，一行日志都不打。现在由 `read_claim_gate_readiness` 在进程
        // 启动时把这半句补上；改这里的语义时记得两处一起看。
        "SELECT work.work_ref,work.job_ref,job.processor_kind \
         FROM linggan_media_processing_work work \
         JOIN linggan_media_processing_job job USING(job_ref) \
         JOIN linggan_media_processing_concurrency concurrency \
           ON concurrency.processor_kind = job.processor_kind \
         WHERE work.state IN ('pending','retry_wait') AND work.attempt_count < $1 \
           AND job.processor_kind = ANY($2) \
           AND work.next_attempt_at <= scope_001_now() \
           AND (SELECT count(*) \
                FROM linggan_media_processing_work in_flight \
                JOIN linggan_media_processing_job in_flight_job \
                  ON in_flight_job.job_ref = in_flight.job_ref \
                WHERE in_flight.state = 'leased' \
                  AND in_flight_job.processor_kind = job.processor_kind) \
               < concurrency.max_in_flight \
         ORDER BY CASE job.processor_kind \
                    WHEN 'image_ocr' THEN 1 WHEN 'thumbnail' THEN 2 \
                    WHEN 'audio_extract' THEN 3 WHEN 'asr' THEN 4 ELSE 5 END, \
                  work.next_attempt_at,work.created_at \
         LIMIT 1 FOR UPDATE OF work SKIP LOCKED",
    )
    .bind(MAX_PROCESSING_ATTEMPTS)
    .bind(enabled_processors)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(candidate) = candidate else {
        tx.commit().await?;
        return Ok(None);
    };
    let work_ref: Uuid = candidate.get("work_ref");
    let job_ref: Uuid = candidate.get("job_ref");
    let processor_kind: String = candidate.get("processor_kind");
    let lease_interval = if processor_kind == "asr" {
        "6 hours"
    } else {
        "30 minutes"
    };
    let row = sqlx::query(
        "UPDATE linggan_media_processing_work SET state='leased', \
           attempt_count=attempt_count+1,claim_generation=claim_generation+1, \
           worker_instance_ref=$2,lease_expires_at=scope_001_now()+$3::interval, \
           updated_at=scope_001_now() WHERE work_ref=$1 \
         RETURNING claim_generation,lease_expires_at::text AS lease_expires_at",
    )
    .bind(work_ref)
    .bind(worker_instance_ref)
    .bind(lease_interval)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'running',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(job_ref)
    .execute(&mut *tx)
    .await?;
    let input = sqlx::query(
        "SELECT job.processor_version,job.blob_sha256,blob.mime_type,blob.storage_key, \
                content.public_ref AS content_public_ref \
         FROM linggan_media_processing_job job \
         JOIN linggan_media_blob blob ON blob.sha256=job.blob_sha256 \
         JOIN linggan_media_slot slot ON slot.slot_key=job.slot_key \
         JOIN linggan_material_content content \
           ON content.platform=slot.platform AND content.content_external_id=slot.content_external_id \
         WHERE job.job_ref=$1",
    )
    .bind(job_ref)
    .fetch_one(&mut *tx)
    .await?;
    let claim = MediaProcessingClaim {
        work_ref,
        job_ref,
        claim_generation: row.get("claim_generation"),
        processor_kind,
        processor_version: input.get("processor_version"),
        blob_sha256: input.get("blob_sha256"),
        mime_type: input.get("mime_type"),
        storage_key: input.get("storage_key"),
        content_public_ref: input.get("content_public_ref"),
        lease_expires_at: row.get("lease_expires_at"),
    };
    tx.commit().await?;
    Ok(Some(claim))
}

#[allow(clippy::too_many_arguments)]
pub async fn complete_media_processing_text(
    database: &Database,
    claim: &MediaProcessingClaim,
    worker_instance_ref: Uuid,
    derivative_kind: &str,
    content_hash: &str,
    byte_size: i64,
    storage_key: &str,
    text_content: &str,
    display_text: &str,
    language_tag: Option<&str>,
) -> Result<Uuid, ProducerRuntimeError> {
    if !derivative_matches_processor(&claim.processor_kind, derivative_kind)
        || !is_sha256(content_hash)
        || text_content.trim().is_empty()
        || display_text.trim().is_empty()
        || !crate::material_storage_key::is_safe_storage_key(storage_key)
        || !(1..=crate::material_storage_key::maximum_local_asset_bytes()).contains(&byte_size)
    {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let live: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_media_processing_work \
         WHERE work_ref=$1 AND job_ref=$2 AND state='leased' \
           AND worker_instance_ref=$3 AND claim_generation=$4 \
           AND lease_expires_at>scope_001_now())",
    )
    .bind(claim.work_ref)
    .bind(claim.job_ref)
    .bind(worker_instance_ref)
    .bind(claim.claim_generation)
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !live {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT derivative_ref FROM linggan_media_derivative WHERE job_ref=$1 LIMIT 1",
    )
    .bind(claim.job_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if let Some(existing) = existing {
        return Ok(existing);
    }
    let derivative_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_derivative \
         (derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) \
         VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(derivative_ref)
    .bind(claim.job_ref)
    .bind(derivative_kind)
    .bind(content_hash)
    .bind(byte_size)
    .bind(storage_key)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_material_derived_text \
         (derivative_ref,content_public_ref,kind,text_content,display_text,language_state,language_tag,source_location) \
         VALUES($1,$2,$3,$4,$5,$6,$7,jsonb_build_object('blobSha256',$8,'processorVersion',$9))",
    )
    .bind(derivative_ref)
    .bind(claim.content_public_ref)
    .bind(derivative_kind)
    .bind(text_content)
    .bind(display_text)
    .bind(if language_tag.is_some() { "KNOWN" } else { "UNKNOWN" })
    .bind(language_tag)
    .bind(&claim.blob_sha256)
    .bind(&claim.processor_version)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'succeeded',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(claim.job_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "UPDATE linggan_media_processing_work SET state='completed',worker_instance_ref=NULL, \
         lease_expires_at=NULL,last_error=NULL,completed_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE work_ref=$1",
    )
    .bind(claim.work_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(derivative_ref)
}

pub async fn fail_media_processing_work(
    database: &Database,
    claim: &MediaProcessingClaim,
    worker_instance_ref: Uuid,
    reason: &str,
) -> Result<String, sqlx::Error> {
    let mut tx = database.pool().begin().await?;
    let row = sqlx::query(
        "SELECT attempt_count FROM linggan_media_processing_work \
         WHERE work_ref=$1 AND job_ref=$2 AND state='leased' \
           AND worker_instance_ref=$3 AND claim_generation=$4 FOR UPDATE",
    )
    .bind(claim.work_ref)
    .bind(claim.job_ref)
    .bind(worker_instance_ref)
    .bind(claim.claim_generation)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok("lost_authority".to_owned());
    };
    let attempt_count: i32 = row.get("attempt_count");
    let next_state = if attempt_count >= MAX_PROCESSING_ATTEMPTS {
        "terminal"
    } else {
        "retry_wait"
    };
    let safe_reason: String = reason.chars().take(500).collect();
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'failed',$3)",
    )
    .bind(Uuid::new_v4())
    .bind(claim.job_ref)
    .bind(&safe_reason)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE linggan_media_processing_work SET state=$2,worker_instance_ref=NULL, \
         lease_expires_at=NULL,last_error=$3, \
         next_attempt_at=scope_001_now()+make_interval(secs => 30*attempt_count), \
         updated_at=scope_001_now() WHERE work_ref=$1",
    )
    .bind(claim.work_ref)
    .bind(next_state)
    .bind(safe_reason)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(next_state.to_owned())
}

/// Finish a valid local processor run that observed no textual output.
///
/// Empty OCR is a qualified result, not a transient failure. Retrying the same image three times
/// cannot manufacture text and would turn a truthful empty observation into an artificial error.
pub async fn complete_media_processing_without_output(
    database: &Database,
    claim: &MediaProcessingClaim,
    worker_instance_ref: Uuid,
    reason: &str,
) -> Result<(), ProducerRuntimeError> {
    let safe_reason: String = reason.chars().take(500).collect();
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let completed = sqlx::query(
        "UPDATE linggan_media_processing_work SET state='completed',worker_instance_ref=NULL, \
         lease_expires_at=NULL,last_error=NULL,completed_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE work_ref=$1 AND job_ref=$2 AND state='leased' AND worker_instance_ref=$3 \
           AND claim_generation=$4 AND lease_expires_at>scope_001_now()",
    )
    .bind(claim.work_ref)
    .bind(claim.job_ref)
    .bind(worker_instance_ref)
    .bind(claim.claim_generation)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if completed.rows_affected() != 1 {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'succeeded',$3)",
    )
    .bind(Uuid::new_v4())
    .bind(claim.job_ref)
    .bind(safe_reason)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn complete_media_processing_derivative(
    database: &Database,
    claim: &MediaProcessingClaim,
    worker_instance_ref: Uuid,
    derivative_kind: &str,
    content_hash: &str,
    byte_size: i64,
    storage_key: &str,
) -> Result<Uuid, ProducerRuntimeError> {
    if !derivative_matches_processor(&claim.processor_kind, derivative_kind)
        || !is_sha256(content_hash)
        || !crate::material_storage_key::is_safe_storage_key(storage_key)
        || !(1..=crate::material_storage_key::maximum_local_asset_bytes()).contains(&byte_size)
    {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let live: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_media_processing_work \
         WHERE work_ref=$1 AND job_ref=$2 AND state='leased' AND worker_instance_ref=$3 \
           AND claim_generation=$4 AND lease_expires_at>scope_001_now())",
    )
    .bind(claim.work_ref)
    .bind(claim.job_ref)
    .bind(worker_instance_ref)
    .bind(claim.claim_generation)
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !live {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let derivative_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_derivative \
         (derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) \
         VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(derivative_ref)
    .bind(claim.job_ref)
    .bind(derivative_kind)
    .bind(content_hash)
    .bind(byte_size)
    .bind(storage_key)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'succeeded',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(claim.job_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "UPDATE linggan_media_processing_work SET state='completed',worker_instance_ref=NULL, \
         lease_expires_at=NULL,last_error=NULL,completed_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE work_ref=$1",
    )
    .bind(claim.work_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(derivative_ref)
}

/// Legacy narrow completion used by existing API fixtures for non-text derivatives.
pub async fn record_media_derivative_completion(
    database: &Database,
    job_ref: Uuid,
    derivative_kind: &str,
    content_hash: &str,
    byte_size: i64,
    storage_key: Option<&str>,
) -> Result<Uuid, ProducerRuntimeError> {
    if storage_key.is_some_and(|value| !crate::material_storage_key::is_safe_storage_key(value))
        || !(1..=crate::material_storage_key::maximum_local_asset_bytes()).contains(&byte_size)
        || crate::material_storage_key::derivative_mime(derivative_kind).is_none()
    {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let job = sqlx::query(
        "SELECT processor_kind FROM linggan_media_processing_job WHERE job_ref=$1 FOR UPDATE",
    )
    .bind(job_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?
    .ok_or(ProducerRuntimeError::MaterialIdentityConflict)?;
    let processor_kind: String = job.get("processor_kind");
    if !derivative_matches_processor(&processor_kind, derivative_kind) || !is_sha256(content_hash) {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT derivative_ref FROM linggan_media_derivative WHERE job_ref=$1 LIMIT 1",
    )
    .bind(job_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if existing.is_some() {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'succeeded',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(job_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let derivative_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_derivative (derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(derivative_ref)
    .bind(job_ref)
    .bind(derivative_kind)
    .bind(content_hash)
    .bind(byte_size)
    .bind(storage_key)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(derivative_ref)
}

/// 登记的处理器名。与 `0004` 里 `processor_kind` 的 `CHECK` 同一份名单。
///
/// 单独列出来是给两个调用方校验名字用的：摄取路径不该生出一个没有版本的处理器，
/// 重排工具的命令行也不该把名字打错读成「这一类没有旧作业」。
pub const REGISTERED_PROCESSOR_KINDS: [&str; 5] = [
    "thumbnail",
    "image_ocr",
    "audio_extract",
    "asr",
    "video_frame_ocr",
];

/// 这个处理器当前的版本号。**改了处理器的行为，就必须同时抬它的版本号。**
///
/// 作业的唯一键是 `(blob_sha256,slot_key,processor_kind,processor_version,input_scope)`，摄取路径
/// 用 `ON CONFLICT DO NOTHING` 去重。不抬版本号时，同一张图/同一段视频**连唯一键都对得上**，
/// 新作业一条也插不进去——改成什么样，都只对今后新采的字节生效，库里已有的结果一条都不会重做。
/// 2026-09-16 那批 OCR 繁体串扰（2243 条）与 195 条转录失败留在库里，根因就是这个。
///
/// 抬版本号**不替换**任何东西：`linggan_media_processing_job` 与 `linggan_media_derivative` 都是
/// 只追加（触发器禁 UPDATE/DELETE）。旧结果是留在库里更早的一个版本，新结果追加在其后，读路径
/// 按作业逐条列出、各自带着 `processorVersion`（`material_media_read.rs`）。
///
/// 返回 `None` 表示「这个处理器没有登记版本」，调用方必须停下来，不给默认值——与 `0088` 的并发闸
/// 同一条 Closed World 规矩：没登记的处理器不该悄悄按旧版本跑起来，也不该悄悄什么都不做。
pub fn processor_version_for_kind(processor_kind: &str) -> Option<&'static str> {
    match processor_kind {
        // 2026-09-17 抬版：Tesseract 语言表去掉 `chi_tra`（简繁串扰，同一张图上「家长」「家長」
        // 并存），whisper 换 medium 并加简体提示、修掉交给 whisper 子进程的 PATH（缺 ffmpeg）。
        "image_ocr" | "video_frame_ocr" | "asr" => Some("local-v2"),
        // 行为没变的两类留在 `local-v1`：它们没有需要重做的存量，存量重排也不该碰它们。
        "thumbnail" | "audio_extract" => Some("local-v1"),
        _ => None,
    }
}

/// 重排出来的作业在事件流里留下的理由，与 `queued_for_local_processor` 分开记：
/// 「当初采回来的」和「按新版本补排的」在证据上必须分得开。
const REQUEUE_REASON: &str = "requeued_after_processor_version_upgrade";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorRequeueSummary {
    pub processor_kind: String,
    pub target_version: String,
    /// 本次排出来的作业条数。`dry_run` 时报的是「真的做一遍会排多少条」——见下。
    pub enqueued_jobs: i64,
}

/// 把旧版本处理器做过的作业，按当前版本重新排一遍（存量重跑）。
///
/// 每条旧作业补出一条同 `(blob_sha256,slot_key,input_scope)` 的新作业，只把 `processor_version`
/// 换成当前版本；同时补一条 `pending` 事件，与摄取路径成对写入的形状一致（读路径取作业的最后一条
/// 事件当状态，缺了它界面会显示成没有状态）。**可认领的 work 行不在这里建**：那是
/// `ensure_media_processing_work` 每 tick 的投影，它有唯一的主人，不另开一条路。
///
/// **幂等**：同一组键已经有当前版本的那些不再补，所以重复运行是 0 条，不是报错。
/// **有序**由调用方决定：每次只排它点名的那几类；认领顺序仍由 `claim_media_processing_work`
/// 的 `ORDER BY` 决定，这个函数不替它排。
/// **`dry_run` 是真的做一遍再撤回**，不是另写一套「预估」SQL：同一句语句、同一个计数，跑在事务里，
/// 结束前 `rollback`。两份谓词各写一遍必然有一天会漂，那时预演报的数就与真做出来的不一样了。
pub async fn requeue_outdated_processor_jobs(
    database: &Database,
    processor_kinds: &[String],
    dry_run: bool,
) -> Result<Vec<ProcessorRequeueSummary>, ProducerRuntimeError> {
    let mut summaries = Vec::new();
    for processor_kind in processor_kinds {
        let target_version = processor_version_for_kind(processor_kind).ok_or_else(|| {
            ProducerRuntimeError::ProcessorKindNotRegistered(processor_kind.clone())
        })?;
        let mut tx = database
            .pool()
            .begin()
            .await
            .map_err(ProducerRuntimeError::Internal)?;
        // `DISTINCT ON` 是给「同一个键上躺着两个以上旧版本」准备的：那种情况下两条源行会长出
        // **同一个唯一键**，一条语句里插两遍同键的行会当场违反唯一约束、整句失败。取哪一条都行
        // （它们的 blob、槽位、作用域本来就相同，只差版本号），所以按 `created_at` 定一条。
        let enqueued_jobs: Vec<Uuid> = sqlx::query_scalar(
            "WITH outdated AS ( \
               SELECT DISTINCT ON (job.blob_sha256,job.slot_key,job.processor_kind,job.input_scope) \
                      job.blob_sha256,job.slot_key,job.processor_kind,job.input_scope \
               FROM linggan_media_processing_job job \
               WHERE job.processor_kind=$1 \
                 AND NOT EXISTS (SELECT 1 FROM linggan_media_processing_job current \
                                 WHERE current.blob_sha256=job.blob_sha256 \
                                   AND current.slot_key IS NOT DISTINCT FROM job.slot_key \
                                   AND current.processor_kind=job.processor_kind \
                                   AND current.processor_version=$2 \
                                   AND current.input_scope=job.input_scope) \
               ORDER BY job.blob_sha256,job.slot_key,job.processor_kind,job.input_scope, \
                        job.created_at DESC), \
             enqueued AS ( \
               INSERT INTO linggan_media_processing_job \
                 (job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) \
               SELECT gen_random_uuid(),outdated.blob_sha256,outdated.slot_key, \
                      outdated.processor_kind,$2,outdated.input_scope \
               FROM outdated RETURNING job_ref) \
             INSERT INTO linggan_media_processing_job_event (event_ref,job_ref,state,reason) \
             SELECT gen_random_uuid(),enqueued.job_ref,'pending',$3 FROM enqueued \
             RETURNING job_ref",
        )
        .bind(processor_kind)
        .bind(target_version)
        .bind(REQUEUE_REASON)
        .fetch_all(&mut *tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
        if dry_run {
            tx.rollback().await.map_err(ProducerRuntimeError::Internal)?;
        } else {
            tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
        }
        summaries.push(ProcessorRequeueSummary {
            processor_kind: processor_kind.clone(),
            target_version: target_version.to_owned(),
            enqueued_jobs: enqueued_jobs.len() as i64,
        });
    }
    Ok(summaries)
}
