//! The dedicated AI process. Its session heartbeat runs while claims and child calls await.
use super::{ExecutorError, ExecutorSession, ocr_recognition, probe, queue, schema, writing};
use crate::{
    ai_foundation::TaskKey, model_secrets::ModelSecretStore, model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    task::JoinSet,
    time::{Instant, MissedTickBehavior},
};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrainReceipt {
    pub instance_ref: Uuid,
    pub generation: String,
    pub process_id: u32,
    pub source_revision: String,
}

/// Saving configuration or starting this process does not turn on any execution control.
pub async fn run(
    db: Database,
    source_revision: &str,
    concurrency: u32,
    secrets: Arc<dyn ModelSecretStore>,
    adapter: PiAdapter,
    drain: ModelWorkerDrain,
) -> Result<DrainReceipt, ExecutorError> {
    schema::require_ready(&db).await?;
    let mut session = ExecutorSession::acquire(&db, source_revision, concurrency).await?;
    let fence = session.fence();
    // Polling a DB heartbeat must not suspend the transaction which owns a shared
    // instance lock. JoinSet also aborts the work loop and its children on failure.
    let (work_db, work_drain) = (db.clone(), drain.clone());
    let mut work = JoinSet::new();
    work.spawn(async move {
        work_loop(&work_db, fence, concurrency, secrets, adapter, work_drain).await
    });
    let mut stopping = drain.subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut deadline = None;
    loop {
        // Reading the current value also handles a stop requested before subscription.
        if drain.is_requested() && deadline.is_none() {
            deadline = Some(Instant::now() + Duration::from_secs(AI_LIMITS.drain_grace_seconds()));
            tokio::time::timeout(Duration::from_secs(5), session.request_drain())
                .await
                .map_err(|_| ExecutorError::HeartbeatTimedOut)??;
        }
        tokio::select! {
            biased;
            _=stopping.changed(), if deadline.is_none()=>{},
            _=tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some()=>{
                return Err(ExecutorError::DrainTimedOut);
            },
            _=heartbeat.tick()=>{
                tokio::time::timeout(Duration::from_secs(5),session.heartbeat()).await
                    .map_err(|_|ExecutorError::HeartbeatTimedOut)??;
            },
            result=work.join_next()=>{
                result.ok_or(ExecutorError::TaskFailed)?
                    .map_err(|_|ExecutorError::TaskFailed)??;
                break;
            }
        }
    }
    // An idle work loop can observe the stop and finish during the same select
    // poll, before the outer loop handles the watch notification. Persist that
    // transition before completing the drain; an empty queue is not a DB receipt.
    if deadline.is_none() {
        tokio::time::timeout(Duration::from_secs(5), session.request_drain())
            .await
            .map_err(|_| ExecutorError::HeartbeatTimedOut)??;
    }
    session.complete_drain().await?;
    Ok(DrainReceipt {
        instance_ref: fence.instance_ref(),
        generation: fence.generation().to_string(),
        process_id: std::process::id(),
        source_revision: source_revision.into(),
    })
}

async fn work_loop(
    db: &Database,
    fence: super::ExecutorFence,
    concurrency: u32,
    secrets: Arc<dyn ModelSecretStore>,
    adapter: PiAdapter,
    drain: ModelWorkerDrain,
) -> Result<(), ExecutorError> {
    let mut tasks = JoinSet::new();
    let mut failure = None;
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut recovery = Instant::now();
    let mut audit_due = Instant::now();
    loop {
        if drain.is_requested() && tasks.is_empty() {
            return failure.map_or(Ok(()), Err);
        }
        tokio::select! {
            result=tasks.join_next(), if !tasks.is_empty()=>{
                let result=result.expect("a nonempty JoinSet yields a task")
                    .map_err(|_|ExecutorError::TaskFailed).and_then(|r|r);
                if let Err(error)=result {
                    failure.get_or_insert(error);
                    drain.request();
                }
            },
            _=poll.tick()=>{
                if drain.is_requested() {continue;}
                let result=fill_slots(db,fence,concurrency,&mut tasks,&secrets,&adapter,&drain,&mut recovery,&mut audit_due).await;
                if let Err(error)=result {
                    failure.get_or_insert(error);
                    drain.request();
                }
            }
        }
    }
}

async fn fill_slots(
    db: &Database,
    fence: super::ExecutorFence,
    concurrency: u32,
    tasks: &mut JoinSet<Result<(), ExecutorError>>,
    secrets: &Arc<dyn ModelSecretStore>,
    adapter: &PiAdapter,
    drain: &ModelWorkerDrain,
    recovery: &mut Instant,
    audit_due: &mut Instant,
) -> Result<(), ExecutorError> {
    if Instant::now() >= *recovery {
        queue::recover_expired(db, fence).await?;
        *recovery = Instant::now() + Duration::from_secs(5);
    }
    if Instant::now() >= *audit_due {
        // Optional sampling cannot take down unrelated AI handlers. Admission is rechecked by enqueue.
        if crate::ocr_quality_audit::schedule::tick(db).await.is_err() {
            eprintln!("ocr_audit_schedule_deferred");
        }
        *audit_due = Instant::now() + Duration::from_secs(60);
    }
    while tasks.len() < concurrency as usize && !drain.is_requested() {
        let Some(item) = queue::claim_next(db, fence).await? else {
            break;
        };
        if drain.is_requested() {
            item.defer(db, "executor_draining").await?;
            break;
        }
        let (db, secrets, adapter) = (db.clone(), secrets.clone(), adapter.clone());
        tasks.spawn(async move {
            match item.task_key {
                TaskKey::ModelProbe => {
                    probe::execute_probe(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::OcrQualityAudit => {
                    super::ocr_audit::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::OcrRecognition => {
                    ocr_recognition::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::WritingLegacy => {
                    writing::execute_writing(&db, &item, secrets.as_ref(), &adapter).await
                }
                _ => Err(ExecutorError::Invalid),
            }
        });
    }
    Ok(())
}
