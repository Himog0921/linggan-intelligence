//! Auditable reservation of one provider call for one leased comment-study batch.
//!
//! This boundary neither obtains a secret nor calls a provider. It freezes the generic model
//! ledger receipt before an adapter receives the immutable batch envelope, and keeps the receipt
//! linked to every per-target semantic attempt that the batch later admits.

use crate::{
    comment_study_batch::{conservative_token_estimate_json, semantic_model_request_manifest},
    comment_study_policy::{
        CompiledStudyMethod, StudyMethodManifest, StudyModelIdentity, StudyModelSnapshot,
        json_hash, verify_study_method,
    },
    comment_study_run::close_run_if_settled,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

pub const COMMENT_STUDY_SEMANTIC_STAGE: &str = "comment-study.semantic.v1";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservedStudyModelCall {
    pub invocation_ref: Uuid,
    pub run_ref: Uuid,
    pub batch_ref: Uuid,
    pub config_ref: Uuid,
    pub connection_version_ref: Uuid,
    pub model_ref: Uuid,
    pub model_id: String,
    pub output_token_limit: i32,
    pub timeout_seconds: i32,
    /// Present only for an immutable v2 method-backed run. The exact serialized provider payload
    /// and schema are loaded from the frozen method and request snapshot, never rebuilt from UI.
    pub system_instruction: Option<String>,
    pub output_schema: Option<Value>,
    pub prompt: Option<String>,
}

#[derive(Debug, Error)]
pub enum StudyModelDispatchError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the batch is absent, not leased by this worker, or its lease has expired")]
    BatchUnavailable,
    #[error("the Study policy has no model configuration")]
    ModelConfigurationMissing,
    #[error("the configured model connection is disabled or incomplete")]
    ModelUnavailable,
    #[error("the frozen batch envelope exceeds the configured model input limit")]
    InputLimit,
    #[error("the batch already has a terminal model invocation")]
    InvocationUnavailable,
    #[error("the StudyRun does not have a valid immutable method snapshot")]
    MethodUnavailable,
    #[error("the StudyRun request snapshot could not be recorded")]
    RequestSnapshotUnavailable,
    #[error("the batch was returned to the queue while existing reserved calls settle")]
    BudgetDeferred,
    #[error("the undispatched batch was safely returned for a later Run tick")]
    PreDispatchDeferred,
    #[error("the StudyRun exhausted its frozen token budget before this request")]
    BudgetExhausted,
}

/// Reserves exactly one generic `analyze` ledger receipt for a leased batch.
///
/// The caller may make an external request only after this transaction commits. The receipt's
/// result intentionally records identities and lifecycle facts, never the comment text or model
/// prompt; the frozen batch is the authoritative request record.
pub async fn reserve_study_batch_model_call(
    database: &Database,
    batch_ref: Uuid,
    lease_token: Uuid,
) -> Result<ReservedStudyModelCall, StudyModelDispatchError> {
    let mut transaction = database.pool().begin().await?;
    let run_ref: Uuid = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_batch \
         WHERE batch_ref=$1 AND state='leased' AND lease_token=$2 \
           AND lease_expires_at>scope_001_now()",
    )
    .bind(batch_ref)
    .bind(lease_token)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(StudyModelDispatchError::BatchUnavailable)?;
    let locked_run: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked_run.is_none() {
        return Err(StudyModelDispatchError::BatchUnavailable);
    }
    let row = sqlx::query(
        "SELECT batch.run_ref,batch.input_hash,batch.input_manifest,batch.model_invocation_ref, \
                run.selection_manifest->>'contract' AS selection_contract, \
                to_jsonb(run)->>'dispatch_state' AS dispatch_state, \
                to_jsonb(run)->>'dispatch_reason' AS dispatch_reason, \
                to_jsonb(run)->'execution_manifest' AS execution_manifest, \
                to_jsonb(run)->>'token_limit' AS token_limit, \
                to_jsonb(policy)->'method_manifest' AS method_manifest, \
                to_jsonb(policy)->>'method_hash' AS method_hash, \
                policy.model_config_ref,config.input_token_limit,config.output_token_limit,config.timeout_seconds, \
                model.model_ref,model.model_id,version.version_ref,connection.enabled \
         FROM linggan_comment_study_batch batch \
         JOIN linggan_comment_study_run run ON run.run_ref=batch.run_ref \
         JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
         LEFT JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref \
         LEFT JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         LEFT JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         LEFT JOIN linggan_model_connection connection ON connection.connection_ref=version.connection_ref \
         WHERE batch.batch_ref=$1 AND batch.state='leased' AND batch.lease_token=$2 \
           AND batch.lease_expires_at>scope_001_now() FOR UPDATE OF batch",
    )
    .bind(batch_ref)
    .bind(lease_token)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(StudyModelDispatchError::BatchUnavailable)?;
    let selection_contract: String = row.get("selection_contract");
    if selection_contract == "comment-study.run-selection.v2"
        && (row.get::<Option<String>, _>("dispatch_state").as_deref() != Some("enabled")
            || row.get::<Option<String>, _>("dispatch_reason").is_some())
    {
        // A worker may have leased the batch just before a concurrent pause committed.
        // The Run row lock above serializes this decision with pause; if no model invocation
        // has been reserved, return the batch to prepared so resume can continue promptly
        // instead of waiting for the lease timeout. Never detach an existing invocation.
        sqlx::query(
            "UPDATE linggan_comment_study_batch \
             SET state='prepared',lease_token=NULL,leased_by=NULL,lease_expires_at=NULL \
             WHERE batch_ref=$1 AND state='leased' AND lease_token=$2 \
               AND model_invocation_ref IS NULL",
        )
        .bind(batch_ref)
        .bind(lease_token)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        return Err(StudyModelDispatchError::PreDispatchDeferred);
    }
    let config_ref = row
        .get::<Option<Uuid>, _>("model_config_ref")
        .ok_or(StudyModelDispatchError::ModelConfigurationMissing)?;
    let model_ref = row
        .get::<Option<Uuid>, _>("model_ref")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    let connection_version_ref = row
        .get::<Option<Uuid>, _>("version_ref")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    let model_id = row
        .get::<Option<String>, _>("model_id")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    if !row.get::<Option<bool>, _>("enabled").unwrap_or(false) {
        return Err(StudyModelDispatchError::ModelUnavailable);
    }
    let input_limit = row
        .get::<Option<i32>, _>("input_token_limit")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    let output_token_limit = row
        .get::<Option<i32>, _>("output_token_limit")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    let timeout_seconds = row
        .get::<Option<i32>, _>("timeout_seconds")
        .ok_or(StudyModelDispatchError::ModelUnavailable)?;
    let input_manifest = row.get::<serde_json::Value, _>("input_manifest");
    let v2_request = if selection_contract == "comment-study.run-selection.v2" {
        let policy_manifest = row
            .get::<Option<Value>, _>("method_manifest")
            .ok_or(StudyModelDispatchError::MethodUnavailable)?;
        let method_hash: String = row
            .get::<Option<String>, _>("method_hash")
            .ok_or(StudyModelDispatchError::MethodUnavailable)?;
        let execution_manifest = row
            .get::<Option<Value>, _>("execution_manifest")
            .ok_or(StudyModelDispatchError::MethodUnavailable)?;
        if execution_manifest["methodHash"].as_str() != Some(method_hash.as_str()) {
            return Err(StudyModelDispatchError::MethodUnavailable);
        }
        let manifest: StudyMethodManifest = serde_json::from_value(policy_manifest)
            .map_err(|_| StudyModelDispatchError::MethodUnavailable)?;
        let model_snapshot = StudyModelSnapshot {
            model_config_ref: config_ref,
            identity: StudyModelIdentity {
                model_ref,
                connection_version_ref,
                model_id: model_id.clone(),
            },
            input_token_limit: input_limit,
            output_token_limit,
            timeout_seconds,
        };
        let method = CompiledStudyMethod {
            manifest,
            method_hash,
        };
        verify_study_method(&method, &model_snapshot)
            .map_err(|_| StudyModelDispatchError::MethodUnavailable)?;
        let stage = &method.manifest.stages.semantic;
        let (prompt, request_manifest) = semantic_model_request_manifest(
            batch_ref,
            row.get::<Uuid, _>("run_ref"),
            &input_manifest,
            &stage.system_instruction,
            &stage.output_schema,
        )
        .map_err(|_| StudyModelDispatchError::MethodUnavailable)?;
        let request_hash =
            json_hash(&request_manifest).map_err(|_| StudyModelDispatchError::MethodUnavailable)?;
        let request_input_tokens = conservative_token_estimate_json(&request_manifest);
        if request_input_tokens > i64::from(input_limit) {
            return Err(StudyModelDispatchError::InputLimit);
        }
        Some((
            stage.system_instruction.clone(),
            stage.output_schema.clone(),
            prompt,
            request_manifest,
            request_hash,
            request_input_tokens.saturating_add(i64::from(output_token_limit)),
        ))
    } else {
        None
    };
    let input_size = conservative_token_estimate_json(&input_manifest);
    if input_size > i64::from(input_limit) {
        return Err(StudyModelDispatchError::InputLimit);
    }
    let request_hash = v2_request
        .as_ref()
        .map(|request| request.4.clone())
        .unwrap_or_else(|| row.get::<String, _>("input_hash"));
    if let Some((_, _, _, _, _, reserved_tokens)) = &v2_request {
        let run_ref: Uuid = row.get("run_ref");
        let run_token_limit = row
            .get::<Option<String>, _>("token_limit")
            .and_then(|v| v.parse::<i64>().ok())
            .ok_or(StudyModelDispatchError::MethodUnavailable)?;
        let charged_tokens: i64 = sqlx::query_scalar(
             "SELECT COALESCE(SUM(CASE WHEN invocation.state='running' \
                                     AND (invocation.input_tokens IS NULL OR invocation.output_tokens IS NULL) \
                                     THEN GREATEST(invocation.reserved_tokens,COALESCE(invocation.charged_tokens,0)) \
                                     ELSE COALESCE(invocation.charged_tokens,invocation.reserved_tokens) END),0)::bigint \
             FROM linggan_model_invocation invocation \
             WHERE invocation.invocation_ref IN ( \
               SELECT request.invocation_ref FROM linggan_comment_study_model_request request WHERE request.run_ref=$1 \
               UNION SELECT resolution.model_invocation_ref \
                 FROM linggan_comment_study_resolution resolution \
                 JOIN linggan_comment_study_signal signal USING(signal_ref) \
                 JOIN linggan_comment_study_target target USING(target_ref) \
                 JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
                 WHERE target.run_ref=$1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                   AND resolution.model_invocation_ref IS NOT NULL \
               UNION SELECT pair.model_invocation_ref \
                 FROM linggan_comment_study_problem_pair pair \
                 JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
                 JOIN linggan_comment_study_target target USING(target_ref) \
                 JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
                 WHERE target.run_ref=$1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                   AND pair.model_invocation_ref IS NOT NULL \
               UNION SELECT legacy.invocation_ref FROM linggan_model_invocation legacy \
                 WHERE legacy.result->>'legacyRequestLedgerMissing'='true' \
                   AND legacy.result->>'legacyRunRef'=$1::text \
             )",
        )
        .bind(run_ref)
        .fetch_one(&mut *transaction)
        .await?;
        if charged_tokens.saturating_add(*reserved_tokens) > run_token_limit {
            let active_calls: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM linggan_model_invocation invocation \
                 WHERE invocation.state='running' AND invocation.invocation_ref IN ( \
                   SELECT request.invocation_ref FROM linggan_comment_study_model_request request WHERE request.run_ref=$1 \
                   UNION SELECT resolution.model_invocation_ref \
                     FROM linggan_comment_study_resolution resolution \
                     JOIN linggan_comment_study_signal signal USING(signal_ref) \
                     JOIN linggan_comment_study_target target USING(target_ref) \
                     JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
                     WHERE target.run_ref=$1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                       AND resolution.model_invocation_ref IS NOT NULL \
                   UNION SELECT pair.model_invocation_ref \
                     FROM linggan_comment_study_problem_pair pair \
                     JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
                     JOIN linggan_comment_study_target target USING(target_ref) \
                     JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
                     WHERE target.run_ref=$1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                       AND pair.model_invocation_ref IS NOT NULL \
                   UNION SELECT legacy.invocation_ref FROM linggan_model_invocation legacy \
                     WHERE legacy.result->>'legacyRequestLedgerMissing'='true' \
                       AND legacy.result->>'legacyRunRef'=$1::text \
                 )",
            )
            .bind(run_ref)
            .fetch_one(&mut *transaction)
            .await?;
            if active_calls > 0 {
                sqlx::query(
                    "UPDATE linggan_comment_study_batch SET state='prepared',lease_token=NULL, \
                     leased_by=NULL,lease_expires_at=NULL \
                     WHERE batch_ref=$1 AND state='leased' AND lease_token=$2 \
                       AND model_invocation_ref IS NULL",
                )
                .bind(batch_ref)
                .bind(lease_token)
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await?;
                return Err(StudyModelDispatchError::BudgetDeferred);
            }
            sqlx::query(
                "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
                 dispatch_reason='budget_exhausted',control_version=control_version+1 \
                 WHERE run_ref=$1 AND dispatch_state='enabled'",
            )
            .bind(run_ref)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE linggan_comment_study_batch SET state='cancelled', \
                 lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
                 output_manifest=jsonb_build_object('reason','budget_exhausted'), \
                 finished_at=scope_001_now() \
                 WHERE run_ref=$1 AND state IN ('prepared','leased') \
                   AND model_invocation_ref IS NULL",
            )
            .bind(run_ref)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE linggan_comment_study_target SET state='cancelled', \
                 finished_at=scope_001_now(),terminal_reason='budget_exhausted' \
                 WHERE run_ref=$1 AND state IN ('queued','running')",
            )
            .bind(run_ref)
            .execute(&mut *transaction)
            .await?;
            close_run_if_settled(&mut transaction, run_ref).await?;
            transaction.commit().await?;
            return Err(StudyModelDispatchError::BudgetExhausted);
        }
    }
    let invocation_ref = match row.get::<Option<Uuid>, _>("model_invocation_ref") {
        Some(existing) => {
            let running: bool = sqlx::query_scalar(
                "SELECT state='running' FROM linggan_model_invocation WHERE invocation_ref=$1",
            )
            .bind(existing)
            .fetch_optional(&mut *transaction)
            .await?
            .unwrap_or(false);
            if !running {
                return Err(StudyModelDispatchError::InvocationUnavailable);
            }
            existing
        }
        None => {
            let invocation_ref = Uuid::new_v4();
            let reserved_tokens =
                v2_request
                    .as_ref()
                    .map(|request| request.5)
                    .unwrap_or_else(|| {
                        i64::from(input_limit).saturating_add(i64::from(output_token_limit))
                    });
            sqlx::query(
                "INSERT INTO linggan_model_invocation( \
                   invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
                   state,reserved_tokens,charged_tokens,result \
                 ) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,0,$7)",
            )
            .bind(invocation_ref)
            .bind(connection_version_ref)
            .bind(model_ref)
            .bind(config_ref)
            .bind(&request_hash)
            .bind(reserved_tokens)
            .bind(json!({
                "runRef": row.get::<Uuid, _>("run_ref"),
                "batchRef": batch_ref,
                "stage": COMMENT_STUDY_SEMANTIC_STAGE,
                "callStarted": false
            }))
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE linggan_comment_study_batch SET model_invocation_ref=$2 \
                 WHERE batch_ref=$1 AND model_invocation_ref IS NULL",
            )
            .bind(batch_ref)
            .bind(invocation_ref)
            .execute(&mut *transaction)
            .await?;
            invocation_ref
        }
    };
    if let Some((_, _, _, request_manifest, request_hash, _)) = &v2_request {
        let existing: Option<(String, Value)> = sqlx::query_as(
            "SELECT request_hash,request_manifest FROM linggan_comment_study_model_request \
             WHERE invocation_ref=$1 AND batch_ref=$2 AND stage='semantic'",
        )
        .bind(invocation_ref)
        .bind(batch_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        match existing {
            Some((stored_hash, stored_manifest))
                if stored_hash == *request_hash && stored_manifest == *request_manifest => {}
            Some(_) => return Err(StudyModelDispatchError::RequestSnapshotUnavailable),
            None => {
                let run_ref: Uuid = row.get("run_ref");
                let policy_ref: Uuid = sqlx::query_scalar(
                    "SELECT policy_ref FROM linggan_comment_study_run WHERE run_ref=$1",
                )
                .bind(run_ref)
                .fetch_one(&mut *transaction)
                .await?;
                let inserted = sqlx::query(
                    "INSERT INTO linggan_comment_study_model_request( \
                       invocation_ref,run_ref,policy_ref,stage,batch_ref,attempt_ordinal, \
                       input_context_hash,request_manifest,request_hash,deadline_at) \
                     VALUES($1,$2,$3,'semantic',$4,1,$5,$6,$7, \
                       scope_001_now()+make_interval(secs=>$8)) ON CONFLICT DO NOTHING",
                )
                .bind(invocation_ref)
                .bind(run_ref)
                .bind(policy_ref)
                .bind(batch_ref)
                .bind(row.get::<String, _>("input_hash"))
                .bind(request_manifest)
                .bind(request_hash)
                .bind(timeout_seconds)
                .execute(&mut *transaction)
                .await?;
                if inserted.rows_affected() != 1 {
                    return Err(StudyModelDispatchError::RequestSnapshotUnavailable);
                }
            }
        }
    }
    transaction.commit().await?;
    Ok(ReservedStudyModelCall {
        invocation_ref,
        run_ref: row.get("run_ref"),
        batch_ref,
        config_ref,
        connection_version_ref,
        model_ref,
        model_id,
        output_token_limit,
        timeout_seconds,
        system_instruction: v2_request.as_ref().map(|request| request.0.clone()),
        output_schema: v2_request.as_ref().map(|request| request.1.clone()),
        prompt: v2_request.map(|request| request.2),
    })
}

/// Marks the exact v2 request as sent immediately before the adapter call. A retry cannot silently
/// spend a second provider call for the same frozen batch while its first response is uncertain.
pub async fn mark_study_batch_model_dispatch_started(
    database: &Database,
    invocation_ref: Uuid,
    batch_ref: Uuid,
    lease_token: Uuid,
) -> Result<i64, StudyModelDispatchError> {
    let mut transaction = database.pool().begin().await?;
    let run_ref: Uuid = sqlx::query_scalar(
        "SELECT batch.run_ref FROM linggan_comment_study_batch batch \
         WHERE batch.batch_ref=$1 AND batch.state='leased' AND batch.lease_token=$2 \
           AND batch.lease_expires_at>scope_001_now()",
    )
    .bind(batch_ref)
    .bind(lease_token)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(StudyModelDispatchError::InvocationUnavailable)?;
    let locked_run: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked_run.is_none() {
        return Err(StudyModelDispatchError::InvocationUnavailable);
    }
    let locked_batch: Option<Uuid> = sqlx::query_scalar(
        "SELECT batch_ref FROM linggan_comment_study_batch \
         WHERE batch_ref=$1 AND state='leased' AND lease_token=$2 \
           AND lease_expires_at>scope_001_now() AND model_invocation_ref=$3 \
         FOR UPDATE",
    )
    .bind(batch_ref)
    .bind(lease_token)
    .bind(invocation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked_batch.is_none() {
        return Err(StudyModelDispatchError::InvocationUnavailable);
    }
    let remaining_timeout_ms: Option<i64> = sqlx::query_scalar(
        "UPDATE linggan_comment_study_model_request request \
         SET dispatch_started_at=scope_001_now() \
         FROM linggan_comment_study_batch batch,linggan_comment_study_run run \
         WHERE request.invocation_ref=$1 AND request.batch_ref=$2 AND request.stage='semantic' \
           AND request.dispatch_started_at IS NULL \
           AND request.deadline_at>scope_001_now()+interval '250 milliseconds' \
           AND batch.batch_ref=request.batch_ref AND batch.run_ref=run.run_ref \
           AND batch.state='leased' AND batch.lease_token=$3 \
           AND batch.lease_expires_at>scope_001_now() \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL \
         RETURNING floor(extract(epoch FROM (request.deadline_at-request.dispatch_started_at))*1000)::bigint-250",
    )
    .bind(invocation_ref)
    .bind(batch_ref)
    .bind(lease_token)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(remaining_timeout_ms) = remaining_timeout_ms.filter(|remaining| *remaining > 0) else {
        let paused = sqlx::query_scalar::<_, bool>(
            "SELECT to_jsonb(run)->>'dispatch_state'='paused' \
             FROM linggan_comment_study_run run WHERE run.run_ref=$1",
        )
        .bind(run_ref)
        .fetch_optional(&mut *transaction)
        .await?
        .unwrap_or(false);
        if paused {
            let invocation = sqlx::query(
                "UPDATE linggan_model_invocation invocation \
                 SET state='failed',charged_tokens=0,failure_code='user_paused_before_dispatch', \
                     result=COALESCE(invocation.result,'{}'::jsonb)||jsonb_build_object( \
                       'ok',false,'callStarted',false,'failureCode','user_paused_before_dispatch'), \
                     finished_at=scope_001_now() \
                 WHERE invocation.invocation_ref=$1 AND invocation.state='running' \
                   AND invocation.result->>'callStarted'='false' \
                   AND EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
                              WHERE request.invocation_ref=invocation.invocation_ref \
                                AND request.batch_ref=$2 AND request.stage='semantic' \
                                AND request.dispatch_started_at IS NULL)",
            )
            .bind(invocation_ref)
            .bind(batch_ref)
            .execute(&mut *transaction)
            .await?;
            if invocation.rows_affected() == 1 {
                let batch = sqlx::query(
                    "UPDATE linggan_comment_study_batch \
                     SET state='cancelled',lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
                         output_manifest=jsonb_build_object('reason','user_paused_before_dispatch'), \
                         finished_at=scope_001_now() \
                     WHERE batch_ref=$1 AND run_ref=$2 AND state='leased' \
                       AND lease_token=$3 AND model_invocation_ref=$4",
                )
                .bind(batch_ref)
                .bind(run_ref)
                .bind(lease_token)
                .bind(invocation_ref)
                .execute(&mut *transaction)
                .await?;
                if batch.rows_affected() != 1 {
                    return Err(StudyModelDispatchError::InvocationUnavailable);
                }
                sqlx::query(
                    "UPDATE linggan_comment_study_target target \
                     SET state='queued',finished_at=NULL,terminal_reason=NULL \
                     FROM linggan_comment_study_batch_target member \
                     WHERE member.batch_ref=$1 AND member.target_ref=target.target_ref \
                       AND target.state='running'",
                )
                .bind(batch_ref)
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await?;
                return Err(StudyModelDispatchError::PreDispatchDeferred);
            }
        }
        return Err(StudyModelDispatchError::InvocationUnavailable);
    };
    let invocation_changed = sqlx::query(
        "UPDATE linggan_model_invocation SET result=COALESCE(result,'{}'::jsonb)||'{\"callStarted\":true}'::jsonb \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .execute(&mut *transaction)
    .await?;
    if invocation_changed.rows_affected() != 1 {
        return Err(StudyModelDispatchError::InvocationUnavailable);
    }
    transaction.commit().await?;
    Ok(remaining_timeout_ms)
}
