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
mod comment_problem;
mod comment_research;
mod content_understanding;
mod content_understanding_deep;
mod creator_account_research;
mod creator_research;
mod note_analysis;
mod opportunity_sync;
mod research_call;
mod topic_research;
mod work_comparison;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DrainReceipt {
    pub instance_ref: Uuid,
    pub generation: String,
    pub process_id: u32,
    pub source_revision: String,
}
/// Saving configuration or starting this process never enables execution controls.
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
    let (work_db, work_drain) = (db.clone(), drain.clone());
    let opportunity_sync = opportunity_sync::OpportunitySync::from_environment();
    let mut work = JoinSet::new();
    work.spawn(async move {
        work_loop(
            &work_db,
            fence,
            concurrency,
            secrets,
            adapter,
            work_drain,
            opportunity_sync,
        )
        .await
    });
    let mut stopping = drain.subscribe();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut deadline = None;
    loop {
        if drain.is_requested() && deadline.is_none() {
            deadline = Some(Instant::now() + Duration::from_secs(AI_LIMITS.drain_grace_seconds()));
            tokio::time::timeout(Duration::from_secs(5), session.request_drain())
                .await
                .map_err(|_| ExecutorError::HeartbeatTimedOut)??;
        }
        tokio::select! {
            biased;
            _=stopping.changed(),if deadline.is_none()=>{},
            _=tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)),if deadline.is_some()=>{return Err(ExecutorError::DrainTimedOut);},
            _=heartbeat.tick()=>{tokio::time::timeout(Duration::from_secs(5),session.heartbeat()).await.map_err(|_|ExecutorError::HeartbeatTimedOut)??;},
            result=work.join_next()=>{result.ok_or(ExecutorError::TaskFailed)?.map_err(|_|ExecutorError::TaskFailed)??;break;}
        }
    }
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
    opportunity_sync: Option<Arc<opportunity_sync::OpportunitySync>>,
) -> Result<(), ExecutorError> {
    let mut tasks = JoinSet::new();
    let mut opportunity_jobs = JoinSet::new();
    let mut failure = None;
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut recovery = Instant::now();
    let mut automation_scan = Instant::now();
    let mut automation_cursor = None;
    loop {
        if drain.is_requested() && tasks.is_empty() {
            return failure.map_or(Ok(()), Err);
        }
        tokio::select! {
            result=opportunity_jobs.join_next(),if !opportunity_jobs.is_empty()=>{
                if let Some(Err(error))=result {
                    if let Some(sync)=&opportunity_sync { sync.defer(&error); }
                }
            },
            result=tasks.join_next(),if !tasks.is_empty()=>{
                let result=result.expect("a nonempty JoinSet yields a task").map_err(|_|ExecutorError::TaskFailed).and_then(|r|r);
                if let Err(error)=result{failure.get_or_insert(error);drain.request();}
            },
            _=poll.tick()=>{
                if drain.is_requested(){continue;}
                if Instant::now() >= automation_scan {
                    if opportunity_jobs.is_empty() && let Some(sync) = &opportunity_sync {
                        let sync = sync.clone();
                        let db = db.clone();
                        let opportunity_drain = drain.clone();
                        opportunity_jobs.spawn(async move {
                            if !sync.ensure_gate(&db, &opportunity_drain).await {
                                return;
                            }
                            if tokio::time::timeout(
                                Duration::from_secs(AI_LIMITS.drain_grace_seconds()),
                                sync.run_if_pending(&db),
                            ).await.is_err() {
                                sync.defer(&"projection timed out");
                            }
                        });
                    }
                    // Queue production is isolated from execution: one bad source is not a drain request.
                    if let Err(error) = crate::ai_foundation::topic_research_automation::tick(db, &mut automation_cursor).await {
                        eprintln!("topic automation scan deferred: {error}");
                    }
                    automation_scan = Instant::now() + Duration::from_secs(5);
                }
                if let Err(error)=fill_slots(db,fence,concurrency,&mut tasks,&secrets,&adapter,&drain,&mut recovery,&opportunity_sync).await{
                    failure.get_or_insert(error);drain.request();
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
    opportunity_sync: &Option<Arc<opportunity_sync::OpportunitySync>>,
) -> Result<(), ExecutorError> {
    if Instant::now() >= *recovery {
        queue::recover_expired(db, fence).await?;
        *recovery = Instant::now() + Duration::from_secs(5);
    }
    while tasks.len() < concurrency as usize && !drain.is_requested() {
        let claimed = match queue::claim_next(db, fence).await {
            Err(ExecutorError::FenceLost) if drain.is_requested() => {
                fence.begin(db, false).await?.commit().await?;
                break;
            }
            result => result?,
        };
        let Some(item) = claimed else { break };
        if drain.is_requested() {
            item.defer(db, "executor_draining").await?;
            break;
        }
        let (db, secrets, adapter, opportunity_sync) = (
            db.clone(),
            secrets.clone(),
            adapter.clone(),
            opportunity_sync.clone(),
        );
        tasks.spawn(async move {
            let result = match item.task_key {
                TaskKey::ContentUnderstanding => {
                    content_understanding::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::ContentUnderstandingDeep => {
                    content_understanding_deep::execute(&db, &item, secrets.as_ref(), &adapter)
                        .await
                }
                TaskKey::NoteInterpretation => {
                    note_analysis::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::ModelProbe => {
                    probe::execute_probe(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::OcrRecognition => {
                    ocr_recognition::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::WritingLegacy | TaskKey::CreationTitle => {
                    writing::execute_writing(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::WorkComparison => {
                    work_comparison::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::CommentProblemResolution | TaskKey::CommentProblemPairReview => {
                    comment_problem::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::CommentStudy => {
                    comment_research::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::CreatorAccountResearch => {
                    creator_account_research::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::CreatorResearch => {
                    creator_research::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                task if task.topic_stage().is_some() => {
                    topic_research::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                _ => Err(ExecutorError::Invalid),
            };
            if item.task_key == TaskKey::CommentProblemPairReview
                && result.is_ok()
                && let Some(sync) = opportunity_sync
            {
                sync.request();
            }
            result
        });
    }
    Ok(())
}
