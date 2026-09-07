#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use linggan_intelligence::{
    model_invocation::*, model_secrets::*, model_settings::*, pi_adapter::*,
};
use serde_json::json;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn truncated_probe_is_callable_but_not_qualified_and_keeps_usage() {
    let db = fixture::proof_database("model_probe_output_limit").await;
    let node = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join(".nvm/versions/node/v24.13.0/bin/node");
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/pi-adapter/test/fixture-server.mjs");
    let mut server = tokio::process::Command::new(node)
        .arg(script)
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut url = String::new();
    BufReader::new(server.stdout.take().unwrap())
        .read_line(&mut url)
        .await
        .unwrap();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let connection: SaveModelConnection = serde_json::from_value(json!({
        "connectionRef":Uuid::new_v4(),"versionRef":version,"expectedRevision":0,"name":"Synthetic limit proof",
        "api":"openai-completions","baseUrl":url.trim(),"localEndpoint":true,"apiKey":"SYNTHETIC-NOT-A-CREDENTIAL"
    })).unwrap();
    save_model_connection(&db, &SyntheticModelSecrets, &connection)
        .await
        .unwrap();
    save_model_entry(
        &db,
        &SaveModelEntry {
            model_ref: model,
            connection_version_ref: version,
            model_id: "synthetic-limit".into(),
        },
    )
    .await
    .unwrap();
    let request = ProbeModel {
        invocation_ref: Uuid::new_v4(),
        connection_version_ref: version,
        model_ref: Some(model),
        operation: "probe".into(),
    };
    let result = probe_model(
        &db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &request,
    )
    .await
    .unwrap();
    assert_eq!(result["modelCallable"], true);
    assert_eq!(result["commentQualified"], false);
    assert_eq!(result["ok"], false);
    assert_eq!(result["failureCode"], "output_limit");
    assert_eq!(result["usage"]["outputTokens"], 1024);
    let replay = probe_model(
        &db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &request,
    )
    .await
    .unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"], result);
    let config: SaveModelConfig = serde_json::from_value(json!({
        "configRef":Uuid::new_v4(),"expectedConfigRef":null,"modelRef":model,"inputTokenLimit":16000,"outputTokenLimit":2000,
        "timeoutSeconds":30,"maxAttempts":2,"autoSourceLimit":20,"autoTokenLimit":100000
    })).unwrap();
    assert!(matches!(
        save_model_config(&db, &config).await,
        Err(ModelError::NotQualified)
    ));
    let rows: (i64, i64) =
        sqlx::query_as("SELECT count(*),sum(charged_tokens)::bigint FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(rows, (1, 1824));
}
