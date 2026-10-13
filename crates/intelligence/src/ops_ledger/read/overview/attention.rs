//! 今日必须处理 (PAGE-OPS-002 §3.3): drops of 30% or more against the previous window, overdue
//! Xiaohongshu views per operator, and registrations whose publisher is not the account's
//! operator. The page shows five; the rest are counted. Sync checks wait for D5.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::super::{OpsBusiness, OpsReadError};
use super::people::OperatorFigures;
use super::{OverviewWindow, change_pct};

/// A figure falling this far against the previous window needs a review (−30%).
const DROP_PCT: f64 = -30.0;
const SHOWN: usize = 5;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewAttentionItem {
    /// `drop`, `pending_views` or `owner_mismatch`.
    pub kind: &'static str,
    /// `high` or `medium`.
    pub severity: &'static str,
    pub operator: Option<String>,
    pub account: Option<String>,
    /// For drops: `published`, `links_opened` or `followers_gained`.
    pub metric: Option<&'static str>,
    pub current_value: Option<i64>,
    pub previous_value: Option<i64>,
    pub drop_pct: Option<f64>,
    pub count: Option<i64>,
    pub oldest_date: Option<String>,
    /// For owner mismatches: the account's operator.
    pub account_operator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewAttention {
    pub items: Vec<OverviewAttentionItem>,
    pub hidden_count: usize,
    pub total_count: usize,
    #[serde(skip)]
    all: Vec<OverviewAttentionItem>,
}

impl OverviewAttention {
    /// The first item an operator has anywhere in the list, shown or not: the list is sorted, so
    /// this is the most severe one.
    pub(super) fn risk_of(&self, operator: &str) -> Option<&OverviewAttentionItem> {
        self.all
            .iter()
            .find(|item| item.operator.as_deref() == Some(operator))
    }
}

fn item(kind: &'static str, severity: &'static str) -> OverviewAttentionItem {
    OverviewAttentionItem {
        kind,
        severity,
        operator: None,
        account: None,
        metric: None,
        current_value: None,
        previous_value: None,
        drop_pct: None,
        count: None,
        oldest_date: None,
        account_operator: None,
    }
}

pub(super) async fn attention(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
    operators: &[OperatorFigures],
) -> Result<OverviewAttention, OpsReadError> {
    let mut all = Vec::new();
    for figures in operators {
        all.extend(drops(figures));
        if figures.pending_views > 0 {
            all.push(OverviewAttentionItem {
                operator: Some(figures.name.clone()),
                count: Some(figures.pending_views),
                oldest_date: figures.oldest_pending.clone(),
                ..item("pending_views", "medium")
            });
        }
    }
    all.extend(owner_mismatches(database, business, window).await?);
    all.sort_by(|a, b| {
        let rank = |severity: &str| if severity == "high" { 0 } else { 1 };
        rank(a.severity)
            .cmp(&rank(b.severity))
            .then(
                a.drop_pct
                    .unwrap_or(0.0)
                    .total_cmp(&b.drop_pct.unwrap_or(0.0)),
            )
            .then(b.count.unwrap_or(0).cmp(&a.count.unwrap_or(0)))
            .then_with(|| a.operator.cmp(&b.operator))
            .then_with(|| a.account.cmp(&b.account))
    });
    Ok(OverviewAttention {
        items: all.iter().take(SHOWN).cloned().collect(),
        hidden_count: all.len().saturating_sub(SHOWN),
        total_count: all.len(),
        all,
    })
}

/// Registration counts are facts of the mirror, so a fall to 0 is a drop. Opened links and new
/// followers are compared only when the window has daily reports: a missing report is unknown,
/// not a fall to 0.
fn drops(figures: &OperatorFigures) -> Vec<OverviewAttentionItem> {
    let has_daily = figures.current_daily_rows > 0;
    [
        (
            "published",
            Some(figures.current_published),
            Some(figures.previous_published),
        ),
        (
            "links_opened",
            figures.current_links.filter(|_| has_daily),
            figures.previous_links,
        ),
        (
            "followers_gained",
            figures.current_followers.filter(|_| has_daily),
            figures.previous_followers,
        ),
    ]
    .into_iter()
    .filter_map(|(metric, current, previous)| {
        let pct = change_pct(current, previous)?;
        (pct <= DROP_PCT).then(|| OverviewAttentionItem {
            operator: Some(figures.name.clone()),
            metric: Some(metric),
            current_value: current,
            previous_value: previous,
            drop_pct: Some(pct),
            ..item("drop", "high")
        })
    })
    .collect()
}

async fn owner_mismatches(
    database: &Database,
    business: OpsBusiness,
    window: &OverviewWindow,
) -> Result<Vec<OverviewAttentionItem>, OpsReadError> {
    let Some((covered, _)) = window.counted() else {
        return Ok(Vec::new());
    };
    let rows = sqlx::query(
        "SELECT a.account_name, a.operator_name AS account_operator, p.publisher_name, count(*) AS registrations \
           FROM linggan_ops_publication p \
           JOIN linggan_ops_account a ON a.account_ref = p.account_ref \
          WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL \
            AND p.published_business_date BETWEEN $2::date AND $3::date \
            AND a.operator_name IS NOT NULL AND p.publisher_name IS NOT NULL \
            AND a.operator_name <> p.publisher_name \
            AND ($1::uuid IS NULL OR a.domain_ref = $1) \
          GROUP BY 1, 2, 3",
    )
    .bind(business.domain_ref())
    .bind(&covered.from)
    .bind(&covered.to)
    .fetch_all(database.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(OverviewAttentionItem {
                operator: Some(row.try_get("publisher_name")?),
                account: Some(row.try_get("account_name")?),
                account_operator: Some(row.try_get("account_operator")?),
                count: Some(row.try_get("registrations")?),
                ..item("owner_mismatch", "medium")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::people::{OperatorFigures, ranking};
    use super::{OverviewAttention, drops};

    #[test]
    fn a_missing_daily_report_is_not_a_drop_but_fewer_registrations_are() {
        let figures = OperatorFigures {
            name: "运营甲".into(),
            current_published: 0,
            previous_published: 4,
            current_daily_rows: 0,
            current_links: None,
            previous_links: Some(10),
            current_followers: None,
            previous_followers: Some(5),
            ..OperatorFigures::default()
        };
        let found = drops(&figures);
        assert_eq!(
            found.len(),
            1,
            "only the registration count is known to have fallen"
        );
        assert_eq!(found[0].metric, Some("published"));
        assert_eq!(found[0].drop_pct, Some(-100.0));
        let reported = OperatorFigures {
            current_daily_rows: 2,
            current_links: Some(7),
            current_followers: Some(4),
            current_published: 2,
            ..figures
        };
        let found = drops(&reported);
        assert_eq!(
            found.iter().map(|item| item.metric).collect::<Vec<_>>(),
            [Some("published"), Some("links_opened")],
            "−50% published and −30% links drop; −20% followers does not"
        );
    }

    #[test]
    fn the_ranking_row_names_the_figure_that_fell() {
        let figures = OperatorFigures {
            name: "运营甲".into(),
            current_published: 1,
            previous_published: 4,
            ..OperatorFigures::default()
        };
        let all = drops(&figures);
        let attention = OverviewAttention {
            items: all.clone(),
            hidden_count: 0,
            total_count: all.len(),
            all,
        };
        let rows = ranking(&[figures], &attention);
        assert_eq!(
            (rows[0].risk, rows[0].risk_kind, rows[0].risk_metric),
            (Some("high"), Some("drop"), Some("published"))
        );
    }
}
