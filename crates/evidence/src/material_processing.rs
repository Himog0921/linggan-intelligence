//! Append-only completion facts for an already admitted media processing job.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

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
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) \
         VALUES($1,$2,'succeeded',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(job_ref)
    .execute(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let derivative_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_derivative \
         (derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) \
         VALUES($1,$2,$3,$4,$5,$6)",
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

fn derivative_matches_processor(processor: &str, derivative: &str) -> bool {
    matches!(
        (processor, derivative),
        ("thumbnail", "thumbnail")
            | ("image_ocr", "ocr_text")
            | ("audio_extract", "audio")
            | ("asr", "asr_text")
            | ("video_frame_ocr", "frame_ocr_text")
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
