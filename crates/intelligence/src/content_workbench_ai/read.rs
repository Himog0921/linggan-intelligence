//! Historical suggestions retain their frozen input lineage and current source qualification.
use super::{AiError, PreviewRequest, qualify_frozen_inputs};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn status(db: &Database, id: Uuid) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;
    // Acquire the withdrawal gate before reading, using READ COMMITTED so a waiter cannot
    // retain a snapshot taken before a completed withdrawal.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *tx)
        .await?;
    let r=sqlx::query("SELECT request.*,p.work_ref,p.input_revision,p.action,p.method_version,p.input_manifest,
        private.manifest AS protected_manifest,private.suggestion AS protected_suggestion,
        i.input_tokens,i.output_tokens,i.charged_tokens,i.usage_state,i.measured_tokens,i.cached_input_tokens,i.cached_write_tokens,i.reasoning_tokens,
        i.cost_pico_usd::text AS cost_quote,work.current_revision_ref
        FROM linggan_creation_ai_request request JOIN linggan_creation_ai_preview p USING(preview_ref)
        LEFT JOIN linggan_creation_ai_experience_material private USING(preview_ref)
        LEFT JOIN linggan_model_invocation i ON i.invocation_ref=request.invocation_ref
        JOIN linggan_creation_work work ON work.work_ref=p.work_ref WHERE request.request_ref=$1")
        .bind(id).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
    let public: Value = r.get("input_manifest");
    let protected = public.get("experienceProtected").and_then(Value::as_bool) == Some(true);
    let manifest = if protected {
        r.get::<Option<Value>, _>("protected_manifest")
    } else {
        Some(public)
    };
    let readable = if let Some(value) = manifest {
        let input: PreviewRequest = serde_json::from_value(value).map_err(|_| AiError::Invalid)?;
        qualify_frozen_inputs(&mut tx, &input).await?
    } else {
        false
    };
    tx.commit().await?;
    let suggestion = if readable {
        if protected {
            r.get::<Option<Value>, _>("protected_suggestion")
        } else {
            r.get::<Option<Value>, _>("suggestion")
        }
    } else {
        None
    };
    let mut response = json!({"requestId":id,"workRef":r.get::<Uuid,_>("work_ref"),"invocationRef":r.get::<Option<Uuid>,_>("invocation_ref"),
        "batchRef":r.get::<Option<Uuid>,_>("batch_ref"),"taskConfigRef":r.get::<Option<Uuid>,_>("task_config_ref"),"state":r.get::<String,_>("state"),
        "inputRevision":r.get::<Uuid,_>("input_revision"),"action":r.get::<String,_>("action"),"methodVersion":r.get::<String,_>("method_version"),
        "suggestion":suggestion,"sourceQualification":if readable {"readable"} else {"unavailable"},
        "failureCode":r.get::<Option<String>,_>("failure_code"),"staleBase":r.get::<Uuid,_>("input_revision")!=r.get::<Uuid,_>("current_revision_ref"),
        "usage":{"state":r.get::<Option<String>,_>("usage_state").unwrap_or_else(||"not_reserved".into()),
            "measuredTokens":r.get::<Option<i64>,_>("measured_tokens"),"cachedInputTokens":r.get::<Option<i64>,_>("cached_input_tokens"),
            "cachedWriteTokens":r.get::<Option<i64>,_>("cached_write_tokens"),"reasoningTokens":r.get::<Option<i64>,_>("reasoning_tokens"),
            "inputTokens":r.get::<Option<i64>,_>("input_tokens"),"outputTokens":r.get::<Option<i64>,_>("output_tokens"),
            "chargedTokens":r.get::<Option<i64>,_>("charged_tokens"),"costUsd":null,"costPicoUsd":r.get::<Option<String>,_>("cost_quote")}});
    if r.get::<String, _>("action") == "title_ideas" {
        let warnings = suggestion.as_ref().and_then(|value| serde_json::from_value::<super::title::TitleSuggestion>(value.clone()).ok()).map(|titles| json!({
            "cover": titles.cover_titles.iter().map(|title| crate::creation_assets::title_has_cure_promise(title)).collect::<Vec<_>>(),
            "note": titles.note_titles.iter().map(|title| crate::creation_assets::title_has_cure_promise(title)).collect::<Vec<_>>()
        }));
        response["titleWarnings"] = json!(warnings);
    }
    Ok(response)
}
