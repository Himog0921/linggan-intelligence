//! 每日数据 list (OPS-HTTP-001 §3): the latest 200 operator-days. Daily reports are kept per
//! operator, not per business, so the business selection does not narrow them.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use super::{
    LATEST_VIEWS_CTE, OpsAsOf, OpsBusiness, OpsReadError, ensure_business, names, read_ops_as_of,
};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

pub const OPS_DAILY_LIMIT: i64 = 200;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsDailyItem {
    pub r#ref: Uuid,
    pub source_id: String,
    pub business_date: String,
    pub weekday: &'static str,
    pub operator: String,
    pub published: i64,
    pub links_opened: i64,
    pub followers_gained: i64,
    pub add_rate: Option<f64>,
    pub hits: i64,
    pub month_followers_gained: i64,
    pub month_published: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsDailyPage {
    pub as_of: Option<OpsAsOf>,
    pub scope: &'static str,
    pub limit: i64,
    pub operators: Vec<String>,
    pub items: Vec<OpsDailyItem>,
}

/// Published and hit counts are recounted from kept registrations by publisher name and day;
/// running month totals are per operator. `$1` operator, `$2` hit threshold, `$3` row limit.
const DAILY_SQL: &str = "per_day AS ( \
    SELECT p.publisher_name, p.published_business_date AS business_date, count(*) AS published, \
           count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2) AS hits \
      FROM linggan_ops_publication p \
      LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
     WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL AND p.publisher_name IS NOT NULL \
     GROUP BY 1, 2), \
  days AS ( \
    SELECT d.daily_ref, d.source_record_id, d.business_date, d.operator_name, d.links_opened_count, d.followers_gained, \
           coalesce(pd.published, 0) AS published, coalesce(pd.hits, 0) AS hits \
      FROM linggan_ops_daily_report d \
      LEFT JOIN per_day pd ON pd.publisher_name = d.operator_name AND pd.business_date = d.business_date \
     WHERE d.missing_since_batch_ref IS NULL AND ($1::text IS NULL OR d.operator_name = $1)) \
  SELECT daily_ref, source_record_id, business_date::text AS business_date, extract(isodow FROM business_date)::int AS isodow, \
         operator_name, links_opened_count, followers_gained, published, hits, \
         (sum(followers_gained) OVER month_window)::bigint AS month_followers, \
         (sum(published) OVER month_window)::bigint AS month_published \
    FROM days \
  WINDOW month_window AS (PARTITION BY operator_name, date_trunc('month', business_date) \
                          ORDER BY business_date ROWS UNBOUNDED PRECEDING) \
   ORDER BY business_date DESC, operator_name, daily_ref \
   LIMIT $3";

pub async fn read_ops_daily(
    database: &Database,
    business: OpsBusiness,
    operator: Option<&str>,
    limit: Option<i64>,
) -> Result<OpsDailyPage, OpsReadError> {
    ensure_business(database, business).await?;
    let operator = operator.map(str::trim).filter(|text| !text.is_empty());
    let limit = limit.unwrap_or(OPS_DAILY_LIMIT);
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE}, {DAILY_SQL}"
    )))
    .bind(operator)
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    let items = rows.iter().map(item).collect::<Result<Vec<_>, _>>()?;
    let operators = names(
        sqlx::query(
            "SELECT DISTINCT operator_name FROM linggan_ops_daily_report \
              WHERE missing_since_batch_ref IS NULL ORDER BY 1",
        )
        .fetch_all(database.pool())
        .await?,
    )?;
    Ok(OpsDailyPage {
        as_of: read_ops_as_of(database).await?,
        scope: "all_businesses",
        limit,
        operators,
        items,
    })
}

fn item(row: &sqlx::postgres::PgRow) -> Result<OpsDailyItem, OpsReadError> {
    let links: i32 = row.try_get("links_opened_count")?;
    let followers: i32 = row.try_get("followers_gained")?;
    Ok(OpsDailyItem {
        r#ref: row.try_get("daily_ref")?,
        source_id: row.try_get("source_record_id")?,
        business_date: row.try_get("business_date")?,
        weekday: weekday(row.try_get("isodow")?),
        operator: row.try_get("operator_name")?,
        published: row.try_get("published")?,
        links_opened: i64::from(links),
        followers_gained: i64::from(followers),
        add_rate: add_rate(i64::from(followers), i64::from(links)),
        hits: row.try_get("hits")?,
        month_followers_gained: row.try_get("month_followers")?,
        month_published: row.try_get("month_published")?,
    })
}

/// 添加率 = 加微 ÷ 打开; with no opened links the rate is undefined, not 0.
pub(crate) fn add_rate(followers: i64, links: i64) -> Option<f64> {
    (links > 0).then(|| followers as f64 / links as f64)
}

fn weekday(isodow: i32) -> &'static str {
    match isodow {
        1 => "星期一",
        2 => "星期二",
        3 => "星期三",
        4 => "星期四",
        5 => "星期五",
        6 => "星期六",
        _ => "星期日",
    }
}

#[cfg(test)]
mod tests {
    use super::add_rate;

    #[test]
    fn add_rate_is_unknown_without_opened_links() {
        assert_eq!(add_rate(3, 0), None);
        assert_eq!(add_rate(0, 4), Some(0.0));
        assert_eq!(add_rate(1, 4), Some(0.25));
    }
}
