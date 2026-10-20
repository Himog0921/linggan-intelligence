//! A bounded, read-only ledger. Legacy receipts stay visible without the retired defaults API.
use super::{TaskKey, store::FoundationError};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryQuery {
    pub before: Option<Uuid>,
    pub task_key: Option<TaskKey>,
}

pub async fn read_history(db: &Database, query: &HistoryQuery) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    if let Some(reference) = query.before {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_model_invocation WHERE invocation_ref=$1)",
        )
        .bind(reference)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(FoundationError::NotFound);
        }
    }
    let mut rows: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object(
        'invocationRef',i.invocation_ref,'taskKey',i.task_key,'taskConfigRef',i.task_config_ref,'modelConfigRef',i.config_ref,
        'batchRef',i.batch_ref,'roundNo',i.round_no,'operation',i.operation,'state',i.state,
        'connectionVersionRef',i.connection_version_ref,'modelId',m.model_id,'connectionName',v.name,
        'usageState',i.usage_state,'dispatchState',i.dispatch_state,'budgetDay',i.budget_day,
        'inputTokens',i.input_tokens::text,'outputTokens',i.output_tokens::text,
        'measuredTokens',i.measured_tokens::text,'cachedInputTokens',i.cached_input_tokens::text,
        'cacheWriteTokens',i.cached_write_tokens::text,'reasoningTokens',i.reasoning_tokens::text,
        'quotaHeldTokens',i.charged_tokens::text,'legacyChargedTokens',i.legacy_charged_tokens::text,
        'quotePicoUsd',i.cost_pico_usd::text,'failureCode',i.failure_code,
        'createdAt',i.created_at,'finishedAt',i.finished_at)
        FROM linggan_model_invocation i JOIN linggan_model_connection_version v ON v.version_ref=i.connection_version_ref
        LEFT JOIN linggan_model_entry m ON m.model_ref=i.model_ref
        WHERE ($1::uuid IS NULL OR (i.created_at,i.invocation_ref)<(SELECT created_at,invocation_ref FROM linggan_model_invocation WHERE invocation_ref=$1))
          AND ($2::text IS NULL OR i.task_key=$2)
        ORDER BY i.created_at DESC,i.invocation_ref DESC LIMIT 51")
        .bind(query.before).bind(query.task_key.map(TaskKey::as_str)).fetch_all(&mut *tx).await?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    let next_cursor = if has_more {
        rows.last()
            .and_then(|row| row.get("invocationRef"))
            .cloned()
    } else {
        None
    };
    tx.commit().await?;
    Ok(
        json!({"contract":"ai-invocation-history.v1","items":rows,"pageSize":50,"hasMore":has_more,"nextCursor":next_cursor}),
    )
}
