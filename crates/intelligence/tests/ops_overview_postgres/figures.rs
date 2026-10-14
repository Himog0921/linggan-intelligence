//! 总览 figures over the synthetic export. Beijing calendar: 2026-09-01 is a Tuesday in ISO week
//! 2026-W36 (Monday 08-31).

use super::{imported, overview};
use linggan_intelligence::ops_ledger::read::{OpsBusiness, OpsOverview, OverviewSlice};
use linggan_intelligence::ops_ledger::targets::{SetTargetRequest, TargetPeriod, set_ops_target};
use linggan_storage_postgres::Database;
use uuid::Uuid;

use super::ops::ADHD_DOMAIN;

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn a_week_so_far_is_read_with_unknowns_and_every_block_filled() {
    let db = imported("ops_overview_week").await;
    let week = overview(&db, OpsBusiness::All, OverviewSlice::Week, "2026-09-05").await;
    let window = &week.window;
    assert_eq!(
        (
            window.current.from.as_str(),
            window.current.to.as_str(),
            window.current.days
        ),
        ("2026-08-31", "2026-09-05", 6)
    );
    assert_eq!(
        (
            window.previous.as_ref().unwrap().from.as_str(),
            window.previous.as_ref().unwrap().to.as_str()
        ),
        ("2026-08-24", "2026-08-29"),
        "same weekdays of last week"
    );
    assert_eq!(
        (window.this_week.as_str(), window.this_month.as_str()),
        ("2026-W36", "2026-09")
    );
    let kpi = &week.kpi;
    assert_eq!(
        (kpi.current.published, kpi.current.hits),
        (Some(5), Some(1)),
        "merged duplicates count once; only the Xiaohongshu 6000 is a hit"
    );
    assert_eq!(
        (kpi.current.links_opened, kpi.current.followers_gained),
        (Some(10), Some(4))
    );
    assert_eq!(kpi.current.conversion_rate, Some(40.0));
    assert_eq!(
        (
            kpi.previous.links_opened,
            kpi.delta.followers_gained_pct,
            kpi.delta.published_pct
        ),
        (None, None, None),
        "no daily report last week: unknown, never a fall to 0; a previous 0 is not compared"
    );
    assert_eq!(kpi.effective_publication.state, "not_connected");
    assert_eq!(
        (week.target.state, week.target.period_key.as_deref()),
        ("not_set", Some("2026-W36"))
    );
    assert!(
        week.target.achieved_pct.is_none() && week.insights.is_empty(),
        "no target, no estimate, no conclusion"
    );

    let kinds: Vec<(&str, Option<&str>)> = week
        .attention
        .items
        .iter()
        .map(|item| (item.kind, item.operator.as_deref()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("owner_mismatch", Some("运营乙")),
            ("pending_views", Some("运营甲"))
        ],
        "the 2024 post is overdue; the 09-06 post is after today and not yet"
    );
    assert_eq!(
        week.attention.items[1].oldest_date.as_deref(),
        Some("2024-03-05")
    );
    assert_eq!(
        week.attention.items[0].account_operator.as_deref(),
        Some("运营甲")
    );
    week_ranking_share_and_matrix(&week);
}

/// The same week, blocks 4 and 5: 运营人排行, 平台占比 and the 运营人 × 账号 matrix.
fn week_ranking_share_and_matrix(week: &OpsOverview) {
    let first = &week.ranking[0];
    assert_eq!(
        (
            first.operator.as_str(),
            first.platform,
            first.published,
            first.followers_gained
        ),
        ("运营甲", Some("both"), 4, Some(4))
    );
    assert_eq!(
        (first.responsible_accounts, first.pending_views, first.hits),
        (3, 1, 1)
    );
    assert_eq!(
        first.sparkline,
        [None, None, Some(4), Some(0), Some(0), None, None]
    );
    assert_eq!(
        (first.risk, first.risk_kind, first.risk_metric),
        (Some("medium"), Some("pending_views"), None)
    );
    let second = &week.ranking[1];
    assert_eq!(
        (
            second.operator.as_str(),
            second.platform,
            second.followers_gained
        ),
        ("运营乙", Some("douyin"), None),
        "a publisher without daily reports sorts after known 加微"
    );

    assert_eq!(
        (week.platform_share.xhs, week.platform_share.douyin),
        (3, 2)
    );
    assert_eq!(week.daily_output.days.len(), 6);
    assert_eq!(week.matrix.operators, ["运营乙", "运营甲"]);
    let names: Vec<&str> = week
        .matrix
        .accounts
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["SYNTHETIC 已删号", "SYNTHETIC 账号甲", "SYNTHETIC 账号乙"]
    );
    assert!(week.matrix.accounts[0].retired);
    let cell = week
        .matrix
        .cells
        .iter()
        .find(|c| c.operator == "运营甲" && c.account == week.matrix.accounts[1].r#ref)
        .unwrap();
    assert_eq!((cell.published, cell.hits), (2, 1));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn a_month_so_far_compares_the_same_days_and_flags_real_drops_only() {
    let db = imported("ops_overview_month").await;
    let month = overview(&db, OpsBusiness::All, OverviewSlice::Month, "2026-10-02").await;
    assert_eq!(
        (
            month.window.previous.as_ref().unwrap().from.as_str(),
            month.window.previous.as_ref().unwrap().to.as_str()
        ),
        ("2026-09-01", "2026-09-02")
    );
    let kpi = &month.kpi;
    assert_eq!(
        (kpi.previous.links_opened, kpi.previous.followers_gained),
        (Some(10), Some(4)),
        "the 09-20 report (+20) is outside the equal-length window"
    );
    assert_eq!(
        (
            kpi.delta.published_pct,
            kpi.delta.links_opened_pct,
            kpi.delta.followers_gained_pct
        ),
        (Some(0.0), Some(-70.0), Some(-75.0))
    );
    let items: Vec<(&str, Option<&str>, Option<f64>)> = month
        .attention
        .items
        .iter()
        .map(|item| (item.kind, item.metric, item.drop_pct))
        .collect();
    assert_eq!(
        items,
        [
            ("drop", Some("published"), Some(-100.0)),
            ("pending_views", None, None)
        ],
        "运营甲 has no daily report this month: his 加微 is unknown, not a fall"
    );
    assert_eq!(month.attention.items[1].count, Some(2));
    let insight_kinds: Vec<&str> = month.insights.iter().map(|insight| insight.kind).collect();
    assert_eq!(insight_kinds, ["drag", "review"]);
    assert_eq!(
        (month.insights[0].metric, month.insights[0].delta_pct),
        (Some("links_opened"), Some(-70.0))
    );
    assert_eq!(month.insights[1].names, ["运营甲"]);
    assert_eq!(
        month
            .ranking
            .iter()
            .map(|row| row.operator.as_str())
            .collect::<Vec<_>>(),
        ["运营乙"],
        "only operators active in the window are ranked"
    );
    a_business_keeps_daily_figures_whole(&db).await;

    let period = TargetPeriod::parse("month", "2026-10").unwrap();
    set_ops_target(
        &db,
        &SetTargetRequest {
            period,
            target_value: 30,
            expected_revision: None,
            request_ref: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let set = overview(&db, OpsBusiness::All, OverviewSlice::Month, "2026-10-02").await;
    let target = &set.target;
    assert_eq!(
        (target.state, target.period_followers, target.elapsed_days),
        ("set", Some(1), Some(2))
    );
    assert_eq!(
        (target.achieved_pct, target.time_progress_pct, target.pace),
        (Some(3.3), Some(6.5), Some("on_track"))
    );
    assert_eq!(
        (target.daily_needed, target.projected_end),
        (Some(1), Some(16))
    );
    assert_eq!(
        (set.insights[0].kind, set.insights[0].projected_pct),
        ("pace", Some(53.0))
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn yesterday_and_custom_windows_have_their_own_comparison_and_target_rules() {
    let db = imported("ops_overview_windows").await;
    let yesterday = overview(
        &db,
        OpsBusiness::All,
        OverviewSlice::Yesterday,
        "2026-09-02",
    )
    .await;
    assert_eq!(
        (
            yesterday.window.current.from.as_str(),
            yesterday.window.previous.as_ref().unwrap().from.as_str(),
            yesterday.kpi.current.published
        ),
        ("2026-09-01", "2026-08-31", Some(1))
    );
    assert_eq!(yesterday.target.period_key.as_deref(), Some("2026-W36"));
    let custom = overview(
        &db,
        OpsBusiness::All,
        OverviewSlice::Custom {
            from: "2026-09-01".into(),
            to: "2026-09-05".into(),
        },
        "2026-10-13",
    )
    .await;
    assert_eq!(
        (
            custom.window.current.days,
            custom.window.previous.as_ref().unwrap().from.as_str(),
            custom.window.previous.as_ref().unwrap().to.as_str()
        ),
        (5, "2026-08-27", "2026-08-31")
    );
    assert_eq!(custom.target.state, "not_applicable");
    let too_long = linggan_intelligence::ops_ledger::read::read_ops_overview(
        &db,
        OpsBusiness::All,
        &linggan_intelligence::ops_ledger::read::OpsOverviewQuery {
            slice: OverviewSlice::Custom {
                from: "2025-01-01".into(),
                to: "2026-09-05".into(),
            },
            today: Some("2026-10-13".into()),
        },
    )
    .await;
    assert!(too_long.is_err(), "longer than 366 days is refused");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn days_from_the_export_day_on_are_unknown_not_zero() {
    let db = imported("ops_overview_coverage").await;
    // Exported 2026-10-12 22:43: 10-12 is the export day, 10-11 the last complete one.
    let week = overview(&db, OpsBusiness::All, OverviewSlice::Week, "2026-10-13").await;
    assert_eq!(week.window.complete_through.as_deref(), Some("2026-10-11"));
    assert!(week.window.covered.is_none() && week.window.previous.is_none());
    assert_eq!(
        (
            week.kpi.current.published,
            week.kpi.current.followers_gained,
            week.kpi.delta.followers_gained_pct
        ),
        (None, None, None),
        "the export-day row of zeros is not counted"
    );
    assert!(
        week.attention
            .items
            .iter()
            .all(|item| item.kind == "pending_views"),
        "no drop is claimed from days not migrated yet"
    );
    assert!(week.ranking.is_empty() && week.insights.is_empty());
    assert_eq!(week.daily_output.covered_to, None);
    assert_eq!(
        (week.target.period_key.as_deref(), week.target.elapsed_days),
        (Some("2026-W42"), None)
    );

    let month = overview(&db, OpsBusiness::All, OverviewSlice::Month, "2026-10-13").await;
    let covered = month.window.covered.as_ref().unwrap();
    assert_eq!((covered.to.as_str(), covered.days), ("2026-10-11", 11));
    let previous = month.window.previous.as_ref().unwrap();
    assert_eq!(
        (previous.from.as_str(), previous.to.as_str()),
        ("2026-09-01", "2026-09-11")
    );
    assert_eq!(
        (
            month.kpi.current.followers_gained,
            month.kpi.previous.followers_gained
        ),
        (Some(1), Some(4)),
        "October through 10-11 against September 1–11; 09-20 and the 10-12 zeros are out"
    );

    let trend = linggan_intelligence::ops_ledger::read::read_ops_overview_trend(
        &db,
        OpsBusiness::All,
        None,
        Some("2026-10-13"),
    )
    .await
    .unwrap();
    let last: Vec<Option<i64>> = trend
        .points
        .iter()
        .rev()
        .take(2)
        .map(|p| p.followers_gained)
        .collect();
    assert_eq!(
        last,
        [None, None],
        "10-12 and 10-13 are unknown, though 10-12 has a row of zeros"
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn a_period_recorded_all_zero_asks_for_a_check_instead_of_claiming_a_fall() {
    let db = imported("ops_overview_zero").await;
    // 09-03..09-06: 运营甲's two daily reports are 0 and 0; 08-30..09-02 had 10 links and 4.
    let window = overview(
        &db,
        OpsBusiness::All,
        OverviewSlice::Custom {
            from: "2026-09-03".into(),
            to: "2026-09-06".into(),
        },
        "2026-10-02",
    )
    .await;
    let mine: Vec<_> = window
        .attention
        .items
        .iter()
        .filter(|item| item.operator.as_deref() == Some("运营甲"))
        .collect();
    assert!(
        mine.iter()
            .all(|item| item.kind != "drop" || item.metric == Some("published")),
        "zeros in the daily form are never a fall in 链接打开 or 加微"
    );
    let check = mine
        .iter()
        .find(|item| item.kind == "zero_report")
        .expect("a check for the period recorded as 0");
    assert_eq!((check.severity, check.count), ("medium", Some(2)));
    let attention = &window.attention;
    assert_eq!(
        (
            attention.rest.len(),
            attention.items.len() + attention.rest.len()
        ),
        (attention.hidden_count, attention.total_count),
        "the folded items come with the page, not only their number"
    );
    assert_eq!(
        check
            .zero_metrics
            .iter()
            .map(|zero| (zero.metric, zero.previous_value))
            .collect::<Vec<_>>(),
        [("links_opened", 10), ("followers_gained", 4)]
    );
    let row = window
        .ranking
        .iter()
        .find(|row| row.operator == "运营甲")
        .unwrap();
    assert_eq!(
        (row.links_opened, row.followers_gained),
        (Some(0), Some(0)),
        "the ranking still shows what was recorded"
    );
    let dropped: Vec<&str> = window
        .attention
        .items
        .iter()
        .filter(|item| item.kind == "drop")
        .filter_map(|item| item.operator.as_deref())
        .collect();
    assert!(
        window
            .insights
            .iter()
            .filter(|insight| insight.kind == "review")
            .flat_map(|insight| insight.names.iter())
            .all(|name| dropped.contains(&name.as_str())),
        "only a real drop names someone for review; a possibly unfilled period does not"
    );
}

/// Selecting a business filters registrations only: 加微 is recorded per operator, never per
/// business, and 发布 → 打开 is not computed across the two.
async fn a_business_keeps_daily_figures_whole(db: &Database) {
    let adhd = overview(
        db,
        OpsBusiness::Domain(ADHD_DOMAIN),
        OverviewSlice::Month,
        "2026-10-02",
    )
    .await;
    assert_eq!(
        (
            adhd.kpi.current.published,
            adhd.kpi.current.hits,
            adhd.kpi.current.followers_gained
        ),
        (Some(0), Some(0), Some(1)),
        "the October post has no account; daily reports are not split by business"
    );
    assert!(adhd.business_selected && !adhd.funnel.publish_to_open_available);
    assert_eq!(adhd.funnel.current.publish_to_open, None);
}
