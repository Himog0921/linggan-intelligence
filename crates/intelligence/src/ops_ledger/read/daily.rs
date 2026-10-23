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
    pub source: &'static str,
    /// Present only for a native row; callers must send it back to replace that row.
    pub native_version: Option<i64>,
    pub business_date: String,
    pub weekday: &'static str,
    pub operator: String,
    pub published: i64,
    /// Null means the operator did not report this metric; it is distinct from a reported zero.
    pub links_opened: Option<i64>,
    /// Null means the operator did not report this metric; it is distinct from a reported zero.
    pub followers_gained: Option<i64>,
    pub add_rate: Option<f64>,
    pub hits: i64,
    /// Null means there are no reported follower values for this operator-month yet.
    pub month_followers_gained: Option<i64>,
    pub month_published: i64,
    /// 执行状态 (PAGE-OPS-002 §4): `not_published`, `not_filled`, `praise`,
    /// `pending_views` or `done`.
    pub execution: &'static str,
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
    SELECT coalesce(p.publisher_name,publisher.display_name) AS publisher_name, \
           coalesce(p.published_business_date,p.native_published_on) AS business_date, count(*) AS published, \
           count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2) AS hits, \
           count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count IS NULL \
                              AND (now() AT TIME ZONE 'Asia/Shanghai')::date \
                                  - coalesce(p.published_business_date,p.native_published_on) >= 7) AS pending_overdue \
      FROM linggan_ops_publication p \
      LEFT JOIN linggan_ops_member publisher ON publisher.member_ref = p.native_publisher_member_ref \
      LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
     WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
       AND (p.source_system <> 'linggan_native' OR p.native_record_state = 'active') \
       AND coalesce(p.publisher_name,publisher.display_name) IS NOT NULL \
     GROUP BY 1, 2), \
  days AS ( \
    SELECT d.daily_ref, d.source_record_id, d.source_system, d.native_version, d.business_date, d.operator_name, d.links_opened_count, d.followers_gained, \
           coalesce(pd.published, 0) AS published, coalesce(pd.hits, 0) AS hits, \
           coalesce(pd.pending_overdue, 0) AS pending_overdue \
      FROM linggan_ops_daily_report d \
      LEFT JOIN per_day pd ON pd.publisher_name = d.operator_name AND pd.business_date = d.business_date \
     WHERE d.missing_since_batch_ref IS NULL AND ($1::text IS NULL OR d.operator_name = $1)) \
  SELECT daily_ref, source_record_id, source_system, native_version, business_date::text AS business_date, extract(isodow FROM business_date)::int AS isodow, \
         operator_name, links_opened_count, followers_gained, published, hits, pending_overdue, \
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
    let links: Option<i32> = row.try_get("links_opened_count")?;
    let followers: Option<i32> = row.try_get("followers_gained")?;
    let links = links.map(i64::from);
    let followers = followers.map(i64::from);
    Ok(OpsDailyItem {
        r#ref: row.try_get("daily_ref")?,
        source_id: row.try_get("source_record_id")?,
        source: match row.try_get::<String, _>("source_system")?.as_str() {
            "content_workbench" => "imported",
            "linggan_native" => "native",
            _ => return Err(OpsReadError::InvalidParameter("daily_source")),
        },
        native_version: row.try_get("native_version")?,
        business_date: row.try_get("business_date")?,
        weekday: weekday(row.try_get("isodow")?),
        operator: row.try_get("operator_name")?,
        published: row.try_get("published")?,
        links_opened: links,
        followers_gained: followers,
        add_rate: add_rate(followers, links),
        hits: row.try_get("hits")?,
        month_followers_gained: row.try_get("month_followers")?,
        month_published: row.try_get("month_published")?,
        execution: execution(
            row.try_get("published")?,
            row.try_get("hits")?,
            followers,
            row.try_get("pending_overdue")?,
        ),
    })
}

/// 未完成, then 可表扬, then 待补阅读量, else 已完成. 「需复盘」 waits until 加微 0 and 没填 can be told
/// apart (D4b).
pub(crate) fn execution(
    published: i64,
    hits: i64,
    followers: Option<i64>,
    pending_overdue: i64,
) -> &'static str {
    if published == 0 {
        "not_published"
    } else if followers.is_none() {
        "not_filled"
    } else if hits > 0 || followers.is_some_and(|count| count > 0) {
        "praise"
    } else if pending_overdue > 0 {
        "pending_views"
    } else {
        "done"
    }
}

/// 添加率 = 加微 ÷ 打开; with no opened links the rate is undefined, not 0.
pub(crate) fn add_rate(followers: Option<i64>, links: Option<i64>) -> Option<f64> {
    match (followers, links) {
        (Some(followers), Some(links)) if links > 0 => Some(followers as f64 / links as f64),
        _ => None,
    }
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
    use super::{add_rate, execution};

    #[test]
    fn execution_status_follows_the_new_rules_in_order() {
        assert_eq!(
            execution(0, 0, Some(3), 0),
            "not_published",
            "no registration that day"
        );
        assert_eq!(
            execution(2, 1, Some(0), 1),
            "praise",
            "a hit outranks a missing view"
        );
        assert_eq!(execution(2, 0, Some(4), 0), "praise");
        assert_eq!(execution(3, 0, Some(0), 1), "pending_views");
        assert_eq!(
            execution(3, 0, Some(0), 0),
            "done",
            "0 followers is not 需复盘 any more"
        );
    }

    #[test]
    fn add_rate_is_unknown_without_opened_links() {
        assert_eq!(add_rate(Some(3), Some(0)), None);
        assert_eq!(add_rate(Some(0), Some(4)), Some(0.0));
        assert_eq!(add_rate(Some(1), Some(4)), Some(0.25));
        assert_eq!(add_rate(None, Some(4)), None);
    }
}
