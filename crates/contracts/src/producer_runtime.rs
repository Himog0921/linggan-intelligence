//! Versioned, platform-neutral Browser Producer Runtime contracts.
//!
//! This boundary deliberately describes *execution* only.  A producer receives a bounded
//! `TaskSpec` and returns one immutable `CapturePackage` per attempt.  It has no Domain,
//! Topic, Claim, research, or market-intelligence vocabulary.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

pub const PRODUCER_TASK_SPEC_VERSION: &str = "linggan.producer.task-spec.v1";
pub const PRODUCER_ATTEMPT_VERSION: &str = "linggan.producer.attempt.v1";
pub const CAPTURE_PACKAGE_VERSION: &str = "linggan.producer.capture-package.v1";

const PLATFORM_VALUES: &[&str] = &["xhs", "douyin"];
const SOURCE_VALUES: &[&str] = &["manual", "scheduled"];
const PACKAGE_KINDS: &[&str] = &[
    "discovery_search",
    "profile_discovery",
    "content_detail",
    "comments",
    "replies",
    "author_profile",
    "media_slots",
    "media_bytes",
    "batch_checkpoint",
];
const STOP_CONDITIONS: &[&str] = &[
    "manual_stop",
    "maximum_quota",
    "current_surface_read_once",
    "surface_ended",
    "time_budget",
    "risk_budget",
    "detail_read_complete",
    "collector_complete",
];

#[derive(Debug, Error)]
pub enum ProducerRuntimeContractError {
    #[error("producer runtime contract schema is invalid: {0}")]
    SchemaInvalid(String),
    #[error("producer runtime contract version is unsupported: {0}")]
    UnsupportedContractVersion(String),
    #[error("producer runtime contains invalid identifiers")]
    InvalidIdentifiers,
    #[error("producer runtime contains an unsupported enum value")]
    UnsupportedValue,
    #[error("producer runtime contains a NUL character unsupported by storage")]
    UnsupportedNullCharacter,
    #[error("producer runtime task spec is not bounded")]
    UnboundedTask,
    #[error("producer runtime coverage is invalid")]
    InvalidCoverage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskSpecWire {
    contract_version: String,
    task_id: String,
    source: String,
    platform: String,
    page_type: String,
    target: Value,
    capabilities_requested: Vec<String>,
    maximum_quota: Option<u32>,
    /// **这一轮该拿回多少**——完工判据唯一读的那个数（见 `surface_scan_complete_sql`）。
    ///
    /// 与 `maximum_quota` 分开是因为它们本来就不是一件事：后者是授权上限、也是搜索结果页的
    /// 加载预算；前者是这一轮按规则实际要的份数。关键词巡查上两者不同（加载预算 200、按
    /// 口径取赞前 20），把它压成一个数，是那一轮「按规则停对了却永远不算成功」的原因。
    ///
    /// 可选：旧任务说明书没有这一项，照常解析——它们按判据不能被记成完成，而不是被
    /// 记成完成（宁可漏记一次，不可误记一次）。
    expected_count: Option<u32>,
    comment_limit: Value,
    acquire_media: Value,
    risk_policy: String,
    stop_conditions: Vec<String>,
}

/// Flat, bounded execution instruction.  `scheduled` is valid at the protocol boundary but a
/// scheduler is not implied by accepting the contract.
#[derive(Debug, Clone)]
pub struct ProducerTaskSpec {
    task_id: Uuid,
    source: String,
    platform: String,
    page_type: String,
    raw: Value,
}

impl ProducerTaskSpec {
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn platform(&self) -> &str {
        &self.platform
    }
    pub fn page_type(&self) -> &str {
        &self.page_type
    }
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AttemptWire {
    contract_version: String,
    producer_instance_id: String,
    task_id: String,
    attempt_id: String,
}

#[derive(Debug, Clone)]
pub struct ProducerAttempt {
    producer_instance_id: Uuid,
    task_id: Uuid,
    attempt_id: Uuid,
}

impl ProducerAttempt {
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
struct CapturePackageWire {
    contract_version: String,
    package_ref: String,
    package_kind: String,
    platform: String,
    observed_at: String,
    captured_at: String,
    coverage: CoverageWire,
    records: Vec<Value>,
    checkpoint: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageWire {
    target: Value,
    layers: Vec<CoverageLayerWire>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageLayerWire {
    capability: String,
    observed: u32,
    attempted: u32,
    acquired: u32,
    verified: u32,
    failed: u32,
    not_attempted: u32,
    unknown: u32,
    stopped_reason: String,
}

/// Immutable producer result.  `records` remain opaque to this layer: each downstream evidence
/// admission is allowed to validate its own record shape without turning a package into a generic
/// fact container.
#[derive(Debug, Clone)]
pub struct ProducerCapturePackage {
    package_ref: Uuid,
    package_kind: String,
    platform: String,
    observed_at: String,
    captured_at: String,
    coverage: Value,
    records: Vec<Value>,
    checkpoint: Option<Value>,
    raw: Value,
}

impl ProducerCapturePackage {
    pub fn package_ref(&self) -> Uuid {
        self.package_ref
    }
    pub fn package_kind(&self) -> &str {
        &self.package_kind
    }
    pub fn platform(&self) -> &str {
        &self.platform
    }
    pub fn observed_at(&self) -> &str {
        &self.observed_at
    }
    pub fn captured_at(&self) -> &str {
        &self.captured_at
    }
    pub fn coverage(&self) -> &Value {
        &self.coverage
    }
    pub fn records(&self) -> &[Value] {
        &self.records
    }
    pub fn checkpoint(&self) -> Option<&Value> {
        self.checkpoint.as_ref()
    }
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SubmissionWire {
    contract_version: String,
    producer_instance_id: String,
    task_id: String,
    attempt_id: String,
    submission_id: String,
    capture_package: Value,
}

#[derive(Debug, Clone)]
pub struct ProducerSubmission {
    producer_instance_id: Uuid,
    task_id: Uuid,
    attempt_id: Uuid,
    submission_id: Uuid,
    capture_package: ProducerCapturePackage,
}

impl ProducerSubmission {
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
    pub fn capture_package(&self) -> &ProducerCapturePackage {
        &self.capture_package
    }
}

pub fn parse_producer_task_spec(
    input: &str,
) -> Result<ProducerTaskSpec, ProducerRuntimeContractError> {
    let raw: Value = serde_json::from_str(input)
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    let wire: TaskSpecWire = serde_json::from_value(raw.clone())
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != PRODUCER_TASK_SPEC_VERSION {
        return Err(ProducerRuntimeContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    let task_id = parse_uuid(&wire.task_id)?;
    if !SOURCE_VALUES.contains(&wire.source.as_str())
        || !PLATFORM_VALUES.contains(&wire.platform.as_str())
        || wire.page_type.trim().is_empty()
        || !wire.target.is_object()
        || wire.capabilities_requested.is_empty()
        || wire
            .capabilities_requested
            .iter()
            .any(|value| !PACKAGE_KINDS.contains(&value.as_str()))
        || wire.comment_limit.is_null()
        || wire.acquire_media.is_null()
        || !risk_policy_matches_source(&wire.source, &wire.risk_policy)
        || wire.stop_conditions.is_empty()
        || wire
            .stop_conditions
            .iter()
            .any(|value| !STOP_CONDITIONS.contains(&value.as_str()))
    {
        return Err(ProducerRuntimeContractError::UnsupportedValue);
    }
    if wire.capabilities_requested.len() != 1
        || !valid_target_for_capability(&wire.target, &wire.capabilities_requested[0])
        || !valid_comment_limit(&wire.comment_limit)
        || !valid_acquire_media(&wire.acquire_media)
    {
        return Err(ProducerRuntimeContractError::UnsupportedValue);
    }
    if wire.maximum_quota.is_none()
        && !wire.stop_conditions.iter().any(|value| {
            value == "surface_ended"
                || value == "manual_stop"
                || value == "time_budget"
                || value == "risk_budget"
        })
    {
        return Err(ProducerRuntimeContractError::UnboundedTask);
    }
    // 预期份数是一个**判据要用的承诺**，不能自己站不住：至少 1，且不超过授权上限——超过
    // 上限的期待是拿不到的数，那样写出来的任务永远不可能被记成完成，而表面上一切正常。
    if let Some(expected_count) = wire.expected_count {
        if expected_count == 0
            || wire
                .maximum_quota
                .is_some_and(|maximum_quota| expected_count > maximum_quota)
        {
            return Err(ProducerRuntimeContractError::UnsupportedValue);
        }
    }
    Ok(ProducerTaskSpec {
        task_id,
        source: wire.source,
        platform: wire.platform,
        page_type: wire.page_type,
        raw,
    })
}

pub fn parse_producer_attempt(
    input: &str,
) -> Result<ProducerAttempt, ProducerRuntimeContractError> {
    let wire: AttemptWire = serde_json::from_str(input)
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != PRODUCER_ATTEMPT_VERSION {
        return Err(ProducerRuntimeContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    Ok(ProducerAttempt {
        producer_instance_id: parse_uuid(&wire.producer_instance_id)?,
        task_id: parse_uuid(&wire.task_id)?,
        attempt_id: parse_uuid(&wire.attempt_id)?,
    })
}

pub fn parse_producer_capture_package(
    input: &str,
) -> Result<ProducerCapturePackage, ProducerRuntimeContractError> {
    let raw: Value = serde_json::from_str(input)
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    parse_producer_capture_value(raw)
}

pub fn parse_producer_submission(
    input: &str,
) -> Result<ProducerSubmission, ProducerRuntimeContractError> {
    let wire: SubmissionWire = serde_json::from_str(input)
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != CAPTURE_PACKAGE_VERSION {
        return Err(ProducerRuntimeContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    let capture_package = parse_producer_capture_value(wire.capture_package)?;
    Ok(ProducerSubmission {
        producer_instance_id: parse_uuid(&wire.producer_instance_id)?,
        task_id: parse_uuid(&wire.task_id)?,
        attempt_id: parse_uuid(&wire.attempt_id)?,
        submission_id: parse_uuid(&wire.submission_id)?,
        capture_package,
    })
}

// Reject a deterministic representation failure before it becomes a retryable DB outage.
// Keys matter too: PostgreSQL jsonb rejects U+0000 anywhere in a JSON string.
fn validate_storage_text(value: &Value) -> Result<(), ProducerRuntimeContractError> {
    match value {
        Value::String(text) if text.contains('\0') => {
            return Err(ProducerRuntimeContractError::UnsupportedNullCharacter);
        }
        Value::Array(values) => {
            for value in values {
                validate_storage_text(value)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key.contains('\0') {
                    return Err(ProducerRuntimeContractError::UnsupportedNullCharacter);
                }
                validate_storage_text(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_producer_capture_value(
    raw: Value,
) -> Result<ProducerCapturePackage, ProducerRuntimeContractError> {
    validate_storage_text(&raw)?;
    let wire: CapturePackageWire = serde_json::from_value(raw.clone())
        .map_err(|error| ProducerRuntimeContractError::SchemaInvalid(error.to_string()))?;
    if wire.contract_version != CAPTURE_PACKAGE_VERSION {
        return Err(ProducerRuntimeContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    if !PACKAGE_KINDS.contains(&wire.package_kind.as_str())
        || !PLATFORM_VALUES.contains(&wire.platform.as_str())
        || wire.records.len() > 2048
        || !is_timestamp(&wire.observed_at)
        || !is_timestamp(&wire.captured_at)
    {
        return Err(ProducerRuntimeContractError::UnsupportedValue);
    }
    validate_coverage(&wire.coverage, &wire.package_kind)?;
    Ok(ProducerCapturePackage {
        package_ref: parse_uuid(&wire.package_ref)?,
        package_kind: wire.package_kind,
        platform: wire.platform,
        observed_at: wire.observed_at,
        captured_at: wire.captured_at,
        coverage: serde_json::to_value(wire.coverage).expect("coverage is serializable"),
        records: wire.records,
        checkpoint: wire.checkpoint,
        raw,
    })
}

fn validate_coverage(
    coverage: &CoverageWire,
    package_kind: &str,
) -> Result<(), ProducerRuntimeContractError> {
    if !coverage.target.is_object() || coverage.layers.is_empty() || coverage.layers.len() > 16 {
        return Err(ProducerRuntimeContractError::InvalidCoverage);
    }
    for layer in &coverage.layers {
        let known_set = coverage.target.get("basis").and_then(Value::as_str) == Some("known_set");
        if !PACKAGE_KINDS.contains(&layer.capability.as_str())
            || layer.stopped_reason.trim().is_empty()
            || layer.attempted > layer.observed
            || layer.acquired > layer.attempted
            || layer.verified > layer.acquired
            || (!known_set && layer.not_attempted != 0)
        {
            return Err(ProducerRuntimeContractError::InvalidCoverage);
        }
    }
    validate_comment_collection_receipt(coverage, package_kind)?;
    Ok(())
}

fn validate_comment_collection_receipt(
    coverage: &CoverageWire,
    package_kind: &str,
) -> Result<(), ProducerRuntimeContractError> {
    let Some(receipt) = coverage.target.get("commentCollection") else {
        return Ok(());
    };
    if package_kind != "comments" {
        return Err(ProducerRuntimeContractError::InvalidCoverage);
    }
    let Some(receipt) = receipt.as_object() else {
        return Err(ProducerRuntimeContractError::InvalidCoverage);
    };
    let exact_string = |key: &str| receipt.get(key).and_then(Value::as_str);
    let count = |key: &str| receipt.get(key).and_then(Value::as_u64);
    let nullable_count = |key: &str| {
        receipt
            .get(key)
            .map(|value| value.is_null() || value.as_u64().is_some())
            .unwrap_or(false)
    };
    let scope = exact_string("scope");
    let state = exact_string("state");
    let usability = exact_string("analysisUsability");
    let target_identity = exact_string("targetIdentity");
    let stop_reason = exact_string("stopReason");
    let version = receipt.get("version").and_then(Value::as_u64);
    let expected = receipt.get("expectedCount").and_then(Value::as_u64);
    let page_count = receipt.get("pageCommentCount").and_then(Value::as_u64);
    let acquired = count("uniqueCollectedCount");
    let complete_matches_expected = state != Some("complete")
        || match (scope, expected, acquired) {
            (Some("all_public_comments"), Some(expected), Some(acquired)) => acquired >= expected,
            (Some("detail_window"), Some(expected), Some(acquired)) => acquired == expected,
            _ => false,
        };
    let complete_all_public_matches_page = state != Some("complete")
        || scope != Some("all_public_comments")
        || match (page_count, expected, acquired) {
            (Some(page_count), Some(expected), Some(acquired)) => {
                page_count == expected && acquired >= expected
            }
            _ => false,
        };
    let requested_limit = count("requestedLimit");
    let detail_window_limit_is_valid = scope != Some("detail_window")
        || requested_limit.is_some_and(|limit| (1..=30).contains(&limit));
    let complete_detail_window_is_full = state != Some("complete")
        || scope != Some("detail_window")
        || requested_limit.is_some_and(|limit| {
            let required = page_count.map_or(limit, |page_count| page_count.min(limit));
            expected == Some(required) && acquired == Some(required)
        });
    let state_matches_identity = matches!(
        (state, target_identity),
        (Some("complete"), Some("matched"))
            | (Some("partial"), Some("matched" | "unverified"))
            | (Some("invalid_target"), Some("mismatched"))
    );
    let usability_matches_identity_and_count = match (target_identity, acquired, usability) {
        (Some("matched"), Some(0), Some("empty")) => true,
        (Some("matched"), Some(value), Some("usable")) if value > 0 => true,
        (Some("mismatched" | "unverified"), Some(_), Some("not_usable")) => true,
        _ => false,
    };
    let complete_has_nonterminal_stop = state != Some("complete")
        || !matches!(
            stop_reason,
            Some(
                "risk_control"
                    | "manual_stop"
                    | "comment_collection_failed"
                    | "target_identity_mismatch"
                    | "wrong_content"
                    | "api_unobserved"
            )
        );
    if version != Some(1)
        || !matches!(scope, Some("detail_window") | Some("all_public_comments"))
        || !matches!(
            state,
            Some("complete") | Some("partial") | Some("invalid_target")
        )
        || !matches!(
            usability,
            Some("usable") | Some("empty") | Some("not_usable")
        )
        || !matches!(
            target_identity,
            Some("matched") | Some("mismatched") | Some("unverified")
        )
        || exact_string("noteId").is_none_or(str::is_empty)
        || exact_string("stopReason").is_none_or(str::is_empty)
        || !nullable_count("pageCommentCount")
        || !nullable_count("expectedCount")
        || acquired.is_none()
        || !complete_matches_expected
        || !complete_all_public_matches_page
        || !detail_window_limit_is_valid
        || !complete_detail_window_is_full
        || !state_matches_identity
        || !usability_matches_identity_and_count
        || !complete_has_nonterminal_stop
    {
        return Err(ProducerRuntimeContractError::InvalidCoverage);
    }
    Ok(())
}

/// 风险策略必须与来源配对，两者不是各自独立的字段。
///
/// `local_trusted_user_initiated` 的安全性来自「有人在键盘前，是他点的，他看着」。服务端
/// 派发的任务没有这个人——它的边界来自租约。把两者混用会让追责链指向错误的人：查日志
/// 时会以为某次访问是人手动触发的。
///
/// 配对是双向的。只放开「scheduled 可用服务端策略」而不禁止「manual 用服务端策略」，
/// 插件就能自己铸造一份「服务端已授权」的任务，整条控制链被绕过。
fn risk_policy_matches_source(source: &str, risk_policy: &str) -> bool {
    match source {
        "manual" => risk_policy == LOCAL_TRUSTED_RISK_POLICY,
        "scheduled" => risk_policy == SERVER_LEASED_RISK_POLICY,
        _ => false,
    }
}

/// 人在键盘前发起、并且看着它跑。
pub const LOCAL_TRUSTED_RISK_POLICY: &str = "local_trusted_user_initiated";

/// 服务端在一份有到期时间的租约内授权。没有人在看，边界由租约与工单给定。
pub const SERVER_LEASED_RISK_POLICY: &str = "server_authorized_leased";

fn valid_target_for_capability(target: &Value, capability: &str) -> bool {
    let non_empty = |key: &str| {
        target
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    };
    match capability {
        "discovery_search" => non_empty("query"),
        "profile_discovery" | "author_profile" => non_empty("authorExternalId"),
        "content_detail" | "comments" | "replies" | "media_bytes" => non_empty("contentExternalId"),
        // A media slot either belongs to a work or to an independently observed creator
        // profile.  Requiring exactly one prevents a producer from inventing a work context
        // for an author avatar or from smuggling an ambiguous mixed target through the lane.
        "media_slots" => non_empty("contentExternalId") ^ non_empty("authorExternalId"),
        "batch_checkpoint" => non_empty("taskType"),
        _ => false,
    }
}

fn valid_comment_limit(value: &Value) -> bool {
    value.as_str() == Some("not_requested") || value.as_u64().is_some_and(|limit| limit > 0)
}

fn valid_acquire_media(value: &Value) -> bool {
    matches!(value.as_str(), Some("not_requested" | "slots" | "bytes"))
}

fn parse_uuid(value: &str) -> Result<Uuid, ProducerRuntimeContractError> {
    Uuid::parse_str(value).map_err(|_| ProducerRuntimeContractError::InvalidIdentifiers)
}

fn is_timestamp(value: &str) -> bool {
    // The boundary does not derive platform time.  It only requires the producer to submit an
    // explicit RFC3339 timestamp string; parsing/precision policy remains record-specific.
    value.contains('T') && value.ends_with('Z') && value.len() >= 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_boundary_rejects_nul_in_nested_values_and_keys_but_allows_encoded_original() {
        for value in [
            serde_json::json!({"records": [{"payload": {"bodyText": "前\0后"}}]}),
            serde_json::json!({"records": [{"bad\0key": "value"}]}),
        ] {
            assert!(matches!(
                validate_storage_text(&value),
                Err(ProducerRuntimeContractError::UnsupportedNullCharacter)
            ));
        }
        let original = "前\0后";
        let encoded = serde_json::to_string(original).unwrap();
        assert!(
            validate_storage_text(&serde_json::json!({"content": "前后", "originalJson": encoded}))
                .is_ok()
        );
        assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), original);
    }

    #[test]
    fn task_spec_allows_manual_and_scheduled_without_implementing_a_poller() {
        let value = r#"{"contractVersion":"linggan.producer.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"scheduled","platform":"xhs","pageType":"detail","target":{"contentExternalId":"note-1"},"capabilitiesRequested":["content_detail"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"server_authorized_leased","stopConditions":["maximum_quota"]}"#;
        let spec = parse_producer_task_spec(value).expect("flat task spec is accepted");
        assert_eq!(spec.source(), "scheduled");
        assert_eq!(spec.platform(), "xhs");
    }

    #[test]
    fn coverage_does_not_turn_unknown_into_a_zero_completion_claim() {
        let value = r#"{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"11111111-1111-4111-8111-111111111111","packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set"},"layers":[{"capability":"media_slots","observed":9,"attempted":7,"acquired":6,"verified":6,"failed":1,"notAttempted":2,"unknown":0,"stoppedReason":"risk_control"}]},"records":[]}"#;
        assert!(parse_producer_capture_package(value).is_ok());
    }

    #[test]
    fn quota_target_cannot_invent_unattempted_remainder() {
        let value = r#"{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"11111111-1111-4111-8111-111111111111","packageKind":"comments","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"maximum_quota","contentExternalId":"n"},"layers":[{"capability":"comments","observed":50,"attempted":50,"acquired":50,"verified":0,"failed":0,"notAttempted":50,"unknown":1,"stoppedReason":"risk_budget"}]},"records":[]}"#;
        assert!(matches!(
            parse_producer_capture_package(value),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));
    }

    #[test]
    fn comment_collection_complete_requires_reaching_the_observed_page_count() {
        let valid = r#"{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"11111111-1111-4111-8111-111111111111","packageKind":"comments","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"n","commentCollection":{"version":1,"noteId":"n","scope":"all_public_comments","pageCommentCount":300,"expectedCount":300,"uniqueCollectedCount":300,"state":"complete","analysisUsability":"usable","targetIdentity":"matched","stopReason":"comment_area_end"}},"layers":[{"capability":"comments","observed":300,"attempted":300,"acquired":300,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"comment_area_end"}]},"records":[]}"#;
        assert!(parse_producer_capture_package(valid).is_ok());

        let invalid = valid.replace(
            "\"uniqueCollectedCount\":300",
            "\"uniqueCollectedCount\":200",
        );
        assert!(matches!(
            parse_producer_capture_package(&invalid),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));

        let above_observed_count = valid
            .replace(
                "\"uniqueCollectedCount\":300",
                "\"uniqueCollectedCount\":302",
            )
            .replace(
                "\"observed\":300,\"attempted\":300,\"acquired\":300",
                "\"observed\":302,\"attempted\":302,\"acquired\":302",
            )
            .replace("comment_area_end", "no_progress");
        assert!(parse_producer_capture_package(&above_observed_count).is_ok());

        let truncated_but_self_consistent = valid
            .replace("\"expectedCount\":300", "\"expectedCount\":200")
            .replace(
                "\"uniqueCollectedCount\":300",
                "\"uniqueCollectedCount\":200",
            );
        assert!(matches!(
            parse_producer_capture_package(&truncated_but_self_consistent),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));

        let mismatched = valid.replace(
            "\"targetIdentity\":\"matched\"",
            "\"targetIdentity\":\"mismatched\"",
        );
        assert!(matches!(
            parse_producer_capture_package(&mismatched),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));

        let risk_stopped = valid.replace("comment_area_end", "risk_control");
        assert!(matches!(
            parse_producer_capture_package(&risk_stopped),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));
    }

    #[test]
    fn detail_window_complete_requires_the_full_standard_window() {
        let valid = r#"{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"11111111-1111-4111-8111-111111111111","packageKind":"comments","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"n","commentCollection":{"version":1,"noteId":"n","scope":"detail_window","requestedLimit":30,"pageCommentCount":80,"expectedCount":30,"uniqueCollectedCount":30,"state":"complete","analysisUsability":"usable","targetIdentity":"matched","stopReason":"target_reached"}},"layers":[{"capability":"comments","observed":30,"attempted":30,"acquired":30,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"target_reached"}]},"records":[]}"#;
        assert!(parse_producer_capture_package(valid).is_ok());

        let short_window = valid
            .replace("\"expectedCount\":30", "\"expectedCount\":20")
            .replace("\"uniqueCollectedCount\":30", "\"uniqueCollectedCount\":20");
        assert!(matches!(
            parse_producer_capture_package(&short_window),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));

        let oversized_window = valid
            .replace("\"requestedLimit\":30", "\"requestedLimit\":31")
            .replace("\"expectedCount\":30", "\"expectedCount\":31")
            .replace("\"uniqueCollectedCount\":30", "\"uniqueCollectedCount\":31");
        assert!(matches!(
            parse_producer_capture_package(&oversized_window),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));

        let known_short_page = valid
            .replace("\"pageCommentCount\":80", "\"pageCommentCount\":20")
            .replace("\"expectedCount\":30", "\"expectedCount\":20")
            .replace("\"uniqueCollectedCount\":30", "\"uniqueCollectedCount\":20");
        assert!(parse_producer_capture_package(&known_short_page).is_ok());

        let unknown_page = valid.replace("\"pageCommentCount\":80", "\"pageCommentCount\":null");
        assert!(parse_producer_capture_package(&unknown_page).is_ok());
        let unknown_short_window = unknown_page
            .replace("\"expectedCount\":30", "\"expectedCount\":20")
            .replace("\"uniqueCollectedCount\":30", "\"uniqueCollectedCount\":20");
        assert!(matches!(
            parse_producer_capture_package(&unknown_short_window),
            Err(ProducerRuntimeContractError::InvalidCoverage)
        ));
    }
}
