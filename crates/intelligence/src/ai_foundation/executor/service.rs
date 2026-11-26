//! The dedicated AI process. Its session heartbeat runs while claims and child calls await.
use super::{ExecutorError, ExecutorSession, ocr_recognition, probe, queue, schema, writing};
use crate::{
    ai_foundation::TaskKey, model_secrets::ModelSecretStore, model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_evidence::selected_collection_run;
use linggan_storage_postgres::Database;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    task::JoinSet,
    time::{Instant, MissedTickBehavior},
};
use uuid::Uuid;
mod comment_problem_consolidation;
mod comment_understanding;
mod content_understanding;
mod content_understanding_deep;
mod creator_account_research;
mod creator_research;
mod opportunity_sync;
mod ops_title_label;
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
    let opportunity_sync = (!selected_collection_run::configured())
        .then(opportunity_sync::OpportunitySync::from_environment)
        .flatten();
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
    let mut corpus_jobs = JoinSet::new();
    let corpus_db = db.clone();
    let corpus_drain = drain.clone();
    corpus_jobs.spawn(async move { scan_corpus_automation(&corpus_db, &corpus_drain).await });
    let mut opportunity_jobs = JoinSet::new();
    let mut selection_jobs = JoinSet::new();
    let mut lens_jobs = JoinSet::new();
    let mut failure = None;
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut recovery = Instant::now();
    let mut automation_scan = Instant::now();
    let scoped = selected_collection_run::configured();
    let lens_enabled = !scoped && crate::ops_content_lens::enabled_from_env();
    loop {
        if drain.is_requested() && tasks.is_empty() {
            corpus_jobs.shutdown().await;
            selection_jobs.shutdown().await;
            lens_jobs.shutdown().await;
            return failure.map_or(Ok(()), Err);
        }
        tokio::select! {
            result=corpus_jobs.join_next(),if !corpus_jobs.is_empty()=>{
                if let Some(Err(error)) = result {
                    eprintln!("corpus automation scan interrupted: {error}");
                    failure.get_or_insert(ExecutorError::TaskFailed);
                    drain.request();
                }
            },
            result=opportunity_jobs.join_next(),if !opportunity_jobs.is_empty()=>{
                if let Some(Err(error))=result {
                    if let Some(sync)=&opportunity_sync { sync.defer(&error); }
                }
            },
            result=lens_jobs.join_next(),if !lens_jobs.is_empty()=>{
                if let Some(Err(error))=result {
                    eprintln!("content lens refresh interrupted: {error}");
                }
            },
            result=selection_jobs.join_next(),if !selection_jobs.is_empty()=>{
                if let Some(Err(error))=result {
                    eprintln!("comment research selection scan interrupted: {error}");
                }
            },
            result=tasks.join_next(),if !tasks.is_empty()=>{
                let result=result.expect("a nonempty JoinSet yields a task").map_err(|_|ExecutorError::TaskFailed).and_then(|r|r);
                if let Err(error)=result{failure.get_or_insert(error);drain.request();}
            },
            _=poll.tick()=>{
                if drain.is_requested(){continue;}
                if !scoped && Instant::now() >= automation_scan {
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
                    if lens_enabled && lens_jobs.is_empty() {
                        let lens_db = db.clone();
                        lens_jobs.spawn(async move {
                            if let Err(error) = crate::ops_content_lens::label::advance(&lens_db).await {
                                eprintln!("content lens follow-up deferred: {error}");
                            }
                            if let Err(error) = crate::ops_content_lens::refresh_pending(&lens_db).await {
                                eprintln!("content lens refresh deferred: {error}");
                            }
                        });
                    }
                    if selection_jobs.is_empty() {
                        let selection_db = db.clone();
                        selection_jobs.spawn(async move {
                            if let Err(error) = crate::ai_foundation::comment_understanding::automation::tick(&selection_db).await {
                                eprintln!("comment research selection scan deferred: {error}");
                            }
                        });
                    }
                    if let Err(error) = crate::ai_foundation::comment_understanding::consolidation_queue::tick(db).await {
                        eprintln!("comment consolidation scan deferred: {error}");
                    }
                    automation_scan = Instant::now() + Duration::from_secs(5);
                }
                if let Err(error)=fill_slots(db,fence,concurrency,&mut tasks,&secrets,&adapter,&drain,&mut recovery,&opportunity_sync,lens_enabled).await{
                    failure.get_or_insert(error);drain.request();
                }
            }
        }
    }
}

/// Source reads may take longer than the dispatch poll. Keep the reconciliation
/// cursors together, without holding up claims, comment schedules or settlements.
async fn scan_corpus_automation(db: &Database, drain: &ModelWorkerDrain) {
    let mut stopping = drain.subscribe();
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut topic_cursor = None;
    let mut understanding_cursor = None;
    while !drain.is_requested() {
        tokio::select! {
            biased;
            _ = stopping.changed() => break,
            _ = tick.tick() => {}
        }
        if let Err(error) = crate::ai_foundation::topic_research_automation::tick(db, &mut topic_cursor).await {
            eprintln!("topic automation scan deferred: {error}");
        }
        if !drain.is_requested()
            && let Err(error) = crate::ai_foundation::content_understanding::automation_tick(db, &mut understanding_cursor).await
        {
            eprintln!("content understanding automation scan deferred: {error}");
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
    lens_enabled: bool,
) -> Result<(), ExecutorError> {
    if Instant::now() >= *recovery {
        queue::recover_expired(db, fence).await?;
        *recovery = Instant::now() + Duration::from_secs(5);
    }
    while tasks.len() < concurrency as usize && !drain.is_requested() {
        let claimed = match queue::claim_with_lens(db, fence, lens_enabled).await {
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
                TaskKey::OpsTitleLabel => ops_title_label::execute(&db, &item, secrets.as_ref(), &adapter).await,
                TaskKey::CommentProblemConsolidation => {
                    comment_problem_consolidation::execute(&db, &item, secrets.as_ref(), &adapter).await
                }
                TaskKey::CommentUnderstanding => {
                    comment_understanding::execute(&db, &item, secrets.as_ref(), &adapter).await
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
            if result.is_ok() && let Some(sync) = opportunity_sync {
                if matches!(item.task_key, TaskKey::CommentUnderstanding | TaskKey::CommentProblemConsolidation) {
                    let succeeded = sqlx::query_scalar(
                        "SELECT state='succeeded' FROM linggan_ai_batch_item_control WHERE item_ref=$1",
                    )
                    .bind(item.item_ref)
                    .fetch_one(db.pool())
                    .await;
                    match succeeded {
                        Ok(true) => sync.request(),
                        Ok(false) => {}
                        Err(error) => sync.defer(&error),
                    }
                }
            }
            result
        });
    }
    Ok(())
}
