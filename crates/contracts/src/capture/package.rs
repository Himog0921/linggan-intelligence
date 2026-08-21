//! Top-level envelope of `capture.package.v1` and its single parse entry point.

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use super::{Coverage, KnownSetTarget, KnownTargetResult, Terminal, record::CaptureRecord};
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

/// Where a package claims to belong. Ingress never trusts these values on their own; it still
/// has to resolve and lock the matching routing rows and verify they belong together.
#[derive(Debug)]
pub struct PackageRouting {
    work_order_ref: Uuid,
    attempt_ref: Uuid,
    capture_identity: Uuid,
    lease_epoch: u32,
}

impl PackageRouting {
    pub fn work_order_ref(&self) -> Uuid {
        self.work_order_ref
    }

    pub fn attempt_ref(&self) -> Uuid {
        self.attempt_ref
    }

    pub fn capture_identity(&self) -> Uuid {
        self.capture_identity
    }

    pub fn lease_epoch(&self) -> u32 {
        self.lease_epoch
    }
}

/// A capture package whose canonical form, package hash and per-record hashes have been
/// verified. Verified shape is not acceptance: only ingress decides accepted/replay/conflict.
#[derive(Debug)]
pub struct CapturePackage {
    routing: PackageRouting,
    contract_version: String,
    package_hash: String,
    target: KnownSetTarget,
    terminal: Terminal,
    coverage: Coverage,
    known_target_results: Vec<KnownTargetResult>,
    records: Vec<CaptureRecord>,
}

impl CapturePackage {
    pub fn routing(&self) -> &PackageRouting {
        &self.routing
    }

    pub fn contract_version(&self) -> &str {
        &self.contract_version
    }

    pub fn package_hash(&self) -> &str {
        &self.package_hash
    }

    pub fn target(&self) -> &KnownSetTarget {
        &self.target
    }

    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    pub fn known_target_results(&self) -> &[KnownTargetResult] {
        &self.known_target_results
    }

    pub fn records(&self) -> &[CaptureRecord] {
        &self.records
    }
}

pub fn parse_capture_package(input: &str) -> Result<CapturePackage, ContractError> {
    let value = parse_canonical_value(input)?;
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

    verify_package_hash(&value, &wire.package_hash)?;
    verify_record_hashes(&value)?;

    Ok(CapturePackage {
        routing: PackageRouting {
            work_order_ref: wire.work_order_ref,
            attempt_ref: wire.attempt_ref,
            capture_identity: wire.capture_identity,
            lease_epoch: wire.lease_epoch,
        },
        contract_version: wire.contract_version,
        package_hash: wire.package_hash,
        target: wire.target,
        terminal: wire.terminal,
        coverage: wire.coverage,
        known_target_results: wire.known_target_results,
        records: wire.records,
    })
}

fn parse_canonical_value(input: &str) -> Result<Value, ContractError> {
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
    Ok(serde_json::from_str(input)?)
}

fn verify_package_hash(value: &Value, declared_package_hash: &str) -> Result<(), ContractError> {
    let mut hash_input = value.clone();
    hash_input
        .as_object_mut()
        .expect("validated capture package must be a JSON object")
        .remove("packageHash");
    let computed_package_hash = canonical_json_sha256(&hash_input)
        .map_err(|error| ContractError::PackageSchemaInvalid(error.to_string()))?;
    if declared_package_hash != computed_package_hash {
        return Err(ContractError::PackageHashInvalid);
    }
    Ok(())
}

fn verify_record_hashes(value: &Value) -> Result<(), ContractError> {
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
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapturePackageWire {
    schema_version: String,
    work_order_ref: Uuid,
    attempt_ref: Uuid,
    capture_identity: Uuid,
    lease_epoch: u32,
    contract_version: String,
    target: KnownSetTarget,
    terminal: Terminal,
    coverage: Coverage,
    known_target_results: Vec<KnownTargetResult>,
    records: Vec<CaptureRecord>,
    package_hash: String,
}
