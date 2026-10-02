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
const MAX_EXPIRED_BATCHES_PER_TICK: i64 = 32;

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
    let mut last_run_ref = None;
    claim_next_study_batch_after(database, worker_ref, lease_seconds, &mut last_run_ref).await
}

/// Claims one prepared batch using a process-local circular Run cursor and `FOR UPDATE SKIP
/// LOCKED`. This keeps a Run with many prepared batches from monopolizing a worker after restart
/// or after older queued work becomes eligible. Cursor loss is harmless: it affects order only.
pub async fn claim_next_study_batch_after(
    database: &Database,
    worker_ref: Uuid,
    lease_seconds: i64,
    last_run_ref: &mut Option<Uuid>,
) -> Result<Option<ClaimedStudyBatch>, StudyBatchWorkerError> {
    if !(1..=300).contains(&lease_seconds) {
        return Err(StudyBatchWorkerError::InvalidLeaseDuration);
    }
    let mut transaction = database.pool().begin().await?;
    let run_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT run.run_ref FROM linggan_comment_study_run run \
         WHERE EXISTS(SELECT 1 FROM linggan_comment_study_batch batch \
                     WHERE batch.run_ref=run.run_ref AND batch.state='prepared') \
           AND (run.selection_manifest->>'contract'='comment-study.run-selection.v1' \
                OR (run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                    AND to_jsonb(run)->>'dispatch_state'='enabled' \
                    AND to_jsonb(run)->>'dispatch_reason' IS NULL)) \
         ORDER BY CASE WHEN $1::uuid IS NULL THEN \
                           (SELECT min(batch.created_at) FROM linggan_comment_study_batch batch \
                            WHERE batch.run_ref=run.run_ref AND batch.state='prepared') \
                       END NULLS LAST, \
                  CASE WHEN $1::uuid IS NULL THEN 0 \
                       WHEN run.run_ref>$1 THEN 0 ELSE 1 END,run.run_ref \
         LIMIT 1 FOR UPDATE OF run SKIP LOCKED",
    )
    .bind(*last_run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(run_ref) = run_ref else {
        transaction.commit().await?;
        return Ok(None);
    };
    *last_run_ref = Some(run_ref);
    let row = sqlx::query(
        "SELECT batch_ref,run_ref,input_manifest FROM linggan_comment_study_batch \
         WHERE run_ref=$1 AND state='prepared' ORDER BY created_at,batch_ref \
         LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(run_ref)
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
    let has_model_request_table: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_model_request') IS NOT NULL")
            .fetch_one(&mut *transaction)
            .await?;
    let run_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_batch \
         WHERE state='leased' AND lease_expires_at<=scope_001_now() \
         GROUP BY run_ref ORDER BY min(lease_expires_at),run_ref LIMIT $1",
    )
    .bind(MAX_EXPIRED_BATCHES_PER_TICK)
    .fetch_all(&mut *transaction)
    .await?;
    let mut recovered = 0_u64;
    let mut remaining = MAX_EXPIRED_BATCHES_PER_TICK;
    for run_ref in run_refs {
        let locked_run: Option<Uuid> = sqlx::query_scalar(
            "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE SKIP LOCKED",
        )
        .bind(run_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        if locked_run.is_none() {
            continue;
        }
        let batches = sqlx::query(
            "SELECT batch_ref,run_ref,model_invocation_ref FROM linggan_comment_study_batch \
             WHERE run_ref=$1 AND state='leased' AND lease_expires_at<=scope_001_now() \
             ORDER BY lease_expires_at,batch_ref LIMIT $2 FOR UPDATE SKIP LOCKED",
        )
        .bind(run_ref)
        .bind(remaining)
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
                let result = json!({
                    "stage":"comment-study.semantic.v1",
                    "batchRef":batch_ref,
                    "leaseExpired":true
                });
                if has_model_request_table {
                    sqlx::query(
                        "UPDATE linggan_model_invocation invocation \
                         SET charged_tokens=CASE \
                               WHEN EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
                                           WHERE request.invocation_ref=invocation.invocation_ref \
                                             AND request.dispatch_started_at IS NULL) THEN 0 \
                               WHEN invocation.input_tokens IS NOT NULL AND invocation.output_tokens IS NOT NULL \
                                 THEN COALESCE(invocation.charged_tokens,invocation.reserved_tokens) \
                               ELSE GREATEST(invocation.reserved_tokens,COALESCE(invocation.charged_tokens,0)) \
                             END, \
                             state='failed',failure_code='lease_expired', \
                             result=COALESCE(result,'{}'::jsonb)||$2,finished_at=scope_001_now() \
                         WHERE invocation.invocation_ref=$1 AND invocation.state='running'",
                    )
                    .bind(invocation_ref)
                    .bind(result)
                    .execute(&mut *transaction)
                    .await?;
                } else {
                    // Before the request ledger exists there is no durable fence proving that a
                    // leased call was never dispatched. Keep the reservation charged unless full
                    // measured usage is already present.
                    sqlx::query(
                        "UPDATE linggan_model_invocation invocation \
                         SET charged_tokens=CASE \
                               WHEN invocation.input_tokens IS NOT NULL AND invocation.output_tokens IS NOT NULL \
                                 THEN COALESCE(invocation.charged_tokens,invocation.reserved_tokens) \
                               ELSE GREATEST(invocation.reserved_tokens,COALESCE(invocation.charged_tokens,0)) \
                             END, \
                             state='failed',failure_code='lease_expired', \
                             result=COALESCE(result,'{}'::jsonb)||$2,finished_at=scope_001_now() \
                         WHERE invocation.invocation_ref=$1 AND invocation.state='running'",
                    )
                    .bind(invocation_ref)
                    .bind(result)
                    .execute(&mut *transaction)
                    .await?;
                }
            }
            // Recovery can be what makes a run's last target terminal, so the run has to be able to
            // close here too, not only on the acceptance path.
            close_run_if_settled(&mut transaction, run_ref).await?;
            recovered += 1;
        }
        remaining -= i64::try_from(batches.len()).unwrap_or(remaining);
        if remaining <= 0 {
            break;
        }
    }
    transaction.commit().await?;
    Ok(recovered)
}

pub(crate) async fn sources_remain_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    let invalid: i64 = sqlx::query_scalar(
        "SELECT count(*) \
         FROM linggan_comment_study_batch_target member \
         JOIN linggan_comment_study_target target ON target.target_ref=member.target_ref \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         LEFT JOIN linggan_material_content_author work_author \
           ON work_author.content_public_ref=source.content_public_ref \
         LEFT JOIN linggan_material_comment_restriction restriction \
           ON restriction.content_public_ref=source.content_public_ref \
          AND restriction.comment_external_id=source.comment_external_id \
         LEFT JOIN linggan_material_comment parent ON parent.material_ref=target.parent_source_ref \
         LEFT JOIN linggan_material_comment_restriction parent_restriction \
           ON parent_restriction.content_public_ref=parent.content_public_ref \
          AND parent_restriction.comment_external_id=parent.comment_external_id \
         WHERE member.batch_ref=$1 \
           AND (source.body_state<>'KNOWN' OR source.body_text IS NULL OR btrim(source.body_text)='' \
                OR restriction.comment_external_id IS NOT NULL \
                OR parent_restriction.comment_external_id IS NOT NULL \
                OR NULLIF(btrim(source.author_external_id),'') IS NULL \
                OR NULLIF(btrim(work_author.author_external_id),'') IS NULL \
                OR btrim(source.author_external_id)=btrim(work_author.author_external_id))",
    )
    .bind(batch_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(invalid == 0)
}

pub(crate) async fn unqualified_batch_target_refs(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT target.target_ref \
         FROM linggan_comment_study_batch_target member \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         LEFT JOIN linggan_material_content_author work_author \
           ON work_author.content_public_ref=source.content_public_ref \
         LEFT JOIN linggan_material_comment_restriction restriction \
           ON restriction.content_public_ref=source.content_public_ref \
          AND restriction.comment_external_id=source.comment_external_id \
         LEFT JOIN linggan_material_comment parent ON parent.material_ref=target.parent_source_ref \
         LEFT JOIN linggan_material_comment_restriction parent_restriction \
           ON parent_restriction.content_public_ref=parent.content_public_ref \
          AND parent_restriction.comment_external_id=parent.comment_external_id \
         WHERE member.batch_ref=$1 \
           AND (source.body_state<>'KNOWN' OR source.body_text IS NULL OR btrim(source.body_text)='' \
                OR restriction.comment_external_id IS NOT NULL \
                OR parent_restriction.comment_external_id IS NOT NULL \
                OR NULLIF(btrim(source.author_external_id),'') IS NULL \
                OR NULLIF(btrim(work_author.author_external_id),'') IS NULL \
                OR btrim(source.author_external_id)=btrim(work_author.author_external_id)) \
         ORDER BY target.target_ref",
    )
    .bind(batch_ref)
    .fetch_all(&mut **transaction)
    .await
}

pub(crate) async fn cancel_unqualified_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    batch_ref: Uuid,
) -> Result<(), sqlx::Error> {
    let has_productization_columns: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='linggan_comment_study_target' \
           AND column_name='terminal_reason')",
    )
    .fetch_one(&mut **transaction)
    .await?;
    if has_productization_columns {
        sqlx::query(
            "WITH qualification AS MATERIALIZED ( \
               SELECT member.target_ref, \
                 (source.body_state='KNOWN' AND source.body_text IS NOT NULL \
                  AND btrim(source.body_text)<>'' AND restriction.comment_external_id IS NULL \
                  AND parent_restriction.comment_external_id IS NULL \
                  AND NULLIF(btrim(source.author_external_id),'') IS NOT NULL \
                  AND NULLIF(btrim(work_author.author_external_id),'') IS NOT NULL \
                  AND btrim(source.author_external_id)<>btrim(work_author.author_external_id)) AS qualified \
               FROM linggan_comment_study_batch_target member \
               JOIN linggan_comment_study_target target USING(target_ref) \
               JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
               LEFT JOIN linggan_material_content_author work_author \
                 ON work_author.content_public_ref=source.content_public_ref \
               LEFT JOIN linggan_material_comment_restriction restriction \
                 ON restriction.content_public_ref=source.content_public_ref \
                AND restriction.comment_external_id=source.comment_external_id \
               LEFT JOIN linggan_material_comment parent ON parent.material_ref=target.parent_source_ref \
               LEFT JOIN linggan_material_comment_restriction parent_restriction \
                 ON parent_restriction.content_public_ref=parent.content_public_ref \
                AND parent_restriction.comment_external_id=parent.comment_external_id \
               WHERE member.batch_ref=$1 AND target.state='running' \
             ) \
             UPDATE linggan_comment_study_target target \
             SET state=CASE WHEN qualification.qualified THEN 'queued' ELSE 'excluded' END, \
                 dependency_state=CASE WHEN qualification.qualified THEN target.dependency_state ELSE 'input_invalid' END, \
                 exclusion_reason=CASE WHEN qualification.qualified THEN NULL ELSE 'source_unavailable_after_freeze' END, \
                 finished_at=CASE WHEN qualification.qualified THEN NULL ELSE scope_001_now() END, \
                 terminal_reason=CASE WHEN qualification.qualified THEN NULL ELSE 'source_unavailable' END \
             FROM qualification WHERE target.target_ref=qualification.target_ref AND target.state='running'",
        )
        .bind(batch_ref)
        .execute(&mut **transaction)
        .await?;
    } else {
        sqlx::query(
            "WITH qualification AS MATERIALIZED ( \
               SELECT member.target_ref, \
                 (source.body_state='KNOWN' AND source.body_text IS NOT NULL \
                  AND btrim(source.body_text)<>'' AND restriction.comment_external_id IS NULL \
                  AND parent_restriction.comment_external_id IS NULL \
                  AND NULLIF(btrim(source.author_external_id),'') IS NOT NULL \
                  AND NULLIF(btrim(work_author.author_external_id),'') IS NOT NULL \
                  AND btrim(source.author_external_id)<>btrim(work_author.author_external_id)) AS qualified \
               FROM linggan_comment_study_batch_target member \
               JOIN linggan_comment_study_target target USING(target_ref) \
               JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
               LEFT JOIN linggan_material_content_author work_author \
                 ON work_author.content_public_ref=source.content_public_ref \
               LEFT JOIN linggan_material_comment_restriction restriction \
                 ON restriction.content_public_ref=source.content_public_ref \
                AND restriction.comment_external_id=source.comment_external_id \
               LEFT JOIN linggan_material_comment parent ON parent.material_ref=target.parent_source_ref \
               LEFT JOIN linggan_material_comment_restriction parent_restriction \
                 ON parent_restriction.content_public_ref=parent.content_public_ref \
                AND parent_restriction.comment_external_id=parent.comment_external_id \
               WHERE member.batch_ref=$1 AND target.state='running' \
             ) \
             UPDATE linggan_comment_study_target target \
             SET state=CASE WHEN qualification.qualified THEN 'queued' ELSE 'excluded' END, \
                 dependency_state=CASE WHEN qualification.qualified THEN target.dependency_state ELSE 'input_invalid' END, \
                 exclusion_reason=CASE WHEN qualification.qualified THEN NULL ELSE 'source_unavailable_after_freeze' END \
             FROM qualification WHERE target.target_ref=qualification.target_ref AND target.state='running'",
        )
        .bind(batch_ref)
        .execute(&mut **transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET state='cancelled',lease_token=NULL,leased_by=NULL,lease_expires_at=NULL, \
             output_manifest=$2,finished_at=scope_001_now() WHERE batch_ref=$1",
    )
    .bind(batch_ref)
    .bind(json!({"reason":"source_unavailable_after_freeze"}))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
