//! Same-work batch envelopes and their strictly addressable model-output contract.
//!
//! A batch exists to share one frozen work context. It does not turn several comments into one
//! source: every output carries its target reference and is later admitted against that target's
//! own immutable comment text.

use crate::comment_study_semantic::{AcceptedSignal, SEMANTIC_CONTRACT, accept_semantic_output};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const BATCH_CONTRACT: &str = "comment-study.note-batch.v1";
pub const MAX_TARGETS_PER_BATCH: i64 = 12;

#[derive(Debug, Clone)]
pub struct PrepareStudyBatchRequest {
    pub run_ref: Uuid,
    pub maximum_targets: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedStudyBatch {
    pub batch_ref: Uuid,
    pub run_ref: Uuid,
    pub content_public_ref: Uuid,
    pub target_refs: Vec<Uuid>,
    pub input_manifest: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedBatchTarget {
    pub target_ref: Uuid,
    pub state: BatchTargetState,
    pub signals: Vec<AcceptedSignal>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchTargetState {
    Succeeded,
    NoSignal,
    NeedsContext,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedBatchOutput {
    pub targets: Vec<ParsedBatchTarget>,
    pub rejected_targets: Vec<RejectedBatchTarget>,
    pub missing_target_refs: Vec<Uuid>,
    /// Results whose `targetRef` could not be read at all, so they belong to no target. They are
    /// counted rather than rejected: attributing them would invent a victim.
    pub unattributable_result_count: usize,
    /// Results addressed to a target outside this batch. They are never admitted and never create
    /// a source, but they also do not implicate the targets that were addressed correctly.
    pub unexpected_result_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedBatchTarget {
    pub target_ref: Uuid,
    pub rejection_code: &'static str,
}

#[derive(Debug, Error)]
pub enum StudyBatchError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the requested batch target limit must be between one and twelve")]
    InvalidTargetLimit,
    #[error("the StudyRun is absent or cannot create a batch in its current state")]
    RunUnavailable,
    #[error("the StudyRun has no queued target eligible for a same-work batch")]
    NoQueuedTargets,
    #[error("batch output does not satisfy the JSON contract")]
    OutputSchema,
    #[error("batch output names a different contract, batch, or work")]
    OutputIdentityMismatch,
    #[error("batch output uses an outcome incompatible with its per-target payload")]
    OutputOutcomeMismatch,
    #[error("batch output has an invalid per-target semantic contract")]
    Semantic(#[source] crate::comment_study_semantic::SemanticContractError),
}

struct BatchWork {
    content_public_ref: Uuid,
    context_manifest: Value,
}

#[derive(Clone)]
struct BatchTargetRow {
    target_ref: Uuid,
    source_ref: Uuid,
    research_text: String,
    dependency_state: String,
    input_manifest: Value,
}

/// Freezes a same-work batch that fits the configured input and output budget. The caller's
/// maximum remains a hard safety ceiling; a configured model may reduce it further.
pub async fn prepare_study_batch(
    database: &Database,
    request: PrepareStudyBatchRequest,
) -> Result<PreparedStudyBatch, StudyBatchError> {
    if !(1..=MAX_TARGETS_PER_BATCH).contains(&request.maximum_targets) {
        return Err(StudyBatchError::InvalidTargetLimit);
    }
    let mut transaction = database.pool().begin().await?;
    ensure_batchable_run(&mut transaction, request.run_ref).await?;
    let work = lock_next_work(&mut transaction, request.run_ref).await?;
    let candidate_targets = lock_work_targets(
        &mut transaction,
        request.run_ref,
        work.content_public_ref,
        request.maximum_targets,
    )
    .await?;
    if candidate_targets.is_empty() {
        return Err(StudyBatchError::NoQueuedTargets);
    }
    let targets =
        fit_targets_to_model_budget(&mut transaction, request.run_ref, &work, candidate_targets)
            .await?;
    if targets.is_empty() {
        return Err(StudyBatchError::NoQueuedTargets);
    }
    let batch_ref = Uuid::new_v4();
    let manifest = batch_manifest(batch_ref, request.run_ref, &work, &targets);
    sqlx::query(
        "INSERT INTO linggan_comment_study_batch( \
           batch_ref,run_ref,content_public_ref,state,input_manifest,input_hash \
         ) VALUES($1,$2,$3,'prepared',$4,$5)",
    )
    .bind(batch_ref)
    .bind(request.run_ref)
    .bind(work.content_public_ref)
    .bind(&manifest)
    .bind(hash_value(&manifest))
    .execute(&mut *transaction)
    .await?;
    for (index, target) in targets.iter().enumerate() {
        sqlx::query(
            "INSERT INTO linggan_comment_study_batch_target(batch_ref,target_ref,ordinal) \
             VALUES($1,$2,$3)",
        )
        .bind(batch_ref)
        .bind(target.target_ref)
        .bind(i32::try_from(index + 1).expect("batch ordinal fits i32"))
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_target SET state='running' \
             WHERE target_ref=$1 AND state='queued'",
        )
        .bind(target.target_ref)
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query(
        "UPDATE linggan_comment_study_run SET state='running' \
         WHERE run_ref=$1 AND state IN ('prepared','queued')",
    )
    .bind(request.run_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(PreparedStudyBatch {
        batch_ref,
        run_ref: request.run_ref,
        content_public_ref: work.content_public_ref,
        target_refs: targets.iter().map(|target| target.target_ref).collect(),
        input_manifest: manifest,
    })
}

/// Finds the oldest run that still has queued targets waiting to be packaged into a batch, other
/// than any run in `exclude_run_refs`.
///
/// `prepare_study_batch` only ever creates one batch for one already-identified run; nothing in
/// the codebase previously decided *which* run to call it for. Without this, a StudyRun's targets
/// stayed `queued` forever no matter how long the model worker's `claim_next_study_batch` loop
/// ran, because that loop only claims batches that already exist — it never creates one. This
/// does not select which comments enter research (that stays `prepare_study_run`'s job, driven by
/// the user's own choice of notes and budget); it only decides which already-frozen, already
/// user-authorized queue gets packaged next.
///
/// `exclude_run_refs` lets a caller skip a run it already tried and failed to batch in the same
/// pass (see `model_runner::prepare_next_batch_across_runs`), so one run whose queued target can
/// never fit its configured model budget does not permanently block every run created after it.
pub async fn next_run_needing_batch(
    database: &Database,
    exclude_run_refs: &[Uuid],
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT run.run_ref FROM linggan_comment_study_run run \
         WHERE run.state IN ('prepared','queued','running') \
           AND run.selection_manifest->>'contract'='comment-study.run-selection.v1' \
           AND NOT (run.run_ref = ANY($1)) \
           AND EXISTS(SELECT 1 FROM linggan_comment_study_target target \
                      WHERE target.run_ref=run.run_ref AND target.state='queued') \
         ORDER BY run.created_at,run.run_ref LIMIT 1",
    )
    .bind(exclude_run_refs)
    .fetch_optional(database.pool())
    .await
}

async fn fit_targets_to_model_budget(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    work: &BatchWork,
    candidates: Vec<BatchTargetRow>,
) -> Result<Vec<BatchTargetRow>, StudyBatchError> {
    let limits: Option<(i32, i32)> = sqlx::query_as(
        "SELECT config.input_token_limit,config.output_token_limit \
         FROM linggan_comment_study_run run JOIN linggan_comment_study_policy policy USING(policy_ref) \
         JOIN linggan_model_config config ON config.config_ref=policy.model_config_ref WHERE run.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((input_limit, output_limit)) = limits else {
        return Ok(candidates);
    };
    let maximum_targets = usize::try_from((output_limit / 256).max(1)).unwrap_or(1);
    let mut selected = Vec::new();
    for candidate in candidates {
        if selected.len() >= maximum_targets {
            break;
        }
        let mut next = selected.clone();
        next.push(candidate);
        let probe = batch_manifest(Uuid::nil(), run_ref, work, &next);
        if conservative_token_estimate_json(&probe) > i64::from(input_limit) {
            break;
        }
        selected = next;
    }
    Ok(selected)
}

/// A provider-independent conservative estimator: every non-ASCII scalar costs two tokens,
/// ASCII runs cost one token per four bytes, and JSON framing has a fixed margin. It is used only
/// to avoid overpacking; the configured provider remains free to have a larger true context.
pub fn conservative_token_estimate_json(value: &Value) -> i64 {
    let text = value.to_string();
    let mut estimate = 256_i64;
    let mut ascii = 0_i64;
    for character in text.chars() {
        if character.is_ascii() {
            ascii += 1;
        } else {
            estimate += 2;
        }
    }
    estimate + (ascii + 3) / 4
}

/// Parses one batch response without writing it.
///
/// Only the envelope is judged for the batch as a whole: if the contract, batch, or work identity
/// is wrong, or the outer JSON will not parse, nothing in it can be attributed and the caller
/// rejects every target. Each result inside is then judged on its own. A single malformed result —
/// the provider once sent `"outcome":"experience"` — used to fail the deserialization of the whole
/// response and cost all twelve of its targets an attempt; here it costs only its own.
pub fn parse_batch_output(
    raw_output: Value,
    batch_ref: Uuid,
    content_public_ref: Uuid,
    target_texts: &BTreeMap<Uuid, String>,
) -> Result<ParsedBatchOutput, StudyBatchError> {
    let output: BatchOutput =
        serde_json::from_value(raw_output).map_err(|_| StudyBatchError::OutputSchema)?;
    if output.contract != BATCH_CONTRACT
        || output.batch_ref != batch_ref
        || output.content_public_ref != content_public_ref
    {
        return Err(StudyBatchError::OutputIdentityMismatch);
    }
    let mut accepted: BTreeMap<Uuid, ParsedBatchTarget> = BTreeMap::new();
    let mut rejected: BTreeMap<Uuid, &'static str> = BTreeMap::new();
    let mut addressed: BTreeSet<Uuid> = BTreeSet::new();
    let mut unattributable_result_count = 0;
    let mut unexpected_result_count = 0;
    for result in output.results {
        let Some(target_ref) = result
            .get("targetRef")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            unattributable_result_count += 1;
            continue;
        };
        let Some(source) = target_texts.get(&target_ref) else {
            unexpected_result_count += 1;
            continue;
        };
        if !addressed.insert(target_ref) {
            // Two results for one target cannot be told apart, so neither is admitted; taking the
            // last one would let a repeat silently overwrite an already valid result.
            accepted.remove(&target_ref);
            rejected.insert(target_ref, "semantic_contract");
            continue;
        }
        match serde_json::from_value::<BatchResult>(result)
            .map_err(|_| StudyBatchError::OutputSchema)
            .and_then(|result| parse_one_target(result, source))
        {
            Ok(target) => {
                accepted.insert(target_ref, target);
            }
            Err(error) => {
                rejected.insert(target_ref, target_rejection_code(&error));
            }
        }
    }
    let expected: BTreeSet<Uuid> = target_texts.keys().copied().collect();
    let missing_target_refs = expected.difference(&addressed).copied().collect();
    Ok(ParsedBatchOutput {
        targets: accepted.into_values().collect(),
        rejected_targets: rejected
            .into_iter()
            .map(|(target_ref, rejection_code)| RejectedBatchTarget {
                target_ref,
                rejection_code,
            })
            .collect(),
        missing_target_refs,
        unattributable_result_count,
        unexpected_result_count,
    })
}

fn target_rejection_code(error: &StudyBatchError) -> &'static str {
    match error {
        StudyBatchError::Semantic(error) => error.rejection_code(),
        StudyBatchError::OutputOutcomeMismatch => "semantic_contract",
        _ => "semantic_json_schema",
    }
}

async fn ensure_batchable_run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<(), StudyBatchError> {
    let state: Option<String> = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_study_run WHERE run_ref=$1 AND selection_manifest->>'contract'='comment-study.run-selection.v1' FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    match state.as_deref() {
        Some("prepared" | "queued" | "running") => Ok(()),
        _ => Err(StudyBatchError::RunUnavailable),
    }
}

async fn lock_next_work(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<BatchWork, StudyBatchError> {
    let row = sqlx::query(
        "SELECT target.content_public_ref,work.context_manifest \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_work work \
           ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         WHERE target.run_ref=$1 AND target.state='queued' \
         ORDER BY target.created_at,target.target_ref LIMIT 1 FOR UPDATE OF target SKIP LOCKED",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(StudyBatchError::NoQueuedTargets)?;
    Ok(BatchWork {
        content_public_ref: row.get("content_public_ref"),
        context_manifest: row.get("context_manifest"),
    })
}

async fn lock_work_targets(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    content_public_ref: Uuid,
    target_limit: i64,
) -> Result<Vec<BatchTargetRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT target.target_ref,target.source_ref,target.research_text,target.dependency_state,target.input_manifest \
         FROM linggan_comment_study_target target \
         WHERE target.run_ref=$1 AND target.content_public_ref=$2 AND target.state='queued' \
         ORDER BY target.created_at,target.target_ref LIMIT $3 FOR UPDATE SKIP LOCKED",
    )
    .bind(run_ref)
    .bind(content_public_ref)
    .bind(target_limit)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| BatchTargetRow {
            target_ref: row.get("target_ref"),
            source_ref: row.get("source_ref"),
            research_text: row.get("research_text"),
            dependency_state: row.get("dependency_state"),
            input_manifest: row.get("input_manifest"),
        })
        .collect())
}

fn batch_manifest(
    batch_ref: Uuid,
    run_ref: Uuid,
    work: &BatchWork,
    targets: &[BatchTargetRow],
) -> Value {
    json!({
        "contract":BATCH_CONTRACT,
        "batchRef":batch_ref,
        "runRef":run_ref,
        "workRef":work.content_public_ref,
        "workContext":work.context_manifest,
        "targets":targets.iter().map(|target| json!({
            "targetRef":target.target_ref,
            "sourceRef":target.source_ref,
            "researchText":target.research_text,
            "dependencyState":target.dependency_state,
            "parentContext":target.input_manifest["parentContext"],
        })).collect::<Vec<_>>(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchOutput {
    contract: String,
    batch_ref: Uuid,
    content_public_ref: Uuid,
    /// Left raw on purpose: deserializing the results as strict per-target structs here would put
    /// one bad result's error on the whole envelope again.
    results: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchResult {
    target_ref: Uuid,
    outcome: BatchOutcome,
    reason: Option<String>,
    signals: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BatchOutcome {
    Signals,
    NoSignal,
    NeedsContext,
}

fn parse_one_target(
    result: BatchResult,
    source: &str,
) -> Result<ParsedBatchTarget, StudyBatchError> {
    let semantic_output = json!({"contract":SEMANTIC_CONTRACT,"signals":result.signals});
    match result.outcome {
        BatchOutcome::Signals => {
            if result.reason.is_some() {
                return Err(StudyBatchError::OutputOutcomeMismatch);
            }
            let signals = accept_semantic_output(semantic_output, source)
                .map_err(StudyBatchError::Semantic)?;
            if signals.is_empty() {
                return Err(StudyBatchError::OutputOutcomeMismatch);
            }
            Ok(ParsedBatchTarget {
                target_ref: result.target_ref,
                state: BatchTargetState::Succeeded,
                signals,
                reason: None,
            })
        }
        BatchOutcome::NoSignal => {
            if !semantic_output["signals"]
                .as_array()
                .is_some_and(Vec::is_empty)
            {
                return Err(StudyBatchError::OutputOutcomeMismatch);
            }
            Ok(ParsedBatchTarget {
                target_ref: result.target_ref,
                state: BatchTargetState::NoSignal,
                signals: Vec::new(),
                reason: bounded_reason(result.reason)?,
            })
        }
        BatchOutcome::NeedsContext => {
            if !semantic_output["signals"]
                .as_array()
                .is_some_and(Vec::is_empty)
            {
                return Err(StudyBatchError::OutputOutcomeMismatch);
            }
            Ok(ParsedBatchTarget {
                target_ref: result.target_ref,
                state: BatchTargetState::NeedsContext,
                signals: Vec::new(),
                reason: bounded_reason(result.reason)?,
            })
        }
    }
}

fn bounded_reason(reason: Option<String>) -> Result<Option<String>, StudyBatchError> {
    let reason = reason.ok_or(StudyBatchError::OutputOutcomeMismatch)?;
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 200 {
        return Err(StudyBatchError::OutputOutcomeMismatch);
    }
    Ok(Some(trimmed.to_owned()))
}

fn hash_value(value: &Value) -> String {
    Sha256::digest(
        serde_json::to_string(value)
            .expect("JSON values serialize")
            .as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_signal(evidence: &str) -> Value {
        json!({
            "kind":"problem",
            "proposition":"孩子在家庭作业中存在自主启动困难。",
            "evidence":evidence,
            "problemFrame":{
                "actor":{"value":"评论者","basis":"我很着急"},
                "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
                "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                "context":{"value":"家庭作业","basis":"写作业"}
            }
        })
    }

    #[test]
    fn addressed_partial_batch_keeps_valid_target_and_marks_the_missing_target() {
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let source = "孩子每天写作业都要催，不催就不开始，我很着急。";
        let targets = BTreeMap::from([(first, source.to_owned()), (second, source.to_owned())]);
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[{"targetRef":first,"outcome":"signals","reason":null,
                "signals":[complete_signal("每天写作业都要催,不催就不开始")]}]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert_eq!(parsed.targets.len(), 1);
        assert_eq!(parsed.targets[0].state, BatchTargetState::Succeeded);
        assert_eq!(parsed.missing_target_refs, vec![second]);
    }

    #[test]
    fn no_signal_is_not_an_empty_or_omitted_result() {
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let target = Uuid::new_v4();
        let targets = BTreeMap::from([(target, "这个视频拍得真好。".to_owned())]);
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[{"targetRef":target,"outcome":"no_signal","reason":"未表达研究合同中的信号","signals":[]}]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert_eq!(parsed.targets[0].state, BatchTargetState::NoSignal);
        assert!(parsed.missing_target_refs.is_empty());
    }

    #[test]
    fn an_unknown_target_is_recorded_without_implicating_the_addressed_target() {
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let target = Uuid::new_v4();
        let targets = BTreeMap::from([(target, "这个视频拍得真好。".to_owned())]);
        let unknown = Uuid::new_v4();
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[
                {"targetRef":target,"outcome":"no_signal","reason":"未表达研究合同中的信号","signals":[]},
                {"targetRef":unknown,"outcome":"no_signal","reason":"未表达研究合同中的信号","signals":[]}
            ]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert_eq!(parsed.targets.len(), 1);
        assert_eq!(parsed.targets[0].target_ref, target);
        assert_eq!(parsed.unexpected_result_count, 1);
        assert!(parsed.rejected_targets.is_empty());
        assert!(parsed.missing_target_refs.is_empty());
    }

    #[test]
    fn a_repeated_target_admits_neither_copy() {
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let target = Uuid::new_v4();
        let source = "孩子每天写作业都要催，不催就不开始，我很着急。";
        let targets = BTreeMap::from([(target, source.to_owned())]);
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[
                {"targetRef":target,"outcome":"signals","reason":null,
                    "signals":[complete_signal("每天写作业都要催,不催就不开始")]},
                {"targetRef":target,"outcome":"no_signal","reason":"未表达研究合同中的信号","signals":[]}
            ]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert!(parsed.targets.is_empty());
        assert_eq!(parsed.rejected_targets.len(), 1);
        assert_eq!(parsed.rejected_targets[0].target_ref, target);
        assert_eq!(parsed.rejected_targets[0].rejection_code, "semantic_contract");
    }

    #[test]
    fn one_malformed_result_does_not_cost_its_siblings_an_attempt() {
        // The provider really sent `"outcome":"experience"` on 2026-09-17: a signal `kind` in the
        // outcome slot. Deserializing the whole response at once turned that single bad result
        // into a failed attempt for every target in its batch.
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let healthy = Uuid::new_v4();
        let malformed = Uuid::new_v4();
        let source = "孩子每天写作业都要催，不催就不开始，我很着急。";
        let targets = BTreeMap::from([
            (healthy, source.to_owned()),
            (malformed, source.to_owned()),
        ]);
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[
                {"targetRef":healthy,"outcome":"signals","reason":null,
                    "signals":[complete_signal("每天写作业都要催,不催就不开始")]},
                {"targetRef":malformed,"outcome":"experience","reason":null,"signals":[]}
            ]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert_eq!(parsed.targets.len(), 1);
        assert_eq!(parsed.targets[0].target_ref, healthy);
        assert_eq!(parsed.targets[0].state, BatchTargetState::Succeeded);
        assert_eq!(parsed.rejected_targets.len(), 1);
        assert_eq!(parsed.rejected_targets[0].target_ref, malformed);
        assert_eq!(
            parsed.rejected_targets[0].rejection_code,
            "semantic_json_schema"
        );
        assert!(parsed.missing_target_refs.is_empty());
    }

    #[test]
    fn a_result_without_a_readable_target_ref_blames_no_target() {
        let batch_ref = Uuid::new_v4();
        let work_ref = Uuid::new_v4();
        let target = Uuid::new_v4();
        let targets = BTreeMap::from([(target, "这个视频拍得真好。".to_owned())]);
        let output = json!({
            "contract":BATCH_CONTRACT,
            "batchRef":batch_ref,
            "contentPublicRef":work_ref,
            "results":[{"outcome":"no_signal","reason":"缺少 targetRef","signals":[]}]
        });
        let parsed = parse_batch_output(output, batch_ref, work_ref, &targets).unwrap();
        assert_eq!(parsed.unattributable_result_count, 1);
        assert!(parsed.rejected_targets.is_empty());
        assert_eq!(parsed.missing_target_refs, vec![target]);
    }
}
