//! OPS-MIGRATION-001 D3 isolated proofs (Issue #409): the 总览 read model, the weekly and monthly
//! target saves, and the daily execution status. Real SQL over synthetic exports, `today` pinned.
#![recursion_limit = "256"]
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/ops_export_fixture.rs"]
mod ops;

use fixture::proof_database;
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, OpsOverview, OpsOverviewQuery, OverviewSlice, read_ops_daily, read_ops_overview,
    read_ops_overview_trend,
};
use linggan_intelligence::ops_ledger::{apply_ops_import, preview_ops_import};
use linggan_storage_postgres::Database;
use ops::{ADHD_DOMAIN, Export, NOTE_9, daily, first_export, publication, xhs_note};
use serde_json::json;

const EXPORTED_AT: &str = "2026-10-12T22:43:00+08:00";

/// The D1 export plus: a later September day that an equal-length month comparison must leave
/// out, a pending view with its daily row, a quiet day, and a registration by someone other than
/// the account's operator.
fn overview_export() -> Export {
    let mut export = first_export();
    let a1 = Some(("acc-1", "SYNTHETIC 账号甲"));
    export.publications.push(publication(
        "pub-9",
        a1,
        "xhs",
        &xhs_note(NOTE_9),
        "2026-09-06T02:00:00.000Z",
        None,
    ));
    let mut other = publication(
        "pub-10",
        Some(("acc-2", "SYNTHETIC 账号乙")),
        "douyin",
        "https://v.douyin.com/SYN10/",
        "2026-09-05T03:00:00.000Z",
        None,
    );
    other["publisherName"] = json!("运营乙");
    export.publications.push(other);
    export.daily.extend([
        daily("day-4", "运营甲", "2026-09-20", 50, 20, 1),
        daily("day-5", "运营甲", "2026-09-06", 0, 0, 1),
        daily("day-6", "运营甲", "2026-09-03", 0, 0, 1),
        // The export day: the workbench has the row, nobody has filled it in yet.
        daily("day-7", "运营乙", "2026-10-12", 0, 0, 0),
    ]);
    export
}

pub(crate) async fn imported(name: &str) -> Database {
    let db = proof_database(name).await;
    let request = overview_export().request(EXPORTED_AT);
    let preview = preview_ops_import(&db, &request).await.unwrap();
    apply_ops_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    db
}

pub(crate) async fn overview(
    db: &Database,
    business: OpsBusiness,
    slice: OverviewSlice,
    today: &str,
) -> OpsOverview {
    read_ops_overview(
        db,
        business,
        &OpsOverviewQuery {
            slice,
            today: Some(today.into()),
        },
    )
    .await
    .unwrap()
}

#[path = "ops_overview_postgres/figures.rs"]
mod figures;
#[path = "ops_overview_postgres/targets.rs"]
mod targets;

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn trend_coverage_distinguishes_missing_partial_and_known_zero_buckets() {
    let empty = proof_database("ops_trend_never_imported").await;
    let db = imported("ops_trend_coverage").await;
    for granularity in ["day", "week", "month"] {
        let unknown = read_ops_overview_trend(
            &empty,
            OpsBusiness::All,
            Some(granularity),
            Some("2026-10-14"),
        )
        .await
        .unwrap();
        let data = serde_json::to_value(unknown).unwrap();
        for point in data["points"].as_array().unwrap() {
            assert!(
                point["hits"].is_null(),
                "no import: every bucket is unknown"
            );
            assert!(point["followersGained"].is_null());
            assert_eq!(point["coverage"], "not_migrated");
            assert!(point["coveredTo"].is_null());
        }
        let trend =
            read_ops_overview_trend(&db, OpsBusiness::All, Some(granularity), Some("2026-10-14"))
                .await
                .unwrap();
        let data = serde_json::to_value(trend).unwrap();
        let points = data["points"].as_array().unwrap();
        let last = points.last().unwrap();
        if granularity == "month" {
            assert_eq!(last["hits"], 1);
            assert_eq!(last["followersGained"], 1);
            assert_eq!(last["coverage"], "partial");
            assert_eq!(last["coveredTo"], "2026-10-11");
        } else {
            assert!(last["hits"].is_null());
            assert_eq!(last["coverage"], "not_migrated");
            assert!(last["coveredTo"].is_null());
            let known_zero = &points[points.len() - if granularity == "day" { 4 } else { 2 }];
            assert_eq!(known_zero["hits"], 0);
            assert_eq!(known_zero["coverage"], "full");
        }
    }
    let partial_db = proof_database("ops_trend_partial_week").await;
    let mut export = overview_export();
    export
        .daily
        .retain(|row| row["dateKey"].as_str().unwrap() <= "2026-10-08");
    let request = export.request("2026-10-09T22:43:00+08:00");
    let preview = preview_ops_import(&partial_db, &request).await.unwrap();
    apply_ops_import(&partial_db, &request, &preview.preview_hash)
        .await
        .unwrap();
    let trend = read_ops_overview_trend(
        &partial_db,
        OpsBusiness::All,
        Some("week"),
        Some("2026-10-10"),
    )
    .await
    .unwrap();
    let data = serde_json::to_value(trend).unwrap();
    let last = data["points"].as_array().unwrap().last().unwrap();
    assert_eq!(last["coverage"], "partial");
    assert_eq!(last["coveredTo"], "2026-10-08");
    assert_eq!(last["hits"], 0, "known zero only within the covered part");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn trend_buckets_end_today_and_keep_unknown_weeks_apart_from_zero() {
    let db = imported("ops_overview_trend").await;
    let month = read_ops_overview_trend(&db, OpsBusiness::All, Some("month"), Some("2026-10-02"))
        .await
        .unwrap();
    let labels: Vec<&str> = month.points.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "2026-05", "2026-06", "2026-07", "2026-08", "2026-09", "2026-10"
        ]
    );
    let september = &month.points[4];
    assert_eq!(
        (september.followers_gained, september.hits),
        (Some(24), Some(1)),
        "4 + 0 + 20 + 0 + 0; only the Xiaohongshu 6000 is a hit"
    );
    assert_eq!(
        month.points[0].followers_gained, None,
        "no daily report in May: unknown"
    );
    assert_eq!(
        (month.points[5].followers_gained, month.points[5].hits),
        (Some(1), Some(1))
    );
    let adhd = read_ops_overview_trend(
        &db,
        OpsBusiness::Domain(ADHD_DOMAIN),
        Some("month"),
        Some("2026-10-02"),
    )
    .await
    .unwrap();
    assert_eq!(
        (adhd.points[5].followers_gained, adhd.points[5].hits),
        (Some(1), Some(0)),
        "the October hit has no account, so no business; 加微 is not split by business"
    );
    let weeks = read_ops_overview_trend(&db, OpsBusiness::All, Some("week"), Some("2026-10-02"))
        .await
        .unwrap();
    assert_eq!(weeks.points.len(), 12);
    assert_eq!(weeks.points.last().unwrap().label, "2026-W40");
    let days = read_ops_overview_trend(&db, OpsBusiness::All, None, Some("2026-10-02"))
        .await
        .unwrap();
    assert_eq!(days.points.len(), 90);
    assert_eq!(days.points.last().unwrap().start, "2026-10-02");
    assert!(
        read_ops_overview_trend(&db, OpsBusiness::All, Some("quarter"), None)
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn daily_execution_status_uses_the_new_rules() {
    let db = imported("ops_overview_execution").await;
    let page = read_ops_daily(&db, OpsBusiness::All, None, None)
        .await
        .unwrap();
    let status = |id: &str| {
        page.items
            .iter()
            .find(|item| item.source_id == id)
            .unwrap()
            .execution
    };
    assert_eq!(status("day-1"), "praise", "a 6000 hit that day");
    assert_eq!(
        status("day-2"),
        "not_published",
        "its registration was merged into day-1's"
    );
    assert_eq!(
        status("day-3"),
        "not_published",
        "the hit that day has no publisher"
    );
    assert_eq!(
        status("day-5"),
        "pending_views",
        "the 09-06 post still has no views"
    );
    assert_eq!(
        status("day-6"),
        "done",
        "a post with views, no hit and no followers is no longer 需复盘"
    );
}
