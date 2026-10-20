//! Historical suggestions retain their frozen input lineage and current source qualification.
use super::{AiError, PreviewRequest, qualify_frozen_inputs};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn status(db: &Database, id: Uuid) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let r=sqlx::query("SELECT request.*,p.work_ref,p.input_revision,p.action,p.method_version,p.input_manifest,
        i.input_tokens,i.output_tokens,i.charged_tokens,i.usage_state,i.measured_tokens,i.cached_input_tokens,i.cached_write_tokens,i.reasoning_tokens,
        i.cost_pico_usd::text AS cost_quote,work.current_revision_ref
        FROM linggan_creation_ai_request request JOIN linggan_creation_ai_preview p USING(preview_ref)
        LEFT JOIN linggan_model_invocation i ON i.invocation_ref=request.invocation_ref
        JOIN linggan_creation_work work ON work.work_ref=p.work_ref WHERE request.request_ref=$1")
        .bind(id).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
    let input: PreviewRequest =
        serde_json::from_value(r.get("input_manifest")).map_err(|_| AiError::Invalid)?;
    let readable = qualify_frozen_inputs(&mut tx, &input).await?;
    tx.commit().await?;
    Ok(
        json!({"requestId":id,"workRef":r.get::<Uuid,_>("work_ref"),"invocationRef":r.get::<Option<Uuid>,_>("invocation_ref"),
        "batchRef":r.get::<Option<Uuid>,_>("batch_ref"),"taskConfigRef":r.get::<Option<Uuid>,_>("task_config_ref"),"state":r.get::<String,_>("state"),
        "inputRevision":r.get::<Uuid,_>("input_revision"),"action":r.get::<String,_>("action"),"methodVersion":r.get::<String,_>("method_version"),
        "suggestion":if readable {r.get::<Option<Value>,_>("suggestion")} else {None},"sourceQualification":if readable {"readable"} else {"unavailable"},
        "failureCode":r.get::<Option<String>,_>("failure_code"),"staleBase":r.get::<Uuid,_>("input_revision")!=r.get::<Uuid,_>("current_revision_ref"),
        "usage":{"state":r.get::<Option<String>,_>("usage_state").unwrap_or_else(||"not_reserved".into()),
            "measuredTokens":r.get::<Option<i64>,_>("measured_tokens"),"cachedInputTokens":r.get::<Option<i64>,_>("cached_input_tokens"),
            "cachedWriteTokens":r.get::<Option<i64>,_>("cached_write_tokens"),"reasoningTokens":r.get::<Option<i64>,_>("reasoning_tokens"),
            "inputTokens":r.get::<Option<i64>,_>("input_tokens"),"outputTokens":r.get::<Option<i64>,_>("output_tokens"),
            "chargedTokens":r.get::<Option<i64>,_>("charged_tokens"),"costUsd":null,"costPicoUsd":r.get::<Option<String>,_>("cost_quote")}}),
    )
}
