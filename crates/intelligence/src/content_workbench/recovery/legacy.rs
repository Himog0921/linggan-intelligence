//! Explicit pre-block citation DTO, used only by the operator recovery command.
//! Field order is the historical e38a4398 serialization order, including nested aliases.
use super::super::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LegacyCitation {
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub field: SourceField,
    pub source_version: String,
    pub start: usize,
    pub end: usize,
    pub text_hash: String,
    // This bounded recovery supports native note/comment citations only, never legacy OCR.
    pub location: Option<serde_json::Value>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LegacyExcerpt {
    pub id: Uuid,
    pub citation: LegacyCitation,
    pub selected_text: Option<String>,
    pub text_hash: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LegacyDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<MaterialSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_preparation: Option<MaterialPreparation>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub contract: String,
    pub title: String,
    pub account_ref: Option<Uuid>,
    pub topic_ref: Option<Uuid>,
    pub angle: String,
    pub outline: Vec<String>,
    pub paragraphs: Vec<Paragraph>,
    pub excerpts: Vec<LegacyExcerpt>,
    pub canvas: Vec<CanvasBlock>,
    pub pages: Vec<ImagePage>,
    pub uses: Vec<MaterialUse>,
}

pub(super) fn decode(value: serde_json::Value) -> Result<LegacyDocument, WorkbenchError> {
    let doc: LegacyDocument = serde_json::from_value(value)?;
    if doc.contract != "content-workbench.v1" || doc.excerpts.is_empty() || doc.excerpts.len() > 200
    {
        return Err(WorkbenchError::Invalid("legacy_document"));
    }
    Ok(doc)
}

/// Ignore only the identities explicitly requalified by this operation, never authored content.
pub(super) fn authored_digest(mut value: serde_json::Value) -> Result<String, WorkbenchError> {
    if let Some(items) = value["excerpts"].as_array_mut() {
        for item in items {
            if let Some(c) = item["citation"].as_object_mut() {
                c.remove("asOf");
                c.remove("blockRef");
                c.remove("sourceVersion");
            }
        }
    }
    if let Some(items) = value["materials"].as_array_mut() {
        for item in items {
            if let Some(m) = item.as_object_mut() {
                m.remove("sourceVersion");
            }
        }
    }
    Ok(hash(serde_json::to_vec(&value)?))
}
