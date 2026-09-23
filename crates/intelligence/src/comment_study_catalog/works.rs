//! Work-wide directory: server-side title search precedes UUID keyset pagination.
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use uuid::Uuid;

use super::{CLEANER_VERSION, CatalogStudyState, StudyCatalogError, cursor, literal_substring_pattern};
use super::read::{classify_read_error, resolve_as_of};

const WORK_SCOPE: &str = r#",
cs_title_scope AS MATERIALIZED (
    SELECT content.public_ref AS work_ref, content.created_at AS work_created_at
    FROM linggan_material_content content
    JOIN linggan_runtime_capture_package package ON package.package_ref = content.first_package_ref
    WHERE content.domain_ref = $1 AND content.created_at <= $2::timestamptz
      AND package.accepted_at <= $2::timestamptz
)"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkCatalogQuery {
    pub domain: Uuid,
    pub q: Option<String>,
    #[serde(default)]
    pub study_state: CatalogStudyState,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

struct Scope {
    pattern: Option<String>,
    hash: String,
    study: String,
    limit: i64,
}

impl WorkCatalogQuery {
    fn validate(&self) -> Result<Scope, StudyCatalogError> {
        if self.domain.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
            return Err(StudyCatalogError::UnsupportedDomain);
        }
        let limit = self.limit.unwrap_or(50);
        if !(1..=100).contains(&limit) { return Err(StudyCatalogError::InvalidLimit); }
        let q = self.q.as_deref().unwrap_or("");
        if q.contains('\0') { return Err(StudyCatalogError::InvalidQuery); }
        if matches!(self.study_state, CatalogStudyState::InputChanged) {
            return Err(StudyCatalogError::InputComparisonUnavailable);
        }
        let study = serde_json::to_value(self.study_state)
            .map_err(|_| StudyCatalogError::InvalidQuery)?;
        let hash = cursor::scope_hash(&json!({
            "resource":"works", "domain":self.domain, "q":q, "studyState":study,
            "cleanerVersion":CLEANER_VERSION, "sort":"work_ref_asc.v1"
        }))?;
        Ok(Scope {
            pattern: literal_substring_pattern(q)?, hash, limit,
            study: study.as_str().ok_or(StudyCatalogError::InvalidQuery)?.to_owned(),
        })
    }
}

fn statement(ocr_schema_ready: bool) -> Result<String, StudyCatalogError> {
    let titles = linggan_evidence::work_display_title_ctes(ocr_schema_ready)
        .map_err(|_| StudyCatalogError::ProjectionInvalid)?;
    // All fragments are owned compile-time constants. Caller values are only bind parameters.
    Ok(format!("{}{},\n{}\n{}", super::facts_sql(), WORK_SCOPE,
        titles, include_str!("works.sql")))
}

pub async fn read_work_catalog(
    database: &Database,
    query: &WorkCatalogQuery,
) -> Result<Value, StudyCatalogError> {
    read_page(database, query).await.map_err(classify_read_error)
}

async fn read_page(database: &Database, query: &WorkCatalogQuery) -> Result<Value, StudyCatalogError> {
    let scope = query.validate()?;
    let previous: Option<cursor::Cursor<cursor::EntryPosition>> = query.cursor.as_deref()
        .map(|value| cursor::decode_for("works", value, &scope.hash)).transpose()?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout = '15s'").execute(&mut *tx).await?;
    let as_of = resolve_as_of(&mut tx, previous.as_ref()).await?;
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_media_ocr_layout') IS NOT NULL \
          AND to_regclass('linggan_media_ocr_layering_result') IS NOT NULL \
          AND to_regclass('linggan_media_ocr_retirement') IS NOT NULL",
    ).fetch_one(&mut *tx).await?;
    let projection: Value = sqlx::query_scalar(AssertSqlSafe(statement(ready)?))
        .bind(query.domain).bind(&as_of).bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None).bind(Option::<&str>::None)
        .bind(&scope.pattern).bind(&scope.study)
        .bind(previous.as_ref().map(|cursor| cursor.last.reference))
        .bind(scope.limit + 1)
        .fetch_one(&mut *tx).await?;
    tx.commit().await?;
    response(query.domain, &scope, &as_of, projection)
}

fn response(domain: Uuid, scope: &Scope, as_of: &str, projection: Value) -> Result<Value, StudyCatalogError> {
    let mut rows = projection["rows"].as_array().ok_or(StudyCatalogError::ProjectionInvalid)?.clone();
    let total = projection["totalWorkCount"].as_u64().ok_or(StudyCatalogError::ProjectionInvalid)?;
    let has_more = rows.len() > scope.limit as usize;
    rows.truncate(scope.limit as usize);
    let next = if has_more {
        let row = rows.last().ok_or(StudyCatalogError::ProjectionInvalid)?;
        let last: cursor::EntryPosition = serde_json::from_value(row["position"].clone())
            .map_err(|_| StudyCatalogError::ProjectionInvalid)?;
        // createdAt checks the material cutoff; ordering is UUID only, not live study counts.
        Some(cursor::encode_for("works", &scope.hash, as_of, last)?)
    } else { None };
    let items: Vec<_> = rows.into_iter().map(|row| row["item"].clone()).collect();
    Ok(json!({
        "contract":"comment-study.read.v2", "domainRef":domain, "items":items,
        "totalWorkCount":total,
        "page":{"limit":scope.limit,"hasMore":has_more,"nextCursor":next,"asOf":as_of},
        "indexCoverage":projection["indexCoverage"]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> WorkCatalogQuery {
        serde_json::from_value(json!({"domain":crate::comment_study_source::ADHD_DOMAIN_REF})).unwrap()
    }

    #[test]
    fn work_scope_is_stable_across_page_sizes_but_not_queries() {
        let mut q = query();
        let hash = q.validate().unwrap().hash;
        q.limit = Some(1);
        assert_eq!(hash, q.validate().unwrap().hash);
        q.q = Some(String::new());
        assert_eq!(hash, q.validate().unwrap().hash);
        q.q = Some("药".into());
        assert_ne!(hash, q.validate().unwrap().hash);
    }

    #[test]
    fn work_queries_reject_invalid_or_unimplemented_inputs() {
        let mut q = query();
        for limit in [0, -1, 101] {
            q.limit = Some(limit);
            assert!(matches!(q.validate(), Err(StudyCatalogError::InvalidLimit)));
        }
        q.limit = None;
        q.study_state = CatalogStudyState::InputChanged;
        assert!(matches!(q.validate(), Err(StudyCatalogError::InputComparisonUnavailable)));
        assert!(serde_json::from_value::<WorkCatalogQuery>(json!({
            "domain":crate::comment_study_source::ADHD_DOMAIN_REF,"voiceRole":"all"
        })).is_err());
    }

    #[test]
    fn work_cursor_is_not_a_comment_history_cursor() {
        let q = query().validate().unwrap();
        let last = cursor::EntryPosition {
            created_at:"2026-09-21T00:00:00.000000Z".into(), reference:Uuid::from_u128(3),
        };
        let token = cursor::encode_for("works", &q.hash, "2026-09-23T00:00:00.000000Z", last).unwrap();
        assert!(matches!(cursor::decode_for::<cursor::EntryPosition>("comment-history", &token, &q.hash),
            Err(StudyCatalogError::CursorScopeMismatch)));
    }

    #[test]
    fn title_search_precedes_pagination_and_has_no_full_work_read_loop() {
        let sql = statement(true).unwrap();
        assert!(sql.contains(&super::super::facts_sql()));
        assert!(sql.contains("display_title ILIKE $6"));
        assert!(sql.find("display_title ILIKE $6").unwrap() < sql.find("ORDER BY work_ref ASC LIMIT $9").unwrap());
        assert!(!sql.contains("LIMIT 100"));
        assert!(!sql.contains("OFFSET "));
        assert!(!sql.contains("$1::uuid[]"));
    }
}
