use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::canonical::{
    JsonIngressError, canonical_json_sha256, first_unsafe_integer_token, reject_duplicate_keys,
    sanitized_surrogate_input,
};

const PACKAGE_SCHEMA_VERSION: &str = "capture.package.v1";
const CONTENT_CONTRACT_VERSION: &str = "content-detail.synthetic.v1";

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("capture package is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("capture package cannot be canonicalized: {0}")]
    CanonicalizationInvalid(String),
    #[error("capture package schema is invalid: {0}")]
    PackageSchemaInvalid(String),
    #[error("capture package hash does not match its canonical content")]
    PackageHashInvalid,
    #[error("capture record hash does not match its canonical content")]
    RecordHashInvalid,
    #[error("unsupported capture package schema: {0}")]
    UnsupportedPackageSchema(String),
    #[error("unsupported content contract: {0}")]
    UnsupportedContentContract(String),
}

#[derive(Debug)]
pub struct CapturePackage {
    contract_version: String,
    target: KnownSetTarget,
    terminal: Terminal,
    coverage: Coverage,
    record_count: usize,
}

impl CapturePackage {
    pub fn contract_version(&self) -> &str {
        &self.contract_version
    }

    pub fn known_target_count(&self) -> u32 {
        self.target.known_target_count
    }

    pub fn record_count(&self) -> usize {
        self.record_count
    }

    pub fn coverage_counts(&self) -> (u32, u32, u32, Option<u32>) {
        (
            self.coverage.attempted,
            self.coverage.emitted,
            self.coverage.failed,
            self.coverage.known_not_attempted,
        )
    }

    pub fn terminal_reason(&self) -> &str {
        &self.terminal.reason
    }
}

pub fn parse_capture_package(input: &str) -> Result<CapturePackage, ContractError> {
    let sanitized_surrogate_input = sanitized_surrogate_input(input);
    let syntax_input = sanitized_surrogate_input.as_deref().unwrap_or(input);
    match reject_duplicate_keys(syntax_input) {
        Err(JsonIngressError::InvalidJson(error)) => return Err(ContractError::InvalidJson(error)),
        Err(JsonIngressError::DuplicateKey(key)) => {
            return Err(ContractError::CanonicalizationInvalid(format!(
                "duplicate JSON object key: {key}"
            )));
        }
        Ok(()) => {}
    }
    if let Some(token) = first_unsafe_integer_token(input) {
        return Err(ContractError::CanonicalizationInvalid(format!(
            "unsafe JSON integer: {token}"
        )));
    }
    if sanitized_surrogate_input.is_some() {
        return Err(ContractError::CanonicalizationInvalid(
            "unpaired Unicode surrogate escape".to_owned(),
        ));
    }
    let value: Value = serde_json::from_str(input)?;
    let wire: CapturePackageWire = serde_json::from_value(value.clone())
        .map_err(|error| ContractError::PackageSchemaInvalid(error.to_string()))?;

    if wire.schema_version != PACKAGE_SCHEMA_VERSION {
        return Err(ContractError::UnsupportedPackageSchema(wire.schema_version));
    }
    if wire.contract_version != CONTENT_CONTRACT_VERSION {
        return Err(ContractError::UnsupportedContentContract(
            wire.contract_version,
        ));
    }

    let mut hash_input = value.clone();
    hash_input
        .as_object_mut()
        .expect("validated capture package must be a JSON object")
        .remove("packageHash");
    let computed_package_hash = canonical_json_sha256(&hash_input)
        .map_err(|error| ContractError::PackageSchemaInvalid(error.to_string()))?;
    if wire.package_hash != computed_package_hash {
        return Err(ContractError::PackageHashInvalid);
    }

    for record in value["records"]
        .as_array()
        .expect("validated capture package records must be an array")
    {
        let literal_record_hash = record
            .get("recordHash")
            .expect("validated capture package records must contain recordHash");
        let mut record_hash_input = record.clone();
        record_hash_input
            .as_object_mut()
            .expect("validated capture record must be a JSON object")
            .remove("recordHash");
        let computed_record_hash = canonical_json_sha256(&record_hash_input)
            .map_err(|error| ContractError::PackageSchemaInvalid(error.to_string()))?;

        if literal_record_hash != &Value::String(computed_record_hash) {
            return Err(ContractError::RecordHashInvalid);
        }
    }

    Ok(CapturePackage {
        contract_version: wire.contract_version,
        target: wire.target,
        terminal: wire.terminal,
        coverage: wire.coverage,
        record_count: wire.records.len(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapturePackageWire {
    schema_version: String,
    #[serde(rename = "workOrderRef")]
    _work_order_ref: String,
    #[serde(rename = "attemptRef")]
    _attempt_ref: String,
    #[serde(rename = "captureIdentity")]
    _capture_identity: String,
    #[serde(rename = "leaseEpoch")]
    _lease_epoch: u32,
    contract_version: String,
    target: KnownSetTarget,
    terminal: Terminal,
    coverage: Coverage,
    #[serde(rename = "knownTargetResults")]
    _known_target_results: Vec<Value>,
    records: Vec<RecordWire>,
    #[serde(rename = "packageHash")]
    package_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordWire {
    #[serde(rename = "ordinal")]
    _ordinal: Value,
    #[serde(rename = "recordKind")]
    _record_kind: Value,
    #[serde(rename = "targetExternalId")]
    _target_external_id: Value,
    #[serde(rename = "source")]
    _source: Value,
    #[serde(rename = "observedAt")]
    _observed_at: Value,
    #[serde(rename = "payload")]
    _payload: Value,
    #[serde(rename = "recordHash")]
    _record_hash: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KnownSetTarget {
    #[serde(rename = "basis")]
    _basis: KnownSetBasis,
    #[serde(rename = "unit")]
    _unit: ContentDetailUnit,
    #[serde(rename = "targetManifestHash")]
    _target_manifest_hash: String,
    known_target_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnownSetBasis {
    KnownSet,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContentDetailUnit {
    ContentDetail,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Terminal {
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Coverage {
    #[serde(rename = "unit")]
    _unit: ContentDetailUnit,
    attempted: u32,
    emitted: u32,
    failed: u32,
    known_not_attempted: Option<u32>,
    #[serde(rename = "remainingScope")]
    _remaining_scope: String,
}
