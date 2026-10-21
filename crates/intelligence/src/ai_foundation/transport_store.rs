//! Loads one frozen configuration against the current route/key version. No secret is read,
//! no permissions are granted, and no provider is contacted here.
use super::{
    capability::{Capability, ModelLimits},
    provider_store::OutputMode,
    store::FoundationError,
    transport::{PiExecution, ResponseSchema},
};
use crate::pi_adapter::{PI_PROTOCOL, PiRequest};
use serde_json::Value;
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

pub(crate) fn execution_from_row(
    row: &PgRow,
    capability_ref: Uuid,
    limits: &ModelLimits,
) -> Result<PiExecution, FoundationError> {
    let capability = Capability {
        context_tokens: row.try_get::<i32, _>("context_tokens")? as u64,
        max_output_tokens: row.try_get::<i32, _>("max_output_tokens")? as u64,
        max_timeout_seconds: row.try_get::<i32, _>("max_timeout_seconds")? as u64,
        thinking_levels: serde_json::from_value(row.try_get::<Value, _>("thinking_levels")?)
            .map_err(|_| FoundationError::Invalid)?,
    };
    capability.permits(limits)?;
    let output_mode: OutputMode =
        serde_json::from_value(Value::String(row.try_get("output_mode")?))
            .map_err(|_| FoundationError::Invalid)?;
    let execution = PiExecution {
        capability_ref,
        limits: capability,
        input_token_limit: limits.input_tokens,
        thinking_level: limits.thinking_level.clone(),
        compat: row.try_get("compat")?,
        thinking_level_map: serde_json::from_value(row.try_get::<Value, _>("thinking_level_map")?)
            .map_err(|_| FoundationError::Invalid)?,
        output_mode,
        strict_schema: row.try_get("strict_schema")?,
        response_schema: None,
    };
    execution
        .validate(
            &row.try_get::<String, _>("api")?,
            limits.output_tokens,
            limits.timeout_seconds * 1000,
            "",
            "",
        )
        .map_err(|_| FoundationError::ConfigurationIncomplete)?;
    Ok(execution)
}
/// A probe freezes its own selected profile; it is never a business task configuration or trial.
pub async fn load_probe_execution_in(
    conn: &mut sqlx::PgConnection,
    model: Uuid,
    endpoint: Uuid,
    capability: Uuid,
    limits: &ModelLimits,
) -> Result<PiExecution, FoundationError> {
    let row=sqlx::query("SELECT cap.*,e.api FROM linggan_model_capability cap
        JOIN linggan_ai_endpoint e USING(endpoint_ref) JOIN linggan_ai_model_identity identity USING(identity_ref)
        JOIN linggan_model_connection_version v ON v.connection_ref=identity.connection_ref
        JOIN linggan_model_entry m ON m.connection_version_ref=v.version_ref AND m.model_id=identity.model_id
        WHERE cap.capability_ref=$1 AND cap.endpoint_ref=$2 AND m.model_ref=$3")
        .bind(capability).bind(endpoint).bind(model).fetch_optional(&mut *conn).await?
        .ok_or(FoundationError::ConfigurationIncomplete)?;
    execution_from_row(&row, capability, limits)
}
pub struct FrozenModel {
    pub config_ref: Uuid,
    pub model_ref: Uuid,
    pub current_model_ref: Option<Uuid>,
    pub identity_ref: Uuid,
    pub endpoint_ref: Uuid,
    pub connection_version_ref: Uuid,
    pub secret_ref: Uuid,
    pub model_id: String,
    pub api: String,
    pub base_url: String,
    pub local_endpoint: bool,
    pub limits: ModelLimits,
    pub execution: PiExecution,
}
impl FrozenModel {
    pub fn request(
        &self,
        api_key: String,
        system: String,
        prompt: String,
        response_schema: Option<ResponseSchema>,
    ) -> PiRequest {
        let mut execution = self.execution.clone();
        execution.response_schema = response_schema;
        PiRequest {
            images: vec![],
            version: PI_PROTOCOL,
            operation: "analyze".into(),
            api: self.api.clone(),
            base_url: self.base_url.clone(),
            local_endpoint: self.local_endpoint,
            api_key,
            model_id: self.model_id.clone(),
            timeout_ms: self.limits.timeout_seconds * 1000,
            max_output_tokens: self.limits.output_tokens as i32,
            system,
            prompt,
            execution: Some(execution),
        }
    }
}
/// Caller owns its dispatch transaction and must separately recheck task switches, budget,
/// sources, exact-current-version callability and egress. Loading is not admission.
pub async fn load_frozen_in(
    conn: &mut sqlx::PgConnection,
    config_ref: Uuid,
) -> Result<FrozenModel, FoundationError> {
    let row=sqlx::query("SELECT config.model_ref,config.capability_ref,config.input_token_limit,config.output_token_limit,
        config.timeout_seconds,config.max_attempts,config.thinking_level,model.model_id,identity.identity_ref,
        cap.context_tokens,cap.max_output_tokens,cap.max_timeout_seconds,cap.thinking_levels,cap.compat,
        cap.thinking_level_map,cap.output_mode,cap.strict_schema,cap.endpoint_ref,
        current.version_ref,current.secret_ref,current.api,current.base_url,current.local_endpoint,current_model.model_ref AS current_model_ref,
        connection.enabled,route.endpoint_ref AS current_endpoint
        FROM linggan_model_config config JOIN linggan_model_entry model ON model.model_ref=config.model_ref
        JOIN linggan_model_connection_version original ON original.version_ref=model.connection_version_ref
        JOIN linggan_model_connection connection ON connection.connection_ref=original.connection_ref
        JOIN linggan_ai_model_identity identity ON identity.connection_ref=connection.connection_ref AND identity.model_id=model.model_id
        JOIN linggan_model_capability cap ON cap.capability_ref=config.capability_ref AND cap.identity_ref=identity.identity_ref
        JOIN LATERAL(SELECT * FROM linggan_model_connection_version v WHERE v.connection_ref=connection.connection_ref ORDER BY revision DESC LIMIT 1)current ON true
        JOIN linggan_ai_connection_endpoint route ON route.version_ref=current.version_ref
        LEFT JOIN linggan_model_entry current_model ON current_model.connection_version_ref=current.version_ref AND current_model.model_id=model.model_id
        WHERE config.config_ref=$1 FOR SHARE OF connection")
        .bind(config_ref).fetch_optional(&mut *conn).await?.ok_or(FoundationError::ConfigurationIncomplete)?;
    if !row.try_get::<bool, _>("enabled")? {
        return Err(FoundationError::AdmissionDenied(vec![
            "model_connection_disabled".into(),
        ]));
    }
    if row.try_get::<Uuid, _>("endpoint_ref")? != row.try_get::<Uuid, _>("current_endpoint")? {
        return Err(FoundationError::Conflict);
    }
    let limits = ModelLimits {
        input_tokens: row.try_get::<i32, _>("input_token_limit")? as u64,
        output_tokens: row.try_get::<i32, _>("output_token_limit")? as u64,
        timeout_seconds: row.try_get::<i32, _>("timeout_seconds")? as u64,
        attempts: row.try_get::<i32, _>("max_attempts")? as u32,
        thinking_level: row.try_get("thinking_level")?,
    };
    let execution = execution_from_row(&row, row.try_get("capability_ref")?, &limits)?;
    Ok(FrozenModel {
        config_ref,
        model_ref: row.try_get("model_ref")?,
        current_model_ref: row.try_get("current_model_ref")?,
        identity_ref: row.try_get("identity_ref")?,
        endpoint_ref: row.try_get("endpoint_ref")?,
        connection_version_ref: row.try_get("version_ref")?,
        secret_ref: row.try_get("secret_ref")?,
        model_id: row.try_get("model_id")?,
        api: row.try_get("api")?,
        base_url: row.try_get("base_url")?,
        local_endpoint: row.try_get("local_endpoint")?,
        limits,
        execution,
    })
}
