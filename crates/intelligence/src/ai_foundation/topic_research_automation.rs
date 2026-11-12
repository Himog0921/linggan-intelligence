//! Domain opt-in and bounded production into the existing topic execution ledger.
use super::store::{Command, FoundationError};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
mod comments;
mod definition_repair;
pub use comments::{Retry as RetryCommentAssignment, retry as retry_comment_assignment};
mod producer;
pub use producer::tick;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub domain_ref: Uuid,
    pub enabled: bool,
    pub revision: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_ready: Option<bool>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_revision: i64,
    pub enabled: bool,
}
pub async fn read(db: &Database, domain: Uuid) -> Result<Setting, FoundationError> {
    let row=sqlx::query("SELECT d.domain_ref,COALESCE(a.enabled,false) AS enabled,COALESCE(a.revision,0) AS revision FROM observation_domain d LEFT JOIN linggan_topic_research_automation a USING(domain_ref) WHERE d.domain_ref=$1")
        .bind(domain).fetch_optional(db.pool()).await?.ok_or(FoundationError::NotFound)?;
    let mut tx = db.pool().begin().await?;
    let mut ready = true;
    for task in super::topic_research::TASKS {
        let config: Uuid = sqlx::query_scalar(
            "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key=$1",
        )
        .bind(task.as_str())
        .fetch_one(&mut *tx)
        .await?;
        match super::admission::check_queue_configuration(&mut tx, task, config, false).await {
            Ok(()) => {}
            Err(FoundationError::AdmissionDenied(_)) => ready = false,
            Err(error) => return Err(error),
        }
    }
    tx.commit().await?;
    Ok(Setting {
        configuration_ready: Some(ready),
        domain_ref: domain,
        enabled: row.get("enabled"),
        revision: row.get("revision"),
    })
}
pub async fn change(db: &Database, request: &Change) -> Result<Setting, FoundationError> {
    if request.command_ref.is_nil() || request.domain_ref.is_nil() || request.expected_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "topic_research_automation",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(request.domain_ref)
            .fetch_one(&mut *command.tx)
            .await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    sqlx::query("INSERT INTO linggan_topic_research_automation(domain_ref) VALUES($1) ON CONFLICT DO NOTHING").bind(request.domain_ref).execute(&mut *command.tx).await?;
    let row = sqlx::query(
        "SELECT revision FROM linggan_topic_research_automation WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(request.domain_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    if row.get::<i64, _>("revision") != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    let revision:i64=sqlx::query_scalar("UPDATE linggan_topic_research_automation SET enabled=$2,revision=revision+1,updated_at=scope_001_now() WHERE domain_ref=$1 RETURNING revision")
        .bind(request.domain_ref).bind(request.enabled).fetch_one(&mut *command.tx).await?;
    command
        .finish(Setting {
            configuration_ready: None,
            domain_ref: request.domain_ref,
            enabled: request.enabled,
            revision,
        })
        .await
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
pub(crate) mod definition_tests;
