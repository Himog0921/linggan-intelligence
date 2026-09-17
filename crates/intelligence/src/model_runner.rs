//! Bounded executor for the clean comment-study batch lifecycle.
//!
//! A worker only claims already-prepared batches. It never selects comments, creates a StudyRun,
//! or uses a historical execution path.

use crate::{
    comment_study_batch_acceptance::accept_study_batch_output,
    comment_study_batch_worker::{
        DEFAULT_BATCH_LEASE_SECONDS, StudyBatchWorkerError, claim_next_study_batch,
        recover_expired_study_batch_leases,
    },
    comment_study_candidate_recall::{advance_next_problem_pair, advance_next_problem_resolution},
    comment_study_model_dispatch::StudyModelDispatchError,
    comment_study_model_runner::{StudyModelRunnerError, call_study_batch_model},
    comment_study_pair_worker::run_one_problem_pair,
    comment_study_read,
    comment_study_resolution_worker::run_one_problem_resolution,
    model_secrets::{ModelSecretStore, model_secret_store},
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

pub async fn run_model_work_once(
    database: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    if run_one_problem_pair(database, store, adapter)
        .await
        .map_err(|error| match error {
            crate::comment_study_pair_worker::PairWorkerError::Model(error) => error,
            crate::comment_study_pair_worker::PairWorkerError::Database(error) => {
                ModelError::Database(error)
            }
            crate::comment_study_pair_worker::PairWorkerError::Manifest => ModelError::Conflict,
        })?
    {
        return Ok(true);
    }
    if run_one_problem_resolution(database, store, adapter)
        .await
        .map_err(|error| match error {
            crate::comment_study_resolution_worker::ResolutionWorkerError::Model(error) => error,
            crate::comment_study_resolution_worker::ResolutionWorkerError::Database(error) => {
                ModelError::Database(error)
            }
            crate::comment_study_resolution_worker::ResolutionWorkerError::Idle
            | crate::comment_study_resolution_worker::ResolutionWorkerError::Manifest => {
                ModelError::Conflict
            }
        })?
    {
        return Ok(true);
    }
    if advance_next_problem_resolution(database)
        .await
        .map_err(|_| ModelError::Conflict)?
    {
        return Ok(true);
    }
    if advance_next_problem_pair(database)
        .await
        .map_err(|_| ModelError::Conflict)?
    {
        return Ok(true);
    }
    recover_expired_study_batch_leases(database)
        .await
        .map_err(worker_error)?;
    let Some(claim) = claim_next_study_batch(database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .map_err(worker_error)?
    else {
        return Ok(false);
    };
    let output =
        call_study_batch_model(database, store, adapter, claim.batch_ref, claim.lease_token)
            .await
            .map_err(runner_error)?;
    accept_study_batch_output(database, claim.batch_ref, claim.lease_token, output.output)
        .await
        .map_err(|_| ModelError::InvalidOutput)?;
    Ok(true)
}

pub async fn model_schema_ready(database: &Database) -> bool {
    comment_study_read::schema_ready(database)
        .await
        .unwrap_or(false)
}

pub async fn run_model_worker(database: Database) {
    let _ = run_model_worker_with_drain(database, ModelWorkerDrain::new()).await;
}

pub async fn run_model_worker_with_drain(
    database: Database,
    drain: ModelWorkerDrain,
) -> Result<(), ModelError> {
    let store = model_secret_store();
    let adapter = PiAdapter::configured();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    let mut shutdown = drain.subscribe();
    loop {
        if drain.is_requested() {
            break;
        }
        tokio::select! {
            _ = interval.tick() => {}
            changed = shutdown.changed() => { if changed.is_ok() && *shutdown.borrow() { break; } }
        }
        if drain.is_requested() || !model_schema_ready(&database).await {
            continue;
        }
        model_worker_heartbeat(&database, "running", None).await?;
        match run_model_work_once(&database, store.as_ref(), &adapter).await {
            Ok(_) => model_worker_heartbeat(&database, "idle", None).await?,
            Err(error) => {
                model_worker_heartbeat(&database, "error", Some(error.code())).await?;
                if drain.is_requested() {
                    return Err(error);
                }
            }
        }
    }
    model_worker_heartbeat(&database, "idle", None).await
}

fn worker_error(error: StudyBatchWorkerError) -> ModelError {
    match error {
        StudyBatchWorkerError::Database(error) => ModelError::Database(error),
        StudyBatchWorkerError::InvalidLeaseDuration => ModelError::Invalid,
    }
}

fn runner_error(error: StudyModelRunnerError) -> ModelError {
    match error {
        StudyModelRunnerError::Model(error) => error,
        StudyModelRunnerError::Database(error) => ModelError::Database(error),
        StudyModelRunnerError::Dispatch(StudyModelDispatchError::Database(error)) => {
            ModelError::Database(error)
        }
        StudyModelRunnerError::Dispatch(StudyModelDispatchError::InputLimit) => {
            ModelError::InputLimit
        }
        StudyModelRunnerError::Dispatch(_) | StudyModelRunnerError::BatchUnavailable => {
            ModelError::Conflict
        }
        StudyModelRunnerError::OutputNotJson => ModelError::InvalidOutput,
        StudyModelRunnerError::ProviderFailure => ModelError::AdapterUnavailable,
    }
}

pub async fn model_worker_heartbeat(
    database: &Database,
    state: &str,
    error: Option<&str>,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_model_workspace \
         SET worker_last_seen_at=scope_001_now(),worker_state=$1,worker_last_error=$2 \
         WHERE singleton",
    )
    .bind(state)
    .bind(error)
    .execute(database.pool())
    .await?;
    Ok(())
}
