//! Read eligibility for revocable local media and derivative assets.

use linggan_storage_postgres::Database;
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

#[derive(Debug)]
pub struct LocalMaterialAsset {
    pub mime_type: String,
    pub storage_key: String,
    pub expected_sha256: String,
    pub expected_byte_size: Option<i64>,
    pub maximum_byte_size: i64,
    pub delivery_mime_type: String,
    pub inline_safe: bool,
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
           AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition event \
             WHERE (event.blob_sha256=$2 OR event.materialization_ref=$1 OR event.slot_key=observation.slot_key) \
            )",
    )
    .bind(materialization_ref)
    .bind(sha256)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| {
        let mime_type: String = row.get("mime_type");
        let inline_safe = crate::material_storage_key::safe_inline_mime(&mime_type).is_some();
        let delivery_mime_type = if inline_safe {
            mime_type.clone()
        } else {
            "application/octet-stream".to_owned()
        };
        LocalMaterialAsset {
            mime_type,
            storage_key: row.get("storage_key"),
            expected_sha256: row.get("sha256"),
            expected_byte_size: Some(row.get("byte_size")),
            maximum_byte_size: crate::material_storage_key::maximum_local_asset_bytes(),
            delivery_mime_type,
            inline_safe,
        }
    }))
}

pub async fn read_local_derivative(
    database: &Database,
    derivative_ref: Uuid,
) -> Result<Option<LocalMaterialAsset>, sqlx::Error> {
    let ocr_retirement_schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_ocr_retirement') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    let retirement_filter = if ocr_retirement_schema_ready {
        " AND NOT EXISTS (SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref=job.job_ref)"
    } else {
        ""
    };
    // The only interpolation is a private literal selected by schema readiness above.
    let row = sqlx::query(AssertSqlSafe(format!(
        "SELECT derivative.derivative_kind,derivative.storage_key,derivative.content_hash,derivative.byte_size FROM linggan_media_derivative derivative \
         JOIN linggan_media_processing_job job USING(job_ref) \
         WHERE derivative.derivative_ref=$1 AND derivative.storage_key IS NOT NULL \
           {retirement_filter} \
           AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition event \
             WHERE (event.derivative_ref=$1 OR event.blob_sha256=job.blob_sha256 OR event.slot_key=job.slot_key) \
            )"
    )))
    .bind(derivative_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| {
        let mime_type = crate::material_storage_key::derivative_mime(
            row.get::<String, _>("derivative_kind").as_str(),
        )
        .unwrap_or("application/octet-stream")
        .to_owned();
        let inline_safe = crate::material_storage_key::safe_inline_mime(&mime_type).is_some();
        let delivery_mime_type = if inline_safe {
            mime_type.clone()
        } else {
            "application/octet-stream".to_owned()
        };
        LocalMaterialAsset {
            mime_type,
            storage_key: row.get("storage_key"),
            expected_sha256: row.get("content_hash"),
            expected_byte_size: row.get("byte_size"),
            maximum_byte_size: crate::material_storage_key::maximum_local_asset_bytes(),
            delivery_mime_type,
            inline_safe,
        }
    }))
}
