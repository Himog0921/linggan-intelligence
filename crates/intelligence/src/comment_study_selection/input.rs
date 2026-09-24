//! Build content equivalence separately from the exact immutable evidence references.
use super::{CommentKey, StudySelectionError};
use crate::comment_cleaning::{CLEANER_VERSION, clean};
use crate::comment_study_policy::json_hash;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// Contains research text: deliberately not Debug/Serialize and never placed in error messages.
pub struct PreparedStudyInput {
    pub research_text: String,
    pub research_sha256: String,
    pub raw_sha256: String,
    pub dependency_state: &'static str,
    pub parent_source_ref: Option<Uuid>,
    pub input_manifest: Value,
    pub input_hash: String,
    pub input_fingerprint: String,
}

fn hash_text(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn hash(value: &Value) -> Result<String, StudySelectionError> {
    json_hash(value).map_err(|_| StudySelectionError::InvalidSnapshot)
}

fn text(value: &Value) -> Result<&str, StudySelectionError> {
    value.as_str().filter(|v| !v.contains('\0')).ok_or(StudySelectionError::InvalidSnapshot)
}

fn reference(value: &Value) -> Result<Uuid, StudySelectionError> {
    Uuid::parse_str(text(value)?).ok().filter(|v| !v.is_nil()).ok_or(StudySelectionError::InvalidSnapshot)
}

fn retained_context(key: &CommentKey, manifest: &Value) -> Result<Value, StudySelectionError> {
    if manifest["contract"] != "comment-study.context.v1" || manifest["cleanerVersion"] != CLEANER_VERSION
        || reference(&manifest["workRef"])? != key.work_ref
    { return Err(StudySelectionError::InvalidSnapshot); }
    let budget = manifest["characterBudget"].as_u64().filter(|v| (1..=20000).contains(v))
        .ok_or(StudySelectionError::InvalidSnapshot)?;
    let sources = manifest["sources"].as_array().ok_or(StudySelectionError::InvalidSnapshot)?;
    let mut used = 0_u64;
    let mut retained = Vec::new();
    for (order, source) in sources.iter().enumerate() {
        let kind = text(&source["kind"])?;
        if !matches!(kind, "native_title" | "body" | "image_substantive_text" | "asr_text" | "frame_ocr_text") {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        reference(&source["sourceRef"])?;
        let content = text(&source["text"])?;
        let length = content.chars().count() as u64;
        if source["characterCount"].as_u64() != Some(length) { return Err(StudySelectionError::InvalidSnapshot); }
        used = used.checked_add(length).ok_or(StudySelectionError::InvalidSnapshot)?;
        if used > budget { return Err(StudySelectionError::InvalidSnapshot); }
        let ordinal = match &source["slotOrdinal"] {
            Value::Null => None,
            value => Some(value.as_i64().filter(|v| *v >= 0).ok_or(StudySelectionError::InvalidSnapshot)?),
        };
        retained.push(json!({"kind":kind,"order":order,"slotOrdinal":ordinal,"text":content}));
    }
    if manifest["includedCharacterCount"].as_u64() != Some(used) {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    Ok(Value::Array(retained))
}

fn parent_context(
    key: &CommentKey, parent: Option<&Value>,
) -> Result<(Value, Value, Option<Uuid>, &'static str), StudySelectionError> {
    let missing = |identity: Value| (json!({"state":"missing","commentKey":identity}),
        json!({"commentKey":identity,"rawSha256":null}), None, "parent_required_missing");
    let Some(parent) = parent else { return Ok(missing(Value::Null)); };
    let parent_key: CommentKey = serde_json::from_value(parent["commentKey"].clone())
        .map_err(|_| StudySelectionError::InvalidSnapshot)?;
    parent_key.validate()?;
    if parent_key.work_ref != key.work_ref || &parent_key == key {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let identity = json!(parent_key);
    match text(&parent["sourceState"])? {
        "unknown" | "restricted" => return Ok(missing(identity)),
        "known" => {},
        _ => return Err(StudySelectionError::InvalidSnapshot),
    }
    // A known but deterministically dropped/anomalous parent is not usable context.
    let Some(raw) = parent["commentText"].as_str() else { return Ok(missing(identity)); };
    if raw.contains('\0') || raw.chars().count() > 16000 { return Err(StudySelectionError::InvalidSnapshot); }
    let cleaned = clean(raw);
    if !matches!(cleaned.state.as_str(), "direct" | "context") { return Ok(missing(identity)); }
    let source = reference(&parent["sourceRef"])?;
    let raw_hash = hash_text(raw);
    let manifest = json!({"commentKey":identity,"sourceRef":source,
        "rawSha256":raw_hash,"researchText":cleaned.text});
    Ok((manifest, json!({"commentKey":identity,"rawSha256":raw_hash}), Some(source), "parent_available"))
}

/// source_ref/raw/parent/context must originate in one authorized freeze snapshot. This function
/// verifies shape/content, not DB ownership or current restrictions. The caller must not reread
/// different work/parent versions after selection. Already bounded context is never truncated here.
pub fn prepare_study_input(
    key: &CommentKey, source_ref: Uuid, raw: &str, work_context: &Value, parent: Option<&Value>,
) -> Result<PreparedStudyInput, StudySelectionError> {
    key.validate()?;
    if source_ref.is_nil() || raw.contains('\0') || raw.chars().count() > 16000 {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let cleaned = clean(raw);
    if !matches!(cleaned.state.as_str(), "direct" | "context") {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let context = retained_context(key, work_context)?;
    let (parent_manifest, parent_identity, parent_source_ref, dependency_state) = if cleaned.state == "direct" {
        (Value::Null, Value::Null, None, "self_contained")
    } else { parent_context(key, parent)? };
    let raw_sha256 = hash_text(raw);
    let research_sha256 = hash_text(&cleaned.text);
    let input_fingerprint = hash(&json!({"contract":"comment-study.input-fingerprint.v1",
        "commentKey":key,"rawSha256":raw_sha256,"researchSha256":research_sha256,
        "cleanerVersion":CLEANER_VERSION,"dependencyState":dependency_state,
        "workContext":context,"parentContext":parent_identity}))?;
    let input_manifest = json!({"contract":"comment-study.target-input.v2","targetSourceRef":source_ref,
        "workRef":key.work_ref,"workContextHash":hash(work_context)?,"parentContext":parent_manifest,
        "rawSha256":raw_sha256,"cleanerVersion":CLEANER_VERSION});
    let input_hash = hash(&input_manifest)?;
    Ok(PreparedStudyInput { research_text:cleaned.text, research_sha256, raw_sha256,
        dependency_state, parent_source_ref, input_manifest, input_hash, input_fingerprint })
}
