//! Compare-and-set of the existing default pointer; never mutates a method or a Run.
use super::store::{self, StudyPolicyStoreError};
use super::StudyPolicyContractError;
use crate::comment_study_selection::study_domain_lock_key;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateStudyPolicyCommand {
    #[serde(deserialize_with = "required_expected")]
    pub expected_active_policy_ref: Option<Uuid>,
}

fn required_expected<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Uuid>, D::Error> {
    Option::<Uuid>::deserialize(d)
}

impl ActivateStudyPolicyCommand {
    pub fn validate(&self) -> Result<(), StudyPolicyContractError> {
        if self.expected_active_policy_ref.is_some_and(|id| id.is_nil()) {
            return Err(StudyPolicyContractError::InvalidRequest);
        }
        Ok(())
    }
}

/// The caller supplies the supported domain, not a client-supplied origin or global fallback.
/// An absent singleton is CASed with INSERT; concurrent creation cannot overwrite a default.
pub async fn activate_study_policy(
    database: &Database,
    domain: Uuid,
    reference: Uuid,
    command: ActivateStudyPolicyCommand,
) -> Result<Value, StudyPolicyStoreError> {
    store::validate_domain(domain)?;
    command.validate()?;
    if reference.is_nil() {
        return Err(StudyPolicyContractError::InvalidRequest.into());
    }
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL lock_timeout='3s'").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    let key = study_domain_lock_key(domain)
        .map_err(|_| StudyPolicyContractError::UnsupportedDomain)?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(key).execute(&mut *tx).await?;
    store::ensure_schema(&mut tx, true).await?;
    let ready: bool = sqlx::query_scalar(
        "SELECT count(*)=3 FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='linggan_comment_study_active_policy' \
         AND (column_name,udt_name) IN (('singleton','bool'),('policy_ref','uuid'),('updated_at','timestamptz'))",
    ).fetch_one(&mut *tx).await?;
    if !ready { return Err(StudyPolicyStoreError::SchemaUnavailable); }
    let policy = store::load_policy(&mut tx, domain, reference).await?;
    if policy["recordingState"] != "recorded" { return Err(StudyPolicyStoreError::Unrecorded); }
    let config: Uuid = serde_json::from_value(policy["methodManifest"]["modelConfigRef"].clone())
        .map_err(|_| StudyPolicyStoreError::ModelUnavailable)?;
    store::model_snapshot(&mut tx, config, true).await?;
    // Reverify against the model rows now held FOR SHARE. The initial read may precede a model
    // edit committed while we waited for those row locks; it is not sufficient on its own.
    store::load_policy(&mut tx, domain, reference).await?;
    let affected = match command.expected_active_policy_ref {
        Some(expected) => sqlx::query(
            "UPDATE linggan_comment_study_active_policy SET policy_ref=$1,updated_at=scope_001_now() \
             WHERE singleton AND policy_ref=$2",
        ).bind(reference).bind(expected).execute(&mut *tx).await?.rows_affected(),
        None => sqlx::query(
            "INSERT INTO linggan_comment_study_active_policy(singleton,policy_ref) VALUES(true,$1) \
             ON CONFLICT(singleton) DO NOTHING",
        ).bind(reference).execute(&mut *tx).await?.rows_affected(),
    };
    if affected != 1 { return Err(StudyPolicyStoreError::ActiveConflict); }
    tx.commit().await?;
    Ok(json!({"contract":"comment-study.read.v2","domainRef":domain,
        "policyRef":reference,"isActive":true}))
}
