//! Durable local execution for already-admitted media processing jobs.
//!
//! Jobs, events and derivatives are append-only facts. `linggan_media_processing_work` is the
//! bounded mutable lease that prevents duplicate processing and infinite pending retries.

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

pub async fn claim_media_processing_work(
    database: &Database,
    worker_instance_ref: Uuid,
    enabled_processors: &[String],
) -> Result<Option<MediaProcessingClaim>, sqlx::Error> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_processing_work') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !ready || enabled_processors.is_empty() {
        return Ok(None);
    }
    let mut tx = database.pool().begin().await?;
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
        "SELECT work.work_ref,work.job_ref,job.processor_kind \
         FROM linggan_media_processing_work work \
         JOIN linggan_media_processing_job job USING(job_ref) \
         WHERE work.state IN ('pending','retry_wait') AND work.attempt_count < $1 \
           AND job.processor_kind = ANY($2) \
           AND work.next_attempt_at <= scope_001_now() \
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
