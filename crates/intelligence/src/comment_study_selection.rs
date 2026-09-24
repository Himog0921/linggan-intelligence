//! Shared deterministic selection for preview and start. No database or provider side effects.
//! Callers must supply one authorized snapshot; this module does not establish source permissions,
//! database idempotency, model readiness or the absence of concurrent/in-flight work.
use crate::comment_study_policy::json_hash;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

mod input;
#[cfg(test)]
mod tests;
pub use input::{PreparedStudyInput, prepare_study_input};

pub const SELECTION_ORDER: &str = "work_round_robin_oldest_first.v1";
const EXCLUSIONS: [&str; 10] = [
    "sourceRestricted", "bodyUnavailable", "indexPending", "textNotResearchable",
    "workAuthorUnknown", "commentAuthorUnknown", "creatorVoice", "inProgress",
    "notSelectedByMode", "budgetNotSelected",
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StudySelectionError {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("unsupported_domain")]
    UnsupportedDomain,
    #[error("invalid_limit")]
    InvalidLimit,
    #[error("invalid_selection_snapshot")]
    InvalidSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentKey {
    pub work_ref: Uuid,
    pub comment_external_id: String,
}

impl CommentKey {
    pub fn validate(&self) -> Result<(), StudySelectionError> {
        if self.work_ref.is_nil() || self.comment_external_id.trim().is_empty()
            || self.comment_external_id.len() > 512 || self.comment_external_id.contains('\0')
        { return Err(StudySelectionError::InvalidRequest); }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StudyScope {
    Works { #[serde(rename = "workRefs")] work_refs: Vec<Uuid> },
    Comments { #[serde(rename = "commentKeys")] comment_keys: Vec<CommentKey> },
}

impl StudyScope {
    fn normalize(&mut self) -> Result<(), StudySelectionError> {
        match self {
            Self::Works { work_refs } => {
                if !(1..=100).contains(&work_refs.len()) || work_refs.iter().any(Uuid::is_nil) {
                    return Err(StudySelectionError::InvalidRequest);
                }
                work_refs.sort_unstable();
                work_refs.dedup();
            }
            Self::Comments { comment_keys } => {
                if !(1..=3000).contains(&comment_keys.len()) {
                    return Err(StudySelectionError::InvalidRequest);
                }
                for key in comment_keys.iter() { key.validate()?; }
                comment_keys.sort_unstable();
                comment_keys.dedup();
                if comment_keys.iter().map(|k| k.work_ref).collect::<BTreeSet<_>>().len() > 100 {
                    return Err(StudySelectionError::InvalidRequest);
                }
            }
        }
        Ok(())
    }

    fn contains(&self, key: &CommentKey) -> bool {
        match self {
            Self::Works { work_refs } => work_refs.binary_search(&key.work_ref).is_ok(),
            Self::Comments { comment_keys } => comment_keys.binary_search(key).is_ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudySelectionMode { NewOnly, InputChanged, RetryFailed, Reanalyse }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyRunLimits {
    pub comment_budget: i32,
    pub context_character_budget: i32,
    pub token_limit: i64,
}

impl StudyRunLimits {
    pub fn validate(&self) -> Result<(), StudySelectionError> {
        if !(1..=3000).contains(&self.comment_budget)
            || !(1..=20000).contains(&self.context_character_budget)
            || !(1024..=10000000).contains(&self.token_limit)
        { return Err(StudySelectionError::InvalidLimit); }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionPreviewCommand {
    pub domain_ref: Uuid,
    pub policy_ref: Uuid,
    pub scope: StudyScope,
    pub mode: StudySelectionMode,
    pub limits: StudyRunLimits,
}

impl SelectionPreviewCommand {
    pub fn normalize(mut self) -> Result<Self, StudySelectionError> {
        validate_domain(self.domain_ref)?;
        if self.policy_ref.is_nil() { return Err(StudySelectionError::InvalidRequest); }
        self.limits.validate()?;
        self.scope.normalize()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartStudyRunCommand {
    pub request_ref: Uuid,
    pub domain_ref: Uuid,
    pub policy_ref: Uuid,
    pub scope: StudyScope,
    pub mode: StudySelectionMode,
    pub limits: StudyRunLimits,
    // Missing is not equivalent to explicit null: the approved start Schema requires this field.
    #[serde(deserialize_with = "required_reason")]
    pub reason: Option<String>,
}

fn required_reason<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

impl StartStudyRunCommand {
    pub fn normalize(mut self) -> Result<Self, StudySelectionError> {
        if self.request_ref.is_nil() { return Err(StudySelectionError::InvalidRequest); }
        let selection = self.preview().normalize()?;
        self.scope = selection.scope;
        match (self.mode, &self.reason) {
            (StudySelectionMode::NewOnly, None) => {},
            (StudySelectionMode::NewOnly, Some(_)) => return Err(StudySelectionError::InvalidRequest),
            (_, Some(reason)) if !reason.contains('\0') && reason.chars().count() <= 500
                && !reason.trim().is_empty() => { self.reason = Some(reason.trim().to_owned()); },
            _ => return Err(StudySelectionError::InvalidRequest),
        }
        Ok(self)
    }

    pub fn preview(&self) -> SelectionPreviewCommand {
        SelectionPreviewCommand { domain_ref: self.domain_ref, policy_ref: self.policy_ref,
            scope: self.scope.clone(), mode: self.mode, limits: self.limits.clone() }
    }

    /// Manual intent checksum only; SQL must still persist/check request_ref atomically.
    /// The receipt key is separate from its content. No HTTP-supplied origin is accepted.
    pub fn manual_request_hash(&self) -> Result<String, StudySelectionError> {
        let command = self.clone().normalize()?;
        let value = json!({"domainRef":command.domain_ref,"policyRef":command.policy_ref,
            "scope":command.scope,"mode":command.mode,"limits":command.limits,"reason":command.reason,
            "origin":"manual","originRef":null,"scheduledFor":null});
        json_hash(&value).map_err(|_| StudySelectionError::InvalidRequest)
    }
}

fn validate_domain(domain: Uuid) -> Result<(), StudySelectionError> {
    if domain.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
        return Err(StudySelectionError::UnsupportedDomain);
    }
    Ok(())
}

/// Interoperable advisory-lock key, not a new entity ID or proof that a lock was acquired.
pub fn study_domain_lock_key(domain: Uuid) -> Result<i64, StudySelectionError> {
    validate_domain(domain)?;
    let digest = Sha256::digest(format!("comment-study/domain/{domain}").as_bytes());
    let mut bytes = [0; 8];
    bytes.copy_from_slice(&digest[..8]);
    Ok(i64::from_be_bytes(bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyTargetState { Ready, Queued, Running, Succeeded, NoSignal, NeedsContext, Failed, Excluded, Cancelled }

#[derive(Debug, Clone)]
pub struct SelectionHistory {
    pub state: StudyTargetState,
    pub input_fingerprint: Option<String>,
    /// True only if stopped/legacy_unrecorded AND no live invocation were both established.
    pub legacy_stopped_without_live_invocation: bool,
}

/// Snapshot metadata, not a persisted object. source_rank comes from (created_at, material_ref)
/// inside each work. in_progress must include ALL authorized attempts, not just the latest one.
#[derive(Debug, Clone)]
pub struct SelectionCandidate {
    pub comment_key: CommentKey,
    pub source_ref: Uuid,
    pub source_rank: u64,
    pub source_flags: [bool; 7],
    pub in_progress: bool,
    pub latest: Option<SelectionHistory>,
    pub input_fingerprint: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySelection {
    pub scope_comment_count: usize,
    pub target_count: usize,
    /// Indices into the caller's immutable snapshot, in actual selection order.
    #[serde(skip)]
    pub selected_indices: Vec<usize>,
    pub exclusion_counts: BTreeMap<&'static str, usize>,
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn selected_by_mode(mode: StudySelectionMode, row: &SelectionCandidate) -> bool {
    use StudySelectionMode::*;
    use StudyTargetState::*;
    let Some(history) = &row.latest else { return matches!(mode, NewOnly | Reanalyse); };
    let terminal = matches!(history.state, Succeeded | NoSignal | NeedsContext | Failed | Excluded | Cancelled);
    let stopped_legacy = !terminal && history.legacy_stopped_without_live_invocation;
    match mode {
        NewOnly => false,
        InputChanged => terminal && history.input_fingerprint.as_ref().is_some_and(|old|
            row.input_fingerprint.as_ref().is_some_and(|current| current != old)),
        RetryFailed => matches!(history.state, Failed | Cancelled) || stopped_legacy,
        Reanalyse => terminal || stopped_legacy,
    }
}

/// One selection rule for both entry points. This does not reserve targets, perform a lookup,
/// or assert that missing comment keys exist. The SQL snapshot reader owns those obligations.
pub fn choose_study_targets(
    command: &SelectionPreviewCommand, rows: &[SelectionCandidate],
) -> Result<StudySelection, StudySelectionError> {
    let command = command.clone().normalize()?;
    let mut seen = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut groups = BTreeMap::<Uuid, Vec<usize>>::new();
    let mut counts: BTreeMap<_, _> = EXCLUSIONS.into_iter().map(|name| (name, 0)).collect();
    for (index, row) in rows.iter().enumerate() {
        row.comment_key.validate()?;
        if row.source_ref.is_nil() || row.source_rank == 0 || !command.scope.contains(&row.comment_key)
            || !seen.insert(row.comment_key.clone()) || !seen_sources.insert(row.source_ref)
            || row.input_fingerprint.as_ref().is_some_and(|h| !valid_fingerprint(h))
            || row.latest.as_ref().and_then(|h| h.input_fingerprint.as_ref())
                .is_some_and(|h| !valid_fingerprint(h))
        { return Err(StudySelectionError::InvalidSnapshot); }
        let reason = crate::comment_study_source::gate::exclusion(row.source_flags);
        if let Some(reason) = reason { *counts.get_mut(reason).ok_or(StudySelectionError::InvalidSnapshot)? += 1; }
        else if row.in_progress { *counts.get_mut("inProgress").ok_or(StudySelectionError::InvalidSnapshot)? += 1; }
        else {
            if row.input_fingerprint.is_none() { return Err(StudySelectionError::InvalidSnapshot); }
            if !selected_by_mode(command.mode, row) {
                *counts.get_mut("notSelectedByMode").ok_or(StudySelectionError::InvalidSnapshot)? += 1;
            } else { groups.entry(row.comment_key.work_ref).or_default().push(index); }
        }
    }
    if let StudyScope::Comments { comment_keys } = &command.scope
        && comment_keys.len() != seen.len()
    { return Err(StudySelectionError::InvalidSnapshot); }
    let mut order = Vec::new();
    for (work, indices) in &mut groups {
        indices.sort_by_key(|i| (rows[*i].source_rank, rows[*i].source_ref));
        for (rank, index) in indices.iter().enumerate() {
            order.push((rank, *work, rows[*index].comment_key.comment_external_id.as_str(), *index));
        }
    }
    order.sort_unstable();
    let target_count = order.len().min(command.limits.comment_budget as usize);
    *counts.get_mut("budgetNotSelected").ok_or(StudySelectionError::InvalidSnapshot)? = order.len() - target_count;
    let selected_indices = order.into_iter().take(target_count).map(|item| item.3).collect();
    Ok(StudySelection { scope_comment_count: rows.len(), target_count, selected_indices, exclusion_counts: counts })
}
