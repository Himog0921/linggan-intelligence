//! Safe projections never select a credential or its reference for browser output.
use crate::{model_plans::model_version, model_settings::*};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub async fn read_model_settings(db: &Database, synthetic: bool) -> Result<Value, ModelError> {
    let workspace=sqlx::query("SELECT workspace_ref,default_config_ref,active_auto_plan_ref,worker_last_seen_at::text AS worker_seen,worker_last_seen_at>scope_001_now()-interval '90 seconds' AS worker_recent,worker_state,worker_last_error FROM linggan_model_workspace WHERE singleton").fetch_one(db.pool()).await?;
    let connections=sqlx::query("SELECT c.connection_ref,c.enabled,c.revision,v.version_ref,v.name,v.api,v.base_url,v.local_endpoint,(SELECT result FROM linggan_model_invocation i WHERE i.connection_version_ref=v.version_ref AND operation IN ('connect','discover') ORDER BY i.created_at DESC LIMIT 1) AS test FROM linggan_model_connection c JOIN LATERAL(SELECT * FROM linggan_model_connection_version WHERE connection_ref=c.connection_ref ORDER BY revision DESC LIMIT 1)v ON true ORDER BY c.created_at")
        .fetch_all(db.pool()).await?;
    let models=sqlx::query("SELECT m.model_ref,m.model_id,m.connection_version_ref,v.name AS connection_name,c.enabled,(SELECT result FROM linggan_model_invocation i WHERE i.model_ref=m.model_ref AND operation='probe' ORDER BY i.created_at DESC LIMIT 1) AS test FROM linggan_model_entry m JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) ORDER BY m.created_at")
        .fetch_all(db.pool()).await?;
    let config:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('configRef',config_ref,'modelRef',model_ref,'inputTokenLimit',input_token_limit,'outputTokenLimit',output_token_limit,'timeoutSeconds',timeout_seconds,'maxAttempts',max_attempts,'autoSourceLimit',auto_source_limit,'autoTokenLimit',auto_token_limit) FROM linggan_model_config WHERE config_ref=$1")
        .bind(workspace.get::<Option<Uuid>,_>("default_config_ref")).fetch_optional(db.pool()).await?;
    let plans=sqlx::query("SELECT p.*,p.created_at::text AS created,(SELECT count(*) FROM linggan_comment_model_work b JOIN linggan_model_config f ON f.config_ref=b.config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=p.plan_ref AND w.state='pending' AND NOT c.enabled) AS disabled_count,(SELECT min(f.input_token_limit+f.output_token_limit) FROM linggan_comment_model_work b JOIN linggan_model_config f ON f.config_ref=b.config_ref JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=p.plan_ref AND w.state='pending') AS next_reservation,(SELECT count(*) FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=p.plan_ref AND w.state='pending') AS pending_count,(SELECT count(*) FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=p.plan_ref AND w.state='running') AS running_count,(SELECT count(*) FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=p.plan_ref AND w.state IN ('succeeded','no_signal','failed')) AS finished_count,(SELECT count(*) FROM linggan_comment_model_work w WHERE w.plan_ref=p.plan_ref) AS source_count,(SELECT COALESCE(sum(charged_tokens),0)::bigint FROM linggan_model_invocation i WHERE i.plan_ref=p.plan_ref) AS budget_used,(SELECT count(*) FROM linggan_model_invocation i WHERE i.plan_ref=p.plan_ref AND (i.input_tokens IS NULL OR i.output_tokens IS NULL)) AS unknown_usage_count FROM linggan_model_plan p ORDER BY p.created_at DESC LIMIT 50")
        .fetch_all(db.pool()).await?;
    let runs=sqlx::query("SELECT i.invocation_ref,i.operation,i.state,i.failure_code,i.input_tokens,i.output_tokens,i.charged_tokens,i.reserved_tokens,i.elapsed_ms,i.created_at::text AS created,i.model_ref,i.work_ref,i.config_ref,i.result,model.model_id,work.source_ref,work.attempts FROM linggan_model_invocation i LEFT JOIN linggan_model_entry model USING(model_ref) LEFT JOIN linggan_comment_analysis_work work USING(work_ref) ORDER BY i.created_at DESC LIMIT 100")
        .fetch_all(db.pool()).await?;
    Ok(
        json!({"workspaceRef":workspace.get::<Uuid,_>("workspace_ref"),"activeAutoPlanRef":workspace.get::<Option<Uuid>,_>("active_auto_plan_ref"),"secretStorage":if synthetic{"SYNTHETIC_PREVIEW_ONLY"}else{"MACOS_KEYCHAIN"},
        "worker":{"lastSeenAt":workspace.get::<Option<String>,_>("worker_seen"),"recent":workspace.get::<Option<bool>,_>("worker_recent").unwrap_or(false),"state":workspace.get::<Option<String>,_>("worker_state"),"lastError":workspace.get::<Option<String>,_>("worker_last_error")},
        "connections":connections.iter().map(|r|json!({"connectionRef":r.get::<Uuid,_>("connection_ref"),"versionRef":r.get::<Uuid,_>("version_ref"),"name":r.get::<String,_>("name"),"api":r.get::<String,_>("api"),"baseUrl":r.get::<String,_>("base_url"),"localEndpoint":r.get::<bool,_>("local_endpoint"),"enabled":r.get::<bool,_>("enabled"),"revision":r.get::<i32,_>("revision"),"test":r.get::<Option<Value>,_>("test"),"credentialStored":true})).collect::<Vec<_>>(),
        "models":models.iter().map(|r|json!({"modelRef":r.get::<Uuid,_>("model_ref"),"modelId":r.get::<String,_>("model_id"),"connectionVersionRef":r.get::<Uuid,_>("connection_version_ref"),"connectionName":r.get::<String,_>("connection_name"),"enabled":r.get::<bool,_>("enabled"),"test":r.get::<Option<Value>,_>("test")})).collect::<Vec<_>>(),"config":config,
        "plans":plans.iter().map(|r|json!({"planRef":r.get::<Uuid,_>("plan_ref"),"kind":r.get::<String,_>("kind"),"enabled":r.get::<bool,_>("enabled"),"sourceLimit":r.get::<i32,_>("source_limit"),"sourceCount":r.get::<i64,_>("source_count"),"tokenLimit":r.get::<i64,_>("token_limit"),"budgetUsed":r.get::<i64,_>("budget_used"),"unknownUsageCount":r.get::<i64,_>("unknown_usage_count"),"pendingCount":r.get::<i64,_>("pending_count"),"disabledCount":r.get::<i64,_>("disabled_count"),"nextReservation":r.get::<Option<i32>,_>("next_reservation"),"runningCount":r.get::<i64,_>("running_count"),"finishedCount":r.get::<i64,_>("finished_count"),"createdAt":r.get::<String,_>("created")})).collect::<Vec<_>>(),
        "runs":runs.iter().map(|r|json!({"invocationRef":r.get::<Uuid,_>("invocation_ref"),"operation":r.get::<String,_>("operation"),"state":r.get::<String,_>("state"),"failureCode":r.get::<Option<String>,_>("failure_code"),"inputTokens":r.get::<Option<i64>,_>("input_tokens"),"outputTokens":r.get::<Option<i64>,_>("output_tokens"),"budgetAccounted":r.get::<i64,_>("charged_tokens"),"reservedTokens":r.get::<i64,_>("reserved_tokens"),"costUsd":null,"elapsedMs":r.get::<Option<i64>,_>("elapsed_ms"),"createdAt":r.get::<String,_>("created"),"modelId":r.get::<Option<String>,_>("model_id"),"workRef":r.get::<Option<Uuid>,_>("work_ref"),"sourceRef":r.get::<Option<Uuid>,_>("source_ref"),"configRef":r.get::<Option<Uuid>,_>("config_ref"),"attempts":r.get::<Option<i32>,_>("attempts")})).collect::<Vec<_>>() }),
    )
}
pub async fn current_model_version(db: &Database) -> Result<Option<String>, ModelError> {
    let config: Option<Uuid> = sqlx::query_scalar(
        "SELECT default_config_ref FROM linggan_model_workspace WHERE singleton",
    )
    .fetch_one(db.pool())
    .await?;
    Ok(config.map(model_version))
}

pub async fn current_comment_model_state(db: &Database) -> Result<Value, ModelError> {
    let row=sqlx::query("SELECT c.enabled,config.config_ref FROM linggan_model_workspace ws JOIN linggan_model_config config ON config.config_ref=ws.default_config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE ws.singleton").fetch_optional(db.pool()).await?;
    Ok(match row {
        Some(r) => {
            json!({"modelConnected":r.get::<bool,_>("enabled"),"modelState":if r.get::<bool,_>("enabled"){"CONFIGURED"}else{"PAUSED"}})
        }
        None => json!({"modelConnected":false,"modelState":"NOT_CONFIGURED"}),
    })
}
