//! Native unknown-dispatch proofs with a local synthetic child; no provider requests.
#[path = "support/topic_map_core_fixture.rs"]
mod support;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    model_settings::reserve_research_model_semantic_dispatch_permit,
    pi_adapter::PiAdapter,
    topic_map_research::{ResearchCommand, apply_research_command},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use std::time::Duration;
use support::*;
use uuid::Uuid;

const UNKNOWN_BODY: &str = "SYNTHETIC SLOW_UNKNOWN 开始练习前先明确一步。";

async fn start_run(db: &Database, work: Uuid) -> Uuid {
    let receipt = apply_research_command(db, &start(vec![work]))
        .await
        .unwrap();
    assert_eq!(receipt["state"], "queued");
    receipt["runRef"].as_str().unwrap().parse().unwrap()
}

fn spawn_worker(db: &Database, adapter: &PiAdapter) -> tokio::task::JoinHandle<bool> {
    let db = db.clone();
    let adapter = adapter.clone();
    tokio::spawn(async move {
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    })
}

async fn join_worker(worker: tokio::task::JoinHandle<bool>) {
    assert!(
        tokio::time::timeout(Duration::from_secs(15), worker)
            .await
            .unwrap()
            .unwrap()
    );
}

async fn dispatched(db: &Database, run: Uuid) -> Uuid {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let task = sqlx::query_scalar("SELECT q.task_ref FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1 AND q.dispatch_started_at IS NOT NULL AND i.state='running'")
                .bind(run).fetch_optional(db.pool()).await.unwrap();
            if let Some(task) = task { return task; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("the actual request ledger must show transmission before the mutation")
}

async fn request_counts(db: &Database, run: Uuid) -> (i64, i64) {
    sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE dispatch_started_at IS NOT NULL) FROM linggan_topic_map_research_request WHERE run_ref=$1")
        .bind(run).fetch_one(db.pool()).await.unwrap()
}

async fn assert_unknown(db: &Database, run: Uuid) {
    let row = sqlx::query("SELECT t.state AS task_state,t.last_reason,t.lease_token IS NULL AND t.lease_expires_at IS NULL AS lease_clear,i.state AS invocation_state,i.failure_code,i.reserved_tokens,i.charged_tokens,i.input_tokens IS NULL AND i.output_tokens IS NULL AS usage_unknown,i.result FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_request q USING(task_ref) JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("task_state"), "unknown_dispatch");
    assert_eq!(row.get::<String, _>("last_reason"), "unknown_dispatch");
    assert!(row.get::<bool, _>("lease_clear"));
    assert_eq!(row.get::<String, _>("invocation_state"), "failed");
    assert_eq!(row.get::<String, _>("failure_code"), "unknown_dispatch");
    assert!(row.get::<i64, _>("reserved_tokens") > 0);
    assert_eq!(
        row.get::<i64, _>("charged_tokens"),
        row.get::<i64, _>("reserved_tokens")
    );
    assert!(row.get::<bool, _>("usage_unknown"));
    let result: Value = row.get("result");
    assert_eq!(result["dispatchUnknown"], true);
    assert_eq!(
        result["diagnostic"]["stage"], "failed",
        "the synthetic response passed the real adapter validator"
    );
    assert_eq!(result["diagnostic"]["sdkErrorType"], "network");
    assert_eq!(result["diagnostic"]["usageKnown"], false);
    assert!(result["diagnostic"]["responseStarted"].is_null());
    assert_eq!(request_counts(db, run).await, (1, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE q.run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL sessions/advisory locks; synthetic adapter only"]
async fn another_config_waiting_for_the_same_model_cannot_replay_a_new_unknown_dispatch() {
    let (db, config_a, adapter) = setup("topic_core_unknown_model_wait").await;
    let work = work(&db, "unknown-model-wait", UNKNOWN_BODY).await;
    apply_research_command(&db, &configure(config_a, 100000, false))
        .await
        .unwrap();
    let run_a = start_run(&db, work).await;
    let config_b = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$2")
        .bind(config_b).bind(config_a).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(config_b, 100000, false))
        .await
        .unwrap();
    let run_b = start_run(&db, work).await;
    let queued = sqlx::query("SELECT t.input_hash,t.input_refs,t.state,r.config_ref,c.model_ref FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref) JOIN linggan_model_config c ON c.config_ref=r.config_ref ORDER BY t.created_at")
        .fetch_all(db.pool()).await.unwrap();
    assert_eq!(queued.len(), 2);
    assert_eq!(queued[0].get::<Uuid, _>("config_ref"), config_a);
    assert_eq!(queued[1].get::<Uuid, _>("config_ref"), config_b);
    assert!(
        queued
            .iter()
            .all(|row| row.get::<String, _>("state") == "queued")
    );
    assert_eq!(
        queued[0].get::<Uuid, _>("model_ref"),
        queued[1].get::<Uuid, _>("model_ref")
    );
    assert_ne!(
        queued[0].get::<String, _>("input_hash"),
        queued[1].get::<String, _>("input_hash")
    );
    assert_eq!(
        queued[0].get::<Value, _>("input_refs")["fragments"],
        queued[1].get::<Value, _>("input_refs")["fragments"]
    );

    // Discover the public permit's actual native lock tag; no guessed lock names.
    let mut permit = reserve_research_model_semantic_dispatch_permit(&db, config_a)
        .await
        .unwrap();
    let mut lock_tx = permit.begin().await.unwrap();
    let tag: (i64, i64, i32) = sqlx::query_as("SELECT classid::bigint,objid::bigint,objsubid::int FROM pg_locks WHERE pid=pg_backend_pid() AND locktype='advisory' AND granted")
        .fetch_one(&mut *lock_tx).await.unwrap();
    lock_tx.commit().await.unwrap();
    permit.release().await.unwrap();

    let worker_a = spawn_worker(&db, &adapter);
    let task_a = dispatched(&db, run_a).await;
    // Hold A's acceptance until native PostgreSQL proves B has already prepared
    // and is waiting for exactly A's model permit. A still produces its own
    // unknown ledger through the real adapter + settlement, without SQL seeding.
    let mut acceptance = db.pool().begin().await.unwrap();
    let state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1 FOR UPDATE",
    )
    .bind(task_a)
    .fetch_one(&mut *acceptance)
    .await
    .unwrap();
    assert_eq!(state, "running");
    let worker_b = spawn_worker(&db, &adapter);
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks waiting JOIN pg_locks held ON held.locktype=waiting.locktype AND held.database=waiting.database AND held.classid=waiting.classid AND held.objid=waiting.objid AND held.objsubid=waiting.objsubid WHERE waiting.locktype='advisory' AND NOT waiting.granted AND held.granted AND waiting.database=(SELECT oid FROM pg_database WHERE datname=current_database()) AND waiting.classid::bigint=$1 AND waiting.objid::bigint=$2 AND waiting.objsubid::int=$3 AND held.pid=ANY(pg_blocking_pids(waiting.pid)))")
                .bind(tag.0).bind(tag.1).bind(tag.2).fetch_one(&mut *acceptance).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("B must wait on the same native model lock before A becomes unknown");
    let before_send: (i64, i64) = sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE dispatch_started_at IS NOT NULL) FROM linggan_topic_map_research_request WHERE run_ref=$1")
        .bind(run_b).fetch_one(&mut *acceptance).await.unwrap();
    assert_eq!(before_send, (0, 0));
    acceptance.commit().await.unwrap();
    join_worker(worker_a).await;
    join_worker(worker_b).await;
    assert_unknown(&db, run_a).await;

    let refused = sqlx::query("SELECT t.state,t.last_reason,t.attempt_count,t.phase_attempt_count,t.lease_token IS NULL AND t.lease_expires_at IS NULL AS lease_clear,i.charged_tokens,i.reserved_tokens,i.state AS invocation_state,i.failure_code FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_request q USING(task_ref) JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1")
        .bind(run_b).fetch_one(db.pool()).await.unwrap();
    assert_eq!(refused.get::<String, _>("state"), "stale");
    assert_eq!(
        refused.get::<String, _>("last_reason"),
        "prior_unknown_dispatch"
    );
    assert_eq!(refused.get::<String, _>("invocation_state"), "failed");
    assert_eq!(
        refused.get::<String, _>("failure_code"),
        "prior_unknown_dispatch"
    );
    assert_eq!(refused.get::<i32, _>("attempt_count"), 1);
    assert_eq!(refused.get::<i32, _>("phase_attempt_count"), 0);
    assert!(refused.get::<bool, _>("lease_clear"));
    assert!(
        refused.get::<i64, _>("reserved_tokens") > 0,
        "a real reservation preceded the final send gate"
    );
    assert_eq!(refused.get::<i64, _>("charged_tokens"), 0);
    assert_eq!(request_counts(&db, run_b).await, (1, 0));
    // Historical reservation estimates remain auditable; no live charge survives.
    let live: i64 = sqlx::query_scalar("SELECT COALESCE(sum(i.reserved_tokens),0)::bigint FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE q.run_ref=$1 AND i.state='running'")
        .bind(run_b).fetch_one(db.pool()).await.unwrap();
    assert_eq!(live, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE q.run_ref=$1"
        )
        .bind(run_b)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic delayed unknown response only"]
async fn pause_cannot_replace_a_transmitted_unknown_with_a_replayable_task() {
    let (db, config, adapter) = setup("topic_core_unknown_paused").await;
    let work = work(&db, "unknown-paused", UNKNOWN_BODY).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let run = start_run(&db, work).await;
    let worker = spawn_worker(&db, &adapter);
    let task = dispatched(&db, run).await;
    apply_research_command(
        &db,
        &ResearchCommand::Pause {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            run_ref: Some(run),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1"
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "paused"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1"
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "running",
        "pause must precede synthetic response settlement"
    );
    join_worker(worker).await;
    assert_unknown(&db, run).await;
    apply_research_command(
        &db,
        &ResearchCommand::Resume {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            run_ref: Some(run),
        },
    )
    .await
    .unwrap();
    finish_pending(&db, &adapter).await;
    let repeated = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    assert_eq!(repeated["state"], "no_new_input");
    finish_pending(&db, &adapter).await;
    assert_unknown(&db, run).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic canonical observations only"]
async fn source_changes_cannot_erase_unknown_or_allow_the_same_source_to_be_sent_again() {
    let (db, config, adapter) = setup("topic_core_unknown_source_changed").await;
    let id = "unknown-source-changed";
    let work = work(&db, id, UNKNOWN_BODY).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let run = start_run(&db, work).await;
    let worker = spawn_worker(&db, &adapter);
    let task = dispatched(&db, run).await;
    let mut acceptance = db.pool().begin().await.unwrap();
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(D)
    .execute(&mut *acceptance)
    .await
    .unwrap();
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_topic_map_research_task WHERE task_ref=$1")
            .bind(task)
            .fetch_one(&mut *acceptance)
            .await
            .unwrap();
    assert_eq!(
        state, "running",
        "the policy lock must precede unknown settlement"
    );
    let replacement = "SYNTHETIC 已替换的练习来源。";
    assert_eq!(
        work_at(&db, id, "", replacement, 10, "2026-08-29T10:00:00Z").await,
        work
    );
    let changed = linggan_evidence::creator_discovery::load_topic_materials(&db, D)
        .await
        .unwrap();
    assert!(changed.works.iter().any(|current| current.work_ref == work
        && current.fragments.iter().any(|f| f.text == replacement)));
    acceptance.commit().await.unwrap();
    join_worker(worker).await;
    assert_unknown(&db, run).await;
    // A new physical observation restoring identical text is the same source
    // range for replay protection, even though its observation UUID is newer.
    assert_eq!(
        work_at(&db, id, "", UNKNOWN_BODY, 10, "2026-08-30T10:00:00Z").await,
        work
    );
    let repeated = apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    assert_eq!(repeated["state"], "no_new_input");
    finish_pending(&db, &adapter).await;
    assert_unknown(&db, run).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_research_request")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}
