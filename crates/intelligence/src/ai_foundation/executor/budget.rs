//! Serialize each task's day admission and bind insertion to that same database instant.
//! A midnight boundary cannot make the reservation read one day and charge another.
use super::ExecutorError;
use crate::ai_foundation::{TaskKey, store::FoundationError};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(super) async fn admit_in(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
    reservation: u64,
) -> Result<String, ExecutorError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_database()||':'||current_schema()||':ai-budget:'||$1,4540139))")
        .bind(task.as_str()).execute(&mut **tx).await?;
    let admitted_at: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let config=sqlx::query("SELECT budget_mode,daily_token_limit FROM linggan_ai_task_config WHERE task_config_ref=$1 AND task_key=$2")
        .bind(config).bind(task.as_str()).fetch_one(&mut **tx).await?;
    let held:String=sqlx::query_scalar("SELECT COALESCE(sum(charged_tokens),0)::text FROM linggan_model_invocation WHERE task_key=$1 AND budget_day=linggan_ai_budget_day($2::text::timestamptz)")
        .bind(task.as_str()).bind(&admitted_at).fetch_one(&mut **tx).await?;
    let total = held
        .parse::<u64>()
        .ok()
        .and_then(|held| held.checked_add(reservation))
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or(ExecutorError::Invalid)?;
    let mode: String = config.get("budget_mode");
    if mode == "unconfigured"
        || (mode == "capped" && total > config.get::<i64, _>("daily_token_limit") as u64)
    {
        return Err(FoundationError::AdmissionDenied(vec![
            if mode == "unconfigured" {
                "daily_budget_unconfigured"
            } else {
                "daily_budget_exhausted"
            }
            .into(),
        ])
        .into());
    }
    Ok(admitted_at)
}
pub(super) async fn latest_price_in(
    tx: &mut Transaction<'_, Postgres>,
    model: Option<Uuid>,
    endpoint: Uuid,
) -> Result<Option<Uuid>, ExecutorError> {
    Ok(sqlx::query_scalar("SELECT price.price_ref FROM linggan_model_price price
        JOIN linggan_ai_model_identity identity USING(identity_ref)
        JOIN linggan_model_connection_version v ON v.connection_ref=identity.connection_ref
        JOIN linggan_model_entry m ON m.connection_version_ref=v.version_ref AND m.model_id=identity.model_id
        WHERE m.model_ref=$1 AND price.endpoint_ref=$2 ORDER BY price.revision_sequence DESC LIMIT 1")
        .bind(model).bind(endpoint).fetch_optional(&mut **tx).await?)
}

/// Reserve a conservative USD amount before dispatch. The database serializes creative tasks
/// and leaves ambiguous calls charged at their reservation until a known cost is available.
pub(super) async fn reserve_creative_in(
    tx: &mut Transaction<'_, Postgres>,
    invocation: Uuid,
    price: Option<Uuid>,
    tokens: i64,
    admitted_at: &str,
) -> Result<(), ExecutorError> {
    let outcome: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,$3,$4::text::timestamptz)")
            .bind(invocation)
            .bind(price)
            .bind(tokens)
            .bind(admitted_at)
            .fetch_one(&mut **tx)
            .await?;
    if outcome != "reserved" {
        return Err(FoundationError::AdmissionDenied(vec![outcome]).into());
    }
    Ok(())
}
