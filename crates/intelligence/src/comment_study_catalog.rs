//! Deterministic catalog preparation. No Run creation, model call or permission caching.
//!
//! Directory reads do not activate the cache tick or the P2 shared selection/write path.
use crate::comment_cleaning::{CLEANER_VERSION, clean};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

#[path = "comment_study_catalog/cursor.rs"]
mod cursor;
#[path = "comment_study_catalog/read.rs"]
mod read;
#[path = "comment_study_catalog/detail.rs"]
mod detail;
#[path = "comment_study_catalog/works.rs"]
mod works;

pub use read::{
    CatalogStudyState, CatalogSummaryQuery, CatalogVoiceRole, CommentCatalogQuery,
    read_catalog_summary, read_comment_catalog,
};

pub use detail::{
    CommentDetailQuery, CommentHistoryQuery, read_comment_detail, read_comment_history,
    read_comment_versions,
};
pub use works::{WorkCatalogQuery, read_work_catalog};

const MAX_REFRESH_LIMIT: i64 = 200;
const MAX_QUERY_CHARS: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum StudyCatalogError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("invalid comment catalog argument")]
    InvalidQuery,
    #[error("comment cleaning cache conflicts with its immutable source")]
    CacheConflict,
    #[error("unsupported comment-study domain")]
    UnsupportedDomain,
    #[error("catalog limit must be between 1 and 100")]
    InvalidLimit,
    #[error("invalid catalog cursor")]
    InvalidCursor,
    #[error("catalog cursor belongs to a different query")]
    CursorScopeMismatch,
    #[error("the complete input comparison is not available yet")]
    InputComparisonUnavailable,
    #[error("the required catalog schema is unavailable")]
    SchemaUnavailable,
    #[error("the catalog query timed out")]
    QueryTimeout,
    #[error("the catalog projection does not match its contract")]
    ProjectionInvalid,
    #[error("the requested comment is absent or outside the selected domain")]
    ResourceNotFound,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanCacheRefresh {
    pub examined_count: usize,
    pub inserted_count: usize,
    pub already_present_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct CacheProjection {
    raw_sha256: String,
    research_text: String,
    clean_state: String,
    clean_reasons: Vec<String>,
}

fn projection(raw_prefix: &str, raw_sha256: String) -> Result<CacheProjection, StudyCatalogError> {
    // SQL bounds transfer to 16001 scalars while hashing the complete immutable raw UTF-8.
    // For short inputs the complete text is available, so verify SQL/Rust hash agreement too.
    if raw_prefix.chars().count() <= 16000
        && format!("{:x}", Sha256::digest(raw_prefix.as_bytes())) != raw_sha256
    {
        return Err(StudyCatalogError::CacheConflict);
    }
    let cleaned = clean(raw_prefix);
    Ok(CacheProjection {
        raw_sha256,
        research_text: cleaned.text,
        clean_state: cleaned.state,
        clean_reasons: cleaned.reasons,
    })
}

/// Materialize at most `limit` missing current-version cache entries in a bounded transaction.
/// A zero count means this pass found no candidate, not that the domain was fully researched.
/// Missing schema is an error, never an instruction to initialize or reset a database.
pub async fn refresh_clean_cache(
    database: &Database,
    domain_ref: Uuid,
    limit: i64,
) -> Result<CleanCacheRefresh, StudyCatalogError> {
    if domain_ref.is_nil() || !(1..=MAX_REFRESH_LIMIT).contains(&limit) {
        return Err(StudyCatalogError::InvalidQuery);
    }
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SET LOCAL statement_timeout = '15s'")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '3s'")
        .execute(&mut *transaction)
        .await?;
    let rows = sqlx::query(include_str!("comment_study_catalog/refresh_candidates.sql"))
        .bind(domain_ref)
        .bind(CLEANER_VERSION)
        .bind(limit)
        .fetch_all(&mut *transaction)
        .await?;
    let mut report = CleanCacheRefresh {
        examined_count: rows.len(),
        ..CleanCacheRefresh::default()
    };
    for row in rows {
        let source_ref: Uuid = row.try_get("material_ref")?;
        let raw_prefix: String = row.try_get("raw_prefix")?;
        let item = projection(&raw_prefix, row.try_get("raw_sha256")?)?;
        let written = sqlx::query(include_str!("comment_study_catalog/insert_clean_cache.sql"))
            .bind(source_ref)
            .bind(CLEANER_VERSION)
            .bind(&item.raw_sha256)
            .bind(&item.research_text)
            .bind(&item.clean_state)
            .bind(json!(item.clean_reasons))
            .bind(domain_ref)
            .execute(&mut *transaction)
            .await?;
        if written.rows_affected() == 1 {
            report.inserted_count += 1;
            continue;
        }
        // A concurrent refresher can win this key. Never turn conflicting content into success.
        let existing = sqlx::query(
            "SELECT raw_sha256,research_text,clean_state,clean_reasons \
             FROM linggan_comment_study_clean_cache WHERE source_ref=$1 AND cleaner_version=$2",
        )
        .bind(source_ref)
        .bind(CLEANER_VERSION)
        .fetch_optional(&mut *transaction)
        .await?;
        match existing {
            Some(existing) => {
                if existing.try_get::<String, _>("raw_sha256")? != item.raw_sha256
                    || existing.try_get::<String, _>("research_text")? != item.research_text
                    || existing.try_get::<String, _>("clean_state")? != item.clean_state
                    || existing.try_get::<Value, _>("clean_reasons")? != json!(item.clean_reasons)
                {
                    return Err(StudyCatalogError::CacheConflict);
                }
                report.already_present_count += 1;
            }
            None => report.skipped_count += 1, // source became unavailable before insertion
        }
    }
    transaction.commit().await?;
    Ok(report)
}

/// A bind parameter for `ILIKE $n ESCAPE E'\\\\'`, not a SQL expression or FTS query.
/// Whitespace and wildcard-looking characters are literal input. An empty string means no filter.
pub fn literal_substring_pattern(query: &str) -> Result<Option<String>, StudyCatalogError> {
    if query.chars().count() > MAX_QUERY_CHARS {
        return Err(StudyCatalogError::InvalidQuery);
    }
    if query.is_empty() {
        return Ok(None);
    }
    let mut pattern = String::with_capacity(query.len() + 2);
    pattern.push('%');
    for character in query.chars() {
        if matches!(character, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    Ok(Some(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_raw(raw: &str) -> CacheProjection {
        projection(raw, format!("{:x}", Sha256::digest(raw.as_bytes()))).unwrap()
    }

    #[test]
    fn projection_reuses_the_existing_cleaner_without_persisting_offsets() {
        for raw in ["😀😀", "@小明 ", "写作业很困难😀", "我也是", "", "Ａ&amp;Ｂ"] {
            let expected = clean(raw);
            let actual = from_raw(raw);
            assert_eq!(actual.research_text, expected.text);
            assert_eq!(actual.clean_state, expected.state);
            assert_eq!(actual.clean_reasons, expected.reasons);
            let object = serde_json::to_value(actual).unwrap();
            assert!(object.get("offsets").is_none());
            assert!(object.get("rawText").is_none());
        }
        assert_eq!(from_raw("😀😀").clean_state, "dropped");
        assert_eq!(from_raw("写作业很困难😀").clean_state, "direct");
    }

    #[test]
    fn overlong_source_is_anomaly_not_a_truncated_valid_comment() {
        let raw = "中".repeat(16001);
        let actual = from_raw(&raw);
        assert_eq!(actual.clean_state, "anomaly");
        assert!(actual.research_text.is_empty());
        assert_eq!(actual.clean_reasons, vec!["source_too_long"]);
        assert!(matches!(projection("证据", "0".repeat(64)), Err(StudyCatalogError::CacheConflict)));
    }

    #[test]
    fn literal_search_escapes_wildcards_and_keeps_short_chinese_queries() {
        assert_eq!(literal_substring_pattern("").unwrap(), None);
        assert_eq!(literal_substring_pattern("药").unwrap().as_deref(), Some("%药%"));
        assert_eq!(literal_substring_pattern(r"50%_\").unwrap().as_deref(), Some(r"%50\%\_\\%"));
        assert!(literal_substring_pattern(&"中".repeat(200)).is_ok());
        assert!(literal_substring_pattern(&"中".repeat(201)).is_err());
    }
}
