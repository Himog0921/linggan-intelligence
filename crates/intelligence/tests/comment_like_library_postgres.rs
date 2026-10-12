//! COMMENT-LIKE-LIBRARY-001 isolated proofs (Issue #397). Real SQL over synthetic packages only.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package_at};
use linggan_intelligence::comment_study_catalog::{
    CatalogPostUsage, CatalogSort, CatalogSummaryQuery, CommentCatalogQuery, PostUsageAction,
    PostUsageCommand, PostUsageError, read_catalog_summary, read_comment_catalog,
    record_comment_post_usage, refresh_clean_cache,
};
use linggan_storage_postgres::Database;
use research_fixture::detail_with_author;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const BASE: &str = concat!(
    include_str!("../../../database/bootstrap/comment-study-001.sql"),
    "\n",
    include_str!("../../../database/migrations/0114_comment_study_effective_head.sql")
);
const DELTA: &str =
    include_str!("../../../database/migrations/0107_comment_study_productization_schema.sql");
const MIGRATION: &str = include_str!("../../../database/migrations/0122_comment_like_library.sql");
const ADHD_DOMAIN_REF: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);
const OTHER_DOMAIN: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0002);

async fn database(name: &str) -> Database {
    let db = proof_database(name).await;
    sqlx::raw_sql(BASE).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(DELTA).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    db
}

async fn attach(db: &Database, note: &str) {
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage( \
            usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref \
         ) SELECT gen_random_uuid(),public_ref,$2,'primary','legacy_domain_migration',first_package_ref \
             FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1 \
         ON CONFLICT (content_public_ref,domain_ref) \
             WHERE basis_kind='legacy_domain_migration' DO NOTHING",
    )
    .bind(note)
    .bind(ADHD_DOMAIN_REF)
    .execute(db.pool())
    .await
    .unwrap();
}

/// One accepted comment observation through the real admission path.
async fn observe(
    db: &Database,
    note: &str,
    id: &str,
    like_text: Option<&str>,
    likes: Option<Value>,
    observed_at: &str,
) -> Uuid {
    let mut payload = json!({
        "commentId": id, "noteId": note, "authorId": format!("reader-{id}"),
        "text": format!("SYNTHETIC 孩子写作业总是拖到很晚，{id} 仅为测试"),
    });
    if let Some(text) = like_text {
        payload["likeText"] = json!(text);
    }
    if let Some(likes) = likes {
        payload["likes"] = likes;
    }
    let package = submit_package_at(
        db,
        "comments",
        json!({"contentExternalId": note}),
        json!({"kind": "comment",
               "sourceObject": {"platform": "xhs", "type": "content", "externalId": note},
               "payload": payload}),
        observed_at,
    )
    .await;
    attach(db, note).await;
    sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
        .bind(package)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn cache(db: &Database) {
    for _ in 0..4 {
        let report = refresh_clean_cache(db, ADHD_DOMAIN_REF, 200).await.unwrap();
        if report.examined_count == 0 {
            return;
        }
    }
    panic!("synthetic test cache did not finish within four bounded passes");
}

fn query(extra: Value) -> CommentCatalogQuery {
    let mut value = json!({"domain": ADHD_DOMAIN_REF});
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

/// Every page of one query, following the server's own cursor.
async fn all_pages(db: &Database, mut request: CommentCatalogQuery) -> Vec<Value> {
    let mut items = Vec::new();
    for _ in 0..20 {
        let response = read_comment_catalog(db, &request).await.unwrap();
        items.extend(response["items"].as_array().unwrap().iter().cloned());
        match response["page"]["nextCursor"].as_str() {
            Some(next) => request.cursor = Some(next.to_owned()),
            None => return items,
        }
    }
    panic!("catalog paging did not terminate");
}

fn id_of(item: &Value) -> String {
    item["commentKey"]["commentExternalId"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn admission_and_migration_backfill_apply_the_same_like_rule() {
    let db = database("comment_like_rule").await;
    detail_with_author(&db, "likes-rule", "SYNTHETIC 点赞规则", Some("creator")).await;
    let cases: [(&str, Option<&str>, Option<Value>, Option<i64>); 9] = [
        ("zan", Some("赞"), Some(json!(0)), Some(0)),
        ("digits", Some("3"), Some(json!(3)), Some(3)),
        ("wan", Some("3.5万"), Some(json!(35_000)), Some(35_000)),
        ("legacy-empty", None, Some(json!(74)), Some(74)),
        ("default-zero", Some("0"), Some(json!(0)), None),
        ("blank", Some(""), Some(json!(0)), None),
        ("missing", None, None, None),
        ("float", None, Some(json!(12.5)), None),
        ("string", None, Some(json!("12")), None),
    ];
    let mut expected = BTreeMap::new();
    for (id, text, likes, count) in cases {
        let material = observe(&db, "likes-rule", id, text, likes, "2026-10-01T08:00:00Z").await;
        expected.insert(material, (id, count));
    }

    let stored: Vec<(Uuid, Option<i64>, String)> = sqlx::query_as(
        "SELECT material_ref, like_count, like_count_state FROM linggan_material_comment_engagement",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        stored.len(),
        expected.len(),
        "one observation row per comment row"
    );
    for (material, count, state) in &stored {
        let (id, want) = expected[material];
        assert_eq!(*count, want, "{id}");
        assert_eq!(
            state,
            if want.is_some() { "KNOWN" } else { "UNKNOWN" },
            "{id}"
        );
    }

    // The migration's own backfill SELECT, run over the same accepted records, must agree with
    // what admission wrote; otherwise historical and new comments would follow two rules.
    let insert_at = MIGRATION
        .find("INSERT INTO linggan_material_comment_engagement")
        .unwrap();
    let select_at = insert_at
        + MIGRATION[insert_at..]
            .find("SELECT comment.material_ref")
            .unwrap();
    let conflict_at = select_at + MIGRATION[select_at..].find("ON CONFLICT").unwrap();
    let statement_end =
        conflict_at + MIGRATION[conflict_at..].find("DO NOTHING;").unwrap() + "DO NOTHING".len();
    let backfill: Vec<(Uuid, Option<i64>, String)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT material_ref, like_count, like_count_state FROM ({}) \
         AS backfill(material_ref, like_count, like_count_state, like_source_text, parser_version)",
        &MIGRATION[select_at..conflict_at]
    )))
    .fetch_all(db.pool())
    .await
    .unwrap();
    let as_map = |rows: Vec<(Uuid, Option<i64>, String)>| {
        rows.into_iter()
            .map(|(material, count, state)| (material, (count, state)))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(as_map(backfill), as_map(stored));

    let rerun = sqlx::raw_sql(sqlx::AssertSqlSafe(&MIGRATION[insert_at..statement_end]))
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        rerun.rows_affected(),
        0,
        "re-running the backfill adds nothing"
    );
    let update = sqlx::query("UPDATE linggan_material_comment_engagement SET like_count = 1")
        .execute(db.pool())
        .await;
    assert!(update.is_err(), "observations are append-only");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn like_order_pages_through_ties_and_unknown_tail_and_filters_match_the_summary() {
    let db = database("comment_like_order").await;
    detail_with_author(&db, "likes-order", "SYNTHETIC 点赞排序", Some("creator")).await;
    let t1 = "2026-10-01T08:00:00Z";
    let t2 = "2026-10-05T08:00:00Z";
    for (id, text, likes) in [
        ("a", "500", 500),
        ("b", "120", 120),
        ("c", "120", 120),
        ("d", "30", 30),
        ("e", "赞", 0),
        ("f", "0", 0),
    ] {
        observe(&db, "likes-order", id, Some(text), Some(json!(likes)), t1).await;
    }
    observe(&db, "likes-order", "g", None, None, t1).await;
    // A newer observation that could not read the count must not hide the older known count.
    observe(&db, "likes-order", "h", Some("120"), Some(json!(120)), t1).await;
    observe(&db, "likes-order", "h", Some("0"), Some(json!(0)), t2).await;
    // A newer known count replaces an older one.
    observe(&db, "likes-order", "i", Some("10"), Some(json!(10)), t1).await;
    observe(&db, "likes-order", "i", Some("900"), Some(json!(900)), t2).await;
    cache(&db).await;

    let ordered = all_pages(&db, query(json!({"sort": "like_desc", "limit": 2}))).await;
    let ids: Vec<String> = ordered.iter().map(id_of).collect();
    assert_eq!(
        ids.iter().cloned().collect::<BTreeSet<_>>(),
        ["a", "b", "c", "d", "e", "f", "g", "h", "i"]
            .map(str::to_owned)
            .into_iter()
            .collect(),
        "every comment exactly once: {ids:?}"
    );
    assert_eq!(ids.len(), 9, "no duplicates across pages: {ids:?}");
    let counts: Vec<Option<i64>> = ordered
        .iter()
        .map(|item| item["likeCount"].as_i64())
        .collect();
    let known: Vec<i64> = counts.iter().map_while(|count| *count).collect();
    assert_eq!(known, vec![900, 500, 120, 120, 120, 30, 0], "{ids:?}");
    assert!(
        counts[known.len()..].iter().all(Option::is_none),
        "unknown last"
    );
    assert_eq!(
        ids[known.len()..].iter().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["f".to_owned(), "g".to_owned()])
    );

    let by_id: BTreeMap<String, &Value> = ordered.iter().map(|item| (id_of(item), item)).collect();
    assert_eq!(by_id["h"]["likeCount"], json!(120));
    assert_eq!(
        by_id["h"]["likeCountAsOf"],
        json!("2026-10-01T08:00:00.000000Z")
    );
    assert_eq!(by_id["i"]["likeCount"], json!(900));
    assert_eq!(
        by_id["i"]["likeCountAsOf"],
        json!("2026-10-05T08:00:00.000000Z")
    );
    assert_eq!(by_id["f"]["likeCountAsOf"], Value::Null);
    assert_eq!(
        by_id["e"]["likeCount"],
        json!(0),
        "a known zero is not unknown"
    );

    let filtered = all_pages(
        &db,
        query(json!({"sort": "like_desc", "minLikes": 100, "limit": 2})),
    )
    .await;
    assert_eq!(
        filtered.iter().map(id_of).collect::<BTreeSet<_>>(),
        ["a", "b", "c", "h", "i"]
            .map(str::to_owned)
            .into_iter()
            .collect()
    );
    let summary = read_catalog_summary(
        &db,
        &serde_json::from_value::<CatalogSummaryQuery>(
            json!({"domain": ADHD_DOMAIN_REF, "minLikes": 100}),
        )
        .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(summary["summary"]["displayableCommentCount"], json!(5));

    let received = all_pages(&db, query(json!({"limit": 4}))).await;
    assert_eq!(
        received.len(),
        9,
        "the default order still lists everything"
    );

    // A cursor belongs to its own order and filters.
    let first = read_comment_catalog(&db, &query(json!({"sort": "like_desc", "limit": 2})))
        .await
        .unwrap();
    let mut reused = query(json!({"limit": 2}));
    reused.cursor = Some(first["page"]["nextCursor"].as_str().unwrap().to_owned());
    assert!(read_comment_catalog(&db, &reused).await.is_err());
}

fn command(work: Uuid, id: &str, action: PostUsageAction, request: Uuid) -> PostUsageCommand {
    PostUsageCommand {
        domain_ref: ADHD_DOMAIN_REF,
        work_ref: work,
        comment_external_id: id.to_owned(),
        action,
        request_ref: request,
    }
}

async fn event_count(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_comment_post_usage_event")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn post_usage_marks_are_idempotent_domain_scoped_and_filterable() {
    let db = database("comment_post_usage").await;
    detail_with_author(&db, "usage-note", "SYNTHETIC 改帖标记", Some("creator")).await;
    observe(
        &db,
        "usage-note",
        "u1",
        Some("10"),
        Some(json!(10)),
        "2026-10-01T08:00:00Z",
    )
    .await;
    observe(
        &db,
        "usage-note",
        "u2",
        Some("20"),
        Some(json!(20)),
        "2026-10-01T08:00:00Z",
    )
    .await;
    cache(&db).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='usage-note'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let (r1, r2, r3, r4) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );

    let marked = record_comment_post_usage(&db, &command(work, "u1", PostUsageAction::Mark, r1))
        .await
        .unwrap();
    assert_eq!(marked["outcome"], json!("applied"));
    assert_eq!(marked["postUsage"]["state"], json!("used"));
    assert!(marked["postUsage"]["markedAt"].is_string());

    let again = record_comment_post_usage(&db, &command(work, "u1", PostUsageAction::Mark, r2))
        .await
        .unwrap();
    assert_eq!(again["outcome"], json!("unchanged"));
    assert_eq!(event_count(&db).await, 1, "no event without a state change");

    let replay = record_comment_post_usage(&db, &command(work, "u1", PostUsageAction::Mark, r1))
        .await
        .unwrap();
    assert_eq!(replay["outcome"], json!("replayed"));
    assert_eq!(replay["postUsage"]["state"], json!("used"));

    let conflict =
        record_comment_post_usage(&db, &command(work, "u1", PostUsageAction::Unmark, r1)).await;
    assert!(matches!(conflict, Err(PostUsageError::IdempotencyConflict)));

    let unmarked =
        record_comment_post_usage(&db, &command(work, "u1", PostUsageAction::Unmark, r3))
            .await
            .unwrap();
    assert_eq!(unmarked["outcome"], json!("applied"));
    assert_eq!(
        unmarked["postUsage"],
        json!({"state": "unused", "markedAt": null})
    );
    record_comment_post_usage(&db, &command(work, "u2", PostUsageAction::Mark, r4))
        .await
        .unwrap();
    assert_eq!(event_count(&db).await, 3);

    let used = all_pages(&db, query(json!({"postUsage": "used"}))).await;
    assert_eq!(
        used.iter().map(id_of).collect::<Vec<_>>(),
        vec!["u2".to_owned()]
    );
    assert_eq!(used[0]["postUsage"]["state"], json!("used"));
    let unused = all_pages(&db, query(json!({"postUsage": "unused"}))).await;
    assert_eq!(
        unused.iter().map(id_of).collect::<Vec<_>>(),
        vec!["u1".to_owned()]
    );
    let mut by_filter = query(json!({}));
    by_filter.post_usage = CatalogPostUsage::Used;
    by_filter.sort = CatalogSort::LikeDesc;
    assert_eq!(all_pages(&db, by_filter).await.len(), 1);

    let mut elsewhere = command(work, "u2", PostUsageAction::Unmark, Uuid::new_v4());
    elsewhere.domain_ref = OTHER_DOMAIN;
    assert!(matches!(
        record_comment_post_usage(&db, &elsewhere).await,
        Err(PostUsageError::CommentNotFound)
    ));
    assert!(matches!(
        record_comment_post_usage(
            &db,
            &command(work, "absent", PostUsageAction::Mark, Uuid::new_v4())
        )
        .await,
        Err(PostUsageError::CommentNotFound)
    ));
    assert_eq!(event_count(&db).await, 3, "rejected commands store nothing");
    let tamper = sqlx::query("DELETE FROM linggan_comment_post_usage_event")
        .execute(db.pool())
        .await;
    assert!(tamper.is_err(), "marks are append-only");
}
