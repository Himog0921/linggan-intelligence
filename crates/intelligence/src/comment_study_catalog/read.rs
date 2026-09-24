//! Bounded directory reads over retained comments, not over one Run's first 100 Targets.
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Postgres;
use super::cursor::CursorPosition;
use uuid::Uuid;

use super::{CLEANER_VERSION, StudyCatalogError, cursor, literal_substring_pattern};

fn page_sql() -> String { format!("{}{}", super::facts_sql(), include_str!("comments.sql")) }

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogVoiceRole {
    Reader,
    Creator,
    Unknown,
    #[default]
    ReaderAndUnknown,
    All,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogStudyState {
    #[default]
    All,
    NeverStudied,
    InProgress,
    Studied,
    NeedsContext,
    Failed,
    InputChanged,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentCatalogQuery {
    pub domain: Uuid,
    pub q: Option<String>,
    pub work_ref: Option<Uuid>,
    #[serde(default)]
    pub voice_role: CatalogVoiceRole,
    #[serde(default)]
    pub study_state: CatalogStudyState,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSummaryQuery {
    pub domain: Uuid,
    pub q: Option<String>,
    pub work_ref: Option<Uuid>,
    #[serde(default)]
    pub voice_role: CatalogVoiceRole,
    #[serde(default)]
    pub study_state: CatalogStudyState,
}

struct Scope {
    pattern: Option<String>,
    hash: String,
    voice: String,
    study: String,
    limit: i64,
}

impl CommentCatalogQuery {
    fn validate(&self) -> Result<Scope, StudyCatalogError> {
        if self.domain.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
            return Err(StudyCatalogError::UnsupportedDomain);
        }
        let limit = self.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) {
            return Err(StudyCatalogError::InvalidLimit);
        }
        if self.work_ref.is_some_and(|value| value.is_nil())
            || self.q.as_deref().is_some_and(|value| value.contains('\0'))
        {
            return Err(StudyCatalogError::InvalidQuery);
        }
        // P2 owns complete input-fingerprint comparison (including retained work/parent context).
        // Do not silently implement this filter as only a raw-text comparison.
        if matches!(self.study_state, CatalogStudyState::InputChanged) {
            return Err(StudyCatalogError::InputComparisonUnavailable);
        }
        let query = self.q.as_deref().unwrap_or("");
        let voice = serde_json::to_value(self.voice_role)
            .map_err(|_| StudyCatalogError::InvalidQuery)?;
        let study = serde_json::to_value(self.study_state)
            .map_err(|_| StudyCatalogError::InvalidQuery)?;
        let hash = cursor::scope_hash(&json!({
            "domain": self.domain, "q": query, "workRef": self.work_ref,
            "voiceRole": voice, "studyState": study, "cleanerVersion": CLEANER_VERSION,
            "sort": "received_desc_work_asc_comment_C_asc.v1"
        }))?;
        Ok(Scope {
            pattern: literal_substring_pattern(query)?,
            hash,
            voice: voice.as_str().ok_or(StudyCatalogError::InvalidQuery)?.to_owned(),
            study: study.as_str().ok_or(StudyCatalogError::InvalidQuery)?.to_owned(),
            limit,
        })
    }
}

pub async fn read_comment_catalog(
    database: &Database,
    query: &CommentCatalogQuery,
) -> Result<Value, StudyCatalogError> {
    read_catalog(database, query, false).await.map_err(classify_read_error)
}

pub async fn read_catalog_summary(
    database: &Database,
    query: &CatalogSummaryQuery,
) -> Result<Value, StudyCatalogError> {
    let query = CommentCatalogQuery {
        domain: query.domain,
        q: query.q.clone(),
        work_ref: query.work_ref,
        voice_role: query.voice_role,
        study_state: query.study_state,
        cursor: None,
        limit: Some(50),
    };
    read_catalog(database, &query, true).await.map_err(classify_read_error)
}

async fn read_catalog(
    database: &Database,
    query: &CommentCatalogQuery,
    summary_only: bool,
) -> Result<Value, StudyCatalogError> {
    let scope = query.validate()?;
    let previous = query.cursor.as_deref()
        .map(|value| cursor::decode(value, &scope.hash)).transpose()?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout = '15s'").execute(&mut *tx).await?;
    let as_of = resolve_as_of(&mut tx, previous.as_ref()).await?;
    let last = previous.as_ref().map(|value| &value.last);
    let mut projection: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(page_sql()))
        .bind(query.domain).bind(&as_of).bind(CLEANER_VERSION).bind(query.work_ref)
        .bind(Option::<&str>::None)
        .bind(&scope.pattern).bind(&scope.voice).bind(&scope.study)
        .bind(last.map(|value| value.received_at.as_str()))
        .bind(last.map(|value| value.work_ref))
        .bind(last.map(|value| value.comment_external_id.as_str()))
        .bind(if summary_only { 0 } else { scope.limit + 1 })
        .fetch_one(&mut *tx).await?;
    if !summary_only { enrich_titles(&mut tx,&mut projection,&as_of).await?; }
    tx.commit().await?;
    if summary_only {
        return Ok(json!({
            "contract": "comment-study.read.v2", "domainRef": query.domain,
            "asOf": as_of, "summary": projection["summary"],
            "indexCoverage": projection["indexCoverage"]
        }));
    }
    page_response(query.domain, &scope, &as_of, projection)
}

async fn enrich_titles(
    tx: &mut sqlx::Transaction<'_, Postgres>, projection: &mut Value, as_of: &str,
) -> Result<(),StudyCatalogError> {
    let rows=projection["rows"].as_array().ok_or(StudyCatalogError::ProjectionInvalid)?;
    let works: Vec<Uuid>=rows.iter().map(|row| {
        row.pointer("/item/commentKey/workRef").and_then(Value::as_str)
            .and_then(|v|Uuid::parse_str(v).ok()).ok_or(StudyCatalogError::ProjectionInvalid)
    }).collect::<Result<std::collections::BTreeSet<_>,_>>()?.into_iter().collect();
    let titles=crate::comment_study_source::context::read_titles(tx,&works,as_of).await?;
    for row in projection["rows"].as_array_mut().ok_or(StudyCatalogError::ProjectionInvalid)? {
        let id=row["item"]["commentKey"]["workRef"].as_str()
            .and_then(|s|Uuid::parse_str(s).ok()).ok_or(StudyCatalogError::ProjectionInvalid)?;
        let title=titles.get(&id).ok_or(StudyCatalogError::ProjectionInvalid)?;
        row["item"]["workTitle"]=title["displayTitle"].clone();
        row["item"]["workTitleSource"]=title["displayTitleSource"].clone();
    }
    Ok(())
}


/// Read one stable comment using exactly the same eligibility and projection as the directory.
/// Non-displayable sources return metadata, not a fallback to an older known version.
pub(super) async fn read_one_projection(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    external_id: &str,
    as_of: &str,
) -> Result<Value, StudyCatalogError> {
    let projection: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(page_sql()))
        .bind(domain).bind(as_of).bind(CLEANER_VERSION).bind(Some(work))
        .bind(Some(external_id))
        .bind(Option::<&str>::None).bind("all").bind("all")
        .bind(Option::<&str>::None).bind(Option::<Uuid>::None).bind(Option::<&str>::None)
        .bind(1_i64)
        .fetch_one(&mut **tx).await?;
    if projection.get("currentSource").is_none_or(Value::is_null) {
        return Err(StudyCatalogError::ResourceNotFound);
    }
    Ok(projection)
}

pub(super) async fn resolve_as_of<P: CursorPosition>(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    previous: Option<&cursor::Cursor<P>>,
) -> Result<String, StudyCatalogError> {
    let now: String = sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    ).fetch_one(&mut **tx).await?;
    match previous {
        None => Ok(now),
        Some(previous) => {
            // Dates are parameters and are calendar-checked by PostgreSQL, never interpolated.
            let valid: bool = sqlx::query_scalar(
                "SELECT $1::timestamptz <= $2::timestamptz AND $3::timestamptz <= $1::timestamptz",
            ).bind(&previous.as_of).bind(&now).bind(previous.last.timestamp())
                .fetch_one(&mut **tx).await?;
            if !valid { return Err(StudyCatalogError::InvalidCursor); }
            Ok(previous.as_of.clone())
        }
    }
}

fn page_response(
    domain: Uuid,
    scope: &Scope,
    as_of: &str,
    projection: Value,
) -> Result<Value, StudyCatalogError> {
    let mut rows = projection["rows"].as_array()
        .ok_or(StudyCatalogError::ProjectionInvalid)?.clone();
    let has_more = rows.len() > scope.limit as usize;
    rows.truncate(scope.limit as usize);
    let next_cursor = if has_more {
        let last = rows.last().ok_or(StudyCatalogError::ProjectionInvalid)?;
        let position = serde_json::from_value(last["position"].clone())
            .map_err(|_| StudyCatalogError::ProjectionInvalid)?;
        Some(cursor::encode(&scope.hash, as_of, position)?)
    } else {
        None
    };
    let items: Vec<Value> = rows.into_iter().map(|row| row["item"].clone()).collect();
    Ok(json!({
        "contract": "comment-study.read.v2", "domainRef": domain,
        "items": items,
        "page": { "limit": scope.limit, "hasMore": has_more, "nextCursor": next_cursor, "asOf": as_of },
        "indexCoverage": projection["indexCoverage"]
    }))
}

pub(super) fn classify_read_error(error: StudyCatalogError) -> StudyCatalogError {
    let StudyCatalogError::Database(database_error) = &error else { return error; };
    let code = database_error.as_database_error().and_then(|value| value.code()).map(|value| value.into_owned());
    match code.as_deref() {
        Some("42P01" | "42703" | "42883") => StudyCatalogError::SchemaUnavailable,
        Some("57014") => StudyCatalogError::QueryTimeout,
        Some("22007" | "22008") => StudyCatalogError::InvalidCursor,
        _ => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> CommentCatalogQuery {
        serde_json::from_value(json!({"domain": crate::comment_study_source::ADHD_DOMAIN_REF})).unwrap()
    }

    #[test]
    fn defaults_and_page_size_preserve_scope() {
        let mut query = query();
        let original = query.validate().unwrap();
        assert_eq!(original.voice, "reader_and_unknown");
        assert_eq!(original.study, "all");
        assert_eq!(original.limit, 50);
        query.limit = Some(1);
        assert_eq!(query.validate().unwrap().hash, original.hash);
        query.q = Some(String::new());
        assert_eq!(query.validate().unwrap().hash, original.hash);
        query.q = Some("药".to_owned());
        assert_ne!(query.validate().unwrap().hash, original.hash);
    }

    #[test]
    fn query_rejects_unbounded_or_unimplemented_filters_instead_of_guessing() {
        let mut query = query();
        for limit in [0, -1, 101] {
            query.limit = Some(limit);
            assert!(matches!(query.validate(), Err(StudyCatalogError::InvalidLimit)));
        }
        query.limit = None;
        query.study_state = CatalogStudyState::InputChanged;
        assert!(matches!(query.validate(), Err(StudyCatalogError::InputComparisonUnavailable)));
        query.study_state = CatalogStudyState::All;
        query.domain = Uuid::nil();
        assert!(matches!(query.validate(), Err(StudyCatalogError::UnsupportedDomain)));
        assert!(serde_json::from_value::<CommentCatalogQuery>(json!({
            "domain": crate::comment_study_source::ADHD_DOMAIN_REF, "origin": "scheduled"
        })).is_err());
    }
}
