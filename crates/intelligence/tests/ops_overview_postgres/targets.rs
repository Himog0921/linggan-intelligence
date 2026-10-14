//! Target saves: one receipt per request, a linear revision history per period, no lost update.

use super::fixture::proof_database;
use linggan_intelligence::ops_ledger::targets::{
    OpsTargetError, SetTargetRequest, TargetPeriod, read_ops_target_history, set_ops_target,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn monday_yesterday_reads_and_changes_todays_week_target() {
    let db = super::imported("ops_monday_yesterday_target").await;
    for (today, yesterday, previous_key, current_key) in [
        ("2026-10-12", "2026-10-11", "2026-W41", "2026-W42"),
        ("2026-11-02", "2026-11-01", "2026-W44", "2026-W45"),
        ("2027-01-04", "2027-01-03", "2026-W53", "2027-W01"),
    ] {
        let previous = TargetPeriod::parse("week", previous_key).unwrap();
        let current = TargetPeriod::parse("week", current_key).unwrap();
        for (period, value) in [(previous.clone(), 100), (current.clone(), 200)] {
            set_ops_target(
                &db,
                &SetTargetRequest {
                    period,
                    target_value: value,
                    expected_revision: None,
                    request_ref: Uuid::new_v4(),
                },
            )
            .await
            .unwrap();
        }
        let page = super::overview(
            &db,
            super::OpsBusiness::All,
            super::OverviewSlice::Yesterday,
            today,
        )
        .await;
        assert_eq!(page.window.current.from, yesterday);
        assert_eq!(page.target.period_key.as_deref(), Some(current_key));
        assert_eq!(
            page.target.period_key.as_deref(),
            Some(page.window.this_week.as_str())
        );
        let revision = page.target.revision.unwrap();
        assert_eq!(
            revision.target_value, 200,
            "display the current week, never last week"
        );
        let changed = set_ops_target(
            &db,
            &SetTargetRequest {
                period: current,
                target_value: 300,
                expected_revision: Some(revision.revision_ref),
                request_ref: Uuid::new_v4(),
            },
        )
        .await
        .unwrap();
        assert_eq!(changed.outcome, "accepted");
        let refreshed = super::overview(
            &db,
            super::OpsBusiness::All,
            super::OverviewSlice::Yesterday,
            today,
        )
        .await;
        assert_eq!(refreshed.target.revision.unwrap().target_value, 300);
        assert_eq!(
            read_ops_target_history(&db, &previous)
                .await
                .unwrap()
                .current
                .unwrap()
                .target_value,
            100
        );
    }
}

fn save(value: i32, expected: Option<Uuid>, request: Uuid) -> SetTargetRequest {
    SetTargetRequest {
        period: TargetPeriod::parse("week", "2026-W36").unwrap(),
        target_value: value,
        expected_revision: expected,
        request_ref: request,
    }
}

async fn count(db: &Database, table: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn every_save_leaves_a_receipt_and_only_a_current_view_can_change_the_target() {
    let db = proof_database("ops_target_saves").await;
    let first = Uuid::new_v4();
    let accepted = set_ops_target(&db, &save(100, None, first)).await.unwrap();
    assert_eq!(
        (accepted.outcome.as_str(), accepted.replayed),
        ("accepted", false)
    );
    let v1 = accepted.current.clone().unwrap();
    assert_eq!(
        (
            v1.target_value,
            v1.previous_revision_ref,
            v1.set_by.as_str()
        ),
        (100, None, "local_machine")
    );

    let replay = set_ops_target(&db, &save(100, None, first)).await.unwrap();
    assert_eq!(
        (
            replay.outcome.as_str(),
            replay.replayed,
            replay.current.map(|r| r.revision_ref)
        ),
        ("accepted", true, Some(v1.revision_ref)),
        "a retried request returns its first receipt and saves nothing"
    );
    assert!(matches!(
        set_ops_target(&db, &save(101, None, first)).await,
        Err(OpsTargetError::RequestConflict)
    ));

    let stale = set_ops_target(&db, &save(120, None, Uuid::new_v4()))
        .await
        .unwrap();
    assert_eq!(
        (
            stale.outcome.as_str(),
            stale.current.map(|r| r.target_value)
        ),
        ("stale", Some(100)),
        "a page that saw no target cannot overwrite the one set since"
    );
    let unchanged = set_ops_target(&db, &save(100, Some(v1.revision_ref), Uuid::new_v4()))
        .await
        .unwrap();
    assert_eq!(unchanged.outcome, "unchanged");
    let changed = set_ops_target(&db, &save(150, Some(v1.revision_ref), Uuid::new_v4()))
        .await
        .unwrap();
    let v2 = changed.current.unwrap();
    assert_eq!(
        (
            changed.outcome.as_str(),
            v2.target_value,
            v2.previous_revision_ref
        ),
        ("accepted", 150, Some(v1.revision_ref))
    );

    let history = read_ops_target_history(&db, &TargetPeriod::parse("week", "2026-W36").unwrap())
        .await
        .unwrap();
    assert_eq!(
        history.current.map(|r| r.revision_ref),
        Some(v2.revision_ref)
    );
    assert_eq!(
        history
            .revisions
            .iter()
            .map(|r| r.target_value)
            .collect::<Vec<_>>(),
        [150, 100]
    );
    assert_eq!(
        (
            count(&db, "linggan_ops_target_revision").await,
            count(&db, "linggan_ops_target_request").await
        ),
        (2, 4),
        "four receipts (accepted, stale, unchanged, accepted); the conflicting retry left none"
    );
    for statement in [
        "UPDATE linggan_ops_target_revision SET target_value = 1",
        "DELETE FROM linggan_ops_target_revision",
        "UPDATE linggan_ops_target_request SET outcome = 'accepted'",
        "DELETE FROM linggan_ops_target_request",
    ] {
        assert!(
            sqlx::query(statement).execute(db.pool()).await.is_err(),
            "{statement}"
        );
    }
    assert!(matches!(
        set_ops_target(&db, &save(0, Some(v2.revision_ref), Uuid::new_v4())).await,
        Err(OpsTargetError::InvalidParameter(_))
    ));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn two_saves_that_saw_the_same_target_cannot_both_land() {
    let db = proof_database("ops_target_race").await;
    let (ten, twenty) = (
        save(10, None, Uuid::new_v4()),
        save(20, None, Uuid::new_v4()),
    );
    let (left, right) = tokio::join!(set_ops_target(&db, &ten), set_ops_target(&db, &twenty));
    let mut outcomes = [left.unwrap().outcome, right.unwrap().outcome];
    outcomes.sort();
    assert_eq!(outcomes, ["accepted", "stale"]);
    assert_eq!(count(&db, "linggan_ops_target_revision").await, 1);
    let second_first = sqlx::query(
        "INSERT INTO linggan_ops_target_revision (revision_ref, period_type, period_key, metric, \
           target_value, previous_revision_ref, request_ref, set_by) \
         VALUES ($1, 'week', '2026-W36', 'followers_gained', 5, NULL, $2, 'local_machine')",
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(db.pool())
    .await;
    assert!(
        second_first.is_err(),
        "the database itself refuses a second start of the same period's history"
    );
}
