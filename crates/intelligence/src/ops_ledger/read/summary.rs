//! 发布记录页读数条 (OPS-HTTP-001 §3). Months are Beijing calendar months.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::{
    LATEST_VIEWS_CTE, OpsAsOf, OpsBusiness, OpsReadError, ensure_business, read_ops_as_of,
};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsNotConnected {
    pub state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsUnscoped {
    pub value: Option<i64>,
    pub scope: &'static str,
}

/// Every count is `None` until the first import: an empty mirror proves nothing about the month.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsSummary {
    pub as_of: Option<OpsAsOf>,
    pub month: String,
    pub active_accounts: Option<i64>,
    pub ledger_accounts: Option<i64>,
    pub month_published: Option<i64>,
    pub effective_publication: OpsNotConnected,
    pub month_followers_gained: OpsUnscoped,
    pub month_hits: Option<i64>,
}

pub async fn read_ops_summary(
    database: &Database,
    business: OpsBusiness,
) -> Result<OpsSummary, OpsReadError> {
    ensure_business(database, business).await?;
    let as_of = read_ops_as_of(database).await?;
    let month: String =
        sqlx::query_scalar("SELECT to_char(now() AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM')")
            .fetch_one(database.pool())
            .await?;
    let not_connected = OpsNotConnected {
        state: "not_connected",
    };
    if as_of.is_none() {
        return Ok(OpsSummary {
            as_of,
            month,
            active_accounts: None,
            ledger_accounts: None,
            month_published: None,
            effective_publication: not_connected,
            month_followers_gained: OpsUnscoped {
                value: None,
                scope: "all_businesses",
            },
            month_hits: None,
        });
    }
    let accounts = sqlx::query(
        "SELECT count(*) FILTER (WHERE status IN ('active', 'warming')) AS active, count(*) AS total \
           FROM linggan_ops_account \
          WHERE ledger_state = 'in_ledger' AND ($1::uuid IS NULL OR domain_ref = $1)",
    )
    .bind(business.domain_ref())
    .fetch_one(database.pool())
    .await?;
    let publications = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE} \
         SELECT count(*) AS published, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2) AS hits \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
           LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND ($1::uuid IS NULL OR a.domain_ref = $1) \
            AND to_char(p.published_business_date, 'YYYY-MM') = $3"
    )))
    .bind(business.domain_ref())
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(&month)
    .fetch_one(database.pool())
    .await?;
    // Daily reports are kept per operator, not per business; with no row this month the sum is
    // unknown rather than zero.
    let followers: Option<i64> = sqlx::query_scalar(
        "SELECT sum(followers_gained)::bigint FROM linggan_ops_daily_report \
          WHERE missing_since_batch_ref IS NULL AND to_char(business_date, 'YYYY-MM') = $1",
    )
    .bind(&month)
    .fetch_one(database.pool())
    .await?;
    Ok(OpsSummary {
        as_of,
        month,
        active_accounts: Some(accounts.try_get("active")?),
        ledger_accounts: Some(accounts.try_get("total")?),
        month_published: Some(publications.try_get("published")?),
        effective_publication: not_connected,
        month_followers_gained: OpsUnscoped {
            value: followers,
            scope: "all_businesses",
        },
        month_hits: Some(publications.try_get("hits")?),
    })
}
