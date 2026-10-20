//! Small transaction boundary shared by configuration commands. No provider or secret access.
use super::RuleError;
use linggan_storage_postgres::Database;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FoundationError {
    #[error("ai_invalid_command")]
    Invalid,
    #[error("ai_revision_conflict")]
    Conflict,
    #[error("ai_not_found")]
    NotFound,
    #[error("ai_task_not_connected")]
    NotConnected,
    #[error("ai_configuration_incomplete")]
    ConfigurationIncomplete,
    #[error("ai_skill_trial_required")]
    SkillTrialRequired,
    #[error("ai_admission_denied")]
    AdmissionDenied(Vec<String>),
    #[error("ai_skill_loader_unavailable")]
    LoaderUnavailable,
    #[error("ai_system_cannot_resume")]
    SystemCannotResume,
    #[error("ai_rule_rejected: {0}")]
    Rule(#[from] RuleError),
    #[error("ai_database_unavailable")]
    Database(#[from] sqlx::Error),
}
pub(crate) fn valid_reason(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}
pub(crate) struct Command<'a> {
    pub tx: Transaction<'a, Postgres>,
    command_ref: Uuid,
    operation: &'static str,
    payload: Value,
    receipt: Option<Value>,
}
impl<'a> Command<'a> {
    pub async fn begin<T: Serialize>(
        db: &'a Database,
        command_ref: Uuid,
        operation: &'static str,
        payload: &T,
    ) -> Result<Self, FoundationError> {
        let payload = serde_json::to_value(payload).map_err(|_| FoundationError::Invalid)?;
        if !payload.is_object() || payload.to_string().len() > 65536 {
            return Err(FoundationError::Invalid);
        }
        let mut tx = db.pool().begin().await?;
        // Serializes duplicate command IDs even when they name different scope rows.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 410132))")
            .bind(command_ref.to_string())
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query(
            "SELECT operation,payload,receipt FROM linggan_ai_command WHERE command_ref=$1",
        )
        .bind(command_ref)
        .fetch_optional(&mut *tx)
        .await?;
        let receipt = if let Some(row) = existing {
            if row.get::<String, _>("operation") != operation
                || row.get::<Value, _>("payload") != payload
            {
                return Err(FoundationError::Conflict);
            }
            Some(row.get("receipt"))
        } else {
            None
        };
        Ok(Self {
            tx,
            command_ref,
            operation,
            payload,
            receipt,
        })
    }
    pub fn replay<T: DeserializeOwned>(&self) -> Result<Option<T>, FoundationError> {
        self.receipt
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| FoundationError::Invalid)
    }
    pub async fn finish<T: Serialize>(mut self, receipt: T) -> Result<T, FoundationError> {
        let value = serde_json::to_value(&receipt).map_err(|_| FoundationError::Invalid)?;
        sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,$2,$3,$4)")
            .bind(self.command_ref).bind(self.operation).bind(self.payload).bind(value)
            .execute(&mut *self.tx).await?;
        self.tx.commit().await?;
        Ok(receipt)
    }
}
