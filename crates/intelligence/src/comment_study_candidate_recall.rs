//! Server-side candidate recall for clean StudySignals.
//!
//! Recall is deliberately not a decision.  Its job is to provide a bounded same-domain candidate
//! set to the resolution contract, with transparent structured lexical evidence and a recency
//! fallback so a zero lexical overlap never becomes a hidden “no match” verdict.

use crate::comment_study_embedding::active_profile;
use crate::comment_study_problem_store::{
    PreparedProblemResolution, ProblemStoreError, accept_problem_resolution, prepare_problem_pair,
    prepare_problem_resolution, resolve_retrieval_incomplete,
    resume_retrieval_incomplete_resolution,
};
use crate::comment_study_recall::{RecallCompleteness, recall_candidates};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

const MAX_CANDIDATES: i64 = 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecalledProblemCandidate {
    pub problem_ref: Uuid,
    pub definition: String,
    pub stable_identity: Value,
    pub include_criteria: Value,
    pub exclude_criteria: Value,
    pub lexical_overlap: bool,
}

/// Finds one legal pair of independently authored deferred-novel Signals, choosing the partner by
/// vector proximity rather than by arrival order. Pairing the two Signals that merely happened to
/// arrive first spends a model call on two texts nothing ever suggested were about the same thing.
///
/// The pair is only a frozen comparison task; it does not itself create a Problem.
pub async fn advance_next_problem_pair(
    database: &Database,
) -> Result<bool, ProblemCandidateRecallError> {
    // Path C is a vector search. With no qualified profile there is no pool to search at all, and
    // falling back to arrival order would record a pairing as if proximity had been considered.
    let Some(profile_ref) = active_profile(database).await? else {
        return Ok(false);
    };
    for seeker in novel_signals_awaiting_pairing(database).await? {
        let recalled = recall_candidates(database, profile_ref, seeker).await?;
        // A Signal is only `deferred_novel` relative to the catalogue as it stood when it was
        // resolved. If the catalogue cannot be fully searched now, the Problem this pair would
        // create may already exist unseen — which is the duplicate this module exists to prevent.
        if recalled.completeness != RecallCompleteness::Complete {
            continue;
        }
        if let Some(partner) =
            nearest_admissible_partner(database, seeker, &recalled.pool_signal_refs).await?
        {
            prepare_problem_pair(database, seeker, partner).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// How many Signals one tick will look for a partner for before giving up. A Signal whose whole
/// pool has already been compared with it must not block every Signal behind it, so the scan moves
/// on rather than returning "nothing to pair" at the first exhausted one.
const MAX_PAIR_SEEKERS_PER_TICK: i64 = 8;

/// Signals still waiting to be paired, earliest first.
///
/// The only thing that retires a Signal from pairing is being assigned to a Problem. Having taken
/// part in a comparison that came apart is not a reason: it establishes that those *two* are not
/// the same Problem, and nothing about either one's relation to anything else.
async fn novel_signals_awaiting_pairing(
    database: &Database,
) -> Result<Vec<Uuid>, ProblemCandidateRecallError> {
    Ok(sqlx::query_scalar(
        "SELECT resolution.signal_ref FROM linggan_comment_study_resolution resolution \
         WHERE resolution.state='deferred_novel' \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_problem_membership membership \
                 WHERE membership.signal_ref=resolution.signal_ref) \
         ORDER BY resolution.created_at,resolution.signal_ref LIMIT $1",
    )
    .bind(MAX_PAIR_SEEKERS_PER_TICK)
    .fetch_all(database.pool())
    .await?)
}

/// The nearest pool candidate that may actually be admitted, keeping the recall's distance order.
///
/// The pool deliberately recalls same-account Signals too — they are evidence that the pool is not
/// empty — but a second reading from the same account is not independent support, so it can never
/// become a pair. Filtering here rather than letting `prepare_problem_pair` refuse means the
/// *nearest admissible* candidate is found instead of stopping at the nearest one overall.
///
/// Only the exact combination already compared is excluded, never every Signal that has ever been
/// compared with anything: the table's `UNIQUE(first,second)` is what stops one pair being bought
/// twice, and a wider exclusion would retire both sides of every inconclusive comparison.
async fn nearest_admissible_partner(
    database: &Database,
    seeker: Uuid,
    pool: &[Uuid],
) -> Result<Option<Uuid>, ProblemCandidateRecallError> {
    if pool.is_empty() {
        return Ok(None);
    }
    Ok(sqlx::query_scalar(
        "WITH seeker AS ( \
           SELECT target.source_ref,source.author_external_id \
           FROM linggan_comment_study_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
           WHERE signal.signal_ref=$1), \
         ranked AS (SELECT signal_ref,ordinality FROM unnest($2::uuid[]) \
                    WITH ORDINALITY AS entry(signal_ref,ordinality)) \
         SELECT ranked.signal_ref FROM ranked \
         JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         JOIN linggan_comment_study_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
         JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
         CROSS JOIN seeker \
         WHERE resolution.state='deferred_novel' \
           AND target.source_ref<>seeker.source_ref \
           AND source.author_external_id IS NOT NULL \
           AND seeker.author_external_id IS NOT NULL \
           AND source.author_external_id<>seeker.author_external_id \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_problem_membership membership \
                 WHERE membership.signal_ref=ranked.signal_ref) \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_problem_pair pair \
                 WHERE (pair.first_signal_ref=ranked.signal_ref AND pair.second_signal_ref=$1) \
                    OR (pair.first_signal_ref=$1 AND pair.second_signal_ref=ranked.signal_ref)) \
         ORDER BY ranked.ordinality LIMIT 1",
    )
    .bind(seeker)
    .bind(pool)
    .fetch_optional(database.pool())
    .await?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemCandidateRecall {
    pub signal_ref: Uuid,
    pub domain_ref: Uuid,
    pub recall_contract: &'static str,
    pub query_terms: Vec<String>,
    pub candidates: Vec<RecalledProblemCandidate>,
}

#[derive(Debug, Error)]
pub enum ProblemCandidateRecallError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Store(#[from] ProblemStoreError),
    #[error("the Signal is absent, outside Problem Resolution, or already resolved")]
    SignalUnavailable,
}

/// Retrieves at most twenty active same-domain Problems, ranking only explicit normalized field
/// overlap. The rank never assigns a Problem; if it is weak, the recency fallback still leaves a
/// closed candidate comparison to the model and deterministic resolver.
pub async fn recall_problem_candidates(
    database: &Database,
    signal_ref: Uuid,
) -> Result<ProblemCandidateRecall, ProblemCandidateRecallError> {
    let signal = sqlx::query(
        "SELECT signal.target_ref,target.run_ref,policy.domain_ref,signal.proposition,signal.problem_frame \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE signal.signal_ref=$1 AND signal.kind IN ('problem','need') \
           AND signal.eligibility_state='eligible' \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_resolution resolution WHERE resolution.signal_ref=signal.signal_ref)",
    )
    .bind(signal_ref)
    .fetch_optional(database.pool())
    .await?
    .ok_or(ProblemCandidateRecallError::SignalUnavailable)?;
    let domain_ref: Uuid = signal.get("domain_ref");
    let query_terms = recall_terms(signal.get("proposition"), signal.get("problem_frame"));
    let rows = sqlx::query(
        "WITH terms AS (SELECT unnest($2::text[]) AS term) \
         SELECT problem.problem_ref,revision.definition,revision.core_frame AS stable_identity, \
                revision.inclusions AS include_criteria,revision.exclusions AS exclude_criteria, \
                EXISTS(SELECT 1 FROM terms WHERE lower(revision.definition) LIKE '%' || lower(term) || '%' \
                       OR lower(revision.core_frame::text) LIKE '%' || lower(term) || '%') AS lexical_overlap, \
                COALESCE((SELECT count(*) FROM terms WHERE lower(revision.definition) LIKE '%' || lower(term) || '%' \
                       OR lower(revision.core_frame::text) LIKE '%' || lower(term) || '%'),0) AS overlap_count \
         FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.domain_ref=$1 AND problem.state='active' \
         ORDER BY overlap_count DESC,problem.created_at DESC,problem.problem_ref DESC LIMIT $3",
    )
    .bind(domain_ref)
    .bind(&query_terms)
    .bind(MAX_CANDIDATES)
    .fetch_all(database.pool())
    .await?;
    Ok(ProblemCandidateRecall {
        signal_ref,
        domain_ref,
        recall_contract: "comment-study.problem-candidate-recall.v1",
        query_terms,
        candidates: rows
            .into_iter()
            .map(|row| RecalledProblemCandidate {
                problem_ref: row.get("problem_ref"),
                definition: row.get("definition"),
                stable_identity: row.get("stable_identity"),
                include_criteria: row.get("include_criteria"),
                exclude_criteria: row.get("exclude_criteria"),
                lexical_overlap: row.get("lexical_overlap"),
            })
            .collect(),
    })
}

/// The only normal entry point for a candidate comparison.  It freezes exactly the server recall
/// returned above; callers cannot supply arbitrary candidate IDs from a browser or model output.
pub async fn prepare_recalled_problem_resolution(
    database: &Database,
    signal_ref: Uuid,
) -> Result<(ProblemCandidateRecall, PreparedProblemResolution), ProblemCandidateRecallError> {
    let recall = recall_problem_candidates(database, signal_ref).await?;
    let prepared = prepare_problem_resolution(
        database,
        signal_ref,
        recall
            .candidates
            .iter()
            .map(|candidate| candidate.problem_ref)
            .collect(),
    )
    .await?;
    Ok((recall, prepared))
}

/// Advances one eligible Signal into the closed candidate-comparison lifecycle. An empty
/// same-domain candidate set is deterministically `deferred_novel`; it never spends a model call
/// merely to learn that there was nothing to compare. Non-empty sets remain `pending` for the
/// resolution-model worker.
pub async fn advance_next_problem_resolution(
    database: &Database,
) -> Result<bool, ProblemCandidateRecallError> {
    // Without a qualified encoding profile there is no catalogue to search at all. Falling back to
    // the lexical path here would be worse than doing nothing: it would answer "no candidates"
    // with a method that cannot see semantic matches, and that answer creates duplicates.
    let profile_ref = active_profile(database).await?;
    let candidate: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT signal.signal_ref,resolution.resolution_ref \
         FROM linggan_comment_study_signal signal \
         LEFT JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         WHERE signal.kind IN ('problem','need') AND signal.eligibility_state='eligible' \
           AND (resolution.signal_ref IS NULL OR ($1 AND resolution.state='retrieval_incomplete')) \
         ORDER BY signal.created_at,signal.signal_ref LIMIT 1",
    )
    .bind(profile_ref.is_some())
    .fetch_optional(database.pool())
    .await?;
    let Some((signal_ref, existing_resolution_ref)) = candidate else {
        return Ok(false);
    };
    let Some(profile_ref) = profile_ref else {
        let prepared = prepare_problem_resolution(database, signal_ref, Vec::new()).await?;
        if prepared.state == "pending" {
            resolve_retrieval_incomplete(database, prepared.resolution_ref, "no_qualified_profile")
                .await?;
        }
        return Ok(true);
    };
    let recalled = recall_candidates(database, profile_ref, signal_ref).await?;
    // An identity match is a shortcut *into* the comparison queue, never past it, so it joins the
    // candidate set rather than resolving anything on its own.
    let mut candidate_problem_refs = recalled.identity_problem_refs.clone();
    for problem_ref in recalled.problem_refs {
        if !candidate_problem_refs.contains(&problem_ref) {
            candidate_problem_refs.push(problem_ref);
        }
    }
    let prepared = match existing_resolution_ref {
        Some(resolution_ref) => {
            resume_retrieval_incomplete_resolution(
                database,
                resolution_ref,
                candidate_problem_refs.clone(),
            )
            .await?
        }
        None => {
            prepare_problem_resolution(database, signal_ref, candidate_problem_refs.clone()).await?
        }
    };
    if prepared.state != "pending" {
        return Ok(true);
    }
    match recalled.completeness {
        // "Could not search the catalogue" and "searched it and found nothing" both arrive as an
        // empty list. Only the second is evidence that this Signal is novel.
        RecallCompleteness::Incomplete { reason } => {
            resolve_retrieval_incomplete(database, prepared.resolution_ref, reason).await?;
        }
        RecallCompleteness::Complete if candidate_problem_refs.is_empty() => {
            accept_problem_resolution(
                database,
                prepared.resolution_ref,
                serde_json::json!({"contract":"comment-study.problem-resolution.v1","candidates":[]}),
            )
            .await?;
        }
        RecallCompleteness::Complete => {}
    }
    Ok(true)
}

fn recall_terms(proposition: String, problem_frame: Value) -> Vec<String> {
    let mut values = vec![proposition];
    for field in [
        "actor",
        "goalOrExpectedState",
        "barrierOrUnmetNeed",
        "context",
    ] {
        if let Some(value) = problem_frame
            .get(field)
            .and_then(|entry| entry.get("value"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            values.push(value.to_owned());
        }
    }
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::recall_terms;
    use serde_json::json;

    #[test]
    fn structured_frame_terms_are_explicit_and_deduplicated() {
        assert_eq!(
            recall_terms(
                "作业启动困难".into(),
                json!({"actor":{"value":"孩子"},"barrierOrUnmetNeed":{"value":"需要催促"}})
            ),
            vec!["作业启动困难", "孩子", "需要催促"]
        );
    }
}
