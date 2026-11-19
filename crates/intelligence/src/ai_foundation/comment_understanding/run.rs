//! A run is one batch of units. Its receipt and its cancellation, without any extra state: every
//! figure is derived from the unit items, the stored results and the invocation ledger.
use crate::ai_foundation::{
    batch_store,
    store::{Command, FoundationError},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub batch_ref: Uuid,
}

/// Stop what has not started. A unit already in a provider call settles normally, so its
/// comments are neither lost nor left half written.
pub async fn cancel(db: &Database, request: &Cancel) -> Result<Value, FoundationError> {
    let mut command = Command::begin(
        db,
        request.command_ref,
        "comment_understanding_cancel",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_unit u JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref WHERE i.batch_ref=$1 AND u.domain_ref=$2)")
        .bind(request.batch_ref)
        .bind(request.domain_ref)
        .fetch_one(&mut *command.tx)
        .await?;
    if !owned {
        return Err(FoundationError::NotFound);
    }
    batch_store::cancel_in(&mut command.tx, request.batch_ref).await?;
    command
        .finish(json!({"batchRef": request.batch_ref, "cancelRequested": true}))
        .await
}

/// What a run is doing and what it cost.
pub async fn read_run(db: &Database, domain: Uuid, batch: Uuid) -> Result<Value, FoundationError> {
    let units = sqlx::query("SELECT u.unit_ref,u.work_ref,c.state,c.failure_code,(SELECT count(*) FROM linggan_comment_research_unit_target t WHERE t.unit_ref=u.unit_ref) AS targets FROM linggan_comment_research_unit u JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref WHERE i.batch_ref=$1 AND u.domain_ref=$2 ORDER BY i.position")
        .bind(batch)
        .bind(domain)
        .fetch_all(db.pool())
        .await?;
    if units.is_empty() {
        return Err(FoundationError::NotFound);
    }
    // The same derivation the comment directory uses for a comment's latest study.
    let targets = sqlx::query("SELECT state,failure_code,count(*) AS n FROM (SELECT CASE WHEN x.result IS NOT NULL THEN 'succeeded' WHEN x.failure_code IS NOT NULL THEN 'failed' WHEN c.state IN('pending','running') THEN 'running' WHEN c.state='cancelled' THEN 'cancelled' ELSE 'failed' END AS state,COALESCE(x.failure_code,CASE WHEN x.request_ref IS NULL THEN c.failure_code END) AS failure_code FROM linggan_comment_research_unit_target ut JOIN linggan_comment_research_unit u ON u.unit_ref=ut.unit_ref JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref LEFT JOIN linggan_comment_research_result x ON x.request_ref=ut.request_ref WHERE i.batch_ref=$1) t GROUP BY state,failure_code ORDER BY state,failure_code")
        .bind(batch)
        .fetch_all(db.pool())
        .await?;
    let usage = sqlx::query("SELECT count(*) AS calls,COALESCE(sum(input_tokens),0)::bigint AS input,COALESCE(sum(output_tokens),0)::bigint AS output FROM linggan_model_invocation WHERE batch_ref=$1 AND task_key='comment_understanding' AND dispatch_state IS DISTINCT FROM 'not_dispatched'")
        .bind(batch)
        .fetch_one(db.pool())
        .await?;
    let cancel_requested: bool = sqlx::query_scalar(
        "SELECT cancel_requested FROM linggan_ai_batch_control WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await?;
    Ok(json!({
        "batchRef": batch,
        "cancelRequested": cancel_requested,
        "units": units.iter().map(|row| Ok(json!({
            "unitRef": row.try_get::<Uuid, _>("unit_ref")?,
            "workRef": row.try_get::<Uuid, _>("work_ref")?,
            "state": row.try_get::<String, _>("state")?,
            "failureCode": row.try_get::<Option<String>, _>("failure_code")?,
            "targetCount": row.try_get::<i64, _>("targets")?,
        }))).collect::<Result<Vec<_>, sqlx::Error>>()?,
        "targets": targets.iter().map(|row| Ok(json!({
            "state": row.try_get::<String, _>("state")?,
            "failureCode": row.try_get::<Option<String>, _>("failure_code")?,
            "count": row.try_get::<i64, _>("n")?,
        }))).collect::<Result<Vec<_>, sqlx::Error>>()?,
        "usage": {
            "modelCalls": usage.try_get::<i64, _>("calls")?,
            "inputTokens": usage.try_get::<i64, _>("input")?,
            "outputTokens": usage.try_get::<i64, _>("output")?,
        },
    }))
}
