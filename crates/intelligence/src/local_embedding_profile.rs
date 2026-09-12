//! The one immutable embedding profile for LOCAL-EMBEDDING-001.
//!
//! This is deliberately not a provider registry.  Comment Research V1 has one approved local
//! implementation; a profile exists solely to make vector identity queryable and non-mixable.

use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

pub const MODEL_ID: &str = "Tencent/WeMM-Embedding-2B";
pub const MODEL_REVISION: &str = "bbd6cd4bf52cfc6716f752a2df80b2706720bd95";
pub const ENCODING_MODE: &str = "document";
pub const DIMENSION: usize = 512;
pub const PREPROCESSING_VERSION: &str = "comment-research.atom-canonical-text.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEmbeddingProfile {
    pub profile_ref: Uuid,
    pub model_ref: Uuid,
    pub connection_version_ref: Uuid,
    pub model_id: String,
    pub model_revision: String,
    pub encoding_mode: String,
    pub dimension: usize,
    pub preprocessing_version: String,
}

pub async fn active(database: &Database) -> Result<Option<LocalEmbeddingProfile>, ModelError> {
    let row = sqlx::query(
        "SELECT profile.profile_ref,profile.model_ref,model.connection_version_ref,\
                profile.model_id,profile.model_revision,profile.encoding_mode,profile.dimension,\
                profile.preprocessing_version \
         FROM linggan_comment_research_embedding_profile profile \
         JOIN linggan_model_entry model USING(model_ref) \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE profile.singleton AND profile.enabled AND connection.enabled",
    )
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| LocalEmbeddingProfile {
        profile_ref: row.get("profile_ref"),
        model_ref: row.get("model_ref"),
        connection_version_ref: row.get("connection_version_ref"),
        model_id: row.get("model_id"),
        model_revision: row.get("model_revision"),
        encoding_mode: row.get("encoding_mode"),
        dimension: usize::try_from(row.get::<i32, _>("dimension")).unwrap_or_default(),
        preprocessing_version: row.get("preprocessing_version"),
    }))
}

pub async fn ready_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS( \
            SELECT 1 FROM linggan_comment_research_embedding_profile profile \
            JOIN linggan_model_entry model USING(model_ref) \
            JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
            JOIN linggan_model_connection connection USING(connection_ref) \
            WHERE profile.singleton AND profile.enabled AND connection.enabled \
              AND profile.model_id='Tencent/WeMM-Embedding-2B' \
              AND profile.model_revision='bbd6cd4bf52cfc6716f752a2df80b2706720bd95' \
              AND profile.encoding_mode='document' AND profile.dimension=512 \
              AND profile.preprocessing_version='comment-research.atom-canonical-text.v1' \
        )",
    )
    .fetch_one(&mut **transaction)
    .await?)
}
