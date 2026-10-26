//! Original image identity and deterministic digest for current OCR.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageIdentity {
    pub work_ref: uuid::Uuid,
    pub image_ref: String,
    pub blob_sha256: String,
    pub width: u32,
    pub height: u32,
}

/// Source metadata, never inferred from title-like text. FirstImage means known ordinal 1,
/// not the first available member of an incomplete or unordered set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRole {
    ExplicitCover,
    FirstImage,
    Interior,
    Unknown,
}

pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
