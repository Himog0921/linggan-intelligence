//! One semantic-identity and eligibility seam for automatic, prepared, and reserved research.
//! Execution configuration is deliberately absent from the fingerprint: it is provenance, not
//! a reason to reinterpret a comment. The persisted manifest contains only identifiers and
//! hashes, never a second retained copy of comment or context text.

use crate::{
    comment_analysis::CommentAnalysisInput,
    comment_cleaning::{CLEANER_VERSION, clean, outbound},
    comment_packet::{semantic_context_evidence_identity, semantic_context_for_comment},
    comment_research::comment_source_hash,
    comment_research_rules::RuleSnapshot,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// Immutable effective-rule fields that affect a comment's meaning. This intentionally does
/// not contain a model, connection, credential, budget, queue time, or engagement metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleFingerprint {
    pub rule_revision_ref: Uuid,
    pub canonical_hash: String,
    pub schema_version: String,
    pub selector_version: String,
    pub rule_version: String,
}

impl RuleFingerprint {
    pub fn new(
        rule_revision_ref: Uuid,
        canonical_hash: impl Into<String>,
        schema_version: impl Into<String>,
        selector_version: impl Into<String>,
        rule_version: impl Into<String>,
    ) -> Self {
        Self {
            rule_revision_ref,
            canonical_hash: canonical_hash.into(),
            schema_version: schema_version.into(),
            selector_version: selector_version.into(),
            rule_version: rule_version.into(),
        }
    }
}

impl From<&RuleSnapshot> for RuleFingerprint {
    fn from(rule: &RuleSnapshot) -> Self {
        Self::new(
            rule.rule_revision_ref,
            rule.canonical_hash.clone(),
            rule.schema_version.clone(),
            rule.selector_version.clone(),
            rule.rule_version.as_str(),
        )
    }
}

/// A canonical comment is the stable work identity plus its platform comment identity. A
/// material UUID identifies one observation and must not be used as the semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceIdentity {
    pub work_ref: Uuid,
    pub comment_external_id: String,
}

pub fn stable_source_identity(source: &SourceIdentity) -> String {
    comment_source_hash(&format!(
        "{}:{}",
        source.work_ref, source.comment_external_id
    ))
}

/// A non-text manifest is persisted with newly created semantic work. It makes compatibility
/// proof possible without copying raw comment/context material into a long-lived work row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticInput {
    pub source_identity: String,
    pub fingerprint: String,
    pub input_manifest: Value,
}

/// Builds the semantic identity from the actual model input. `CommentAnalysisInput::model_version`
/// is intentionally not read; changing a default model or credential must not invalidate an
/// already accepted interpretation.
pub fn build_semantic_input(
    input: &CommentAnalysisInput,
    source: &SourceIdentity,
    rule: &RuleFingerprint,
) -> SemanticInput {
    let cleaned = outbound(clean(&input.body));
    let semantic_context = semantic_context_for_comment(&input.context, &cleaned.text);
    let context_identity = semantic_context_evidence_identity(&input.context, &cleaned.text);
    let manifest_identity = evidence_manifest_identity(&context_identity);
    let source_identity = stable_source_identity(source);
    let semantic_context_hash = comment_source_hash(&semantic_context.to_string());
    let context_identity_hash = comment_source_hash(&context_identity.to_string());
    let cleaned_text_hash = comment_source_hash(&cleaned.text);
    let manifest = json!({
        "sourceIdentity": source_identity,
        "sourceSha256": input.source_sha256,
        "cleanedTextHash": cleaned_text_hash,
        "cleanState": cleaned.state,
        "context": {
            "semanticHash": semantic_context_hash,
            "evidenceIdentity": manifest_identity,
            "evidenceIdentityHash": context_identity_hash,
        },
        "rule": {
            "ruleRevisionRef": rule.rule_revision_ref,
            "canonicalHash": rule.canonical_hash,
            "schemaVersion": rule.schema_version,
            "selectorVersion": rule.selector_version,
            "ruleVersion": rule.rule_version,
        },
        "cleanerVersion": CLEANER_VERSION,
    });
    let fingerprint = comment_source_hash(
        &json!({
            "sourceIdentity": source_identity,
            "sourceSha256": input.source_sha256,
            "cleanedText": cleaned.text,
            "semanticContext": semantic_context,
            "contextEvidenceIdentity": context_identity,
            "rule": {
                "canonicalHash": rule.canonical_hash,
                "schemaVersion": rule.schema_version,
                "selectorVersion": rule.selector_version,
                "ruleVersion": rule.rule_version,
            },
            "cleanerVersion": CLEANER_VERSION,
        })
        .to_string(),
    );
    SemanticInput {
        source_identity,
        fingerprint,
        input_manifest: manifest,
    }
}

/// A user-requested reanalysis is an independent generation. It never mutates or overwrites the
/// base semantic key, so historical accepted analyses stay explainable.
pub fn explicit_generation_fingerprint(base_fingerprint: &str, generation: Uuid) -> String {
    comment_source_hash(
        &json!({"baseFingerprint":base_fingerprint,"generation":generation}).to_string(),
    )
}

fn evidence_manifest_identity(context_identity: &Value) -> Value {
    let fragments: Vec<Value> = context_identity
        .as_array()
        .into_iter()
        .flatten()
        .map(|fragment| {
            json!({
                "fragmentRef": fragment["fragmentRef"],
                "kind": fragment["kind"],
                "textHash": comment_source_hash(fragment["text"].as_str().unwrap_or("")),
                "sourceSpanHash": fragment["sourceSpanHash"],
                "stableIdentity": fragment["stableIdentity"],
                "sourcePath": fragment["sourcePath"],
            })
        })
        .collect();
    json!({"fragments":fragments})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanState {
    Direct,
    Context,
    LowInformation,
    Anomaly,
    Dropped,
    Unknown,
}

impl CleanState {
    pub fn is_researchable(self) -> bool {
        matches!(self, Self::Direct | Self::Context)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQualification {
    pub source_readable: bool,
    pub clean_state: CleanState,
    pub input_valid: bool,
    pub context_missing: bool,
    pub reason_code: Option<String>,
}

impl SourceQualification {
    pub fn eligible(clean_state: CleanState) -> Self {
        Self {
            source_readable: true,
            clean_state,
            input_valid: true,
            context_missing: false,
            reason_code: None,
        }
    }

    fn category(&self) -> Option<EligibilityCategory> {
        if !self.source_readable || !self.clean_state.is_researchable() {
            return Some(if self.clean_state == CleanState::Anomaly {
                EligibilityCategory::Anomaly
            } else {
                EligibilityCategory::Excluded
            });
        }
        if self.context_missing {
            return Some(EligibilityCategory::ContextMissing);
        }
        if !self.input_valid {
            return Some(EligibilityCategory::Excluded);
        }
        None
    }

    fn reason(&self, category: EligibilityCategory) -> String {
        self.reason_code.clone().unwrap_or_else(|| {
            match category {
                EligibilityCategory::ContextMissing => "context_missing",
                EligibilityCategory::Anomaly => "cleaning_anomaly",
                EligibilityCategory::Excluded => "source_not_researchable",
                _ => "source_qualified",
            }
            .into()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultState {
    Unstudied,
    Studied,
    NoSignal,
    Outdated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Idle,
    Queued,
    Running,
    Failed,
    WaitingRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityCategory {
    Reusable,
    InProgress,
    FirstResearch,
    Outdated,
    FailedRecovery,
    Excluded,
    ContextMissing,
    Anomaly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryPolicy {
    pub retry_max_attempts: i32,
    pub unknown_retry_max_attempts: i32,
    pub retry_cooldown_seconds: i64,
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self {
            retry_max_attempts: 3,
            unknown_retry_max_attempts: 0,
            retry_cooldown_seconds: 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EligibilityRequest {
    pub workspace_ref: Uuid,
    pub source_ref: Uuid,
    pub input: SemanticInput,
    pub qualification: SourceQualification,
    pub explicit_generation: Option<Uuid>,
    pub recovery: RecoveryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEligibility {
    pub attempts: i32,
    pub unknown_retry_attempts: i32,
    pub retry_limit: i32,
    pub unknown_charge: bool,
    pub failure_recoverable: bool,
    pub cooldown_elapsed: bool,
    pub retry_permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eligibility {
    pub source_ref: Uuid,
    pub source_identity: String,
    pub base_fingerprint: String,
    pub fingerprint: String,
    pub input_manifest: Value,
    pub category: EligibilityCategory,
    pub result_state: ResultState,
    pub execution_state: ExecutionState,
    pub last_accepted_analysis_ref: Option<Uuid>,
    pub current_attempt_ref: Option<Uuid>,
    pub semantic_ref: Option<Uuid>,
    pub reason_code: String,
    pub eligible_to_dispatch: bool,
    pub recovery: Option<RecoveryEligibility>,
}

#[derive(Debug, Clone)]
struct SemanticWork {
    semantic_ref: Uuid,
    state: String,
    analysis_ref: Option<Uuid>,
    invocation_ref: Option<Uuid>,
    failure_code: Option<String>,
    attempts: i32,
    unknown_retry_attempts: i32,
    cooldown_elapsed: bool,
    unknown_charge: bool,
}

/// Database wrapper for callers without an existing reservation transaction. Reservation must
/// call `evaluate_eligibility_in` using the same locked connection so its classification and
/// ownership claim share one snapshot.
pub async fn evaluate_eligibility(
    db: &Database,
    request: &EligibilityRequest,
) -> Result<Eligibility, ModelError> {
    let mut connection = db.pool().acquire().await?;
    evaluate_eligibility_in(&mut *connection, request).await
}

pub async fn evaluate_eligibility_in(
    connection: &mut PgConnection,
    request: &EligibilityRequest,
) -> Result<Eligibility, ModelError> {
    let target_fingerprint = request
        .explicit_generation
        .map(|generation| explicit_generation_fingerprint(&request.input.fingerprint, generation))
        .unwrap_or_else(|| request.input.fingerprint.clone());
    let current = read_semantic_work(
        connection,
        request.workspace_ref,
        &request.input.source_identity,
        &target_fingerprint,
        request.recovery.retry_cooldown_seconds,
    )
    .await?;
    let base = if target_fingerprint == request.input.fingerprint {
        current.clone()
    } else {
        read_semantic_work(
            connection,
            request.workspace_ref,
            &request.input.source_identity,
            &request.input.fingerprint,
            request.recovery.retry_cooldown_seconds,
        )
        .await?
    };
    let last_accepted = read_last_accepted(
        connection,
        request.workspace_ref,
        &request.input.source_identity,
        &request.input.fingerprint,
    )
    .await?;
    Ok(classify_eligibility(
        request,
        target_fingerprint,
        current,
        base,
        last_accepted,
    ))
}

async fn read_semantic_work(
    connection: &mut PgConnection,
    workspace_ref: Uuid,
    source_identity: &str,
    fingerprint: &str,
    cooldown_seconds: i64,
) -> Result<Option<SemanticWork>, ModelError> {
    let row = sqlx::query(
        r#"
         SELECT sw.semantic_ref,sw.state,sw.analysis_ref,sw.invocation_ref,sw.failure_code,
                sw.attempts,sw.unknown_retry_attempts,
                sw.updated_at<=scope_001_now()-($4::bigint*interval '1 second') AS cooldown_elapsed,
                EXISTS(
                  SELECT 1 FROM linggan_model_invocation invocation
                  WHERE invocation.invocation_ref=sw.invocation_ref
                    AND COALESCE((invocation.result->>'callStarted')::boolean,true)
                    AND (invocation.input_tokens IS NULL OR invocation.output_tokens IS NULL)
                ) AS unknown_charge
         FROM linggan_comment_semantic_work sw
         WHERE sw.workspace_ref=$1 AND sw.identity_key=$2
           AND EXISTS(
             SELECT 1 FROM linggan_comment_research_readable readable
             WHERE readable.material_ref=sw.source_ref
           )
           AND (
             sw.analysis_ref IS NULL OR EXISTS(
               SELECT 1 FROM linggan_comment_analysis_work analysis
               WHERE analysis.work_ref=sw.analysis_ref
                 AND linggan_ci_analysis_context_readable(analysis.result)
             )
           )
           AND (sw.fingerprint=$3 OR (sw.input_manifest->>'baseFingerprint'=$3 AND sw.state IN('succeeded','no_signal')) OR EXISTS(
             SELECT 1 FROM linggan_comment_legacy_fingerprint_alias alias
             WHERE alias.source_identity=$2 AND alias.new_fingerprint=$3
               AND alias.old_fingerprint=sw.fingerprint AND alias.analysis_ref=sw.analysis_ref
           ))
         ORDER BY (sw.state IN('succeeded','no_signal')) DESC,sw.updated_at DESC,sw.semantic_ref DESC
         LIMIT 1
        "#,
    )
    .bind(workspace_ref)
    .bind(source_identity)
    .bind(fingerprint)
    .bind(cooldown_seconds)
    .fetch_optional(&mut *connection)
    .await?;
    Ok(row.map(|row| SemanticWork {
        semantic_ref: row.get("semantic_ref"),
        state: row.get("state"),
        analysis_ref: row.get("analysis_ref"),
        invocation_ref: row.get("invocation_ref"),
        failure_code: row.get("failure_code"),
        attempts: row.get("attempts"),
        unknown_retry_attempts: row.get("unknown_retry_attempts"),
        cooldown_elapsed: row.get("cooldown_elapsed"),
        unknown_charge: row.get("unknown_charge"),
    }))
}

async fn read_last_accepted(
    connection: &mut PgConnection,
    workspace_ref: Uuid,
    source_identity: &str,
    base_fingerprint: &str,
) -> Result<Option<(ResultState, Uuid)>, ModelError> {
    let row = sqlx::query(
        r#"
         SELECT sw.state,sw.analysis_ref
         FROM linggan_comment_semantic_work sw
         JOIN linggan_comment_analysis_work analysis ON analysis.work_ref=sw.analysis_ref
         WHERE sw.workspace_ref=$1 AND sw.identity_key=$2
           AND sw.state IN('succeeded','no_signal')
           AND EXISTS(
             SELECT 1 FROM linggan_comment_research_readable readable
             WHERE readable.material_ref=sw.source_ref
           )
           AND linggan_ci_analysis_context_readable(analysis.result)
         ORDER BY (sw.fingerprint=$3 OR EXISTS(
             SELECT 1 FROM linggan_comment_legacy_fingerprint_alias alias
             WHERE alias.source_identity=$2 AND alias.new_fingerprint=$3
               AND alias.old_fingerprint=sw.fingerprint AND alias.analysis_ref=sw.analysis_ref
           )) DESC,sw.updated_at DESC,sw.semantic_ref DESC
         LIMIT 1
        "#,
    )
    .bind(workspace_ref)
    .bind(source_identity)
    .bind(base_fingerprint)
    .fetch_optional(&mut *connection)
    .await?;
    Ok(row.map(|row| {
        (
            if row.get::<String, _>("state") == "no_signal" {
                ResultState::NoSignal
            } else {
                ResultState::Studied
            },
            row.get("analysis_ref"),
        )
    }))
}

fn classify_eligibility(
    request: &EligibilityRequest,
    target_fingerprint: String,
    current: Option<SemanticWork>,
    base: Option<SemanticWork>,
    last_accepted: Option<(ResultState, Uuid)>,
) -> Eligibility {
    let accepted = last_accepted.as_ref().map(|(_, reference)| *reference);
    if let Some(category) = request.qualification.category() {
        return excluded_eligibility(request, target_fingerprint, accepted, category);
    }
    let explicit = request.explicit_generation.is_some();
    let mut result = initial_eligibility(
        request,
        target_fingerprint,
        current.as_ref(),
        base.as_ref(),
        last_accepted,
        explicit,
    );
    let Some(work) = current else {
        result.eligible_to_dispatch = true;
        return result;
    };
    apply_current_work(&mut result, request, &work);
    result
}

fn excluded_eligibility(
    request: &EligibilityRequest,
    target_fingerprint: String,
    accepted: Option<Uuid>,
    category: EligibilityCategory,
) -> Eligibility {
    Eligibility {
        source_ref: request.source_ref,
        source_identity: request.input.source_identity.clone(),
        base_fingerprint: request.input.fingerprint.clone(),
        fingerprint: target_fingerprint,
        input_manifest: request.input.input_manifest.clone(),
        category,
        result_state: if accepted.is_some() {
            ResultState::Outdated
        } else {
            ResultState::Unstudied
        },
        execution_state: if category == EligibilityCategory::ContextMissing {
            ExecutionState::WaitingRecovery
        } else {
            ExecutionState::Idle
        },
        last_accepted_analysis_ref: accepted,
        current_attempt_ref: None,
        semantic_ref: None,
        reason_code: request.qualification.reason(category),
        eligible_to_dispatch: false,
        recovery: None,
    }
}

fn initial_eligibility(
    request: &EligibilityRequest,
    target_fingerprint: String,
    current: Option<&SemanticWork>,
    base: Option<&SemanticWork>,
    last_accepted: Option<(ResultState, Uuid)>,
    explicit: bool,
) -> Eligibility {
    let accepted = last_accepted.as_ref().map(|(_, reference)| *reference);
    let result_state = if explicit {
        base.and_then(accepted_result)
            .or_else(|| last_accepted.as_ref().map(|(state, _)| *state))
            .unwrap_or(ResultState::Unstudied)
    } else if let Some(result) = current.and_then(accepted_result) {
        result
    } else if accepted.is_some() {
        ResultState::Outdated
    } else {
        ResultState::Unstudied
    };
    Eligibility {
        source_ref: request.source_ref,
        source_identity: request.input.source_identity.clone(),
        base_fingerprint: request.input.fingerprint.clone(),
        fingerprint: target_fingerprint,
        input_manifest: request.input.input_manifest.clone(),
        category: if result_state == ResultState::Outdated || explicit {
            EligibilityCategory::Outdated
        } else {
            EligibilityCategory::FirstResearch
        },
        result_state,
        execution_state: ExecutionState::Idle,
        last_accepted_analysis_ref: accepted,
        current_attempt_ref: None,
        semantic_ref: current.map(|work| work.semantic_ref),
        reason_code: if explicit {
            "explicit_reanalysis"
        } else if result_state == ResultState::Outdated {
            "semantic_input_changed"
        } else {
            "first_research"
        }
        .into(),
        eligible_to_dispatch: false,
        recovery: None,
    }
}

fn apply_current_work(result: &mut Eligibility, request: &EligibilityRequest, work: &SemanticWork) {
    result.current_attempt_ref = work.invocation_ref;
    match work.state.as_str() {
        "succeeded" | "no_signal" => {
            result.category = EligibilityCategory::Reusable;
            result.last_accepted_analysis_ref = work.analysis_ref;
            result.result_state = accepted_result(&work).unwrap_or(ResultState::Unstudied);
            result.reason_code = "current_result_accepted".into();
        }
        "running" => {
            result.category = EligibilityCategory::InProgress;
            result.execution_state = if work.invocation_ref.is_some() {
                ExecutionState::Running
            } else {
                ExecutionState::Queued
            };
            result.reason_code = "execution_owner_exists".into();
        }
        "failed" => {
            let retry_limit = if work.unknown_charge {
                request.recovery.unknown_retry_max_attempts
            } else {
                request.recovery.retry_max_attempts
            };
            let failure_recoverable = work.unknown_charge
                || work
                    .failure_code
                    .as_deref()
                    .is_some_and(recoverable_failure_code);
            let attempts_remaining = if work.unknown_charge {
                work.unknown_retry_attempts < retry_limit
            } else {
                work.attempts < retry_limit
            };
            let retry_permitted =
                failure_recoverable && attempts_remaining && work.cooldown_elapsed;
            result.category = EligibilityCategory::FailedRecovery;
            result.execution_state = if retry_permitted {
                ExecutionState::WaitingRecovery
            } else {
                ExecutionState::Failed
            };
            result.reason_code = if work.unknown_charge {
                "unknown_charge_recovery"
            } else {
                "failed_recovery"
            }
            .into();
            result.eligible_to_dispatch = retry_permitted;
            result.recovery = Some(RecoveryEligibility {
                attempts: work.attempts,
                unknown_retry_attempts: work.unknown_retry_attempts,
                retry_limit,
                unknown_charge: work.unknown_charge,
                failure_recoverable,
                cooldown_elapsed: work.cooldown_elapsed,
                retry_permitted,
            });
        }
        _ => {
            result.category = EligibilityCategory::Anomaly;
            result.execution_state = ExecutionState::Failed;
            result.reason_code = "semantic_work_state_invalid".into();
        }
    }
}

fn accepted_result(work: &SemanticWork) -> Option<ResultState> {
    match work.state.as_str() {
        "succeeded" => Some(ResultState::Studied),
        "no_signal" => Some(ResultState::NoSignal),
        _ => None,
    }
}

pub(crate) const RECOVERABLE_FAILURE_CODES: &[&str] = &[
    "provider_timeout",
    "provider_unavailable",
    "provider_network_error",
    "provider_rate_limited",
    "provider_stream_interrupted",
    "provider_terminal_missing",
    "missing_comment",
];
fn recoverable_failure_code(code: &str) -> bool {
    RECOVERABLE_FAILURE_CODES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> CommentAnalysisInput {
        CommentAnalysisInput {
            work_ref: Uuid::nil(),
            lease_ref: Uuid::nil(),
            source_ref: Uuid::nil(),
            source_sha256: comment_source_hash("我想知道怎样坚持"),
            body: "我想知道怎样坚持".into(),
            context: json!({
                "role":"unknown",
                "parentState":"NOT_APPLICABLE",
                "workIdentity":"work-stable",
                "work":{"title":{"value":"坚持练习"},"body":{"value":"完全无关的烹饪步骤"}},
                "researchFragments":[{"fragmentRef":"F001","kind":"title","sourceSpanHash":"title-hash"}]
            }),
            rule_version: "test-rule".into(),
            model_version: "connection-a:model-a".into(),
            instruction: "test",
            limitations: vec![],
        }
    }

    fn source() -> SourceIdentity {
        SourceIdentity {
            work_ref: Uuid::from_u128(7),
            comment_external_id: "comment-7".into(),
        }
    }

    fn rule() -> RuleFingerprint {
        RuleFingerprint::new(
            Uuid::from_u128(8),
            "rule-hash-a",
            "schema-a",
            "selector-a",
            "v4",
        )
    }

    #[test]
    fn semantic_key_excludes_model_and_unselected_context() {
        let original = input();
        let mut changed = input();
        changed.model_version = "connection-b:model-b".into();
        changed.context["work"]["body"]["value"] = json!("仍然不包含相同词项的无关正文");
        let a = build_semantic_input(&original, &source(), &rule());
        let b = build_semantic_input(&changed, &source(), &rule());
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.input_manifest["rule"]["canonicalHash"], "rule-hash-a");
        assert!(a.input_manifest.to_string().contains("cleanedTextHash"));
        assert!(!a.input_manifest.to_string().contains("我想知道怎样坚持"));
    }

    #[test]
    fn semantic_key_changes_for_selected_context_and_rule() {
        let original = input();
        let mut context_changed = input();
        context_changed.context["work"]["title"]["value"] = json!("新的坚持方法");
        let mut rule_changed = rule();
        rule_changed.canonical_hash = "rule-hash-b".into();
        let base = build_semantic_input(&original, &source(), &rule());
        assert_ne!(
            base.fingerprint,
            build_semantic_input(&context_changed, &source(), &rule()).fingerprint
        );
        assert_ne!(
            base.fingerprint,
            build_semantic_input(&original, &source(), &rule_changed).fingerprint
        );
    }

    #[test]
    fn explicit_generation_keeps_base_result_but_has_a_new_key() {
        let semantic = build_semantic_input(&input(), &source(), &rule());
        let one = explicit_generation_fingerprint(&semantic.fingerprint, Uuid::from_u128(1));
        let two = explicit_generation_fingerprint(&semantic.fingerprint, Uuid::from_u128(2));
        assert_ne!(semantic.fingerprint, one);
        assert_ne!(one, two);
    }

    #[test]
    fn changed_input_with_old_accepted_and_new_failed_is_outdated_and_failed() {
        let semantic = build_semantic_input(&input(), &source(), &rule());
        let request = EligibilityRequest {
            workspace_ref: Uuid::from_u128(9),
            source_ref: Uuid::from_u128(10),
            input: semantic,
            qualification: SourceQualification::eligible(CleanState::Direct),
            explicit_generation: None,
            recovery: RecoveryPolicy::default(),
        };
        let old_analysis = Uuid::from_u128(11);
        let failed = SemanticWork {
            semantic_ref: Uuid::from_u128(12),
            state: "failed".into(),
            analysis_ref: None,
            invocation_ref: Some(Uuid::from_u128(13)),
            failure_code: Some("provider_timeout".into()),
            attempts: 3,
            unknown_retry_attempts: 0,
            cooldown_elapsed: true,
            unknown_charge: false,
        };
        let result = classify_eligibility(
            &request,
            request.input.fingerprint.clone(),
            Some(failed.clone()),
            Some(failed),
            Some((ResultState::Studied, old_analysis)),
        );
        assert_eq!(result.result_state, ResultState::Outdated);
        assert_eq!(result.execution_state, ExecutionState::Failed);
        assert_eq!(result.last_accepted_analysis_ref, Some(old_analysis));
        assert!(!result.eligible_to_dispatch);
    }

    #[test]
    fn clean_exclusion_is_not_an_execution_failure() {
        let semantic = build_semantic_input(&input(), &source(), &rule());
        let request = EligibilityRequest {
            workspace_ref: Uuid::from_u128(9),
            source_ref: Uuid::from_u128(10),
            input: semantic.clone(),
            qualification: SourceQualification {
                source_readable: true,
                clean_state: CleanState::Dropped,
                input_valid: false,
                context_missing: false,
                reason_code: Some("cleaning_dropped".into()),
            },
            explicit_generation: None,
            recovery: RecoveryPolicy::default(),
        };
        let result = classify_eligibility(&request, semantic.fingerprint.clone(), None, None, None);
        assert_eq!(result.category, EligibilityCategory::Excluded);
        assert_eq!(result.execution_state, ExecutionState::Idle);
        assert_ne!(result.category, EligibilityCategory::FailedRecovery);
    }

    #[test]
    fn unknown_cost_gets_only_its_configured_extra_recovery() {
        let semantic = build_semantic_input(&input(), &source(), &rule());
        let request = EligibilityRequest {
            workspace_ref: Uuid::from_u128(9),
            source_ref: Uuid::from_u128(10),
            input: semantic.clone(),
            qualification: SourceQualification::eligible(CleanState::Direct),
            explicit_generation: None,
            recovery: RecoveryPolicy {
                retry_max_attempts: 3,
                unknown_retry_max_attempts: 1,
                retry_cooldown_seconds: 60,
            },
        };
        let mut unknown = SemanticWork {
            semantic_ref: Uuid::from_u128(12),
            state: "failed".into(),
            analysis_ref: None,
            invocation_ref: Some(Uuid::from_u128(13)),
            failure_code: Some("worker_interrupted".into()),
            attempts: 1,
            unknown_retry_attempts: 0,
            cooldown_elapsed: true,
            unknown_charge: true,
        };
        let permitted = classify_eligibility(
            &request,
            semantic.fingerprint.clone(),
            Some(unknown.clone()),
            Some(unknown.clone()),
            None,
        );
        assert!(permitted.eligible_to_dispatch);
        unknown.unknown_retry_attempts = 1;
        let exhausted = classify_eligibility(
            &request,
            semantic.fingerprint.clone(),
            Some(unknown.clone()),
            Some(unknown),
            None,
        );
        assert!(!exhausted.eligible_to_dispatch);
        assert_eq!(exhausted.execution_state, ExecutionState::Failed);
    }

    #[test]
    fn same_explicit_generation_reuses_its_accepted_result() {
        let semantic = build_semantic_input(&input(), &source(), &rule());
        let request = EligibilityRequest {
            workspace_ref: Uuid::from_u128(9),
            source_ref: Uuid::from_u128(10),
            input: semantic.clone(),
            qualification: SourceQualification::eligible(CleanState::Direct),
            explicit_generation: Some(Uuid::from_u128(14)),
            recovery: RecoveryPolicy::default(),
        };
        let completed = SemanticWork {
            semantic_ref: Uuid::from_u128(12),
            state: "succeeded".into(),
            analysis_ref: Some(Uuid::from_u128(15)),
            invocation_ref: Some(Uuid::from_u128(13)),
            failure_code: None,
            attempts: 1,
            unknown_retry_attempts: 0,
            cooldown_elapsed: true,
            unknown_charge: false,
        };
        let result = classify_eligibility(
            &request,
            explicit_generation_fingerprint(&semantic.fingerprint, Uuid::from_u128(14)),
            Some(completed.clone()),
            Some(completed),
            None,
        );
        assert_eq!(result.category, EligibilityCategory::Reusable);
        assert!(!result.eligible_to_dispatch);
    }
}
