#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_evidence::comment_research_read::*;
use linggan_intelligence::{
    comment_daily::*, comment_research_projection::*, model_invocation::*, model_secrets::*,
    model_settings::*, pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;
async fn fixture_server() -> (tokio::process::Child, String) {
    let node = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join(".nvm/versions/node/v24.13.0/bin/node");
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/pi-adapter/test/fixture-server.mjs");
    let mut child = tokio::process::Command::new(node)
        .arg(script)
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .await
        .unwrap();
    (child, line.trim().into())
}
async fn configured(
    db: &Database,
    url: &str,
    model_id: &str,
    expected: Option<Uuid>,
) -> (Uuid, Uuid, Uuid) {
    let connection = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "合成模型连接".into(),
        api: "openai-completions".into(),
        base_url: url.into(),
        local_endpoint: true,
        api_key: "SYNTHETIC-NOT-A-CREDENTIAL".into(),
    };
    save_model_connection(db, &SyntheticModelSecrets, &connection)
        .await
        .unwrap();
    let model_ref = Uuid::new_v4();
    save_model_entry(
        db,
        &SaveModelEntry {
            model_ref,
            connection_version_ref: connection.version_ref,
            model_id: model_id.into(),
        },
    )
    .await
    .unwrap();
    let probe = probe_model(
        db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &ProbeModel {
            invocation_ref: Uuid::new_v4(),
            connection_version_ref: connection.version_ref,
            model_ref: Some(model_ref),
            operation: "probe".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(probe["commentQualified"], true);
    let config = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: expected,
        model_ref,
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 5,
        max_attempts: 2,
        auto_source_limit: 10,
        auto_token_limit: 100000,
    };
    save_model_config(db, &config).await.unwrap();
    (
        config.config_ref,
        connection.connection_ref,
        connection.version_ref,
    )
}

async fn source(db: &Database, note: &str, id: &str, body: &str) -> Uuid {
    research_fixture::comment(db, note, id, body, "2026-09-06T01:00:00Z").await
}
async fn clock(db: &Database, time: &str) {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS daily_test_clock(singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),now_at timestamptz NOT NULL); CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT COALESCE((SELECT now_at FROM daily_test_clock WHERE singleton),clock_timestamp()) $$").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO daily_test_clock(singleton,now_at) VALUES(true,$1::timestamptz) ON CONFLICT(singleton) DO UPDATE SET now_at=EXCLUDED.now_at").bind(time).execute(db.pool()).await.unwrap();
}
async fn selected(db: &Database, config: Uuid, refs: Vec<Uuid>, tokens: i64) -> Uuid {
    let id = Uuid::new_v4();
    create_selected(
        db,
        &SelectedBatch {
            batch_ref: id,
            config_ref: config,
            source_refs: refs,
            token_limit: tokens,
        },
    )
    .await
    .unwrap();
    id
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn work_packet_is_one_call_with_exact_results_and_restriction_propagation() {
    let db = fixture::proof_database("daily_work_packet").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    research_fixture::detail(&db, "work-one", "合成作品").await;
    let a = source(&db, "work-one", "a", "没有效果  ，我每天提醒也没有用").await;
    let b = source(&db, "work-one", "b", "我尝试过陪写，还是每天很晚").await;
    let low = source(&db, "work-one", "low", "😭😭😭").await;
    let batch = selected(&db, config, vec![a, b, low], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    let data = batch_detail(&db, batch, None).await.unwrap();
    assert_eq!(data["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "succeeded")
            .count(),
        2
    );
    assert_eq!(
        data["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "low_information")
            .count(),
        1
    );
    let annotations = read_comment_research_annotations(&db, a).await.unwrap();
    assert_eq!(annotations["analysis"][0]["state"], "succeeded");
    assert_eq!(
        annotations["analysis"][0]["result"]["modelVersion"],
        annotations["analysis"][0]["modelVersion"]
    );
    restrict_comment_research_source(&db, b, "synthetic restriction")
        .await
        .unwrap();
    assert!(
        read_comment_research_annotations(&db, a).await.unwrap()["analysis"][0]["result"].is_null()
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn daily_windows_are_gapless_deduplicate_reobservations_and_allow_delayed_cleaning() {
    let db = fixture::proof_database("daily_windows").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-07 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
        },
    )
    .await
    .unwrap();
    let first = source(&db, "first", "same", "今天首次入库的评论").await;
    clock(&db, "2026-09-07 15:00:00Z").await;
    let late = source(&db, "late", "late", "23点整入库，进入下批").await;
    assert!(seal_due(&db).await.unwrap());
    assert!(!seal_due(&db).await.unwrap());
    let rows = sqlx::query("SELECT source_ref FROM linggan_comment_daily_item")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<Uuid, _>("source_ref"), first);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_clean")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    clock(&db, "2026-09-08 14:00:00Z").await;
    source(&db, "first", "same", "今天首次入库的评论").await;
    clock(&db, "2026-09-09 16:00:00Z").await;
    assert!(seal_due(&db).await.unwrap());
    assert!(seal_due(&db).await.unwrap());
    assert!(!seal_due(&db).await.unwrap());
    let refs: Vec<Uuid> =
        sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item ORDER BY source_ref")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(refs.len(), 2);
    assert!(refs.contains(&late));
    clean_pending(&db).await.unwrap();
    let n:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_item i JOIN linggan_comment_clean c USING(source_ref)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(n, 2);
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn selection_replay_pause_and_budget_do_not_duplicate_dispatch() {
    let db = fixture::proof_database("daily_budget").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "a", "a", "一条需要研究的评论").await;
    let b = source(&db, "b", "b", "另一篇作品的评论").await;
    let id = Uuid::new_v4();
    let request = SelectedBatch {
        batch_ref: id,
        config_ref: config,
        source_refs: vec![a, b],
        token_limit: 18000,
    };
    create_selected(&db, &request).await.unwrap();
    assert_eq!(
        create_selected(&db, &request).await.unwrap()["replayed"],
        true
    );
    set_batch_enabled(&db, id, false).await.unwrap();
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    set_batch_enabled(&db, id, true).await.unwrap();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let detail = batch_detail(&db, id, None).await.unwrap();
    assert_eq!(
        detail["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["state"] == "pending")
            .count(),
        1
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn comment_facts_keep_zero_and_unknown_separate() {
    let db = fixture::proof_database("daily_comment_facts").await;
    let package=fixture::submit_package(&db,"comments",json!({"contentExternalId":"facts"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"facts"},"payload":{"commentId":"zero","noteId":"facts","text":"零点赞的完整表达","likes":0,"publishedAt":1788750000000_i64}})).await;
    let id: Uuid = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1",
    )
    .bind(package)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let s = read_comment_research_source(&db, id).await.unwrap();
    assert_eq!(s.likes, Some(0));
    assert_eq!(s.published_at, Some(1788750000000));
    assert!(s.accepted_at.is_some());
    let unknown = source(&db, "facts", "unknown", "没有附带点赞的表达").await;
    assert_eq!(
        read_comment_research_source(&db, unknown)
            .await
            .unwrap()
            .likes,
        None
    );
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn partial_packet_keeps_good_comments_and_concurrent_ticks_dispatch_once() {
    let db = fixture::proof_database("daily_partial").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "partial", "a", "这条评论有完整的表达").await;
    let b = source(&db, "partial", "b", "[BAD_QUOTE] 不接受虚构引用").await;
    let c = source(&db, "partial", "c", "[MISSING] 模型没有回答此条").await;
    let d = source(&db, "partial", "d", "[NO_SIGNAL] 这条没有信号").await;
    let batch = selected(&db, config, vec![a, b, c, d], 100000).await;
    let adapter = PiAdapter::configured();
    let (x, y) = tokio::join!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter),
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
    );
    assert_ne!(x.unwrap(), y.unwrap());
    let data = batch_detail(&db, batch, None).await.unwrap();
    let items = data["items"].as_array().unwrap();
    assert_eq!(items.iter().filter(|i| i["state"] == "failed").count(), 2);
    assert_eq!(
        items.iter().filter(|i| i["state"] == "succeeded").count(),
        1
    );
    assert_eq!(
        items.iter().filter(|i| i["state"] == "no_signal").count(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1"
        )
        .bind(batch)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn interrupted_packet_keeps_unknown_reservation_and_does_not_silently_rebill() {
    let db = fixture::proof_database("daily_interrupted").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "interrupt", "a", "[HANG_PROVIDER] 模拟进程中断").await;
    let batch = selected(&db, config, vec![a], 100000).await;
    let db2 = db.clone();
    let handle = tokio::spawn(async move {
        run_daily_once(&db2, &SyntheticModelSecrets, &PiAdapter::configured()).await
    });
    for _ in 0..100 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_packet WHERE batch_ref=$1)",
        )
        .bind(batch)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if exists {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    handle.abort();
    let _ = handle.await;
    sqlx::query("UPDATE linggan_comment_daily_packet SET lease_until=scope_001_now()-interval '1 second' WHERE batch_ref=$1").bind(batch).execute(db.pool()).await.unwrap();
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let row=sqlx::query("SELECT v.charged_tokens,v.input_tokens,v.failure_code FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i64, _>("charged_tokens"), 18000);
    assert!(row.get::<Option<i64>, _>("input_tokens").is_none());
    assert_eq!(row.get::<String, _>("failure_code"), "worker_interrupted");
    let command = Uuid::new_v4();
    assert_eq!(
        retry_failed(&db, batch, command).await.unwrap()["queued"],
        1
    );
    assert_eq!(
        retry_failed(&db, batch, command).await.unwrap()["queued"],
        1
    );
    let charged:i64=sqlx::query_scalar("SELECT sum(v.charged_tokens)::bigint FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(charged, 18000);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn cleaning_filter_searches_all_sources_and_binds_cursor() {
    let db = fixture::proof_database("daily_filter").await;
    for i in 0..25 {
        source(&db, "filter", &format!("low-{i}"), "😭😭😭").await;
    }
    source(&db, "filter", "normal", "详细描述一个具体问题").await;
    clean_pending(&db).await.unwrap();
    let members = cleaning_members(&db, "low_information").await.unwrap();
    let page = read_comment_research_subset(
        &db,
        &CommentResearchQuery::default(),
        Some(&members),
        "low_information",
    )
    .await
    .unwrap();
    assert_eq!(page.total, 25);
    assert_eq!(page.items.len(), 20);
    let q = CommentResearchQuery {
        cursor: page.next_cursor,
        ..Default::default()
    };
    assert_eq!(
        read_comment_research_subset(&db, &q, Some(&members), "low_information")
            .await
            .unwrap()
            .items
            .len(),
        5
    );
    assert!(
        read_comment_research_subset(&db, &q, Some(&members), "direct")
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn daily_permission_cannot_be_duplicated_by_legacy_auto_and_pauses_without_connection() {
    use linggan_intelligence::model_plans::{StartModelPlan, start_model_plan};
    let db = fixture::proof_database("daily_permissions").await;
    let (mut server, url) = fixture_server().await;
    let (config, connection, _) = configured(&db, &url, "synthetic-good", None).await;
    let mut schedule = DailySchedule {
        expected_revision: 0,
        enabled: true,
        config_ref: config,
        source_limit: 10,
        token_limit: 100000,
    };
    save_schedule(&db, &schedule).await.unwrap();
    let automatic = StartModelPlan {
        plan_ref: Uuid::new_v4(),
        config_ref: config,
        kind: "automatic".into(),
        source_refs: vec![],
        source_limit: 10,
        token_limit: 100000,
        expected_auto_plan_ref: None,
    };
    assert!(matches!(
        start_model_plan(&db, &automatic).await,
        Err(ModelError::Disabled)
    ));
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    schedule.expected_revision = 1;
    schedule.enabled = false;
    save_schedule(&db, &schedule).await.unwrap();
    assert_eq!(overview(&db).await.unwrap()["schedule"]["enabled"], false);
    server.kill().await.unwrap();
}
