#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_daily::{
        AutoPolicy, DailySchedule, SelectedBatch, create_selected, run_daily_once, save_schedule,
    },
    model_invocation::{ProbeModel, probe_model},
    model_secrets::SyntheticModelSecrets,
    model_settings::{
        SaveModelConfig, SaveModelConnection, SaveModelEntry, save_model_config,
        save_model_connection, save_model_entry,
    },
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
use daily_fixture::*;

const NEW_SOURCE_COUNT: usize = 3_000;
const PACKAGE_SOURCE_LIMIT: usize = 2_048;
const DAILY_SOURCE_LIMIT: i32 = 2;
const DAILY_TOKEN_LIMIT: i64 = 36_000;

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi; A01 3,000-source automatic boundary"]
async fn automatic_intake_at_3000_keeps_overage_and_pre_enable_history_separate() {
    let db = fixture::proof_database("comment_auto_scale").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let history = submit_pre_enable_history(&db).await;
    configure_automatic_schedule(&db, config).await;

    clock(&db, "2026-09-08 14:59:59Z").await;
    let package_refs = submit_scale_intake(&db).await;
    assert_authoritative_intake(&db, &package_refs).await;

    clock(&db, "2026-09-08 15:00:00Z").await;
    let adapter = PiAdapter::configured();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );

    let daily = daily_batch(&db).await;
    assert_new_manifest_and_queue(&db, daily).await;
    assert_bounded_loopback_execution(&db, daily).await;
    assert_history_stays_separate(&db, daily, &history).await;
    server.kill().await.unwrap();
}

async fn submit_pre_enable_history(db: &linggan_storage_postgres::Database) -> Vec<Uuid> {
    clock(db, "2026-09-01 02:00:00Z").await;
    vec![
        source(
            db,
            "scale-history-one",
            "old-1",
            "启用前留下的完整历史表达，需要和新入库分开排队。",
        )
        .await,
        source(
            db,
            "scale-history-two",
            "old-2",
            "这是另一条启用前历史表达，不能因为新样本过量而被当成新增。",
        )
        .await,
    ]
}

async fn configure_automatic_schedule(db: &linggan_storage_postgres::Database, config: Uuid) {
    clock(db, "2026-09-08 02:00:00Z").await;
    save_schedule(
        db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: DAILY_SOURCE_LIMIT,
            token_limit: DAILY_TOKEN_LIMIT,
            auto_policy: AutoPolicy {
                continuous_new: true,
                historical_enabled: true,
                history_start: Some("2026-08-01T00:00:00Z".into()),
                day_token_limit: DAILY_TOKEN_LIMIT,
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
}

async fn submit_scale_intake(db: &linggan_storage_postgres::Database) -> Vec<Uuid> {
    vec![
        submit_new_comment_package(db, "scale-new-intake", 0, PACKAGE_SOURCE_LIMIT).await,
        submit_new_comment_package(
            db,
            "scale-new-intake",
            PACKAGE_SOURCE_LIMIT,
            NEW_SOURCE_COUNT - PACKAGE_SOURCE_LIMIT,
        )
        .await,
    ]
}

async fn assert_authoritative_intake(db: &linggan_storage_postgres::Database, packages: &[Uuid]) {
    let admitted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment WHERE package_ref=ANY($1)",
    )
    .bind(packages)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(admitted, NEW_SOURCE_COUNT as i64);
    let acknowledged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_submission_receipt WHERE package_ref=ANY($1) AND material_admission='ACCEPTED'",
    )
    .bind(packages)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(acknowledged, 2, "both packages use authoritative intake");
}

async fn daily_batch(db: &linggan_storage_postgres::Database) -> Uuid {
    sqlx::query_scalar("SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='daily'")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn assert_new_manifest_and_queue(db: &linggan_storage_postgres::Database, daily: Uuid) {
    let request: serde_json::Value =
        sqlx::query_scalar("SELECT request FROM linggan_comment_daily_batch WHERE batch_ref=$1")
            .bind(daily)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(request["newIntakeCount"], json!(NEW_SOURCE_COUNT));

    let states = sqlx::query(
        "SELECT \
           count(*)::bigint AS total, \
           COALESCE(sum(attempts),0)::bigint AS attempts, \
           count(*) FILTER(WHERE state IN('succeeded','no_signal'))::bigint AS completed, \
           count(*) FILTER(WHERE attempts=0 AND state IN('pending','source_limit'))::bigint AS retained, \
           count(*) FILTER(WHERE state='source_limit')::bigint AS source_limited, \
           count(*) FILTER(WHERE state IN('failed','restricted','anomaly','low_information','context_missing'))::bigint AS terminal_failure \
         FROM linggan_comment_daily_item WHERE batch_ref=$1 AND queue_class='new_intake'",
    )
    .bind(daily)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(states.get::<i64, _>("total"), NEW_SOURCE_COUNT as i64);
    assert_eq!(
        states.get::<i64, _>("attempts"),
        i64::from(DAILY_SOURCE_LIMIT)
    );
    assert_eq!(
        states.get::<i64, _>("completed"),
        i64::from(DAILY_SOURCE_LIMIT)
    );
    assert_eq!(
        states.get::<i64, _>("retained"),
        (NEW_SOURCE_COUNT as i64) - i64::from(DAILY_SOURCE_LIMIT)
    );
    assert!(states.get::<i64, _>("source_limited") > 0);
    assert_eq!(states.get::<i64, _>("terminal_failure"), 0);
}

async fn assert_bounded_loopback_execution(db: &linggan_storage_postgres::Database, daily: Uuid) {
    let packets = sqlx::query(
        "SELECT count(*)::bigint AS calls, COALESCE(sum(cardinality(source_refs)),0)::bigint AS sources \
         FROM linggan_comment_daily_packet WHERE batch_ref=$1",
    )
    .bind(daily)
    .fetch_one(db.pool())
    .await
    .unwrap();
    // The seeded active V4 rule emits up to seven comments per packet, so the two allowed
    // sources are one real loopback call. The assertion is about packet membership, not ticks.
    assert_eq!(packets.get::<i64, _>("calls"), 1);
    assert_eq!(
        packets.get::<i64, _>("sources"),
        i64::from(DAILY_SOURCE_LIMIT)
    );
    let charged: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(invocation.charged_tokens),0)::bigint \
         FROM linggan_comment_daily_packet packet \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         WHERE packet.batch_ref=$1",
    )
    .bind(daily)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(charged <= DAILY_TOKEN_LIMIT);
}

async fn assert_history_stays_separate(
    db: &linggan_storage_postgres::Database,
    daily: Uuid,
    history: &[Uuid],
) {
    let selected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_batch WHERE kind='selected'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(selected, 0, "A01 must not need manual selection");
    let historical = sqlx::query(
        "SELECT count(*)::bigint AS total, \
           count(*) FILTER(WHERE queue_class='historical' AND state='pending' AND attempts=0)::bigint AS retained \
         FROM linggan_comment_daily_item item \
         JOIN linggan_comment_daily_batch batch USING(batch_ref) \
         WHERE batch.kind='backlog' AND item.source_ref=ANY($1)",
    )
    .bind(history)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(historical.get::<i64, _>("total"), history.len() as i64);
    assert_eq!(historical.get::<i64, _>("retained"), history.len() as i64);
    let in_new_manifest: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=ANY($2)",
    )
    .bind(daily)
    .bind(history)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(in_new_manifest, 0);
}

async fn submit_new_comment_package(
    db: &linggan_storage_postgres::Database,
    note: &str,
    start: usize,
    count: usize,
) -> Uuid {
    let records = (start..start + count)
        .map(|ordinal| {
            json!({
                "kind":"comment",
                "sourceObject":{"platform":"xhs","type":"content","externalId":note},
                "payload":{
                    "commentId":format!("scale-{ordinal:04}"),
                    "noteId":note,
                    "text":format!("这是自动新增规模边界的完整评论 {ordinal}，我想了解怎样把计划开始并坚持下去。"),
                    "authorId":"synthetic-scale-author"
                }
            })
        })
        .collect();
    fixture::submit_custom_package(
        db,
        "xhs",
        &["comments"],
        json!({"contentExternalId":note}),
        "comments",
        "xhs",
        json!({
            "target":{"contentExternalId":note},
            "layers":[fixture::coverage_layer("comments", count as i64)]
        }),
        records,
    )
    .await
}
