//! Build content equivalence separately from the exact immutable evidence references.
use super::{CommentKey, StudySelectionError};
use crate::comment_cleaning::{CLEANER_VERSION, clean};
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
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn hash(value: &Value) -> Result<String, StudySelectionError> {
    json_hash(value).map_err(|_| StudySelectionError::InvalidSnapshot)
}

fn text(value: &Value) -> Result<&str, StudySelectionError> {
    value
        .as_str()
        .filter(|v| !v.contains('\0'))
        .ok_or(StudySelectionError::InvalidSnapshot)
}

fn reference(value: &Value) -> Result<Uuid, StudySelectionError> {
    Uuid::parse_str(text(value)?)
        .ok()
        .filter(|v| !v.is_nil())
        .ok_or(StudySelectionError::InvalidSnapshot)
}

fn retained_context(key: &CommentKey, manifest: &Value) -> Result<Value, StudySelectionError> {
    if [
        "characterBudget",
        "truncated",
        "omitted",
        "omittedFragmentCount",
    ]
    .iter()
    .any(|key| manifest.get(key).is_some())
        || manifest["contract"] != "comment-study.context.v2"
        || manifest["cleanerVersion"] != CLEANER_VERSION
        || reference(&manifest["workRef"])? != key.work_ref
    {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let sources = manifest["sources"]
        .as_array()
        .ok_or(StudySelectionError::InvalidSnapshot)?;
    let mut used = 0_u64;
    let mut retained = Vec::new();
    for (order, source) in sources.iter().enumerate() {
        let kind = text(&source["kind"])?;
        if !matches!(
            kind,
            "native_title"
                | "body"
                | "tags"
                | "image_substantive_text"
                | "asr_text"
                | "frame_ocr_text"
        ) {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        reference(&source["sourceRef"])?;
        if text(&source["sourceVersion"])?.is_empty()
            || text(&source["blockRef"])?.is_empty()
            || text(&source["asOf"])?.is_empty()
            || !matches!(
                text(&source["speaker"])?,
                "author" | "other" | "unconfirmed"
            )
            || !source["structure"].is_object()
            || !source["issues"].is_array()
        {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        for key in ["readingRef", "unitRef"] {
            if !source[key].is_null() {
                reference(&source[key])?;
            }
        }
        let content = text(&source["text"])?;
        linggan_evidence::research_dossier::content::validate_blocks(std::slice::from_ref(
            &source["structure"],
        ))
        .map_err(|_| StudySelectionError::InvalidSnapshot)?;
        linggan_evidence::research_dossier::content::validate_issues(&source["issues"])
            .map_err(|_| StudySelectionError::InvalidSnapshot)?;
        if linggan_evidence::research_dossier::content::block_text(&source["structure"]) != content
            || source["structure"]["speaker"]
                .as_str()
                .unwrap_or("unconfirmed")
                != source["speaker"].as_str().unwrap_or("")
        {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        let length = content.chars().count() as u64;
        if source["characterCount"].as_u64() != Some(length) {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        used = used
            .checked_add(length)
            .ok_or(StudySelectionError::InvalidSnapshot)?;
        let ordinal = match &source["slotOrdinal"] {
            Value::Null => None,
            value => Some(
                value
                    .as_i64()
                    .filter(|v| *v >= 0)
                    .ok_or(StudySelectionError::InvalidSnapshot)?,
            ),
        };
        retained.push(json!({"kind":kind,"order":order,"slotOrdinal":ordinal,"text":content, "speaker":source["speaker"],"structure":source["structure"],"issues":source["issues"]}));
    }
    if manifest["includedCharacterCount"].as_u64() != Some(used) {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    Ok(Value::Array(retained))
}

fn parent_context(
    key: &CommentKey,
    parent: Option<&Value>,
) -> Result<(Value, Value, Option<Uuid>, &'static str), StudySelectionError> {
    let missing = |identity: Value| {
        (
            json!({"state":"missing","commentKey":identity}),
            json!({"commentKey":identity,"rawSha256":null}),
            None,
            "parent_required_missing",
        )
    };
    // Absence of a parent relationship means this is a root comment. Its work context is still
    // available to the semantic stage; do not turn a short root comment into a missing-parent
    // failure. A reply whose parent could not be read is represented by Some(parent) with an
    // unknown/restricted source state below. The model decides whether that missing context
    // actually prevents understanding this reply.
    let Some(parent) = parent else {
        return Ok((Value::Null, Value::Null, None, "self_contained"));
    };
    let parent_key: CommentKey = serde_json::from_value(parent["commentKey"].clone())
        .map_err(|_| StudySelectionError::InvalidSnapshot)?;
    parent_key.validate()?;
    if parent_key.work_ref != key.work_ref || &parent_key == key {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let identity = json!(parent_key);
    match text(&parent["sourceState"])? {
        "unknown" | "restricted" => return Ok(missing(identity)),
        "known" => {}
        _ => return Err(StudySelectionError::InvalidSnapshot),
    }
    // A known but deterministically dropped/anomalous parent is not usable context.
    let Some(raw) = parent["commentText"].as_str() else {
        return Ok(missing(identity));
    };
    if raw.contains('\0') || raw.chars().count() > 16000 {
        return Err(StudySelectionError::InvalidSnapshot);
    }
    let cleaned = clean(raw);
    if !matches!(cleaned.state.as_str(), "direct" | "context") {
        return Ok(missing(identity));
    }
    let source = reference(&parent["sourceRef"])?;
    let raw_hash = hash_text(raw);
    let manifest = json!({"commentKey":identity,"sourceRef":source,
        "rawSha256":raw_hash,"researchText":cleaned.text});
    Ok((
        manifest,
        json!({"commentKey":identity,"rawSha256":raw_hash}),
        Some(source),
        "parent_available",
    ))
}

/// source_ref/raw/parent/context must originate in one authorized freeze snapshot. This function
/// verifies shape/content, not DB ownership or current restrictions. The caller must not reread
/// different work/parent versions after selection. Full note context is never truncated here.
pub fn prepare_study_input(
    key: &CommentKey,
    source_ref: Uuid,
    raw: &str,
    work_context: &Value,
    parent: Option<&Value>,
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
    // Parent context follows the actual reply relationship, not the cleaner's text-length
    // classification. It may help resolve a direct comment's pronouns without ever becoming
    // evidence for a Signal.
    let (parent_manifest, parent_identity, parent_source_ref, dependency_state) =
        parent_context(key, parent)?;
    let raw_sha256 = hash_text(raw);
    let research_sha256 = hash_text(&cleaned.text);
    let input_fingerprint = hash(&json!({"contract":"comment-study.input-fingerprint.v3",
        "commentKey":key,"rawSha256":raw_sha256,"researchSha256":research_sha256,
        "cleanerVersion":CLEANER_VERSION,"dependencyState":dependency_state,
        "workContext":context,"workCoverage":work_context["coverage"],"parentContext":parent_identity}))?;
    let input_manifest = json!({"contract":"comment-study.target-input.v2","targetSourceRef":source_ref,
        "workRef":key.work_ref,"workContextHash":hash(work_context)?,"parentContext":parent_manifest,
        "rawSha256":raw_sha256,"cleanerVersion":CLEANER_VERSION});
    let input_hash = hash(&input_manifest)?;
    Ok(PreparedStudyInput {
        research_text: cleaned.text,
        research_sha256,
        raw_sha256,
        dependency_state,
        parent_source_ref,
        input_manifest,
        input_hash,
        input_fingerprint,
    })
}

pub fn canonical_json_v2(value: &Value) -> Result<Vec<u8>, StudySelectionError> {
    fn write(value: &Value, out: &mut Vec<u8>, depth: usize) -> Result<(), StudySelectionError> {
        if depth > 64 {
            return Err(StudySelectionError::InvalidSnapshot);
        }
        match value {
            Value::Object(map) => {
                out.push(b'{');
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
                for (index, key) in keys.iter().enumerate() {
                    if index != 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut *out, key)
                        .map_err(|_| StudySelectionError::InvalidSnapshot)?;
                    out.push(b':');
                    write(&map[*key], out, depth + 1)?;
                }
                out.push(b'}');
            }
            Value::Array(items) => {
                out.push(b'[');
                for (index, item) in items.iter().enumerate() {
                    if index != 0 {
                        out.push(b',');
                    }
                    write(item, out, depth + 1)?;
                }
                out.push(b']');
            }
            Value::Number(n) if n.as_f64().is_none_or(|v| !v.is_finite()) => {
                return Err(StudySelectionError::InvalidSnapshot);
            }
            _ => serde_json::to_writer(out, value)
                .map_err(|_| StudySelectionError::InvalidSnapshot)?,
        }
        Ok(())
    }
    let mut bytes = Vec::new();
    write(value, &mut bytes, 0)?;
    Ok(bytes)
}

pub fn json_hash(value: &Value) -> Result<String, StudySelectionError> {
    Ok(Sha256::digest(canonical_json_v2(value)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
