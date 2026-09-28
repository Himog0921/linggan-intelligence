//! Provider-backed resolution of one frozen, non-empty Problem candidate set.

use crate::{
    comment_study_model_runner::parse_provider_json,
    comment_study_policy::{
        CompiledStudyMethod, StudyMethodManifest, StudyModelIdentity, StudyModelSnapshot,
        verify_study_method,
    },
    comment_study_problem_resolution::PROBLEM_RESOLUTION_CONTRACT,
    comment_study_problem_store::{ProblemStoreError, accept_problem_resolution_from_invocation},
    comment_study_request_ledger::{
        ProblemStageSubject, RequestLedgerError, mark_problem_stage_dispatch_started,
        problem_stage_request_manifest, release_problem_stage_after_failure,
        release_problem_stage_before_dispatch, reserve_problem_stage_call,
    },
    model_invocation::{checkpoint_invocation_usage, connection_request, finish_invocation},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::{PiAdapter, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ResolutionWorkerError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("no pending candidate comparison is available")]
    Idle,
    #[error("the stored candidate manifest is malformed")]
    Manifest,
}

/// Reports whether this invocation crossed the provider boundary, independently of whether its
/// response was still eligible for admission when it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionExecution {
    Idle,
    ProviderAttempt { accepted: bool },
}

struct ClaimedResolution {
    resolution_ref: Uuid,
    signal_ref: Uuid,
    invocation_ref: Uuid,
    connection_version_ref: Uuid,
    model_id: String,
    timeout_seconds: i32,
    output_token_limit: i32,
    system_instruction: String,
    prompt: String,
}

/// Claims and resolves one pending, non-empty candidate comparison. The invocation reference is
/// persisted on the Resolution before provider I/O, so a retry cannot race into a second call.
pub async fn run_one_problem_resolution(
    database: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ResolutionWorkerError> {
    run_one_problem_resolution_with_outcome(database, secrets, adapter)
        .await
        .map(|outcome| {
            matches!(
                outcome,
                ResolutionExecution::ProviderAttempt { accepted: true }
            )
        })
}

/// Detailed counterpart used by the scheduler to enforce its per-tick provider-call limit.
pub async fn run_one_problem_resolution_with_outcome(
    database: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<ResolutionExecution, ResolutionWorkerError> {
    if !crate::comment_study_request_ledger::pair_failure_state_supported(database).await? {
        return Ok(ResolutionExecution::Idle);
    }
    let Some(claim) = claim_resolution(database).await? else {
        return Ok(ResolutionExecution::Idle);
    };
    let mut request =
        match connection_request(database, secrets, claim.connection_version_ref).await {
            Ok(request) => request,
            Err(error) => {
                release_pre_dispatch_claim(database, &claim, error.code()).await?;
                return Err(ResolutionWorkerError::Model(error));
            }
        };
    request.operation = "analyze".into();
    request.model_id = claim.model_id.clone();
    request.timeout_ms = match u64::try_from(claim.timeout_seconds) {
        Ok(seconds) => seconds.saturating_mul(1_000),
        Err(_) => {
            release_pre_dispatch_claim(database, &claim, "invalid_model_command").await?;
            return Err(ResolutionWorkerError::Model(ModelError::Invalid));
        }
    };
    request.max_output_tokens = claim.output_token_limit;
    request.system = claim.system_instruction.clone();
    request.prompt = claim.prompt.clone();
    if let Err(error) = mark_problem_stage_dispatch_started(
        database,
        ProblemStageSubject::Resolution(claim.resolution_ref),
        claim.invocation_ref,
    )
    .await
    {
        release_pre_dispatch_claim(database, &claim, "dispatch_fence_failed").await?;
        return Err(ResolutionWorkerError::Model(
            if matches!(error, sqlx::Error::RowNotFound) {
                ModelError::Conflict
            } else {
                ModelError::Database(error)
            },
        ));
    }
    let response = adapter.call(&request).await;
    match response {
        Ok(response) if response.ok => {
            let raw = match parse_provider_json(response.text.as_deref()) {
                Ok(raw) => raw,
                Err(_) => {
                    finish_failure(
                        database,
                        claim.invocation_ref,
                        Some(&response),
                        "resolution_output_not_json",
                    )
                    .await?;
                    release_problem_stage_after_failure(
                        database,
                        ProblemStageSubject::Resolution(claim.resolution_ref),
                        claim.invocation_ref,
                        "resolution_output_not_json",
                    )
                    .await?;
                    return Err(ResolutionWorkerError::Model(ModelError::InvalidOutput));
                }
            };
            checkpoint_invocation_usage(database, claim.invocation_ref, Some(&response)).await?;
            match accept_problem_resolution_from_invocation(
                database,
                claim.resolution_ref,
                claim.invocation_ref,
                raw.clone(),
            )
            .await
            {
                Ok(_) => {
                    // Cache only an answer admitted by the still-active request fence.
                    let _ = crate::comment_study_comparison_cache::record_resolution_comparisons(
                        database,
                        claim.signal_ref,
                        &raw,
                        Some(claim.invocation_ref),
                    )
                    .await;
                    finish_invocation(database, claim.invocation_ref, Some(&response), true, None, &json!({"contract":PROBLEM_RESOLUTION_CONTRACT,"resolutionRef":claim.resolution_ref,"accepted":true})).await?;
                    Ok(ResolutionExecution::ProviderAttempt { accepted: true })
                }
                Err(ProblemStoreError::ModelRequestUnavailable) => {
                    Ok(ResolutionExecution::ProviderAttempt { accepted: false })
                }
                Err(ProblemStoreError::Contract(_)) => {
                    finish_failure(
                        database,
                        claim.invocation_ref,
                        Some(&response),
                        "resolution_contract_rejected",
                    )
                    .await?;
                    Err(ResolutionWorkerError::Model(ModelError::InvalidOutput))
                }
                Err(_) => {
                    finish_failure(
                        database,
                        claim.invocation_ref,
                        Some(&response),
                        "resolution_contract_rejected",
                    )
                    .await?;
                    release_problem_stage_after_failure(
                        database,
                        ProblemStageSubject::Resolution(claim.resolution_ref),
                        claim.invocation_ref,
                        "resolution_contract_rejected",
                    )
                    .await?;
                    Err(ResolutionWorkerError::Model(ModelError::InvalidOutput))
                }
            }
        }
        Ok(response) => {
            finish_failure(
                database,
                claim.invocation_ref,
                Some(&response),
                response
                    .failure_code
                    .as_deref()
                    .unwrap_or("provider_failed"),
            )
            .await?;
            release_problem_stage_after_failure(
                database,
                ProblemStageSubject::Resolution(claim.resolution_ref),
                claim.invocation_ref,
                response
                    .failure_code
                    .as_deref()
                    .unwrap_or("provider_failed"),
            )
            .await?;
            Err(ResolutionWorkerError::Model(ModelError::AdapterUnavailable))
        }
        Err(error) => {
            finish_failure(database, claim.invocation_ref, None, error.code()).await?;
            release_problem_stage_after_failure(
                database,
                ProblemStageSubject::Resolution(claim.resolution_ref),
                claim.invocation_ref,
                error.code(),
            )
            .await?;
            Err(ResolutionWorkerError::Model(error))
        }
    }
}

async fn claim_resolution(
    database: &Database,
) -> Result<Option<ClaimedResolution>, ResolutionWorkerError> {
    let mut tx = database.pool().begin().await?;
    let candidate: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT resolution.resolution_ref,target.run_ref \
         FROM linggan_comment_study_resolution resolution \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         WHERE resolution.state='pending' AND resolution.model_invocation_ref IS NULL \
           AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL \
         ORDER BY resolution.created_at,resolution.resolution_ref LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((resolution_ref, run_ref)) = candidate else {
        tx.commit().await?;
        return Ok(None);
    };
    let locked_run: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run \
         WHERE run_ref=$1 AND to_jsonb(linggan_comment_study_run)->>'dispatch_state'='enabled' \
           AND to_jsonb(linggan_comment_study_run)->>'dispatch_reason' IS NULL \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(run_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if locked_run.is_none() {
        tx.commit().await?;
        return Ok(None);
    }
    let locked_resolution: Option<Uuid> = sqlx::query_scalar(
        "SELECT resolution_ref FROM linggan_comment_study_resolution \
         WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref IS NULL \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(resolution_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if locked_resolution.is_none() {
        tx.commit().await?;
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT resolution.resolution_ref,resolution.signal_ref,resolution.candidate_manifest, \
                signal.proposition,signal.problem_frame,run.run_ref,run.policy_ref, \
                to_jsonb(policy)->'method_manifest' AS method_manifest, \
                to_jsonb(policy)->>'method_hash' AS method_hash, \
                to_jsonb(run)->'execution_manifest' AS execution_manifest, \
                config.config_ref,config.input_token_limit,config.output_token_limit,config.timeout_seconds, \
                config.max_attempts, \
                model.model_ref,model.model_id,version.version_ref,connection.enabled \
         FROM linggan_comment_study_resolution resolution \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
         JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection ON connection.connection_ref=version.connection_ref \
         WHERE resolution.resolution_ref=$1 AND run.run_ref=$2 \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL",
    )
    .bind(resolution_ref)
    .bind(run_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ResolutionWorkerError::Model(ModelError::Conflict))?;
    if !row.get::<bool, _>("enabled") {
        return Err(ResolutionWorkerError::Model(ModelError::Disabled));
    }
    let candidate_manifest = row.get::<Value, _>("candidate_manifest");
    let candidates = candidate_manifest["candidateProblemRefs"]
        .as_array()
        .cloned()
        .ok_or(ResolutionWorkerError::Manifest)?;
    if candidates.is_empty() {
        return Err(ResolutionWorkerError::Manifest);
    }
    let refs = candidates
        .iter()
        .map(|value| value.as_str().and_then(|value| value.parse::<Uuid>().ok()))
        .collect::<Option<Vec<_>>>()
        .ok_or(ResolutionWorkerError::Manifest)?;
    let candidate_revisions = candidate_manifest["candidateProblemRevisions"]
        .as_array()
        .cloned()
        .ok_or(ResolutionWorkerError::Manifest)?;
    if candidate_revisions.len() != refs.len() {
        return Err(ResolutionWorkerError::Manifest);
    }
    let problems: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'problemRef',problem.problem_ref, \
           'problemRevisionRef',revision.revision_ref, \
           'definition',revision.definition, \
           'stableIdentity',revision.core_frame, \
           'includeCriteria',revision.inclusions, \
           'excludeCriteria',revision.exclusions \
         ) \
         FROM jsonb_to_recordset($1::jsonb) AS candidate(\"problemRef\" uuid,\"problemRevisionRef\" uuid) \
         JOIN linggan_comment_study_problem problem \
           ON problem.problem_ref=candidate.\"problemRef\" AND problem.state='active' \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=candidate.\"problemRevisionRef\" \
          AND revision.problem_ref=problem.problem_ref \
         ORDER BY problem.created_at,problem.problem_ref",
    )
    .bind(json!(candidate_revisions))
    .fetch_all(&mut *tx)
    .await?;
    if problems.len() != refs.len() {
        return Err(ResolutionWorkerError::Manifest);
    }
    let input = json!({"resolutionRef":row.get::<Uuid,_>("resolution_ref"),"signal":{"proposition":row.get::<String,_>("proposition"),"problemFrame":row.get::<Value,_>("problem_frame")},"candidates":problems});
    let input_limit: i32 = row.get("input_token_limit");
    let config_ref: Uuid = row.get("config_ref");
    let identity = StudyModelIdentity {
        model_ref: row.get("model_ref"),
        connection_version_ref: row.get("version_ref"),
        model_id: row.get("model_id"),
    };
    let model_snapshot = StudyModelSnapshot {
        model_config_ref: config_ref,
        identity: identity.clone(),
        input_token_limit: input_limit,
        output_token_limit: row.get("output_token_limit"),
        timeout_seconds: row.get("timeout_seconds"),
    };
    let method: StudyMethodManifest = serde_json::from_value(row.get("method_manifest"))
        .map_err(|_| ResolutionWorkerError::Manifest)?;
    let method_hash: String = row.get("method_hash");
    let execution_manifest: Value = row.get("execution_manifest");
    if execution_manifest["methodHash"].as_str() != Some(method_hash.as_str()) {
        return Err(ResolutionWorkerError::Manifest);
    }
    verify_study_method(
        &CompiledStudyMethod {
            manifest: method.clone(),
            method_hash: method_hash.clone(),
        },
        &model_snapshot,
    )
    .map_err(|_| ResolutionWorkerError::Manifest)?;
    let stage = &method.stages.resolution;
    let (prompt, request_hash, context_hash, request_manifest) = problem_stage_request_manifest(
        "resolution",
        PROBLEM_RESOLUTION_CONTRACT,
        &method_hash,
        &stage.stage_hash,
        config_ref,
        &identity,
        model_snapshot.timeout_seconds,
        model_snapshot.output_token_limit,
        &stage.system_instruction,
        &stage.output_schema,
        &input,
    )
    .map_err(|_| ResolutionWorkerError::Manifest)?;
    if crate::comment_study_batch::conservative_token_estimate_json(&request_manifest)
        > i64::from(input_limit)
    {
        let failed = sqlx::query(
            "UPDATE linggan_comment_study_resolution SET state='failed', \
               model_invocation_ref=NULL, \
               decision_manifest=jsonb_build_object('reason','input_limit_exceeded'), \
               resolved_at=scope_001_now() \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
        )
        .bind(resolution_ref)
        .execute(&mut *tx)
        .await?;
        if failed.rows_affected() != 1 {
            return Err(ResolutionWorkerError::Model(ModelError::Conflict));
        }
        crate::comment_study_run::close_run_if_settled(&mut tx, run_ref).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let attempt: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(attempt_ordinal),0)+1 FROM linggan_comment_study_model_request \
         WHERE resolution_ref=$1 AND input_context_hash=$2",
    )
    .bind(resolution_ref)
    .bind(&context_hash)
    .fetch_one(&mut *tx)
    .await?;
    if attempt > row.get::<i32, _>("max_attempts") {
        sqlx::query(
            "UPDATE linggan_comment_study_resolution SET state='failed', \
               decision_manifest=jsonb_build_object('reason','attempts_exhausted'),resolved_at=scope_001_now() \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
        )
        .bind(resolution_ref)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(None);
    }
    let reserved_tokens =
        crate::comment_study_batch::conservative_token_estimate_json(&request_manifest)
            .saturating_add(i64::from(model_snapshot.output_token_limit));
    let invocation_ref = match reserve_problem_stage_call(
        &mut tx,
        ProblemStageSubject::Resolution(resolution_ref),
        run_ref,
        row.get("policy_ref"),
        config_ref,
        identity.connection_version_ref,
        identity.model_ref,
        reserved_tokens,
        attempt,
        &context_hash,
        &request_manifest,
        &request_hash,
        model_snapshot.timeout_seconds,
    )
    .await
    {
        Ok(invocation_ref) => invocation_ref,
        Err(RequestLedgerError::BudgetDeferred | RequestLedgerError::BudgetExhausted) => {
            tx.commit().await?;
            return Ok(None);
        }
        Err(RequestLedgerError::RunUnavailable) => {
            tx.commit().await?;
            return Ok(None);
        }
        Err(RequestLedgerError::Database(error)) => return Err(error.into()),
        Err(RequestLedgerError::SnapshotUnavailable) => {
            return Err(ResolutionWorkerError::Manifest);
        }
    };
    let claim = ClaimedResolution {
        resolution_ref: row.get("resolution_ref"),
        signal_ref: row.get("signal_ref"),
        invocation_ref,
        connection_version_ref: row.get("version_ref"),
        model_id: row.get("model_id"),
        timeout_seconds: row.get("timeout_seconds"),
        output_token_limit: row.get("output_token_limit"),
        system_instruction: stage.system_instruction.clone(),
        prompt,
    };
    tx.commit().await?;
    Ok(Some(claim))
}

/// A claim is persisted before provider I/O so two workers cannot buy the same comparison.  If
/// the request cannot even be built (for example, a Keychain entry is temporarily unavailable),
/// preserve that failed invocation receipt but release the pending comparison for a later retry.
async fn release_pre_dispatch_claim(
    database: &Database,
    claim: &ClaimedResolution,
    code: &str,
) -> Result<(), ModelError> {
    release_problem_stage_before_dispatch(
        database,
        ProblemStageSubject::Resolution(claim.resolution_ref),
        claim.invocation_ref,
        code,
    )
    .await?;
    Ok(())
}

async fn finish_failure(
    database: &Database,
    invocation_ref: Uuid,
    response: Option<&crate::pi_adapter::PiResponse>,
    code: &str,
) -> Result<(), ModelError> {
    let result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false,"failureCode":code}));
    finish_invocation(
        database,
        invocation_ref,
        response,
        false,
        Some(code),
        &result,
    )
    .await
}
