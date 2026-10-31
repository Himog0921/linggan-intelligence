//! Reuses the writing domain's source and output contracts; trials never mutate a normal request.
use super::{
    AiError, PreviewRequest, inputs_in, qualify_frozen_inputs, sources, validate_suggestion,
};
use crate::ai_foundation::TaskKey;
use crate::ai_foundation::transport_store::{FrozenModel, load_frozen_in};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) async fn system_in(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
    task: TaskKey,
) -> Result<String, AiError> {
    let (_, mut system, instruction, invocation) =
        crate::ai_foundation::prompt::frozen_in(tx, config, task).await?;
    system.push_str("\n\n");
    system.push_str(&instruction);
    if let Some(invocation) = invocation {
        system.push_str("\n\n");
        system.push_str(&invocation);
    }
    Ok(system)
}
pub(crate) fn preview_hash(method: &str, config: Uuid, body_hash: &str, system: &str) -> String {
    sources::text_hash(&json!([method, config, body_hash, system]).to_string())
}
pub(crate) fn protected_preview_hash(
    method: &str,
    config: Uuid,
    body_hash: &str,
    system: &str,
    salt: Option<Uuid>,
) -> String {
    let base = preview_hash(method, config, body_hash, system);
    match salt {
        Some(salt) => sources::text_hash(&json!([base, salt]).to_string()),
        None => base,
    }
}
pub(crate) async fn model_in(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
    task: TaskKey,
) -> Result<FrozenModel, AiError> {
    let method = if task == TaskKey::CreationTitle {
        super::TITLE_METHOD
    } else {
        super::METHOD
    };
    let reference:Option<Uuid>=sqlx::query_scalar("SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1 AND task_key=$2 AND execution_contract=$3")
        .bind(config).bind(task.as_str()).bind(method).fetch_optional(&mut **tx).await?.flatten();
    load_frozen_in(&mut **tx, reference.ok_or(AiError::Model)?)
        .await
        .map_err(AiError::from)
}
pub(crate) struct Input {
    pub manifest: PreviewRequest,
    pub payload: Value,
    pub hash: String,
    pub preview_config: Uuid,
    pub experience_ref: Option<Uuid>,
}
pub(crate) async fn load_in(
    tx: &mut Transaction<'_, Postgres>,
    preview: Uuid,
) -> Result<Input, AiError> {
    let row=sqlx::query("SELECT p.input_manifest,p.request_hash,p.task_config_ref,p.method_version,m.manifest AS protected_manifest,m.hash_salt,m.experience_ref FROM linggan_creation_ai_preview p LEFT JOIN linggan_creation_ai_experience_material m USING(preview_ref) WHERE p.preview_ref=$1")
        .bind(preview).fetch_optional(&mut **tx).await?.ok_or(AiError::Stale)?;
    let public: Value = row.get("input_manifest");
    let protected = public.get("experienceProtected").and_then(Value::as_bool) == Some(true);
    let manifest_value: Value = if protected {
        row.get::<Option<Value>, _>("protected_manifest")
            .ok_or(AiError::Source)?
    } else {
        public
    };
    let manifest: PreviewRequest =
        serde_json::from_value(manifest_value).map_err(|_| AiError::Invalid)?;
    let experience_ref: Option<Uuid> = row.get("experience_ref");
    let salt: Option<Uuid> = row.get("hash_salt");
    if protected && (experience_ref.is_none() || salt.is_none()) {
        return Err(AiError::Source);
    }
    let config: Option<Uuid> = row.get("task_config_ref");
    let config = config.ok_or(AiError::Model)?;
    if row.get::<String, _>("method_version") != manifest.action.method() {
        return Err(AiError::Stale);
    }
    let (_, payload, body_hash) = inputs_in(tx, &manifest).await?;
    let system = system_in(tx, config, manifest.action.task()).await?;
    let hash: String = row.get("request_hash");
    if protected_preview_hash(manifest.action.method(), config, &body_hash, &system, salt) != hash {
        return Err(AiError::Stale);
    }
    Ok(Input {
        manifest,
        payload,
        hash,
        preview_config: config,
        experience_ref,
    })
}
impl Input {
    pub(crate) async fn readable_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<bool, AiError> {
        qualify_frozen_inputs(tx, &self.manifest).await
    }
    pub(crate) fn validate(&self, text: &str) -> Result<Value, AiError> {
        if self.manifest.action == super::WritingAction::TitleIdeas {
            return serde_json::to_value(super::title::validate(text)?)
                .map_err(|_| AiError::Invalid);
        }
        let pages = self.payload["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["pageId"].as_str()?.parse::<Uuid>().ok())
            .collect::<Vec<_>>();
        let output = validate_suggestion(
            &self.manifest.action,
            text,
            &self.manifest.paragraph_ids,
            &pages,
        )?;
        if self.experience_ref.is_some()
            && (output.title.is_some()
                || !output.pages.is_empty()
                || output.paragraphs.len() != 1
                || output.paragraphs[0].paragraph_id
                    != self.manifest.paragraph_ids.first().copied())
        {
            return Err(AiError::Invalid);
        }
        serde_json::to_value(output).map_err(|_| AiError::Invalid)
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
    // Trial batch outputs are immutable and cannot yet erase experience-derived text.
    if manifest.get("experienceProtected").and_then(Value::as_bool) == Some(true) {
        return Ok(false);
    }
    let manifest: PreviewRequest =
        serde_json::from_value(manifest).map_err(|_| AiError::Invalid)?;
    qualify_frozen_inputs(tx, &manifest).await
}
