//! Real PostgreSQL ownership/queue proof. No provider calls; one explicitly labelled reservation fixture.
use super::{batches, endpoint, fixture, new_connection, version};
use linggan_intelligence::ai_foundation::{
    TaskKey,
    batch_store::{BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation, create_batch},
    control_store::{Actor, ChangeControl, ControlScope, change_control},
    executor::{
        ExecutorError, ExecutorSession,
        queue::{ClaimedItem, claim_next, recover_expired},
    },
    provider_store::MaterialClass,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
use std::{collections::BTreeSet, time::Duration};
use uuid::Uuid;

async fn queued(db: &Database, count: usize) -> Uuid {
    let config = batches::policy(db).await;
    let conn = new_connection(db).await;
    let version = version(db, conn, 1, "https://executor-fixture.invalid/api").await;
    batches::grant(
        db,
        endpoint(db, version).await,
        TaskKey::ModelProbe,
        MaterialClass::ModelCatalog,
    )
    .await;
    batches::on(db, TaskKey::ModelProbe).await;
    create_batch(
        db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: config.revision,
            task_config_ref: config.task_config_ref,
            kind: BatchKind::Probe,
            items: (0..count)
                .map(|n| BatchInput {
                    item_key: n.to_string(),
                    input: InputRef::Probe {
                        sample_ref: Uuid::new_v4(),
                        connection_version_ref: version,
                        model_ref: None,
                        operation: ProbeOperation::Connect,
                        profile: None,
                    },
                })
                .collect(),
        },
    )
    .await
    .unwrap()
    .batch_ref
}
async fn replacement(db: &Database) -> ExecutorSession {
    // Closing a socket releases its session lock asynchronously at PostgreSQL's end.
    for _ in 0..50 {
        match ExecutorSession::acquire(db, "synthetic-replacement", 4).await {
            Ok(owner) => return owner,
            Err(ExecutorError::Busy) => tokio::time::sleep(Duration::from_millis(20)).await,
            Err(error) => panic!("replacement failed: {error}"),
        }
    }
    panic!("closed owner session retained its lock")
}
async fn synthetic_reservation(db: &Database, owner: &ExecutorSession, item: &ClaimedItem) -> Uuid {
    let InputRef::Probe {
        connection_version_ref,
        ..
    } = item.input
    else {
        panic!("probe fixture")
    };
    let invocation = Uuid::new_v4();
    let mut tx = owner.fence().begin(db, true).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,task_config_ref,batch_ref,quota_bucket,classification_source,dispatch_admitted_at,result)
        VALUES($1,$2,'connect',repeat('0',64),'running',60,60,'model_probe',$3,$4,'probe','producer_v1',clock_timestamp(),'{\"syntheticReservationFixture\":true}')")
        .bind(invocation).bind(connection_version_ref).bind(item.task_config_ref).bind(item.batch_ref).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control control SET invocation_ref=$2,attempt_count=1,
         executed_config_ref=config.task_config_ref,executed_skill_version_ref=config.skill_version_ref,execution_contract=config.execution_contract
         FROM linggan_ai_task_config config WHERE control.item_ref=$1 AND config.task_config_ref=$3")
        .bind(item.item_ref).bind(invocation).bind(item.task_config_ref).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    invocation
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_executor_session_lock_heartbeat_expiry_and_generation_fence_are_real() {
    let db = fixture::proof_database("ai_executor_session").await;
    let initial = linggan_intelligence::ai_foundation::settings_read::read_settings(&db, true)
        .await
        .unwrap();
    assert_eq!(
        initial["execution"]["service"]["observedState"],
        "not_started"
    );
    assert!(matches!(
        ExecutorSession::acquire(&db, "synthetic", 3).await,
        Err(ExecutorError::Invalid)
    ));
    let mut first = ExecutorSession::acquire(&db, "synthetic", 8).await.unwrap();
    let stale = first.fence();
    assert!(matches!(
        ExecutorSession::acquire(&db, "synthetic", 4).await,
        Err(ExecutorError::Busy)
    ));
    first.heartbeat().await.unwrap();
    let owned: bool = sqlx::query_scalar("SELECT linggan_ai_executor_is_owner($1,$2)")
        .bind(stale.instance_ref())
        .bind(stale.generation())
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(owned);
    sqlx::query("UPDATE linggan_ai_executor_instance SET heartbeat_at=clock_timestamp()-interval '31 seconds' WHERE instance_ref=$1")
        .bind(stale.instance_ref()).execute(db.pool()).await.unwrap();
    assert!(matches!(
        first.heartbeat().await,
        Err(ExecutorError::FenceLost)
    ));
    let expired = linggan_intelligence::ai_foundation::settings_read::read_settings(&db, true)
        .await
        .unwrap();
    assert_eq!(expired["execution"]["service"]["observedState"], "stale");
    assert!(matches!(
        stale.begin(&db, true).await,
        Err(ExecutorError::FenceLost)
    ));
    drop(first);
    let mut second = replacement(&db).await;
    assert_eq!(second.fence().generation(), stale.generation() + 1);
    assert!(matches!(
        stale.begin(&db, false).await,
        Err(ExecutorError::FenceLost)
    ));
    second.request_drain().await.unwrap();
    second.complete_drain().await.unwrap();
    let old: String =
        sqlx::query_scalar("SELECT state FROM linggan_ai_executor_instance WHERE instance_ref=$1")
            .bind(stale.instance_ref())
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(old, "lost");
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_executor_concurrent_claims_obey_capacity_and_pause_preserves_pending_work() {
    let db = fixture::proof_database("ai_executor_claim").await;
    queued(&db, 8).await;
    let owner = ExecutorSession::acquire(&db, "synthetic", 4).await.unwrap();
    let mut tasks = vec![];
    for _ in 0..8 {
        let db = db.clone();
        let fence = owner.fence();
        tasks.push(tokio::spawn(async move {
            claim_next(&db, fence).await.unwrap()
        }));
    }
    let mut claims = vec![];
    for task in tasks {
        if let Some(claim) = task.await.unwrap() {
            claims.push(claim)
        }
    }
    assert_eq!(claims.len(), 4);
    assert_eq!(
        claims
            .iter()
            .map(|c| c.item_ref)
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    change_control(
        &db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: ControlScope::Global,
            expected_revision: 1,
            enabled: false,
            reason: "synthetic pause".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    for claim in &claims {
        claim.defer(&db, "global_paused").await.unwrap();
    }
    assert!(claim_next(&db, owner.fence()).await.unwrap().is_none());
    let counts:(i64,i64)=sqlx::query_as("SELECT count(*) FILTER(WHERE state='pending'),count(*) FILTER(WHERE state='failed') FROM linggan_ai_batch_item_control")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (8, 0));
    assert!(matches!(
        claims[0].renew(&db).await,
        Err(ExecutorError::FenceLost)
    ));
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_executor_recovery_requeues_only_untouched_items_and_keeps_unknown_quota() {
    let db = fixture::proof_database("ai_executor_recovery").await;
    queued(&db, 2).await;
    let first = ExecutorSession::acquire(&db, "synthetic", 4).await.unwrap();
    let untouched = claim_next(&db, first.fence()).await.unwrap().unwrap();
    let attempted = claim_next(&db, first.fence()).await.unwrap().unwrap();
    let invocation = synthetic_reservation(&db, &first, &attempted).await;
    drop(first);
    let second = replacement(&db).await;
    assert!(matches!(
        attempted.renew(&db).await,
        Err(ExecutorError::FenceLost)
    ));
    assert_eq!(recover_expired(&db, second.fence()).await.unwrap(), 2);
    assert_eq!(recover_expired(&db, second.fence()).await.unwrap(), 0);
    let fact=sqlx::query("SELECT state,usage_state,charged_tokens,input_tokens,output_tokens FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(fact.get::<String, _>("state"), "failed");
    assert_eq!(fact.get::<String, _>("usage_state"), "unknown_reserved");
    assert_eq!(fact.get::<i64, _>("charged_tokens"), 60);
    assert!(fact.get::<Option<i64>, _>("input_tokens").is_none());
    assert!(fact.get::<Option<i64>, _>("output_tokens").is_none());
    let next = claim_next(&db, second.fence()).await.unwrap().unwrap();
    assert_eq!(next.item_ref, untouched.item_ref);
    assert_eq!(next.lease_generation, untouched.lease_generation + 1);
    assert!(claim_next(&db, second.fence()).await.unwrap().is_none());
    assert!(matches!(
        untouched.defer(&db, "stale_owner").await,
        Err(ExecutorError::FenceLost)
    ));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_executor_drain_blocks_claims_but_allows_receipts_and_requires_no_inflight() {
    let db = fixture::proof_database("ai_executor_drain").await;
    queued(&db, 1).await;
    let mut owner = ExecutorSession::acquire(&db, "synthetic", 4).await.unwrap();
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    owner.request_drain().await.unwrap();
    assert!(matches!(
        claim_next(&db, owner.fence()).await,
        Err(ExecutorError::FenceLost)
    ));
    assert!(matches!(
        owner.complete_drain().await,
        Err(ExecutorError::FenceLost)
    ));
    item.renew(&db).await.unwrap();
    item.defer(&db, "executor_draining").await.unwrap();
    owner.complete_drain().await.unwrap();
    let status:bool=sqlx::query_scalar("SELECT state='drained' AND drain_requested_at IS NOT NULL AND drained_at IS NOT NULL FROM linggan_ai_executor_instance WHERE instance_ref=$1")
        .bind(owner.fence().instance_ref()).fetch_one(db.pool()).await.unwrap();
    assert!(status);
    assert!(matches!(
        owner.heartbeat().await,
        Err(ExecutorError::FenceLost)
    ));
}
