//! Server-side candidate recall for clean StudySignals.
//!
//! Recall is deliberately not a decision.  Its job is to provide a bounded same-domain candidate
//! set to the resolution contract, with transparent structured lexical evidence and a recency
//! fallback so a zero lexical overlap never becomes a hidden “no match” verdict.

use crate::comment_study_problem_store::{
    PreparedProblemResolution, ProblemStoreError, accept_problem_resolution, prepare_problem_pair,
    prepare_problem_resolution,
};
use crate::comment_study_embedding::active_profile;
use crate::comment_study_problem_store::resolve_retrieval_incomplete;
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

/// Finds one legal pair of independently authored deferred-novel Signals. The pair is only a
/// frozen comparison task; it does not itself create a Problem.
pub async fn advance_next_problem_pair(
    database: &Database,
) -> Result<bool, ProblemCandidateRecallError> {
    let pair: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT first_signal.signal_ref,second_signal.signal_ref \
         FROM linggan_comment_study_resolution first_resolution \
         JOIN linggan_comment_study_signal first_signal ON first_signal.signal_ref=first_resolution.signal_ref \
         JOIN linggan_comment_study_target first_target ON first_target.target_ref=first_signal.target_ref \
         JOIN linggan_material_comment first_source ON first_source.material_ref=first_target.source_ref \
         JOIN linggan_comment_study_resolution second_resolution ON second_resolution.domain_ref=first_resolution.domain_ref \
              AND second_resolution.state='deferred_novel' AND second_resolution.signal_ref>first_resolution.signal_ref \
         JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=second_resolution.signal_ref \
         JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
         JOIN linggan_material_comment second_source ON second_source.material_ref=second_target.source_ref \
         WHERE first_resolution.state='deferred_novel' AND first_source.material_ref<>second_source.material_ref \
           AND first_source.author_external_id IS NOT NULL AND second_source.author_external_id IS NOT NULL \
           AND first_source.author_external_id<>second_source.author_external_id \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_problem_pair pair \
                 WHERE pair.first_signal_ref=first_signal.signal_ref OR pair.second_signal_ref=first_signal.signal_ref \
                    OR pair.first_signal_ref=second_signal.signal_ref OR pair.second_signal_ref=second_signal.signal_ref) \
         ORDER BY first_resolution.created_at,second_resolution.created_at LIMIT 1",
    ).fetch_optional(database.pool()).await?;
    let Some((first, second)) = pair else {
        return Ok(false);
    };
    prepare_problem_pair(database, first, second).await?;
    Ok(true)
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
         SELECT problem.problem_ref,problem.definition,problem.stable_identity,problem.include_criteria,problem.exclude_criteria, \
                EXISTS(SELECT 1 FROM terms WHERE lower(problem.definition) LIKE '%' || lower(term) || '%' \
                       OR lower(problem.stable_identity::text) LIKE '%' || lower(term) || '%') AS lexical_overlap, \
                COALESCE((SELECT count(*) FROM terms WHERE lower(problem.definition) LIKE '%' || lower(term) || '%' \
                       OR lower(problem.stable_identity::text) LIKE '%' || lower(term) || '%'),0) AS overlap_count \
         FROM linggan_comment_study_problem problem \
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
    let signal_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT signal.signal_ref FROM linggan_comment_study_signal signal \
         WHERE signal.kind IN ('problem','need') AND signal.eligibility_state='eligible' \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_resolution resolution WHERE resolution.signal_ref=signal.signal_ref) \
         ORDER BY signal.created_at,signal.signal_ref LIMIT 1",
    )
    .fetch_optional(database.pool())
    .await?;
    let Some(signal_ref) = signal_ref else {
        return Ok(false);
    };
    // Without a qualified encoding profile there is no catalogue to search at all. Falling back to
    // the lexical path here would be worse than doing nothing: it would answer "no candidates"
    // with a method that cannot see semantic matches, and that answer creates duplicates.
    let Some(profile_ref) = active_profile(database).await? else {
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
    let prepared =
        prepare_problem_resolution(database, signal_ref, candidate_problem_refs.clone()).await?;
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
