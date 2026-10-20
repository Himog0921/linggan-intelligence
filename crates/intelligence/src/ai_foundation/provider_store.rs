//! Provider metadata writes are local facts. Saving never probes or dispatches a request.
use super::{
    TaskKey,
    capability::Capability,
    control_store::Actor,
    store::{Command, FoundationError, valid_reason},
    usage::TokenPrices,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialClass {
    SyntheticProbe,
    ModelCatalog,
    NoteMaterial,
    PublicAuthorBio,
    SelectedWritingMaterial,
}
impl MaterialClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntheticProbe => "synthetic_probe",
            Self::ModelCatalog => "model_catalog",
            Self::NoteMaterial => "note_material",
            Self::PublicAuthorBio => "public_author_bio",
            Self::SelectedWritingMaterial => "selected_writing_material",
        }
    }
    fn permits_task(self, task: TaskKey) -> bool {
        match self {
            Self::SyntheticProbe | Self::ModelCatalog => task == TaskKey::ModelProbe,
            Self::NoteMaterial => matches!(
                task,
                TaskKey::NoteInterpretation
                    | TaskKey::DirectionListDraft
                    | TaskKey::DirectionProposal
            ),
            Self::PublicAuthorBio => task == TaskKey::AuthorOrgIdentity,
            Self::SelectedWritingMaterial => {
                matches!(task, TaskKey::WritingAssistant | TaskKey::WritingLegacy)
            }
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeEgress {
    pub command_ref: Uuid,
    pub endpoint_ref: Uuid,
    pub task_key: TaskKey,
    pub material_class: MaterialClass,
    pub expected_event_ref: Option<Uuid>,
    pub allowed: bool,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EgressReceipt {
    pub event_ref: Uuid,
    pub allowed: bool,
}

async fn lock_endpoint(
    tx: &mut Transaction<'_, Postgres>,
    endpoint: Uuid,
    current_required: bool,
) -> Result<(), FoundationError> {
    let row=sqlx::query("SELECT c.connection_ref,(SELECT p.endpoint_ref FROM linggan_model_connection_version v JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref WHERE v.connection_ref=c.connection_ref ORDER BY v.revision DESC LIMIT 1) AS current_endpoint FROM linggan_model_connection c JOIN linggan_ai_endpoint e USING(connection_ref) WHERE e.endpoint_ref=$1 FOR UPDATE OF c")
        .bind(endpoint).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    if current_required && row.get::<Option<Uuid>, _>("current_endpoint") != Some(endpoint) {
        return Err(FoundationError::Conflict);
    }
    Ok(())
}
pub async fn change_egress(
    db: &Database,
    request: &ChangeEgress,
    actor: Actor,
) -> Result<EgressReceipt, FoundationError> {
    if !valid_reason(&request.reason, 300) || !request.material_class.permits_task(request.task_key)
    {
        return Err(FoundationError::Invalid);
    }
    if actor == Actor::System && request.allowed {
        return Err(FoundationError::SystemCannotResume);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "egress",
        &serde_json::json!({"request":request,"actor":actor}),
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_endpoint(&mut command.tx, request.endpoint_ref, request.allowed).await?;
    let current:Option<Uuid>=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key=$2 AND material_class=$3")
        .bind(request.endpoint_ref).bind(request.task_key.as_str()).bind(request.material_class.as_str())
        .fetch_optional(&mut *command.tx).await?;
    if current != request.expected_event_ref {
        return Err(FoundationError::Conflict);
    }
    sqlx::query("INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(request.command_ref).bind(request.endpoint_ref).bind(request.task_key.as_str())
        .bind(request.material_class.as_str()).bind(request.allowed).bind(actor.as_str()).bind(&request.reason)
        .execute(&mut *command.tx).await?;
    command
        .finish(EgressReceipt {
            event_ref: request.command_ref,
            allowed: request.allowed,
        })
        .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    PiCatalog,
    Manual,
    Probe,
}
impl CapabilitySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::PiCatalog => "pi_catalog",
            Self::Manual => "manual",
            Self::Probe => "probe",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    PromptOnly,
    JsonObject,
    JsonSchema,
}
impl OutputMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PromptOnly => "prompt_only",
            Self::JsonObject => "json_object",
            Self::JsonSchema => "json_schema",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveCapability {
    pub command_ref: Uuid,
    pub identity_ref: Uuid,
    pub endpoint_ref: Uuid,
    pub expected_capability_ref: Option<Uuid>,
    pub source: CapabilitySource,
    pub source_version: String,
    pub probe_invocation_ref: Option<Uuid>,
    pub limits: Capability,
    pub compat: Value,
    pub thinking_level_map: Value,
    pub output_mode: OutputMode,
    pub strict_schema: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetadataReceipt {
    pub revision_ref: Uuid,
}

fn valid_capability_metadata(request: &SaveCapability) -> bool {
    if !valid_reason(&request.source_version, 200)
        || (request.source == CapabilitySource::Probe) != request.probe_invocation_ref.is_some()
        || (request.strict_schema && request.output_mode != OutputMode::JsonSchema)
        || !request.compat.is_object()
        || request.compat.to_string().len() > 16384
    {
        return false;
    }
    let Some(map) = request.thinking_level_map.as_object() else {
        return false;
    };
    if request.thinking_level_map.to_string().len() > 4096 {
        return false;
    }
    map.iter().all(|(key, value)| {
        ["off", "minimal", "low", "medium", "high", "xhigh", "max"].contains(&key.as_str())
            && (value.is_null() || value.as_str().is_some_and(|v| valid_reason(v, 80)))
            && (!value.is_null() || !request.limits.thinking_levels.contains(key))
    })
}
async fn lock_identity(
    tx: &mut Transaction<'_, Postgres>,
    identity: Uuid,
    endpoint: Uuid,
) -> Result<(), FoundationError> {
    lock_endpoint(tx, endpoint, true).await?;
    let matched:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ai_model_identity i JOIN linggan_ai_endpoint e USING(connection_ref) WHERE identity_ref=$1 AND endpoint_ref=$2)")
        .bind(identity).bind(endpoint).fetch_one(&mut **tx).await?;
    if !matched {
        return Err(FoundationError::NotFound);
    }
    Ok(())
}
pub async fn save_capability(
    db: &Database,
    request: &SaveCapability,
) -> Result<MetadataReceipt, FoundationError> {
    request.limits.validate()?;
    if !valid_capability_metadata(request) {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "capability", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_identity(&mut command.tx, request.identity_ref, request.endpoint_ref).await?;
    if request.source == CapabilitySource::Probe {
        let proven: bool = sqlx::query_scalar(
            "SELECT linggan_ai_probe_supports_capability($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(request.probe_invocation_ref)
        .bind(&request.source_version)
        .bind(request.identity_ref)
        .bind(request.endpoint_ref)
        .bind(serde_json::json!(request.limits))
        .bind(&request.compat)
        .bind(&request.thinking_level_map)
        .bind(request.output_mode.as_str())
        .bind(request.strict_schema)
        .fetch_one(&mut *command.tx)
        .await?;
        if !proven {
            return Err(FoundationError::Invalid);
        }
    }
    if request.source == CapabilitySource::PiCatalog {
        let preset = super::catalog::preset_for_identity(
            &mut command.tx,
            request.identity_ref,
            request.endpoint_ref,
            &request.source_version,
        )
        .await?;
        if !preset.supported
            || preset.limits.context_tokens != request.limits.context_tokens
            || preset.limits.max_output_tokens != request.limits.max_output_tokens
            || preset.limits.thinking_levels != request.limits.thinking_levels
            || preset.compat != request.compat
            || preset.thinking_level_map != request.thinking_level_map
            || preset.output_mode != request.output_mode
            || preset.strict_schema != request.strict_schema
        {
            return Err(FoundationError::Invalid);
        }
        // Waiting time is an explicit operator setting; the SDK catalog never supplies it.
    }

    let api: String =
        sqlx::query_scalar("SELECT api FROM linggan_ai_endpoint WHERE endpoint_ref=$1")
            .bind(request.endpoint_ref)
            .fetch_one(&mut *command.tx)
            .await?;
    if !linggan_contracts::ai_protocol::valid_compat(&api, &request.compat)
        || !linggan_contracts::ai_protocol::valid_output_mode(
            &api,
            request.output_mode.as_str(),
            request.strict_schema,
        )
    {
        return Err(FoundationError::Invalid);
    }

    let current:Option<Uuid>=sqlx::query_scalar("SELECT capability_ref FROM linggan_model_capability WHERE identity_ref=$1 AND endpoint_ref=$2 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(request.identity_ref).bind(request.endpoint_ref).fetch_optional(&mut *command.tx).await?;
    if current != request.expected_capability_ref {
        return Err(FoundationError::Conflict);
    }
    sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,probe_invocation_ref,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode,strict_schema) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(request.command_ref).bind(request.identity_ref).bind(request.endpoint_ref)
        .bind(request.source.as_str()).bind(&request.source_version).bind(request.probe_invocation_ref)
        .bind(request.limits.context_tokens as i32).bind(request.limits.max_output_tokens as i32)
        .bind(request.limits.max_timeout_seconds as i32).bind(serde_json::json!(request.limits.thinking_levels))
        .bind(&request.compat).bind(&request.thinking_level_map).bind(request.output_mode.as_str()).bind(request.strict_schema)
        .execute(&mut *command.tx).await?;
    command
        .finish(MetadataReceipt {
            revision_ref: request.command_ref,
        })
        .await
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceSource {
    PiCatalog,
    Manual,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavePrice {
    pub command_ref: Uuid,
    pub identity_ref: Uuid,
    pub endpoint_ref: Uuid,
    pub expected_price_ref: Option<Uuid>,
    pub source: PriceSource,
    pub source_version: String,
    pub prices: TokenPrices,
}
pub async fn save_price(
    db: &Database,
    request: &SavePrice,
) -> Result<MetadataReceipt, FoundationError> {
    let rates = [
        request.prices.input,
        request.prices.output,
        request.prices.cached_read,
        request.prices.cached_write,
    ];
    if !valid_reason(&request.source_version, 200)
        || rates.into_iter().flatten().any(|r| r > i64::MAX as u64)
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "price", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_identity(&mut command.tx, request.identity_ref, request.endpoint_ref).await?;
    if matches!(request.source, PriceSource::PiCatalog) {
        let preset = super::catalog::preset_for_identity(
            &mut command.tx,
            request.identity_ref,
            request.endpoint_ref,
            &request.source_version,
        )
        .await?;
        if preset.prices != request.prices {
            return Err(FoundationError::Invalid);
        }
    }

    let current:Option<Uuid>=sqlx::query_scalar("SELECT price_ref FROM linggan_model_price WHERE identity_ref=$1 AND endpoint_ref=$2 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(request.identity_ref).bind(request.endpoint_ref).fetch_optional(&mut *command.tx).await?;
    if current != request.expected_price_ref {
        return Err(FoundationError::Conflict);
    }
    let source = match request.source {
        PriceSource::PiCatalog => "pi_catalog",
        PriceSource::Manual => "manual",
    };
    sqlx::query("INSERT INTO linggan_model_price(price_ref,identity_ref,endpoint_ref,source_kind,source_version,input_micro_usd_per_million,output_micro_usd_per_million,cached_read_micro_usd_per_million,cached_write_micro_usd_per_million) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(request.command_ref).bind(request.identity_ref).bind(request.endpoint_ref).bind(source).bind(&request.source_version)
        .bind(request.prices.input.map(|v|v as i64)).bind(request.prices.output.map(|v|v as i64))
        .bind(request.prices.cached_read.map(|v|v as i64)).bind(request.prices.cached_write.map(|v|v as i64))
        .execute(&mut *command.tx).await?;
    command
        .finish(MetadataReceipt {
            revision_ref: request.command_ref,
        })
        .await
}
