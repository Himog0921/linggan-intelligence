//! Human batch cancellation is distinct from pausing future dispatch at any switch level.
use super::store::{Command, FoundationError, valid_reason};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelBatch {
    pub command_ref: Uuid,
    pub batch_ref: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelReceipt {
    pub batch_ref: Uuid,
    pub revision: i64,
    pub state: String,
    pub cancel_requested: bool,
}
pub async fn cancel_batch(
    db: &Database,
    request: &CancelBatch,
) -> Result<CancelReceipt, FoundationError> {
    if request.expected_revision < 0 || !valid_reason(&request.reason, 300) {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "batch_cancel", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_batch_control WHERE batch_ref=$1 FOR UPDATE",
    )
    .bind(request.batch_ref)
    .fetch_optional(&mut *command.tx)
    .await?
    .ok_or(FoundationError::NotFound)?;
    if revision != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    super::batch_store::cancel_in(&mut command.tx, request.batch_ref).await?;
    let row = sqlx::query(
        "SELECT revision,state,cancel_requested FROM linggan_ai_batch_control WHERE batch_ref=$1",
    )
    .bind(request.batch_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    command
        .finish(CancelReceipt {
            batch_ref: request.batch_ref,
            revision: row.get("revision"),
            state: row.get("state"),
            cancel_requested: row.get("cancel_requested"),
        })
        .await
}

pub(crate) async fn recent_in(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Value, FoundationError> {
    Ok(sqlx::query_scalar("SELECT COALESCE(jsonb_agg(row ORDER BY created_at DESC,batch_ref DESC),'[]') FROM (
        SELECT b.created_at,b.batch_ref,jsonb_build_object('batchRef',b.batch_ref,'taskKey',b.task_key,
            'taskConfigRef',b.task_config_ref,'kind',b.kind,'createdAt',b.created_at,'itemCount',b.item_count,
            'state',c.state,'revision',c.revision,'cancelRequested',c.cancel_requested,
            'stopReason',c.stop_reason,'lastRoundNo',c.last_round_no,'finishedAt',c.finished_at,
            'pending',(SELECT count(*) FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control item USING(item_ref) WHERE i.batch_ref=b.batch_ref AND item.state='pending'),
            'running',(SELECT count(*) FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control item USING(item_ref) WHERE i.batch_ref=b.batch_ref AND item.state='running')) AS row
        FROM linggan_ai_batch b JOIN linggan_ai_batch_control c USING(batch_ref)
        ORDER BY b.created_at DESC,b.batch_ref DESC LIMIT 50) batches")
        .fetch_one(&mut **tx).await?)
}

pub async fn read_batch(
    db: &Database,
    batch: Uuid,
    from_position: u32,
) -> Result<Value, FoundationError> {
    if from_position >= linggan_contracts::ai_limits::AI_LIMITS.max_batch_items {
        return Err(FoundationError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let batch_row:Value=sqlx::query_scalar("SELECT jsonb_build_object('batchRef',b.batch_ref,'taskKey',b.task_key,
        'taskConfigRef',b.task_config_ref,'kind',b.kind,'itemCount',b.item_count,'createdAt',b.created_at,
        'state',c.state,'revision',c.revision,'cancelRequested',c.cancel_requested,'stopReason',c.stop_reason,
        'lastRoundNo',c.last_round_no,'finishedAt',c.finished_at)
        FROM linggan_ai_batch b JOIN linggan_ai_batch_control c USING(batch_ref) WHERE b.batch_ref=$1")
        .bind(batch).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let items:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(row ORDER BY position),'[]') FROM (
        SELECT item.position,jsonb_build_object('itemRef',item.item_ref,'position',item.position,
            'state',c.state,'attemptCount',c.attempt_count,'usable',c.usable,'invocationRef',c.invocation_ref,
            'failureCode',c.failure_code,'waitReason',c.wait_reason,'nextAttemptAt',c.next_attempt_at,
            'leaseExpiresAt',c.lease_expires_at,'finishedAt',c.finished_at) AS row
        FROM linggan_ai_batch_item item JOIN linggan_ai_batch_item_control c USING(item_ref)
        WHERE item.batch_ref=$1 AND item.position>=$2 ORDER BY item.position LIMIT 100)items")
        .bind(batch).bind(from_position as i32).fetch_one(&mut *tx).await?;
    let rounds:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('roundRef',round_ref,
        'roundNo',round_no,'attempted',attempted,'failed',failed,'usable',usable,
        'consecutiveWithoutUsable',consecutive_without_usable,'brakeReason',brake_reason) ORDER BY round_no),'[]')
        FROM linggan_ai_round WHERE batch_ref=$1").bind(batch).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        serde_json::json!({"batch":batch_row,"items":items,"rounds":rounds,"fromPosition":from_position,"pageSize":100}),
    )
}
