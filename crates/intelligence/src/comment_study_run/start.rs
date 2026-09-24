//! P2 transaction core. Not exposed by the legacy HTTP /runs handler; no model dispatch here.
use crate::comment_study_policy::{StudyPolicyStoreError, store};
use crate::comment_study_selection::{SelectionPreviewCommand, StartStudyRunCommand, StudyRunLimits,
    StudySelectionError, snapshot::{FrozenSelection, read_frozen_selection}, study_domain_lock_key};
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
pub enum TrustedStudyOrigin { Manual }

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

pub(crate) async fn ensure_start_schema(tx: &mut Transaction<'_, Postgres>) -> Result<(), StudyStartError> {
    store::ensure_schema(tx, true).await?;
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_start_request') IS NOT NULL \
        AND to_regclass('linggan_comment_study_model_request') IS NOT NULL \
        AND to_regclass('linggan_comment_study_clean_cache') IS NOT NULL \
        AND (SELECT count(*)=5 FROM pg_trigger WHERE NOT tgisinternal AND tgenabled IN ('O','A') \
          AND tgrelid IN ('linggan_comment_study_run'::regclass,'linggan_comment_study_target'::regclass, \
                         COALESCE(to_regclass('linggan_comment_study_start_request'),0)) \
          AND tgname IN ('cs_run_insert_guard','cs_run_frozen_guard','cs_target_input_guard', \
                         'cs_start_immutable','cs_start_no_truncate')) \
        AND EXISTS(SELECT 1 FROM pg_index WHERE indexrelid=to_regclass('cs_target_active_comment_uq') \
          AND indisunique AND indisvalid AND indisready)")
        .fetch_one(&mut **tx).await?;
    if !ready { return Err(StudyStartError::SchemaUnavailable); }
    Ok(())
}

async fn begin(database: &Database) -> Result<Transaction<'_, Postgres>, StudyStartError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL lock_timeout='3s'").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    Ok(tx)
}

async fn method(tx: &mut Transaction<'_, Postgres>, command: &SelectionPreviewCommand, writing: bool)
    -> Result<Value, StudyStartError>
{
    let policy = store::load_policy(tx, command.domain_ref, command.policy_ref).await?;
    if policy["recordingState"] != "recorded" { return Err(StudyPolicyStoreError::Unrecorded.into()); }
    let config: Uuid = serde_json::from_value(policy["methodManifest"]["modelConfigRef"].clone())
        .map_err(|_| StudyPolicyStoreError::ModelUnavailable)?;
    let (model, enabled) = store::model_snapshot(tx, config, writing).await?;
    if !enabled { return Err(StudyPolicyStoreError::ModelDisabled.into()); }
    // Recheck against the rows now locked, not the earlier unlocked metadata read.
    let compiled = crate::comment_study_policy::CompiledStudyMethod {
        manifest: serde_json::from_value(policy["methodManifest"].clone())
            .map_err(|_| StudyPolicyStoreError::ModelUnavailable)?,
        method_hash: policy["methodHash"].as_str().ok_or(StudyPolicyStoreError::Unrecorded)?.into(),
    };
    crate::comment_study_policy::verify_study_method(&compiled, &model)
        .map_err(StudyPolicyStoreError::from)?;
    Ok(policy)
}

async fn as_of(tx: &mut Transaction<'_, Postgres>) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')")
        .fetch_one(&mut **tx).await
}

/// The preview owns no reservation and starts no work. Its selection is advisory until start commits.
pub async fn preview_study_selection(database: &Database, command: SelectionPreviewCommand)
    -> Result<Value, StudyStartError>
{
    let command = command.normalize()?;
    let mut tx = begin(database).await?;
    sqlx::query("SET TRANSACTION READ ONLY").execute(&mut *tx).await?;
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
        "limits":command.limits,"exclusionCounts":receipt.exclusion_counts,
        "indexCoverage":receipt.index_coverage});
    tx.commit().await?;
    Ok(value)
}

/// Atomic core shared with a future scheduler. HTTP/dispatcher cutover remains a separate gate.
pub async fn start_study_run(database: &Database, command: StartStudyRunCommand, origin: TrustedStudyOrigin)
    -> Result<StudyStartReceipt, StudyStartError>
{
    let TrustedStudyOrigin::Manual = origin;
    let command = command.normalize()?;
    let hash = command.manual_request_hash()?;
    for attempt in 0..=3 {
        match start_once(database, &command, &hash).await {
            Ok(receipt) => return Ok(receipt),
            Err(error) if retryable(&error) => {
                if attempt == 3 { return Err(StudyStartError::Busy); }
                tokio::time::sleep(Duration::from_millis([50,150,450][attempt])).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(StudyStartError::Busy)
}

fn retryable(error: &StudyStartError) -> bool {
    match error {
        StudyStartError::Database(sqlx::Error::Database(e))
        | StudyStartError::Policy(StudyPolicyStoreError::Database(sqlx::Error::Database(e))) =>
            matches!(e.code().as_deref(), Some("40001" | "40P01" | "55P03"))
            || (e.code().as_deref()==Some("23505") && e.constraint()==Some("cs_target_active_comment_uq")),
        _ => false,
    }
}

async fn replay(tx: &mut Transaction<'_, Postgres>, command: &StartStudyRunCommand, hash: &str)
    -> Result<Option<StudyStartReceipt>, StudyStartError>
{
    let row = sqlx::query("SELECT domain_ref,request_hash,result_manifest,run_ref,outcome \
        FROM linggan_comment_study_start_request WHERE request_ref=$1")
        .bind(command.request_ref).fetch_optional(&mut **tx).await?;
    let Some(row) = row else { return Ok(None); };
    if row.try_get::<Uuid,_>("domain_ref")? != command.domain_ref
        || row.try_get::<String,_>("request_hash")? != hash
    { return Err(StudyStartError::IdempotencyConflict); }
    let mut receipt: StudyStartReceipt = serde_json::from_value(row.try_get("result_manifest")?)
        .map_err(|_| StudyStartError::ReceiptInvalid)?;
    if receipt.request_ref != command.request_ref || receipt.limits != command.limits
        || receipt.run_ref != row.try_get::<Option<Uuid>,_>("run_ref")?
        || receipt.outcome != row.try_get::<String,_>("outcome")?
        || !matches!(receipt.outcome.as_str(),"created"|"no_work"|"index_pending")
        || (receipt.outcome=="created") != receipt.run_ref.is_some()
    { return Err(StudyStartError::ReceiptInvalid); }
    receipt.idempotent_replay = true;
    Ok(Some(receipt))
}

async fn start_once(database: &Database, command: &StartStudyRunCommand, hash: &str)
    -> Result<StudyStartReceipt, StudyStartError>
{
    let mut tx = begin(database).await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(study_domain_lock_key(command.domain_ref)?)
        .execute(&mut *tx).await?;
    ensure_start_schema(&mut tx).await?;
    if let Some(receipt) = replay(&mut tx, command, hash).await? {
        tx.commit().await?;
        return Ok(receipt);
    }
    let selection = command.preview();
    let policy = method(&mut tx, &selection, true).await?;
    let cutoff = as_of(&mut tx).await?;
    let snapshot = read_frozen_selection(&mut tx, &selection, cutoff).await?;
    let run = if snapshot.selection.target_count>0 { Some(Uuid::new_v4()) } else { None };
    let receipt = write::receipt(&snapshot, command.request_ref, &command.limits, run);
    if let Some(run) = run { write::insert_run(&mut tx, command, run, &policy, snapshot).await?; }
    let inserted = sqlx::query("INSERT INTO linggan_comment_study_start_request \
        (request_ref,domain_ref,policy_ref,request_hash,origin,command_manifest,outcome,run_ref,result_manifest) \
        VALUES($1,$2,$3,$4,'manual',$5,$6,$7,$8) ON CONFLICT(request_ref) DO NOTHING")
        .bind(command.request_ref).bind(command.domain_ref).bind(command.policy_ref).bind(hash)
        .bind(json!(command)).bind(&receipt.outcome).bind(run).bind(json!(receipt))
        .execute(&mut *tx).await?;
    if inserted.rows_affected()!=1 { return Err(StudyStartError::IdempotencyConflict); }
    tx.commit().await?;
    Ok(receipt)
}
