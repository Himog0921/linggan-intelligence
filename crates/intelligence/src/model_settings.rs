//! Versioned local workspace model configuration; no save command invokes a provider.
use crate::{
    comment_research::{CommentResearchError, valid_text},
    model_secrets::ModelSecretStore,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("invalid_model_command")]
    Invalid,
    #[error("model_revision_conflict")]
    Conflict,
    #[error("model_not_found")]
    NotFound,
    #[error("model_disabled")]
    Disabled,
    #[error("model_secret_unavailable")]
    SecretUnavailable,
    #[error("model_adapter_unavailable")]
    AdapterUnavailable,
    #[error("provider_timeout")]
    Timeout,
    #[error("model_input_limit")]
    InputLimit,
    #[error("model_invalid_output")]
    InvalidOutput,
    #[error("model_budget_exhausted")]
    Budget,
    #[error("model_not_qualified")]
    NotQualified,
    #[error("model_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error("model_source_unavailable")]
    Source,
}
impl From<CommentResearchError> for ModelError {
    fn from(_: CommentResearchError) -> Self {
        Self::Source
    }
}
impl From<linggan_evidence::comment_research_read::CommentResearchReadError> for ModelError {
    fn from(_: linggan_evidence::comment_research_read::CommentResearchReadError) -> Self {
        Self::Source
    }
}
impl ModelError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "invalid_model_command",
            Self::Conflict => "model_revision_conflict",
            Self::NotFound => "model_not_found",
            Self::Disabled => "model_disabled",
            Self::SecretUnavailable => "model_secret_unavailable",
            Self::AdapterUnavailable => "model_adapter_unavailable",
            Self::Timeout => "provider_timeout",
            Self::InputLimit => "model_input_limit",
            Self::InvalidOutput => "model_invalid_output",
            Self::Budget => "model_budget_exhausted",
            Self::NotQualified => "model_not_qualified",
            Self::Database(_) => "model_database_unavailable",
            Self::Source => "model_source_unavailable",
        }
    }
}

pub async fn workspace_ref(db: &Database) -> Result<Uuid, ModelError> {
    Ok(
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await?,
    )
}
pub fn validate_endpoint(base: &str, local: bool) -> Result<(), ModelError> {
    let url = url::Url::parse(base).map_err(|_| ModelError::Invalid)?;
    if base.len() > 1000
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (local && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
        || !(url.scheme() == "https"
            || (local
                && url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
    {
        return Err(ModelError::Invalid);
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveModelConnection {
    pub version_ref: Uuid,
    pub connection_ref: Uuid,
    pub expected_revision: i32,
    pub name: String,
    pub api: String,
    pub base_url: String,
    pub local_endpoint: bool,
    pub api_key: String,
}

pub async fn save_model_connection(
    db: &Database,
    store: &dyn ModelSecretStore,
    r: &SaveModelConnection,
) -> Result<Value, ModelError> {
    if !valid_text(&r.name, 100)
        || !matches!(
            r.api.as_str(),
            "openai-completions" | "openai-responses" | "anthropic-messages"
        )
        || r.expected_revision < 0
        || r.expected_revision == i32::MAX
        || r.api_key.len() > 4096
        || (!r.local_endpoint && r.api_key.trim().is_empty())
    {
        return Err(ModelError::Invalid);
    }
    validate_endpoint(&r.base_url, r.local_endpoint)?;
    if store.is_synthetic() && !(r.local_endpoint && r.base_url.starts_with("http://127.0.0.1:")) {
        return Err(ModelError::Invalid);
    }
    let workspace = workspace_ref(db).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query(
        "INSERT INTO linggan_model_connection(connection_ref) VALUES($1) ON CONFLICT DO NOTHING",
    )
    .bind(r.connection_ref)
    .execute(&mut *tx)
    .await?;
    let revision: i32 = sqlx::query_scalar(
        "SELECT revision FROM linggan_model_connection WHERE connection_ref=$1 FOR UPDATE",
    )
    .bind(r.connection_ref)
    .fetch_one(&mut *tx)
    .await?;
    if let Some(row) =
        sqlx::query("SELECT * FROM linggan_model_connection_version WHERE version_ref=$1")
            .bind(r.version_ref)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<Uuid, _>("connection_ref") != r.connection_ref
            || row.get::<i32, _>("revision") != r.expected_revision + 1
            || row.get::<String, _>("name") != r.name
            || row.get::<String, _>("api") != r.api
            || row.get::<String, _>("base_url") != r.base_url
            || row.get::<bool, _>("local_endpoint") != r.local_endpoint
            || store.get(workspace, row.get("secret_ref"))? != r.api_key
        {
            return Err(ModelError::Conflict);
        }
        return Ok(
            json!({"connectionRef":r.connection_ref,"versionRef":r.version_ref,"revision":revision,"replayed":true}),
        );
    }
    if revision != r.expected_revision {
        return Err(ModelError::Conflict);
    }
    let secret_ref = Uuid::new_v4();
    store.put(workspace, secret_ref, &r.api_key)?;
    let result=async{
        sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(r.version_ref).bind(r.connection_ref).bind(revision+1).bind(&r.name).bind(&r.api).bind(&r.base_url).bind(r.local_endpoint).bind(secret_ref).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_model_connection SET revision=revision+1 WHERE connection_ref=$1").bind(r.connection_ref).execute(&mut *tx).await?;
        tx.commit().await?;Ok::<(),sqlx::Error>(())
    }.await;
    if result.is_err() {
        // A connection error during commit is ambiguous. Only remove the new secret when
        // a fresh database read proves no committed version references it.
        let absent = sqlx::query_scalar::<_, bool>(
            "SELECT NOT EXISTS(SELECT 1 FROM linggan_model_connection_version WHERE secret_ref=$1)",
        )
        .bind(secret_ref)
        .fetch_one(db.pool())
        .await;
        if matches!(absent, Ok(true)) {
            let _ = store.delete(workspace, secret_ref);
        }
    }
    result?;
    Ok(
        json!({"connectionRef":r.connection_ref,"versionRef":r.version_ref,"revision":revision+1,"state":"SAVED_UNTESTED"}),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetModelConnectionEnabled {
    pub connection_ref: Uuid,
    pub expected_revision: i32,
    pub enabled: bool,
}
pub async fn set_model_connection_enabled(
    db: &Database,
    r: &SetModelConnectionEnabled,
) -> Result<Value, ModelError> {
    let result=sqlx::query("UPDATE linggan_model_connection SET enabled=$3,revision=revision+1 WHERE connection_ref=$1 AND revision=$2")
        .bind(r.connection_ref).bind(r.expected_revision).bind(r.enabled).execute(db.pool()).await?;
    if result.rows_affected() != 1 {
        return Err(ModelError::Conflict);
    }
    Ok(json!({"enabled":r.enabled,"revision":r.expected_revision+1}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveModelEntry {
    pub model_ref: Uuid,
    pub connection_version_ref: Uuid,
    pub model_id: String,
}
pub async fn save_model_entry(db: &Database, r: &SaveModelEntry) -> Result<Value, ModelError> {
    if !valid_text(&r.model_id, 200) {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_connection_version v JOIN linggan_model_connection c USING(connection_ref) WHERE v.version_ref=$1 AND c.enabled)")
        .bind(r.connection_version_ref).fetch_one(&mut *tx).await?;
    if !exists {
        return Err(ModelError::Disabled);
    }
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,$3,'manual') ON CONFLICT DO NOTHING")
        .bind(r.model_ref).bind(r.connection_version_ref).bind(&r.model_id).execute(&mut *tx).await?;
    let row=sqlx::query("SELECT model_ref,connection_version_ref,model_id FROM linggan_model_entry WHERE model_ref=$1 OR (connection_version_ref=$2 AND model_id=$3) ORDER BY (model_ref=$1) DESC LIMIT 1")
        .bind(r.model_ref).bind(r.connection_version_ref).bind(&r.model_id).fetch_one(&mut *tx).await?;
    if row.get::<Uuid, _>("connection_version_ref") != r.connection_version_ref
        || row.get::<String, _>("model_id") != r.model_id
    {
        return Err(ModelError::Conflict);
    }
    let model_ref: Uuid = row.get("model_ref");
    tx.commit().await?;
    Ok(json!({"modelRef":model_ref,"state":"UNTESTED"}))
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveModelConfig {
    pub config_ref: Uuid,
    pub expected_config_ref: Option<Uuid>,
    pub model_ref: Uuid,
    pub input_token_limit: i32,
    pub output_token_limit: i32,
    pub timeout_seconds: i32,
    pub max_attempts: i32,
    pub auto_source_limit: i32,
    pub auto_token_limit: i64,
}
impl SaveModelConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        if !(1024..=32768).contains(&self.input_token_limit)
            || !(128..=8192).contains(&self.output_token_limit)
            || !(1..=60).contains(&self.timeout_seconds)
            || !(1..=3).contains(&self.max_attempts)
            || !(1..=1000).contains(&self.auto_source_limit)
            || !(1024..=10_000_000).contains(&self.auto_token_limit)
        {
            Err(ModelError::Invalid)
        } else {
            Ok(())
        }
    }
}
pub async fn save_model_config(db: &Database, r: &SaveModelConfig) -> Result<Value, ModelError> {
    r.validate()?;
    let mut tx = db.pool().begin().await?;
    let current: Option<Uuid> = sqlx::query_scalar(
        "SELECT default_config_ref FROM linggan_model_workspace WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(row) = sqlx::query(
        "SELECT to_jsonb(c)-'created_at' AS config FROM linggan_model_config c WHERE config_ref=$1",
    )
    .bind(r.config_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        let old: Value = row.get("config");
        let expected = json!({"config_ref":r.config_ref,"model_ref":r.model_ref,"input_token_limit":r.input_token_limit,"output_token_limit":r.output_token_limit,"timeout_seconds":r.timeout_seconds,"max_attempts":r.max_attempts,"auto_source_limit":r.auto_source_limit,"auto_token_limit":r.auto_token_limit});
        if old != expected {
            return Err(ModelError::Conflict);
        }
        return Ok(json!({"configRef":current,"replayed":true}));
    }
    if current != r.expected_config_ref {
        return Err(ModelError::Conflict);
    }
    let enabled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_entry m JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE m.model_ref=$1 AND c.enabled)")
        .bind(r.model_ref).fetch_one(&mut *tx).await?;
    if !enabled {
        return Err(ModelError::Disabled);
    }
    let qualified:bool=sqlx::query_scalar("SELECT COALESCE((SELECT state='succeeded' AND result->>'commentQualified'='true' FROM linggan_model_invocation WHERE model_ref=$1 AND operation='probe' ORDER BY created_at DESC LIMIT 1),false)")
        .bind(r.model_ref).fetch_one(&mut *tx).await?;
    if !qualified {
        return Err(ModelError::NotQualified);
    }
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,auto_source_limit,auto_token_limit) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(r.config_ref).bind(r.model_ref).bind(r.input_token_limit).bind(r.output_token_limit).bind(r.timeout_seconds).bind(r.max_attempts).bind(r.auto_source_limit).bind(r.auto_token_limit).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=$1 WHERE singleton")
        .bind(r.config_ref)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({"configRef":r.config_ref,"state":"SAVED_NO_ANALYSIS_STARTED"}))
}
