//! Transactional, per-target admission for a completed same-work batch.
//!
//! The batch response is stored once on the batch. Each target receives its own bounded semantic
//! attempt and state transition, so a missing or malformed sibling cannot erase valid target
//! Signals or masquerade as a no-signal outcome.

use crate::comment_study_batch::{
    BatchTargetState, ParsedBatchTarget, StudyBatchError, parse_batch_output,
};
use crate::comment_study_semantic::AcceptedSignal;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

const MAX_SEMANTIC_ATTEMPTS: i32 = 3;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAcceptanceReceipt {
    pub batch_ref: Uuid,
    pub state: String,
    pub accepted_target_count: usize,
    pub retried_target_count: usize,
    pub failed_target_count: usize,
}

#[derive(Debug, Error)]
pub enum BatchAcceptanceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    BatchContract(#[from] StudyBatchError),
    #[error("the batch is absent, already terminal, or its frozen target is no longer readable")]
    BatchUnavailable,
}

struct LockedBatch {
    run_ref: Uuid,
    content_public_ref: Uuid,
    model_invocation_ref: Option<Uuid>,
    targets: BTreeMap<Uuid, String>,
}

/// Accepts the valid part of a batch response and makes omitted targets retriable. A malformed
/// outer response is a bounded rejected attempt for every target in that batch, never no_signal.
pub async fn accept_study_batch_output(
    database: &Database,
    batch_ref: Uuid,
    lease_token: Uuid,
    raw_output: Value,
) -> Result<BatchAcceptanceReceipt, BatchAcceptanceError> {
    let mut transaction = database.pool().begin().await?;
    let batch = lock_batch(&mut transaction, batch_ref, lease_token).await?;
    let parsed = match parse_batch_output(
        raw_output.clone(),
        batch_ref,
        batch.content_public_ref,
        &batch.targets,
    ) {
        Ok(parsed) => parsed,
        Err(error) => {
            let (_retried_target_count, _failed_target_count) = reject_all_targets(
                &mut transaction,
                batch_ref,
                batch.model_invocation_ref,
                &batch.targets,
                "semantic_batch_contract",
            )
            .await?;
            finish_batch(&mut transaction, batch_ref, "failed", raw_output).await?;
            finish_model_invocation(
                &mut transaction,
                batch.model_invocation_ref,
                false,
                Some("semantic_batch_contract"),
                json!({
                    "contract":"comment-study.note-batch.v1",
                    "runRef":batch.run_ref,
                    "batchRef":batch_ref,
                    "stage":"comment-study.semantic.v1",
                    "acceptedTargetCount":0
                }),
            )
            .await?;
            transaction.commit().await?;
            return Err(BatchAcceptanceError::BatchContract(error));
        }
    };
    let mut accepted_target_count = 0;
    for target in parsed.targets {
        accept_target(
            &mut transaction,
            batch_ref,
            batch.model_invocation_ref,
            target,
        )
        .await?;
        accepted_target_count += 1;
    }
    let mut retried_target_count = 0;
    let mut failed_target_count = 0;
    for target_ref in parsed.missing_target_refs {
        let state = reject_target(
            &mut transaction,
            batch_ref,
            batch.model_invocation_ref,
            target_ref,
            "semantic_target_missing",
        )
        .await?;
        if state == "queued" {
            retried_target_count += 1;
        } else {
            failed_target_count += 1;
        }
    }
    let state = if retried_target_count == 0 && failed_target_count == 0 {
        "accepted"
    } else {
        "completed_with_failures"
    };
    finish_batch(&mut transaction, batch_ref, state, raw_output).await?;
    finish_model_invocation(
        &mut transaction,
        batch.model_invocation_ref,
        true,
        None,
        json!({
            "contract":"comment-study.note-batch.v1",
            "runRef":batch.run_ref,
            "batchRef":batch_ref,
            "stage":"comment-study.semantic.v1",
            "acceptedTargetCount":accepted_target_count,
            "retriedTargetCount":retried_target_count,
            "failedTargetCount":failed_target_count
        }),
    )
    .await?;
    transaction.commit().await?;
    Ok(BatchAcceptanceReceipt {
        batch_ref,
        state: state.to_owned(),
        accepted_target_count,
        retried_target_count,
        failed_target_count,
    })
}

async fn lock_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    lease_token: Uuid,
) -> Result<LockedBatch, BatchAcceptanceError> {
    let batch = sqlx::query(
        "SELECT run_ref,content_public_ref,model_invocation_ref FROM linggan_comment_study_batch \
         WHERE batch_ref=$1 AND state='leased' AND lease_token=$2 \
           AND lease_expires_at>scope_001_now() FOR UPDATE",
    )
    .bind(batch_ref)
    .bind(lease_token)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(BatchAcceptanceError::BatchUnavailable)?;
    let rows = sqlx::query(
        "SELECT target.target_ref,source.body_text,source.body_state,target.state \
         FROM linggan_comment_study_batch_target member \
         JOIN linggan_comment_study_target target ON target.target_ref=member.target_ref \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         WHERE member.batch_ref=$1 FOR UPDATE OF target",
    )
    .bind(batch_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let mut targets = BTreeMap::new();
    for row in rows {
        if row.get::<String, _>("state") != "running"
            || row.get::<String, _>("body_state") != "KNOWN"
        {
            return Err(BatchAcceptanceError::BatchUnavailable);
        }
        let source = row
            .get::<Option<String>, _>("body_text")
            .filter(|text| !text.trim().is_empty())
            .ok_or(BatchAcceptanceError::BatchUnavailable)?;
        targets.insert(row.get("target_ref"), source);
    }
    if targets.is_empty() {
        return Err(BatchAcceptanceError::BatchUnavailable);
    }
    Ok(LockedBatch {
        run_ref: batch.get("run_ref"),
        content_public_ref: batch.get("content_public_ref"),
        model_invocation_ref: batch.get("model_invocation_ref"),
        targets,
    })
}

async fn accept_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    model_invocation_ref: Option<Uuid>,
    target: ParsedBatchTarget,
) -> Result<(), sqlx::Error> {
    let ordinal = next_attempt_ordinal(transaction, target.target_ref).await?;
    let manifest = json!({
        "contract":"comment-study.note-batch.v1",
        "batchRef":batch_ref,
        "targetRef":target.target_ref,
        "outcome":match target.state {
            BatchTargetState::Succeeded => "signals",
            BatchTargetState::NoSignal => "no_signal",
            BatchTargetState::NeedsContext => "needs_context",
        },
        "reason":target.reason,
    });
    insert_attempt(
        transaction,
        batch_ref,
        target.target_ref,
        ordinal,
        "accepted",
        manifest,
        None,
        model_invocation_ref,
    )
    .await?;
    insert_signals(transaction, target.target_ref, batch_ref, target.signals).await?;
    let state = match target.state {
        BatchTargetState::Succeeded => "succeeded",
        BatchTargetState::NoSignal => "no_signal",
        BatchTargetState::NeedsContext => "needs_context",
    };
    update_target_state(transaction, target.target_ref, state).await
}

async fn reject_all_targets(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    model_invocation_ref: Option<Uuid>,
    targets: &BTreeMap<Uuid, String>,
    rejection_code: &str,
) -> Result<(usize, usize), sqlx::Error> {
    let mut retry = 0;
    let mut failed = 0;
    for target_ref in targets.keys().copied() {
        let state = reject_target(
            transaction,
            batch_ref,
            model_invocation_ref,
            target_ref,
            rejection_code,
        )
        .await?;
        if state == "queued" {
            retry += 1;
        } else {
            failed += 1;
        }
    }
    Ok((retry, failed))
}

async fn reject_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    model_invocation_ref: Option<Uuid>,
    target_ref: Uuid,
    rejection_code: &str,
) -> Result<&'static str, sqlx::Error> {
    let ordinal = next_attempt_ordinal(transaction, target_ref).await?;
    let next_state = if ordinal == MAX_SEMANTIC_ATTEMPTS {
        "failed"
    } else {
        "queued"
    };
    insert_attempt(
        transaction,
        batch_ref,
        target_ref,
        ordinal,
        "rejected",
        json!({"contract":"comment-study.note-batch.v1","batchRef":batch_ref,"targetRef":target_ref}),
        Some(rejection_code),
        model_invocation_ref,
    )
    .await?;
    update_target_state(transaction, target_ref, next_state).await?;
    Ok(next_state)
}

async fn next_attempt_ordinal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<i32, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(i32::try_from(count).unwrap_or(MAX_SEMANTIC_ATTEMPTS) + 1)
}

async fn insert_attempt(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    target_ref: Uuid,
    ordinal: i32,
    state: &str,
    output_manifest: Value,
    rejection_code: Option<&str>,
    model_invocation_ref: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_comment_study_semantic_attempt( \
           attempt_ref,target_ref,batch_ref,model_invocation_ref,attempt_ordinal,request_hash,state,output_manifest,rejection_code,finished_at \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,scope_001_now())",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(batch_ref)
    .bind(model_invocation_ref)
    .bind(ordinal)
    .bind("0000000000000000000000000000000000000000000000000000000000000000")
    .bind(state)
    .bind(output_manifest)
    .bind(rejection_code)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_signals(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    batch_ref: Uuid,
    signals: Vec<AcceptedSignal>,
) -> Result<(), sqlx::Error> {
    let attempt_ref: Uuid = sqlx::query_scalar(
        "SELECT attempt_ref FROM linggan_comment_study_semantic_attempt \
         WHERE batch_ref=$1 AND target_ref=$2 ORDER BY attempt_ordinal DESC LIMIT 1",
    )
    .bind(batch_ref)
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await?;
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

async fn update_target_state(
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

async fn finish_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
    state: &str,
    output_manifest: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET state=$2,output_manifest=$3,lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
             finished_at=scope_001_now() WHERE batch_ref=$1",
    )
    .bind(batch_ref)
    .bind(state)
    .bind(output_manifest)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn finish_model_invocation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invocation_ref: Option<Uuid>,
    succeeded: bool,
    failure_code: Option<&str>,
    result: Value,
) -> Result<(), sqlx::Error> {
    let Some(invocation_ref) = invocation_ref else {
        return Ok(());
    };
    sqlx::query(
        "UPDATE linggan_model_invocation \
         SET state=$2,failure_code=$3,result=COALESCE(result,'{}'::jsonb)||$4,finished_at=scope_001_now() \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .bind(if succeeded { "succeeded" } else { "failed" })
    .bind(failure_code)
    .bind(result)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
