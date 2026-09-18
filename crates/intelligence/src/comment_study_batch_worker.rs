//! Lease ownership and safe recovery for comment-study model batches.
//!
//! This module deliberately does not call a provider. It gives a worker a short-lived, immutable
//! batch envelope; the provider request happens outside the database transaction, and any later
//! result must present the exact lease token before admission.

use crate::comment_study_batch_acceptance::settle_dispatched_batch_targets;
use crate::comment_study_run::close_run_if_settled;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_BATCH_LEASE_SECONDS: i64 = 60;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedStudyBatch {
    pub batch_ref: Uuid,
    pub lease_token: Uuid,
    pub worker_ref: Uuid,
    pub lease_expires_at: String,
    pub input_manifest: Value,
}

#[derive(Debug, Error)]
pub enum StudyBatchWorkerError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the requested batch lease duration is outside the safe range")]
    InvalidLeaseDuration,
}

/// Claims one prepared batch using `FOR UPDATE SKIP LOCKED`; callers must perform all provider
/// I/O after this function has committed. A batch whose source became restricted since freezing is
/// cancelled without sending any of its text to a model.
pub async fn claim_next_study_batch(
    database: &Database,
    worker_ref: Uuid,
    lease_seconds: i64,
) -> Result<Option<ClaimedStudyBatch>, StudyBatchWorkerError> {
    if !(1..=300).contains(&lease_seconds) {
        return Err(StudyBatchWorkerError::InvalidLeaseDuration);
    }
    let mut transaction = database.pool().begin().await?;
    let row = sqlx::query(
        "SELECT batch_ref,run_ref,input_manifest FROM linggan_comment_study_batch \
         WHERE state='prepared' ORDER BY created_at,batch_ref LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(row) = row else {
        transaction.commit().await?;
        return Ok(None);
    };
    let batch_ref: Uuid = row.get("batch_ref");
    if sources_remain_qualified(&mut transaction, batch_ref).await? {
        let lease_token = Uuid::new_v4();
        let expires_at: String =
            sqlx::query_scalar("SELECT (scope_001_now()+make_interval(secs=>$1))::text")
                .bind(lease_seconds)
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_batch \
             SET state='leased',lease_token=$2,leased_by=$3,lease_expires_at=$4::timestamptz \
             WHERE batch_ref=$1 AND state='prepared'",
        )
        .bind(batch_ref)
        .bind(lease_token)
        .bind(worker_ref)
        .bind(&expires_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        return Ok(Some(ClaimedStudyBatch {
            batch_ref,
            lease_token,
            worker_ref,
            lease_expires_at: expires_at,
            input_manifest: row.get("input_manifest"),
        }));
    }
    cancel_unqualified_batch(&mut transaction, batch_ref).await?;
    // Every target of this run may have just become `excluded`, which is a run that never had
    // anything left to study rather than one still waiting on a model.
    close_run_if_settled(&mut transaction, row.get("run_ref")).await?;
    transaction.commit().await?;
    Ok(None)
}

/// Recovers expired work without accepting a late result. The expired lease token is cleared and
/// every still-running target returns to `queued`, allowing a new batch to be frozen later.
pub async fn recover_expired_study_batch_leases(
    database: &Database,
) -> Result<u64, StudyBatchWorkerError> {
    let mut transaction = database.pool().begin().await?;
    let batches = sqlx::query(
        "SELECT batch_ref,run_ref,model_invocation_ref FROM linggan_comment_study_batch \
         WHERE state='leased' AND lease_expires_at<=scope_001_now() FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *transaction)
    .await?;
    for batch in &batches {
        let batch_ref: Uuid = batch.get("batch_ref");
        let model_invocation_ref: Option<Uuid> = batch.get("model_invocation_ref");
        // An expired lease means a dispatch was made, or may have been, and no result came back.
        // Returning the targets straight to `queued` recorded nothing, so a batch that always
        // outlives its lease re-dispatched on real, billed calls without ever exhausting a bound.
        // The attempt is therefore counted conservatively, exactly as a rejected response is.
        settle_dispatched_batch_targets(
            &mut transaction,
            batch_ref,
            model_invocation_ref,
            "lease_expired",
            None,
        )
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_batch \
             SET state='failed',lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
                 output_manifest=jsonb_build_object('reason','lease_expired'),finished_at=scope_001_now() \
             WHERE batch_ref=$1 AND state='leased'",
        )
        .bind(batch_ref)
        .execute(&mut *transaction)
        .await?;
        if let Some(invocation_ref) = model_invocation_ref {
            sqlx::query(
                "UPDATE linggan_model_invocation \
                 SET state='failed',failure_code='lease_expired', \
                     result=COALESCE(result,'{}'::jsonb)||$2,finished_at=scope_001_now() \
                 WHERE invocation_ref=$1 AND state='running'",
            )
            .bind(invocation_ref)
            .bind(json!({
                "stage":"comment-study.semantic.v1",
                "batchRef":batch_ref,
                "leaseExpired":true
            }))
            .execute(&mut *transaction)
            .await?;
        }
        // Recovery can be what makes a run's last target terminal, so the run has to be able to
        // close here too, not only on the acceptance path.
        close_run_if_settled(&mut transaction, batch.get("run_ref")).await?;
    }
    transaction.commit().await?;
    Ok(u64::try_from(batches.len()).unwrap_or(0))
}

async fn sources_remain_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    let invalid: i64 = sqlx::query_scalar(
        "SELECT count(*) \
         FROM linggan_comment_study_batch_target member \
         JOIN linggan_comment_study_target target ON target.target_ref=member.target_ref \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         LEFT JOIN linggan_material_comment_restriction restriction \
           ON restriction.content_public_ref=source.content_public_ref \
          AND restriction.comment_external_id=source.comment_external_id \
         WHERE member.batch_ref=$1 \
           AND (source.body_state<>'KNOWN' OR source.body_text IS NULL OR btrim(source.body_text)='' \
                OR restriction.comment_external_id IS NOT NULL)",
    )
    .bind(batch_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(invalid == 0)
}

async fn cancel_unqualified_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE linggan_comment_study_target target \
         SET state=CASE WHEN source.body_state='KNOWN' AND source.body_text IS NOT NULL \
                             AND btrim(source.body_text)<>'' AND restriction.comment_external_id IS NULL \
                        THEN 'queued' ELSE 'excluded' END, \
             dependency_state=CASE WHEN source.body_state='KNOWN' AND source.body_text IS NOT NULL \
                                        AND btrim(source.body_text)<>'' AND restriction.comment_external_id IS NULL \
                                   THEN target.dependency_state ELSE 'input_invalid' END, \
             exclusion_reason=CASE WHEN source.body_state='KNOWN' AND source.body_text IS NOT NULL \
                                        AND btrim(source.body_text)<>'' AND restriction.comment_external_id IS NULL \
                                   THEN NULL ELSE 'source_unavailable_after_freeze' END \
         FROM linggan_comment_study_batch_target member, \
              linggan_material_comment source \
         LEFT JOIN linggan_material_comment_restriction restriction \
           ON restriction.content_public_ref=source.content_public_ref \
          AND restriction.comment_external_id=source.comment_external_id \
         WHERE member.batch_ref=$1 AND target.target_ref=member.target_ref \
           AND source.material_ref=target.source_ref AND target.state='running'",
    )
    .bind(batch_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET state='cancelled',output_manifest=$2,finished_at=scope_001_now() WHERE batch_ref=$1",
    )
    .bind(batch_ref)
    .bind(json!({"reason":"source_unavailable_after_freeze"}))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
