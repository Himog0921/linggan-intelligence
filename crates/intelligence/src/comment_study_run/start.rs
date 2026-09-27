//! P2 transaction core. Not exposed by the legacy HTTP /runs handler; no model dispatch here.
use crate::comment_study_policy::{StudyPolicyStoreError, store};
use crate::comment_study_selection::{
    SelectionPreviewCommand, StartStudyRunCommand, StudyRunLimits, StudySelectionError,
    snapshot::{FrozenSelection, read_frozen_selection},
    study_domain_lock_key,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use std::time::Duration;
use uuid::Uuid;

#[path = "write.rs"]
mod write;

#[derive(Debug, thiserror::Error)]
pub enum StudyStartError {
    #[error(transparent)]
    Selection(#[from] StudySelectionError),
    #[error(transparent)]
    Policy(#[from] StudyPolicyStoreError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("study_schema_unavailable")]
    SchemaUnavailable,
    #[error("resource_not_found")]
    NotFound,
    #[error("idempotency_conflict")]
    IdempotencyConflict,
    #[error("study_busy")]
    Busy,
    #[error("query_timeout")]
    QueryTimeout,
    #[error("study_build_unrecorded")]
    BuildUnrecorded,
    #[error("study_receipt_invalid")]
    ReceiptInvalid,
}

/// Constructed only by trusted callers; never Deserialize and never supplied by an HTTP body.
pub enum TrustedStudyOrigin {
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyStartReceipt {
    pub request_ref: Uuid,
    pub outcome: String,
    pub run_ref: Option<Uuid>,
    pub as_of: String,
    pub requested_work_count: usize,
    pub covered_work_count: usize,
    pub target_count: usize,
    pub queued_count: usize,
    pub needs_context_count: usize,
    pub limits: StudyRunLimits,
    pub exclusion_counts: BTreeMap<String, usize>,
    pub index_coverage: Value,
    pub dispatch_state: Option<String>,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyRunCancellationReceipt {
    pub run_ref: Uuid,
    pub domain_ref: Uuid,
    pub state: String,
    pub dispatch_state: String,
    pub dispatch_reason: Option<String>,
    pub control_version: i64,
    pub as_of: String,
    pub finished_at: Option<String>,
}

pub(crate) async fn ensure_start_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), StudyStartError> {
    store::ensure_schema(tx, true).await?;
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_start_request') IS NOT NULL \
        AND to_regclass('linggan_comment_study_model_request') IS NOT NULL \
        AND to_regclass('linggan_comment_study_clean_cache') IS NOT NULL \
        AND (SELECT count(*)=5 FROM pg_trigger WHERE NOT tgisinternal AND tgenabled IN ('O','A') \
          AND tgrelid IN ('linggan_comment_study_run'::regclass,'linggan_comment_study_target'::regclass, \
                         COALESCE(to_regclass('linggan_comment_study_start_request'),0)) \
          AND tgname IN ('cs_run_insert_guard','cs_run_frozen_guard','cs_target_input_guard', \
                         'cs_start_immutable','cs_start_no_truncate')) \
        AND (SELECT count(*)=2 FROM pg_trigger WHERE NOT tgisinternal AND tgenabled IN ('O','A') \
          AND tgrelid=to_regclass('linggan_comment_study_model_request') \
          AND tgname IN ('cs_request_immutable','cs_request_no_truncate')) \
        AND EXISTS(SELECT 1 FROM pg_constraint \
          WHERE conrelid=to_regclass('linggan_comment_study_problem_pair') \
            AND conname='cs_problem_pair_state_ck' AND contype='c' \
            AND pg_get_constraintdef(oid) LIKE '%failed%') \
        AND EXISTS(SELECT 1 FROM pg_constraint \
          WHERE conrelid=to_regclass('linggan_comment_study_problem_pair') \
            AND conname='cs_problem_pair_terminal_ck' AND contype='c' \
            AND pg_get_constraintdef(oid) LIKE '%failed%') \
        AND EXISTS(SELECT 1 FROM pg_index WHERE indexrelid=to_regclass('cs_target_active_comment_uq') \
          AND indisunique AND indisvalid AND indisready)")
        .fetch_one(&mut **tx).await?;
    if !ready {
        return Err(StudyStartError::SchemaUnavailable);
    }
    Ok(())
}

async fn begin(database: &Database) -> Result<Transaction<'_, Postgres>, StudyStartError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn method(
    tx: &mut Transaction<'_, Postgres>,
    command: &SelectionPreviewCommand,
    writing: bool,
) -> Result<Value, StudyStartError> {
    let policy = store::load_policy(tx, command.domain_ref, command.policy_ref).await?;
    if policy["recordingState"] != "recorded" {
        return Err(StudyPolicyStoreError::Unrecorded.into());
    }
    let config: Uuid = serde_json::from_value(policy["methodManifest"]["modelConfigRef"].clone())
        .map_err(|_| StudyPolicyStoreError::ModelUnavailable)?;
    let (model, enabled) = store::model_snapshot(tx, config, writing).await?;
    if !enabled {
        return Err(StudyPolicyStoreError::ModelDisabled.into());
    }
    // Recheck against the rows now locked, not the earlier unlocked metadata read.
    let compiled = crate::comment_study_policy::CompiledStudyMethod {
        manifest: serde_json::from_value(policy["methodManifest"].clone())
            .map_err(|_| StudyPolicyStoreError::ModelUnavailable)?,
        method_hash: policy["methodHash"]
            .as_str()
            .ok_or(StudyPolicyStoreError::Unrecorded)?
            .into(),
    };
    crate::comment_study_policy::verify_study_method(&compiled, &model)
        .map_err(StudyPolicyStoreError::from)?;
    Ok(policy)
}

async fn as_of(tx: &mut Transaction<'_, Postgres>) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut **tx)
    .await
}

/// Stops future dispatch for one v2 Run while preserving any request that already crossed its
/// durable provider-dispatch fence. Requests still reserved but not dispatched are settled at
/// zero charge, and their unprocessed targets become explicit user-stopped history.
pub async fn cancel_study_run(
    database: &Database,
    domain_ref: Uuid,
    run_ref: Uuid,
) -> Result<StudyRunCancellationReceipt, StudyStartError> {
    if domain_ref.is_nil() || run_ref.is_nil() {
        return Err(StudyStartError::NotFound);
    }
    let mut tx = begin(database).await?;
    ensure_start_schema(&mut tx).await?;
    let locked = sqlx::query(
        "SELECT run.run_ref,policy.domain_ref,run.selection_manifest->>'contract' AS selection_contract, \
                run.state,run.finished_at::text AS finished_at,run.dispatch_state,run.dispatch_reason,run.control_version \
         FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE run.run_ref=$1 AND policy.domain_ref=$2 FOR UPDATE OF run",
    )
    .bind(run_ref)
    .bind(domain_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(StudyStartError::NotFound)?;
    if locked
        .get::<Option<String>, _>("selection_contract")
        .as_deref()
        != Some("comment-study.run-selection.v2")
    {
        return Err(StudyStartError::NotFound);
    }

    let finished_at: Option<String> = locked.get("finished_at");
    let dispatch_state: String = locked.get("dispatch_state");
    let dispatch_reason: Option<String> = locked.get("dispatch_reason");
    let should_apply_user_stop = finished_at.is_none()
        && (matches!(dispatch_state.as_str(), "enabled" | "paused")
            || (dispatch_state == "stopped" && dispatch_reason.as_deref() == Some("user_stopped")));

    if finished_at.is_none() && matches!(dispatch_state.as_str(), "enabled" | "paused") {
        sqlx::query(
            "UPDATE linggan_comment_study_run \
             SET dispatch_state='stopped',dispatch_reason='user_stopped',control_version=control_version+1 \
             WHERE run_ref=$1 AND dispatch_state IN ('enabled','paused')",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
    }

    if should_apply_user_stop {
        // Resolution and pair reservations have not acquired permission to call a provider yet.
        // Release their reservation and detach the pending subject so a later explicit Run may
        // resume it. The request snapshot itself is append-only and remains available for audit.
        sqlx::query(
            "UPDATE linggan_model_invocation invocation \
             SET state='failed',charged_tokens=0,failure_code='user_stopped', \
                 result=COALESCE(invocation.result,'{}'::jsonb)||jsonb_build_object('ok',false,'failureCode','user_stopped'), \
                 finished_at=scope_001_now() \
             FROM linggan_comment_study_model_request request \
             WHERE request.invocation_ref=invocation.invocation_ref AND request.run_ref=$1 \
               AND request.dispatch_started_at IS NULL AND invocation.state='running'",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_resolution resolution SET model_invocation_ref=NULL \
             FROM linggan_comment_study_model_request request \
             WHERE request.run_ref=$1 AND request.stage='resolution' AND request.dispatch_started_at IS NULL \
               AND resolution.resolution_ref=request.resolution_ref AND resolution.state='pending' \
               AND resolution.model_invocation_ref=request.invocation_ref",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_problem_pair pair SET model_invocation_ref=NULL \
             FROM linggan_comment_study_model_request request \
             WHERE request.run_ref=$1 AND request.stage='pair' AND request.dispatch_started_at IS NULL \
               AND pair.pair_ref=request.pair_ref AND pair.state='pending' \
               AND pair.model_invocation_ref=request.invocation_ref",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_batch batch \
             SET state='cancelled',lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
                 output_manifest=jsonb_build_object('reason','user_stopped'),finished_at=scope_001_now() \
             WHERE batch.run_ref=$1 AND batch.state IN ('prepared','leased') \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
                 WHERE request.batch_ref=batch.batch_ref AND request.stage='semantic' \
                   AND request.dispatch_started_at IS NOT NULL)",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_target target \
             SET state='cancelled',finished_at=scope_001_now(),terminal_reason='user_stopped' \
             WHERE target.run_ref=$1 AND target.state IN ('ready','queued','running') \
               AND NOT EXISTS( \
                 SELECT 1 FROM linggan_comment_study_batch_target member \
                 JOIN linggan_comment_study_batch batch ON batch.batch_ref=member.batch_ref \
                 JOIN linggan_comment_study_model_request request ON request.batch_ref=batch.batch_ref \
                 JOIN linggan_model_invocation invocation ON invocation.invocation_ref=request.invocation_ref \
                 WHERE member.target_ref=target.target_ref AND request.stage='semantic' \
                   AND request.dispatch_started_at IS NOT NULL AND invocation.state='running')",
        )
        .bind(run_ref)
        .execute(&mut *tx)
        .await?;
        super::close_run_if_settled(&mut tx, run_ref).await?;
    }

    let current = sqlx::query(
        "SELECT state,dispatch_state,dispatch_reason,control_version,finished_at::text AS finished_at \
         FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(&mut *tx)
    .await?;
    let as_of = as_of(&mut tx).await?;
    let receipt = StudyRunCancellationReceipt {
        run_ref,
        domain_ref,
        state: current.get("state"),
        dispatch_state: current.get("dispatch_state"),
        dispatch_reason: current.get("dispatch_reason"),
        control_version: current.get("control_version"),
        as_of,
        finished_at: current.get("finished_at"),
    };
    tx.commit().await?;
    Ok(receipt)
}

/// The preview owns no reservation and starts no work. Its selection is advisory until start commits.
pub async fn preview_study_selection(
    database: &Database,
    command: SelectionPreviewCommand,
) -> Result<Value, StudyStartError> {
    let command = command.normalize()?;
    let mut tx = begin(database).await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await?;
    ensure_start_schema(&mut tx).await?;
    let policy = method(&mut tx, &command, false).await?;
    let cutoff = as_of(&mut tx).await?;
    let snapshot = read_frozen_selection(&mut tx, &command, cutoff).await?;
    let receipt = write::receipt(&snapshot, Uuid::nil(), &command.limits, None);
    let value = json!({"contract":"comment-study.read.v2","domainRef":command.domain_ref,
        "policyRef":command.policy_ref,"methodHash":policy["methodHash"],"asOf":snapshot.as_of,
        "scopeCommentCount":snapshot.selection.scope_comment_count,"targetCount":receipt.target_count,
        "queuedCount":receipt.queued_count,"needsContextCount":receipt.needs_context_count,
        "requestedWorkCount":receipt.requested_work_count,"coveredWorkCount":receipt.covered_work_count,
        "requestedWorkRoles":command.work_roles,
        "limits":command.limits,"exclusionCounts":receipt.exclusion_counts,
        "indexCoverage":receipt.index_coverage});
    tx.commit().await?;
    Ok(value)
}

/// Atomic core shared with a future scheduler. HTTP/dispatcher cutover remains a separate gate.
pub async fn start_study_run(
    database: &Database,
    command: StartStudyRunCommand,
    origin: TrustedStudyOrigin,
) -> Result<StudyStartReceipt, StudyStartError> {
    let TrustedStudyOrigin::Manual = origin;
    let command = command.normalize()?;
    let hash = command.manual_request_hash()?;
    for attempt in 0..=3 {
        match start_once(database, &command, &hash).await {
            Ok(receipt) => return Ok(receipt),
            Err(error) if retryable(&error) => {
                if attempt == 3 {
                    return Err(StudyStartError::Busy);
                }
                tokio::time::sleep(Duration::from_millis([50, 150, 450][attempt])).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(StudyStartError::Busy)
}

fn retryable(error: &StudyStartError) -> bool {
    match error {
        StudyStartError::Database(sqlx::Error::Database(e))
        | StudyStartError::Policy(StudyPolicyStoreError::Database(sqlx::Error::Database(e))) => {
            matches!(e.code().as_deref(), Some("40001" | "40P01" | "55P03"))
                || (e.code().as_deref() == Some("23505")
                    && e.constraint() == Some("cs_target_active_comment_uq"))
        }
        _ => false,
    }
}

async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    command: &StartStudyRunCommand,
    hash: &str,
) -> Result<Option<StudyStartReceipt>, StudyStartError> {
    let row = sqlx::query(
        "SELECT domain_ref,request_hash,result_manifest,run_ref,outcome \
        FROM linggan_comment_study_start_request WHERE request_ref=$1",
    )
    .bind(command.request_ref)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.try_get::<Uuid, _>("domain_ref")? != command.domain_ref
        || row.try_get::<String, _>("request_hash")? != hash
    {
        return Err(StudyStartError::IdempotencyConflict);
    }
    let mut receipt: StudyStartReceipt = serde_json::from_value(row.try_get("result_manifest")?)
        .map_err(|_| StudyStartError::ReceiptInvalid)?;
    if receipt.request_ref != command.request_ref
        || receipt.limits != command.limits
        || receipt.run_ref != row.try_get::<Option<Uuid>, _>("run_ref")?
        || receipt.outcome != row.try_get::<String, _>("outcome")?
        || !matches!(
            receipt.outcome.as_str(),
            "created" | "no_work" | "index_pending"
        )
        || (receipt.outcome == "created") != receipt.run_ref.is_some()
    {
        return Err(StudyStartError::ReceiptInvalid);
    }
    receipt.idempotent_replay = true;
    Ok(Some(receipt))
}

async fn start_once(
    database: &Database,
    command: &StartStudyRunCommand,
    hash: &str,
) -> Result<StudyStartReceipt, StudyStartError> {
    let mut tx = begin(database).await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(study_domain_lock_key(command.domain_ref)?)
        .execute(&mut *tx)
        .await?;
    ensure_start_schema(&mut tx).await?;
    if let Some(receipt) = replay(&mut tx, command, hash).await? {
        tx.commit().await?;
        return Ok(receipt);
    }
    let domain_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM observation_domain WHERE domain_ref=$1 FOR SHARE")
            .bind(command.domain_ref)
            .fetch_optional(&mut *tx)
            .await?;
    match domain_status.as_deref() {
        Some("active") => {}
        Some(_) => return Err(StudyPolicyStoreError::DomainUnavailable.into()),
        None => return Err(StudyPolicyStoreError::NotFound.into()),
    }
    let selection = command.preview();
    let policy = method(&mut tx, &selection, true).await?;
    let cutoff = as_of(&mut tx).await?;
    let snapshot = read_frozen_selection(&mut tx, &selection, cutoff).await?;
    let run = if snapshot.selection.target_count > 0 {
        Some(Uuid::new_v4())
    } else {
        None
    };
    let receipt = write::receipt(&snapshot, command.request_ref, &command.limits, run);
    if let Some(run) = run {
        write::insert_run(&mut tx, command, run, &policy, snapshot).await?;
    }
    let inserted = sqlx::query("INSERT INTO linggan_comment_study_start_request \
        (request_ref,domain_ref,policy_ref,request_hash,origin,command_manifest,outcome,run_ref,result_manifest) \
        VALUES($1,$2,$3,$4,'manual',$5,$6,$7,$8) ON CONFLICT(request_ref) DO NOTHING")
        .bind(command.request_ref).bind(command.domain_ref).bind(command.policy_ref).bind(hash)
        .bind(json!(command)).bind(&receipt.outcome).bind(run).bind(json!(receipt))
        .execute(&mut *tx).await?;
    if inserted.rows_affected() != 1 {
        return Err(StudyStartError::IdempotencyConflict);
    }
    tx.commit().await?;
    Ok(receipt)
}
