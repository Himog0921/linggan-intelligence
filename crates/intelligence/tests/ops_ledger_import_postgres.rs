//! OPS-MIGRATION-001 D1 isolated proofs (Issue #402). Real SQL over synthetic exports only;
//! no real operation data and no shared database.
// The synthetic export rows carry every one of the export's 49 keys in one `json!` literal.
#![recursion_limit = "256"]
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/ops_export_fixture.rs"]
mod ops;

use fixture::proof_database;
use linggan_intelligence::ops_ledger::{
    ApplyOutcome, ChangeCounts, OpsImportError, apply_ops_import, compare_with_ledger,
    preview_ops_import, read_ledger_totals,
};
use linggan_storage_postgres::Database;
use ops::{Export, NOTE_9, first_export, publication, xhs_note};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const MIGRATION: &str = include_str!("../../../database/migrations/0123_ops_ledger.sql");
const FIRST_EXPORTED_AT: &str = "2026-10-12T22:43:00+08:00";
const SECOND_EXPORTED_AT: &str = "2026-10-13T09:00:00+08:00";
const TABLES: [&str; 5] = [
    "linggan_ops_import_batch",
    "linggan_ops_account",
    "linggan_ops_publication",
    "linggan_ops_view_report",
    "linggan_ops_daily_report",
];

#[test]
fn full_schema_fixtures_record_the_actual_ops_ledger_migration_digest() {
    let digest: String = Sha256::digest(MIGRATION.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let ledger_entry = format!("('0123_ops_ledger', '{digest}')");
    for (name, source) in [
        ("material", fixture::MIGRATIONS),
        (
            "api",
            include_str!("../../../apps/api/src/local_web/full_schema_fixture.rs"),
        ),
        ("runtime", include_str!("../../../scripts/local-runtime.sh")),
    ] {
        let registered = if name == "runtime" {
            source.contains("apply_migration_once \"0123_ops_ledger\"")
        } else {
            source.contains(&ledger_entry)
        };
        assert!(
            registered,
            "{name} does not register the actual 0123 migration"
        );
    }
}

async fn row_counts(db: &Database) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in TABLES {
        let sql = format!("SELECT count(*) FROM {table}");
        counts.push(
            sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(db.pool())
                .await
                .unwrap(),
        );
    }
    counts
}

async fn applied(db: &Database, export: &Export, exported_at: &str) -> ChangeCounts {
    let request = export.request(exported_at);
    let preview = preview_ops_import(db, &request).await.unwrap();
    let receipt = apply_ops_import(db, &request, &preview.preview_hash)
        .await
        .unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(
        receipt.changes, preview.changes,
        "apply writes exactly what the preview showed"
    );
    receipt.changes
}

async fn scalar<T>(db: &Database, sql: &'static str, id: &str) -> T
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
{
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn preview_reports_the_reconciliation_and_never_writes() {
    let db = proof_database("ops_ledger_preview").await;
    let preview = preview_ops_import(&db, &first_export().request(FIRST_EXPORTED_AT))
        .await
        .unwrap();
    assert_eq!(
        row_counts(&db).await,
        vec![0; 5],
        "a preview writes nothing"
    );
    assert_eq!(preview.exported_at, "2026-10-12T14:43:00.000Z");
    let figures = serde_json::to_value(&preview.reconciliation).unwrap();
    assert_eq!(
        figures["accounts"]["inLedgerByPlatform"],
        json!({"douyin": 1, "xhs": 2})
    );
    assert_eq!(figures["accounts"]["rebuiltFromPublications"], 1);
    assert_eq!(figures["accounts"]["publicationsOnRebuiltAccounts"], 2);
    assert_eq!(
        figures["accounts"]["profileLinkStates"],
        json!({"missing": 2, "parsed": 1, "pending_match": 1})
    );
    let publications = &figures["publications"];
    assert_eq!(publications["exportRows"], 8);
    assert_eq!(
        (
            publications["duplicateGroups"].clone(),
            publications["mergedRecords"].clone(),
            publications["keptAfterMerge"].clone()
        ),
        (json!(2), json!(2), json!(6))
    );
    assert_eq!(publications["groupsWithConflictingViews"], 1);
    assert_eq!(publications["groupsWithDifferentDates"], 1);
    assert_eq!(
        publications["contentIdStates"],
        json!({"douyin:pending_match": 1, "xhs:parsed_publication_url": 4, "xhs:pending_match": 3})
    );
    assert_eq!(
        publications["byMonth"]["2026-09"],
        json!({"records": 6, "kept": 4})
    );
    assert_eq!(
        publications["byMonth"]["2026-10"],
        json!({"records": 1, "kept": 1}),
        "16:30Z is the next Beijing day"
    );
    assert_eq!(
        (
            publications["suspectYear"].clone(),
            publications["withoutAccount"].clone()
        ),
        (json!(1), json!(1))
    );
    let views = &figures["views"];
    assert_eq!(
        views["withViewCountByPlatform"],
        json!({"douyin": 1, "xhs": 5})
    );
    assert_eq!(
        views["xhsKeptHits"], 2,
        "6000 and exactly 5000 are hits; 4999 and Douyin 8000 are not"
    );
    assert_eq!(
        views["xhsKeptHitsByMonth"],
        json!({"2026-09": 1, "2026-10": 1})
    );
    assert_eq!(
        figures["daily"]["byMonth"]["2026-09"],
        json!({"rows": 2, "linksOpened": 10, "followersGained": 4, "legacyPublishedCount": 3,
               "recomputedPublishedCount": 1, "legacyViralCount": 1})
    );
    assert_eq!(preview.changes.accounts_inserted, 4);
    assert_eq!(preview.changes.publications_inserted, 8);
    assert_eq!(
        preview.changes.view_reports_appended, 6,
        "unknown view counts get no report"
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn apply_keeps_unknowns_rebuilds_deleted_accounts_and_replays_without_duplicates() {
    let db = proof_database("ops_ledger_apply").await;
    let export = first_export();
    let request = export.request(FIRST_EXPORTED_AT);
    let preview = preview_ops_import(&db, &request).await.unwrap();
    let first = apply_ops_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    assert_eq!(first.outcome, ApplyOutcome::Applied);
    let after_first = row_counts(&db).await;
    assert_eq!(after_first, vec![1, 4, 8, 6, 3]);

    let replay = apply_ops_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    assert_eq!(replay.outcome, ApplyOutcome::AlreadyApplied);
    assert_eq!(
        (replay.batch_ref, replay.changes),
        (first.batch_ref, first.changes)
    );
    assert_eq!(
        row_counts(&db).await,
        after_first,
        "replaying the same preview writes nothing"
    );

    let (state, status): (String, Option<String>) = sqlx::query_as(
        "SELECT ledger_state, status FROM linggan_ops_account WHERE source_record_id = 'acc-gone'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), status),
        ("absent_from_export", None),
        "old status is unknown, not active"
    );
    let followers: Option<i64> = scalar(
        &db,
        "SELECT follower_count FROM linggan_ops_account WHERE source_record_id = $1",
        "acc-3",
    )
    .await;
    assert_eq!(followers, None, "an empty follower count stays unknown");
    let merged: Option<String> = scalar(
        &db,
        "SELECT kept.source_record_id FROM linggan_ops_publication p \
           JOIN linggan_ops_publication kept ON kept.publication_ref = p.merged_into_ref \
          WHERE p.source_record_id = $1",
        "pub-2",
    )
    .await;
    assert_eq!(
        merged.as_deref(),
        Some("pub-1"),
        "the duplicate stays and points at the kept record"
    );
    let (date, date_state, content_id): (String, String, Option<String>) = sqlx::query_as(
        "SELECT published_business_date::text, published_date_state, platform_content_id \
           FROM linggan_ops_publication WHERE source_record_id = 'pub-8'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (date.as_str(), date_state.as_str()),
        ("2026-10-01", "as_recorded")
    );
    assert!(content_id.is_some());
    let unknown_views: i64 = scalar(
        &db,
        "SELECT count(*) FROM linggan_ops_view_report r JOIN linggan_ops_publication p USING (publication_ref) \
          WHERE p.source_record_id = ANY(string_to_array($1, ','))",
        "pub-2,pub-6",
    )
    .await;
    assert_eq!(
        unknown_views, 0,
        "no view count means no report, never a zero"
    );
    let checks = compare_with_ledger(
        &preview.reconciliation,
        &read_ledger_totals(&db).await.unwrap(),
    );
    assert!(checks.iter().all(|check| check.consistent), "{checks:?}");
}

fn second_export() -> Export {
    let mut export = first_export();
    export.accounts.retain(|account| account["id"] != "acc-3");
    export.accounts[0]["followerCount"] = json!(1500);
    export.accounts[0]["updatedAt"] = json!("2026-10-12T20:00:00.000Z");
    for publication in &mut export.publications {
        let id = publication["id"].as_str().unwrap().to_owned();
        let touched = match id.as_str() {
            "pub-1" => {
                publication["viewCount"] = json!(7000);
                true
            }
            "pub-3" => {
                publication["title"] = json!("SYNTHETIC 改过的标题");
                true
            }
            "pub-5" => {
                publication["viewCount"] = Value::Null;
                true
            }
            _ => false,
        };
        if touched {
            publication["updatedAt"] = json!("2026-10-12T21:00:00.000Z");
        }
    }
    export
        .publications
        .retain(|publication| publication["id"] != "pub-7");
    export.publications.push(publication(
        "pub-9",
        Some(("acc-1", "SYNTHETIC 账号甲")),
        "xhs",
        &xhs_note(NOTE_9),
        "2026-10-12T02:00:00.000Z",
        Some(300),
    ));
    export.daily.retain(|row| row["id"] != "day-2");
    export
        .daily
        .push(ops::daily("day-4", "运营甲", "2026-09-02", 5, 2, 0));
    export
}

async fn latest_view(db: &Database, id: &str) -> (String, Option<i64>) {
    sqlx::query_as(
        "SELECT r.report_kind, r.view_count FROM linggan_ops_view_report r \
           JOIN linggan_ops_publication p USING (publication_ref) \
          WHERE p.source_record_id = $1 ORDER BY r.report_sequence DESC LIMIT 1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn a_later_export_updates_appends_and_marks_missing_without_deleting() {
    let db = proof_database("ops_ledger_reimport").await;
    let first = applied(&db, &first_export(), FIRST_EXPORTED_AT).await;
    assert_eq!(first.view_reports_appended, 6);
    let other_domain = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO observation_domain (domain_ref, name) VALUES ($1, 'SYNTHETIC 另一领域')",
    )
    .bind(other_domain)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_ops_account SET domain_ref = $1 WHERE source_record_id = 'acc-1'")
        .bind(other_domain)
        .execute(db.pool())
        .await
        .unwrap();

    let second = applied(&db, &second_export(), SECOND_EXPORTED_AT).await;
    let expected = ChangeCounts {
        accounts_inserted: 0,
        accounts_updated: 2,
        accounts_unchanged: 2,
        publications_inserted: 1,
        publications_updated: 3,
        publications_unchanged: 4,
        publications_marked_missing: 1,
        daily_inserted: 1,
        daily_updated: 0,
        daily_unchanged: 2,
        daily_marked_missing: 1,
        view_reports_appended: 3,
    };
    assert_eq!(second, expected);
    let domain: Uuid = scalar(
        &db,
        "SELECT domain_ref FROM linggan_ops_account WHERE source_record_id = $1",
        "acc-1",
    )
    .await;
    assert_eq!(
        domain, other_domain,
        "a re-import never overwrites Linggan's own domain attribution"
    );
    let (state, status): (String, Option<String>) = sqlx::query_as(
        "SELECT ledger_state, status FROM linggan_ops_account WHERE source_record_id = 'acc-3'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (state.as_str(), status.as_deref()),
        ("absent_from_export", Some("active")),
        "last known fields stay"
    );

    assert_eq!(
        latest_view(&db, "pub-1").await,
        ("value".to_owned(), Some(7000))
    );
    assert_eq!(
        latest_view(&db, "pub-5").await,
        ("cleared".to_owned(), None),
        "a cleared value is unknown, not 0"
    );
    let reports_for_pub1: i64 = scalar(
        &db,
        "SELECT count(*) FROM linggan_ops_view_report r JOIN linggan_ops_publication p USING (publication_ref) \
          WHERE p.source_record_id = $1",
        "pub-1",
    )
    .await;
    assert_eq!(
        reports_for_pub1, 2,
        "the old report is kept, the new one appended"
    );
    let missing: bool = scalar(
        &db,
        "SELECT missing_since_batch_ref IS NOT NULL FROM linggan_ops_publication WHERE source_record_id = $1",
        "pub-7",
    )
    .await;
    assert!(
        missing,
        "a record absent from the newer export is marked, not deleted"
    );
    let unchanged_batch: bool = scalar(
        &db,
        "SELECT last_changed_batch_ref = first_batch_ref FROM linggan_ops_publication WHERE source_record_id = $1",
        "pub-8",
    )
    .await;
    assert!(unchanged_batch, "unchanged rows are not rewritten");
    let live_days: i64 = scalar(
        &db,
        "SELECT count(*) FROM linggan_ops_daily_report \
          WHERE operator_name = $1 AND business_date = '2026-09-02' AND missing_since_batch_ref IS NULL",
        "运营甲",
    )
    .await;
    assert_eq!(
        live_days, 1,
        "the replacement row is the only live row of that operator-day"
    );
    let totals = read_ledger_totals(&db).await.unwrap();
    let preview = preview_ops_import(&db, &second_export().request(SECOND_EXPORTED_AT))
        .await
        .unwrap();
    assert!(
        compare_with_ledger(&preview.reconciliation, &totals)
            .iter()
            .all(|check| check.consistent)
    );
    for statement in [
        "DELETE FROM linggan_ops_publication WHERE source_record_id = 'pub-7'",
        "DELETE FROM linggan_ops_account WHERE source_record_id = 'acc-3'",
        "DELETE FROM linggan_ops_daily_report WHERE source_record_id = 'day-2'",
        "UPDATE linggan_ops_view_report SET view_count = 1",
        "DELETE FROM linggan_ops_view_report",
        "UPDATE linggan_ops_import_batch SET daily_rows = 0",
        "DELETE FROM linggan_ops_import_batch",
    ] {
        assert!(
            sqlx::query(statement).execute(db.pool()).await.is_err(),
            "{statement} must be refused"
        );
    }
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn mismatched_previews_unknown_domains_and_bad_export_times_write_nothing() {
    let db = proof_database("ops_ledger_refusals").await;
    let export = first_export();
    let request = export.request(FIRST_EXPORTED_AT);
    let preview = preview_ops_import(&db, &request).await.unwrap();
    let mut changed = first_export();
    changed.publications[0]["viewCount"] = json!(6001);
    let changed_request = changed.request(FIRST_EXPORTED_AT);
    assert!(matches!(
        apply_ops_import(&db, &changed_request, &preview.preview_hash).await,
        Err(OpsImportError::PreviewHashMismatch)
    ));
    let mut other_domain = export.request(FIRST_EXPORTED_AT);
    other_domain.domain_ref = Uuid::new_v4();
    assert!(matches!(
        preview_ops_import(&db, &other_domain).await,
        Err(OpsImportError::UnknownDomain)
    ));
    for exported_at in [
        "2026-10-12T22:43:00",
        "2026-09-30T00:00:00Z",
        "2099-01-01T00:00:00Z",
        "yesterday",
    ] {
        assert!(
            matches!(
                preview_ops_import(&db, &export.request(exported_at)).await,
                Err(OpsImportError::InvalidExportedAt)
            ),
            "{exported_at} must be refused"
        );
    }
    assert_eq!(row_counts(&db).await, vec![0; 5]);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn a_failure_midway_rolls_back_the_whole_batch() {
    let db = proof_database("ops_ledger_atomic").await;
    sqlx::raw_sql(
        "CREATE FUNCTION ops_proof_fail() RETURNS trigger LANGUAGE plpgsql AS \
           $$ BEGIN RAISE EXCEPTION 'synthetic failure after earlier writes'; END $$; \
         CREATE TRIGGER ops_proof_fail BEFORE INSERT ON linggan_ops_daily_report \
           FOR EACH ROW EXECUTE FUNCTION ops_proof_fail();",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let export = first_export();
    let request = export.request(FIRST_EXPORTED_AT);
    let preview = preview_ops_import(&db, &request).await.unwrap();
    assert!(matches!(
        apply_ops_import(&db, &request, &preview.preview_hash).await,
        Err(OpsImportError::Database(_))
    ));
    assert_eq!(
        row_counts(&db).await,
        vec![0; 5],
        "accounts and publications written before the failure are gone"
    );
    sqlx::raw_sql("DROP TRIGGER ops_proof_fail ON linggan_ops_daily_report")
        .execute(db.pool())
        .await
        .unwrap();
    let receipt = apply_ops_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    assert_eq!(
        receipt.outcome,
        ApplyOutcome::Applied,
        "the failed attempt left no receipt behind"
    );
}

fn rejected(error: Result<impl std::fmt::Debug, OpsImportError>) -> String {
    match error {
        Err(error @ OpsImportError::InvalidExport { .. }) => error.to_string(),
        other => panic!("expected an export rejection, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn malformed_exports_are_rejected_whole_without_echoing_values() {
    let db = proof_database("ops_ledger_malformed").await;
    let secret = "SYNTHETIC-不应回显的内容";
    let cases: Vec<(&str, Box<dyn Fn(&mut Export)>)> = vec![
        (
            "unknown field",
            Box::new(|e: &mut Export| {
                e.publications[0]["newField"] = json!(1);
            }),
        ),
        (
            "missing field",
            Box::new(|e: &mut Export| {
                e.accounts[0].as_object_mut().unwrap().remove("handle");
            }),
        ),
        (
            "bad enum",
            Box::new(move |e: &mut Export| {
                e.accounts[0]["status"] = json!(secret);
            }),
        ),
        (
            "text where a count belongs",
            Box::new(move |e: &mut Export| {
                e.publications[0]["viewCount"] = json!(secret);
            }),
        ),
        (
            "bad time",
            Box::new(|e: &mut Export| {
                e.publications[0]["publishedAt"] = json!("2026-09-01 10:00");
            }),
        ),
        (
            "negative count",
            Box::new(|e: &mut Export| {
                e.publications[0]["likeCount"] = json!(-1);
            }),
        ),
        (
            "platform differs from account",
            Box::new(|e: &mut Export| {
                e.publications[0]["platform"] = json!("douyin");
            }),
        ),
        (
            "per-account daily row",
            Box::new(|e: &mut Export| {
                e.daily[0]["accountName"] = json!("SYNTHETIC 账号甲");
            }),
        ),
        (
            "duplicate id",
            Box::new(|e: &mut Export| {
                e.daily[1]["id"] = json!("day-1");
            }),
        ),
        (
            "same operator-day twice",
            Box::new(|e: &mut Export| {
                e.daily[1]["dateKey"] = json!("2026-09-01");
                e.daily[1]["date"] = json!("2026-09-01T00:00:00.000Z");
            }),
        ),
        (
            "deleted account renamed",
            Box::new(|e: &mut Export| {
                e.publications[5]["accountName"] = json!("SYNTHETIC 另一个名字");
            }),
        ),
        (
            "two workspaces",
            Box::new(|e: &mut Export| {
                e.daily[0]["workspaceId"] = json!("another-workspace");
            }),
        ),
        (
            "nul character",
            Box::new(|e: &mut Export| {
                e.publications[0]["title"] = json!("SYNTHETIC\u{0}");
            }),
        ),
        (
            "deleted record exported",
            Box::new(|e: &mut Export| {
                e.accounts[0]["deletedAt"] = json!("2026-10-01T00:00:00.000Z");
            }),
        ),
    ];
    for (name, mutate) in cases {
        let mut export = first_export();
        mutate(&mut export);
        let message = rejected(preview_ops_import(&db, &export.request(FIRST_EXPORTED_AT)).await);
        assert!(
            !message.contains(secret),
            "{name}: the error must not echo values: {message}"
        );
        assert!(
            matches!(
                apply_ops_import(
                    &db,
                    &export.request(FIRST_EXPORTED_AT),
                    "0".repeat(64).as_str()
                )
                .await,
                Err(OpsImportError::InvalidExport { .. })
            ),
            "{name}: apply refuses before writing"
        );
    }
    assert_eq!(row_counts(&db).await, vec![0; 5]);
}
