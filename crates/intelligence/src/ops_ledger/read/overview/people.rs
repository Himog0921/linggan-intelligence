//! Block 4 (PAGE-OPS-002 §3.4): per-operator figures behind 运营人排行, also used by the drop
//! rules of 今日必须处理. Publications count by publisher name and follow the business selection;
//! daily reports count by operator name and cover all businesses.

use std::collections::BTreeMap;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::super::{LATEST_VIEWS_CTE, OpsBusiness, OpsReadError};
use super::attention::OverviewAttention;
use super::{OverviewWindow, PENDING_VIEWS_OVERDUE_DAYS, ratio_pct};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct OperatorFigures {
    pub name: String,
    pub current_published: i64,
    pub previous_published: i64,
    pub current_hits: i64,
    pub xhs: bool,
    pub douyin: bool,
    /// Xiaohongshu posts still without views 7 days after publishing: the whole backlog, not
    /// only the window.
    pub pending_views: i64,
    pub oldest_pending: Option<String>,
    pub current_daily_rows: i64,
    pub current_links: Option<i64>,
    pub current_followers: Option<i64>,
    pub previous_links: Option<i64>,
    pub previous_followers: Option<i64>,
    /// 加微 for the seven days ending today; `None` for a day without a daily report.
    pub sparkline: Vec<Option<i64>>,
    pub responsible_accounts: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewRankingRow {
    pub operator: String,
    /// `xhs`, `douyin` or `both`, from the window's registrations.
    pub platform: Option<&'static str>,
    pub sparkline: Vec<Option<i64>>,
    pub responsible_accounts: i64,
    pub published: i64,
    pub links_opened: Option<i64>,
    pub followers_gained: Option<i64>,
    pub conversion_rate: Option<f64>,
    pub hits: i64,
    pub pending_views: i64,
    /// The most severe item this operator has in 今日必须处理: severity, kind and, for a drop, the
    /// metric that fell.
    pub risk: Option<&'static str>,
    pub risk_kind: Option<&'static str>,
    pub risk_metric: Option<&'static str>,
}

pub(super) async fn operators(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<Vec<OperatorFigures>, OpsReadError> {
    let mut by_name: BTreeMap<String, OperatorFigures> = BTreeMap::new();
    let entry = |map: &mut BTreeMap<String, OperatorFigures>, name: String| {
        map.entry(name.clone()).or_insert_with(|| OperatorFigures {
            name,
            sparkline: vec![None; 7],
            ..OperatorFigures::default()
        });
    };
    for row in publication_rows(database, business, window).await? {
        let name: String = row.try_get("operator")?;
        entry(&mut by_name, name.clone());
        let figures = by_name.get_mut(&name).expect("inserted above");
        figures.current_published = row.try_get("cur_published")?;
        figures.previous_published = row.try_get("prev_published")?;
        figures.current_hits = row.try_get("cur_hits")?;
        figures.xhs = row.try_get::<Option<bool>, _>("cur_xhs")?.unwrap_or(false);
        figures.douyin = row
            .try_get::<Option<bool>, _>("cur_douyin")?
            .unwrap_or(false);
        figures.pending_views = row.try_get("pending_views")?;
        figures.oldest_pending = row.try_get("oldest_pending")?;
    }
    for row in daily_rows(database, window).await? {
        let name: String = row.try_get("operator")?;
        entry(&mut by_name, name.clone());
        let figures = by_name.get_mut(&name).expect("inserted above");
        figures.current_daily_rows = row.try_get("cur_rows")?;
        figures.current_links = row.try_get("cur_links")?;
        figures.current_followers = row.try_get("cur_followers")?;
        figures.previous_links = row.try_get("prev_links")?;
        figures.previous_followers = row.try_get("prev_followers")?;
    }
    for row in sparkline_rows(database, window).await? {
        let name: String = row.try_get("operator")?;
        let day_index: i32 = row.try_get("day_index")?;
        if let (Some(figures), Ok(index)) = (by_name.get_mut(&name), usize::try_from(day_index))
            && let Some(slot) = figures.sparkline.get_mut(index)
        {
            *slot = Some(i64::from(row.try_get::<i32, _>("followers_gained")?));
        }
    }
    let accounts = sqlx::query(
        "SELECT operator_name AS operator, count(*) AS accounts FROM linggan_ops_account \
          WHERE ledger_state = 'in_ledger' AND operator_name IS NOT NULL \
            AND ($1::uuid IS NULL OR domain_ref = $1) \
          GROUP BY 1",
    )
    .bind(business.domain_ref())
    .fetch_all(database.pool())
    .await?;
    for row in accounts {
        let name: String = row.try_get("operator")?;
        if let Some(figures) = by_name.get_mut(&name) {
            figures.responsible_accounts = row.try_get("accounts")?;
        }
    }
    Ok(by_name.into_values().collect())
}

/// Counted current and previous figures by publisher, plus the overdue-view backlog, which does
/// not depend on the window. With nothing migrated in the window the ranges are NULL and only the
/// backlog counts.
async fn publication_rows(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let counted = window.counted();
    let (current, previous) = (counted.map(|(c, _)| c), counted.map(|(_, p)| p));
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE} \
         SELECT p.publisher_name AS operator, \
                count(*) FILTER (WHERE p.published_business_date BETWEEN $3::date AND $4::date) AS cur_published, \
                count(*) FILTER (WHERE p.published_business_date BETWEEN $5::date AND $6::date) AS prev_published, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2 \
                                   AND p.published_business_date BETWEEN $3::date AND $4::date) AS cur_hits, \
                bool_or(p.platform = 'xhs') FILTER (WHERE p.published_business_date BETWEEN $3::date AND $4::date) AS cur_xhs, \
                bool_or(p.platform = 'douyin') FILTER (WHERE p.published_business_date BETWEEN $3::date AND $4::date) AS cur_douyin, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count IS NULL \
                                   AND $7::date - p.published_business_date >= $8) AS pending_views, \
                min(p.published_business_date) FILTER (WHERE p.platform = 'xhs' AND lv.view_count IS NULL \
                                   AND $7::date - p.published_business_date >= $8)::text AS oldest_pending \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
           LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND p.publisher_name IS NOT NULL AND ($1::uuid IS NULL OR a.domain_ref = $1) \
          GROUP BY 1"
    )))
    .bind(business.domain_ref())
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(current.map(|range| range.from.as_str()))
    .bind(current.map(|range| range.to.as_str()))
    .bind(previous.map(|range| range.from.as_str()))
    .bind(previous.map(|range| range.to.as_str()))
    .bind(&window.today)
    .bind(PENDING_VIEWS_OVERDUE_DAYS)
    .fetch_all(database.pool())
    .await
}

async fn daily_rows(
    database: &Database,
    window: &OverviewWindow,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    let Some((current, previous)) = window.counted() else {
        return Ok(Vec::new());
    };
    sqlx::query(
        "SELECT operator_name AS operator, \
                count(*) FILTER (WHERE business_date BETWEEN $1::date AND $2::date) AS cur_rows, \
                sum(links_opened_count) FILTER (WHERE business_date BETWEEN $1::date AND $2::date)::bigint AS cur_links, \
                sum(followers_gained) FILTER (WHERE business_date BETWEEN $1::date AND $2::date)::bigint AS cur_followers, \
                sum(links_opened_count) FILTER (WHERE business_date BETWEEN $3::date AND $4::date)::bigint AS prev_links, \
                sum(followers_gained) FILTER (WHERE business_date BETWEEN $3::date AND $4::date)::bigint AS prev_followers \
           FROM linggan_ops_daily_report \
          WHERE missing_since_batch_ref IS NULL \
            AND business_date BETWEEN least($1::date, $3::date) AND greatest($2::date, $4::date) \
          GROUP BY 1",
    )
    .bind(&current.from)
    .bind(&current.to)
    .bind(&previous.from)
    .bind(&previous.to)
    .fetch_all(database.pool())
    .await
}

async fn sparkline_rows(
    database: &Database,
    window: &OverviewWindow,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error> {
    sqlx::query(
        "SELECT operator_name AS operator, (business_date - ($1::date - 6))::int AS day_index, followers_gained \
           FROM linggan_ops_daily_report \
          WHERE missing_since_batch_ref IS NULL \
            AND business_date BETWEEN $1::date - 6 AND $1::date AND business_date <= $2::date",
    )
    .bind(&window.today)
    .bind(window.complete_through.as_deref())
    .fetch_all(database.pool())
    .await
}

/// Operators active in the window, most new followers first; unknown counts sort last.
pub(super) fn ranking(
    operators: &[OperatorFigures],
    attention: &OverviewAttention,
) -> Vec<OverviewRankingRow> {
    let mut rows: Vec<OverviewRankingRow> = operators
        .iter()
        .filter(|figures| figures.current_published > 0 || figures.current_daily_rows > 0)
        .map(|figures| {
            let risk = attention.risk_of(&figures.name);
            OverviewRankingRow {
                operator: figures.name.clone(),
                platform: match (figures.xhs, figures.douyin) {
                    (true, true) => Some("both"),
                    (true, false) => Some("xhs"),
                    (false, true) => Some("douyin"),
                    (false, false) => None,
                },
                sparkline: figures.sparkline.clone(),
                responsible_accounts: figures.responsible_accounts,
                published: figures.current_published,
                links_opened: figures.current_links,
                followers_gained: figures.current_followers,
                conversion_rate: ratio_pct(figures.current_followers, figures.current_links),
                hits: figures.current_hits,
                pending_views: figures.pending_views,
                risk: risk.map(|item| item.severity),
                risk_kind: risk.map(|item| item.kind),
                risk_metric: risk.and_then(|item| item.metric),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.followers_gained
            .unwrap_or(-1)
            .cmp(&a.followers_gained.unwrap_or(-1))
            .then(b.published.cmp(&a.published))
            .then_with(|| a.operator.cmp(&b.operator))
    });
    rows
}
