//! Shared synthetic provider and frozen daily-grant fixture setup.
use super::*;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
pub(super) async fn fixture_server() -> (tokio::process::Child, String) {
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
pub(super) async fn configured(
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

pub(super) async fn source(db: &Database, note: &str, id: &str, body: &str) -> Uuid {
    research_fixture::comment(db, note, id, body, "2026-09-06T01:00:00Z").await
}
pub(super) async fn clock(db: &Database, time: &str) {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS daily_test_clock(singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),now_at timestamptz NOT NULL); CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT COALESCE((SELECT now_at FROM daily_test_clock WHERE singleton),clock_timestamp()) $$").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO daily_test_clock(singleton,now_at) VALUES(true,$1::timestamptz) ON CONFLICT(singleton) DO UPDATE SET now_at=EXCLUDED.now_at").bind(time).execute(db.pool()).await.unwrap();
}
pub(super) async fn selected(db: &Database, config: Uuid, refs: Vec<Uuid>, tokens: i64) -> Uuid {
    let id = Uuid::new_v4();
    create_selected(
        db,
        &SelectedBatch {
            reanalyze: false,
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

pub(super) async fn tick(db: &Database) -> bool {
    run_daily_once(db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap()
}
