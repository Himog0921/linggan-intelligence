//! Actual service tasks, Node/SDK and held loopback requests, never a runtime service or provider.
#![recursion_limit = "256"]
#[path = "support/ai_commit_proxy.rs"]
mod commit_proxy;
#[path = "support/ai_service_endpoint.rs"]
mod endpoint;
#[allow(dead_code)]
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
#[path = "support/ai_service_fixture.rs"]
mod service_fixture;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[allow(dead_code)]
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::executor::{ExecutorError, service},
    model_secrets::{ModelSecretStore, SyntheticModelSecrets},
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use service_fixture::queued;
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;
fn adapter() -> PiAdapter {
    PiAdapter::configured_with_test_command(
        PathBuf::from(
            std::env::var_os("AI_FOUNDATION_PROOF_NODE").expect("owned proof supplies Node"),
        ),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    )
}
async fn observed_count(endpoint: &endpoint::Endpoint, count: usize) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while endpoint.requests.lock().unwrap().len() < count {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual requests reach owned endpoint");
}
async fn state(db: &Database, expected: &str) {
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {
            let state:Option<String>=sqlx::query_scalar("SELECT i.state FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)").fetch_optional(db.pool()).await.unwrap();
            if state.as_deref()==Some(expected) {break;}
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("database executor state");
}
async fn concurrency(capacity: u32, heartbeat: bool) {
    let db = fixture::db(&format!("ai_service_{capacity}")).await;
    let endpoint = endpoint::Endpoint::start().await;
    let batch = queued(&db, &endpoint.url, capacity as usize + 1).await;
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        db.clone(),
        "synthetic-service-proof",
        capacity,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        drain.clone(),
    ));
    observed_count(&endpoint, capacity as usize).await;
    if heartbeat {
        let before:String=sqlx::query_scalar("SELECT i.heartbeat_at::text FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)").fetch_one(db.pool()).await.unwrap();
        tokio::time::sleep(Duration::from_secs(6)).await;
        let after:(bool,bool)=sqlx::query_as("SELECT i.heartbeat_at>$1::text::timestamptz,clock_timestamp()-i.heartbeat_at<interval '6 seconds' FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)").bind(before).fetch_one(db.pool()).await.unwrap();
        assert_eq!(
            after,
            (true, true),
            "held children cannot starve the independent session heartbeat"
        );
        let renewed:bool=sqlx::query_scalar("SELECT bool_and(lease_expires_at-started_at>interval '33 seconds') FROM linggan_ai_batch_item_control WHERE state='running'").fetch_one(db.pool()).await.unwrap();
        assert!(renewed, "each real call renews its own lease while blocked");
    }
    assert_eq!(endpoint.requests.lock().unwrap().len(), capacity as usize);
    let counts:(i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE c.state='running'),count(*) FILTER(WHERE c.state='pending') FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (capacity as i64, 1));
    drain.request();
    state(&db, "draining").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !task.is_finished(),
        "drain must wait for actual in-flight receipts"
    );
    endpoint.release();
    let receipt = tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    state(&db, "drained").await;
    let settled:(i64,i64,Option<i64>)=sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE state='succeeded' AND usage_state='measured'),sum(measured_tokens)::bigint FROM linggan_model_invocation WHERE task_key='model_probe'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        settled,
        (capacity as i64, capacity as i64, Some(capacity as i64 * 26))
    );
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        capacity as usize,
        "drain never sends the queued remainder"
    );
    assert_eq!(receipt.process_id, std::process::id());
    let final_state:(String,i64)=sqlx::query_as("SELECT state,(SELECT count(*) FROM linggan_ai_batch_item_control WHERE state='running') FROM linggan_ai_executor_instance WHERE instance_ref=$1").bind(receipt.instance_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(final_state, ("drained".into(), 0));
    endpoint.finish().await;
}
#[tokio::test]
#[ignore = "owned PostgreSQL and Node/SDK only"]
async fn actual_service_four_calls_renew_and_heartbeat_independently_then_drain_without_claiming_remainder()
 {
    concurrency(4, true).await;
}
#[tokio::test]
#[ignore = "owned PostgreSQL and Node/SDK only"]
async fn actual_service_eight_calls_respect_capacity_and_finish_receipts_before_drain() {
    concurrency(8, false).await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL and actual Node/SDK only"]
async fn cancel_batch_during_service_calls_retains_actual_usage_and_discards_outputs() {
    use linggan_intelligence::ai_foundation::batch_control::{
        CancelBatch, cancel_batch, read_batch,
    };
    let db = fixture::db("ai_service_cancel").await;
    let endpoint = endpoint::Endpoint::start().await;
    let batch = queued(&db, &endpoint.url, 5).await;
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        db.clone(),
        "synthetic-cancel",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        drain.clone(),
    ));
    observed_count(&endpoint, 4).await;
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM linggan_ai_batch_control WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let receipt = cancel_batch(
        &db,
        &CancelBatch {
            command_ref: Uuid::new_v4(),
            batch_ref: batch,
            expected_revision: revision,
            reason: "Synthetic in-flight cancellation".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "open");
    assert!(receipt.cancel_requested);
    let counts:(i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE state='running'),count(*) FILTER(WHERE state='cancelled') FROM linggan_ai_batch_item_control").fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (4, 1));
    drain.request();
    state(&db, "draining").await;
    endpoint.release();
    tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let final_batch = read_batch(&db, batch, 0).await.unwrap();
    assert_eq!(final_batch["batch"]["state"], "cancelled");
    assert!(
        final_batch["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["state"] == "cancelled" && i["usable"] == false)
    );
    assert_eq!(final_batch["rounds"][0]["attempted"], 4);
    assert_eq!(final_batch["rounds"][0]["usable"], 0);
    let usage:(i64,Option<i64>)=sqlx::query_as("SELECT count(*),sum(measured_tokens)::bigint FROM linggan_model_invocation WHERE usage_state='measured'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(usage, (4, Some(104)));
    assert_eq!(endpoint.requests.lock().unwrap().len(), 4);
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL only"]
async fn service_refuses_missing_or_different_migration_before_acquiring_an_instance() {
    let db = fixture::db("ai_service_schema").await;
    sqlx::query(
        "DELETE FROM linggan_local_schema_migration WHERE migration_id='0140_ai_gen1_writing'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let result = service::run(
        db.clone(),
        "synthetic-schema",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        ModelWorkerDrain::new(),
    )
    .await;
    assert!(matches!(result, Err(ExecutorError::SchemaNotReady)));
    sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES('0140_ai_gen1_writing',repeat('0',64))").execute(db.pool()).await.unwrap();
    let result = service::run(
        db.clone(),
        "synthetic-schema",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        ModelWorkerDrain::new(),
    )
    .await;
    assert!(matches!(result, Err(ExecutorError::SchemaNotReady)));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ai_executor_instance")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "owned PostgreSQL and Node/SDK only"]
async fn service_retries_a_failed_receipt_transaction_without_recalling_the_model() {
    let db = fixture::db("ai_service_receipt").await;
    let endpoint = endpoint::Endpoint::start().await;
    queued(&db, &endpoint.url, 1).await;
    sqlx::raw_sql("CREATE SEQUENCE synthetic_receipt_attempt;
        CREATE FUNCTION synthetic_fail_first_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF OLD.state='running' AND NEW.state IN ('succeeded','failed') AND nextval('synthetic_receipt_attempt')=1
        THEN RAISE EXCEPTION 'synthetic receipt transaction retry' USING ERRCODE='40001'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER synthetic_receipt_failure BEFORE UPDATE OF state ON linggan_model_invocation
        FOR EACH ROW EXECUTE FUNCTION synthetic_fail_first_receipt();").execute(db.pool()).await.unwrap();
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        db.clone(),
        "synthetic-receipt",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        drain.clone(),
    ));
    observed_count(&endpoint, 1).await;
    drain.request();
    state(&db, "draining").await;
    endpoint.release();
    tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let attempts: i64 = sqlx::query_scalar("SELECT last_value FROM synthetic_receipt_attempt")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(attempts, 2);
    let settled:(i64,Option<i64>)=sqlx::query_as("SELECT count(*) FILTER(WHERE state='succeeded'),sum(measured_tokens)::bigint FROM linggan_model_invocation").fetch_one(db.pool()).await.unwrap();
    assert_eq!(settled, (1, Some(26)));
    assert_eq!(endpoint.requests.lock().unwrap().len(), 1);
    endpoint.finish().await;
}

struct MissingSecrets;
impl ModelSecretStore for MissingSecrets {
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        Err(ModelError::SecretUnavailable)
    }
    fn is_synthetic(&self) -> bool {
        true
    }
}
#[tokio::test]
#[ignore = "owned PostgreSQL only; no adapter dispatch"]
async fn missing_secret_is_a_visible_unattempted_wait_and_can_drain() {
    let db = fixture::db("ai_service_secret").await;
    queued(&db, "http://127.0.0.1:1/v1", 1).await;
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        db.clone(),
        "synthetic-no-secret",
        4,
        Arc::new(MissingSecrets),
        adapter(),
        drain.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_batch_item_control WHERE state='pending' AND attempt_count=0 AND invocation_ref IS NULL AND wait_reason='model_secret_unavailable')").fetch_one(db.pool()).await.unwrap();if waiting {break;}tokio::time::sleep(Duration::from_millis(20)).await;}
    }).await.unwrap();
    drain.request();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "owned PostgreSQL wire fault and actual Node/SDK only"]
async fn service_reconciles_a_committed_receipt_after_network_ack_loss_without_resending_or_late_usage()
 {
    let observed_started = std::time::Instant::now();
    let db = fixture::db("ai_service_commit_ack").await;
    let endpoint = endpoint::Endpoint::start().await;
    queued(&db, &endpoint.url, 1).await;
    let proxy =
        commit_proxy::CommitProxy::start(&std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap())
            .await;
    let proxied = Database::connect_within_schema(&proxy.url, "ai_service_commit_ack")
        .await
        .unwrap();
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        proxied.clone(),
        "synthetic-lost-commit-ack",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        drain.clone(),
    ));
    observed_count(&endpoint, 1).await;
    let wire_observed = observed_started.elapsed();
    drain.request();
    state(&db, "draining").await;
    endpoint.release();
    let released_at = observed_started.elapsed();
    let result = tokio::time::timeout(Duration::from_secs(20), task)
        .await
        .expect("bounded service join")
        .expect("service task did not panic");
    eprintln!(
        "commit-ack timing: wire={wire_observed:?} released={released_at:?} total={:?} dropped={} result={:?}",
        observed_started.elapsed(),
        proxy.dropped.load(std::sync::atomic::Ordering::SeqCst),
        result.as_ref().err()
    );
    let failure_state:serde_json::Value=sqlx::query_scalar("SELECT jsonb_build_object('instanceState',i.state,'heartbeatAgeSeconds',extract(epoch FROM clock_timestamp()-i.heartbeat_at),'ownerAlive',EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=i.owner_backend_pid),'items',(SELECT jsonb_agg(jsonb_build_object('state',state,'leaseSeconds',extract(epoch FROM lease_expires_at-clock_timestamp()),'attempts',attempt_count)) FROM linggan_ai_batch_item_control),'invocations',(SELECT jsonb_agg(jsonb_build_object('state',state,'dispatch',dispatch_state,'usage',usage_state,'hasReceipt',result ? 'executorReceipt')) FROM linggan_model_invocation)) FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)")
        .fetch_one(db.pool()).await.expect("owned failure diagnostics");
    if let Err(error) = &result {
        panic!("commit-ack service failed: {error}; owned state={failure_state}");
    }
    let receipt = result.unwrap();
    assert!(
        proxy.dropped.load(std::sync::atomic::Ordering::SeqCst),
        "the actual COMMIT acknowledgement must be discarded on the wire"
    );
    let ledger:(i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_model_invocation WHERE state='succeeded' AND measured_tokens=26),(SELECT count(*) FROM linggan_ai_execution_attempt),(SELECT count(*) FROM linggan_ai_late_usage_receipt),(SELECT count(*) FROM linggan_ai_batch_item_control WHERE state='succeeded')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(ledger, (1, 1, 0, 1));
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        1,
        "only the DB receipt may be reconciled, never the model call"
    );
    state(&db, "drained").await;
    assert_eq!(receipt.process_id, std::process::id());
    proxied.pool().close().await;
    drop(proxy);
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL, empty queue and actual service loop; no SDK calls"]
async fn service_stop_at_start_of_an_empty_queue_always_persists_draining_before_drained() {
    let db = fixture::db("ai_service_idle_stop").await;
    for generation in 1..=12_i64 {
        let drain = ModelWorkerDrain::new();
        let task = tokio::spawn(service::run(
            db.clone(),
            "synthetic-idle-stop",
            4,
            Arc::new(SyntheticModelSecrets),
            adapter(),
            drain.clone(),
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_executor_instance WHERE generation=$1 AND state='running')").bind(generation).fetch_one(db.pool()).await.unwrap();
                if current {break;}
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        drain.request();
        let receipt = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let persisted:bool=sqlx::query_scalar("SELECT state='drained' AND drain_requested_at IS NOT NULL AND drained_at>=drain_requested_at FROM linggan_ai_executor_instance WHERE instance_ref=$1").bind(receipt.instance_ref).fetch_one(db.pool()).await.unwrap();
        assert!(persisted);
    }
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "owned PostgreSQL lock scheduling and actual service loop; no SDK calls"]
async fn service_heartbeat_wait_cannot_suspend_the_claim_transaction_that_releases_its_row_lock() {
    let db = fixture::db("ai_service_heartbeat_lock").await;
    let drain = ModelWorkerDrain::new();
    let task = tokio::spawn(service::run(
        db.clone(),
        "synthetic-heartbeat-lock",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter(),
        drain.clone(),
    ));
    state(&db, "running").await;
    let owner: Uuid =
        sqlx::query_scalar("SELECT instance_ref FROM linggan_ai_executor_current WHERE singleton")
            .fetch_one(db.pool())
            .await
            .unwrap();
    // Own session socket, never a lock-bearing connection returned to the pool.
    let mut blocker = db.pool().acquire().await.unwrap().detach();
    sqlx::query("SELECT pg_advisory_lock(hashtextextended($1,4540139))")
        .bind(owner.to_string())
        .execute(&mut blocker)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))").fetch_one(db.pool()).await.unwrap();
            if waiting {break;}
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    // Cross the real five-second heartbeat tick while the claim owns FOR SHARE.
    tokio::time::sleep(Duration::from_secs(6)).await;
    sqlx::query("SELECT pg_advisory_unlock(hashtextextended($1,4540139))")
        .bind(owner.to_string())
        .execute(&mut blocker)
        .await
        .unwrap();
    drop(blocker);
    drain.request();
    tokio::time::timeout(Duration::from_secs(8), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    state(&db, "drained").await;
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}
