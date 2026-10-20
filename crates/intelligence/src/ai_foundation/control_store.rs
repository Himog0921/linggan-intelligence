//! Versioned switch intent, never a substitute for per-dispatch admission.
use super::{
    TaskKey,
    store::{Command, FoundationError, valid_reason},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Human,
    System,
}
impl Actor {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::System => "system",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "task",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ControlScope {
    Global,
    Executor,
    Task(TaskKey),
}
impl ControlScope {
    pub fn key(&self) -> String {
        match self {
            Self::Global => "global".into(),
            Self::Executor => "executor".into(),
            Self::Task(task) => format!("task:{}", task.as_str()),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeControl {
    pub command_ref: Uuid,
    pub scope: ControlScope,
    pub expected_revision: i64,
    pub enabled: bool,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlReceipt {
    pub scope_key: String,
    pub revision: i64,
    pub enabled: bool,
    pub command_ref: Uuid,
}
/// Actor is supplied by the trusted caller, not accepted in the HTTP command body.
pub async fn change_control(
    db: &Database,
    request: &ChangeControl,
    actor: Actor,
) -> Result<ControlReceipt, FoundationError> {
    if !valid_reason(&request.reason, 300)
        || request.expected_revision < 0
        || request.expected_revision == i64::MAX
    {
        return Err(FoundationError::Invalid);
    }
    if actor == Actor::System && request.enabled {
        return Err(FoundationError::SystemCannotResume);
    }
    let payload = serde_json::json!({"request":request,"actor":actor});
    let mut command = Command::begin(db, request.command_ref, "control", &payload).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let key = request.scope.key();
    let row = sqlx::query("SELECT c.revision,t.lifecycle FROM linggan_ai_control c LEFT JOIN linggan_ai_task t USING(task_key) WHERE scope_key=$1 FOR UPDATE OF c")
        .bind(&key).fetch_optional(&mut *command.tx).await?.ok_or(FoundationError::NotFound)?;
    if row.get::<i64, _>("revision") != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    if request.enabled
        && row
            .get::<Option<String>, _>("lifecycle")
            .is_some_and(|l| l != "active")
    {
        return Err(FoundationError::NotConnected);
    }
    let revision = request.expected_revision + 1;
    sqlx::query("UPDATE linggan_ai_control SET enabled=$2,revision=$3,actor_kind=$4,reason=$5,command_ref=$6,updated_at=scope_001_now() WHERE scope_key=$1")
        .bind(&key).bind(request.enabled).bind(revision).bind(actor.as_str()).bind(&request.reason)
        .bind(request.command_ref).execute(&mut *command.tx).await?;
    command
        .finish(ControlReceipt {
            scope_key: key,
            revision,
            enabled: request.enabled,
            command_ref: request.command_ref,
        })
        .await
}
