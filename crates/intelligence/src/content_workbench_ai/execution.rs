//! Reuses the writing domain's source and output contracts; trials never mutate a normal request.
use super::{
    AiError, METHOD, PreviewRequest, SYSTEM, WritingSuggestion, inputs_in, qualify_frozen_inputs,
    sources, validate_suggestion,
};
use crate::ai_foundation::transport_store::{FrozenModel, load_frozen_in};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) async fn system_in(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
) -> Result<String, AiError> {
    let invocation:Option<String>=sqlx::query_scalar("SELECT v.invocation FROM linggan_ai_task_config c LEFT JOIN linggan_skill_version v ON v.version_ref=c.skill_version_ref WHERE c.task_config_ref=$1 AND c.task_key='writing_legacy' AND c.execution_contract=$2")
        .bind(config).bind(METHOD).fetch_one(&mut **tx).await?;
    Ok(match invocation {
        Some(invocation) => format!("{SYSTEM}\n\n{invocation}"),
        None => SYSTEM.into(),
    })
}
pub(crate) fn preview_hash(config: Uuid, body_hash: &str, system: &str) -> String {
    sources::text_hash(&json!([METHOD, config, body_hash, system]).to_string())
}
pub(crate) async fn model_in(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
) -> Result<FrozenModel, AiError> {
    let reference:Option<Uuid>=sqlx::query_scalar("SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1 AND task_key='writing_legacy' AND execution_contract=$2")
        .bind(config).bind(METHOD).fetch_optional(&mut **tx).await?.flatten();
    load_frozen_in(&mut **tx, reference.ok_or(AiError::Model)?)
        .await
        .map_err(AiError::from)
}
pub(crate) struct Input {
    pub manifest: PreviewRequest,
    pub payload: Value,
    pub hash: String,
    pub preview_config: Uuid,
}
pub(crate) async fn load_in(
    tx: &mut Transaction<'_, Postgres>,
    preview: Uuid,
) -> Result<Input, AiError> {
    let row=sqlx::query("SELECT input_manifest,request_hash,task_config_ref FROM linggan_creation_ai_preview WHERE preview_ref=$1 AND method_version=$2")
        .bind(preview).bind(METHOD).fetch_optional(&mut **tx).await?.ok_or(AiError::Stale)?;
    let manifest: PreviewRequest =
        serde_json::from_value(row.get("input_manifest")).map_err(|_| AiError::Invalid)?;
    let config: Option<Uuid> = row.get("task_config_ref");
    let config = config.ok_or(AiError::Model)?;
    let (_, payload, body_hash) = inputs_in(tx, &manifest).await?;
    let system = system_in(tx, config).await?;
    let hash: String = row.get("request_hash");
    if preview_hash(config, &body_hash, &system) != hash {
        return Err(AiError::Stale);
    }
    Ok(Input {
        manifest,
        payload,
        hash,
        preview_config: config,
    })
}
impl Input {
    pub(crate) async fn readable_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<bool, AiError> {
        qualify_frozen_inputs(tx, &self.manifest).await
    }
    pub(crate) fn validate(&self, text: &str) -> Result<WritingSuggestion, AiError> {
        let pages = self.payload["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["pageId"].as_str()?.parse::<Uuid>().ok())
            .collect::<Vec<_>>();
        validate_suggestion(
            &self.manifest.action,
            text,
            &self.manifest.paragraph_ids,
            &pages,
        )
    }
}
pub(crate) async fn readable_preview_in(
    tx: &mut Transaction<'_, Postgres>,
    preview: Uuid,
) -> Result<bool, AiError> {
    let manifest: Option<Value> = sqlx::query_scalar(
        "SELECT input_manifest FROM linggan_creation_ai_preview WHERE preview_ref=$1",
    )
    .bind(preview)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(manifest) = manifest else {
        return Ok(false);
    };
    let manifest: PreviewRequest =
        serde_json::from_value(manifest).map_err(|_| AiError::Invalid)?;
    qualify_frozen_inputs(tx, &manifest).await
}
