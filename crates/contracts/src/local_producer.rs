//! LOCAL-TRUSTED's narrow browser-producer boundary.
//!
//! A task specification only describes a bounded browser action. It deliberately contains no
//! Domain, Topic, Claim, Observation, scheduler decision, or authority language.

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

const TASK_SPEC_VERSION: &str = "linggan.task-spec.v1";
const ATTEMPT_VERSION: &str = "linggan.local-trusted.attempt.v1";
const SUBMISSION_VERSION: &str = "linggan.local-trusted.submission.v1";

#[derive(Debug, Error)]
pub enum LocalProducerContractError {
    #[error("local producer contract schema is invalid: {0}")]
    SchemaInvalid(String),
    #[error("local producer contract version is unsupported: {0}")]
    UnsupportedContractVersion(String),
    #[error("LOCAL-TRUSTED only accepts the fixed manual visible-card TaskSpec")]
    UnsupportedTaskSpec,
    #[error("a local producer identity must be a UUID")]
    InvalidProducerIdentity,
    #[error("the local producer identifiers must be UUIDs")]
    InvalidIdentifiers,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalTaskSpecWire {
    contract_version: String,
    task_id: String,
    source: String,
    platform: String,
    page_type: String,
    target: String,
    capabilities_requested: Vec<String>,
    maximum_quota: u16,
    comment_limit: String,
    acquire_media: String,
    risk_policy: String,
    stop_conditions: Vec<String>,
}

/// A flat, executable task description. The testing slice intentionally supports one manual
/// visible-card task only; `scheduler` is kept out of the accepted set rather than simulated.
#[derive(Debug)]
pub struct LocalTaskSpec {
    task_id: Uuid,
    raw: Value,
}

impl LocalTaskSpec {
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalProducerAttemptWire {
    contract_version: String,
    producer_instance_id: String,
    task_id: String,
    attempt_id: String,
}

#[derive(Debug)]
pub struct LocalProducerAttempt {
    producer_instance_id: Uuid,
    task_id: Uuid,
    attempt_id: Uuid,
}

impl LocalProducerAttempt {
    pub fn producer_instance_id(&self) -> Uuid {
        self.producer_instance_id
    }
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }
    pub fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LocalProducerSubmissionWire {
    contract_version: String,
    producer_instance_id: String,
    task_id: String,
    attempt_id: String,
    submission_id: String,
    discovery_package: Value,
}

#[derive(Debug)]
pub struct LocalProducerSubmission {
    producer_instance_id: Uuid,
    task_id: Uuid,
    attempt_id: Uuid,
    submission_id: Uuid,
    discovery_package: Value,
}

impl LocalProducerSubmission {
    pub fn producer_instance_id(&self) -> Uuid {
        self.producer_instance_id
    }
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }
    pub fn attempt_id(&self) -> Uuid {
        self.attempt_id
    }
    pub fn submission_id(&self) -> Uuid {
        self.submission_id
    }
    pub fn discovery_package(&self) -> &Value {
        &self.discovery_package
    }
}

pub fn parse_local_task_spec(input: &str) -> Result<LocalTaskSpec, LocalProducerContractError> {
    let raw: Value = serde_json::from_str(input)
        .map_err(|error| LocalProducerContractError::SchemaInvalid(error.to_string()))?;
    let wire: LocalTaskSpecWire = serde_json::from_value(raw.clone())
        .map_err(|error| LocalProducerContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != TASK_SPEC_VERSION {
        return Err(LocalProducerContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    let task_id = Uuid::parse_str(&wire.task_id)
        .map_err(|_| LocalProducerContractError::InvalidIdentifiers)?;
    if wire.source != "manual"
        || wire.platform != "xhs"
        || wire.page_type != "search_results"
        || wire.target != "current_visible_search_surface"
        || wire.capabilities_requested != ["discover_visible_cards"]
        || wire.maximum_quota != 20
        || wire.comment_limit != "not_requested"
        || wire.acquire_media != "not_requested"
        || wire.risk_policy != "local_trusted_user_initiated"
        || wire.stop_conditions != ["current_surface_read_once", "maximum_quota"]
    {
        return Err(LocalProducerContractError::UnsupportedTaskSpec);
    }
    Ok(LocalTaskSpec { task_id, raw })
}

pub fn parse_local_producer_attempt(
    input: &str,
) -> Result<LocalProducerAttempt, LocalProducerContractError> {
    let wire: LocalProducerAttemptWire = serde_json::from_str(input)
        .map_err(|error| LocalProducerContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != ATTEMPT_VERSION {
        return Err(LocalProducerContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    parse_attempt_identifiers(&wire.producer_instance_id, &wire.task_id, &wire.attempt_id)
}

pub fn parse_local_producer_submission(
    input: &str,
) -> Result<LocalProducerSubmission, LocalProducerContractError> {
    let wire: LocalProducerSubmissionWire = serde_json::from_str(input)
        .map_err(|error| LocalProducerContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != SUBMISSION_VERSION {
        return Err(LocalProducerContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    let attempt =
        parse_attempt_identifiers(&wire.producer_instance_id, &wire.task_id, &wire.attempt_id)?;
    let submission_id = Uuid::parse_str(&wire.submission_id)
        .map_err(|_| LocalProducerContractError::InvalidIdentifiers)?;
    Ok(LocalProducerSubmission {
        producer_instance_id: attempt.producer_instance_id,
        task_id: attempt.task_id,
        attempt_id: attempt.attempt_id,
        submission_id,
        discovery_package: wire.discovery_package,
    })
}

fn parse_attempt_identifiers(
    producer_instance_id: &str,
    task_id: &str,
    attempt_id: &str,
) -> Result<LocalProducerAttempt, LocalProducerContractError> {
    let producer_instance_id = Uuid::parse_str(producer_instance_id)
        .map_err(|_| LocalProducerContractError::InvalidProducerIdentity)?;
    let task_id =
        Uuid::parse_str(task_id).map_err(|_| LocalProducerContractError::InvalidIdentifiers)?;
    let attempt_id =
        Uuid::parse_str(attempt_id).map_err(|_| LocalProducerContractError::InvalidIdentifiers)?;
    Ok(LocalProducerAttempt {
        producer_instance_id,
        task_id,
        attempt_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_manual_task_spec_is_accepted_but_scheduler_is_not_simulated() {
        let accepted = parse_local_task_spec(
            r#"{"contractVersion":"linggan.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"manual","platform":"xhs","pageType":"search_results","target":"current_visible_search_surface","capabilitiesRequested":["discover_visible_cards"],"maximumQuota":20,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["current_surface_read_once","maximum_quota"]}"#,
        );
        assert!(accepted.is_ok());
        let scheduler = parse_local_task_spec(
            r#"{"contractVersion":"linggan.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"scheduler","platform":"xhs","pageType":"search_results","target":"current_visible_search_surface","capabilitiesRequested":["discover_visible_cards"],"maximumQuota":20,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["current_surface_read_once","maximum_quota"]}"#,
        );
        assert!(matches!(
            scheduler,
            Err(LocalProducerContractError::UnsupportedTaskSpec)
        ));
    }
}
