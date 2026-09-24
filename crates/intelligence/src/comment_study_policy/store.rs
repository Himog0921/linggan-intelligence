//! Transactional policy versions. Saving never activates a policy or creates model work.
use super::{
    CompiledStudyMethod, CreateStudyPolicyCommand, StudyModelIdentity, StudyModelSnapshot,
    StudyPolicyContractError, compile_study_method, verify_study_method,
};
use crate::comment_study_catalog::StudyCatalogError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum StudyPolicyStoreError {
    #[error(transparent)]
    Contract(#[from] StudyPolicyContractError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Catalog(#[from] StudyCatalogError),
    #[error("study_schema_unavailable")]
    SchemaUnavailable,
    #[error("resource_not_found")]
    NotFound,
    #[error("policy_unrecorded")]
    Unrecorded,
    #[error("policy_model_unavailable")]
    ModelUnavailable,
    #[error("policy_model_disabled")]
    ModelDisabled,
}

pub(super) fn validate_domain(domain: Uuid) -> Result<(), StudyPolicyStoreError> {
    if domain.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
        return Err(StudyPolicyContractError::UnsupportedDomain.into());
    }
    Ok(())
}

pub(super) async fn ensure_schema(
    tx: &mut Transaction<'_, Postgres>, writing: bool,
) -> Result<(), StudyPolicyStoreError> {
    let columns: bool = sqlx::query_scalar(
        "SELECT count(*)=4 FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='linggan_comment_study_policy' \
         AND (column_name,udt_name) IN \
         (('method_name','text'),('method_manifest','jsonb'),('method_hash','text'),('parent_policy_ref','uuid'))",
    ).fetch_one(&mut **tx).await?;
    if !columns { return Err(StudyPolicyStoreError::SchemaUnavailable); }
    if writing {
        // Do not accept new versions on a partly upgraded database lacking immutable guards.
        let guards: bool = sqlx::query_scalar(
            "SELECT count(*)=3 FROM pg_trigger t \
             WHERE t.tgrelid='linggan_comment_study_policy'::regclass \
             AND NOT t.tgisinternal AND t.tgenabled IN ('O','A') \
             AND t.tgname IN ('cs_policy_insert_guard','cs_policy_immutable','cs_policy_no_truncate')",
        ).fetch_one(&mut **tx).await?;
        if !guards { return Err(StudyPolicyStoreError::SchemaUnavailable); }
    }
    Ok(())
}

pub(super) async fn model_snapshot(
    tx: &mut Transaction<'_, Postgres>, config: Uuid, writing: bool,
) -> Result<(StudyModelSnapshot, bool), StudyPolicyStoreError> {
    const SQL: &str = "SELECT c.config_ref,c.input_token_limit,c.output_token_limit,c.timeout_seconds, \
        m.model_ref,m.model_id,v.version_ref,n.enabled \
        FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) \
        JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref \
        JOIN linggan_model_connection n USING(connection_ref) WHERE c.config_ref=$1";
    let statement = if writing { format!("{SQL} FOR SHARE OF c,m,v,n") } else { SQL.to_owned() };
    let row = sqlx::query(sqlx::AssertSqlSafe(statement)).bind(config)
        .fetch_optional(&mut **tx).await?.ok_or(StudyPolicyStoreError::ModelUnavailable)?;
    let enabled: bool = row.try_get("enabled")?;
    if writing && !enabled { return Err(StudyPolicyStoreError::ModelDisabled); }
    Ok((StudyModelSnapshot {
        model_config_ref: row.try_get("config_ref")?,
        identity: StudyModelIdentity {
            model_ref: row.try_get("model_ref")?, connection_version_ref: row.try_get("version_ref")?,
            model_id: row.try_get("model_id")?,
        },
        input_token_limit: row.try_get("input_token_limit")?,
        output_token_limit: row.try_get("output_token_limit")?,
        timeout_seconds: row.try_get("timeout_seconds")?,
    }, enabled))
}

fn model_info(model: &StudyModelSnapshot, enabled: bool) -> Value {
    json!({"modelConfigRef":model.model_config_ref,"modelIdentity":model.identity,
        "inputTokenLimit":model.input_token_limit,"outputTokenLimit":model.output_token_limit,
        "timeoutSeconds":model.timeout_seconds,"enabled":enabled})
}

pub(super) const SUMMARY_COLUMNS: &str = "p.policy_ref,p.domain_ref,p.method_name,p.parent_policy_ref, \
    p.model_config_ref,p.method_hash,p.comment_budget,p.context_character_budget, \
    to_char(p.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
    EXISTS(SELECT 1 FROM linggan_comment_study_active_policy a WHERE a.singleton AND a.policy_ref=p.policy_ref) AS is_active";

pub(super) fn summary(row: &sqlx::postgres::PgRow) -> Result<Value, sqlx::Error> {
    let hash: Option<String> = row.try_get("method_hash")?;
    Ok(json!({
        "policyRef": row.try_get::<Uuid,_>("policy_ref")?,
        "methodName": row.try_get::<Option<String>,_>("method_name")?,
        "parentPolicyRef": row.try_get::<Option<Uuid>,_>("parent_policy_ref")?,
        "createdAt": row.try_get::<String,_>("created_at")?,
        "recordingState": if hash.is_some() { "recorded" } else { "legacy_unrecorded" },
        "methodHash": hash,
        "defaults": {"commentBudget": row.try_get::<i32,_>("comment_budget")?,
            "contextCharacterBudget": row.try_get::<i32,_>("context_character_budget")?},
        "isActive": row.try_get::<bool,_>("is_active")?,
    }))
}

pub(super) async fn load_policy(
    tx: &mut Transaction<'_, Postgres>, domain: Uuid, reference: Uuid,
) -> Result<Value, StudyPolicyStoreError> {
    let statement = format!("SELECT {SUMMARY_COLUMNS},p.method_manifest \
        FROM linggan_comment_study_policy p WHERE p.domain_ref=$1 AND p.policy_ref=$2");
    let row = sqlx::query(sqlx::AssertSqlSafe(statement)).bind(domain).bind(reference)
        .fetch_optional(&mut **tx).await?.ok_or(StudyPolicyStoreError::NotFound)?;
    let mut value = summary(&row)?;
    let manifest: Option<Value> = row.try_get("method_manifest")?;
    let hash: Option<String> = row.try_get("method_hash")?;
    match (manifest, hash) {
        (None, None) => {
            // No reconstruction of unrecorded historic prompts from current templates.
            value["methodManifest"] = Value::Null;
            value["model"] = match row.try_get::<Option<Uuid>,_>("model_config_ref")? {
                Some(config) => {
                    let (model, enabled) = model_snapshot(tx, config, false).await?;
                    model_info(&model, enabled)
                }
                None => Value::Null,
            };
        }
        (Some(manifest), Some(method_hash)) => {
            let config = row.try_get::<Option<Uuid>,_>("model_config_ref")?
                .ok_or(StudyPolicyStoreError::ModelUnavailable)?;
            let (model, enabled) = model_snapshot(tx, config, false).await?;
            let method = CompiledStudyMethod {
                manifest: serde_json::from_value(manifest.clone())
                    .map_err(|_| StudyPolicyContractError::IntegrityMismatch)?, method_hash,
            };
            verify_study_method(&method, &model)?;
            value["methodManifest"] = manifest;
            value["model"] = model_info(&model, enabled);
        }
        _ => return Err(StudyPolicyContractError::IntegrityMismatch.into()),
    }
    Ok(value)
}

/// Creates or explicitly copies one immutable version. All edit fields are supplied; parent is
/// verified only as lineage, never taken from the global active pointer. No auto-activation.
pub async fn create_study_policy(
    database: &Database, command: CreateStudyPolicyCommand,
) -> Result<Value, StudyPolicyStoreError> {
    command.validate()?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET LOCAL lock_timeout='5s'").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='15s'").execute(&mut *tx).await?;
    ensure_schema(&mut tx, true).await?;
    if let Some(parent) = command.parent_policy_ref {
        let parent = load_policy(&mut tx, command.domain_ref, parent).await?;
        if parent["recordingState"] != "recorded" { return Err(StudyPolicyStoreError::Unrecorded); }
    }
    let (model, _) = model_snapshot(&mut tx, command.model_config_ref, true).await?;
    let compiled = compile_study_method(&command, &model)?;
    let manifest = serde_json::to_value(compiled.manifest)
        .map_err(|_| StudyPolicyContractError::CanonicalJson)?;
    let reference = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_study_policy \
        (policy_ref,domain_ref,model_config_ref,contract,comment_budget,context_character_budget, \
         method_name,parent_policy_ref,method_manifest,method_hash) \
        VALUES($1,$2,$3,'comment-study.v1',$4,$5,$6,$7,$8,$9)")
        .bind(reference).bind(command.domain_ref).bind(command.model_config_ref)
        .bind(command.defaults.comment_budget).bind(command.defaults.context_character_budget)
        .bind(command.method_name.trim()).bind(command.parent_policy_ref)
        .bind(manifest).bind(compiled.method_hash).execute(&mut *tx).await?;
    let policy = load_policy(&mut tx, command.domain_ref, reference).await?;
    tx.commit().await?;
    Ok(json!({"contract":"comment-study.read.v2","domainRef":command.domain_ref,"policy":policy}))
}
