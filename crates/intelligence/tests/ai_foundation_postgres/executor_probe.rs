//! Actual DB claim -> fixed Node -> SDK -> owned TCP -> fenced typed settlement.
//! Every secret and response is synthetic; no runtime DSN/store/provider is loaded.
use super::{batches, endpoint, fixture, new_connection};
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        batch_store::{
            BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation, ProbeProfile,
            create_batch,
        },
        capability::{Capability, ModelLimits},
        control_store::{Actor, ChangeControl, ControlScope, change_control},
        executor::{
            ExecutorError, ExecutorSession,
            probe::execute_probe,
            queue::{claim_next, recover_expired},
        },
        provider_store::{
            CapabilitySource, MaterialClass, OutputMode, SaveCapability, save_capability,
        },
        store::FoundationError,
    },
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use uuid::Uuid;

#[derive(Clone)]
struct Secrets {
    current: Uuid,
    reads: Arc<Mutex<Vec<Uuid>>>,
}
impl ModelSecretStore for Secrets {
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn get(&self, _: Uuid, reference: Uuid) -> Result<String, ModelError> {
        self.reads.lock().unwrap().push(reference);
        if reference != self.current {
            return Err(ModelError::SecretUnavailable);
        }
        Ok("SYNTHETIC-ROTATED-NOT-A-CREDENTIAL".into())
    }
    fn is_synthetic(&self) -> bool {
        true
    }
}
fn adapter() -> PiAdapter {
    PiAdapter::configured_with_test_command(
        PathBuf::from(
            std::env::var_os("AI_FOUNDATION_PROOF_NODE").expect("runner supplies explicit Node"),
        ),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    )
}
async fn version(db: &Database, connection: Uuid, revision: i32, url: &str, secret: Uuid) -> Uuid {
    let reference = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,$3,'Synthetic owned endpoint','openai-completions',$4,true,$5)")
        .bind(reference).bind(connection).bind(revision).bind(url).bind(secret).execute(db.pool()).await.unwrap();
    reference
}
async fn queued(db: &Database, url: &str, operation: ProbeOperation) -> (Uuid, Uuid, Secrets) {
    queued_profile(db, url, operation, false).await
}
async fn queued_profile(
    db: &Database,
    url: &str,
    operation: ProbeOperation,
    native: bool,
) -> (Uuid, Uuid, Secrets) {
    let config = batches::policy(db).await;
    let connection = new_connection(db).await;
    let secret = Uuid::new_v4();
    let version = version(db, connection, 1, url, secret).await;
    let model = if operation == ProbeOperation::Probe {
        let reference = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'unlisted-synthetic-model','manual')")
            .bind(reference).bind(version).execute(db.pool()).await.unwrap();
        Some(reference)
    } else {
        None
    };
    let profile = if native {
        let identity:Uuid=sqlx::query_scalar("SELECT identity_ref FROM linggan_ai_model_identity WHERE connection_ref=$1 AND model_id='unlisted-synthetic-model'")
            .bind(connection).fetch_one(db.pool()).await.unwrap();
        let capability = save_capability(
            db,
            &native_capability(identity, endpoint(db, version).await),
        )
        .await
        .unwrap();
        Some(ProbeProfile {
            capability_ref: capability.revision_ref,
            limits: ModelLimits {
                input_tokens: 2048,
                output_tokens: 1024,
                timeout_seconds: 10,
                attempts: 1,
                thinking_level: "off".into(),
            },
        })
    } else {
        None
    };
    batches::grant(
        db,
        endpoint(db, version).await,
        TaskKey::ModelProbe,
        if operation == ProbeOperation::Probe {
            MaterialClass::SyntheticProbe
        } else {
            MaterialClass::ModelCatalog
        },
    )
    .await;
    batches::on(db, TaskKey::ModelProbe).await;
    let batch = create_batch(
        db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: config.revision,
            task_config_ref: config.task_config_ref,
            kind: BatchKind::Probe,
            items: vec![BatchInput {
                item_key: "synthetic".into(),
                input: InputRef::Probe {
                    sample_ref: Uuid::new_v4(),
                    connection_version_ref: version,
                    model_ref: model,
                    operation,
                    profile,
                },
            }],
        },
    )
    .await
    .unwrap()
    .batch_ref;
    (
        batch,
        connection,
        Secrets {
            current: secret,
            reads: Arc::new(Mutex::new(vec![])),
        },
    )
}
fn native_capability(identity: Uuid, endpoint: Uuid) -> SaveCapability {
    SaveCapability {
        command_ref: Uuid::new_v4(),
        identity_ref: identity,
        endpoint_ref: endpoint,
        expected_capability_ref: None,
        source: CapabilitySource::Manual,
        source_version: "synthetic-declared-profile".into(),
        probe_invocation_ref: None,
        limits: Capability {
            context_tokens: 32768,
            max_output_tokens: 4096,
            max_timeout_seconds: 60,
            thinking_levels: ["off".into()].into(),
        },
        compat: json!({"maxTokensField":"max_completion_tokens","supportsStrictMode":true}),
        thinking_level_map: json!({}),
        output_mode: OutputMode::JsonSchema,
        strict_schema: true,
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and loopback proof only"]
async fn ai_foundation_executor_native_profile_is_frozen_and_probe_provenance_cannot_invent_capacity()
 {
    let db = fixture::proof_database("ai_executor_native_profile").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (batch, connection, secrets) = queued_profile(&db, &url, ProbeOperation::Probe, true).await;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let (_, body) = read_http(&mut socket).await;
        assert_eq!(body["max_completion_tokens"], 1024);
        assert!(body.get("max_tokens").is_none());
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
        respond(&mut socket, false).await;
    });
    let owner = ExecutorSession::acquire(&db, "synthetic-native", 4)
        .await
        .unwrap();
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        execute_probe(&db, &item, &secrets, &adapter()),
    )
    .await
    .unwrap()
    .unwrap();
    server.await.unwrap();
    let fact = sqlx::query(
        "SELECT invocation_ref,result FROM linggan_model_invocation WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let result: Value = fact.get("result");
    assert_eq!(result["syntheticFormatMatched"], true);
    assert_eq!(result["testedProfile"]["selection"]["outputTokens"], 1024);
    let row=sqlx::query("SELECT identity_ref,endpoint_ref,capability_ref FROM linggan_model_capability WHERE identity_ref IN (SELECT identity_ref FROM linggan_ai_model_identity WHERE connection_ref=$1)")
        .bind(connection).fetch_one(db.pool()).await.unwrap();
    let mut request = native_capability(row.get("identity_ref"), row.get("endpoint_ref"));
    request.source = CapabilitySource::Probe;
    request.source_version = linggan_intelligence::pi_adapter::PI_PROTOCOL.into();
    request.probe_invocation_ref = Some(fact.get("invocation_ref"));
    request.expected_capability_ref = Some(row.get("capability_ref"));
    request.limits.context_tokens = 65536;
    assert!(matches!(
        save_capability(&db, &request).await,
        Err(FoundationError::Invalid)
    ));
    request.limits.context_tokens = 32768;
    save_capability(&db, &request).await.unwrap();
    let settings = linggan_intelligence::ai_foundation::settings_read::read_settings(&db, true)
        .await
        .unwrap();
    assert_eq!(settings["execution"]["service"]["observedState"], "running");
    assert_eq!(
        settings["execution"]["dailyUsage"][0]["measuredTokens"],
        "26"
    );
    assert_eq!(settings["execution"]["dailyUsage"][0]["unknownCalls"], "0");
}
async fn read_http(socket: &mut TcpStream) -> (String, Value) {
    let mut bytes = vec![];
    let mut chunk = [0; 4096];
    let end = loop {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() < 16384, "synthetic request envelope is bounded");
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let header = std::str::from_utf8(&bytes[..end]).unwrap().to_owned();
    assert!(
        header.lines().any(|line| line
            .eq_ignore_ascii_case("authorization: Bearer SYNTHETIC-ROTATED-NOT-A-CREDENTIAL")),
        "only the explicitly supplied synthetic key may be sent"
    );
    let size = header
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    assert!(size < 8192);
    while bytes.len() < end + size {
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    let body = if size == 0 {
        Value::Null
    } else {
        serde_json::from_slice(&bytes[end..end + size]).unwrap()
    };
    (header, body)
}
async fn respond(socket: &mut TcpStream, catalog: bool) {
    let (kind, body) = if catalog {
        (
            "application/json",
            json!({"data":[{"id":"synthetic-listed"}]}).to_string(),
        )
    } else {
        (
            "text/event-stream",
            format!(
                "data: {}\n\ndata: [DONE]\n\n",
                json!({"id":"synthetic","object":"chat.completion.chunk","created":0,"model":"unlisted-synthetic-model","choices":[{"index":0,"delta":{"role":"assistant","content":"{\"lingganProbe\":\"ok\"}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":17,"completion_tokens":9,"prompt_tokens_details":{"cached_tokens":0},"completion_tokens_details":{"reasoning_tokens":0}}})
            ),
        )
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    socket.write_all(body.as_bytes()).await.unwrap();
    socket.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and loopback proof only"]
async fn ai_foundation_executor_actual_probe_uses_current_key_version_and_settles_wire_usage_atomically()
 {
    let db = fixture::proof_database("ai_executor_actual_probe").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (batch, connection, mut secrets) = queued(&db, &url, ProbeOperation::Probe).await;
    secrets.current = Uuid::new_v4();
    let rotated = version(&db, connection, 2, &url, secrets.current).await;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let (header, body) = read_http(&mut socket).await;
        assert!(header.starts_with("POST /v1/chat/completions "));
        assert_eq!(body["max_tokens"], 1024);
        assert!(
            body.get("response_format").is_none(),
            "missing capability never inherits native format from a model name"
        );
        respond(&mut socket, false).await;
    });
    let owner = ExecutorSession::acquire(&db, "synthetic-actual", 4)
        .await
        .unwrap();
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        execute_probe(&db, &item, &secrets, &adapter()),
    )
    .await
    .unwrap()
    .unwrap();
    server.await.unwrap();
    assert_eq!(*secrets.reads.lock().unwrap(), vec![secrets.current]);
    let fact=sqlx::query("SELECT i.*,c.state AS item_state,b.state AS batch_state FROM linggan_model_invocation i JOIN linggan_ai_batch_item_control c USING(invocation_ref) JOIN linggan_ai_batch_control b USING(batch_ref) WHERE i.batch_ref=$1")
        .bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(fact.get::<Uuid, _>("connection_version_ref"), rotated);
    assert_eq!(fact.get::<String, _>("item_state"), "succeeded");
    assert_eq!(fact.get::<String, _>("batch_state"), "completed");
    assert_eq!(fact.get::<String, _>("usage_state"), "measured");
    assert_eq!(fact.get::<String, _>("dispatch_state"), "started");
    assert_eq!(fact.get::<i64, _>("charged_tokens"), 26);
    assert_eq!(fact.get::<Option<i64>, _>("cached_input_tokens"), Some(0));
    assert_eq!(fact.get::<Option<i64>, _>("reasoning_tokens"), Some(0));
    let result: Value = fact.get("result");
    assert_eq!(result["modelCallable"], true);
    assert_eq!(result["syntheticFormatMatched"], true);
    assert!(result["testedProfile"].is_null());
    let attempt: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_ai_execution_attempt WHERE item_ref=$1")
            .bind(item.item_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(attempt, 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and loopback proof only"]
async fn ai_foundation_executor_actual_catalog_probe_is_independent_of_model_callability() {
    let db = fixture::proof_database("ai_executor_actual_catalog").await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (batch, _, secrets) = queued(&db, &url, ProbeOperation::Discover).await;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let (header, body) = read_http(&mut socket).await;
        assert!(header.starts_with("GET /v1/models "));
        assert!(body.is_null());
        respond(&mut socket, true).await;
    });
    let owner = ExecutorSession::acquire(&db, "synthetic-catalog", 4)
        .await
        .unwrap();
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        execute_probe(&db, &item, &secrets, &adapter()),
    )
    .await
    .unwrap()
    .unwrap();
    server.await.unwrap();
    let result: Value =
        sqlx::query_scalar("SELECT result FROM linggan_model_invocation WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(result["modelIds"], json!(["synthetic-listed"]));
    assert!(result.get("modelCallable").is_none());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_executor_switch_recheck_after_claim_never_starts_a_child_or_reserves_tokens()
{
    let db = fixture::proof_database("ai_executor_last_switch").await;
    let (_, _, secrets) = queued(&db, "http://127.0.0.1:1/v1", ProbeOperation::Probe).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-last-check", 4)
        .await
        .unwrap();
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    change_control(
        &db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: ControlScope::Global,
            expected_revision: 1,
            enabled: false,
            reason: "synthetic last-moment pause".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    let unavailable = PiAdapter::configured_with_test_command(
        PathBuf::from("/nonexistent-owned-proof-node"),
        PathBuf::from("/unused"),
    );
    execute_probe(&db, &item, &secrets, &unavailable)
        .await
        .unwrap();
    assert!(secrets.reads.lock().unwrap().is_empty());
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_ai_batch_item_control WHERE item_ref=$1")
            .bind(item.item_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "pending");
}

async fn in_flight(db: &Database, cancel: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (batch, _, secrets) = queued(db, &url, ProbeOperation::Probe).await;
    let (seen_tx, seen_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_http(&mut socket).await;
        seen_tx.send(()).unwrap();
        release_rx.await.unwrap();
        respond(&mut socket, false).await;
    });
    let owner = ExecutorSession::acquire(db, "synthetic-inflight", 4)
        .await
        .unwrap();
    let item = claim_next(db, owner.fence()).await.unwrap().unwrap();
    let worker = {
        let db = db.clone();
        let item = item.clone();
        tokio::spawn(async move { execute_probe(&db, &item, &secrets, &adapter()).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(15), seen_rx)
        .await
        .unwrap()
        .unwrap();
    if cancel {
        sqlx::query("UPDATE linggan_ai_batch_control SET cancel_requested=true,revision=revision+1 WHERE batch_ref=$1").bind(batch).execute(db.pool()).await.unwrap();
    } else {
        let mut tx = owner.fence().begin(db, false).await.unwrap();
        sqlx::query("UPDATE linggan_ai_batch_item_control SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE item_ref=$1").bind(item.item_ref).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(recover_expired(db, owner.fence()).await.unwrap(), 1);
    }
    release_tx.send(()).unwrap();
    let result = worker.await.unwrap();
    server.await.unwrap();
    if cancel {
        result.unwrap()
    } else {
        assert!(matches!(result, Err(ExecutorError::FenceLost)))
    }
    let fact=sqlx::query("SELECT i.usage_state,i.charged_tokens,i.reserved_tokens,c.state,c.output FROM linggan_model_invocation i JOIN linggan_ai_batch_item_control c USING(invocation_ref) WHERE i.batch_ref=$1")
        .bind(batch).fetch_one(db.pool()).await.unwrap();
    assert!(fact.get::<Option<Value>, _>("output").is_none());
    if cancel {
        assert_eq!(fact.get::<String, _>("state"), "cancelled");
        assert_eq!(fact.get::<String, _>("usage_state"), "measured");
        assert_eq!(fact.get::<i64, _>("charged_tokens"), 26);
    } else {
        assert_eq!(fact.get::<String, _>("state"), "failed");
        assert_eq!(fact.get::<String, _>("usage_state"), "unknown_reserved");
        assert_eq!(
            fact.get::<i64, _>("charged_tokens"),
            fact.get::<i64, _>("reserved_tokens")
        );
        let late:(Option<i64>,Option<i64>)=sqlx::query_as("SELECT input_tokens,output_tokens FROM linggan_ai_late_usage_receipt WHERE item_ref=$1").bind(item.item_ref).fetch_one(db.pool()).await.unwrap();
        assert_eq!(late, (Some(17), Some(9)));
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and loopback proof only"]
async fn ai_foundation_executor_inflight_cancel_settles_usage_without_publishing_output() {
    in_flight(
        &fixture::proof_database("ai_executor_actual_cancel").await,
        true,
    )
    .await;
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and loopback proof only"]
async fn ai_foundation_executor_late_actual_usage_is_appended_without_rewriting_unknown_terminal_quota()
 {
    in_flight(
        &fixture::proof_database("ai_executor_actual_late").await,
        false,
    )
    .await;
}
