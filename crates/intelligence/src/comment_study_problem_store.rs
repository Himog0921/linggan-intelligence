//! Transactional persistence for the clean comment-study Problem lifecycle.
//!
//! The caller supplies only a server-selected candidate set. This module freezes that set before
//! any model comparison, and it writes a membership only after the deterministic resolver admits
//! exactly one match. Pair creation is deliberately separate from no-match handling.

use crate::comment_study_canonical::{canonical_hash, canonical_text};
use crate::comment_study_problem_resolution::{
    ExistingResolutionDecision, NewProblemDefinition, PROBLEM_PAIR_CONTRACT, PairCreationDecision,
    ProblemResolutionContractError, decide_existing_resolution, decide_pair_creation,
};
use crate::comment_study_request_ledger::ProblemStageSubject;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

const MAX_SERVER_CANDIDATES: usize = 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedProblemResolution {
    pub resolution_ref: Uuid,
    pub signal_ref: Uuid,
    pub state: String,
    pub candidate_problem_refs: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemResolutionReceipt {
    pub resolution_ref: Uuid,
    pub state: String,
    pub problem_ref: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedProblemPair {
    pub pair_ref: Uuid,
    pub first_signal_ref: Uuid,
    pub second_signal_ref: Uuid,
}

/// Facts about how the server selected a new-Problem comparison. These are selection provenance,
/// not a similarity threshold or a verdict.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairSelection {
    pub profile_ref: Uuid,
    pub recall_rank: i64,
    pub admissible_rank: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemPairReceipt {
    pub pair_ref: Uuid,
    pub state: String,
    pub problem_ref: Option<Uuid>,
    pub decision_reason: String,
}

#[derive(Debug, Error)]
pub enum ProblemStoreError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Contract(#[from] ProblemResolutionContractError),
    #[error("the Signal is absent, is not a Problem or Need, or already has a resolution")]
    SignalUnavailable,
    #[error("the Resolution is absent or is not awaiting a candidate comparison")]
    ResolutionUnavailable,
    #[error("the pair is absent or is not awaiting a shared-definition comparison")]
    PairUnavailable,
    #[error("the model response no longer belongs to an active, unexpired request")]
    ModelRequestUnavailable,
    #[error(
        "a candidate Problem is absent, retired, from another domain, duplicated, or exceeds the maximum candidate count"
    )]
    InvalidCandidateSet,
    #[error("only eligible Problem or Need Signals may receive a candidate comparison")]
    SignalNotEligible,
    #[error("the owning Run is not an enabled v2 Run")]
    RunUnavailable,
    #[error(
        "a new Problem pair requires two independently authored source comments that are both deferred as novel"
    )]
    PairNotIndependentOrNovel,
    #[error("the stored candidate or pair manifest is malformed")]
    StoredManifest,
}

struct SignalForResolution {
    signal_ref: Uuid,
    domain_ref: Uuid,
    eligibility_state: String,
}

struct NovelSignal {
    signal_ref: Uuid,
    domain_ref: Uuid,
    source_ref: Uuid,
    author_external_id: Option<String>,
}

pub(crate) fn independently_authored(first: Option<&str>, second: Option<&str>) -> bool {
    let first = first.map(str::trim).filter(|id| !id.is_empty());
    let second = second.map(str::trim).filter(|id| !id.is_empty());
    match (first, second) {
        (Some(first), Some(second)) => first != second,
        _ => false,
    }
}

/// Freezes a server-selected candidate set. A deferred-context or not-user-problem Signal is
/// terminally categorized without invoking a comparison model.
pub async fn prepare_problem_resolution(
    database: &Database,
    signal_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    prepare_problem_resolution_inner(database, signal_ref, server_candidate_refs, false).await
}

/// Scheduler variant that serializes stage creation with Run stop and budget exhaustion.
pub async fn prepare_problem_resolution_for_enabled_v2_run(
    database: &Database,
    signal_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    prepare_problem_resolution_inner(database, signal_ref, server_candidate_refs, true).await
}

async fn prepare_problem_resolution_inner(
    database: &Database,
    signal_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
    enabled_v2_only: bool,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    let candidates = normalized_candidates(server_candidate_refs)?;
    let mut transaction = database.pool().begin().await?;
    if enabled_v2_only {
        lock_enabled_v2_run_for_signal(&mut transaction, signal_ref).await?;
    }
    let signal = lock_signal_for_resolution(&mut transaction, signal_ref).await?;
    let (state, candidate_problem_refs, candidate_problem_revisions) =
        match signal.eligibility_state.as_str() {
            "eligible" => {
                let revisions =
                    freeze_candidate_revisions(&mut transaction, signal.domain_ref, &candidates)
                        .await?;
                ("pending", candidates, revisions)
            }
            "deferred_context" => ("deferred_context", Vec::new(), Vec::new()),
            "not_user_problem" => ("not_user_problem", Vec::new(), Vec::new()),
            _ => return Err(ProblemStoreError::SignalNotEligible),
        };
    let resolution_ref = Uuid::new_v4();
    let manifest = json!({
        "contract":"comment-study.problem-candidate-set.v1",
        "candidateProblemRefs":candidate_problem_refs,
        "candidateProblemRevisions":candidate_problem_revisions,
    });
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,resolved_at \
         ) VALUES($1,$2,$3,$4,$5,CASE WHEN $4='pending' THEN NULL ELSE scope_001_now() END)",
    )
    .bind(resolution_ref)
    .bind(signal.signal_ref)
    .bind(signal.domain_ref)
    .bind(state)
    .bind(manifest)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(PreparedProblemResolution {
        resolution_ref,
        signal_ref,
        state: state.to_owned(),
        candidate_problem_refs,
    })
}

/// Closes a resolution that never got a trustworthy candidate set.
///
/// This exists so that "recall could not cover the catalogue" can never be recorded as "compared
/// and found nothing". The two look identical downstream — an empty candidate list — but only the
/// second is evidence of novelty. Recording the first as novel is how a duplicate Problem gets
/// created from a catalogue that merely was not searchable.
pub async fn resolve_retrieval_incomplete(
    database: &Database,
    resolution_ref: Uuid,
    reason: &str,
) -> Result<ProblemResolutionReceipt, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    lock_pending_resolution(&mut transaction, resolution_ref, None).await?;
    finish_resolution(
        &mut transaction,
        resolution_ref,
        "retrieval_incomplete",
        None,
        json!({
            "contract":"comment-study.problem-candidate-set.v1",
            "retrievalIncompleteReason":reason
        }),
    )
    .await?;
    transaction.commit().await?;
    Ok(ProblemResolutionReceipt {
        resolution_ref,
        state: "retrieval_incomplete".to_owned(),
        problem_ref: None,
    })
}

/// Reopens a previously incomplete recall after its search prerequisites have changed.
///
/// The Resolution keeps its identity because this is not a second conclusion about the Signal: it
/// is the same comparison that could not previously cover the catalogue. The prior incomplete
/// receipt remains attached to the refreshed candidate manifest rather than being silently lost.
pub async fn resume_retrieval_incomplete_resolution(
    database: &Database,
    resolution_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    resume_retrieval_incomplete_resolution_inner(
        database,
        resolution_ref,
        server_candidate_refs,
        false,
    )
    .await
}

/// Scheduler variant that cannot reopen work after the Run has stopped.
pub async fn resume_retrieval_incomplete_resolution_for_enabled_v2_run(
    database: &Database,
    resolution_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    resume_retrieval_incomplete_resolution_inner(
        database,
        resolution_ref,
        server_candidate_refs,
        true,
    )
    .await
}

async fn resume_retrieval_incomplete_resolution_inner(
    database: &Database,
    resolution_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
    enabled_v2_only: bool,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    let candidates = normalized_candidates(server_candidate_refs)?;
    let mut transaction = database.pool().begin().await?;
    if enabled_v2_only {
        lock_enabled_v2_run_for_resolution(&mut transaction, resolution_ref).await?;
    }
    let resolution = lock_retrieval_incomplete_resolution(&mut transaction, resolution_ref).await?;
    let candidate_problem_revisions =
        freeze_candidate_revisions(&mut transaction, resolution.domain_ref, &candidates).await?;
    let manifest = json!({
        "contract":"comment-study.problem-candidate-set.v1",
        "candidateProblemRefs":candidates,
        "candidateProblemRevisions":candidate_problem_revisions,
        "priorRetrievalIncomplete":resolution.decision_manifest,
    });
    sqlx::query(
        "UPDATE linggan_comment_study_resolution \
         SET state='pending',candidate_manifest=$2,decision_manifest=NULL,resolved_problem_ref=NULL,resolved_at=NULL \
         WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .bind(manifest)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(PreparedProblemResolution {
        resolution_ref,
        signal_ref: resolution.signal_ref,
        state: "pending".to_owned(),
        candidate_problem_refs: candidates,
    })
}

/// Accepts a closed candidate comparison. A malformed model output is recorded as a protocol
/// rejection, never reinterpreted as “there was no matching Problem”.
pub async fn accept_problem_resolution(
    database: &Database,
    resolution_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemResolutionReceipt, ProblemStoreError> {
    accept_problem_resolution_inner(database, resolution_ref, None, raw_output).await
}

#[doc(hidden)]
pub async fn accept_problem_resolution_from_invocation(
    database: &Database,
    resolution_ref: Uuid,
    invocation_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemResolutionReceipt, ProblemStoreError> {
    accept_problem_resolution_inner(database, resolution_ref, Some(invocation_ref), raw_output)
        .await
}

async fn accept_problem_resolution_inner(
    database: &Database,
    resolution_ref: Uuid,
    expected_invocation: Option<Uuid>,
    raw_output: Value,
) -> Result<ProblemResolutionReceipt, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    let row = if let Some(invocation_ref) = expected_invocation {
        let run_ref = lock_problem_stage_run(
            &mut transaction,
            ProblemStageSubject::Resolution(resolution_ref),
            invocation_ref,
        )
        .await?;
        let row = lock_pending_resolution(&mut transaction, resolution_ref, Some(invocation_ref))
            .await
            .map_err(|error| match error {
                ProblemStoreError::ResolutionUnavailable => {
                    ProblemStoreError::ModelRequestUnavailable
                }
                other => other,
            })?;
        lock_active_problem_stage_request(
            &mut transaction,
            ProblemStageSubject::Resolution(resolution_ref),
            run_ref,
            invocation_ref,
        )
        .await?;
        row
    } else {
        lock_pending_resolution(&mut transaction, resolution_ref, None).await?
    };
    let current: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_effective_signal \
         WHERE signal_ref=$1 AND eligibility_state='eligible')",
    )
    .bind(row.signal_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if !current {
        finish_resolution(
            &mut transaction,
            resolution_ref,
            "failed",
            None,
            json!({"reason":"source_superseded"}),
        )
        .await?;
        transaction.commit().await?;
        return Err(ProblemStoreError::SignalUnavailable);
    }
    let candidates = candidate_refs(&row.candidate_manifest)?;
    let decision = match decide_existing_resolution(raw_output.clone(), &candidates) {
        Ok(decision) => decision,
        Err(error) => {
            finish_resolution(
                &mut transaction,
                resolution_ref,
                "protocol_rejected",
                None,
                raw_output,
            )
            .await?;
            transaction.commit().await?;
            return Err(ProblemStoreError::Contract(error));
        }
    };
    let receipt = match decision {
        ExistingResolutionDecision::Assign(problem_ref) => {
            validate_candidates(&mut transaction, row.domain_ref, &[problem_ref]).await?;
            let mut decision_manifest = raw_output.clone();
            let problem_revision_ref = if membership_revision_supported(&mut transaction).await? {
                let revision_ref = candidate_revision_ref(&row.candidate_manifest, problem_ref)?;
                decision_manifest
                    .as_object_mut()
                    .ok_or(ProblemStoreError::StoredManifest)?
                    .insert("problemRevisionRef".into(), json!(revision_ref));
                Some(revision_ref)
            } else {
                None
            };
            finish_resolution(
                &mut transaction,
                resolution_ref,
                "assigned",
                Some(problem_ref),
                decision_manifest,
            )
            .await?;
            if let Some(problem_revision_ref) = problem_revision_ref {
                sqlx::query(
                    "INSERT INTO linggan_comment_study_problem_membership( \
                       membership_ref,signal_ref,problem_ref,resolution_ref,problem_revision_ref \
                     ) VALUES($1,$2,$3,$4,$5)",
                )
                .bind(Uuid::new_v4())
                .bind(row.signal_ref)
                .bind(problem_ref)
                .bind(resolution_ref)
                .bind(problem_revision_ref)
                .execute(&mut *transaction)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO linggan_comment_study_problem_membership( \
                       membership_ref,signal_ref,problem_ref,resolution_ref \
                     ) VALUES($1,$2,$3,$4)",
                )
                .bind(Uuid::new_v4())
                .bind(row.signal_ref)
                .bind(problem_ref)
                .bind(resolution_ref)
                .execute(&mut *transaction)
                .await?;
            }
            ProblemResolutionReceipt {
                resolution_ref,
                state: "assigned".to_owned(),
                problem_ref: Some(problem_ref),
            }
        }
        ExistingResolutionDecision::DeferAmbiguous => {
            finish_resolution(
                &mut transaction,
                resolution_ref,
                "deferred_ambiguous",
                None,
                raw_output,
            )
            .await?;
            ProblemResolutionReceipt {
                resolution_ref,
                state: "deferred_ambiguous".to_owned(),
                problem_ref: None,
            }
        }
        ExistingResolutionDecision::DeferNovel => {
            finish_resolution(
                &mut transaction,
                resolution_ref,
                "deferred_novel",
                None,
                raw_output,
            )
            .await?;
            ProblemResolutionReceipt {
                resolution_ref,
                state: "deferred_novel".to_owned(),
                problem_ref: None,
            }
        }
    };
    transaction.commit().await?;
    Ok(receipt)
}

/// Opens a pair only for two independently authored, no-match Signals. Pairing the same author,
/// the same source comment, or a non-novel result is refused before any model work begins.
pub async fn prepare_problem_pair(
    database: &Database,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    selection: PairSelection,
) -> Result<PreparedProblemPair, ProblemStoreError> {
    prepare_problem_pair_inner(
        database,
        first_signal_ref,
        second_signal_ref,
        selection,
        false,
    )
    .await
}

/// Scheduler variant that serializes pair creation with Run stop and budget exhaustion.
pub async fn prepare_problem_pair_for_enabled_v2_run(
    database: &Database,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    selection: PairSelection,
) -> Result<PreparedProblemPair, ProblemStoreError> {
    prepare_problem_pair_inner(
        database,
        first_signal_ref,
        second_signal_ref,
        selection,
        true,
    )
    .await
}

async fn prepare_problem_pair_inner(
    database: &Database,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    selection: PairSelection,
    enabled_v2_only: bool,
) -> Result<PreparedProblemPair, ProblemStoreError> {
    if first_signal_ref == second_signal_ref {
        return Err(ProblemStoreError::PairNotIndependentOrNovel);
    }
    let (first_ref, second_ref) = ordered_pair(first_signal_ref, second_signal_ref);
    let mut transaction = database.pool().begin().await?;
    if enabled_v2_only {
        lock_enabled_v2_run_for_pair(&mut transaction, first_ref, second_ref).await?;
    }
    let first = lock_novel_signal(&mut transaction, first_ref).await?;
    let second = lock_novel_signal(&mut transaction, second_ref).await?;
    // The row locks serialize competing pair preparations. Recheck after acquiring them so a
    // comparison committed while we waited cannot give either Signal a second primary pair.
    let already_paired: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_problem_pair pair \
         WHERE pair.first_signal_ref=ANY($1::uuid[]) \
            OR pair.second_signal_ref=ANY($1::uuid[]))",
    )
    .bind(vec![first_ref, second_ref])
    .fetch_one(&mut *transaction)
    .await?;
    if first.domain_ref != second.domain_ref
        || first.source_ref == second.source_ref
        || already_paired
        || !independently_authored(
            first.author_external_id.as_deref(),
            second.author_external_id.as_deref(),
        )
    {
        return Err(ProblemStoreError::PairNotIndependentOrNovel);
    }
    let pair_ref = Uuid::new_v4();
    let manifest = json!({
        "contract":"comment-study.problem-pair-input.v1",
        "outputContractVersion":"v2",
        "domainRef":first.domain_ref,
        "first":{"signalRef":first.signal_ref,"sourceRef":first.source_ref},
        "second":{"signalRef":second.signal_ref,"sourceRef":second.source_ref},
        "independentSources":true,
        "independentAuthors":true,
        "selection":{
            "contract":"comment-study.problem-pair-selection.v1",
            "method":"nearest_admissible",
            "embeddingProfileRef":selection.profile_ref,
            "recallRank":selection.recall_rank,
            "admissibleRank":selection.admissible_rank,
        },
    });
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_pair( \
           pair_ref,first_signal_ref,second_signal_ref,state,pair_manifest \
         ) VALUES($1,$2,$3,'pending',$4)",
    )
    .bind(pair_ref)
    .bind(first.signal_ref)
    .bind(second.signal_ref)
    .bind(manifest)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(PreparedProblemPair {
        pair_ref,
        first_signal_ref: first.signal_ref,
        second_signal_ref: second.signal_ref,
    })
}

#[cfg(test)]
mod author_independence_tests {
    use super::independently_authored;

    #[test]
    fn independent_pair_requires_both_known_nonblank_and_distinct_accounts() {
        assert!(independently_authored(Some("reader-a"), Some("reader-b")));
        assert!(!independently_authored(Some("reader-a"), Some("reader-a")));
        assert!(!independently_authored(Some("reader-a"), None));
        assert!(!independently_authored(None, Some("reader-a")));
        assert!(!independently_authored(None, None));
        assert!(!independently_authored(Some("  "), Some("reader-a")));
        assert!(!independently_authored(Some("reader-a"), Some(" \t")));
    }
}

/// Gives a pair one bounded retry when its earlier provider output was rejected solely because it
/// predated the current output contract. The failed invocation receipt remains immutable; only the
/// pair's claim is reopened, and only once per output-contract version.
pub async fn resume_pre_v2_pair_contract_rejection(
    database: &Database,
) -> Result<bool, ProblemStoreError> {
    resume_pre_v2_pair_contract_rejection_inner(database, false).await
}

/// Scheduler variant that only reopens a contract-rejected pair owned by an enabled v2 Run.
pub async fn resume_pre_v2_pair_contract_rejection_for_enabled_v2_run(
    database: &Database,
) -> Result<bool, ProblemStoreError> {
    resume_pre_v2_pair_contract_rejection_inner(database, true).await
}

async fn resume_pre_v2_pair_contract_rejection_inner(
    database: &Database,
    enabled_v2_only: bool,
) -> Result<bool, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    let pair_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT pair.pair_ref FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_model_invocation invocation ON invocation.invocation_ref=pair.model_invocation_ref \
         JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         WHERE pair.state='rejected' AND invocation.failure_code='pair_contract_rejected' \
           AND pair.pair_manifest->>'outputContractVersion' IS NULL \
           AND (NOT $1 OR (run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
             AND to_jsonb(run)->>'dispatch_state'='enabled' \
             AND to_jsonb(run)->>'dispatch_reason' IS NULL)) \
         ORDER BY pair.created_at,pair.pair_ref LIMIT 1",
    )
    .bind(enabled_v2_only)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(pair_ref) = pair_ref else {
        transaction.commit().await?;
        return Ok(false);
    };
    if enabled_v2_only {
        lock_enabled_v2_run_for_pair_ref(&mut transaction, pair_ref).await?;
    }
    let locked_pair_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT pair.pair_ref FROM linggan_comment_study_problem_pair pair \
         JOIN linggan_model_invocation invocation ON invocation.invocation_ref=pair.model_invocation_ref \
         JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         WHERE pair.pair_ref=$1 AND pair.state='rejected' \
           AND invocation.failure_code='pair_contract_rejected' \
           AND pair.pair_manifest->>'outputContractVersion' IS NULL \
           AND (NOT $2 OR (run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
             AND to_jsonb(run)->>'dispatch_state'='enabled' \
             AND to_jsonb(run)->>'dispatch_reason' IS NULL)) \
         FOR UPDATE OF pair SKIP LOCKED",
    )
    .bind(pair_ref)
    .bind(enabled_v2_only)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked_pair_ref.is_none() {
        transaction.commit().await?;
        return Ok(false);
    }
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair \
         SET state='pending',model_invocation_ref=NULL,resolved_at=NULL,proposed_problem=NULL, \
             pair_manifest=jsonb_set(pair_manifest,'{outputContractVersion}','\"v2\"'::jsonb) \
         WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(true)
}

/// Creates a durable Problem and assigns both Signals only when the pair contract and independent
/// source checks admit it. Any non-create decision leaves both original Signals visible but
/// unassigned.
pub async fn accept_problem_pair(
    database: &Database,
    pair_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemPairReceipt, ProblemStoreError> {
    accept_problem_pair_inner(database, pair_ref, None, raw_output).await
}

#[doc(hidden)]
pub async fn accept_problem_pair_from_invocation(
    database: &Database,
    pair_ref: Uuid,
    invocation_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemPairReceipt, ProblemStoreError> {
    accept_problem_pair_inner(database, pair_ref, Some(invocation_ref), raw_output).await
}

async fn accept_problem_pair_inner(
    database: &Database,
    pair_ref: Uuid,
    expected_invocation: Option<Uuid>,
    raw_output: Value,
) -> Result<ProblemPairReceipt, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    let pair = if let Some(invocation_ref) = expected_invocation {
        let run_ref = lock_problem_stage_run(
            &mut transaction,
            ProblemStageSubject::Pair(pair_ref),
            invocation_ref,
        )
        .await?;
        let pair = lock_pending_pair(&mut transaction, pair_ref, Some(invocation_ref))
            .await
            .map_err(|error| match error {
                ProblemStoreError::PairUnavailable => ProblemStoreError::ModelRequestUnavailable,
                other => other,
            })?;
        lock_active_problem_stage_request(
            &mut transaction,
            ProblemStageSubject::Pair(pair_ref),
            run_ref,
            invocation_ref,
        )
        .await?;
        pair
    } else {
        lock_pending_pair(&mut transaction, pair_ref, None).await?
    };
    let current_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_effective_signal \
         WHERE signal_ref=ANY($1) AND eligibility_state='eligible'",
    )
    .bind(vec![pair.first_signal_ref, pair.second_signal_ref])
    .fetch_one(&mut *transaction)
    .await?;
    if current_count != 2 {
        finish_pair(
            &mut transaction,
            pair_ref,
            "failed",
            None,
            "source_superseded",
            json!({}),
        )
        .await?;
        transaction.commit().await?;
        return Err(ProblemStoreError::SignalUnavailable);
    }
    let (domain_ref, independent_sources, frozen_independent_authors) =
        pair_manifest(&pair.manifest)?;
    let current_independent_authors: bool = sqlx::query_scalar(
        "SELECT first.domain_ref=second.domain_ref AND first.domain_ref=$3 \
           AND btrim(first.current_author_external_id)<>btrim(second.current_author_external_id) \
         FROM linggan_comment_study_effective_signal first \
         JOIN linggan_comment_study_effective_signal second ON second.signal_ref=$2 \
         WHERE first.signal_ref=$1 AND first.eligibility_state='eligible' \
           AND second.eligibility_state='eligible'",
    )
    .bind(pair.first_signal_ref)
    .bind(pair.second_signal_ref)
    .bind(domain_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .unwrap_or(false);
    let decision = match decide_pair_creation(
        raw_output.clone(),
        pair.first_signal_ref,
        pair.second_signal_ref,
        independent_sources,
        frozen_independent_authors && current_independent_authors,
    ) {
        Ok(decision) => decision,
        Err(error) => {
            let decision_reason = pair_contract_failure_code(&error);
            finish_pair(
                &mut transaction,
                pair_ref,
                "rejected",
                None,
                decision_reason,
                raw_output,
            )
            .await?;
            transaction.commit().await?;
            return Err(ProblemStoreError::Contract(error));
        }
    };
    let receipt = match decision {
        PairCreationDecision::Create(definition) => {
            let (problem_ref, problem_revision_ref) = insert_or_find_problem(
                &mut transaction,
                domain_ref,
                &definition,
                &[pair.first_signal_ref, pair.second_signal_ref],
            )
            .await?;
            assign_novel_signal_from_pair(
                &mut transaction,
                pair.first_signal_ref,
                problem_ref,
                problem_revision_ref,
                pair_ref,
            )
            .await?;
            assign_novel_signal_from_pair(
                &mut transaction,
                pair.second_signal_ref,
                problem_ref,
                problem_revision_ref,
                pair_ref,
            )
            .await?;
            finish_pair(
                &mut transaction,
                pair_ref,
                "approved",
                Some(problem_ref),
                "approved",
                raw_output,
            )
            .await?;
            ProblemPairReceipt {
                pair_ref,
                state: "approved".to_owned(),
                problem_ref: Some(problem_ref),
                decision_reason: "approved".to_owned(),
            }
        }
        PairCreationDecision::DeferInsufficientIndependentEvidence => {
            finish_pair(
                &mut transaction,
                pair_ref,
                "rejected",
                None,
                "insufficient_independent_evidence",
                raw_output,
            )
            .await?;
            ProblemPairReceipt {
                pair_ref,
                state: "rejected".to_owned(),
                problem_ref: None,
                decision_reason: "insufficient_independent_evidence".to_owned(),
            }
        }
        PairCreationDecision::DeferAmbiguous => {
            finish_pair(
                &mut transaction,
                pair_ref,
                "rejected",
                None,
                "ambiguous",
                raw_output,
            )
            .await?;
            ProblemPairReceipt {
                pair_ref,
                state: "rejected".to_owned(),
                problem_ref: None,
                decision_reason: "ambiguous".to_owned(),
            }
        }
        PairCreationDecision::DeferNotSameProblem => {
            finish_pair(
                &mut transaction,
                pair_ref,
                "rejected",
                None,
                "not_same_problem",
                raw_output,
            )
            .await?;
            ProblemPairReceipt {
                pair_ref,
                state: "rejected".to_owned(),
                problem_ref: None,
                decision_reason: "not_same_problem".to_owned(),
            }
        }
    };
    transaction.commit().await?;
    Ok(receipt)
}

async fn lock_signal_for_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
) -> Result<SignalForResolution, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT signal.signal_ref,run_policy.domain_ref,signal.eligibility_state,signal.kind \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run USING(run_ref) \
         JOIN linggan_comment_study_policy run_policy ON run_policy.policy_ref=run.policy_ref \
         LEFT JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         WHERE signal.signal_ref=$1 AND resolution.signal_ref IS NULL \
           AND EXISTS(SELECT 1 FROM linggan_comment_study_effective_signal effective \
                      WHERE effective.signal_ref=signal.signal_ref) \
         FOR UPDATE OF signal",
    )
    .bind(signal_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::SignalUnavailable)?;
    let kind: String = row.get("kind");
    if !matches!(kind.as_str(), "problem" | "need") {
        return Err(ProblemStoreError::SignalUnavailable);
    }
    Ok(SignalForResolution {
        signal_ref: row.get("signal_ref"),
        domain_ref: row.get("domain_ref"),
        eligibility_state: row.get("eligibility_state"),
    })
}

async fn validate_candidates(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
    candidates: &[Uuid],
) -> Result<(), ProblemStoreError> {
    if candidates.len() > MAX_SERVER_CANDIDATES
        || candidates.iter().copied().collect::<BTreeSet<_>>().len() != candidates.len()
    {
        return Err(ProblemStoreError::InvalidCandidateSet);
    }
    let valid_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_current_problem \
         WHERE domain_ref=$1 AND state IN ('active','support_insufficient') AND problem_ref=ANY($2)",
    )
    .bind(domain_ref)
    .bind(candidates)
    .fetch_one(&mut **transaction)
    .await?;
    if usize::try_from(valid_count).unwrap_or(0) != candidates.len() {
        return Err(ProblemStoreError::InvalidCandidateSet);
    }
    Ok(())
}

async fn membership_revision_supported(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns \
         WHERE table_schema=current_schema() \
           AND table_name='linggan_comment_study_problem_membership' \
           AND column_name='problem_revision_ref')",
    )
    .fetch_one(&mut **transaction)
    .await
}

async fn freeze_candidate_revisions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
    candidates: &[Uuid],
) -> Result<Vec<Value>, ProblemStoreError> {
    validate_candidates(transaction, domain_ref, candidates).await?;
    let rows = sqlx::query(
        "SELECT problem.problem_ref,revision.revision_ref \
         FROM linggan_comment_study_current_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
          AND revision.problem_ref=problem.problem_ref \
         WHERE problem.domain_ref=$1 AND problem.state IN ('active','support_insufficient') \
           AND problem.problem_ref=ANY($2) \
         ORDER BY problem.problem_ref",
    )
    .bind(domain_ref)
    .bind(candidates)
    .fetch_all(&mut **transaction)
    .await?;
    if rows.len() != candidates.len() {
        return Err(ProblemStoreError::InvalidCandidateSet);
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "problemRef":row.get::<Uuid,_>("problem_ref"),
                "problemRevisionRef":row.get::<Uuid,_>("revision_ref")
            })
        })
        .collect())
}

struct PendingResolution {
    signal_ref: Uuid,
    domain_ref: Uuid,
    candidate_manifest: Value,
}

struct RetrievalIncompleteResolution {
    signal_ref: Uuid,
    domain_ref: Uuid,
    decision_manifest: Value,
}

async fn lock_retrieval_incomplete_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolution_ref: Uuid,
) -> Result<RetrievalIncompleteResolution, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT resolution.signal_ref,resolution.domain_ref,resolution.decision_manifest,signal.kind,signal.eligibility_state \
         FROM linggan_comment_study_resolution resolution \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         WHERE resolution.resolution_ref=$1 AND resolution.state='retrieval_incomplete' \
         FOR UPDATE OF resolution,signal",
    )
    .bind(resolution_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::ResolutionUnavailable)?;
    let kind: String = row.get("kind");
    let eligibility_state: String = row.get("eligibility_state");
    if !matches!(kind.as_str(), "problem" | "need") || eligibility_state != "eligible" {
        return Err(ProblemStoreError::SignalNotEligible);
    }
    Ok(RetrievalIncompleteResolution {
        signal_ref: row.get("signal_ref"),
        domain_ref: row.get("domain_ref"),
        decision_manifest: row.get("decision_manifest"),
    })
}

async fn lock_pending_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolution_ref: Uuid,
    expected_invocation: Option<Uuid>,
) -> Result<PendingResolution, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT signal_ref,domain_ref,candidate_manifest \
         FROM linggan_comment_study_resolution \
         WHERE resolution_ref=$1 AND state='pending' \
           AND (($2::uuid IS NULL AND model_invocation_ref IS NULL) \
                OR model_invocation_ref=$2) FOR UPDATE",
    )
    .bind(resolution_ref)
    .bind(expected_invocation)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::ResolutionUnavailable)?;
    Ok(PendingResolution {
        signal_ref: row.get("signal_ref"),
        domain_ref: row.get("domain_ref"),
        candidate_manifest: row.get("candidate_manifest"),
    })
}

async fn finish_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolution_ref: Uuid,
    state: &str,
    problem_ref: Option<Uuid>,
    decision_manifest: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE linggan_comment_study_resolution \
         SET state=$2,resolved_problem_ref=$3,decision_manifest=$4,resolved_at=scope_001_now() \
         WHERE resolution_ref=$1",
    )
    .bind(resolution_ref)
    .bind(state)
    .bind(problem_ref)
    .bind(decision_manifest)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn candidate_refs(manifest: &Value) -> Result<Vec<Uuid>, ProblemStoreError> {
    if manifest.get("contract").and_then(Value::as_str)
        != Some("comment-study.problem-candidate-set.v1")
    {
        return Err(ProblemStoreError::StoredManifest);
    }
    manifest
        .get("candidateProblemRefs")
        .and_then(Value::as_array)
        .ok_or(ProblemStoreError::StoredManifest)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or(ProblemStoreError::StoredManifest)
                .and_then(|value| {
                    Uuid::parse_str(value).map_err(|_| ProblemStoreError::StoredManifest)
                })
        })
        .collect()
}

fn candidate_revision_ref(manifest: &Value, problem_ref: Uuid) -> Result<Uuid, ProblemStoreError> {
    if !candidate_refs(manifest)?.contains(&problem_ref) {
        return Err(ProblemStoreError::StoredManifest);
    }
    let problem_ref = problem_ref.to_string();
    let revisions = manifest
        .get("candidateProblemRevisions")
        .and_then(Value::as_array)
        .ok_or(ProblemStoreError::StoredManifest)?;
    let matches: Vec<Uuid> = revisions
        .iter()
        .filter(|candidate| {
            candidate.get("problemRef").and_then(Value::as_str) == Some(problem_ref.as_str())
        })
        .map(|candidate| {
            candidate
                .get("problemRevisionRef")
                .and_then(Value::as_str)
                .ok_or(ProblemStoreError::StoredManifest)
                .and_then(|value| {
                    Uuid::parse_str(value).map_err(|_| ProblemStoreError::StoredManifest)
                })
        })
        .collect::<Result<_, _>>()?;
    if matches.len() != 1 {
        return Err(ProblemStoreError::StoredManifest);
    }
    Ok(matches[0])
}

async fn lock_novel_signal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
) -> Result<NovelSignal, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT signal.signal_ref,policy.domain_ref,target.source_ref, \
                effective.current_author_external_id AS author_external_id \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_effective_signal effective USING(signal_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
         LEFT JOIN linggan_comment_study_problem_membership membership USING(signal_ref) \
         WHERE signal.signal_ref=$1 AND signal.kind IN ('problem','need') \
           AND signal.eligibility_state='eligible' AND resolution.state='deferred_novel' \
           AND membership.signal_ref IS NULL \
         FOR UPDATE OF signal,resolution",
    )
    .bind(signal_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::PairNotIndependentOrNovel)?;
    Ok(NovelSignal {
        signal_ref: row.get("signal_ref"),
        domain_ref: row.get("domain_ref"),
        source_ref: row.get("source_ref"),
        author_external_id: row.get("author_external_id"),
    })
}

async fn lock_enabled_v2_run_for_signal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    let run_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT target.run_ref FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) WHERE signal.signal_ref=$1",
    )
    .bind(signal_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let run_ref = run_ref.ok_or(ProblemStoreError::RunUnavailable)?;
    lock_enabled_v2_run(transaction, run_ref).await
}

async fn lock_enabled_v2_run_for_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolution_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    let run_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT target.run_ref FROM linggan_comment_study_resolution resolution \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         WHERE resolution.resolution_ref=$1",
    )
    .bind(resolution_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let run_ref = run_ref.ok_or(ProblemStoreError::RunUnavailable)?;
    lock_enabled_v2_run(transaction, run_ref).await
}

async fn lock_enabled_v2_run_for_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    let run_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT target.run_ref FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         WHERE signal.signal_ref=ANY($1) ORDER BY target.run_ref",
    )
    .bind(vec![first_signal_ref, second_signal_ref])
    .fetch_all(&mut **transaction)
    .await?;
    if run_refs.len() != 1 {
        return Err(ProblemStoreError::RunUnavailable);
    }
    lock_enabled_v2_run(transaction, run_refs[0]).await
}

async fn lock_enabled_v2_run_for_pair_ref(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pair_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    let signals: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT first_signal_ref,second_signal_ref FROM linggan_comment_study_problem_pair \
         WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let (first_signal_ref, second_signal_ref) = signals.ok_or(ProblemStoreError::RunUnavailable)?;
    lock_enabled_v2_run_for_pair(transaction, first_signal_ref, second_signal_ref).await
}

async fn lock_enabled_v2_run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    sqlx::query_scalar(
        "SELECT run.run_ref FROM linggan_comment_study_run run \
         WHERE run.run_ref=$1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
           AND to_jsonb(run)->>'dispatch_state'='enabled' \
           AND to_jsonb(run)->>'dispatch_reason' IS NULL \
         FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::RunUnavailable)
}

struct PendingPair {
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    manifest: Value,
}

async fn lock_pending_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pair_ref: Uuid,
    expected_invocation: Option<Uuid>,
) -> Result<PendingPair, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT first_signal_ref,second_signal_ref,pair_manifest \
         FROM linggan_comment_study_problem_pair \
         WHERE pair_ref=$1 AND state='pending' \
           AND (($2::uuid IS NULL AND model_invocation_ref IS NULL) \
                OR model_invocation_ref=$2) FOR UPDATE",
    )
    .bind(pair_ref)
    .bind(expected_invocation)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::PairUnavailable)?;
    Ok(PendingPair {
        first_signal_ref: row.get("first_signal_ref"),
        second_signal_ref: row.get("second_signal_ref"),
        manifest: row.get("pair_manifest"),
    })
}

async fn lock_problem_stage_run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject: ProblemStageSubject,
    invocation_ref: Uuid,
) -> Result<Uuid, ProblemStoreError> {
    let run_ref: Option<Uuid> = match subject {
        ProblemStageSubject::Resolution(resolution_ref) => {
            sqlx::query_scalar(
                "SELECT run_ref FROM linggan_comment_study_model_request \
             WHERE invocation_ref=$1 AND stage='resolution' AND resolution_ref=$2",
            )
            .bind(invocation_ref)
            .bind(resolution_ref)
            .fetch_optional(&mut **transaction)
            .await?
        }
        ProblemStageSubject::Pair(pair_ref) => {
            sqlx::query_scalar(
                "SELECT request.run_ref FROM linggan_comment_study_model_request request \
             JOIN linggan_comment_study_problem_pair pair ON pair.pair_ref=request.pair_ref \
             JOIN linggan_comment_study_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
             JOIN linggan_comment_study_target first_target USING(target_ref) \
             JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
             JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
             WHERE request.invocation_ref=$1 AND request.stage='pair' AND request.pair_ref=$2 \
               AND first_target.run_ref=request.run_ref AND second_target.run_ref=request.run_ref",
            )
            .bind(invocation_ref)
            .bind(pair_ref)
            .fetch_optional(&mut **transaction)
            .await?
        }
    };
    let run_ref = run_ref.ok_or(ProblemStoreError::ModelRequestUnavailable)?;
    let locked_run: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run \
         WHERE run_ref=$1 AND selection_manifest->>'contract'='comment-study.run-selection.v2' \
         FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    locked_run.ok_or(ProblemStoreError::ModelRequestUnavailable)
}

async fn lock_active_problem_stage_request(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject: ProblemStageSubject,
    run_ref: Uuid,
    invocation_ref: Uuid,
) -> Result<(), ProblemStoreError> {
    let active: Option<Uuid> =
        match subject {
            ProblemStageSubject::Resolution(resolution_ref) => sqlx::query_scalar(
                "SELECT request.invocation_ref FROM linggan_comment_study_model_request request \
             JOIN linggan_model_invocation invocation USING(invocation_ref) \
             WHERE request.invocation_ref=$1 AND request.run_ref=$2 AND request.stage='resolution' \
               AND request.resolution_ref=$3 AND request.dispatch_started_at IS NOT NULL \
               AND request.deadline_at>scope_001_now() AND invocation.state='running' \
             FOR UPDATE OF request,invocation",
            )
            .bind(invocation_ref)
            .bind(run_ref)
            .bind(resolution_ref)
            .fetch_optional(&mut **transaction)
            .await?,
            ProblemStageSubject::Pair(pair_ref) => sqlx::query_scalar(
                "SELECT request.invocation_ref FROM linggan_comment_study_model_request request \
             JOIN linggan_model_invocation invocation USING(invocation_ref) \
             WHERE request.invocation_ref=$1 AND request.run_ref=$2 AND request.stage='pair' \
               AND request.pair_ref=$3 AND request.dispatch_started_at IS NOT NULL \
               AND request.deadline_at>scope_001_now() AND invocation.state='running' \
             FOR UPDATE OF request,invocation",
            )
            .bind(invocation_ref)
            .bind(run_ref)
            .bind(pair_ref)
            .fetch_optional(&mut **transaction)
            .await?,
        };
    active
        .map(|_| ())
        .ok_or(ProblemStoreError::ModelRequestUnavailable)
}

fn pair_manifest(manifest: &Value) -> Result<(Uuid, bool, bool), ProblemStoreError> {
    if manifest.get("contract").and_then(Value::as_str)
        != Some("comment-study.problem-pair-input.v1")
    {
        return Err(ProblemStoreError::StoredManifest);
    }
    let domain_ref = manifest
        .get("domainRef")
        .and_then(Value::as_str)
        .ok_or(ProblemStoreError::StoredManifest)
        .and_then(|value| Uuid::parse_str(value).map_err(|_| ProblemStoreError::StoredManifest))?;
    let independent_sources = manifest
        .get("independentSources")
        .and_then(Value::as_bool)
        .ok_or(ProblemStoreError::StoredManifest)?;
    let independent_authors = manifest
        .get("independentAuthors")
        .and_then(Value::as_bool)
        .ok_or(ProblemStoreError::StoredManifest)?;
    Ok((domain_ref, independent_sources, independent_authors))
}

/// Creates a Problem and its first immutable revision together, or returns the Problem that
/// already carries this definition.
///
/// The two rows are one fact: a Problem with no revision has no meaning, and a revision names the
/// core that recall will encode and that every later member is compared against. The definition
/// hash therefore lives on the revision — deduplicating on the Problem row would tie identity to a
/// definition the Problem is not allowed to keep once it is revised.
async fn insert_or_find_problem(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
    definition: &NewProblemDefinition,
    seed_signal_refs: &[Uuid],
) -> Result<(Uuid, Uuid), sqlx::Error> {
    let definition_value = json!({
        "title":definition.title,
        "definition":definition.definition,
        "stableIdentity":definition.stable_identity,
        "includeCriteria":definition.include_criteria,
        "excludeCriteria":definition.exclude_criteria,
    });
    let definition_hash = sha256_json(&definition_value);
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text,0))")
        .bind(domain_ref)
        .bind(&definition_hash)
        .execute(&mut **transaction)
        .await?;
    if let Some(existing) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT problem.problem_ref,revision.revision_ref FROM linggan_comment_study_current_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.domain_ref=$1 AND revision.definition_hash=$2",
    )
    .bind(domain_ref)
    .bind(&definition_hash)
    .fetch_optional(&mut **transaction)
    .await?
    {
        return Ok(existing);
    }
    let problem_ref = Uuid::new_v4();
    let revision_ref = Uuid::new_v4();
    // The same builder a Signal goes through, so the core and the Signals judged
    // against it land in one space rather than two that merely look alike.
    let canonical = canonical_text(&definition.definition, Some(&definition.stable_identity));
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem(problem_ref,domain_ref,state) \
         VALUES($1,$2,'active')",
    )
    .bind(problem_ref)
    .bind(domain_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_revision( \
           revision_ref,problem_ref,domain_ref,identity_version,title,definition,core_frame, \
           inclusions,exclusions,seed_signal_refs,canonical_text,canonical_hash,definition_hash,reason \
         ) VALUES($1,$2,$3,1,$4,$5,$6,$7,$8,$9,$10,$11,$12,'pair_creation')",
    )
    .bind(revision_ref)
    .bind(problem_ref)
    .bind(domain_ref)
    .bind(&definition.title)
    .bind(&definition.definition)
    .bind(&definition.stable_identity)
    .bind(json!(definition.include_criteria))
    .bind(json!(definition.exclude_criteria))
    .bind(seed_signal_refs)
    .bind(&canonical)
    .bind(canonical_hash(&canonical))
    .bind(&definition_hash)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_study_problem SET current_revision_ref=$2,updated_at=scope_001_now() \
         WHERE problem_ref=$1",
    )
    .bind(problem_ref)
    .bind(revision_ref)
    .execute(&mut **transaction)
    .await?;
    Ok((problem_ref, revision_ref))
}

async fn assign_novel_signal_from_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
    problem_ref: Uuid,
    problem_revision_ref: Uuid,
    pair_ref: Uuid,
) -> Result<(), sqlx::Error> {
    let revision_supported = membership_revision_supported(transaction).await?;
    let decision_manifest = if revision_supported {
        json!({
            "contract":PROBLEM_PAIR_CONTRACT,
            "pairRef":pair_ref,
            "problemRevisionRef":problem_revision_ref
        })
    } else {
        json!({"contract":PROBLEM_PAIR_CONTRACT,"pairRef":pair_ref})
    };
    let resolution_ref: Uuid = sqlx::query_scalar(
        "UPDATE linggan_comment_study_resolution \
         SET state='assigned',resolved_problem_ref=$2,decision_manifest=$3,resolved_at=scope_001_now() \
         WHERE signal_ref=$1 AND state='deferred_novel' \
         RETURNING resolution_ref",
    )
    .bind(signal_ref)
    .bind(problem_ref)
    .bind(decision_manifest)
    .fetch_one(&mut **transaction)
    .await?;
    if revision_supported {
        sqlx::query(
            "INSERT INTO linggan_comment_study_problem_membership( \
               membership_ref,signal_ref,problem_ref,resolution_ref,problem_revision_ref \
             ) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(Uuid::new_v4())
        .bind(signal_ref)
        .bind(problem_ref)
        .bind(resolution_ref)
        .bind(problem_revision_ref)
        .execute(&mut **transaction)
        .await?;
    } else {
        sqlx::query(
            "INSERT INTO linggan_comment_study_problem_membership( \
               membership_ref,signal_ref,problem_ref,resolution_ref \
             ) VALUES($1,$2,$3,$4)",
        )
        .bind(Uuid::new_v4())
        .bind(signal_ref)
        .bind(problem_ref)
        .bind(resolution_ref)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn finish_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pair_ref: Uuid,
    state: &str,
    problem_ref: Option<Uuid>,
    decision_reason: &str,
    proposed_problem: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair \
         SET state=$2,created_problem_ref=$3,proposed_problem=$4,resolved_at=scope_001_now(), \
             pair_manifest=jsonb_set(pair_manifest,'{decision}',jsonb_build_object('code',$5::text)) \
         WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(state)
    .bind(problem_ref)
    .bind(proposed_problem)
    .bind(decision_reason)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub fn pair_contract_failure_code(error: &ProblemResolutionContractError) -> &'static str {
    match error {
        ProblemResolutionContractError::JsonSchema => "contract_rejected_json_schema",
        ProblemResolutionContractError::Contract => "contract_rejected_contract",
        ProblemResolutionContractError::CandidateSetMismatch => {
            "contract_rejected_candidate_set_mismatch"
        }
        ProblemResolutionContractError::InvalidVerdict => "contract_rejected_invalid_verdict",
        ProblemResolutionContractError::InvalidProblemDefinition => {
            "contract_rejected_invalid_problem_definition"
        }
        ProblemResolutionContractError::PairSignalMismatch => {
            "contract_rejected_pair_signal_mismatch"
        }
    }
}

fn normalized_candidates(candidates: Vec<Uuid>) -> Result<Vec<Uuid>, ProblemStoreError> {
    if candidates.len() > MAX_SERVER_CANDIDATES
        || candidates.iter().copied().collect::<BTreeSet<_>>().len() != candidates.len()
    {
        return Err(ProblemStoreError::InvalidCandidateSet);
    }
    Ok(candidates)
}

fn ordered_pair(first: Uuid, second: Uuid) -> (Uuid, Uuid) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn sha256_json(value: &Value) -> String {
    Sha256::digest(
        serde_json::to_string(value)
            .expect("JSON values serialize")
            .as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}
