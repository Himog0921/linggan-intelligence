//! Transactional admission for one structured Study semantic output.
//!
//! This boundary owns no provider call. A worker must first lease a target, then hand the raw
//! response to this function. The result is accepted completely or rejected completely; no
//! malformed response can leave a partial list of signals behind.

use crate::comment_study_semantic::{SemanticContractError, accept_semantic_output};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

const MAX_SEMANTIC_ATTEMPTS: i32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticAcceptanceReceipt {
    pub attempt_ref: Uuid,
    pub target_ref: Uuid,
    pub state: String,
    pub signal_count: usize,
    pub rejection_code: Option<String>,
}

#[derive(Debug, Error)]
pub enum SemanticAcceptanceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the target is absent or is not leased for semantic acceptance")]
    TargetNotLeased,
    #[error("the target has exhausted its bounded semantic attempts")]
    AttemptsExhausted,
    #[error("the frozen source comment is not readable")]
    SourceUnavailable,
    #[error("the request hash is not a SHA-256 hex value")]
    InvalidRequestHash,
}

/// Stores one accepted or rejected output and advances exactly its leased target.
pub async fn accept_target_output(
    database: &Database,
    target_ref: Uuid,
    request_hash: &str,
    model_invocation_ref: Option<Uuid>,
    raw_output: Value,
) -> Result<SemanticAcceptanceReceipt, SemanticAcceptanceError> {
    if !is_sha256(request_hash) {
        return Err(SemanticAcceptanceError::InvalidRequestHash);
    }
    let mut transaction = database.pool().begin().await?;
    let source = locked_target_source(&mut transaction, target_ref).await?;
    let attempt_ordinal = next_attempt_ordinal(&mut transaction, target_ref).await?;
    let validation = accept_semantic_output(raw_output.clone(), &source);
    let attempt_ref = Uuid::new_v4();
    let receipt = match validation {
        Ok(signals) => {
            let signal_count = signals.len();
            insert_attempt(
                &mut transaction,
                attempt_ref,
                target_ref,
                attempt_ordinal,
                model_invocation_ref,
                request_hash,
                "accepted",
                Some(raw_output),
                None,
            )
            .await?;
            insert_signals(&mut transaction, attempt_ref, target_ref, signals).await?;
            complete_target(
                &mut transaction,
                target_ref,
                if signal_count == 0 {
                    "no_signal"
                } else {
                    "succeeded"
                },
            )
            .await?;
            SemanticAcceptanceReceipt {
                attempt_ref,
                target_ref,
                state: "accepted".to_owned(),
                signal_count,
                rejection_code: None,
            }
        }
        Err(error) => {
            rejected_receipt(
                &mut transaction,
                attempt_ref,
                target_ref,
                attempt_ordinal,
                model_invocation_ref,
                request_hash,
                raw_output,
                error,
            )
            .await?
        }
    };
    transaction.commit().await?;
    Ok(receipt)
}

async fn locked_target_source(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<String, SemanticAcceptanceError> {
    let row = sqlx::query(
        "SELECT source.body_text,source.body_state,target.state \
         FROM linggan_comment_study_target target \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         WHERE target.target_ref=$1 FOR UPDATE OF target",
    )
    .bind(target_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(SemanticAcceptanceError::TargetNotLeased)?;
    if row.get::<String, _>("state") != "running" {
        return Err(SemanticAcceptanceError::TargetNotLeased);
    }
    if row.get::<String, _>("body_state") != "KNOWN" {
        return Err(SemanticAcceptanceError::SourceUnavailable);
    }
    row.get::<Option<String>, _>("body_text")
        .filter(|value| !value.trim().is_empty())
        .ok_or(SemanticAcceptanceError::SourceUnavailable)
}

async fn next_attempt_ordinal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<i32, SemanticAcceptanceError> {
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await?;
    let ordinal = i32::try_from(completed).unwrap_or(MAX_SEMANTIC_ATTEMPTS) + 1;
    if ordinal > MAX_SEMANTIC_ATTEMPTS {
        return Err(SemanticAcceptanceError::AttemptsExhausted);
    }
    Ok(ordinal)
}

#[allow(clippy::too_many_arguments)]
async fn insert_attempt(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attempt_ref: Uuid,
    target_ref: Uuid,
    attempt_ordinal: i32,
    model_invocation_ref: Option<Uuid>,
    request_hash: &str,
    state: &str,
    output_manifest: Option<Value>,
    rejection_code: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_comment_study_semantic_attempt( \
           attempt_ref,target_ref,attempt_ordinal,model_invocation_ref,request_hash,state, \
           output_manifest,rejection_code,finished_at \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,scope_001_now())",
    )
    .bind(attempt_ref)
    .bind(target_ref)
    .bind(attempt_ordinal)
    .bind(model_invocation_ref)
    .bind(request_hash)
    .bind(state)
    .bind(output_manifest)
    .bind(rejection_code)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_signals(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attempt_ref: Uuid,
    target_ref: Uuid,
    signals: Vec<crate::comment_study_semantic::AcceptedSignal>,
) -> Result<(), sqlx::Error> {
    for signal in signals {
        sqlx::query(
            "INSERT INTO linggan_comment_study_signal( \
               signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence, \
               evidence_start,evidence_end,problem_frame,eligibility_state,eligibility_reason \
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(Uuid::new_v4())
        .bind(target_ref)
        .bind(attempt_ref)
        .bind(signal.kind)
        .bind(signal.proposition)
        .bind(signal.evidence)
        .bind(signal.evidence_start)
        .bind(signal.evidence_end)
        .bind(signal.problem_frame)
        .bind(signal.eligibility_state)
        .bind(signal.eligibility_reason)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn complete_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    state: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE linggan_comment_study_target SET state=$2 WHERE target_ref=$1")
        .bind(target_ref)
        .bind(state)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn rejected_receipt(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attempt_ref: Uuid,
    target_ref: Uuid,
    attempt_ordinal: i32,
    model_invocation_ref: Option<Uuid>,
    request_hash: &str,
    raw_output: Value,
    error: SemanticContractError,
) -> Result<SemanticAcceptanceReceipt, SemanticAcceptanceError> {
    let code = error.rejection_code();
    insert_attempt(
        transaction,
        attempt_ref,
        target_ref,
        attempt_ordinal,
        model_invocation_ref,
        request_hash,
        "rejected",
        Some(raw_output),
        Some(code),
    )
    .await?;
    let next_state = if attempt_ordinal == MAX_SEMANTIC_ATTEMPTS {
        "failed"
    } else {
        "queued"
    };
    complete_target(transaction, target_ref, next_state).await?;
    Ok(SemanticAcceptanceReceipt {
        attempt_ref,
        target_ref,
        state: "rejected".to_owned(),
        signal_count: 0,
        rejection_code: Some(code.to_owned()),
    })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
