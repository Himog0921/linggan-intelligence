//! Typed, batchable owner of Work Resource field-wise Current.
//!
//! This module is intentionally crate-private. Page adapters continue to consume the small
//! `read_work_resource(s)` interface; derived read models can reuse Current inside the same
//! transaction and `as_of` without inventing another HTTP truth seam or issuing N+1 reads.

use sqlx::{postgres::PgRow, AssertSqlSafe, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct WorkResourceCurrentSource {
    pub material_ref: Option<Uuid>,
    pub package_ref: Option<Uuid>,
    pub record_ordinal: Option<i32>,
    pub source_lane: Option<String>,
    pub observed_at: Option<String>,
    pub recorded_at: Option<String>,
}

#[derive(Debug)]
pub(crate) struct WorkResourceCurrent {
    pub platform: String,
    pub content_external_id: String,
    pub public_ref: Uuid,
    pub detail_material_ref: Option<Uuid>,
    pub package_ref: Option<Uuid>,
    pub record_ordinal: Option<i32>,
    pub observed_at: String,
    pub title: Option<String>,
    pub title_state: String,
    pub title_source: WorkResourceCurrentSource,
    pub body_text: Option<String>,
    pub body_state: String,
    pub body_source: WorkResourceCurrentSource,
    pub creator_display_name: Option<String>,
    pub creator_display_name_state: String,
    pub creator_source: WorkResourceCurrentSource,
    pub published_at: Option<String>,
    pub published_local_date: Option<String>,
    pub published_at_epoch_ms: Option<i64>,
    pub published_at_source_text: Option<String>,
    pub published_at_source_field: Option<String>,
    pub published_at_source_kind: String,
    pub published_at_precision: String,
    pub published_at_reference_observed_at: Option<String>,
    pub published_at_parser_version: Option<String>,
    pub published_source: WorkResourceCurrentSource,
    pub author_external_id: Option<String>,
    pub cover_source_state: String,
    pub like_count: Option<i64>,
    pub like_count_state: String,
    pub like_source: WorkResourceCurrentSource,
    pub comment_count: Option<i64>,
    pub comment_count_state: String,
    pub comment_source: WorkResourceCurrentSource,
    pub collect_count: Option<i64>,
    pub collect_count_state: String,
    pub collect_source: WorkResourceCurrentSource,
    pub share_count: Option<i64>,
    pub share_count_state: String,
    pub share_source: WorkResourceCurrentSource,
}

pub(crate) struct WorkResourceCurrentPageQuery<'a> {
    pub text: Option<&'a str>,
    pub as_of: &'a str,
    pub observed_before: Option<&'a str>,
    pub platform_after: Option<&'a str>,
    pub content_external_id_after: Option<&'a str>,
    pub lane: Option<&'a str>,
    pub media_kind: Option<&'a str>,
    pub one_public_ref: Option<Uuid>,
}

pub(crate) async fn read_work_resource_current_page(
    tx: &mut Transaction<'_, Postgres>,
    query: WorkResourceCurrentPageQuery<'_>,
) -> Result<Vec<WorkResourceCurrent>, sqlx::Error> {
    let ocr_retirement_schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_ocr_retirement') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await?;
    let sql = crate::material_query_sql::material_page_sql(ocr_retirement_schema_ready);
    // Both query strings are assembled only from private compile-time literals in
    // `material_query_sql`; no caller input is interpolated. Runtime values stay bound.
    sqlx::query(AssertSqlSafe(sql))
        .bind(query.text)
        .bind(query.as_of)
        .bind(query.observed_before)
        .bind(query.platform_after)
        .bind(query.content_external_id_after)
        .bind(query.lane)
        .bind(query.media_kind)
        .bind(query.one_public_ref)
        .fetch_all(&mut **tx)
        .await
        .map(|rows| rows.into_iter().map(map_current).collect())
}

pub(crate) async fn read_work_resource_currents(
    tx: &mut Transaction<'_, Postgres>,
    public_refs: &[Uuid],
    as_of: &str,
) -> Result<Vec<WorkResourceCurrent>, sqlx::Error> {
    if public_refs.is_empty() {
        return Ok(Vec::new());
    }
    let sql = crate::material_query_sql::work_resource_currents_sql();
    sqlx::query(AssertSqlSafe(sql))
        .bind(public_refs)
        .bind(as_of)
        .fetch_all(&mut **tx)
        .await
        .map(|rows| rows.into_iter().map(map_current).collect())
}

fn map_current(row: PgRow) -> WorkResourceCurrent {
    WorkResourceCurrent {
        platform: row.get("platform"),
        content_external_id: row.get("content_external_id"),
        public_ref: row.get("public_ref"),
        detail_material_ref: row.get("detail_material_ref"),
        package_ref: row.get("package_ref"),
        record_ordinal: row.get("record_ordinal"),
        observed_at: row.get("observed_at"),
        title: row.get("title"),
        title_state: row.get("title_state"),
        title_source: detail_source(&row, "title"),
        body_text: row.get("body_text"),
        body_state: row.get("body_state"),
        body_source: detail_source(&row, "body"),
        creator_display_name: row.get("creator_display_name"),
        creator_display_name_state: row.get("creator_display_name_state"),
        creator_source: detail_source(&row, "creator"),
        published_at: row.get("published_at"),
        published_local_date: row.get("published_local_date"),
        published_at_epoch_ms: row.get("published_at_epoch_ms"),
        published_at_source_text: row.get("published_at_source_text"),
        published_at_source_field: row.get("published_at_source_field"),
        published_at_source_kind: row.get("published_at_source_kind"),
        published_at_precision: row.get("published_at_precision"),
        published_at_reference_observed_at: row.get("published_at_reference_observed_at"),
        published_at_parser_version: row.get("published_at_parser_version"),
        published_source: detail_source(&row, "published"),
        author_external_id: row.get("author_external_id"),
        cover_source_state: row.get("cover_source_state"),
        like_count: row.get("like_count"),
        like_count_state: row.get("like_count_state"),
        like_source: engagement_source(&row, "like"),
        comment_count: row.get("comment_count"),
        comment_count_state: row.get("comment_count_state"),
        comment_source: engagement_source(&row, "comment"),
        collect_count: row.get("collect_count"),
        collect_count_state: row.get("collect_count_state"),
        collect_source: engagement_source(&row, "collect"),
        share_count: row.get("share_count"),
        share_count_state: row.get("share_count_state"),
        share_source: engagement_source(&row, "share"),
    }
}

fn detail_source(row: &PgRow, prefix: &str) -> WorkResourceCurrentSource {
    WorkResourceCurrentSource {
        material_ref: row.get(format!("{prefix}_source_material_ref").as_str()),
        package_ref: row.get(format!("{prefix}_source_package_ref").as_str()),
        record_ordinal: row.get(format!("{prefix}_source_record_ordinal").as_str()),
        source_lane: None,
        observed_at: row.get(format!("{prefix}_source_observed_at").as_str()),
        recorded_at: row.get(format!("{prefix}_source_recorded_at").as_str()),
    }
}

fn engagement_source(row: &PgRow, prefix: &str) -> WorkResourceCurrentSource {
    WorkResourceCurrentSource {
        material_ref: None,
        package_ref: row.get(format!("{prefix}_source_package_ref").as_str()),
        record_ordinal: None,
        source_lane: row.get(format!("{prefix}_source_lane").as_str()),
        observed_at: row.get(format!("{prefix}_source_observed_at").as_str()),
        recorded_at: row.get(format!("{prefix}_source_recorded_at").as_str()),
    }
}
