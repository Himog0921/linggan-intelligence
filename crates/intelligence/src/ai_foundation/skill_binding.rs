//! Human activation is separate from import, candidate saves, trial execution and task switches.
use super::{
    TaskKey,
    store::{Command, FoundationError, valid_reason},
    task_config::{advance_setting, lock_setting},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateSkill {
    pub command_ref: Uuid,
    pub task_key: TaskKey,
    pub expected_task_revision: i64,
    pub expected_binding_revision: i64,
    pub candidate_config_ref: Uuid,
    pub trial_comparison_ref: Option<Uuid>,
    pub first_version_exception: bool,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationReceipt {
    pub version_ref: Uuid,
    pub task_config_ref: Uuid,
    pub task_revision: i64,
    pub binding_revision: i64,
    pub approval_source: String,
}
pub async fn activate_skill(
    db: &Database,
    request: &ActivateSkill,
) -> Result<ActivationReceipt, FoundationError> {
    if !valid_reason(&request.reason, 300)
        || request.expected_task_revision < 0
        || request.expected_binding_revision < 0
        || request.expected_task_revision == i64::MAX
        || request.expected_binding_revision == i64::MAX
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "skill_activate", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_setting(
        &mut command.tx,
        request.task_key,
        request.expected_task_revision,
    )
    .await?;
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_skill_binding WHERE task_key=$1 FOR UPDATE",
    )
    .bind(request.task_key.as_str())
    .fetch_one(&mut *command.tx)
    .await?;
    if revision != request.expected_binding_revision {
        return Err(FoundationError::Conflict);
    }
    let candidate=sqlx::query("SELECT v.version_ref,v.skill_ref,v.version_no,t.lifecycle,t.skill_allowed,
        (c.model_config_ref IS NULL OR m.capability_ref IS NOT NULL) AS model_config_valid
        FROM linggan_ai_task_config c JOIN linggan_skill_version v ON v.version_ref=c.skill_version_ref
        JOIN linggan_ai_task t USING(task_key) LEFT JOIN linggan_model_config m ON m.config_ref=c.model_config_ref
        WHERE c.task_config_ref=$1 AND c.task_key=$2")
        .bind(request.candidate_config_ref).bind(request.task_key.as_str()).fetch_optional(&mut *command.tx).await?.ok_or(FoundationError::NotFound)?;
    if matches!(
        candidate.get::<String, _>("lifecycle").as_str(),
        "frozen" | "retired" | "legacy"
    ) {
        return Err(FoundationError::NotConnected);
    }
    if !candidate.get::<bool, _>("skill_allowed") {
        return Err(FoundationError::Invalid);
    }
    if !candidate.get::<bool, _>("model_config_valid") {
        return Err(FoundationError::ConfigurationIncomplete);
    }
    let version: Uuid = candidate.get("version_ref");
    let source = if let Some(comparison) = request.trial_comparison_ref {
        let qualifies: bool = sqlx::query_scalar("SELECT linggan_ai_trial_qualifies($1,$2,$3,$4)")
            .bind(comparison)
            .bind(request.task_key.as_str())
            .bind(version)
            .bind(request.candidate_config_ref)
            .fetch_one(&mut *command.tx)
            .await?;
        if !qualifies {
            return Err(FoundationError::SkillTrialRequired);
        }
        "trial"
    } else {
        let prior:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_skill_binding_event WHERE task_key=$1 AND version_ref=$2)")
            .bind(request.task_key.as_str()).bind(version).fetch_one(&mut *command.tx).await?;
        if prior {
            "prior_approval"
        } else {
            let used_skill:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_skill_binding_event e JOIN linggan_skill_version v ON v.version_ref=e.version_ref WHERE e.task_key=$1 AND v.skill_ref=$2)")
                .bind(request.task_key.as_str()).bind(candidate.get::<Uuid,_>("skill_ref")).fetch_one(&mut *command.tx).await?;
            if !request.first_version_exception
                || candidate.get::<i32, _>("version_no") != 1
                || used_skill
            {
                return Err(FoundationError::SkillTrialRequired);
            }
            "first_version"
        }
    };
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,command_ref) SELECT $1,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,'skill_activation',$1 FROM linggan_ai_task_config WHERE task_config_ref=$2")
        .bind(request.command_ref).bind(request.candidate_config_ref).execute(&mut *command.tx).await?;
    sqlx::query("UPDATE linggan_skill_binding SET current_version_ref=$2,revision=revision+1,activation_config_ref=$3,
        approval_source=$4,trial_comparison_ref=$5,reason=$6,command_ref=$3,updated_at=scope_001_now() WHERE task_key=$1")
        .bind(request.task_key.as_str()).bind(version).bind(request.command_ref).bind(source)
        .bind(request.trial_comparison_ref).bind(&request.reason).execute(&mut *command.tx).await?;
    let task_revision =
        advance_setting(&mut command.tx, request.task_key, request.command_ref).await?;
    command
        .finish(ActivationReceipt {
            version_ref: version,
            task_config_ref: request.command_ref,
            task_revision,
            binding_revision: revision + 1,
            approval_source: source.into(),
        })
        .await
}
