//! Transactional persistence for the clean comment-study Problem lifecycle.
//!
//! The caller supplies only a server-selected candidate set. This module freezes that set before
//! any model comparison, and it writes a membership only after the deterministic resolver admits
//! exactly one match. Pair creation is deliberately separate from no-match handling.

use crate::comment_study_problem_resolution::{
    ExistingResolutionDecision, NewProblemDefinition, PROBLEM_PAIR_CONTRACT, PairCreationDecision,
    ProblemResolutionContractError, decide_existing_resolution, decide_pair_creation,
};
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemPairReceipt {
    pub pair_ref: Uuid,
    pub state: String,
    pub problem_ref: Option<Uuid>,
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
    #[error(
        "a candidate Problem is absent, retired, from another domain, duplicated, or exceeds the maximum candidate count"
    )]
    InvalidCandidateSet,
    #[error("only eligible Problem or Need Signals may receive a candidate comparison")]
    SignalNotEligible,
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

/// Freezes a server-selected candidate set. A deferred-context or not-user-problem Signal is
/// terminally categorized without invoking a comparison model.
pub async fn prepare_problem_resolution(
    database: &Database,
    signal_ref: Uuid,
    server_candidate_refs: Vec<Uuid>,
) -> Result<PreparedProblemResolution, ProblemStoreError> {
    let candidates = normalized_candidates(server_candidate_refs)?;
    let mut transaction = database.pool().begin().await?;
    let signal = lock_signal_for_resolution(&mut transaction, signal_ref).await?;
    let (state, candidate_problem_refs) = match signal.eligibility_state.as_str() {
        "eligible" => {
            validate_candidates(&mut transaction, signal.domain_ref, &candidates).await?;
            ("pending", candidates)
        }
        "deferred_context" => ("deferred_context", Vec::new()),
        "not_user_problem" => ("not_user_problem", Vec::new()),
        _ => return Err(ProblemStoreError::SignalNotEligible),
    };
    let resolution_ref = Uuid::new_v4();
    let manifest = json!({
        "contract":"comment-study.problem-candidate-set.v1",
        "candidateProblemRefs":candidate_problem_refs,
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

/// Accepts a closed candidate comparison. A malformed model output is recorded as a protocol
/// rejection, never reinterpreted as “there was no matching Problem”.
pub async fn accept_problem_resolution(
    database: &Database,
    resolution_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemResolutionReceipt, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    let row = lock_pending_resolution(&mut transaction, resolution_ref).await?;
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
            finish_resolution(
                &mut transaction,
                resolution_ref,
                "assigned",
                Some(problem_ref),
                raw_output,
            )
            .await?;
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
) -> Result<PreparedProblemPair, ProblemStoreError> {
    if first_signal_ref == second_signal_ref {
        return Err(ProblemStoreError::PairNotIndependentOrNovel);
    }
    let (first_ref, second_ref) = ordered_pair(first_signal_ref, second_signal_ref);
    let mut transaction = database.pool().begin().await?;
    let first = lock_novel_signal(&mut transaction, first_ref).await?;
    let second = lock_novel_signal(&mut transaction, second_ref).await?;
    if first.domain_ref != second.domain_ref
        || first.source_ref == second.source_ref
        || first.author_external_id.is_none()
        || first.author_external_id == second.author_external_id
    {
        return Err(ProblemStoreError::PairNotIndependentOrNovel);
    }
    let pair_ref = Uuid::new_v4();
    let manifest = json!({
        "contract":"comment-study.problem-pair-input.v1",
        "domainRef":first.domain_ref,
        "first":{"signalRef":first.signal_ref,"sourceRef":first.source_ref},
        "second":{"signalRef":second.signal_ref,"sourceRef":second.source_ref},
        "independentSources":true,
        "independentAuthors":true,
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

/// Creates a durable Problem and assigns both Signals only when the pair contract and independent
/// source checks admit it. Any non-create decision leaves both original Signals visible but
/// unassigned.
pub async fn accept_problem_pair(
    database: &Database,
    pair_ref: Uuid,
    raw_output: Value,
) -> Result<ProblemPairReceipt, ProblemStoreError> {
    let mut transaction = database.pool().begin().await?;
    let pair = lock_pending_pair(&mut transaction, pair_ref).await?;
    let (domain_ref, independent_sources, independent_authors) = pair_manifest(&pair.manifest)?;
    let decision = match decide_pair_creation(
        raw_output.clone(),
        pair.first_signal_ref,
        pair.second_signal_ref,
        independent_sources,
        independent_authors,
    ) {
        Ok(decision) => decision,
        Err(error) => {
            finish_pair(&mut transaction, pair_ref, "rejected", None, raw_output).await?;
            transaction.commit().await?;
            return Err(ProblemStoreError::Contract(error));
        }
    };
    let receipt = match decision {
        PairCreationDecision::Create(definition) => {
            let problem_ref =
                insert_or_find_problem(&mut transaction, domain_ref, &definition).await?;
            assign_novel_signal_from_pair(
                &mut transaction,
                pair.first_signal_ref,
                problem_ref,
                pair_ref,
            )
            .await?;
            assign_novel_signal_from_pair(
                &mut transaction,
                pair.second_signal_ref,
                problem_ref,
                pair_ref,
            )
            .await?;
            finish_pair(
                &mut transaction,
                pair_ref,
                "approved",
                Some(problem_ref),
                raw_output,
            )
            .await?;
            ProblemPairReceipt {
                pair_ref,
                state: "approved".to_owned(),
                problem_ref: Some(problem_ref),
            }
        }
        PairCreationDecision::DeferInsufficientIndependentEvidence
        | PairCreationDecision::DeferAmbiguous
        | PairCreationDecision::DeferNotSameProblem => {
            finish_pair(&mut transaction, pair_ref, "rejected", None, raw_output).await?;
            ProblemPairReceipt {
                pair_ref,
                state: "rejected".to_owned(),
                problem_ref: None,
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
        "SELECT count(*) FROM linggan_comment_study_problem \
         WHERE domain_ref=$1 AND state='active' AND problem_ref=ANY($2)",
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

struct PendingResolution {
    signal_ref: Uuid,
    domain_ref: Uuid,
    candidate_manifest: Value,
}

async fn lock_pending_resolution(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    resolution_ref: Uuid,
) -> Result<PendingResolution, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT signal_ref,domain_ref,candidate_manifest \
         FROM linggan_comment_study_resolution \
         WHERE resolution_ref=$1 AND state='pending' FOR UPDATE",
    )
    .bind(resolution_ref)
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

async fn lock_novel_signal(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
) -> Result<NovelSignal, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT signal.signal_ref,policy.domain_ref,target.source_ref,source.author_external_id \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         JOIN linggan_comment_study_run run USING(run_ref) \
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

struct PendingPair {
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    manifest: Value,
}

async fn lock_pending_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pair_ref: Uuid,
) -> Result<PendingPair, ProblemStoreError> {
    let row = sqlx::query(
        "SELECT first_signal_ref,second_signal_ref,pair_manifest \
         FROM linggan_comment_study_problem_pair \
         WHERE pair_ref=$1 AND state='pending' FOR UPDATE",
    )
    .bind(pair_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ProblemStoreError::PairUnavailable)?;
    Ok(PendingPair {
        first_signal_ref: row.get("first_signal_ref"),
        second_signal_ref: row.get("second_signal_ref"),
        manifest: row.get("pair_manifest"),
    })
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

async fn insert_or_find_problem(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
    definition: &NewProblemDefinition,
) -> Result<Uuid, sqlx::Error> {
    let definition_value = json!({
        "definition":definition.definition,
        "stableIdentity":definition.stable_identity,
        "includeCriteria":definition.include_criteria,
        "excludeCriteria":definition.exclude_criteria,
    });
    let definition_hash = sha256_json(&definition_value);
    let problem_ref = Uuid::new_v4();
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO linggan_comment_study_problem( \
           problem_ref,domain_ref,definition,stable_identity,include_criteria,exclude_criteria,definition_hash,state \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,'active') \
         ON CONFLICT(domain_ref,definition_hash) DO NOTHING RETURNING problem_ref",
    )
    .bind(problem_ref)
    .bind(domain_ref)
    .bind(&definition.definition)
    .bind(&definition.stable_identity)
    .bind(json!(definition.include_criteria))
    .bind(json!(definition.exclude_criteria))
    .bind(&definition_hash)
    .fetch_optional(&mut **transaction)
    .await?;
    match inserted {
        Some(problem_ref) => Ok(problem_ref),
        None => {
            sqlx::query_scalar(
                "SELECT problem_ref FROM linggan_comment_study_problem \
             WHERE domain_ref=$1 AND definition_hash=$2 AND state='active'",
            )
            .bind(domain_ref)
            .bind(definition_hash)
            .fetch_one(&mut **transaction)
            .await
        }
    }
}

async fn assign_novel_signal_from_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    signal_ref: Uuid,
    problem_ref: Uuid,
    pair_ref: Uuid,
) -> Result<(), sqlx::Error> {
    let resolution_ref: Uuid = sqlx::query_scalar(
        "UPDATE linggan_comment_study_resolution \
         SET state='assigned',resolved_problem_ref=$2, \
             decision_manifest=jsonb_build_object('contract',$3,'pairRef',$4),resolved_at=scope_001_now() \
         WHERE signal_ref=$1 AND state='deferred_novel' \
         RETURNING resolution_ref",
    )
    .bind(signal_ref)
    .bind(problem_ref)
    .bind(PROBLEM_PAIR_CONTRACT)
    .bind(pair_ref)
    .fetch_one(&mut **transaction)
    .await?;
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
    Ok(())
}

async fn finish_pair(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pair_ref: Uuid,
    state: &str,
    problem_ref: Option<Uuid>,
    proposed_problem: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE linggan_comment_study_problem_pair \
         SET state=$2,created_problem_ref=$3,proposed_problem=$4,resolved_at=scope_001_now() \
         WHERE pair_ref=$1",
    )
    .bind(pair_ref)
    .bind(state)
    .bind(problem_ref)
    .bind(proposed_problem)
    .execute(&mut **transaction)
    .await?;
    Ok(())
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
