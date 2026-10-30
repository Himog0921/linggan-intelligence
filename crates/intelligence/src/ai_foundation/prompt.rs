//! Immutable task prompts, saved independently of skill packages and frozen in task configs.
use super::{
    TaskKey,
    store::{Command, FoundationError},
    task_config::{advance_setting, lock_setting},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavePrompt {
    pub command_ref: Uuid,
    pub task_key: TaskKey,
    pub expected_task_revision: i64,
    pub system_text: String,
    pub instruction_text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivatePrompt {
    pub command_ref: Uuid,
    pub task_key: TaskKey,
    pub expected_task_revision: i64,
    pub version_ref: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptReceipt {
    pub version_ref: Uuid,
    pub version_no: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptActivation {
    pub version_ref: Uuid,
    pub task_config_ref: Uuid,
    pub task_revision: i64,
}
fn connected(task: TaskKey) -> bool {
    matches!(
        task,
        TaskKey::OcrRecognition
            | TaskKey::WritingLegacy
            | TaskKey::CreationTitle
            | TaskKey::WorkComparison
            | TaskKey::CommentStudy
            | TaskKey::CommentProblemResolution
            | TaskKey::CommentProblemPairReview
            | TaskKey::TopicResearch
            | TaskKey::TopicResearchExtractReview
            | TaskKey::TopicResearchResolve
            | TaskKey::TopicResearchResolveReview
            | TaskKey::CreatorResearch
            | TaskKey::CreatorAccountResearch
    )
}
pub async fn read(db: &Database, task: TaskKey) -> Result<Value, FoundationError> {
    if !connected(task) {
        return Err(FoundationError::NotConnected);
    }
    sqlx::query_scalar("SELECT jsonb_build_object('taskKey',s.task_key,'revision',s.revision,
      'activeVersionRef',c.prompt_version_ref,'versions',COALESCE((SELECT jsonb_agg(jsonb_build_object(
      'versionRef',p.version_ref,'versionNo',p.version_no,'systemText',p.system_text,
      'instructionText',p.instruction_text,'createdAt',p.created_at) ORDER BY p.version_no DESC)
      FROM linggan_ai_prompt_version p WHERE p.task_key=s.task_key),'[]'::jsonb))
      FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key=$1")
        .bind(task.as_str()).fetch_optional(db.pool()).await?.ok_or(FoundationError::NotFound)
}
pub async fn save(db: &Database, request: &SavePrompt) -> Result<PromptReceipt, FoundationError> {
    if !connected(request.task_key) {
        return Err(FoundationError::NotConnected);
    }
    if request.expected_task_revision < 0
        || request.system_text.trim().is_empty()
        || request.instruction_text.trim().is_empty()
        || request.system_text.len() > 24000
        || request.instruction_text.len() > 24000
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "prompt_save", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_setting(
        &mut command.tx,
        request.task_key,
        request.expected_task_revision,
    )
    .await?;
    let number: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(version_no),0)+1 FROM linggan_ai_prompt_version WHERE task_key=$1",
    )
    .bind(request.task_key.as_str())
    .fetch_one(&mut *command.tx)
    .await?;
    sqlx::query("INSERT INTO linggan_ai_prompt_version(version_ref,task_key,version_no,system_text,instruction_text) VALUES($1,$2,$3,$4,$5)")
        .bind(request.command_ref).bind(request.task_key.as_str()).bind(number).bind(&request.system_text).bind(&request.instruction_text)
        .execute(&mut *command.tx).await?;
    command
        .finish(PromptReceipt {
            version_ref: request.command_ref,
            version_no: number,
        })
        .await
}
pub async fn activate(
    db: &Database,
    request: &ActivatePrompt,
) -> Result<PromptActivation, FoundationError> {
    if !connected(request.task_key) {
        return Err(FoundationError::NotConnected);
    }
    if request.expected_task_revision < 0 || request.expected_task_revision == i64::MAX {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "prompt_activate", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let current = lock_setting(
        &mut command.tx,
        request.task_key,
        request.expected_task_revision,
    )
    .await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_prompt_version WHERE version_ref=$1 AND task_key=$2)")
        .bind(request.version_ref).bind(request.task_key.as_str()).fetch_one(&mut *command.tx).await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,command_ref,prompt_version_ref)
      SELECT $1,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,'prompt_activation',$1,$3 FROM linggan_ai_task_config WHERE task_config_ref=$2")
        .bind(request.command_ref).bind(current).bind(request.version_ref).execute(&mut *command.tx).await?;
    let revision = advance_setting(&mut command.tx, request.task_key, request.command_ref).await?;
    command
        .finish(PromptActivation {
            version_ref: request.version_ref,
            task_config_ref: request.command_ref,
            task_revision: revision,
        })
        .await
}
pub(crate) async fn frozen_in(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
    task: TaskKey,
) -> Result<(Uuid, String, String, Option<String>), FoundationError> {
    let row=sqlx::query("SELECT p.version_ref,p.system_text,p.instruction_text,s.invocation FROM linggan_ai_task_config c
      JOIN linggan_ai_prompt_version p ON p.version_ref=c.prompt_version_ref AND p.task_key=c.task_key
      LEFT JOIN linggan_skill_version s ON s.version_ref=c.skill_version_ref WHERE c.task_config_ref=$1 AND c.task_key=$2")
        .bind(config).bind(task.as_str()).fetch_optional(&mut **tx).await?.ok_or(FoundationError::ConfigurationIncomplete)?;
    Ok((
        row.get("version_ref"),
        row.get("system_text"),
        row.get("instruction_text"),
        row.get("invocation"),
    ))
}
