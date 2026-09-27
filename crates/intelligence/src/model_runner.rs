//! Bounded executor for the clean comment-study batch lifecycle.
//!
//! A worker never selects comments or creates a StudyRun (that is `prepare_study_run`, driven by
//! the user's own choice of notes and budget) and never uses a historical execution path. It does
//! decide which already-frozen, already user-authorized run's queued targets get packaged into a
//! batch next (`prepare_next_batch_across_runs`), and it claims already-prepared batches.

use crate::{
    comment_study_batch::{
        MAX_TARGETS_PER_BATCH, PrepareStudyBatchRequest, StudyBatchError, prepare_study_batch,
    },
    comment_study_batch_acceptance::{accept_study_batch_output, reject_study_batch_input_limit},
    comment_study_batch_worker::{
        DEFAULT_BATCH_LEASE_SECONDS, StudyBatchWorkerError, claim_next_study_batch_after,
        recover_expired_study_batch_leases,
    },
    comment_study_candidate_recall::{
        advance_next_problem_pair_for_enabled_v2_run,
        advance_next_problem_resolution_for_enabled_v2_run,
    },
    comment_study_embedding::{EmbeddingError, EmbeddingOutcome, embed_pending_signals},
    comment_study_model_dispatch::StudyModelDispatchError,
    comment_study_model_runner::{StudyModelRunnerError, call_study_batch_model},
    comment_study_pair_worker::{PairExecution, run_one_problem_pair_with_outcome},
    comment_study_read,
    comment_study_resolution_worker::{
        ResolutionExecution, run_one_problem_resolution_with_outcome,
    },
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
    let mut fairness = ModelWorkerFairness::default();
    run_model_work_once_with_fairness(database, store, adapter, &mut fairness).await
}

/// Process-local scheduling state for model lanes and StudyRun selection. Restarting the worker
/// resets only the next-item order; it does not change persisted work or its authorization.
#[derive(Default)]
pub struct ModelWorkerFairness {
    next_lane_index: usize,
    last_run_ref: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelWorkLane {
    Semantic,
    Resolution,
    Pair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaneProgress {
    Idle,
    Local,
    ProviderAttempt,
}

fn resolution_lane_progress(outcome: ResolutionExecution, prepared: bool) -> LaneProgress {
    match outcome {
        ResolutionExecution::ProviderAttempt { .. } => LaneProgress::ProviderAttempt,
        ResolutionExecution::Idle if prepared => LaneProgress::Local,
        ResolutionExecution::Idle => LaneProgress::Idle,
    }
}

fn pair_lane_progress(outcome: PairExecution, prepared: bool) -> LaneProgress {
    match outcome {
        PairExecution::ProviderAttempt { .. } => LaneProgress::ProviderAttempt,
        PairExecution::LocalProgress => LaneProgress::Local,
        PairExecution::Idle if prepared => LaneProgress::Local,
        PairExecution::Idle => LaneProgress::Idle,
    }
}

impl ModelWorkerFairness {
    fn next_lane(&mut self) -> ModelWorkLane {
        let lane = match self.next_lane_index % 3 {
            0 => ModelWorkLane::Semantic,
            1 => ModelWorkLane::Resolution,
            _ => ModelWorkLane::Pair,
        };
        self.next_lane_index = (self.next_lane_index + 1) % 3;
        lane
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EmbeddingError, EmbeddingOutcome, LaneProgress, ModelError, lane_error_may_continue,
        pair_lane_progress, record_embedding_result, resolution_lane_progress,
    };
    use super::{ModelWorkLane, ModelWorkerFairness, PairExecution, ResolutionExecution};

    #[test]
    fn model_work_lanes_rotate_before_each_attempt() {
        let mut fairness = ModelWorkerFairness::default();
        let actual: Vec<_> = (0..6).map(|_| fairness.next_lane()).collect();
        assert_eq!(
            actual,
            vec![
                ModelWorkLane::Semantic,
                ModelWorkLane::Resolution,
                ModelWorkLane::Pair,
                ModelWorkLane::Semantic,
                ModelWorkLane::Resolution,
                ModelWorkLane::Pair,
            ]
        );
    }

    #[test]
    fn a_lane_error_uses_other_lanes_only_when_no_provider_call_may_have_started() {
        assert!(lane_error_may_continue(&ModelError::SecretUnavailable));
        assert!(lane_error_may_continue(&ModelError::Budget));
        assert!(!lane_error_may_continue(&ModelError::Conflict));
        assert!(!lane_error_may_continue(&ModelError::Source));
        assert!(!lane_error_may_continue(&ModelError::AdapterUnavailable));
        assert!(!lane_error_may_continue(&ModelError::Timeout));
        assert!(!lane_error_may_continue(&ModelError::InvalidOutput));
        assert!(!lane_error_may_continue(&ModelError::Database(
            sqlx::Error::RowNotFound
        )));
    }

    #[test]
    fn local_embedding_failure_does_not_abort_model_lanes() {
        let mut made_progress = false;
        record_embedding_result(
            Err(EmbeddingError::Model(ModelError::AdapterUnavailable)),
            &mut made_progress,
        )
        .unwrap();
        assert!(!made_progress);

        record_embedding_result(
            Ok(EmbeddingOutcome::Encoded {
                encoded: 2,
                pending_after: 0,
            }),
            &mut made_progress,
        )
        .unwrap();
        assert!(made_progress);
    }

    #[test]
    fn rejected_late_provider_responses_still_consume_the_scheduler_call_allowance() {
        let resolution = resolution_lane_progress(
            ResolutionExecution::ProviderAttempt { accepted: false },
            false,
        );
        let pair = pair_lane_progress(PairExecution::ProviderAttempt { accepted: false }, false);

        // The scheduler returns from the tick on ProviderAttempt regardless of acceptance.
        assert_eq!(resolution, LaneProgress::ProviderAttempt);
        assert_eq!(pair, LaneProgress::ProviderAttempt);
    }
}

pub async fn run_model_work_once_with_fairness(
    database: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    fairness: &mut ModelWorkerFairness,
) -> Result<bool, ModelError> {
    // P3 recovery must run before any fresh work. Unsent reservations settle at zero; dispatched
    // requests with unknown usage retain their reservation before their subject can be retried.
    // Recovery is bounded; continue with other lanes even when it found expired work.
    let mut made_progress = false;
    let recovered_stage_requests =
        crate::comment_study_request_ledger::recover_expired_problem_stage_requests(database)
            .await
            .map_err(ModelError::Database)?;
    let recovered_batches = recover_expired_study_batch_leases(database)
        .await
        .map_err(worker_error)?;
    made_progress |= recovered_stage_requests > 0 || recovered_batches > 0;
    // P1 deterministic maintenance shares this existing 10s worker tick. It is bounded and local:
    // no provider call, no Run creation and no second scheduler. On pre-P1 schemas it is a no-op.
    crate::comment_study_catalog::maintain_comment_catalog(database)
        .await
        .map_err(catalog_error)?;
    let problem_stages_ready =
        crate::comment_study_request_ledger::pair_failure_state_supported(database)
            .await
            .map_err(ModelError::Database)?;
    if problem_stages_ready {
        // Ahead of the resolution worker: a comparison the cache can answer never becomes a call.
        made_progress |=
            match crate::comment_study_comparison_cache::serve_pending_resolutions_from_cache(
                database,
            )
            .await
            {
                Ok(served) => served > 0,
                Err(crate::comment_study_comparison_cache::ComparisonCacheError::Database(
                    error,
                )) => {
                    return Err(ModelError::Database(error));
                }
                Err(crate::comment_study_comparison_cache::ComparisonCacheError::Acceptance) => {
                    eprintln!("linggan worker: cached resolution admission failed");
                    true
                }
            };
    }
    // Keep deterministic local embedding available on P1/legacy schemas even when the newer P3
    // request ledger is not installed. An unencoded Signal cannot be recalled; embedding has no
    // provider cost and does not depend on the P3 request/terminal-state migrations. It can share
    // a tick with one model request, so a long embedding backlog cannot starve model stages.
    record_embedding_result(
        embed_pending_signals(database, adapter).await,
        &mut made_progress,
    )?;

    for _ in 0..3 {
        let lane = fairness.next_lane();
        let progress = match lane {
            ModelWorkLane::Semantic => {
                run_semantic_lane_once(database, store, adapter, fairness).await
            }
            ModelWorkLane::Resolution if problem_stages_ready => {
                let prepared =
                    match advance_next_problem_resolution_for_enabled_v2_run(database).await {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            let error = candidate_recall_error(error);
                            if matches!(error, ModelError::Database(_)) {
                                return Err(error);
                            }
                            eprintln!(
                                "linggan worker: {:?} lane deferred ({})",
                                lane,
                                error.code()
                            );
                            made_progress = true;
                            continue;
                        }
                    };
                run_one_problem_resolution_with_outcome(database, store, adapter)
                    .await
                    .map(|outcome| resolution_lane_progress(outcome, prepared))
                    .map_err(|error| match error {
                        crate::comment_study_resolution_worker::ResolutionWorkerError::Model(
                            error,
                        ) => error,
                        crate::comment_study_resolution_worker::ResolutionWorkerError::Database(
                            error,
                        ) => ModelError::Database(error),
                        crate::comment_study_resolution_worker::ResolutionWorkerError::Idle
                        | crate::comment_study_resolution_worker::ResolutionWorkerError::Manifest => {
                            ModelError::Invalid
                        }
                    })
            }
            ModelWorkLane::Pair if problem_stages_ready => {
                let prepared = match advance_next_problem_pair_for_enabled_v2_run(database).await {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        let error = candidate_recall_error(error);
                        if matches!(error, ModelError::Database(_)) {
                            return Err(error);
                        }
                        eprintln!(
                            "linggan worker: {:?} lane deferred ({})",
                            lane,
                            error.code()
                        );
                        made_progress = true;
                        continue;
                    }
                };
                run_one_problem_pair_with_outcome(database, store, adapter)
                    .await
                    .map(|outcome| pair_lane_progress(outcome, prepared))
                    .map_err(|error| match error {
                        crate::comment_study_pair_worker::PairWorkerError::Model(error) => error,
                        crate::comment_study_pair_worker::PairWorkerError::Database(error) => {
                            ModelError::Database(error)
                        }
                        crate::comment_study_pair_worker::PairWorkerError::Store(
                            crate::comment_study_problem_store::ProblemStoreError::Database(error),
                        ) => ModelError::Database(error),
                        crate::comment_study_pair_worker::PairWorkerError::Store(_) => {
                            ModelError::Conflict
                        }
                        crate::comment_study_pair_worker::PairWorkerError::Manifest => {
                            ModelError::Invalid
                        }
                    })
            }
            ModelWorkLane::Resolution | ModelWorkLane::Pair => Ok(LaneProgress::Idle),
        };
        match progress {
            Err(error) if matches!(error, ModelError::Database(_)) => return Err(error),
            Err(error) if lane_error_may_continue(&error) => {
                eprintln!(
                    "linggan worker: {:?} lane deferred ({})",
                    lane,
                    error.code()
                );
                made_progress = true;
            }
            Err(error) => {
                // The lane may have crossed the provider dispatch fence. Conservatively consume
                // this tick's one-call allowance even when the response was rejected or failed.
                eprintln!(
                    "linggan worker: {:?} lane ended tick ({})",
                    lane,
                    error.code()
                );
                return Ok(true);
            }
            Ok(LaneProgress::Idle) => {}
            Ok(LaneProgress::Local) => made_progress = true,
            Ok(LaneProgress::ProviderAttempt) => return Ok(true),
        }
    }
    Ok(made_progress)
}

fn record_embedding_result(
    result: Result<EmbeddingOutcome, EmbeddingError>,
    made_progress: &mut bool,
) -> Result<(), ModelError> {
    match result {
        Ok(EmbeddingOutcome::Encoded { encoded, .. }) => *made_progress |= encoded > 0,
        Ok(EmbeddingOutcome::Stood { .. }) => {}
        Err(EmbeddingError::Database(error)) => return Err(ModelError::Database(error)),
        Err(EmbeddingError::Model(error)) => {
            eprintln!(
                "linggan worker: local embedding unavailable ({})",
                error.code()
            );
        }
        Err(EmbeddingError::InvalidVector) => {
            eprintln!("linggan worker: local embedding returned an invalid vector");
        }
    }
    Ok(())
}

fn lane_error_may_continue(error: &ModelError) -> bool {
    matches!(
        error,
        ModelError::SelectionLimit
            | ModelError::Invalid
            | ModelError::NotFound
            | ModelError::Disabled
            | ModelError::SecretUnavailable
            | ModelError::InputLimit
            | ModelError::Budget
            | ModelError::NotQualified
            | ModelError::SchemaMissing
    )
}

fn batch_acceptance_error(
    error: crate::comment_study_batch_acceptance::BatchAcceptanceError,
) -> ModelError {
    match error {
        crate::comment_study_batch_acceptance::BatchAcceptanceError::Database(error) => {
            ModelError::Database(error)
        }
        crate::comment_study_batch_acceptance::BatchAcceptanceError::BatchContract(_) => {
            ModelError::InvalidOutput
        }
        crate::comment_study_batch_acceptance::BatchAcceptanceError::BatchUnavailable => {
            ModelError::Conflict
        }
    }
}

fn candidate_recall_error(
    error: crate::comment_study_candidate_recall::ProblemCandidateRecallError,
) -> ModelError {
    match error {
        crate::comment_study_candidate_recall::ProblemCandidateRecallError::Database(error) => {
            ModelError::Database(error)
        }
        crate::comment_study_candidate_recall::ProblemCandidateRecallError::Store(
            crate::comment_study_problem_store::ProblemStoreError::Database(error),
        ) => ModelError::Database(error),
        _ => ModelError::Conflict,
    }
}

async fn run_semantic_lane_once(
    database: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    fairness: &mut ModelWorkerFairness,
) -> Result<LaneProgress, ModelError> {
    let claim = claim_next_study_batch_after(
        database,
        Uuid::new_v4(),
        DEFAULT_BATCH_LEASE_SECONDS,
        &mut fairness.last_run_ref,
    )
    .await
    .map_err(worker_error)?;
    let Some(claim) = claim else {
        return if prepare_next_batch_across_runs_with_fairness(database, fairness).await? {
            Ok(LaneProgress::Local)
        } else {
            Ok(LaneProgress::Idle)
        };
    };
    let output = match call_study_batch_model(
        database,
        store,
        adapter,
        claim.batch_ref,
        claim.lease_token,
    )
    .await
    {
        Ok(output) => output,
        Err(StudyModelRunnerError::Dispatch(StudyModelDispatchError::BudgetDeferred))
        | Err(StudyModelRunnerError::Dispatch(StudyModelDispatchError::PreDispatchDeferred)) => {
            return Ok(LaneProgress::Local);
        }
        Err(StudyModelRunnerError::Dispatch(StudyModelDispatchError::InputLimit)) => {
            reject_study_batch_input_limit(database, claim.batch_ref, claim.lease_token)
                .await
                .map_err(batch_acceptance_error)?;
            return Ok(LaneProgress::Local);
        }
        Err(error) => return Err(runner_error(error)),
    };
    accept_study_batch_output(database, claim.batch_ref, claim.lease_token, output.output)
        .await
        .map_err(batch_acceptance_error)?;
    Ok(LaneProgress::ProviderAttempt)
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
    let mut fairness = ModelWorkerFairness::default();
    prepare_next_batch_across_runs_with_fairness(database, &mut fairness).await
}

pub async fn prepare_next_batch_across_runs_with_fairness(
    database: &Database,
    fairness: &mut ModelWorkerFairness,
) -> Result<bool, ModelError> {
    let mut skipped_run_refs = Vec::new();
    while skipped_run_refs.len() < MAX_BATCH_PREPARATION_ATTEMPTS_PER_TICK {
        let Some(run_ref) = crate::comment_study_batch::next_run_needing_batch_after(
            database,
            &skipped_run_refs,
            fairness.last_run_ref,
        )
        .await
        .map_err(ModelError::Database)?
        else {
            return Ok(false);
        };
        fairness.last_run_ref = Some(run_ref);
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
    let mut fairness = ModelWorkerFairness::default();
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
        match run_model_work_once_with_fairness(&database, store.as_ref(), &adapter, &mut fairness)
            .await
        {
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

fn catalog_error(error: crate::comment_study_catalog::StudyCatalogError) -> ModelError {
    match error {
        crate::comment_study_catalog::StudyCatalogError::Database(error) => {
            ModelError::Database(error)
        }
        _ => ModelError::Conflict,
    }
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
        StudyModelRunnerError::Dispatch(StudyModelDispatchError::BudgetExhausted) => {
            ModelError::Budget
        }
        StudyModelRunnerError::Dispatch(StudyModelDispatchError::BudgetDeferred) => {
            ModelError::Conflict
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
