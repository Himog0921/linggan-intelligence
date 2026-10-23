//! 加微 · 爆款长趋势 (PAGE-OPS-002 §3.3): by day for 90 days, by ISO week for 12 weeks, by month
//! for 6 months, all ending today; it does not follow the time slice. 加微 comes from daily
//! reports (all businesses); 爆款 counts by publishing day and follows the business selection.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::super::{
    LATEST_VIEWS_CTE, OpsAsOf, OpsBusiness, OpsReadError, ensure_business, read_ops_as_of,
};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewTrendPoint {
    pub start: String,
    /// `MM-DD`, `2026-W42` or `2026-10`.
    pub label: String,
    /// `None` when no daily report falls in the bucket: unknown, not 0.
    pub followers_gained: Option<i64>,
    /// `None` when none of the bucket has been migrated; covered zero remains `Some(0)`.
    pub hits: Option<i64>,
    pub coverage: String,
    pub covered_to: Option<String>,
    /// At least one native publication or daily row is present in this bucket.  It does not
    /// change the historical coverage state above.
    pub native_facts: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsOverviewTrend {
    pub as_of: Option<OpsAsOf>,
    pub granularity: &'static str,
    /// The last complete migrated day; later buckets are unknown or partially covered.
    pub complete_through: Option<String>,
    /// Latest day with a native fact in the displayed scope; sparse native entries are not a
    /// substitute for a continuously migrated historical window.
    pub native_latest_business_date: Option<String>,
    pub daily_scope: &'static str,
    pub points: Vec<OverviewTrendPoint>,
}

pub async fn read_ops_overview_trend(
    database: &Database,
    business: OpsBusiness,
    granularity: Option<&str>,
    today: Option<&str>,
) -> Result<OpsOverviewTrend, OpsReadError> {
    let granularity = match granularity.map(str::trim) {
        None | Some("") | Some("day") => "day",
        Some("week") => "week",
        Some("month") => "month",
        Some(_) => return Err(OpsReadError::InvalidParameter("granularity")),
    };
    ensure_business(database, business).await?;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE}, \
         t AS (SELECT coalesce($1::date, (now() AT TIME ZONE 'Asia/Shanghai')::date) AS today, \
                      (SELECT (exported_at AT TIME ZONE 'Asia/Shanghai')::date - 1 \
                         FROM linggan_ops_import_batch \
                        ORDER BY applied_at DESC, batch_ref DESC LIMIT 1) AS complete_through, \
                      (SELECT max(business_date) FROM ( \
                         SELECT d.business_date FROM linggan_ops_daily_report d \
                          WHERE d.source_system='linggan_native' AND d.missing_since_batch_ref IS NULL \
                         UNION ALL \
                         SELECT p.published_business_date FROM linggan_ops_publication p \
                          LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref \
                          WHERE p.source_system='linggan_native' AND p.merged_into_ref IS NULL \
                            AND p.missing_since_batch_ref IS NULL AND ($4::uuid IS NULL OR a.domain_ref=$4) \
                      ) native_dates) AS native_latest_business_date), \
         b AS (SELECT g::date AS start FROM t, generate_series( \
                 (CASE $2 WHEN 'day' THEN today - 89 \
                          WHEN 'week' THEN date_trunc('week', today::timestamp)::date - 77 \
                          ELSE (date_trunc('month', today::timestamp) - interval '5 months')::date END)::timestamp, \
                 (CASE $2 WHEN 'day' THEN today \
                          WHEN 'week' THEN date_trunc('week', today::timestamp)::date \
                          ELSE date_trunc('month', today::timestamp)::date END)::timestamp, \
                 CASE $2 WHEN 'day' THEN interval '1 day' \
                         WHEN 'week' THEN interval '1 week' ELSE interval '1 month' END) g), \
         c AS (SELECT b.start, t.complete_through, t.native_latest_business_date, least(t.today, \
                 CASE $2 WHEN 'day' THEN b.start WHEN 'week' THEN b.start + 6 \
                         ELSE (b.start + interval '1 month')::date - 1 END) AS bucket_end FROM b, t), \
         f AS (SELECT date_trunc($2, d.business_date::timestamp)::date AS start, sum(d.followers_gained)::bigint AS followers \
                 FROM linggan_ops_daily_report d, t \
                WHERE d.missing_since_batch_ref IS NULL \
                  AND d.business_date BETWEEN (SELECT min(start) FROM b) AND t.today \
                  AND (d.source_system='linggan_native' OR (t.complete_through IS NOT NULL AND d.business_date <= t.complete_through)) \
                GROUP BY 1), \
         h AS (SELECT date_trunc($2, p.published_business_date::timestamp)::date AS start, count(*) AS hits \
                 FROM linggan_ops_publication p \
                 LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
                 JOIN latest_views lv ON lv.publication_ref = p.publication_ref, t \
                WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
                  AND p.platform = 'xhs' AND lv.view_count >= $3 \
                  AND p.published_business_date BETWEEN (SELECT min(start) FROM b) AND t.today \
                  AND (p.source_system='linggan_native' OR (t.complete_through IS NOT NULL AND p.published_business_date <= t.complete_through)) \
                  AND ($4::uuid IS NULL OR a.domain_ref = $4) \
                GROUP BY 1), \
         native_daily AS (SELECT date_trunc($2,d.business_date::timestamp)::date AS start FROM linggan_ops_daily_report d, t \
                           WHERE d.source_system='linggan_native' AND d.missing_since_batch_ref IS NULL \
                             AND d.business_date BETWEEN (SELECT min(start) FROM b) AND t.today GROUP BY 1), \
         native_publication AS (SELECT date_trunc($2,p.published_business_date::timestamp)::date AS start \
                                  FROM linggan_ops_publication p LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref, t \
                                 WHERE p.source_system='linggan_native' AND p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
                                   AND p.published_business_date BETWEEN (SELECT min(start) FROM b) AND t.today \
                                   AND ($4::uuid IS NULL OR a.domain_ref=$4) GROUP BY 1), \
         native_bucket AS (SELECT start FROM native_daily UNION SELECT start FROM native_publication) \
         SELECT b.start::text AS start, \
                CASE $2 WHEN 'day' THEN to_char(b.start, 'MM-DD') \
                        WHEN 'week' THEN to_char(b.start, 'IYYY-\"W\"IW') \
                        ELSE to_char(b.start, 'YYYY-MM') END AS label, \
                f.followers, CASE WHEN b.start <= b.complete_through OR np.start IS NOT NULL THEN coalesce(h.hits, 0) END AS hits, \
                CASE WHEN b.complete_through IS NULL OR b.start > b.complete_through THEN 'not_migrated' \
                     WHEN b.bucket_end > b.complete_through THEN 'partial' ELSE 'full' END AS coverage, \
                CASE WHEN b.start <= b.complete_through THEN least(b.bucket_end, b.complete_through)::text END AS covered_to, \
                nb.start IS NOT NULL AS native_facts, b.complete_through::text AS complete_through, \
                b.native_latest_business_date::text AS native_latest_business_date \
           FROM c b LEFT JOIN f USING (start) LEFT JOIN h USING (start) \
                    LEFT JOIN native_bucket nb USING (start) LEFT JOIN native_publication np USING (start) \
          ORDER BY b.start"
    )))
    .bind(today)
    .bind(granularity)
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(business.domain_ref())
    .fetch_all(database.pool())
    .await?;
    let complete_through: Option<String> = match rows.first() {
        Some(row) => row.try_get("complete_through")?,
        None => None,
    };
    let native_latest_business_date: Option<String> = match rows.first() {
        Some(row) => row.try_get("native_latest_business_date")?,
        None => None,
    };
    let points = rows
        .iter()
        .map(|row| {
            Ok(OverviewTrendPoint {
                start: row.try_get("start")?,
                label: row.try_get("label")?,
                followers_gained: row.try_get("followers")?,
                hits: row.try_get("hits")?,
                coverage: row.try_get("coverage")?,
                covered_to: row.try_get("covered_to")?,
                native_facts: row.try_get("native_facts")?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(OpsOverviewTrend {
        as_of: read_ops_as_of(database).await?,
        granularity,
        complete_through,
        native_latest_business_date,
        daily_scope: "all_businesses",
        points,
    })
}
