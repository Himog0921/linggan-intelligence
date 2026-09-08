//! The single source/qualification seam for local comment research. No platform user identity
//! crosses this interface; accepted material is not a claim about the outside population.

use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentResearchQuery {
    #[serde(default)]
    pub text: String,
    pub work_ref: Option<Uuid>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    as_of: String,
    after: Uuid,
    #[serde(default)]
    subset_key: String,
    text: String,
    work_ref: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchReadError {
    #[error("invalid research query")]
    InvalidQuery,
    #[error("source unavailable for current research use")]
    SourceUnavailable,
    #[error("comment research storage unavailable")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchCommentSource {
    pub source_ref: Uuid,
    pub work_ref: Uuid,
    pub package_ref: Uuid,
    pub body: Option<String>,
    pub body_state: String,
    pub body_truncated: bool,
    pub observed_at: String,
    pub is_reply: bool,
    pub is_current: bool,
    pub likes: Option<i64>,
    pub published_at: Option<i64>,
    pub published_at_text: Option<String>,
    pub accepted_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResearchPage {
    pub items: Vec<ResearchCommentSource>,
    pub total: i64,
    pub next_cursor: Option<String>,
    pub as_of: String,
    pub access_level: &'static str,
}

pub async fn read_comment_research(
    database: &Database,
    query: &CommentResearchQuery,
) -> Result<CommentResearchPage, CommentResearchReadError> {
    read_comment_research_subset(database, query, None, "").await
}

/// Optional caller-owned membership filter; the source owner still controls reads and counts.
pub async fn read_comment_research_subset(
    database: &Database,
    query: &CommentResearchQuery,
    source_refs: Option<&[Uuid]>,
    subset_key: &str,
) -> Result<CommentResearchPage, CommentResearchReadError> {
    if query.text.chars().count() > 200 {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let cursor: Option<Cursor> = query
        .cursor
        .as_ref()
        .map(|raw| {
            if raw.len() > 2000 {
                return Err(CommentResearchReadError::InvalidQuery);
            }
            serde_json::from_str(raw).map_err(|_| CommentResearchReadError::InvalidQuery)
        })
        .transpose()?;
    if cursor.as_ref().is_some_and(|c| {
        c.text != query.text || c.work_ref != query.work_ref || c.subset_key != subset_key
    }) {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of = match &cursor {
        Some(c) => c.as_of.clone(),
        None => {
            sqlx::query_scalar("SELECT scope_001_now()::text")
                .fetch_one(&mut *tx)
                .await?
        }
    };
    let valid_time: bool = sqlx::query_scalar("SELECT $1::timestamptz <= scope_001_now()")
        .bind(&as_of)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if error
                .as_database_error()
                .and_then(|e| e.code())
                .is_some_and(|code| matches!(code.as_ref(), "22007" | "22008"))
            {
                CommentResearchReadError::InvalidQuery
            } else {
                CommentResearchReadError::Database(error)
            }
        })?;
    if !valid_time {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let total = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_current($1::timestamptz) current
         JOIN linggan_comment_research_readable readable USING(material_ref)
         WHERE ($2::uuid IS NULL OR current.content_public_ref=$2)
         AND strpos(lower(COALESCE(current.body_text,'')),lower($3))>0 AND ($4::uuid[] IS NULL OR current.material_ref=ANY($4))",
    )
    .bind(&as_of)
    .bind(query.work_ref)
    .bind(&query.text)
    .bind(source_refs)
    .fetch_one(&mut *tx)
    .await?;
    let mut rows = sqlx::query(
        "SELECT current.*,true AS is_current FROM linggan_comment_research_current($1::timestamptz) current
         JOIN linggan_comment_research_readable readable USING(material_ref)
         WHERE ($2::uuid IS NULL OR current.content_public_ref=$2)
         AND strpos(lower(COALESCE(current.body_text,'')),lower($3))>0
         AND ($5::uuid[] IS NULL OR current.material_ref=ANY($5)) AND ($4::uuid IS NULL OR current.material_ref>$4) ORDER BY current.material_ref LIMIT 21")
        .bind(&as_of).bind(query.work_ref).bind(&query.text).bind(cursor.map(|c| c.after)).bind(source_refs)
        .fetch_all(&mut *tx).await?;
    let more = rows.len() > 20;
    rows.truncate(20);
    let mut items: Vec<_> = rows.iter().map(source_row).collect();
    let next_cursor = if more {
        items.last().map(|last| {
            serde_json::to_string(&Cursor {
                as_of: as_of.clone(),
                after: last.source_ref,
                subset_key: subset_key.into(),
                text: query.text.clone(),
                work_ref: query.work_ref,
            })
            .expect("cursor contains serializable fields")
        })
    } else {
        None
    };
    tx.commit().await?;
    enrich_comment_facts(database, &mut items).await?;
    Ok(CommentResearchPage {
        items,
        total,
        next_cursor,
        as_of,
        access_level: "LOCAL_AUTHORIZED_RESEARCH",
    })
}

pub async fn read_comment_research_source(
    database: &Database,
    source_ref: Uuid,
) -> Result<ResearchCommentSource, CommentResearchReadError> {
    let row = sqlx::query(
        "SELECT readable.*,EXISTS(SELECT 1 FROM linggan_material_comment_current current WHERE current.material_ref=readable.material_ref) AS is_current
         FROM linggan_comment_research_readable readable WHERE readable.material_ref=$1")
        .bind(source_ref).fetch_optional(database.pool()).await?
        .ok_or(CommentResearchReadError::SourceUnavailable)?;
    let mut sources = vec![source_row(&row)];
    enrich_comment_facts(database, &mut sources).await?;
    Ok(sources.remove(0))
}

fn source_row(row: &sqlx::postgres::PgRow) -> ResearchCommentSource {
    ResearchCommentSource {
        source_ref: row.get("material_ref"),
        work_ref: row.get("content_public_ref"),
        package_ref: row.get("package_ref"),
        body: row.get("body_text"),
        body_state: row.get("body_state"),
        body_truncated: false,
        observed_at: row.get("observed_at"),
        is_reply: row.get("is_reply"),
        is_current: row.get("is_current"),
        likes: None,
        published_at: None,
        published_at_text: None,
        accepted_at: None,
    }
}

pub async fn read_comment_research_context(
    database: &Database,
    source_ref: Uuid,
) -> Result<Value, CommentResearchReadError> {
    let source = read_comment_research_source(database, source_ref).await?;
    let comment_author: Option<String> = sqlx::query_scalar(
        "SELECT author_external_id FROM linggan_comment_research_readable WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await?;
    let mut identity_tx = database.pool().begin().await?;
    let identity_as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *identity_tx)
        .await?;
    let identity = crate::work_resource_current::read_work_resource_currents(
        &mut identity_tx,
        &[source.work_ref],
        &identity_as_of,
    )
    .await?;
    let role = if comment_author.as_ref().is_some_and(|author| {
        identity.first().and_then(|w| w.author_external_id.as_ref()) == Some(author)
    }) {
        "author"
    } else {
        "unknown"
    };
    identity_tx.commit().await?;
    let parent = sqlx::query(
        "SELECT parent.*,true AS is_current FROM linggan_comment_research_readable source
         JOIN linggan_material_comment_current parent ON parent.content_public_ref=source.content_public_ref
         AND parent.comment_external_id=source.parent_comment_external_id
         JOIN linggan_comment_research_readable allowed ON allowed.material_ref=parent.material_ref
         WHERE source.material_ref=$1")
        .bind(source_ref).fetch_optional(database.pool()).await?.map(|r| source_row(&r));
    let work = crate::work_resource_read::read_work_resource(database, source.work_ref)
        .await
        .map_err(|_| sqlx::Error::Protocol("work resource context unavailable".into()))?;
    let allowed_slots: std::collections::BTreeSet<String> = work
        .as_ref()
        .and_then(|w| w.inspector.get("mediaSlots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|s| {
            s["purpose"]
                .as_str()
                .is_some_and(|p| matches!(p, "cover" | "body_image" | "video" | "live_photo"))
        })
        .filter_map(|s| s["slotKey"].as_str().map(str::to_owned))
        .collect();
    let derivatives = work
        .as_ref()
        .and_then(|w| w.inspector.get("derivatives"))
        .and_then(Value::as_array)
        .map(|ds| {
            json!(
                ds.iter()
                    .filter(|d| d["slotKey"]
                        .as_str()
                        .is_some_and(|s| allowed_slots.contains(s)))
                    .collect::<Vec<_>>()
            )
        })
        .unwrap_or_else(|| json!([]));
    let mut work_context = work.and_then(|work| work.inspector.get("detailCurrent").cloned());
    if let Some(context) = &mut work_context
        && let Some(body) = context
            .pointer("/body/value")
            .and_then(Value::as_str)
            .map(str::to_owned)
    {
        context["body"]["truncated"] = json!(body.chars().count() > 4000);
        context["body"]["value"] = json!(body.chars().take(4000).collect::<String>());
    }
    Ok(
        json!({"source":source,"role":role,"parent":parent,"parentState":if parent.is_some(){"AVAILABLE"}else if source.is_reply{"MISSING_OR_RESTRICTED"}else{"NOT_APPLICABLE"},
        "derivatives":derivatives,"work":work_context,"workUrl":format!("/api/local/work-resources/{}", source.work_ref)}),
    )
}

/// Shares the caller's read snapshot with the authoritative field-wise Work Current projection.
pub async fn read_comment_work_contexts_in_snapshot(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_refs: &[Uuid],
    as_of: &str,
) -> Result<Value, CommentResearchReadError> {
    if work_refs.len() > 250 {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let works =
        crate::work_resource_current::read_work_resource_currents(tx, work_refs, as_of).await?;
    Ok(json!(works.into_iter().map(|w|json!({"workRef":w.public_ref,"title":w.title,"creatorDisplayName":w.creator_display_name})).collect::<Vec<_>>()))
}

/// Batch projection through the existing Work Resource Current owner, inside Evidence.
pub async fn read_comment_work_contexts(
    database: &Database,
    work_refs: &[Uuid],
) -> Result<Value, CommentResearchReadError> {
    if work_refs.len() > 20 {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let mut tx = database.pool().begin().await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let works =
        crate::work_resource_current::read_work_resource_currents(&mut tx, work_refs, &as_of)
            .await?;
    let result=works.into_iter().map(|w| json!({"workRef":w.public_ref,"title":w.title,"titleState":w.title_state,
        "creatorDisplayName":w.creator_display_name,"creatorDisplayNameState":w.creator_display_name_state,
        "publishedAt":w.published_at,"publishedAtPrecision":w.published_at_precision,"publishedAtSourceText":w.published_at_source_text})).collect::<Vec<_>>();
    tx.commit().await?;
    Ok(json!(result))
}

pub async fn restrict_comment_research_source(
    database: &Database,
    source_ref: Uuid,
    reason: &str,
) -> Result<(), CommentResearchReadError> {
    if reason.trim().is_empty() || reason.chars().count() > 500 {
        return Err(CommentResearchReadError::InvalidQuery);
    }
    let changed=sqlx::query(
        "INSERT INTO linggan_comment_research_restriction(content_public_ref,comment_external_id,reason)
         SELECT content_public_ref,comment_external_id,$2 FROM linggan_material_comment WHERE material_ref=$1
         ON CONFLICT(content_public_ref,comment_external_id) DO NOTHING")
        .bind(source_ref).bind(reason).execute(database.pool()).await?;
    if changed.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_material_comment WHERE material_ref=$1)",
        )
        .bind(source_ref)
        .fetch_one(database.pool())
        .await?;
        if !exists {
            return Err(CommentResearchReadError::SourceUnavailable);
        }
    }
    Ok(())
}

pub async fn comment_research_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT to_regclass('linggan_comment_research_readable') IS NOT NULL AND to_regclass('linggan_comment_asset') IS NOT NULL")
        .fetch_one(database.pool()).await
}

/// Strict field projection from the very same accepted record, never cross-observation fallback.
async fn enrich_comment_facts(
    db: &Database,
    sources: &mut [ResearchCommentSource],
) -> Result<(), CommentResearchReadError> {
    let refs: Vec<_> = sources.iter().map(|s| s.source_ref).collect();
    let rows=sqlx::query("SELECT s.material_ref,p.payload->'records'->s.record_ordinal->'payload' AS record,p.accepted_at::text AS accepted_at FROM linggan_comment_research_readable s JOIN linggan_runtime_capture_package p USING(package_ref) WHERE s.material_ref=ANY($1)").bind(refs).fetch_all(db.pool()).await?;
    for r in rows {
        if let Some(s) = sources
            .iter_mut()
            .find(|s| s.source_ref == r.get::<Uuid, _>("material_ref"))
        {
            let v: Option<Value> = r.get("record");
            if let Some(v) = v {
                s.likes = v.get("likes").and_then(Value::as_i64).filter(|n| *n >= 0);
                s.published_at = v
                    .get("publishedAt")
                    .and_then(Value::as_i64)
                    .filter(|n| *n > 0 && *n < 253402300800000);
                s.published_at_text = v
                    .get("publishedAtText")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty() && s.chars().count() <= 100)
                    .map(str::to_owned);
            }
            s.accepted_at = Some(r.get("accepted_at"));
        }
    }
    Ok(())
}

pub async fn read_comment_work_options(db: &Database) -> Result<Value, CommentResearchReadError> {
    let refs:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT c.content_public_ref FROM linggan_material_comment_current c JOIN linggan_comment_research_readable r USING(material_ref) ORDER BY c.content_public_ref LIMIT 201").fetch_all(db.pool()).await?;
    let mut items = vec![];
    for chunk in refs
        .iter()
        .take(200)
        .copied()
        .collect::<Vec<_>>()
        .chunks(20)
    {
        if let Some(works) = read_comment_work_contexts(db, chunk).await?.as_array() {
            items.extend(works.clone());
        }
    }
    Ok(json!({"items":items,"truncated":refs.len()>200}))
}
