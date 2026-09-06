//! Human research assets and saved conditions. Source text and present eligibility belong to
//! Evidence; every asset read resolves that seam again rather than trusting a stored permission.

use linggan_evidence::comment_research_read::{
    CommentResearchReadError, read_comment_research_source,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchError {
    #[error("invalid comment research command")]
    InvalidCommand,
    #[error("idempotency conflict")]
    IdempotencyConflict,
    #[error("revision conflict")]
    RevisionConflict,
    #[error("source unavailable")]
    SourceUnavailable,
    #[error("comment research database unavailable")]
    Database(#[from] sqlx::Error),
}

impl From<CommentResearchReadError> for CommentResearchError {
    fn from(value: CommentResearchReadError) -> Self {
        match value {
            CommentResearchReadError::Database(e) => Self::Database(e),
            _ => Self::SourceUnavailable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchDimension {
    Scene,
    Problem,
    TriedMethod,
    StatedFailureReason,
    Emotion,
    Expectation,
    Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchBasis {
    Explicit,
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchFacet {
    pub dimension: ResearchDimension,
    pub label: String,
    pub basis: ResearchBasis,
}

pub(crate) fn validate_facets(facets: &[ResearchFacet]) -> Result<(), CommentResearchError> {
    if facets.len() > 14 || facets.iter().any(|f| !valid_text(&f.label, 100)) {
        return Err(CommentResearchError::InvalidCommand);
    }
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveCommentAsset {
    pub asset_ref: Uuid,
    pub source_ref: Uuid,
    pub start_char: i32,
    pub end_char: i32,
    pub source_sha256: String,
    pub reason: String,
    pub collection_ref: Option<Uuid>,
}

pub fn comment_source_hash(body: &str) -> String {
    Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn exact_comment_slice(
    body: &str,
    start: i32,
    end: i32,
) -> Result<String, CommentResearchError> {
    if start < 0 || end <= start || end - start > 4000 || end as usize > body.chars().count() {
        return Err(CommentResearchError::InvalidCommand);
    }
    Ok(body
        .chars()
        .skip(start as usize)
        .take((end - start) as usize)
        .collect())
}

pub async fn save_comment_asset(
    database: &Database,
    request: &SaveCommentAsset,
) -> Result<Value, CommentResearchError> {
    if !valid_text(&request.reason, 1000) {
        return Err(CommentResearchError::InvalidCommand);
    }
    let source = read_comment_research_source(database, request.source_ref).await?;
    let body = source.body.ok_or(CommentResearchError::SourceUnavailable)?;
    exact_comment_slice(&body, request.start_char, request.end_char)?;
    if comment_source_hash(&body) != request.source_sha256 {
        return Err(CommentResearchError::InvalidCommand);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("INSERT INTO linggan_comment_asset(asset_ref,source_ref,start_char,end_char,source_sha256,reason,collection_ref)
        VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(asset_ref) DO NOTHING")
        .bind(request.asset_ref).bind(request.source_ref).bind(request.start_char).bind(request.end_char)
        .bind(&request.source_sha256).bind(&request.reason).bind(request.collection_ref).execute(&mut *tx).await?;
    let row = sqlx::query("SELECT * FROM linggan_comment_asset WHERE asset_ref=$1")
        .bind(request.asset_ref)
        .fetch_one(&mut *tx)
        .await?;
    if row.get::<Uuid, _>("source_ref") != request.source_ref
        || row.get::<i32, _>("start_char") != request.start_char
        || row.get::<i32, _>("end_char") != request.end_char
        || row.get::<String, _>("source_sha256") != request.source_sha256
        || row.get::<String, _>("reason") != request.reason
        || row.get::<Option<Uuid>, _>("collection_ref") != request.collection_ref
    {
        return Err(CommentResearchError::IdempotencyConflict);
    }
    tx.commit().await?;
    let current = sqlx::query(
        "SELECT revision,withdrawn FROM linggan_comment_asset_current WHERE asset_ref=$1",
    )
    .bind(request.asset_ref)
    .fetch_one(database.pool())
    .await?;
    Ok(
        json!({"assetRef":request.asset_ref,"state":if current.get::<bool,_>("withdrawn"){"WITHDRAWN"}else{"SAVED"},"revision":current.get::<i32,_>("revision"),"purpose":"internal_research"}),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveResearchQuery {
    pub query_ref: Uuid,
    pub name: String,
    pub text: String,
    pub work_ref: Option<Uuid>,
}

pub async fn save_research_query(
    database: &Database,
    request: &SaveResearchQuery,
) -> Result<Value, CommentResearchError> {
    if !valid_text(&request.name, 100) || request.text.chars().count() > 200 {
        return Err(CommentResearchError::InvalidCommand);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("INSERT INTO linggan_comment_saved_query(query_ref,name,query_text,work_public_ref) VALUES($1,$2,$3,$4) ON CONFLICT(query_ref) DO NOTHING")
        .bind(request.query_ref).bind(&request.name).bind(&request.text).bind(request.work_ref).execute(&mut *tx).await?;
    let row = sqlx::query("SELECT * FROM linggan_comment_saved_query WHERE query_ref=$1")
        .bind(request.query_ref)
        .fetch_one(&mut *tx)
        .await?;
    if row.get::<String, _>("name") != request.name
        || row.get::<String, _>("query_text") != request.text
        || row.get::<Option<Uuid>, _>("work_public_ref") != request.work_ref
    {
        return Err(CommentResearchError::IdempotencyConflict);
    }
    tx.commit().await?;
    let current = sqlx::query(
        "SELECT revision,deleted FROM linggan_comment_query_current WHERE query_ref=$1",
    )
    .bind(request.query_ref)
    .fetch_one(database.pool())
    .await?;
    Ok(
        json!({"queryRef":request.query_ref,"state":if current.get::<bool,_>("deleted"){"DELETED"}else{"SAVED"},"revision":current.get::<i32,_>("revision"),"execution":"LATEST_ALLOWED_MATERIAL"}),
    )
}

pub async fn read_research_queries(database: &Database) -> Result<Value, CommentResearchError> {
    let rows=sqlx::query("SELECT *,created_at::text AS saved_at FROM linggan_comment_query_current WHERE NOT deleted ORDER BY created_at DESC,query_ref")
        .fetch_all(database.pool()).await?;
    Ok(
        json!({"items":rows.iter().map(|r|json!({"queryRef":r.get::<Uuid,_>("query_ref"),"name":r.get::<String,_>("name"),
        "text":r.get::<String,_>("query_text"),"workRef":r.get::<Option<Uuid>,_>("work_public_ref"),"revision":r.get::<i32,_>("revision"),"createdAt":r.get::<String,_>("saved_at")})).collect::<Vec<_>>() }),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveResearchCollection {
    pub collection_ref: Uuid,
    pub name: String,
}

pub async fn save_research_collection(
    database: &Database,
    request: &SaveResearchCollection,
) -> Result<Value, CommentResearchError> {
    if !valid_text(&request.name, 100) {
        return Err(CommentResearchError::InvalidCommand);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("INSERT INTO linggan_comment_collection(collection_ref,name) VALUES($1,$2) ON CONFLICT(collection_ref) DO NOTHING")
        .bind(request.collection_ref).bind(&request.name).execute(&mut *tx).await?;
    let name: String =
        sqlx::query_scalar("SELECT name FROM linggan_comment_collection WHERE collection_ref=$1")
            .bind(request.collection_ref)
            .fetch_one(&mut *tx)
            .await?;
    if name != request.name {
        return Err(CommentResearchError::IdempotencyConflict);
    }
    tx.commit().await?;
    Ok(json!({"collectionRef":request.collection_ref,"state":"SAVED"}))
}

pub async fn read_research_collections(database: &Database) -> Result<Value, CommentResearchError> {
    let rows=sqlx::query("SELECT collection.*,count(asset.asset_ref) AS asset_count FROM linggan_comment_collection collection
        LEFT JOIN linggan_comment_asset_current asset ON asset.collection_ref=collection.collection_ref AND NOT asset.withdrawn GROUP BY collection.collection_ref ORDER BY collection.created_at DESC,collection.collection_ref")
        .fetch_all(database.pool()).await?;
    Ok(
        json!({"items":rows.iter().map(|r|json!({"collectionRef":r.get::<Uuid,_>("collection_ref"),"name":r.get::<String,_>("name"),"savedCount":r.get::<i64,_>("asset_count")})).collect::<Vec<_>>() }),
    )
}

pub async fn read_comment_assets(
    database: &Database,
    collection_ref: Option<Uuid>,
    after: Option<Uuid>,
) -> Result<Value, CommentResearchError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_asset_current WHERE NOT withdrawn AND ($1::uuid IS NULL OR collection_ref=$1)",
    )
    .bind(collection_ref)
    .fetch_one(database.pool())
    .await?;
    let mut rows=sqlx::query("SELECT *,created_at::text AS saved_at FROM linggan_comment_asset_current WHERE NOT withdrawn AND ($1::uuid IS NULL OR collection_ref=$1)
        AND ($2::uuid IS NULL OR asset_ref>$2) ORDER BY asset_ref LIMIT 21")
        .bind(collection_ref).bind(after).fetch_all(database.pool()).await?;
    let more = rows.len() > 20;
    rows.truncate(20);
    let mut items = Vec::new();
    for row in &rows {
        let source_ref: Uuid = row.get("source_ref");
        let resolved = read_comment_research_source(database, source_ref).await;
        let source = match resolved {
            Ok(s) => Some(s),
            Err(CommentResearchReadError::SourceUnavailable) => None,
            Err(e) => return Err(e.into()),
        };
        let quote = source
            .as_ref()
            .and_then(|s| s.body.as_ref())
            .map(|b| exact_comment_slice(b, row.get("start_char"), row.get("end_char")))
            .transpose()?;
        items.push(json!({"assetRef":row.get::<Uuid,_>("asset_ref"),"sourceRef":source_ref,"quote":quote,
            "reason":if source.is_some(){Some(row.get::<String,_>("reason"))}else{None},
            "collectionRef":row.get::<Option<Uuid>,_>("collection_ref"),"revision":row.get::<i32,_>("revision"),"createdAt":row.get::<String,_>("saved_at"),
            "eligibility":if source.is_some(){"READABLE"}else{"SOURCE_UNAVAILABLE"},"isCurrentSource":source.as_ref().map(|s|s.is_current)}));
    }
    Ok(
        json!({"items":items,"total":total,"nextCursor":if more{rows.last().map(|r|r.get::<Uuid,_>("asset_ref"))}else{None}}),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrectResearchAnnotation {
    pub annotation_ref: Uuid,
    pub source_ref: Uuid,
    pub expected_revision: i32,
    pub facets: Vec<ResearchFacet>,
    pub reason: String,
}

pub async fn correct_research_annotation(
    database: &Database,
    request: &CorrectResearchAnnotation,
) -> Result<Value, CommentResearchError> {
    validate_facets(&request.facets)?;
    if !valid_text(&request.reason, 1000) || request.expected_revision < 0 {
        return Err(CommentResearchError::InvalidCommand);
    }
    read_comment_research_source(database, request.source_ref).await?;
    let mut tx = database.pool().begin().await?;
    sqlx::query(
        "SELECT material_ref FROM linggan_material_comment WHERE material_ref=$1 FOR UPDATE",
    )
    .bind(request.source_ref)
    .fetch_one(&mut *tx)
    .await?;
    let existing = sqlx::query("SELECT * FROM linggan_comment_annotation WHERE annotation_ref=$1")
        .bind(request.annotation_ref)
        .fetch_optional(&mut *tx)
        .await?;
    let facets =
        serde_json::to_value(&request.facets).map_err(|_| CommentResearchError::InvalidCommand)?;
    if let Some(row) = existing {
        if row.get::<Uuid, _>("source_ref") != request.source_ref
            || row.get::<Value, _>("facets") != facets
            || row.get::<String, _>("reason") != request.reason
            || row.get::<i32, _>("revision") != request.expected_revision + 1
        {
            return Err(CommentResearchError::IdempotencyConflict);
        }
    } else {
        let revision: i32 = sqlx::query_scalar(
            "SELECT COALESCE(max(revision),0) FROM linggan_comment_annotation WHERE source_ref=$1",
        )
        .bind(request.source_ref)
        .fetch_one(&mut *tx)
        .await?;
        if revision != request.expected_revision {
            return Err(CommentResearchError::RevisionConflict);
        }
        sqlx::query("INSERT INTO linggan_comment_annotation(annotation_ref,source_ref,revision,facets,reason) VALUES($1,$2,$3,$4,$5)")
            .bind(request.annotation_ref).bind(request.source_ref).bind(revision+1).bind(facets).bind(&request.reason).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"annotationRef":request.annotation_ref,"revision":request.expected_revision+1,"state":"SAVED"}),
    )
}

pub(crate) fn valid_text(text: &str, maximum: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_slices_use_unicode_scalars_not_utf16_or_bytes() {
        assert_eq!(exact_comment_slice("我👨‍👩‍👧好", 1, 6).unwrap(), "👨‍👩‍👧");
        assert!(exact_comment_slice("短", 0, 2).is_err());
        assert!(exact_comment_slice("短", -1, 1).is_err());
        assert_ne!(comment_source_hash("原文"), comment_source_hash("原文。"));
    }
}
