//! Append-only management revisions. Creation replay never restores a deleted or withdrawn item.
use crate::comment_research::{CommentResearchError, valid_text};
use linggan_evidence::comment_research_read::{
    CommentResearchReadError, read_comment_research_source,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviseCommentAsset {
    pub revision_ref: Uuid,
    pub asset_ref: Uuid,
    pub expected_revision: i32,
    pub reason: Option<String>,
    pub collection_ref: Option<Uuid>,
    pub withdrawn: bool,
    pub change_reason: String,
}

pub async fn revise_comment_asset(
    database: &Database,
    request: &ReviseCommentAsset,
) -> Result<Value, CommentResearchError> {
    if !(0..i32::MAX).contains(&request.expected_revision)
        || !valid_text(&request.change_reason, 1000)
        || request
            .reason
            .as_ref()
            .is_some_and(|s| !valid_text(s, 1000))
        || (!request.withdrawn && request.reason.is_none())
    {
        return Err(CommentResearchError::InvalidCommand);
    }
    let mut tx = database.pool().begin().await?;
    let source_ref: Uuid = sqlx::query_scalar(
        "SELECT source_ref FROM linggan_comment_asset WHERE asset_ref=$1 FOR UPDATE",
    )
    .bind(request.asset_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(CommentResearchError::SourceUnavailable)?;
    let current = sqlx::query(
        "SELECT revision,withdrawn FROM linggan_comment_asset_current WHERE asset_ref=$1",
    )
    .bind(request.asset_ref)
    .fetch_one(&mut *tx)
    .await?;
    let existing =
        sqlx::query("SELECT * FROM linggan_comment_asset_revision WHERE revision_ref=$1")
            .bind(request.revision_ref)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(row) = existing {
        if row.get::<Uuid, _>("asset_ref") != request.asset_ref
            || row.get::<i32, _>("revision") != request.expected_revision + 1
            || row.get::<Option<String>, _>("reason") != request.reason
            || row.get::<Option<Uuid>, _>("collection_ref") != request.collection_ref
            || row.get::<bool, _>("withdrawn") != request.withdrawn
            || row.get::<String, _>("change_reason") != request.change_reason
        {
            return Err(CommentResearchError::IdempotencyConflict);
        }
        return Ok(
            json!({"assetRef":request.asset_ref,"revision":current.get::<i32,_>("revision"),
            "state":if current.get::<bool,_>("withdrawn"){"WITHDRAWN"}else{"SAVED"},"replayed":true}),
        );
    }
    if current.get::<i32, _>("revision") != request.expected_revision
        || current.get::<bool, _>("withdrawn")
    {
        return Err(CommentResearchError::RevisionConflict);
    }
    if !request.withdrawn {
        read_comment_research_source(database, source_ref).await?;
    }
    if let Some(collection_ref) = request.collection_ref {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_collection WHERE collection_ref=$1)",
        )
        .bind(collection_ref)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(CommentResearchError::InvalidCommand);
        }
    }
    sqlx::query("INSERT INTO linggan_comment_asset_revision(revision_ref,asset_ref,revision,reason,collection_ref,withdrawn,change_reason) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(request.revision_ref).bind(request.asset_ref).bind(request.expected_revision+1)
        .bind(&request.reason).bind(request.collection_ref).bind(request.withdrawn).bind(&request.change_reason)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"assetRef":request.asset_ref,"revision":request.expected_revision+1,"state":if request.withdrawn{"WITHDRAWN"}else{"SAVED"}}),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviseResearchQuery {
    pub revision_ref: Uuid,
    pub query_ref: Uuid,
    pub expected_revision: i32,
    pub name: String,
    pub text: String,
    pub work_ref: Option<Uuid>,
    pub deleted: bool,
}

pub async fn revise_research_query(
    database: &Database,
    request: &ReviseResearchQuery,
) -> Result<Value, CommentResearchError> {
    if !(0..i32::MAX).contains(&request.expected_revision)
        || !valid_text(&request.name, 100)
        || request.text.chars().count() > 200
    {
        return Err(CommentResearchError::InvalidCommand);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SELECT query_ref FROM linggan_comment_saved_query WHERE query_ref=$1 FOR UPDATE")
        .bind(request.query_ref)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CommentResearchError::SourceUnavailable)?;
    let current = sqlx::query(
        "SELECT revision,deleted FROM linggan_comment_query_current WHERE query_ref=$1",
    )
    .bind(request.query_ref)
    .fetch_one(&mut *tx)
    .await?;
    let existing =
        sqlx::query("SELECT * FROM linggan_comment_query_revision WHERE revision_ref=$1")
            .bind(request.revision_ref)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(row) = existing {
        if row.get::<Uuid, _>("query_ref") != request.query_ref
            || row.get::<i32, _>("revision") != request.expected_revision + 1
            || row.get::<String, _>("name") != request.name
            || row.get::<String, _>("query_text") != request.text
            || row.get::<Option<Uuid>, _>("work_public_ref") != request.work_ref
            || row.get::<bool, _>("deleted") != request.deleted
        {
            return Err(CommentResearchError::IdempotencyConflict);
        }
        return Ok(
            json!({"queryRef":request.query_ref,"revision":current.get::<i32,_>("revision"),
            "state":if current.get::<bool,_>("deleted"){"DELETED"}else{"SAVED"},"replayed":true}),
        );
    }
    if current.get::<i32, _>("revision") != request.expected_revision
        || current.get::<bool, _>("deleted")
    {
        return Err(CommentResearchError::RevisionConflict);
    }
    if let Some(work_ref) = request.work_ref {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_material_content WHERE public_ref=$1)",
        )
        .bind(work_ref)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(CommentResearchError::InvalidCommand);
        }
    }
    sqlx::query("INSERT INTO linggan_comment_query_revision(revision_ref,query_ref,revision,name,query_text,work_public_ref,deleted) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(request.revision_ref).bind(request.query_ref).bind(request.expected_revision+1)
        .bind(&request.name).bind(&request.text).bind(request.work_ref).bind(request.deleted).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"queryRef":request.query_ref,"revision":request.expected_revision+1,"state":if request.deleted{"DELETED"}else{"SAVED"}}),
    )
}

pub async fn read_comment_asset_history(
    database: &Database,
    asset_ref: Uuid,
) -> Result<Value, CommentResearchError> {
    let original = sqlx::query("SELECT * FROM linggan_comment_asset WHERE asset_ref=$1")
        .bind(asset_ref)
        .fetch_optional(database.pool())
        .await?
        .ok_or(CommentResearchError::SourceUnavailable)?;
    let source_ref: Uuid = original.get("source_ref");
    let readable = match read_comment_research_source(database, source_ref).await {
        Ok(_) => true,
        Err(CommentResearchReadError::SourceUnavailable) => false,
        Err(e) => return Err(e.into()),
    };
    let rows = sqlx::query("SELECT *,created_at::text AS changed_at FROM linggan_comment_asset_revision WHERE asset_ref=$1 ORDER BY revision DESC")
        .bind(asset_ref).fetch_all(database.pool()).await?;
    let mut items = rows.iter().map(|r|json!({"revision":r.get::<i32,_>("revision"),
        "reason":if readable{r.get::<Option<String>,_>("reason")}else{None},
        "changeReason":if readable{Some(r.get::<String,_>("change_reason"))}else{None},
        "collectionRef":r.get::<Option<Uuid>,_>("collection_ref"),"withdrawn":r.get::<bool,_>("withdrawn"),
        "changedAt":r.get::<String,_>("changed_at")})).collect::<Vec<_>>();
    items.push(json!({"revision":0,"reason":if readable{Some(original.get::<String,_>("reason"))}else{None},
        "changeReason":if readable{Some("初次收存")}else{None},"collectionRef":original.get::<Option<Uuid>,_>("collection_ref"),"withdrawn":false}));
    Ok(
        json!({"assetRef":asset_ref,"sourceRef":source_ref,"eligibility":if readable{"READABLE"}else{"SOURCE_UNAVAILABLE"},"items":items}),
    )
}
