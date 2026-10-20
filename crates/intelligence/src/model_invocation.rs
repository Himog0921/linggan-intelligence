//! Generic provider invocation ledger used by explicit model executors.
//! Invocation receipts deliberately retain transport and usage facts, not source text.

use crate::{
    model_secrets::ModelSecretStore,
    model_settings::*,
    pi_adapter::{PiRequest, PiResponse},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

/// Retired provider preparation cannot load an old credential or construct a dispatchable request.
pub async fn connection_request(
    _database: &Database,
    _secrets: &dyn ModelSecretStore,
    _connection_version_ref: Uuid,
) -> Result<PiRequest, ModelError> {
    Err(ModelError::LegacyRetired)
}

pub async fn finish_invocation(
    database: &Database,
    invocation_ref: Uuid,
    response: Option<&PiResponse>,
    succeeded: bool,
    failure_code: Option<&str>,
    result: &Value,
) -> Result<(), ModelError> {
    let mut transaction = database.pool().begin().await?;
    let probe_target = sqlx::query(
        "SELECT model_ref,connection_version_ref,operation \
         FROM linggan_model_invocation WHERE invocation_ref=$1 FOR UPDATE",
    )
    .bind(invocation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(target) = probe_target.filter(|target| {
        target.get::<String, _>("operation") == "probe"
            && target.get::<Option<Uuid>, _>("model_ref").is_some()
    }) {
        lock_research_model_probe_completion_in_transaction(
            &mut transaction,
            target.get("model_ref"),
            target.get("connection_version_ref"),
        )
        .await?;
    }
    finish_invocation_in(
        &mut *transaction,
        invocation_ref,
        response,
        succeeded,
        failure_code,
        result,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn finish_invocation_in(
    connection: &mut sqlx::PgConnection,
    invocation_ref: Uuid,
    response: Option<&PiResponse>,
    succeeded: bool,
    failure_code: Option<&str>,
    result: &Value,
) -> Result<(), ModelError> {
    response.map(|r| r.usage.ledger_usage()).transpose()?;
    let input_tokens = response.and_then(|response| response.usage.input_tokens);
    let output_tokens = response.and_then(|response| response.usage.output_tokens);
    let measured = input_tokens
        .unwrap_or(0)
        .checked_add(output_tokens.unwrap_or(0))
        .ok_or(ModelError::Invalid)?;
    let charged = input_tokens
        .zip(output_tokens)
        .and_then(|(input, output)| input.checked_add(output));
    sqlx::query(
        "UPDATE linggan_model_invocation \
         SET state=$2,input_tokens=COALESCE($3,input_tokens),output_tokens=COALESCE($4,output_tokens),\
             cached_input_tokens=COALESCE($11,cached_input_tokens),cached_write_tokens=COALESCE($12,cached_write_tokens),reasoning_tokens=COALESCE($13,reasoning_tokens),\
             charged_tokens=COALESCE($5,GREATEST(reserved_tokens,$9)),elapsed_ms=COALESCE($6,elapsed_ms),\
             failure_code=$7,result=COALESCE(result,'{}'::jsonb)||$8\
               ||CASE WHEN $10::jsonb IS NULL THEN '{}'::jsonb ELSE jsonb_build_object('diagnostic',$10::jsonb) END,finished_at=scope_001_now() \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .bind(if succeeded { "succeeded" } else { "failed" })
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(charged)
    .bind(response.and_then(|response| response.elapsed_ms))
    .bind(failure_code)
    .bind(result)
    .bind(measured)
    .bind(
        response
            .and_then(|response| response.diagnostic.as_ref())
            .map(|diagnostic| {
                serde_json::to_value(diagnostic).expect("Pi diagnostic is serializable")
            }),
    )
    .bind(response.and_then(|r|r.usage.cached_input_tokens))
    .bind(response.and_then(|r|r.usage.cached_write_tokens))
    .bind(response.and_then(|r|r.usage.reasoning_tokens))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn checkpoint_invocation_usage(
    database: &Database,
    invocation_ref: Uuid,
    response: Option<&PiResponse>,
) -> Result<(), ModelError> {
    response.map(|r| r.usage.ledger_usage()).transpose()?;
    let input_tokens = response.and_then(|response| response.usage.input_tokens);
    let output_tokens = response.and_then(|response| response.usage.output_tokens);
    let measured = input_tokens
        .unwrap_or(0)
        .checked_add(output_tokens.unwrap_or(0))
        .ok_or(ModelError::Invalid)?;
    let charged = input_tokens
        .zip(output_tokens)
        .and_then(|(input, output)| input.checked_add(output));
    sqlx::query(
        "UPDATE linggan_model_invocation \
         SET input_tokens=COALESCE($2,input_tokens),output_tokens=COALESCE($3,output_tokens),\
             cached_input_tokens=COALESCE($8,cached_input_tokens),cached_write_tokens=COALESCE($9,cached_write_tokens),reasoning_tokens=COALESCE($10,reasoning_tokens),\
             charged_tokens=COALESCE($4,GREATEST(reserved_tokens,$7)),elapsed_ms=COALESCE($5,elapsed_ms),\
             result=COALESCE(result,'{}'::jsonb)||$6 \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(charged)
    .bind(response.and_then(|response| response.elapsed_ms))
    .bind(json!({
        "callStarted": true,
        "validationPending": true,
        "diagnostic": response.and_then(|response| response.diagnostic.as_ref())
    }))
    .bind(measured)
    .bind(response.and_then(|r|r.usage.cached_input_tokens))
    .bind(response.and_then(|r|r.usage.cached_write_tokens))
    .bind(response.and_then(|r|r.usage.reasoning_tokens))
    .execute(database.pool())
    .await?;
    Ok(())
}
