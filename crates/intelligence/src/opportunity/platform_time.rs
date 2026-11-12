//! C3 platform-time projection and read-only reports. The projection itself lives in SQL so the
//! capture-package extraction rules have one implementation.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformTimeProjection {
    pub inserted: i64,
    pub conflicts: i64,
}

pub async fn project_comment_platform_times(
    database: &Database,
) -> Result<PlatformTimeProjection, sqlx::Error> {
    let row = sqlx::query(
        "SELECT inserted, conflicts FROM linggan_opportunity_project_comment_platform_times()",
    )
    .fetch_one(database.pool())
    .await?;
    Ok(PlatformTimeProjection {
        inserted: row.get("inserted"),
        conflicts: row.get("conflicts"),
    })
}

pub async fn current_as_of(database: &Database) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC',
                         'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
    )
    .fetch_one(database.pool())
    .await
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformTimeCoverageRow {
    pub period_start: String,
    pub eligible_comments: i64,
    pub with_platform_time: i64,
    pub ratio: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayTextMismatch {
    pub content_ref: String,
    pub comment_external_id: String,
    pub published_at: String,
    pub local_date: String,
    pub display_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayTextAgreement {
    pub compared: i64,
    pub matching: i64,
    pub ratio: Option<f64>,
    pub mismatches: Vec<DisplayTextMismatch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformTimeReport {
    pub coverage: Vec<PlatformTimeCoverageRow>,
    pub agreement: DisplayTextAgreement,
    pub timing_violations: i64,
}

pub async fn read_platform_time_report(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    time_zone: &str,
    periods: i32,
) -> Result<PlatformTimeReport, sqlx::Error> {
    let coverage = sqlx::query(
        "SELECT period_start::text AS period_start, eligible_comments, with_platform_time, ratio
         FROM linggan_opportunity_platform_time_coverage($1, $2::timestamptz, $3, $4)
         ORDER BY period_start DESC",
    )
    .bind(domain_ref)
    .bind(as_of)
    .bind(time_zone)
    .bind(periods)
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(|row| PlatformTimeCoverageRow {
        period_start: row.get("period_start"),
        eligible_comments: row.get("eligible_comments"),
        with_platform_time: row.get("with_platform_time"),
        ratio: row.get("ratio"),
    })
    .collect();

    let summary = sqlx::query(
        "SELECT compared, matching, ratio
         FROM linggan_opportunity_platform_time_display_agreement($1, $2::timestamptz, $3)",
    )
    .bind(domain_ref)
    .bind(as_of)
    .bind(time_zone)
    .fetch_one(database.pool())
    .await?;
    let mismatches = sqlx::query(
        "SELECT content_ref::text AS content_ref, comment_external_id,
                to_char(published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS published_at,
                local_date::text AS local_date, display_text
         FROM linggan_opportunity_platform_time_display_mismatches($1, $2::timestamptz, $3)",
    )
    .bind(domain_ref)
    .bind(as_of)
    .bind(time_zone)
    .fetch_all(database.pool())
    .await?
    .into_iter()
    .map(|row| DisplayTextMismatch {
        content_ref: row.get("content_ref"),
        comment_external_id: row.get("comment_external_id"),
        published_at: row.get("published_at"),
        local_date: row.get("local_date"),
        display_text: row.get("display_text"),
    })
    .collect();

    let timing_violations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment_platform_time platform
         JOIN linggan_opportunity_comment_fact fact
           ON fact.content_ref = platform.content_public_ref
          AND fact.comment_external_id = platform.comment_external_id
         WHERE platform.published_at > fact.first_observed_at",
    )
    .fetch_one(database.pool())
    .await?;

    Ok(PlatformTimeReport {
        coverage,
        agreement: DisplayTextAgreement {
            compared: summary.get("compared"),
            matching: summary.get("matching"),
            ratio: summary.get("ratio"),
            mismatches,
        },
        timing_violations,
    })
}
