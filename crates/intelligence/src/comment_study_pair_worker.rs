//! Provider-backed decision for one independently sourced new-Problem pair.

use crate::{
    comment_study_model_runner::parse_provider_json,
    comment_study_policy::{
        CompiledStudyMethod, StudyMethodManifest, StudyModelIdentity, StudyModelSnapshot,
        verify_study_method,
    },
    comment_study_problem_resolution::PROBLEM_PAIR_CONTRACT,
    comment_study_problem_store::{
        ProblemStoreError, accept_problem_pair_from_invocation, pair_contract_failure_code,
    },
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
pub enum PairWorkerError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Store(#[from] ProblemStoreError),
    #[error("pair manifest is invalid")]
    Manifest,
}

/// Reports provider dispatch separately from local queue cleanup and response admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairExecution {
    Idle,
    LocalProgress,
    ProviderAttempt { accepted: bool },
}

struct Claim {
    pair: Uuid,
    invocation: Uuid,
    version: Uuid,
    model: String,
    timeout: i32,
    output: i32,
    system_instruction: String,
    prompt: String,
}

pub async fn run_one_problem_pair(
    database: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, PairWorkerError> {
    run_one_problem_pair_with_outcome(database, secrets, adapter)
        .await
        .map(|outcome| {
            matches!(outcome, PairExecution::LocalProgress)
                || matches!(outcome, PairExecution::ProviderAttempt { accepted: true })
        })
}

/// Detailed counterpart used by the scheduler to enforce its per-tick provider-call limit.
pub async fn run_one_problem_pair_with_outcome(
    database: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<PairExecution, PairWorkerError> {
    if !crate::comment_study_request_ledger::pair_failure_state_supported(database).await? {
        return Ok(PairExecution::Idle);
    }
    if retire_one_legacy_cross_run_pair(database).await? {
        return Ok(PairExecution::LocalProgress);
    }
    let Some(claim) = claim(database).await? else {
        return Ok(PairExecution::Idle);
    };
    let mut request = match connection_request(database, secrets, claim.version).await {
        Ok(request) => request,
        Err(error) => {
            release_pre_dispatch_claim(database, &claim, error.code()).await?;
            return Err(PairWorkerError::Model(error));
        }
    };
    request.operation = "analyze".into();
    request.model_id = claim.model.clone();
    request.timeout_ms = match u64::try_from(claim.timeout) {
        Ok(seconds) => seconds.saturating_mul(1_000),
        Err(_) => {
            release_pre_dispatch_claim(database, &claim, "invalid_model_command").await?;
            return Err(PairWorkerError::Model(ModelError::Invalid));
        }
    };
    request.max_output_tokens = claim.output;
    request.system = claim.system_instruction.clone();
    request.prompt = claim.prompt.clone();
    if let Err(error) = mark_problem_stage_dispatch_started(
        database,
        ProblemStageSubject::Pair(claim.pair),
        claim.invocation,
    )
    .await
    {
        release_pre_dispatch_claim(database, &claim, "dispatch_fence_failed").await?;
        return Err(PairWorkerError::Model(
            if matches!(error, sqlx::Error::RowNotFound) {
                ModelError::Conflict
            } else {
                ModelError::Database(error)
            },
        ));
    }
    match adapter.call(&request).await {
        Ok(response) if response.ok => {
            let raw = match parse_provider_json(response.text.as_deref()) {
                Ok(value) => value,
                Err(_) => {
                    checkpoint_invocation_usage(database, claim.invocation, Some(&response))
                        .await?;
                    finish(
                        database,
                        claim.invocation,
                        Some(&response),
                        "invalid_provider_output",
                    )
                    .await?;
                    release_problem_stage_after_failure(
                        database,
                        ProblemStageSubject::Pair(claim.pair),
                        claim.invocation,
                        "invalid_provider_output",
                    )
                    .await?;
                    return Err(PairWorkerError::Model(ModelError::InvalidOutput));
                }
            };
            checkpoint_invocation_usage(database, claim.invocation, Some(&response)).await?;
            match accept_problem_pair_from_invocation(database, claim.pair, claim.invocation, raw)
                .await
            {
                Ok(_) => {
                    finish_invocation(database,claim.invocation,Some(&response),true,None,&json!({"contract":PROBLEM_PAIR_CONTRACT,"pairRef":claim.pair,"accepted":true})).await?;
                    Ok(PairExecution::ProviderAttempt { accepted: true })
                }
                Err(ProblemStoreError::ModelRequestUnavailable) => {
                    Ok(PairExecution::ProviderAttempt { accepted: false })
                }
                Err(ProblemStoreError::Contract(error)) => {
                    finish(
                        database,
                        claim.invocation,
                        Some(&response),
                        pair_contract_failure_code(&error),
                    )
                    .await?;
                    Err(PairWorkerError::Store(ProblemStoreError::Contract(error)))
                }
                Err(error) => {
                    finish(
                        database,
                        claim.invocation,
                        Some(&response),
                        "pair_acceptance_failed",
                    )
                    .await?;
                    release_problem_stage_after_failure(
                        database,
                        ProblemStageSubject::Pair(claim.pair),
                        claim.invocation,
                        "pair_acceptance_failed",
                    )
                    .await?;
                    Err(PairWorkerError::Store(error))
                }
            }
        }
        Ok(response) => {
            finish(
                database,
                claim.invocation,
                Some(&response),
                response
                    .failure_code
                    .as_deref()
                    .unwrap_or("provider_failed"),
            )
            .await?;
            release_problem_stage_after_failure(
                database,
                ProblemStageSubject::Pair(claim.pair),
                claim.invocation,
                response
                    .failure_code
                    .as_deref()
                    .unwrap_or("provider_failed"),
            )
            .await?;
            Err(PairWorkerError::Model(ModelError::AdapterUnavailable))
        }
        Err(error) => {
            finish(database, claim.invocation, None, error.code()).await?;
            release_problem_stage_after_failure(
                database,
                ProblemStageSubject::Pair(claim.pair),
                claim.invocation,
                error.code(),
            )
            .await?;
            Err(PairWorkerError::Model(error))
        }
    }
}

/// Old scheduler versions could persist a pending pair across Runs, making its budget owner
/// ambiguous. Retire one unclaimed legacy row with an explicit reason before dispatching any pair.
async fn retire_one_legacy_cross_run_pair(database: &Database) -> Result<bool, sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let pair_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT pair.pair_ref FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_comment_study_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target first_target USING(target_ref) \
         JOIN linggan_comment_study_run first_run ON first_run.run_ref=first_target.run_ref \
         JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
         JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
         WHERE pair.state='pending' AND pair.model_invocation_ref IS NULL \
           AND first_target.run_ref<>second_target.run_ref \
           AND first_run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
           AND to_jsonb(first_run)->>'dispatch_state'='enabled' \
           AND to_jsonb(first_run)->>'dispatch_reason' IS NULL \
         ORDER BY pair.created_at,pair.pair_ref LIMIT 1 FOR UPDATE OF pair SKIP LOCKED",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(pair_ref) = pair_ref else {
        transaction.commit().await?;
        return Ok(false);
    };
    let retired = sqlx::query(
        "UPDATE linggan_comment_study_problem_pair SET state='failed',resolved_at=scope_001_now(), \
           pair_manifest=jsonb_set(pair_manifest,'{decision}', \
             jsonb_build_object('code','legacy_cross_run_pair','action','retired'),true) \
         WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
    )
    .bind(pair_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(retired.rows_affected() == 1)
}

async fn claim(database: &Database) -> Result<Option<Claim>, PairWorkerError> {
    let mut tx = database.pool().begin().await?;
    let candidate: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT pair.pair_ref,target.run_ref \
         FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
         JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         WHERE pair.state='pending' AND pair.model_invocation_ref IS NULL \
           AND second_target.run_ref=target.run_ref \
           AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL \
         ORDER BY pair.created_at,pair.pair_ref LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((pair_ref, run_ref)) = candidate else {
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
    let locked_pair: Option<Uuid> = sqlx::query_scalar(
        "SELECT pair_ref FROM linggan_comment_study_problem_pair \
         WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref IS NULL \
         FOR UPDATE SKIP LOCKED",
    )
    .bind(pair_ref)
    .fetch_optional(&mut *tx)
    .await?;
    if locked_pair.is_none() {
        tx.commit().await?;
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT pair.pair_ref,pair.first_signal_ref,pair.second_signal_ref,run.run_ref,run.policy_ref, \
                config.config_ref,config.input_token_limit,config.output_token_limit,config.timeout_seconds, \
                config.max_attempts, \
                to_jsonb(policy)->'method_manifest' AS method_manifest, \
                to_jsonb(policy)->>'method_hash' AS method_hash, \
                to_jsonb(run)->'execution_manifest' AS execution_manifest, \
                model.model_ref,model.model_id,version.version_ref,connection.enabled \
         FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
         JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
         JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection ON connection.connection_ref=version.connection_ref \
         WHERE pair.pair_ref=$1 AND run.run_ref=$2 \
           AND second_target.run_ref=target.run_ref \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL",
    )
    .bind(pair_ref)
    .bind(run_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PairWorkerError::Model(ModelError::Conflict))?;
    if !row.get::<bool, _>("enabled") {
        return Err(PairWorkerError::Model(ModelError::Disabled));
    };
    let first: Value = signal_input(&mut tx, row.get("first_signal_ref")).await?;
    let second: Value = signal_input(&mut tx, row.get("second_signal_ref")).await?;
    let input = json!({"pairRef":row.get::<Uuid,_>("pair_ref"),"first":first,"second":second});
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
        .map_err(|_| PairWorkerError::Manifest)?;
    let method_hash: String = row.get("method_hash");
    let execution_manifest: Value = row.get("execution_manifest");
    if execution_manifest["methodHash"].as_str() != Some(method_hash.as_str()) {
        return Err(PairWorkerError::Manifest);
    }
    verify_study_method(
        &CompiledStudyMethod {
            manifest: method.clone(),
            method_hash: method_hash.clone(),
        },
        &model_snapshot,
    )
    .map_err(|_| PairWorkerError::Manifest)?;
    let stage = &method.stages.pair;
    let (prompt, request_hash, context_hash, request_manifest) = problem_stage_request_manifest(
        "pair",
        PROBLEM_PAIR_CONTRACT,
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
    .map_err(|_| PairWorkerError::Manifest)?;
    if crate::comment_study_batch::conservative_token_estimate_json(&request_manifest)
        > i64::from(input_limit)
    {
        let failed = sqlx::query(
            "UPDATE linggan_comment_study_problem_pair \
             SET state='failed',model_invocation_ref=NULL,resolved_at=scope_001_now(), \
                 pair_manifest=jsonb_set(pair_manifest,'{decision}', \
                   jsonb_build_object('code','input_limit_exceeded'),true) \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
        )
        .bind(pair_ref)
        .execute(&mut *tx)
        .await?;
        if failed.rows_affected() != 1 {
            return Err(PairWorkerError::Model(ModelError::Conflict));
        }
        crate::comment_study_run::close_run_if_settled(&mut tx, run_ref).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let attempt: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(attempt_ordinal),0)+1 FROM linggan_comment_study_model_request \
         WHERE pair_ref=$1 AND input_context_hash=$2",
    )
    .bind(pair_ref)
    .bind(&context_hash)
    .fetch_one(&mut *tx)
    .await?;
    if attempt > row.get::<i32, _>("max_attempts") {
        sqlx::query(
            "UPDATE linggan_comment_study_problem_pair \
             SET state='failed',resolved_at=scope_001_now(), \
                 pair_manifest=jsonb_set(pair_manifest,'{decision}', \
                   jsonb_build_object('code','attempts_exhausted'),true) \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
        )
        .bind(pair_ref)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(None);
    }
    let reserved_tokens =
        crate::comment_study_batch::conservative_token_estimate_json(&request_manifest)
            .saturating_add(i64::from(model_snapshot.output_token_limit));
    let invocation = match reserve_problem_stage_call(
        &mut tx,
        ProblemStageSubject::Pair(pair_ref),
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
        Err(RequestLedgerError::SnapshotUnavailable) => return Err(PairWorkerError::Manifest),
    };
    let value = Claim {
        pair: row.get("pair_ref"),
        invocation,
        version: row.get("version_ref"),
        model: row.get("model_id"),
        timeout: row.get("timeout_seconds"),
        output: row.get("output_token_limit"),
        system_instruction: stage.system_instruction.clone(),
        prompt,
    };
    tx.commit().await?;
    Ok(Some(value))
}
async fn signal_input(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
) -> Result<Value, sqlx::Error> {
    sqlx::query_scalar("SELECT jsonb_build_object('signalRef',signal_ref,'proposition',proposition,'problemFrame',problem_frame) FROM linggan_comment_study_signal WHERE signal_ref=$1").bind(signal_ref).fetch_one(&mut **tx).await
}
async fn finish(
    db: &Database,
    id: Uuid,
    response: Option<&crate::pi_adapter::PiResponse>,
    code: &str,
) -> Result<(), ModelError> {
    let r = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false,"failureCode":code}));
    finish_invocation(db, id, response, false, Some(code), &r).await
}

/// See the equivalent Resolution helper: an unavailable credential before provider I/O must not
/// strand a pending pair behind an invocation reference that no worker may claim again.
async fn release_pre_dispatch_claim(
    database: &Database,
    claim: &Claim,
    code: &str,
) -> Result<(), ModelError> {
    release_problem_stage_before_dispatch(
        database,
        ProblemStageSubject::Pair(claim.pair),
        claim.invocation,
        code,
    )
    .await?;
    Ok(())
}
#[cfg(test)]
fn schema() -> Value {
    let verdict = json!({"type":"string","enum":["same","different","unknown"]});
    json!({"type":"object","additionalProperties":false,"required":["contract","firstSignalRef","secondSignalRef","dimensions","proposedProblem"],"properties":{"contract":{"const":PROBLEM_PAIR_CONTRACT},"firstSignalRef":{"type":"string"},"secondSignalRef":{"type":"string"},"dimensions":{"type":"object","additionalProperties":false,"required":["actor","goalOrExpectedState","barrierOrUnmetNeed","context"],"properties":{"actor":verdict,"goalOrExpectedState":verdict,"barrierOrUnmetNeed":verdict,"context":verdict}},"proposedProblem":{"type":["object","null"]}}})
}

#[cfg(test)]
mod tests {
    use super::schema;
    use serde_json::json;

    #[test]
    fn pair_schema_pins_every_required_contract_field() {
        let schema = schema();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["contract"]["const"],
            "comment-study.problem-pair.v1"
        );
        assert_eq!(
            schema["properties"]["dimensions"]["additionalProperties"],
            false
        );
        assert_eq!(
            schema["properties"]["dimensions"]["properties"]["actor"]["enum"],
            json!(["same", "different", "unknown"])
        );
    }
}
