//! Explicit embedding configuration. Qualification checks transport and shape, not recall quality.
use crate::{
    model_invocation::{connection_request, finish_invocation, invocation_replay},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::*,
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub async fn read(db: &Database) -> Result<Value, ModelError> {
    let r=sqlx::query("SELECT s.revision,c.config_ref,c.model_ref,c.dimensions,c.qualified,c.enabled FROM linggan_embedding_settings s LEFT JOIN linggan_embedding_config c USING(config_ref) WHERE singleton").fetch_one(db.pool()).await?;
    Ok(
        json!({"revision":r.get::<i64,_>("revision"),"configRef":r.get::<Option<Uuid>,_>("config_ref"),"modelRef":r.get::<Option<Uuid>,_>("model_ref"),"dimensions":r.get::<Option<i32>,_>("dimensions"),"qualified":r.get::<Option<bool>,_>("qualified"),"enabled":r.get::<Option<bool>,_>("enabled"),"protocol":"openai-embeddings.v1","qualificationMeaning":"连接与向量结构通过，不代表问题召回质量已验收"}),
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveEmbedding {
    pub expected_revision: i64,
    pub model_ref: Uuid,
    pub enabled: bool,
}
pub async fn save(db: &Database, r: &SaveEmbedding) -> Result<Value, ModelError> {
    let mut tx = db.pool().begin().await?;
    let s=sqlx::query("SELECT s.revision,s.config_ref,c.model_ref,c.qualified FROM linggan_embedding_settings s LEFT JOIN linggan_embedding_config c USING(config_ref) WHERE singleton FOR UPDATE OF s").fetch_one(&mut *tx).await?;
    if s.get::<i64, _>("revision") != r.expected_revision {
        return Err(ModelError::Conflict);
    }
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_entry m JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE model_ref=$1 AND c.enabled AND v.api IN ('openai-completions','openai-responses'))").bind(r.model_ref).fetch_one(&mut *tx).await?;
    if !allowed {
        return Err(ModelError::Disabled);
    }
    let same = s.get::<Option<Uuid>, _>("model_ref") == Some(r.model_ref);
    if r.enabled && (!same || s.get::<Option<bool>, _>("qualified") != Some(true)) {
        return Err(ModelError::EmbeddingNotQualified);
    }
    let config = if same {
        s.get::<Uuid, _>("config_ref")
    } else {
        Uuid::new_v4()
    };
    if !same {
        sqlx::query("INSERT INTO linggan_embedding_config(config_ref,model_ref) VALUES($1,$2)")
            .bind(config)
            .bind(r.model_ref)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE linggan_embedding_config SET enabled=$2,updated_at=scope_001_now() WHERE config_ref=$1").bind(config).bind(r.enabled).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE linggan_embedding_settings SET config_ref=$1,revision=revision+1 WHERE singleton",
    )
    .bind(config)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    read(db).await
}
/// Contains secret reference only; this internal DTO must not be returned by the settings API.
pub async fn active_config(db: &Database) -> Result<Option<Value>, ModelError> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object('configRef',c.config_ref,'modelRef',m.model_ref,'connectionVersionRef',v.version_ref,'modelId',m.model_id,'dimensions',c.dimensions,'baseUrl',v.base_url,'api',v.api,'localEndpoint',v.local_endpoint,'secretRef',v.secret_ref,'workspaceRef',w.workspace_ref) FROM linggan_embedding_settings s JOIN linggan_embedding_config c USING(config_ref) JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection cn USING(connection_ref) CROSS JOIN linggan_model_workspace w WHERE s.singleton AND w.singleton AND c.qualified AND c.enabled AND cn.enabled").fetch_optional(db.pool()).await?)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeEmbedding {
    pub invocation_ref: Uuid,
    pub config_ref: Uuid,
}
pub async fn probe(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    r: &ProbeEmbedding,
) -> Result<Value, ModelError> {
    let row=sqlx::query("SELECT c.model_ref,m.model_id,m.connection_version_ref FROM linggan_embedding_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$1").bind(r.config_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    let model: Uuid = row.get("model_ref");
    let version: Uuid = row.get("connection_version_ref");
    let hash = content_hash(&format!("embedding-probe.v1:{}", r.config_ref));
    let mut request = connection_request(db, store, version).await?;
    request.operation = "embed".into();
    request.model_id = row.get("model_id");
    request.prompt = json!(["SYNTHETIC / NOT EVIDENCE：孩子写作业启动困难"]).to_string();
    let inserted=sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens) VALUES($1,$2,$3,'embed',$4,'running',1024,1024) ON CONFLICT DO NOTHING").bind(r.invocation_ref).bind(version).bind(model).bind(&hash).execute(db.pool()).await?;
    if inserted.rows_affected() == 0 {
        return invocation_replay(db, r.invocation_ref, &hash).await;
    }
    let response = adapter.call(&request).await;
    let parsed = response
        .as_ref()
        .ok()
        .filter(|p| p.ok)
        .and_then(|p| serde_json::from_str::<Value>(p.text.as_deref()?).ok());
    let dimensions = parsed
        .as_ref()
        .and_then(|v| v["dimensions"].as_i64())
        .filter(|n| (1..=8192).contains(n));
    let good = dimensions.is_some()
        && response
            .as_ref()
            .ok()
            .and_then(|p| p.usage.input_tokens)
            .is_none_or(|n| n <= 1024);
    let failure = if good {
        None
    } else {
        Some(
            response
                .as_ref()
                .ok()
                .and_then(|p| p.failure_code.as_deref())
                .unwrap_or("invalid_embedding_output"),
        )
    };
    let result = json!({"embeddingQualified":good,"configRef":r.config_ref,"dimensions":dimensions,"failureCode":failure,"qualificationMeaning":"仅连接和向量结构，不代表语义质量"});
    finish_invocation(
        db,
        r.invocation_ref,
        response.as_ref().ok(),
        good,
        failure,
        &result,
    )
    .await?;
    sqlx::query("UPDATE linggan_embedding_config SET qualified=$2,dimensions=$3,enabled=enabled AND $2,updated_at=scope_001_now() WHERE config_ref=$1").bind(r.config_ref).bind(good).bind(dimensions.map(|n|n as i32)).execute(db.pool()).await?;
    Ok(result)
}
