//! Read eligibility for revocable local media and derivative assets.

use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug)]
pub struct LocalMaterialAsset {
    pub mime_type: String,
    pub storage_key: String,
    pub expected_sha256: String,
    pub expected_byte_size: Option<i64>,
}

pub async fn read_local_materialization(
    database: &Database,
    materialization_ref: Uuid,
    sha256: &str,
) -> Result<Option<LocalMaterialAsset>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT blob.mime_type,blob.storage_key,blob.sha256,blob.byte_size FROM linggan_media_materialization materialization \
         JOIN linggan_media_blob blob ON blob.sha256=materialization.blob_sha256 \
         JOIN linggan_media_download_attempt attempt USING(download_attempt_ref) \
         JOIN linggan_media_observation observation ON observation.observation_ref=attempt.media_observation_ref \
         WHERE materialization.materialization_ref=$1 AND materialization.blob_sha256=$2 \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_media_disposition_event event \
             WHERE event.blob_sha256=$2 OR event.materialization_ref=$1 OR event.slot_key=observation.slot_key)",
    )
    .bind(materialization_ref)
    .bind(sha256)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| LocalMaterialAsset {
        mime_type: row.get("mime_type"),
        storage_key: row.get("storage_key"),
        expected_sha256: row.get("sha256"),
        expected_byte_size: Some(row.get("byte_size")),
    }))
}

pub async fn read_local_derivative(
    database: &Database,
    derivative_ref: Uuid,
) -> Result<Option<LocalMaterialAsset>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT derivative.derivative_kind,derivative.storage_key,derivative.content_hash FROM linggan_media_derivative derivative \
         JOIN linggan_media_processing_job job USING(job_ref) \
         WHERE derivative.derivative_ref=$1 AND derivative.storage_key IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_media_disposition_event event \
             WHERE event.derivative_ref=$1 OR event.blob_sha256=job.blob_sha256 OR event.slot_key=job.slot_key)",
    )
    .bind(derivative_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| LocalMaterialAsset {
        mime_type: derivative_mime(row.get::<String, _>("derivative_kind").as_str()).to_owned(),
        storage_key: row.get("storage_key"),
        expected_sha256: row.get("content_hash"),
        expected_byte_size: None,
    }))
}

fn derivative_mime(kind: &str) -> &'static str {
    match kind {
        "thumbnail" => "image/jpeg",
        "audio" => "audio/mpeg",
        "ocr_text" | "asr_text" | "frame_ocr_text" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
