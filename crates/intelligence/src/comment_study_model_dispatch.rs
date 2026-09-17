//! Auditable reservation of one provider call for one leased comment-study batch.
//!
//! This boundary neither obtains a secret nor calls a provider. It freezes the generic model
//! ledger receipt before an adapter receives the immutable batch envelope, and keeps the receipt
//! linked to every per-target semantic attempt that the batch later admits.

use crate::comment_study_batch::conservative_token_estimate_json;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::json;
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
    let row = sqlx::query(
        "SELECT batch.run_ref,batch.input_hash,batch.input_manifest,batch.model_invocation_ref, \
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
    let input_size = conservative_token_estimate_json(&input_manifest);
    if input_size > i64::from(input_limit) {
        return Err(StudyModelDispatchError::InputLimit);
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
                i64::from(input_limit).saturating_add(i64::from(output_token_limit));
            sqlx::query(
                "INSERT INTO linggan_model_invocation( \
                   invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
                   state,reserved_tokens,charged_tokens,result \
                 ) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)",
            )
            .bind(invocation_ref)
            .bind(connection_version_ref)
            .bind(model_ref)
            .bind(config_ref)
            .bind(row.get::<String, _>("input_hash"))
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
    })
}
