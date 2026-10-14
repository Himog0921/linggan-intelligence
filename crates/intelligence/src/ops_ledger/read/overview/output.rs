//! 每日产出, 平台占比 and 运营人 × 账号矩阵 (PAGE-OPS-002 §3.3, §3.5). Publications are counted by
//! the platform and account of each registration; new followers are recorded per operator, so
//! they are never split by platform or account.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use super::super::{LATEST_VIEWS_CTE, OpsBusiness, OpsReadError};
use super::OverviewWindow;
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPublished {
    pub date: String,
    pub operator: String,
    pub xhs: i64,
    pub douyin: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyFollowers {
    pub date: String,
    pub operator: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewDailyOutput {
    /// Every day of the window, oldest first.
    pub days: Vec<String>,
    /// Days after this one are not migrated yet; `None` when none of the window is.
    pub covered_to: Option<String>,
    pub operators: Vec<String>,
    pub published: Vec<DailyPublished>,
    pub followers: Vec<DailyFollowers>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewPlatformShare {
    pub xhs: i64,
    pub douyin: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixAccount {
    pub r#ref: Uuid,
    pub name: String,
    pub platform: String,
    /// Deleted in the workbench (`absent_from_export`).
    pub retired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixCell {
    pub operator: String,
    pub account: Uuid,
    pub published: i64,
    pub hits: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewMatrix {
    pub covered: bool,
    pub operators: Vec<String>,
    pub accounts: Vec<MatrixAccount>,
    pub cells: Vec<MatrixCell>,
}

pub(super) async fn daily_output(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<OverviewDailyOutput, OpsReadError> {
    let days: Vec<String> = sqlx::query_scalar(
        "SELECT g::date::text FROM generate_series($1::date, $2::date, interval '1 day') g ORDER BY 1",
    )
    .bind(&window.current.from)
    .bind(&window.current.to)
    .fetch_all(database.pool())
    .await?;
    let covered_to = window.covered.as_ref().map(|range| range.to.clone());
    let Some(covered) = &window.covered else {
        return Ok(OverviewDailyOutput {
            days,
            covered_to,
            operators: Vec::new(),
            published: Vec::new(),
            followers: Vec::new(),
        });
    };
    let published = sqlx::query(
        "SELECT p.published_business_date::text AS day, p.publisher_name AS operator, \
                count(*) FILTER (WHERE p.platform = 'xhs') AS xhs, \
                count(*) FILTER (WHERE p.platform = 'douyin') AS douyin \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND p.publisher_name IS NOT NULL \
            AND p.published_business_date BETWEEN $2::date AND $3::date \
            AND ($1::uuid IS NULL OR a.domain_ref = $1) \
          GROUP BY 1, 2 ORDER BY 1, 2",
    )
    .bind(business.domain_ref())
    .bind(&covered.from)
    .bind(&covered.to)
    .fetch_all(database.pool())
    .await?
    .iter()
    .map(|row| {
        Ok(DailyPublished {
            date: row.try_get("day")?,
            operator: row.try_get("operator")?,
            xhs: row.try_get("xhs")?,
            douyin: row.try_get("douyin")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let followers = sqlx::query(
        "SELECT business_date::text AS day, operator_name AS operator, followers_gained \
           FROM linggan_ops_daily_report \
          WHERE missing_since_batch_ref IS NULL AND business_date BETWEEN $1::date AND $2::date \
          ORDER BY 1, 2",
    )
    .bind(&covered.from)
    .bind(&covered.to)
    .fetch_all(database.pool())
    .await?
    .iter()
    .map(|row| {
        Ok(DailyFollowers {
            date: row.try_get("day")?,
            operator: row.try_get("operator")?,
            value: i64::from(row.try_get::<i32, _>("followers_gained")?),
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let mut operators: Vec<String> = published
        .iter()
        .map(|row| row.operator.clone())
        .chain(followers.iter().map(|row| row.operator.clone()))
        .collect();
    operators.sort();
    operators.dedup();
    Ok(OverviewDailyOutput {
        days,
        covered_to,
        operators,
        published,
        followers,
    })
}

pub(super) async fn platform_share(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<Option<OverviewPlatformShare>, OpsReadError> {
    let Some(covered) = &window.covered else {
        return Ok(None);
    };
    let row = sqlx::query(
        "SELECT count(*) FILTER (WHERE p.platform = 'xhs') AS xhs, \
                count(*) FILTER (WHERE p.platform = 'douyin') AS douyin \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND p.published_business_date BETWEEN $2::date AND $3::date \
            AND ($1::uuid IS NULL OR a.domain_ref = $1)",
    )
    .bind(business.domain_ref())
    .bind(&covered.from)
    .bind(&covered.to)
    .fetch_one(database.pool())
    .await?;
    Ok(Some(OverviewPlatformShare {
        xhs: row.try_get("xhs")?,
        douyin: row.try_get("douyin")?,
    }))
}

/// Rows are publishers, columns the accounts that published in the window.
pub(super) async fn matrix(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<OverviewMatrix, OpsReadError> {
    let mut matrix = OverviewMatrix {
        covered: window.covered.is_some(),
        operators: Vec::new(),
        accounts: Vec::new(),
        cells: Vec::new(),
    };
    let Some(covered) = &window.covered else {
        return Ok(matrix);
    };
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE} \
         SELECT p.publisher_name AS operator, a.account_ref, a.account_name, a.platform, \
                a.ledger_state <> 'in_ledger' AS retired, count(*) AS published, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2) AS hits \
           FROM linggan_ops_publication p \
           JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
           LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND p.publisher_name IS NOT NULL \
            AND p.published_business_date BETWEEN $3::date AND $4::date \
            AND ($1::uuid IS NULL OR a.domain_ref = $1) \
          GROUP BY 1, 2, 3, 4, 5 \
          ORDER BY a.platform DESC, a.account_name, a.account_ref, 1"
    )))
    .bind(business.domain_ref())
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(&covered.from)
    .bind(&covered.to)
    .fetch_all(database.pool())
    .await?;
    for row in &rows {
        let operator: String = row.try_get("operator")?;
        let account: Uuid = row.try_get("account_ref")?;
        if !matrix.accounts.iter().any(|known| known.r#ref == account) {
            matrix.accounts.push(MatrixAccount {
                r#ref: account,
                name: row.try_get("account_name")?,
                platform: row.try_get("platform")?,
                retired: row.try_get("retired")?,
            });
        }
        if !matrix.operators.contains(&operator) {
            matrix.operators.push(operator.clone());
        }
        matrix.cells.push(MatrixCell {
            operator,
            account,
            published: row.try_get("published")?,
            hits: row.try_get("hits")?,
        });
    }
    matrix.operators.sort();
    Ok(matrix)
}
