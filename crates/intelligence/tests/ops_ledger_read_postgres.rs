//! OPS-MIGRATION-001 D2 isolated proofs (Issue #405): the read models behind the 「运营」 pages
//! and the historical cover import. Real SQL over synthetic exports only.
#![recursion_limit = "256"]
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/ops_export_fixture.rs"]
mod ops;

use fixture::proof_database;
use linggan_intelligence::ops_ledger::covers::{
    CoverImportRequest, apply_cover_import, preview_cover_import,
};
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, OpsPublicationFilter, OpsReadError, read_ops_accounts, read_ops_businesses,
    read_ops_cover, read_ops_daily, read_ops_publications, read_ops_summary,
};
use linggan_intelligence::ops_ledger::{OpsImportError, apply_ops_import, preview_ops_import};
use linggan_storage_postgres::Database;
use ops::{ADHD_DOMAIN, Export, first_export};
use serde_json::json;
use std::path::PathBuf;
use uuid::Uuid;

const EXPORTED_AT: &str = "2026-10-12T22:43:00+08:00";

async fn imported(name: &str) -> Database {
    let db = proof_database(name).await;
    import(&db, &first_export()).await;
    db
}

async fn import(db: &Database, export: &Export) {
    let request = export.request(EXPORTED_AT);
    let preview = preview_ops_import(db, &request).await.unwrap();
    apply_ops_import(db, &request, &preview.preview_hash)
        .await
        .unwrap();
}

fn filter() -> OpsPublicationFilter {
    OpsPublicationFilter {
        page: 1,
        ..Default::default()
    }
}

async fn ids(db: &Database, business: OpsBusiness, filter: OpsPublicationFilter) -> Vec<String> {
    let page = read_ops_publications(db, business, filter).await.unwrap();
    page.items.into_iter().map(|item| item.source_id).collect()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn publication_list_shows_kept_records_newest_first_with_their_states() {
    let db = imported("ops_read_publications").await;
    let page = read_ops_publications(&db, OpsBusiness::All, filter())
        .await
        .unwrap();
    assert_eq!(
        page.total, 6,
        "two duplicate registrations are merged into their kept records"
    );
    let order: Vec<&str> = page
        .items
        .iter()
        .map(|item| item.source_id.as_str())
        .collect();
    assert_eq!(
        order,
        ["pub-8", "pub-7", "pub-5", "pub-3", "pub-1", "pub-6"],
        "newest first"
    );
    let by_id = |id: &str| page.items.iter().find(|item| item.source_id == id).unwrap();
    assert_eq!(
        by_id("pub-1").duplicates.len(),
        1,
        "pub-2 shows under pub-1"
    );
    assert_eq!(by_id("pub-1").duplicates[0].business_date, "2026-09-02");
    let views = |id: &str| by_id(id).views.clone();
    assert_eq!(
        (views("pub-1").value, views("pub-1").hit),
        (Some(6000), true)
    );
    assert_eq!(
        (views("pub-8").value, views("pub-8").hit),
        (Some(5000), true),
        "exactly 5000 is a hit"
    );
    assert!(!views("pub-5").hit, "4999 is not a hit");
    assert!(!views("pub-7").hit, "Douyin is never a hit");
    assert_eq!(
        (
            views("pub-6").state,
            views("pub-6").value,
            views("pub-6").overdue
        ),
        ("pending", None, true)
    );
    assert!(by_id("pub-8").account.is_none(), "no account stays unknown");
    assert_eq!(
        by_id("pub-5").account.as_ref().unwrap().ledger_state,
        "absent_from_export"
    );
    assert_eq!(by_id("pub-6").date_state, "suspect_year");
    assert_eq!(by_id("pub-1").system_check, "not_checked");
    assert!(!by_id("pub-1").cover.available);
    assert_eq!(page.operators, ["运营甲"]);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn publication_filters_narrow_the_kept_records_and_keep_search_literal() {
    let db = imported("ops_read_publication_filters").await;
    let with = |change: fn(&mut OpsPublicationFilter)| {
        let mut value = filter();
        change(&mut value);
        value
    };
    assert_eq!(
        ids(
            &db,
            OpsBusiness::All,
            with(|f| f.platform = Some("douyin".into()))
        )
        .await,
        ["pub-7"]
    );
    assert_eq!(
        ids(
            &db,
            OpsBusiness::All,
            with(|f| f.operator = Some("运营甲".into()))
        )
        .await
        .len(),
        5
    );
    assert_eq!(
        ids(
            &db,
            OpsBusiness::All,
            with(|f| f.account = Some("账号甲".into()))
        )
        .await,
        ["pub-3", "pub-1"]
    );
    assert!(
        ids(
            &db,
            OpsBusiness::All,
            with(|f| f.account = Some("%".into()))
        )
        .await
        .is_empty(),
        "% is literal"
    );
    let range = with(|f| {
        f.from = Some("2026-09-03".into());
        f.to = Some("2026-09-04".into());
    });
    assert_eq!(
        ids(&db, OpsBusiness::All, range).await,
        ["pub-5", "pub-3"],
        "both ends included"
    );
    assert_eq!(
        ids(&db, OpsBusiness::All, with(|f| f.pending_views = true)).await,
        ["pub-6"]
    );
    let adhd = ids(&db, OpsBusiness::Domain(ADHD_DOMAIN), filter()).await;
    assert_eq!(
        adhd.len(),
        5,
        "a record without account belongs to no business"
    );
    let beyond = read_ops_publications(&db, OpsBusiness::All, with(|f| f.page = 2))
        .await
        .unwrap();
    assert_eq!((beyond.total, beyond.items.len()), (6, 0));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn business_selection_only_offers_domains_with_accounts() {
    let db = imported("ops_read_business").await;
    let businesses = read_ops_businesses(&db).await.unwrap();
    assert_eq!(businesses.len(), 1);
    assert_eq!(
        (businesses[0].domain_ref, businesses[0].account_count),
        (ADHD_DOMAIN, 3),
        "the deleted account is not counted, the same as 账号管理"
    );
    let empty_domain = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO observation_domain (domain_ref, name) VALUES ($1, 'SYNTHETIC 空业务')",
    )
    .bind(empty_domain)
    .execute(db.pool())
    .await
    .unwrap();
    for business in [
        OpsBusiness::Domain(empty_domain),
        OpsBusiness::Domain(Uuid::new_v4()),
    ] {
        assert!(matches!(
            read_ops_publications(&db, business, filter()).await,
            Err(OpsReadError::BusinessNotFound)
        ));
        assert!(matches!(
            read_ops_summary(&db, business).await,
            Err(OpsReadError::BusinessNotFound)
        ));
    }
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn summary_counts_this_beijing_month_and_is_unknown_before_any_import() {
    let db = proof_database("ops_read_summary").await;
    let before = read_ops_summary(&db, OpsBusiness::All).await.unwrap();
    assert_eq!(
        (
            before.month_published,
            before.month_hits,
            before.active_accounts
        ),
        (None, None, None)
    );
    assert_eq!(
        before.month_followers_gained.value, None,
        "an empty mirror proves nothing"
    );
    import(&db, &first_export()).await;
    // pub-8 sits on Beijing 2026-10-01; move it far into the past so the month is the test's own.
    sqlx::query("UPDATE linggan_ops_publication SET published_at = now() - interval '400 days' WHERE source_record_id = 'pub-8'")
        .execute(db.pool())
        .await
        .unwrap();
    for id in ["pub-1", "pub-2", "pub-7"] {
        sqlx::query(
            "UPDATE linggan_ops_publication SET published_at = now() WHERE source_record_id = $1",
        )
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    }
    sqlx::query("UPDATE linggan_ops_daily_report SET business_date = business_date - 400 WHERE source_record_id = 'day-3'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_ops_daily_report SET business_date = (now() AT TIME ZONE 'Asia/Shanghai')::date WHERE source_record_id = 'day-1'")
        .execute(db.pool())
        .await
        .unwrap();
    let summary = read_ops_summary(&db, OpsBusiness::All).await.unwrap();
    assert_eq!(
        summary.month_published,
        Some(2),
        "the merged pub-2 is not counted again"
    );
    assert_eq!(summary.month_hits, Some(1), "Douyin 8000 is not a hit");
    assert_eq!(
        (summary.active_accounts, summary.ledger_accounts),
        (Some(3), Some(3))
    );
    assert_eq!(summary.month_followers_gained.value, Some(4));
    assert_eq!(summary.effective_publication.state, "not_connected");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn accounts_and_daily_rows_follow_the_workbench_and_recount_from_registrations() {
    let db = imported("ops_read_accounts_daily").await;
    let accounts = read_ops_accounts(&db, OpsBusiness::All, None)
        .await
        .unwrap();
    assert_eq!(
        (accounts.total, accounts.active, accounts.items.len()),
        (3, 3, 3),
        "deleted accounts are not listed"
    );
    assert_eq!(
        accounts.items[0].platform, "douyin",
        "operator, then platform, then name"
    );
    assert!(
        accounts
            .items
            .iter()
            .all(|item| item.health.state == "not_filled")
    );
    let missing_followers = accounts
        .items
        .iter()
        .find(|item| item.source_id == "acc-3")
        .unwrap();
    assert_eq!(missing_followers.follower_count, None);

    let daily = read_ops_daily(&db, OpsBusiness::All, None, None)
        .await
        .unwrap();
    let rows: Vec<(&str, i64, i64, i64)> = daily
        .items
        .iter()
        .map(|item| {
            (
                item.source_id.as_str(),
                item.published,
                item.hits,
                item.month_published,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [("day-3", 0, 0, 0), ("day-2", 0, 0, 1), ("day-1", 1, 1, 1)]
    );
    let day_two = &daily.items[1];
    assert_eq!(
        (
            day_two.weekday,
            day_two.add_rate,
            day_two.month_followers_gained
        ),
        ("星期三", None, 4)
    );
    assert_eq!(daily.items[2].add_rate, Some(0.4));
    assert_eq!(daily.scope, "all_businesses");
    let only = read_ops_daily(
        &db,
        OpsBusiness::Domain(ADHD_DOMAIN),
        Some("运营乙"),
        Some(1),
    )
    .await
    .unwrap();
    assert_eq!(
        only.items
            .iter()
            .map(|item| item.source_id.as_str())
            .collect::<Vec<_>>(),
        ["day-3"]
    );
}

fn with_cover(export: &mut Export, id: &str, path: &str) {
    let row = export
        .publications
        .iter_mut()
        .find(|row| row["id"] == id)
        .unwrap();
    row["media"]["cover"] =
        json!({ "source": "proven", "url": format!("/api/local-media/{path}") });
    row["media"]["state"] = json!("ready");
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ops-cover-proof-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const COVER_BASE: &str = "workspaces/ws/media";

/// Two real images, a fake one, a missing file, and a cover for a record the ledger never saw.
fn cover_fixture() -> (PathBuf, PathBuf, CoverImportRequest) {
    let (covers, media) = (scratch(), scratch());
    let folder = covers.join(COVER_BASE).join("covers");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("one.webp"), b"RIFF\x10\0\0\0WEBPVP8 synthetic").unwrap();
    std::fs::write(folder.join("two.jpg"), [0xFF, 0xD8, 0xFF, 0xE0, 1, 2]).unwrap();
    std::fs::write(folder.join("fake.webp"), b"<svg onload=1>").unwrap();
    let mut export = first_export();
    with_cover(
        &mut export,
        "pub-1",
        &format!("{COVER_BASE}/covers/one.webp"),
    );
    with_cover(
        &mut export,
        "pub-7",
        &format!("{COVER_BASE}/covers/two.jpg"),
    );
    with_cover(
        &mut export,
        "pub-3",
        &format!("{COVER_BASE}/covers/fake.webp"),
    );
    with_cover(
        &mut export,
        "pub-5",
        &format!("{COVER_BASE}/covers/missing.webp"),
    );
    let mut extra = ops::publication(
        "pub-x",
        None,
        "xhs",
        "https://xhslink.com/a/X",
        "2026-09-06T02:00:00.000Z",
        None,
    );
    extra["media"]["cover"] = json!({
        "source": "proven",
        "url": format!("/api/local-media/{COVER_BASE}/covers/one.webp")
    });
    export.publications.push(extra);
    let request = CoverImportRequest {
        publications_export: serde_json::to_vec(&export.publications).unwrap(),
        covers_dir: covers.clone(),
        media_root: media.clone(),
    };
    (covers, media, request)
}

async fn apply_previewed(db: &Database, request: &CoverImportRequest) {
    let preview = preview_cover_import(db, request).await.unwrap();
    apply_cover_import(db, request, &preview.preview_hash)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn historical_covers_register_only_real_images_named_by_the_export() {
    let db = imported("ops_read_covers").await;
    let (covers, media, request) = cover_fixture();
    let preview = preview_cover_import(&db, &request).await.unwrap();
    let figures = serde_json::to_value(&preview.figures).unwrap();
    assert_eq!(
        figures,
        json!({"coversInExport": 5, "notInLedger": 1, "missingFiles": 1, "notAnImage": 1,
               "alreadyRegistered": 0, "conflicting": 0, "toRegister": 2, "webp": 1, "jpeg": 1, "png": 0})
    );
    assert!(matches!(
        apply_cover_import(&db, &request, &"0".repeat(64)).await,
        Err(OpsImportError::PreviewHashMismatch)
    ));
    let receipt = apply_cover_import(&db, &request, &preview.preview_hash)
        .await
        .unwrap();
    assert_eq!((receipt.registered, receipt.files_written), (2, 2));
    let pub_one: Uuid = sqlx::query_scalar(
        "SELECT publication_ref FROM linggan_ops_publication WHERE source_record_id = 'pub-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let stored = read_ops_cover(&db, pub_one).await.unwrap().unwrap();
    assert!(
        media.join(&stored.storage_key).is_file(),
        "the file sits under the media root"
    );
    assert_eq!(stored.mime_type, "image/webp");
    let listed = read_ops_publications(&db, OpsBusiness::All, filter())
        .await
        .unwrap();
    let pub_one_item = listed.items.iter().find(|item| item.source_id == "pub-1");
    assert!(pub_one_item.unwrap().cover.available);

    let again = preview_cover_import(&db, &request).await.unwrap();
    assert_eq!(
        (again.figures.to_register, again.figures.already_registered),
        (0, 2)
    );
    let replay = apply_cover_import(&db, &request, &again.preview_hash)
        .await
        .unwrap();
    assert_eq!(
        (replay.registered, replay.import_ref),
        (0, None),
        "nothing is registered twice"
    );
    let _ = (
        std::fs::remove_dir_all(covers),
        std::fs::remove_dir_all(media),
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn registered_covers_cannot_be_rewritten_and_paths_cannot_leave_the_folder() {
    let db = imported("ops_read_cover_guards").await;
    let (covers, media, request) = cover_fixture();
    apply_previewed(&db, &request).await;
    std::fs::write(
        covers.join(COVER_BASE).join("covers/one.webp"),
        b"RIFF\x10\0\0\0WEBPVP8 changed!",
    )
    .unwrap();
    assert_eq!(
        preview_cover_import(&db, &request)
            .await
            .unwrap()
            .figures
            .conflicting,
        1,
        "a changed file is a conflict"
    );
    for statement in [
        "UPDATE linggan_ops_publication_cover SET byte_size = 1",
        "DELETE FROM linggan_ops_publication_cover",
    ] {
        assert!(
            sqlx::query(statement).execute(db.pool()).await.is_err(),
            "{statement} must be refused"
        );
    }
    let mut escape = first_export();
    with_cover(&mut escape, "pub-1", "workspaces/../../etc/passwd");
    let escaping = CoverImportRequest {
        publications_export: serde_json::to_vec(&escape.publications).unwrap(),
        covers_dir: covers.clone(),
        media_root: media.clone(),
    };
    assert!(matches!(
        preview_cover_import(&db, &escaping).await,
        Err(OpsImportError::InvalidExport { .. })
    ));
    let _ = (
        std::fs::remove_dir_all(covers),
        std::fs::remove_dir_all(media),
    );
}
