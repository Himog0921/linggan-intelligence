//! Offline validation of historical local OCR receipts.
use crate::ocr_adoption::{self as rule, *};
use serde::Deserialize;

mod validation;

pub const PROCESSOR_VERSION: &str = "local-vl-v1";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bridge {
    schema_version: u8,
    ok: bool,
    observation: ImageInput,
    anchor_raw_json: String,
    vl_raw_json: String,
}

pub struct PreparedOcrAdoption {
    pub observation: ImageInput,
    pub assessment: Assessment,
    pub raw_hash: String,
    pub raw_size: i64,
}

pub fn prepare(raw: &[u8]) -> Result<PreparedOcrAdoption, InputError> {
    if raw.is_empty() || raw.len() > 8_000_000 {
        return Err(InputError::Bounds);
    }
    let mut b: Bridge = serde_json::from_slice(raw).map_err(|_| InputError::Encoding)?;
    if b.schema_version != 2
        || !b.ok
        || !b.observation.layout.require_termination
        || rule::content_hash(b.anchor_raw_json.as_bytes())
            != b.observation.anchors.pipeline.raw_sha256
        || rule::content_hash(b.vl_raw_json.as_bytes()) != b.observation.layout.pipeline.raw_sha256
    {
        return Err(InputError::Source);
    }
    validation::validate_raw(&mut b)?;
    let assessment = rule::assess(&b.observation)?;
    Ok(PreparedOcrAdoption {
        observation: b.observation,
        assessment,
        raw_hash: rule::content_hash(raw),
        raw_size: raw.len() as i64,
    })
}
