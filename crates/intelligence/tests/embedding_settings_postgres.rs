//! Embedding settings exercised through the public domain functions and the actual local Node transport.
//! Every transmitted sentence is a labelled synthetic fixture; production credentials are never loaded.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use linggan_intelligence::{
    embedding_settings::*, model_secrets::*, model_settings::*, pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    process::Stdio,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;
async fn server() -> (tokio::process::Child, String) {
    let mut child = tokio::process::Command::new(
        std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
            .join(".nvm/versions/node/v24.13.0/bin/node"),
    )
    .arg(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/test/fixture-server.mjs"),
    )
    .env_clear()
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true)
    .spawn()
    .unwrap();
    let mut url = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut url)
        .await
        .unwrap();
    (child, url.trim().to_owned())
}
fn requests(url: &str) -> Value {
    let url = url::Url::parse(url).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    socket.write_all(b"GET /v1/synthetic-embedding-requests HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer SYNTHETIC-NOT-A-CREDENTIAL\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    serde_json::from_str(response.split_once("\r\n\r\n").unwrap().1).unwrap()
}
async fn model(db: &Database, url: &str, id: &str) -> Uuid {
    let connection = SaveModelConnection {
        connection_ref: Uuid::new_v4(),
        version_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "合成向量供应商".into(),
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
            model_id: id.into(),
        },
    )
    .await
    .unwrap();
    model_ref
}
fn assert_public(value: &Value, url: &str) {
    let serialized = value.to_string();
    for forbidden in [
        "secretRef",
        "baseUrl",
        "apiKey",
        "SYNTHETIC-NOT-A-CREDENTIAL",
        url,
    ] {
        assert!(
            !serialized.contains(forbidden),
            "settings DTO must not expose {forbidden}"
        );
    }
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and actual local synthetic embedding transport"]
async fn saving_is_local_probe_is_one_synthetic_sentence_and_enable_requires_qualification() {
    let db = fixture::proof_database("embedding_settings_lifecycle").await;
    let (mut child, url) = server().await;
    let model = model(&db, &url, "synthetic-good").await;
    let initial = read(&db).await.unwrap();
    assert_eq!(initial["revision"], 0);
    assert!(initial["configRef"].is_null());
    assert_public(&initial, &url);
    assert!(matches!(
        save(
            &db,
            &SaveEmbedding {
                expected_revision: 0,
                model_ref: model,
                enabled: true
            }
        )
        .await,
        Err(ModelError::EmbeddingNotQualified)
    ));
    let saved = save(
        &db,
        &SaveEmbedding {
            expected_revision: 0,
            model_ref: model,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved["qualified"], false);
    assert_eq!(saved["enabled"], false);
    assert_eq!(saved["revision"], 1);
    assert_public(&saved, &url);
    assert_eq!(requests(&url), json!([]));
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(invocations, 0);
    assert!(active_config(&db).await.unwrap().is_none());
    let config = Uuid::parse_str(saved["configRef"].as_str().unwrap()).unwrap();
    let request = ProbeEmbedding {
        invocation_ref: Uuid::new_v4(),
        config_ref: config,
    };
    let result = probe(
        &db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &request,
    )
    .await
    .unwrap();
    assert_eq!(result["embeddingQualified"], true);
    assert_eq!(result["dimensions"], 2);
    assert_public(&result, &url);
    let outgoing = requests(&url);
    assert_eq!(outgoing.as_array().unwrap().len(), 1);
    assert_eq!(
        outgoing[0]["input"],
        json!(["SYNTHETIC / NOT EVIDENCE：孩子写作业启动困难"])
    );
    assert_eq!(outgoing[0]["model"], "synthetic-good");
    assert!(
        active_config(&db).await.unwrap().is_none(),
        "probe cannot implicitly enable research"
    );
    let replay = probe(
        &db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &request,
    )
    .await
    .unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["result"]["embeddingQualified"], true);
    assert_public(&replay, &url);
    assert_eq!(requests(&url).as_array().unwrap().len(), 1);
    let enabled = save(
        &db,
        &SaveEmbedding {
            expected_revision: 1,
            model_ref: model,
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(enabled["enabled"], true);
    assert_eq!(enabled["qualified"], true);
    assert_public(&enabled, &url);
    assert!(active_config(&db).await.unwrap().is_some());
    assert!(matches!(
        save(
            &db,
            &SaveEmbedding {
                expected_revision: 1,
                model_ref: model,
                enabled: false
            }
        )
        .await,
        Err(ModelError::Conflict)
    ));
    let disabled = save(
        &db,
        &SaveEmbedding {
            expected_revision: 2,
            model_ref: model,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(disabled["enabled"], false);
    assert!(active_config(&db).await.unwrap().is_none());
    assert_eq!(requests(&url).as_array().unwrap().len(), 1);
    let receipt:Value=sqlx::query_scalar("SELECT jsonb_build_object('state',state,'inputTokens',input_tokens,'outputTokens',output_tokens,'chargedTokens',charged_tokens) FROM linggan_model_invocation WHERE invocation_ref=$1").bind(request.invocation_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(receipt["state"], "succeeded");
    assert_eq!(receipt["inputTokens"], 30);
    assert_eq!(receipt["outputTokens"], 0);
    assert_eq!(receipt["chargedTokens"], 30);
    child.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and actual local synthetic embedding transport"]
async fn zero_vector_never_qualifies_or_enables_and_cannot_reuse_another_probe_identity() {
    let db = fixture::proof_database("embedding_settings_bad_vector").await;
    let (mut child, url) = server().await;
    let bad = model(&db, &url, "synthetic-bad-vector").await;
    let saved = save(
        &db,
        &SaveEmbedding {
            expected_revision: 0,
            model_ref: bad,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let config = Uuid::parse_str(saved["configRef"].as_str().unwrap()).unwrap();
    let request = ProbeEmbedding {
        invocation_ref: Uuid::new_v4(),
        config_ref: config,
    };
    let result = probe(
        &db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &request,
    )
    .await
    .unwrap();
    assert_eq!(result["embeddingQualified"], false);
    assert!(result["dimensions"].is_null());
    assert_public(&result, &url);
    let state = read(&db).await.unwrap();
    assert_eq!(state["qualified"], false);
    assert_eq!(state["enabled"], false);
    assert!(active_config(&db).await.unwrap().is_none());
    assert!(matches!(
        save(
            &db,
            &SaveEmbedding {
                expected_revision: 1,
                model_ref: bad,
                enabled: true
            }
        )
        .await,
        Err(ModelError::EmbeddingNotQualified)
    ));
    let good = model(&db, &url, "synthetic-good").await;
    let other = save(
        &db,
        &SaveEmbedding {
            expected_revision: 1,
            model_ref: good,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let conflicting = ProbeEmbedding {
        invocation_ref: request.invocation_ref,
        config_ref: Uuid::parse_str(other["configRef"].as_str().unwrap()).unwrap(),
    };
    assert!(matches!(
        probe(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            &conflicting
        )
        .await,
        Err(ModelError::Conflict)
    ));
    assert_eq!(requests(&url).as_array().unwrap().len(), 1);
    child.kill().await.unwrap();
}
