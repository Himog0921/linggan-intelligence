//! Configuration intent is immutable. Saving never enables, queues, probes or dispatches.
use super::{
    TaskKey,
    capability::ModelLimits,
    control::BudgetPolicy,
    store::{Command, FoundationError},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialColumn {
    Title,
    CoverHeadline,
    Body,
    Tags,
    Ocr,
    Transcript,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelSelection {
    pub model_ref: Uuid,
    pub capability_ref: Uuid,
    pub limits: ModelLimits,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveTaskConfig {
    pub command_ref: Uuid,
    pub task_key: TaskKey,
    pub expected_revision: i64,
    pub model: Option<ModelSelection>,
    pub skill_version_ref: Option<Uuid>,
    pub material_columns: BTreeSet<MaterialColumn>,
    pub budget: BudgetPolicy,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskConfigReceipt {
    pub task_key: TaskKey,
    pub task_config_ref: Uuid,
    pub model_config_ref: Option<Uuid>,
    pub revision: i64,
    pub applied: bool,
}
pub(crate) async fn lock_setting(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    expected: i64,
) -> Result<Uuid, FoundationError> {
    let row=sqlx::query("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key=$1 FOR UPDATE")
        .bind(task.as_str()).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    if row.get::<i64, _>("revision") != expected {
        return Err(FoundationError::Conflict);
    }
    Ok(row.get("current_config_ref"))
}
pub(crate) async fn advance_setting(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
) -> Result<i64, FoundationError> {
    Ok(sqlx::query_scalar("UPDATE linggan_ai_task_setting SET current_config_ref=$2,revision=revision+1,updated_at=scope_001_now() WHERE task_key=$1 RETURNING revision")
        .bind(task.as_str()).bind(config).fetch_one(&mut **tx).await?)
}
async fn save_model(
    tx: &mut Transaction<'_, Postgres>,
    model: &ModelSelection,
) -> Result<Uuid, FoundationError> {
    let row=sqlx::query("SELECT cap.context_tokens,cap.max_output_tokens,cap.max_timeout_seconds,cap.thinking_levels,cap.compat,cap.thinking_level_map,cap.output_mode,cap.strict_schema,endpoint.api,
      cap.endpoint_ref,i.model_id,c.connection_ref,v.version_ref,p.endpoint_ref AS current_endpoint
      FROM linggan_model_capability cap JOIN linggan_ai_endpoint endpoint ON endpoint.endpoint_ref=cap.endpoint_ref JOIN linggan_ai_model_identity i ON i.identity_ref=cap.identity_ref
      JOIN linggan_model_connection c ON c.connection_ref=i.connection_ref
      JOIN linggan_model_entry selected ON selected.model_ref=$2 AND selected.model_id=i.model_id
      JOIN linggan_model_connection_version selected_version ON selected_version.version_ref=selected.connection_version_ref AND selected_version.connection_ref=c.connection_ref
      JOIN LATERAL(SELECT version_ref FROM linggan_model_connection_version WHERE connection_ref=c.connection_ref ORDER BY revision DESC LIMIT 1)v ON true
      JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
      WHERE cap.capability_ref=$1 FOR UPDATE OF c")
        .bind(model.capability_ref).bind(model.model_ref).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    if row.get::<Uuid, _>("endpoint_ref") != row.get::<Uuid, _>("current_endpoint") {
        return Err(FoundationError::Conflict);
    }
    super::transport_store::execution_from_row(&row, model.capability_ref, &model.limits)?;
    // This is a carried model selection, not a new observation from the provider's catalog.
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,$3,'manual') ON CONFLICT(connection_version_ref,model_id) DO NOTHING")
        .bind(Uuid::new_v4()).bind(row.get::<Uuid,_>("version_ref")).bind(row.get::<String,_>("model_id"))
        .execute(&mut **tx).await?;
    let current_model: Uuid = sqlx::query_scalar(
        "SELECT model_ref FROM linggan_model_entry WHERE connection_version_ref=$1 AND model_id=$2",
    )
    .bind(row.get::<Uuid, _>("version_ref"))
    .bind(row.get::<String, _>("model_id"))
    .fetch_one(&mut **tx)
    .await?;
    let config = Uuid::new_v4();
    let existing:Option<Uuid>=sqlx::query_scalar("SELECT config_ref FROM linggan_model_config WHERE model_ref=$1 AND capability_ref=$2
        AND input_token_limit=$3 AND output_token_limit=$4 AND timeout_seconds=$5 AND max_attempts=$6 AND thinking_level=$7
        ORDER BY created_at,config_ref LIMIT 1")
        .bind(current_model).bind(model.capability_ref).bind(model.limits.input_tokens as i32).bind(model.limits.output_tokens as i32)
        .bind(model.limits.timeout_seconds as i32).bind(model.limits.attempts as i32).bind(&model.limits.thinking_level)
        .fetch_optional(&mut **tx).await?;
    if let Some(existing) = existing {
        return Ok(existing);
    }
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,thinking_level) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(config).bind(current_model).bind(model.capability_ref).bind(model.limits.input_tokens as i32)
        .bind(model.limits.output_tokens as i32).bind(model.limits.timeout_seconds as i32)
        .bind(model.limits.attempts as i32).bind(&model.limits.thinking_level).execute(&mut **tx).await?;
    Ok(config)
}
pub async fn save_task_config(
    db: &Database,
    request: &SaveTaskConfig,
) -> Result<TaskConfigReceipt, FoundationError> {
    if request.expected_revision < 0
        || request.expected_revision == i64::MAX
        || matches!(request.budget,BudgetPolicy::Capped(v) if v>i64::MAX as u64)
        || (request.task_key == TaskKey::ModelProbe && request.model.is_some())
        || (request.task_key == TaskKey::WorkComparison
            && request.model.as_ref().is_some_and(|m| m.limits.attempts != 1))
        || (matches!(
            request.task_key,
            TaskKey::NoteInterpretation | TaskKey::DirectionListDraft | TaskKey::DirectionProposal
        ) && request.material_columns.is_empty())
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "task_config", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_setting(&mut command.tx, request.task_key, request.expected_revision).await?;
    let lifecycle: String =
        sqlx::query_scalar("SELECT lifecycle FROM linggan_ai_task WHERE task_key=$1")
            .bind(request.task_key.as_str())
            .fetch_one(&mut *command.tx)
            .await?;
    if matches!(lifecycle.as_str(), "frozen" | "retired" | "legacy") {
        return Err(FoundationError::NotConnected);
    }
    let skill_allowed: bool =
        sqlx::query_scalar("SELECT skill_allowed FROM linggan_ai_task WHERE task_key=$1")
            .bind(request.task_key.as_str())
            .fetch_one(&mut *command.tx)
            .await?;
    if request.skill_version_ref.is_some() && !skill_allowed {
        return Err(FoundationError::Invalid);
    }
    let model_config_ref = match &request.model {
        Some(model) => Some(save_model(&mut command.tx, model).await?),
        None => None,
    };
    if let Some(skill) = request.skill_version_ref {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_skill_version WHERE version_ref=$1)",
        )
        .bind(skill)
        .fetch_one(&mut *command.tx)
        .await?;
        if !exists {
            return Err(FoundationError::NotFound);
        }
    }
    let (mode, daily) = match request.budget {
        BudgetPolicy::Unconfigured => ("unconfigured", None),
        BudgetPolicy::Unlimited => ("unlimited", None),
        BudgetPolicy::Capped(v) => ("capped", Some(v as i64)),
    };
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,material_columns,budget_mode,daily_token_limit,origin,command_ref,execution_contract) VALUES($1,$2,$3,$4,$5,$6,$7,'human',$1,(SELECT handler_contract FROM linggan_ai_task WHERE task_key=$2))")
        .bind(request.command_ref).bind(request.task_key.as_str()).bind(model_config_ref).bind(request.skill_version_ref)
        .bind(serde_json::json!(request.material_columns)).bind(mode).bind(daily).execute(&mut *command.tx).await?;
    let active_skill: Option<Uuid> = sqlx::query_scalar(
        "SELECT current_version_ref FROM linggan_skill_binding WHERE task_key=$1",
    )
    .bind(request.task_key.as_str())
    .fetch_one(&mut *command.tx)
    .await?;
    let applied = active_skill == request.skill_version_ref;
    let revision = if applied {
        advance_setting(&mut command.tx, request.task_key, request.command_ref).await?
    } else {
        request.expected_revision
    };
    command
        .finish(TaskConfigReceipt {
            task_key: request.task_key,
            task_config_ref: request.command_ref,
            model_config_ref,
            revision,
            applied,
        })
        .await
}
