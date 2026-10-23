//! 总览 (PAGE-OPS-002, OPS-HTTP-001 §4): the five blocks for one time slice. Read-only.
//! Publications, hits and pending views follow the business selection; opened links and new
//! followers come from daily reports, which are kept per operator and not split by business.
//! Every comparison is against a previous window of the same number of days.

mod attention;
mod figures;
mod output;
mod people;
mod trend;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::{
    LATEST_VIEWS_CTE, OpsAsOf, OpsBusiness, OpsReadError, ensure_business, read_ops_as_of,
};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;
use crate::ops_ledger::is_business_date;

pub use attention::OverviewAttention;
pub use figures::{OverviewFunnel, OverviewInsight, OverviewKpi, OverviewTarget};
pub use output::{OverviewDailyOutput, OverviewMatrix, OverviewPlatformShare};
pub use people::OverviewRankingRow;
pub use trend::{OpsOverviewTrend, read_ops_overview_trend};

/// Longest custom slice, in days.
pub const OPS_CUSTOM_SLICE_MAX_DAYS: i64 = 366;
/// A view count still missing this many days after publishing is overdue (PAGE-OPS-001 §3.3).
pub(crate) const PENDING_VIEWS_OVERDUE_DAYS: i32 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewSlice {
    Yesterday,
    Week,
    Month,
    Custom { from: String, to: String },
}

impl OverviewSlice {
    pub fn parse(
        slice: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
    ) -> Result<Self, OpsReadError> {
        match slice.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("week") => Ok(Self::Week),
            Some("yesterday") => Ok(Self::Yesterday),
            Some("month") => Ok(Self::Month),
            Some("custom") => {
                let (Some(from), Some(to)) = (from.map(str::trim), to.map(str::trim)) else {
                    return Err(OpsReadError::InvalidParameter("from"));
                };
                if !is_business_date(from) || !is_business_date(to) || from > to {
                    return Err(OpsReadError::InvalidParameter("from"));
                }
                Ok(Self::Custom {
                    from: from.to_owned(),
                    to: to.to_owned(),
                })
            }
            Some(_) => Err(OpsReadError::InvalidParameter("slice")),
        }
    }

    fn key(&self) -> &'static str {
        match self {
            Self::Yesterday => "yesterday",
            Self::Week => "week",
            Self::Month => "month",
            Self::Custom { .. } => "custom",
        }
    }
}

pub struct OpsOverviewQuery {
    pub slice: OverviewSlice,
    /// The Beijing business day the overview is read on. Proofs pin it; the pages leave it `None`
    /// so the database clock decides.
    pub today: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub from: String,
    pub to: String,
    pub days: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewWindow {
    pub slice: &'static str,
    pub today: String,
    /// Today's target periods, for the target dialog: `2026-W42` and `2026-10`.
    pub this_week: String,
    pub this_month: String,
    /// The last business day the mirror holds complete: the day before the latest export. The
    /// export day itself is usually not filled in yet. `None` before any import.
    pub complete_through: Option<String>,
    /// The calendar window the slice names.
    pub current: DateRange,
    /// The part of `current` up to `complete_through`; every current figure counts only these
    /// days. `None` when none of the window has been migrated yet.
    pub covered: Option<DateRange>,
    /// As many days as `covered`, before it; `None` when nothing is covered.
    pub previous: Option<DateRange>,
}

/// Facts entered natively in this calendar slice.  These are intentionally separate from
/// `window.covered`: a few operator entries prove those facts, not that every day in the slice
/// has been historically mirrored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewNativeFacts {
    pub observed: bool,
    pub latest_business_date: Option<String>,
    pub published: i64,
    pub links_opened: Option<i64>,
    pub followers_gained: Option<i64>,
    pub hits: i64,
}

impl OverviewWindow {
    /// The counted current days and the previous window, both present or both absent.
    pub(super) fn counted(&self) -> Option<(&DateRange, &DateRange)> {
        Some((self.covered.as_ref()?, self.previous.as_ref()?))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsOverview {
    pub as_of: Option<OpsAsOf>,
    /// True when one business is selected: daily-report figures then still cover all businesses.
    pub business_selected: bool,
    pub window: OverviewWindow,
    pub native_facts: OverviewNativeFacts,
    pub kpi: OverviewKpi,
    pub funnel: OverviewFunnel,
    pub target: OverviewTarget,
    pub insights: Vec<OverviewInsight>,
    pub attention: OverviewAttention,
    pub ranking: Vec<OverviewRankingRow>,
    pub ranking_covered: bool,
    pub daily_output: OverviewDailyOutput,
    pub platform_share: Option<OverviewPlatformShare>,
    pub matrix: OverviewMatrix,
}

pub async fn read_ops_overview(
    database: &Database,
    business: OpsBusiness,
    query: &OpsOverviewQuery,
) -> Result<OpsOverview, OpsReadError> {
    ensure_business(database, business).await?;
    let window = resolve_window(database, query).await?;
    let native_facts = native_facts(database, business, &window).await?;
    let kpi = figures::kpi(database, business, &window).await?;
    let funnel = figures::funnel(&kpi, business);
    let target = figures::target(database, &window).await?;
    let operators = people::operators(database, business, &window).await?;
    let attention = attention::attention(database, business, &window, &operators).await?;
    let insights = figures::insights(&kpi, &target, &attention);
    Ok(OpsOverview {
        as_of: read_ops_as_of(database).await?,
        business_selected: business.domain_ref().is_some(),
        native_facts,
        ranking: people::ranking(&operators, &attention),
        ranking_covered: window.covered.is_some(),
        daily_output: output::daily_output(database, business, &window).await?,
        platform_share: output::platform_share(database, business, &window).await?,
        matrix: output::matrix(database, business, &window).await?,
        window,
        kpi,
        funnel,
        target,
        insights,
        attention,
    })
}

async fn native_facts(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<OverviewNativeFacts, OpsReadError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE}, \
         native_publications AS ( \
           SELECT p.published_business_date AS business_date,p.platform,lv.view_count \
             FROM linggan_ops_publication p \
             LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref \
             LEFT JOIN latest_views lv ON lv.publication_ref=p.publication_ref \
            WHERE p.source_system='linggan_native' AND p.merged_into_ref IS NULL \
              AND p.missing_since_batch_ref IS NULL \
              AND p.published_business_date BETWEEN $3::date AND $4::date \
              AND ($2::uuid IS NULL OR a.domain_ref=$2)), \
         native_daily AS ( \
           SELECT business_date,links_opened_count,followers_gained \
             FROM linggan_ops_daily_report \
            WHERE source_system='linggan_native' AND missing_since_batch_ref IS NULL \
              AND business_date BETWEEN $3::date AND $4::date), \
         native_dates AS (SELECT business_date FROM native_publications UNION ALL SELECT business_date FROM native_daily) \
         SELECT EXISTS(SELECT 1 FROM native_dates) AS observed, \
                (SELECT max(business_date)::text FROM native_dates) AS latest_business_date, \
                (SELECT count(*) FROM native_publications) AS published, \
                (SELECT sum(links_opened_count)::bigint FROM native_daily) AS links_opened, \
                (SELECT sum(followers_gained)::bigint FROM native_daily) AS followers_gained, \
                (SELECT count(*) FROM native_publications WHERE platform='xhs' AND view_count >= $1) AS hits"
    )))
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(business.domain_ref())
    .bind(&window.current.from)
    .bind(&window.current.to)
    .fetch_one(database.pool())
    .await?;
    Ok(OverviewNativeFacts {
        observed: row.try_get("observed")?,
        latest_business_date: row.try_get("latest_business_date")?,
        published: row.try_get("published")?,
        links_opened: row.try_get("links_opened")?,
        followers_gained: row.try_get("followers_gained")?,
        hits: row.try_get("hits")?,
    })
}

/// Both windows on the Beijing calendar. Only days the mirror holds complete are counted; the
/// previous window always has the counted length: a week so far is compared with the same
/// weekdays of last week, a month so far with the same days of last month (cut at its end).
async fn resolve_window(
    database: &Database,
    query: &OpsOverviewQuery,
) -> Result<OverviewWindow, OpsReadError> {
    let (from, to) = match &query.slice {
        OverviewSlice::Custom { from, to } => (Some(from.as_str()), Some(to.as_str())),
        _ => (None, None),
    };
    let row = sqlx::query(
        "WITH t AS (SELECT coalesce($1::date, (now() AT TIME ZONE 'Asia/Shanghai')::date) AS today, \
                           (SELECT (exported_at AT TIME ZONE 'Asia/Shanghai')::date - 1 \
                              FROM linggan_ops_import_batch \
                             ORDER BY applied_at DESC, batch_ref DESC LIMIT 1) AS complete_through), \
         w AS (SELECT today, complete_through, \
                CASE $2 WHEN 'yesterday' THEN today - 1 \
                        WHEN 'week' THEN date_trunc('week', today::timestamp)::date \
                        WHEN 'month' THEN date_trunc('month', today::timestamp)::date \
                        ELSE $3::date END AS cur_from, \
                CASE $2 WHEN 'yesterday' THEN today - 1 WHEN 'custom' THEN $4::date ELSE today END AS cur_to \
                 FROM t), \
         c AS (SELECT *, cur_to - cur_from + 1 AS days, \
                      CASE WHEN complete_through IS NOT NULL THEN least(cur_to, complete_through) END AS covered_to, \
                      CASE WHEN complete_through IS NOT NULL THEN least(cur_to, complete_through) - cur_from + 1 END AS covered_days FROM w), \
         d AS (SELECT *, CASE $2 WHEN 'week' THEN cur_from - 7 \
                                 WHEN 'month' THEN (cur_from - interval '1 month')::date \
                                 ELSE cur_from - covered_days END AS prev_from FROM c) \
         SELECT today::text AS today, to_char(today::timestamp, 'IYYY-\"W\"IW') AS this_week, \
                to_char(today::timestamp, 'YYYY-MM') AS this_month, complete_through::text AS complete_through, \
                cur_from::text AS cur_from, cur_to::text AS cur_to, days, \
                covered_to::text AS covered_to, coalesce(covered_days, 0) AS covered_days, \
                prev_from::text AS prev_from, \
                least(prev_from + covered_days - 1, cur_from - 1)::text AS prev_to, \
                least(prev_from + covered_days - 1, cur_from - 1) - prev_from + 1 AS prev_days \
           FROM d",
    )
    .bind(query.today.as_deref())
    .bind(query.slice.key())
    .bind(from)
    .bind(to)
    .fetch_one(database.pool())
    .await?;
    let days: i32 = row.try_get("days")?;
    if !(1..=OPS_CUSTOM_SLICE_MAX_DAYS).contains(&i64::from(days)) {
        return Err(OpsReadError::InvalidParameter("to"));
    }
    let current = DateRange {
        from: row.try_get("cur_from")?,
        to: row.try_get("cur_to")?,
        days: i64::from(days),
    };
    let covered_days: i32 = row.try_get("covered_days")?;
    let (covered, previous) = if covered_days >= 1 {
        let previous_days: i32 = row.try_get("prev_days")?;
        (
            Some(DateRange {
                from: current.from.clone(),
                to: row.try_get("covered_to")?,
                days: i64::from(covered_days),
            }),
            Some(DateRange {
                from: row.try_get("prev_from")?,
                to: row.try_get("prev_to")?,
                days: i64::from(previous_days),
            }),
        )
    } else {
        (None, None)
    };
    Ok(OverviewWindow {
        slice: query.slice.key(),
        today: row.try_get("today")?,
        this_week: row.try_get("this_week")?,
        this_month: row.try_get("this_month")?,
        complete_through: row.try_get("complete_through")?,
        current,
        covered,
        previous,
    })
}

/// Percentage change, rounded to one decimal; unknown when either side is unknown or the
/// previous value is 0.
pub(crate) fn change_pct(current: Option<i64>, previous: Option<i64>) -> Option<f64> {
    let (current, previous) = (current?, previous?);
    (previous != 0).then(|| ((current - previous) as f64 / previous as f64 * 1000.0).round() / 10.0)
}

/// A ratio as a percentage with two decimals; unknown when the denominator is unknown or 0.
pub(crate) fn ratio_pct(numerator: Option<i64>, denominator: Option<i64>) -> Option<f64> {
    let (numerator, denominator) = (numerator?, denominator?);
    (denominator != 0).then(|| (numerator as f64 / denominator as f64 * 10000.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::{OverviewSlice, change_pct, ratio_pct};

    #[test]
    fn slices_and_custom_dates_are_checked_before_any_query() {
        assert_eq!(
            OverviewSlice::parse(None, None, None).unwrap(),
            OverviewSlice::Week
        );
        assert_eq!(
            OverviewSlice::parse(Some("month"), None, None).unwrap(),
            OverviewSlice::Month
        );
        assert!(OverviewSlice::parse(Some("custom"), Some("2026-10-01"), None).is_err());
        assert!(
            OverviewSlice::parse(Some("custom"), Some("2026-10-05"), Some("2026-10-01")).is_err()
        );
        assert!(
            OverviewSlice::parse(Some("custom"), Some("2026-02-30"), Some("2026-03-01")).is_err()
        );
        assert!(OverviewSlice::parse(Some("quarter"), None, None).is_err());
    }

    #[test]
    fn changes_and_ratios_stay_unknown_instead_of_dividing_by_zero() {
        assert_eq!(change_pct(Some(7), Some(10)), Some(-30.0));
        assert_eq!(change_pct(Some(1), Some(0)), None);
        assert_eq!(change_pct(None, Some(3)), None);
        assert_eq!(ratio_pct(Some(1), Some(3)), Some(33.33));
        assert_eq!(ratio_pct(Some(0), Some(4)), Some(0.0));
        assert_eq!(ratio_pct(Some(2), Some(0)), None);
    }
}
