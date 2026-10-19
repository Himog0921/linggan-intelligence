//! A late child response can add bounded usage evidence, never output or a terminal ledger rewrite.
use super::{ExecutorError, queue::ClaimedItem};
use crate::pi_adapter::PiResponse;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Transaction};
use std::{future::Future, time::Duration};
use uuid::Uuid;

/// The provider response remains in this future. Only its database settlement is retried.
/// A token written atomically with the terminal rows resolves an ambiguous commit response.
pub(super) async fn finalize<F, Fut>(
    db: &Database,
    item: &ClaimedItem,
    invocation: Uuid,
    response: Option<&PiResponse>,
    mut action: F,
) -> Result<(), ExecutorError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), ExecutorError>>,
{
    let work = async {
        loop {
            match action().await {
                Ok(()) => return Ok(()),
                Err(error) if error.is_database() || matches!(error, ExecutorError::FenceLost) => {
                    let committed = was_committed(db, item, invocation).await;
                    if matches!(committed, Ok(Some(true))) {
                        return Ok(());
                    }
                    if matches!(error, ExecutorError::FenceLost) {
                        return Err(error);
                    }
                    let _ = item.renew(db).await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error),
            }
        }
    };
    let allowance = Duration::from_secs(
        linggan_contracts::ai_limits::AI_LIMITS.receipt_finalization_allowance_seconds,
    );
    let started = tokio::time::Instant::now();
    let result = tokio::time::timeout(allowance.saturating_sub(Duration::from_secs(1)), work)
        .await
        .unwrap_or(Err(ExecutorError::ReceiptTimedOut));
    if result.is_err() {
        // Preserve a minimal actual receipt when the publication transaction cannot settle.
        // It cannot release quota or publish output, and it shares the same total deadline.
        let _ = tokio::time::timeout(
            allowance.saturating_sub(started.elapsed()),
            append_late_usage(db, item, invocation, response),
        )
        .await;
    }
    result
}

async fn was_committed(
    db: &Database,
    item: &ClaimedItem,
    invocation: Uuid,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(i.state IN ('succeeded','failed')
        AND i.result->>'executorReceipt'=$3 AND c.state IN ('succeeded','failed','cancelled')
        AND c.invocation_ref=i.invocation_ref AND c.lease_generation=$4,false)
        FROM linggan_model_invocation i JOIN linggan_ai_batch_item_control c ON c.item_ref=$2
        WHERE i.invocation_ref=$1",
    )
    .bind(invocation)
    .bind(item.item_ref)
    .bind(item.lease_token.to_string())
    .bind(item.lease_generation)
    .fetch_optional(db.pool())
    .await
}

pub(super) async fn lock_in<'a>(
    db: &'a Database,
    item: &ClaimedItem,
    invocation: Uuid,
    response: Option<&PiResponse>,
) -> Result<(Transaction<'a, Postgres>, bool), ExecutorError> {
    let mut tx = match item.owner.begin(db, false).await {
        Ok(tx) => tx,
        Err(ExecutorError::FenceLost) => {
            append_late_usage(db, item, invocation, response).await?;
            return Err(ExecutorError::FenceLost);
        }
        Err(error) => return Err(error),
    };
    let cancelled = match item.lock_in(&mut tx).await {
        Ok(cancelled) => cancelled,
        Err(ExecutorError::FenceLost) => {
            tx.rollback().await?;
            append_late_usage(db, item, invocation, response).await?;
            return Err(ExecutorError::FenceLost);
        }
        Err(error) => return Err(error),
    };
    Ok((tx, cancelled))
}
pub(super) fn not_dispatched(response: Option<&PiResponse>) -> bool {
    response.is_some_and(|r| r.diagnostic.as_ref().and_then(|d| d.request_sent) == Some(false))
}
pub(super) async fn observe_in(
    tx: &mut Transaction<'_, Postgres>,
    invocation: Uuid,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    if let Some(sent) = response
        .and_then(|r| r.diagnostic.as_ref())
        .and_then(|d| d.request_sent)
    {
        sqlx::query("UPDATE linggan_model_invocation SET dispatch_state=$2 WHERE invocation_ref=$1 AND state='running'")
            .bind(invocation).bind(if sent {"started"} else {"not_dispatched"}).execute(&mut **tx).await?;
    }
    Ok(())
}
pub(super) struct ItemResult<'a> {
    pub success: bool,
    pub cancelled: bool,
    pub output: Option<&'a serde_json::Value>,
    pub failure: Option<&'a str>,
    pub counts_as_failure: bool,
}
pub(super) async fn write_item_in(
    tx: &mut Transaction<'_, Postgres>,
    item: &ClaimedItem,
    result: ItemResult<'_>,
) -> Result<(), ExecutorError> {
    let usable = result.success && !result.cancelled;
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state=$2,usable=$3,output=$4,failure_code=$5,counts_as_failure=$6,
        lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,finished_at=scope_001_now()
        WHERE item_ref=$1")
        .bind(item.item_ref).bind(if result.cancelled {"cancelled"} else if usable {"succeeded"} else {"failed"}).bind(usable)
        .bind(if usable {result.output} else {None}).bind(result.failure).bind(result.counts_as_failure&&!result.cancelled&&!usable)
        .execute(&mut **tx).await?;
    crate::ai_foundation::rounds::evaluate_in(tx, item.batch_ref).await?;
    Ok(())
}

pub(super) async fn append_late_usage(
    db: &Database,
    item: &ClaimedItem,
    invocation: Uuid,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    if was_committed(db, item, invocation).await? == Some(true) {
        return Ok(());
    }
    let Some(response) = response else {
        return Ok(());
    };
    response.usage.ledger_usage()?;
    let usage = &response.usage;
    if usage.input_tokens.is_none() && usage.output_tokens.is_none() {
        return Ok(());
    }
    let inserted=sqlx::query("INSERT INTO linggan_ai_late_usage_receipt(invocation_ref,item_ref,instance_ref,lease_generation,
        input_tokens,output_tokens,cached_input_tokens,cached_write_tokens,reasoning_tokens)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(invocation_ref) DO NOTHING")
        .bind(invocation).bind(item.item_ref).bind(item.owner.instance_ref()).bind(item.lease_generation)
        .bind(usage.input_tokens).bind(usage.output_tokens).bind(usage.cached_input_tokens)
        .bind(usage.cached_write_tokens).bind(usage.reasoning_tokens).execute(db.pool()).await?;
    if inserted.rows_affected() == 0 {
        let identical:bool=sqlx::query_scalar("SELECT item_ref=$2 AND instance_ref=$3 AND lease_generation=$4
            AND input_tokens IS NOT DISTINCT FROM $5 AND output_tokens IS NOT DISTINCT FROM $6
            AND cached_input_tokens IS NOT DISTINCT FROM $7 AND cached_write_tokens IS NOT DISTINCT FROM $8
            AND reasoning_tokens IS NOT DISTINCT FROM $9 FROM linggan_ai_late_usage_receipt WHERE invocation_ref=$1")
            .bind(invocation).bind(item.item_ref).bind(item.owner.instance_ref()).bind(item.lease_generation)
            .bind(usage.input_tokens).bind(usage.output_tokens).bind(usage.cached_input_tokens)
            .bind(usage.cached_write_tokens).bind(usage.reasoning_tokens).fetch_one(db.pool()).await?;
        if !identical {
            return Err(ExecutorError::Invalid);
        }
    }
    Ok(())
}
