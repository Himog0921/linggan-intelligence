//! Read existing method versions without provider calls or automatic historic reconstruction.
use super::store::{SUMMARY_COLUMNS, StudyPolicyStoreError, ensure_schema, load_policy, summary, validate_domain};
use crate::comment_study_catalog::{StudyCatalogError, cursor::{self, EntryPosition}};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyPolicyQuery {
    pub domain: Uuid,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

pub async fn read_study_policy(
    database: &Database, domain: Uuid, reference: Uuid,
) -> Result<Value, StudyPolicyStoreError> {
    validate_domain(domain)?;
    if reference.is_nil() { return Err(StudyPolicyStoreError::NotFound); }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    ensure_schema(&mut tx, false).await?;
    let policy = load_policy(&mut tx, domain, reference).await?;
    tx.commit().await?;
    Ok(json!({"contract":"comment-study.read.v2","domainRef":domain,"policy":policy}))
}

pub async fn read_study_policies(
    database: &Database, query: &StudyPolicyQuery,
) -> Result<Value, StudyPolicyStoreError> {
    validate_domain(query.domain)?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) { return Err(StudyCatalogError::InvalidLimit.into()); }
    let scope = cursor::scope_hash(&json!({"domain":query.domain,"sort":"created_at_desc,policy_ref_desc"}))?;
    let position = query.cursor.as_deref().map(|value|
        cursor::decode_for::<EntryPosition>("policies", value, &scope)).transpose()?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    ensure_schema(&mut tx, false).await?;
    let as_of: String = match &position {
        Some(cursor) => cursor.as_of.clone(),
        None => sqlx::query_scalar("SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')")
            .fetch_one(&mut *tx).await?,
    };
    let statement = format!("SELECT {SUMMARY_COLUMNS} FROM linggan_comment_study_policy p \
        WHERE p.domain_ref=$1 AND p.created_at<=$2::text::timestamptz \
        AND ($3::text IS NULL OR (p.created_at,p.policy_ref)<($3::text::timestamptz,$4)) \
        ORDER BY p.created_at DESC,p.policy_ref DESC LIMIT $5");
    let rows = sqlx::query(sqlx::AssertSqlSafe(statement)).bind(query.domain).bind(&as_of)
        .bind(position.as_ref().map(|p|p.last.created_at.as_str()))
        .bind(position.as_ref().map(|p|p.last.reference)).bind(limit+1)
        .fetch_all(&mut *tx).await?;
    let has_more = rows.len() > limit as usize;
    let rows = &rows[..rows.len().min(limit as usize)];
    let items = rows.iter().map(summary).collect::<Result<Vec<_>,_>>()?;
    let next = if has_more {
        let row = rows.last().ok_or(StudyCatalogError::InvalidCursor)?;
        Some(cursor::encode_for("policies", &scope, &as_of, EntryPosition {
            created_at:row.try_get("created_at")?, reference:row.try_get("policy_ref")?,
        })?)
    } else { None };
    tx.commit().await?;
    Ok(json!({"contract":"comment-study.read.v2","domainRef":query.domain,"items":items,
        "page":{"limit":limit,"hasMore":has_more,"nextCursor":next,"asOf":as_of}}))
}
