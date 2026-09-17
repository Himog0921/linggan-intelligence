//! Browser-safe model configuration projection for the V1 research setup.

use crate::model_settings::*;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

pub async fn read_model_settings(
    database: &Database,
    synthetic: bool,
) -> Result<Value, ModelError> {
    ensure_model_schema(database).await?;
    let workspace = read_workspace(database).await?;
    let connections = read_connections(database).await?;
    let models = read_models(database).await?;
    let configuration = read_configuration(database, &workspace).await?;
    let invocations = read_invocations(database).await?;
    Ok(model_settings_projection(
        workspace,
        connections,
        models,
        configuration,
        invocations,
        synthetic,
    ))
}

async fn read_workspace(database: &Database) -> Result<PgRow, ModelError> {
    Ok(sqlx::query(
        "SELECT workspace_ref,default_config_ref,worker_last_seen_at::text AS worker_seen,\
                worker_last_seen_at>scope_001_now()-interval '90 seconds' AS worker_recent,\
                worker_state,worker_last_error \
         FROM linggan_model_workspace WHERE singleton",
    )
    .fetch_one(database.pool())
    .await?)
}

async fn read_connections(database: &Database) -> Result<Vec<PgRow>, ModelError> {
    Ok(sqlx::query(
        "SELECT connection.connection_ref,connection.enabled,connection.revision,\
                version.version_ref,version.name,version.api,version.base_url,version.local_endpoint,\
                (SELECT result FROM linggan_model_invocation invocation \
                   WHERE invocation.connection_version_ref=version.version_ref \
                     AND invocation.operation IN ('connect','discover') \
                   ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 1) AS test \
         FROM linggan_model_connection connection \
         JOIN LATERAL(SELECT * FROM linggan_model_connection_version \
                      WHERE connection_ref=connection.connection_ref \
                      ORDER BY revision DESC LIMIT 1) version ON true \
         ORDER BY connection.created_at",
    )
    .fetch_all(database.pool())
    .await?)
}

async fn read_models(database: &Database) -> Result<Vec<PgRow>, ModelError> {
    Ok(sqlx::query(with_model_callability(
        "SELECT m.model_ref,m.model_id,m.connection_version_ref,\
                version.name AS connection_name,connection.enabled,\
                version.revision=(SELECT max(latest.revision) \
                                  FROM linggan_model_connection_version latest \
                                  WHERE latest.connection_ref=connection.connection_ref) AS current_version,\
                COALESCE((SELECT invocation.state='succeeded' \
                              AND invocation.result->>'semanticQualified'='true' \
                          FROM linggan_model_invocation invocation \
                          WHERE invocation.model_ref=m.model_ref AND invocation.operation='probe' \
                          ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 1),false) AS semantic_qualified,\
                __MODEL_CALLABLE__ AS callable,\
                (SELECT state FROM linggan_model_invocation invocation \
                 WHERE invocation.model_ref=m.model_ref AND invocation.operation='probe' \
                 ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 1) AS test_state,\
                (SELECT result FROM linggan_model_invocation invocation \
                 WHERE invocation.model_ref=m.model_ref AND invocation.operation='probe' \
                 ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 1) AS test \
         FROM linggan_model_entry m \
         JOIN linggan_model_connection_version version ON version.version_ref=m.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         ORDER BY m.created_at",
    ))
    .fetch_all(database.pool())
    .await?)
}

async fn read_configuration(
    database: &Database,
    workspace: &PgRow,
) -> Result<Option<Value>, ModelError> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object(\
            'configRef',config_ref,'modelRef',model_ref,'inputTokenLimit',input_token_limit,\
            'outputTokenLimit',output_token_limit,'timeoutSeconds',timeout_seconds,\
            'maxAttempts',max_attempts) \
         FROM linggan_model_config WHERE config_ref=$1",
    )
    .bind(workspace.get::<Option<Uuid>, _>("default_config_ref"))
    .fetch_optional(database.pool())
    .await?)
}

async fn read_invocations(database: &Database) -> Result<Vec<PgRow>, ModelError> {
    Ok(sqlx::query(
        "SELECT invocation.invocation_ref,invocation.operation,invocation.state,\
                invocation.failure_code,invocation.input_tokens,invocation.output_tokens,\
                invocation.charged_tokens,invocation.reserved_tokens,invocation.elapsed_ms,\
                invocation.created_at::text AS created,invocation.result,model.model_id \
         FROM linggan_model_invocation invocation \
         LEFT JOIN linggan_model_entry model USING(model_ref) \
         ORDER BY invocation.created_at DESC,invocation.invocation_ref DESC LIMIT 100",
    )
    .fetch_all(database.pool())
    .await?)
}

fn model_settings_projection(
    workspace: PgRow,
    connections: Vec<PgRow>,
    models: Vec<PgRow>,
    configuration: Option<Value>,
    invocations: Vec<PgRow>,
    synthetic: bool,
) -> Value {
    json!({
        "workspaceRef": workspace.get::<Uuid, _>("workspace_ref"),
        "secretStorage": if synthetic { "SYNTHETIC_PREVIEW_ONLY" } else { "MACOS_KEYCHAIN" },
        "worker": {
            "lastSeenAt": workspace.get::<Option<String>, _>("worker_seen"),
            "recent": workspace.get::<Option<bool>, _>("worker_recent").unwrap_or(false),
            "state": workspace.get::<Option<String>, _>("worker_state"),
            "lastError": workspace.get::<Option<String>, _>("worker_last_error")
        },
        "connections": connections.iter().map(|row| json!({
            "connectionRef": row.get::<Uuid, _>("connection_ref"),
            "versionRef": row.get::<Uuid, _>("version_ref"),
            "name": row.get::<String, _>("name"),
            "api": row.get::<String, _>("api"),
            "baseUrl": row.get::<String, _>("base_url"),
            "localEndpoint": row.get::<bool, _>("local_endpoint"),
            "enabled": row.get::<bool, _>("enabled"),
            "revision": row.get::<i32, _>("revision"),
            "test": row.get::<Option<Value>, _>("test"),
            "credentialStored": true
        })).collect::<Vec<_>>(),
        "models": models.iter().map(|row| json!({
            "modelRef": row.get::<Uuid, _>("model_ref"),
            "modelId": row.get::<String, _>("model_id"),
            "connectionVersionRef": row.get::<Uuid, _>("connection_version_ref"),
            "connectionName": row.get::<String, _>("connection_name"),
            "currentVersion": row.get::<bool, _>("current_version"),
            "semanticQualified": row.get::<bool, _>("semantic_qualified"),
            "modelCallable": row.get::<bool, _>("callable"),
            "testState": row.get::<Option<String>, _>("test_state"),
            "enabled": row.get::<bool, _>("enabled"),
            "test": row.get::<Option<Value>, _>("test")
        })).collect::<Vec<_>>(),
        "config": configuration,
        "invocations": invocations.iter().map(|row| json!({
            "invocationRef": row.get::<Uuid, _>("invocation_ref"),
            "operation": row.get::<String, _>("operation"),
            "state": row.get::<String, _>("state"),
            "failureCode": row.get::<Option<String>, _>("failure_code"),
            "inputTokens": row.get::<Option<i64>, _>("input_tokens"),
            "outputTokens": row.get::<Option<i64>, _>("output_tokens"),
            "chargedTokens": row.get::<i64, _>("charged_tokens"),
            "reservedTokens": row.get::<i64, _>("reserved_tokens"),
            "elapsedMs": row.get::<Option<i64>, _>("elapsed_ms"),
            "createdAt": row.get::<String, _>("created"),
            "modelId": row.get::<Option<String>, _>("model_id"),
            "result": row.get::<Option<Value>, _>("result")
        })).collect::<Vec<_>>()
    })
}
