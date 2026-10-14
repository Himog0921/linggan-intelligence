//! Blocks 1 and 2 (PAGE-OPS-002 §3.1–3.2): 加微 with the target, the funnel, 转化率 and 爆款数,
//! and the automatic conclusions drawn only from those figures.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::super::{LATEST_VIEWS_CTE, OpsBusiness, OpsReadError};
use super::attention::OverviewAttention;
use super::{DateRange, OverviewWindow, change_pct, ratio_pct};
use crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD;
use crate::ops_ledger::targets::{OpsTargetRevision, TargetPeriod, current_target};

/// A pace is behind when time has run this many percentage points ahead of completion.
const BEHIND_GAP_POINTS: f64 = 20.0;
/// A funnel stage or headline figure dropping this far is named as the main drag (−20%).
const DRAG_PCT: f64 = -20.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiFigures {
    /// `None` when none of the window has been migrated yet.
    pub published: Option<i64>,
    /// `None` when no daily report falls in the window: unknown, not 0.
    pub links_opened: Option<i64>,
    pub followers_gained: Option<i64>,
    /// 加微 ÷ 链接打开, as a percentage.
    pub conversion_rate: Option<f64>,
    pub hits: Option<i64>,
}

const UNKNOWN: KpiFigures = KpiFigures {
    published: None,
    links_opened: None,
    followers_gained: None,
    conversion_rate: None,
    hits: None,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiDelta {
    pub published_pct: Option<f64>,
    pub links_opened_pct: Option<f64>,
    pub followers_gained_pct: Option<f64>,
    pub hits_pct: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotConnected {
    pub state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewKpi {
    pub current: KpiFigures,
    pub previous: KpiFigures,
    pub delta: KpiDelta,
    /// System-confirmed publications arrive with D5.
    pub effective_publication: NotConnected,
    /// Opened links and new followers come from daily reports, which cover all businesses.
    pub daily_scope: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunnelRates {
    /// 链接打开 ÷ 发布; not computed for one business, whose opened links are not recorded.
    pub publish_to_open: Option<f64>,
    pub open_to_follow: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewFunnel {
    pub current: FunnelRates,
    pub previous: FunnelRates,
    pub publish_to_open_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewTarget {
    /// `set`, `not_set`, or `not_applicable` for a custom window.
    pub state: &'static str,
    pub period_type: Option<&'static str>,
    pub period_key: Option<String>,
    pub period: Option<DateRange>,
    pub elapsed_days: Option<i64>,
    pub revision: Option<OpsTargetRevision>,
    /// 加微 in the target period up to today; `None` before any daily report for the period.
    pub period_followers: Option<i64>,
    pub achieved_pct: Option<f64>,
    pub time_progress_pct: Option<f64>,
    pub daily_needed: Option<i64>,
    pub projected_end: Option<i64>,
    /// `achieved`, `on_track` or `behind`; only when a target is set and progress is known.
    pub pace: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewInsight {
    /// `pace`, `drag` or `review`; the page writes the sentence.
    pub kind: &'static str,
    pub pace: Option<&'static str>,
    pub achieved_pct: Option<f64>,
    pub projected_pct: Option<f64>,
    pub metric: Option<&'static str>,
    pub delta_pct: Option<f64>,
    pub names: Vec<String>,
}

pub(super) async fn kpi(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<OverviewKpi, OpsReadError> {
    let (current, previous) = match window.counted() {
        Some((current, previous)) => counted_kpi(database, business, current, previous).await?,
        None => (UNKNOWN, UNKNOWN),
    };
    Ok(OverviewKpi {
        delta: KpiDelta {
            published_pct: change_pct(current.published, previous.published),
            links_opened_pct: change_pct(current.links_opened, previous.links_opened),
            followers_gained_pct: change_pct(current.followers_gained, previous.followers_gained),
            hits_pct: change_pct(current.hits, previous.hits),
        },
        current,
        previous,
        effective_publication: NotConnected {
            state: "not_connected",
        },
        daily_scope: "all_businesses",
    })
}

async fn counted_kpi(
    database: &Database,
    business: OpsBusiness,
    current: &DateRange,
    previous: &DateRange,
) -> Result<(KpiFigures, KpiFigures), OpsReadError> {
    let publications = sqlx::query(sqlx::AssertSqlSafe(format!(
        "WITH {LATEST_VIEWS_CTE} \
         SELECT count(*) FILTER (WHERE p.published_business_date BETWEEN $3::date AND $4::date) AS cur_published, \
                count(*) FILTER (WHERE p.published_business_date BETWEEN $5::date AND $6::date) AS prev_published, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2 \
                                   AND p.published_business_date BETWEEN $3::date AND $4::date) AS cur_hits, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND lv.view_count >= $2 \
                                   AND p.published_business_date BETWEEN $5::date AND $6::date) AS prev_hits \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
           LEFT JOIN latest_views lv ON lv.publication_ref = p.publication_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND ($1::uuid IS NULL OR a.domain_ref = $1)"
    )))
    .bind(business.domain_ref())
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .bind(&current.from)
    .bind(&current.to)
    .bind(&previous.from)
    .bind(&previous.to)
    .fetch_one(database.pool())
    .await?;
    let daily = sqlx::query(
        "SELECT sum(links_opened_count) FILTER (WHERE business_date BETWEEN $1::date AND $2::date)::bigint AS cur_links, \
                sum(followers_gained) FILTER (WHERE business_date BETWEEN $1::date AND $2::date)::bigint AS cur_followers, \
                sum(links_opened_count) FILTER (WHERE business_date BETWEEN $3::date AND $4::date)::bigint AS prev_links, \
                sum(followers_gained) FILTER (WHERE business_date BETWEEN $3::date AND $4::date)::bigint AS prev_followers \
           FROM linggan_ops_daily_report WHERE missing_since_batch_ref IS NULL",
    )
    .bind(&current.from)
    .bind(&current.to)
    .bind(&previous.from)
    .bind(&previous.to)
    .fetch_one(database.pool())
    .await?;
    let figures = |prefix: &str| -> Result<KpiFigures, sqlx::Error> {
        let links: Option<i64> = daily.try_get(format!("{prefix}_links").as_str())?;
        let followers: Option<i64> = daily.try_get(format!("{prefix}_followers").as_str())?;
        Ok(KpiFigures {
            published: Some(publications.try_get(format!("{prefix}_published").as_str())?),
            links_opened: links,
            followers_gained: followers,
            conversion_rate: ratio_pct(followers, links),
            hits: Some(publications.try_get(format!("{prefix}_hits").as_str())?),
        })
    };
    Ok((figures("cur")?, figures("prev")?))
}

pub(super) fn funnel(kpi: &OverviewKpi, business: OpsBusiness) -> OverviewFunnel {
    let available = business.domain_ref().is_none();
    let rates = |figures: &KpiFigures| FunnelRates {
        publish_to_open: available
            .then(|| ratio_pct(figures.links_opened, figures.published))
            .flatten(),
        open_to_follow: ratio_pct(figures.followers_gained, figures.links_opened),
    };
    OverviewFunnel {
        current: rates(&kpi.current),
        previous: rates(&kpi.previous),
        publish_to_open_available: available,
    }
}

/// Today's week (昨日、本周) or month (本月), matching the target dialog; custom has none.
pub(super) async fn target(
    database: &Database,
    window: &OverviewWindow,
) -> Result<OverviewTarget, OpsReadError> {
    let period_type = match window.slice {
        "month" => "month",
        "custom" => return Ok(not_applicable()),
        _ => "week",
    };
    let row = sqlx::query(
        "WITH p AS (SELECT CASE WHEN $2 = 'month' THEN to_char($1::date, 'YYYY-MM') \
                               ELSE to_char($1::date, 'IYYY-\"W\"IW') END AS period_key, \
                          (CASE WHEN $2 = 'month' THEN date_trunc('month', $1::date::timestamp) \
                                ELSE date_trunc('week', $1::date::timestamp) END)::date AS period_from, \
                          (CASE WHEN $2 = 'month' THEN date_trunc('month', $1::date::timestamp) + interval '1 month' \
                                ELSE date_trunc('week', $1::date::timestamp) + interval '1 week' END)::date - 1 AS period_to), \
              e AS (SELECT *, least($3::date, period_to, $4::date) AS upto FROM p) \
         SELECT period_key, period_from::text AS period_from, period_to::text AS period_to, \
                period_to - period_from + 1 AS total_days, upto - period_from + 1 AS elapsed_days, \
                (SELECT sum(d.followers_gained)::bigint FROM linggan_ops_daily_report d \
                  WHERE d.missing_since_batch_ref IS NULL \
                    AND d.business_date BETWEEN e.period_from AND e.upto) AS period_followers \
           FROM e",
    )
    .bind(&window.today)
    .bind(period_type)
    .bind(&window.today)
    .bind(window.complete_through.as_deref().unwrap_or("0001-01-01"))
    .fetch_one(database.pool())
    .await?;
    let period_key: String = row.try_get("period_key")?;
    let period = TargetPeriod::parse(period_type, &period_key)
        .map_err(|_| OpsReadError::InvalidParameter("period"))?;
    let revision = current_target(database.pool(), &period).await?;
    let total_days = i64::from(row.try_get::<i32, _>("total_days")?);
    // Days of the period the mirror holds complete; none yet means progress is unknown.
    let elapsed_days =
        Some(i64::from(row.try_get::<i32, _>("elapsed_days")?)).filter(|days| *days >= 1);
    let period_followers: Option<i64> = elapsed_days
        .map(|_| row.try_get("period_followers"))
        .transpose()?
        .flatten();
    let mut target = OverviewTarget {
        state: if revision.is_some() { "set" } else { "not_set" },
        period_type: Some(period_type),
        period_key: Some(period_key),
        period: Some(DateRange {
            from: row.try_get("period_from")?,
            to: row.try_get("period_to")?,
            days: total_days,
        }),
        elapsed_days,
        revision,
        period_followers,
        achieved_pct: None,
        time_progress_pct: None,
        daily_needed: None,
        projected_end: None,
        pace: None,
    };
    let goal = target
        .revision
        .as_ref()
        .map(|revision| i64::from(revision.target_value));
    if let (Some(goal), Some(done), Some(elapsed)) = (goal, period_followers, elapsed_days) {
        progress(&mut target, goal, done, elapsed, total_days);
    }
    Ok(target)
}

fn not_applicable() -> OverviewTarget {
    OverviewTarget {
        state: "not_applicable",
        period_type: None,
        period_key: None,
        period: None,
        elapsed_days: None,
        revision: None,
        period_followers: None,
        achieved_pct: None,
        time_progress_pct: None,
        daily_needed: None,
        projected_end: None,
        pace: None,
    }
}

/// Progress against a set target; nothing here is estimated when no target exists.
fn progress(target: &mut OverviewTarget, goal: i64, done: i64, elapsed: i64, total: i64) {
    let achieved = (done as f64 / goal as f64 * 1000.0).round() / 10.0;
    let time = (elapsed as f64 / total as f64 * 1000.0).round() / 10.0;
    let remaining_days = total - elapsed;
    target.achieved_pct = Some(achieved);
    target.time_progress_pct = Some(time);
    target.daily_needed = (remaining_days > 0).then(|| {
        let gap = (goal - done).max(0);
        (gap + remaining_days - 1) / remaining_days
    });
    target.projected_end =
        (elapsed > 0).then(|| (done as f64 / elapsed as f64 * total as f64).round() as i64);
    target.pace = Some(if achieved >= 100.0 {
        "achieved"
    } else if time - achieved > BEHIND_GAP_POINTS {
        "behind"
    } else {
        "on_track"
    });
}

/// At most three conclusions, each drawn from figures already on the page.
pub(super) fn insights(
    kpi: &OverviewKpi,
    target: &OverviewTarget,
    attention: &OverviewAttention,
) -> Vec<OverviewInsight> {
    let blank = OverviewInsight {
        kind: "",
        pace: None,
        achieved_pct: None,
        projected_pct: None,
        metric: None,
        delta_pct: None,
        names: Vec::new(),
    };
    let mut insights = Vec::new();
    if let (Some(pace), Some(revision)) = (target.pace, &target.revision) {
        insights.push(OverviewInsight {
            kind: "pace",
            pace: Some(pace),
            achieved_pct: target.achieved_pct,
            projected_pct: target.projected_end.map(|projected| {
                (projected as f64 / f64::from(revision.target_value) * 100.0).round()
            }),
            ..blank.clone()
        });
    }
    let drag = [
        ("published", kpi.delta.published_pct),
        ("links_opened", kpi.delta.links_opened_pct),
    ]
    .into_iter()
    .filter_map(|(metric, delta)| {
        delta
            .filter(|value| *value <= DRAG_PCT)
            .map(|value| (metric, value))
    })
    .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((metric, delta)) = drag {
        insights.push(OverviewInsight {
            kind: "drag",
            metric: Some(metric),
            delta_pct: Some(delta),
            ..blank.clone()
        });
    }
    let mut names: Vec<String> = Vec::new();
    for item in attention.items.iter().filter(|item| item.kind == "drop") {
        if let Some(name) = &item.operator
            && !names.contains(name)
            && names.len() < 2
        {
            names.push(name.clone());
        }
    }
    if !names.is_empty() {
        insights.push(OverviewInsight {
            kind: "review",
            names,
            ..blank
        });
    }
    insights
}

#[cfg(test)]
mod tests {
    use super::{OverviewTarget, not_applicable, progress};

    fn set(goal: i64, done: i64, elapsed: i64, total: i64) -> OverviewTarget {
        let mut target = not_applicable();
        progress(&mut target, goal, done, elapsed, total);
        target
    }

    #[test]
    fn progress_follows_the_set_target_only() {
        let behind = set(100, 20, 4, 7);
        assert_eq!(behind.achieved_pct, Some(20.0));
        assert_eq!(behind.time_progress_pct, Some(57.1));
        assert_eq!(behind.pace, Some("behind"));
        assert_eq!(
            behind.daily_needed,
            Some(27),
            "80 left over 3 days, rounded up"
        );
        assert_eq!(behind.projected_end, Some(35));
        assert_eq!(set(100, 50, 4, 7).pace, Some("on_track"));
        let done = set(100, 120, 7, 7);
        assert_eq!((done.pace, done.daily_needed), (Some("achieved"), None));
    }
}
