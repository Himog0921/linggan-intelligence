//! Immutable method compilation for Comment Study. No database or provider side effects.
//! Persistence/activation must validate the referenced domain, parent and enabled model separately.
use crate::comment_cleaning::CLEANER_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod templates;
#[cfg(test)]
mod tests;

pub const METHOD_CONTRACT: &str = "comment-study.method.v1";
pub const BUILDER_REVISION: &str = "comment-study.request-builder.v2";
pub const SAFETY_REVISION: &str = "comment-study.safety.v1";
const MAX_SCHEMA_BYTES: usize = 6144;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyStage { Semantic, Resolution, Pair }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyPolicyDefaults {
    pub comment_budget: i32,
    pub context_character_budget: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyStageInstructions {
    pub semantic: String,
    pub resolution: String,
    pub pair: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStudyPolicyCommand {
    pub domain_ref: Uuid,
    pub method_name: String,
    pub parent_policy_ref: Option<Uuid>,
    pub model_config_ref: Uuid,
    pub defaults: StudyPolicyDefaults,
    pub stage_instructions: StudyStageInstructions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyModelIdentity {
    pub model_ref: Uuid,
    pub connection_version_ref: Uuid,
    pub model_id: String,
}

/// Supplied by the server's immutable model-config reader, never deserialized from an HTTP body.
#[derive(Debug, Clone)]
pub struct StudyModelSnapshot {
    pub model_config_ref: Uuid,
    pub identity: StudyModelIdentity,
    pub input_token_limit: i32,
    pub output_token_limit: i32,
    pub timeout_seconds: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyMethodStage {
    pub system_instruction: String,
    pub output_schema: Value,
    pub stage_hash: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StudyMethodStages {
    pub semantic: StudyMethodStage,
    pub resolution: StudyMethodStage,
    pub pair: StudyMethodStage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyMethodManifest {
    pub contract: String,
    pub cleaner_version: String,
    pub builder_revision: String,
    pub safety_rules_revision: String,
    pub model_config_ref: Uuid,
    pub model_identity: StudyModelIdentity,
    pub stages: StudyMethodStages,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledStudyMethod {
    pub manifest: StudyMethodManifest,
    pub method_hash: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StudyPolicyContractError {
    #[error("invalid_study_policy_request")]
    InvalidRequest,
    #[error("unsupported_study_domain")]
    UnsupportedDomain,
    #[error("invalid_study_model_snapshot")]
    InvalidModelSnapshot,
    #[error("invalid_canonical_json")]
    CanonicalJson,
    #[error("invalid_study_output_schema")]
    OutputSchema,
    #[error("unsupported_study_method_revision")]
    UnsupportedRevision,
    #[error("study_method_integrity_mismatch")]
    IntegrityMismatch,
}

impl CreateStudyPolicyCommand {
    pub fn validate(&self) -> Result<(), StudyPolicyContractError> {
        if self.domain_ref.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
            return Err(StudyPolicyContractError::UnsupportedDomain);
        }
        if self.model_config_ref.is_nil()
            || self.parent_policy_ref.is_some_and(|id| id.is_nil())
            || !bounded_text(self.method_name.trim(), 1, 100)
            || !(1..=3000).contains(&self.defaults.comment_budget)
            || !(1..=20000).contains(&self.defaults.context_character_budget)
        { return Err(StudyPolicyContractError::InvalidRequest); }
        for text in [&self.stage_instructions.semantic, &self.stage_instructions.resolution,
                     &self.stage_instructions.pair] {
            if !bounded_text(text, 0, 8000) { return Err(StudyPolicyContractError::InvalidRequest); }
        }
        Ok(())
    }
}

impl StudyModelSnapshot {
    fn validate(&self) -> Result<(), StudyPolicyContractError> {
        // Exact bounds of the existing immutable model_config, not invented recommendations.
        if self.model_config_ref.is_nil() || self.identity.model_ref.is_nil()
            || self.identity.connection_version_ref.is_nil()
            || self.identity.model_id.trim().is_empty() || self.identity.model_id.contains('\0')
            || !(1024..=32768).contains(&self.input_token_limit)
            || !(128..=8192).contains(&self.output_token_limit)
            || !(1..=60).contains(&self.timeout_seconds)
        { return Err(StudyPolicyContractError::InvalidModelSnapshot); }
        Ok(())
    }
}

fn bounded_text(text: &str, minimum: usize, maximum: usize) -> bool {
    !text.contains('\0') && (minimum..=maximum).contains(&text.chars().count())
}

/// Fixed rules for the candidate method. Existing workers are switched only with the full v2 contract.
pub(super) fn base_instruction(stage: StudyStage) -> &'static str { templates::base_instruction(stage) }

/// Compiled Rust schema, checked against the approved contract fixtures in tests.
pub fn study_output_schema(stage: StudyStage) -> Result<Value, StudyPolicyContractError> {
    let schema = templates::output_schema(stage);
    if canonical_json_v1(&schema)?.len() > MAX_SCHEMA_BYTES {
        return Err(StudyPolicyContractError::OutputSchema);
    }
    Ok(schema)
}

/// Pure preparation only. This does NOT establish that a parent exists, belongs to the domain,
/// is recorded, or that a model connection is enabled. The transactional saver owns those checks.
pub fn compile_study_method(
    command: &CreateStudyPolicyCommand, model: &StudyModelSnapshot,
) -> Result<CompiledStudyMethod, StudyPolicyContractError> {
    command.validate()?;
    model.validate()?;
    if command.model_config_ref != model.model_config_ref {
        return Err(StudyPolicyContractError::InvalidModelSnapshot);
    }
    let stage = |kind, extra: &str| -> Result<StudyMethodStage, StudyPolicyContractError> {
        let instruction = templates::versioned_instruction(kind, extra);
        let schema = study_output_schema(kind)?;
        let mut item = StudyMethodStage { system_instruction: instruction, output_schema: schema,
            stage_hash: String::new() };
        item.stage_hash = stage_hash(kind, &item, model, CLEANER_VERSION, BUILDER_REVISION, SAFETY_REVISION)?;
        Ok(item)
    };
    let manifest = StudyMethodManifest {
        contract: METHOD_CONTRACT.into(), cleaner_version: CLEANER_VERSION.into(),
        builder_revision: BUILDER_REVISION.into(), safety_rules_revision: SAFETY_REVISION.into(),
        model_config_ref: model.model_config_ref, model_identity: model.identity.clone(),
        stages: StudyMethodStages {
            semantic: stage(StudyStage::Semantic, &command.stage_instructions.semantic)?,
            resolution: stage(StudyStage::Resolution, &command.stage_instructions.resolution)?,
            pair: stage(StudyStage::Pair, &command.stage_instructions.pair)?,
        },
    };
    let method_hash = json_hash(&serde_json::to_value(&manifest)
        .map_err(|_| StudyPolicyContractError::CanonicalJson)?)?;
    Ok(CompiledStudyMethod { manifest, method_hash })
}

/// Verify saved content, not today's regenerated prompt. A hash is an integrity checksum,
/// not authorization. Historic unknown methods must remain unknown instead of being rebuilt.
pub fn verify_study_method(
    method: &CompiledStudyMethod, model: &StudyModelSnapshot,
) -> Result<(), StudyPolicyContractError> {
    model.validate()?;
    let m = &method.manifest;
    if m.contract != METHOD_CONTRACT || m.builder_revision != BUILDER_REVISION
        || m.safety_rules_revision != SAFETY_REVISION || m.cleaner_version != CLEANER_VERSION
    { return Err(StudyPolicyContractError::UnsupportedRevision); }
    if m.model_config_ref != model.model_config_ref || m.model_identity != model.identity {
        return Err(StudyPolicyContractError::IntegrityMismatch);
    }
    for (kind, item) in [(StudyStage::Semantic, &m.stages.semantic),
        (StudyStage::Resolution, &m.stages.resolution), (StudyStage::Pair, &m.stages.pair)] {
        if item.system_instruction.is_empty() || item.system_instruction.contains('\0')
            || item.output_schema != study_output_schema(kind)?
            || item.stage_hash != stage_hash(kind, item, model, &m.cleaner_version,
                &m.builder_revision, &m.safety_rules_revision)?
        { return Err(StudyPolicyContractError::IntegrityMismatch); }
    }
    let actual = json_hash(&serde_json::to_value(m).map_err(|_| StudyPolicyContractError::CanonicalJson)?)?;
    if actual != method.method_hash { return Err(StudyPolicyContractError::IntegrityMismatch); }
    Ok(())
}

fn stage_hash(stage: StudyStage, item: &StudyMethodStage, model: &StudyModelSnapshot,
    cleaner: &str, builder: &str, safety: &str) -> Result<String, StudyPolicyContractError> {
    json_hash(&json!({
        "stage":stage,"systemInstruction":item.system_instruction,"outputSchema":item.output_schema,
        "cleanerVersion":cleaner,"builderRevision":builder,"safetyRulesRevision":safety,
        "modelConfigRef":model.model_config_ref,"modelIdentity":model.identity,
        "inputTokenLimit":model.input_token_limit,
        "parameters":{"operation":"analyze","maxOutputTokens":model.output_token_limit,
            "timeoutMs":i64::from(model.timeout_seconds) * 1000}
    }))
}

/// Object keys sorted by UTF-8 bytes; arrays and strings stay exact. Only integer numbers.
/// This implementation does not depend on serde_json's optional preserve_order feature.
pub fn canonical_json_v1(value: &Value) -> Result<Vec<u8>, StudyPolicyContractError> {
    fn write(value: &Value, out: &mut Vec<u8>, depth: usize) -> Result<(), StudyPolicyContractError> {
        if depth > 64 { return Err(StudyPolicyContractError::CanonicalJson); }
        match value {
            Value::Object(map) => {
                out.push(b'{');
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
                for (index, key) in keys.iter().enumerate() {
                    if index != 0 { out.push(b','); }
                    serde_json::to_writer(&mut *out, key).map_err(|_| StudyPolicyContractError::CanonicalJson)?;
                    out.push(b':'); write(&map[*key], out, depth + 1)?;
                }
                out.push(b'}');
            }
            Value::Array(items) => {
                out.push(b'[');
                for (index, item) in items.iter().enumerate() {
                    if index != 0 { out.push(b','); } write(item, out, depth + 1)?;
                }
                out.push(b']');
            }
            Value::Number(n) if !n.is_i64() && !n.is_u64() => return Err(StudyPolicyContractError::CanonicalJson),
            _ => serde_json::to_writer(out, value).map_err(|_| StudyPolicyContractError::CanonicalJson)?,
        }
        Ok(())
    }
    let mut bytes = Vec::new(); write(value, &mut bytes, 0)?; Ok(bytes)
}

pub fn json_hash(value: &Value) -> Result<String, StudyPolicyContractError> {
    Ok(Sha256::digest(canonical_json_v1(value)?).iter().map(|byte| format!("{byte:02x}")).collect())
}
