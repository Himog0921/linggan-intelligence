//! Bounded executor for the clean comment-study batch lifecycle.
//!
//! A worker never selects comments or creates a StudyRun (that is `prepare_study_run`, driven by
//! the user's own choice of notes and budget) and never uses a historical execution path. It does
//! decide which already-frozen, already user-authorized run's queued targets get packaged into a
//! batch next (`prepare_next_batch_across_runs`), and it claims already-prepared batches.

use crate::{
    comment_study_batch::{MAX_TARGETS_PER_BATCH, PrepareStudyBatchRequest, StudyBatchError,
        next_run_needing_batch, prepare_study_batch},
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
    // Ahead of the resolution worker: a comparison the cache can answer never becomes a call.
    if crate::comment_study_comparison_cache::serve_pending_resolutions_from_cache(database)
        .await
        .map_err(|_| ModelError::Conflict)?
        > 0
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
    if prepare_next_batch_across_runs(database).await? {
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

/// A run's oldest queued target can be too large for its own configured model's input budget:
/// `fit_targets_to_model_budget` then keeps every candidate out of the batch, and
/// `prepare_study_batch` reports `NoQueuedTargets` even though queued targets still exist. Because
/// `next_run_needing_batch` always picks the *oldest* run with a queued target, retrying without
/// excluding that run would pick the exact same unbatchable run forever, on every tick, and no
/// run created after it would ever get a turn (head-of-line blocking) — silently, since a stuck
/// run's targets just stay `queued` with no error and no log line.
///
/// This tries up to `MAX_ATTEMPTS_PER_TICK` distinct runs in one tick, excluding each one that
/// fails to produce a batch, so a run behind a stuck one still gets processed. It does not resolve
/// the stuck run's target itself (that needs the real chunking work described in the roadmap, not
/// a scheduling fix); it only stops that run from starving every other run's queue.
const MAX_BATCH_PREPARATION_ATTEMPTS_PER_TICK: usize = 5;

pub async fn prepare_next_batch_across_runs(database: &Database) -> Result<bool, ModelError> {
    let mut skipped_run_refs = Vec::new();
    while skipped_run_refs.len() < MAX_BATCH_PREPARATION_ATTEMPTS_PER_TICK {
        let Some(run_ref) = next_run_needing_batch(database, &skipped_run_refs)
            .await
            .map_err(ModelError::Database)?
        else {
            return Ok(false);
        };
        match prepare_study_batch(
            database,
            PrepareStudyBatchRequest {
                run_ref,
                maximum_targets: MAX_TARGETS_PER_BATCH,
            },
        )
        .await
        {
            Ok(_) => return Ok(true),
            Err(StudyBatchError::NoQueuedTargets) => {
                println!(
                    "linggan worker: run {run_ref} has queued targets that do not fit the \
                     configured model's input budget; trying the next run this tick"
                );
                skipped_run_refs.push(run_ref);
            }
            // Another worker tick (or a concurrent run) already moved this run out of a batchable
            // state between the check above and this call; nothing is wrong, try another run.
            Err(StudyBatchError::RunUnavailable) => skipped_run_refs.push(run_ref),
            Err(StudyBatchError::Database(error)) => return Err(ModelError::Database(error)),
            Err(other) => {
                println!("linggan worker: batch preparation rejected for run {run_ref}: {other}");
                skipped_run_refs.push(run_ref);
            }
        }
    }
    Ok(false)
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
