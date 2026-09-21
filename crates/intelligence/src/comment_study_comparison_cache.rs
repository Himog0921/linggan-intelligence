//! Reusing a comparison that has already been paid for.
//!
//! The cache is keyed by what was compared — the two canonical texts and the policy that judged
//! them — never by when. Re-running a Run, re-reading the same pool, or advancing the same Signal
//! twice therefore land on the existing verdict instead of buying it again.
//!
//! It caches a judgement; it never stands in for one. A changed canonical text or a changed policy
//! is a different key, not a stale hit, because a cache that answered for inputs it never saw
//! would be indistinguishable from a model that had actually looked.

use crate::comment_study_problem_resolution::PROBLEM_RESOLUTION_CONTRACT;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;


/// Resolves whatever the cache can answer, before any invocation is reserved.
///
/// Placed ahead of the claim on purpose: serving a cached verdict through the normal worker would
/// leave a reserved-but-unused model invocation on the ledger, which is neither a success nor a
/// failure and would quietly corrupt what the ledger is for. A comparison the cache can answer
/// simply never becomes a call.
pub async fn serve_pending_resolutions_from_cache(
    database: &Database,
) -> Result<usize, ComparisonCacheError> {
    let pending: Vec<(Uuid, Value)> = sqlx::query_as(
        "SELECT resolution_ref,candidate_manifest FROM linggan_comment_study_resolution \
         WHERE state='pending' AND model_invocation_ref IS NULL \
         ORDER BY created_at,resolution_ref LIMIT 32",
    )
    .fetch_all(database.pool())
    .await?;
    let mut served = 0;
    for (resolution_ref, manifest) in pending {
        let signal_ref: Uuid = sqlx::query_scalar(
            "SELECT signal_ref FROM linggan_comment_study_resolution WHERE resolution_ref=$1",
        )
        .bind(resolution_ref)
        .fetch_one(database.pool())
        .await?;
        let candidates: Vec<Uuid> = manifest["candidateProblemRefs"]
            .as_array()
            .map(|refs| {
                refs.iter()
                    .filter_map(|value| value.as_str())
                    .filter_map(|value| Uuid::parse_str(value).ok())
                    .collect()
            })
            .unwrap_or_default();
        let Some(output) = cached_resolution_output(database, signal_ref, &candidates).await? else {
            continue;
        };
        crate::comment_study_problem_store::accept_problem_resolution(
            database,
            resolution_ref,
            output,
        )
        .await
        .map_err(|_| ComparisonCacheError::Acceptance)?;
        served += 1;
    }
    Ok(served)
}

#[derive(Debug, thiserror::Error)]
pub enum ComparisonCacheError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("a cached comparison could not be admitted")]
    Acceptance,
}

/// Rebuilds a complete resolution output from cache, or returns `None`.
///
/// Deliberately all-or-nothing. A partial hit would have to be completed by a model call anyway,
/// and answering with only the cached subset would be the same error this module's callers guard
/// against elsewhere: presenting "some of the candidates were compared" as "the candidates were
/// compared".
pub async fn cached_resolution_output(
    database: &Database,
    signal_ref: Uuid,
    candidate_problem_refs: &[Uuid],
) -> Result<Option<Value>, sqlx::Error> {
    if candidate_problem_refs.is_empty() {
        return Ok(None);
    }
    let Some(context) = comparison_context(database, signal_ref).await? else {
        return Ok(None);
    };
    let mut candidates = Vec::with_capacity(candidate_problem_refs.len());
    for problem_ref in candidate_problem_refs {
        let Some(right_hash) = problem_core_hash(database, *problem_ref).await? else {
            return Ok(None);
        };
        let key = cache_key(&context.signal_hash, &right_hash, context.policy_ref);
        let Some(dimensions) = sqlx::query_scalar::<_, Value>(
            "SELECT dimensions FROM linggan_comment_study_comparison \
             WHERE domain_ref=$1 AND cache_key=$2",
        )
        .bind(context.domain_ref)
        .bind(&key)
        .fetch_optional(database.pool())
        .await?
        else {
            return Ok(None);
        };
        candidates.push(json!({"problemRef":problem_ref,"dimensions":dimensions}));
    }
    Ok(Some(json!({
        "contract": PROBLEM_RESOLUTION_CONTRACT,
        "candidates": candidates,
    })))
}

/// Stores the per-candidate verdicts a model just produced.
///
/// Failing to cache is not failing to resolve: the caller has already recorded the real outcome,
/// so a write conflict here only costs a future repeat comparison.
pub async fn record_resolution_comparisons(
    database: &Database,
    signal_ref: Uuid,
    raw_output: &Value,
    model_invocation_ref: Option<Uuid>,
) -> Result<usize, sqlx::Error> {
    let Some(context) = comparison_context(database, signal_ref).await? else {
        return Ok(0);
    };
    let Some(candidates) = raw_output.get("candidates").and_then(Value::as_array) else {
        return Ok(0);
    };
    let mut stored = 0;
    for candidate in candidates {
        let Some(problem_ref) = candidate
            .get("problemRef")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        let Some(dimensions) = candidate.get("dimensions").filter(|value| value.is_object()) else {
            continue;
        };
        let Some(right_hash) = problem_core_hash(database, problem_ref).await? else {
            continue;
        };
        let key = cache_key(&context.signal_hash, &right_hash, context.policy_ref);
        let inserted = sqlx::query(
            "INSERT INTO linggan_comment_study_comparison( \
               comparison_ref,domain_ref,cache_key,kind,left_canonical_hash,right_canonical_hash, \
               verdict,dimensions,model_invocation_ref \
             ) VALUES($1,$2,$3,'signal_problem',$4,$5,$6,$7,$8) \
             ON CONFLICT(domain_ref,cache_key) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(context.domain_ref)
        .bind(&key)
        .bind(&context.signal_hash)
        .bind(&right_hash)
        .bind(verdict_of(dimensions))
        .bind(dimensions)
        .bind(model_invocation_ref)
        .execute(database.pool())
        .await?;
        stored += usize::try_from(inserted.rows_affected()).unwrap_or(0);
    }
    Ok(stored)
}

struct ComparisonContext {
    domain_ref: Uuid,
    policy_ref: Uuid,
    signal_hash: String,
}

/// The policy belongs in the key because it is what judged the two texts. Two identical sentences
/// under different admission rules are not the same question already answered.
async fn comparison_context(
    database: &Database,
    signal_ref: Uuid,
) -> Result<Option<ComparisonContext>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT work.domain_ref,run.policy_ref,signal.canonical_hash \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
         JOIN linggan_comment_study_work work \
           ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         WHERE signal.signal_ref=$1 AND signal.canonical_hash IS NOT NULL",
    )
    .bind(signal_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| ComparisonContext {
        domain_ref: row.get("domain_ref"),
        policy_ref: row.get("policy_ref"),
        signal_hash: row.get("canonical_hash"),
    }))
}

/// The *current* revision's core, so a Problem that has been revised is a different comparison
/// rather than one already answered under its previous meaning.
async fn problem_core_hash(
    database: &Database,
    problem_ref: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT revision.canonical_hash FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.problem_ref=$1",
    )
    .bind(problem_ref)
    .fetch_optional(database.pool())
    .await
}

fn cache_key(left_hash: &str, right_hash: &str, policy_ref: Uuid) -> String {
    Sha256::digest(format!("{left_hash}\n{right_hash}\n{policy_ref}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Mirrors the resolver's own rule rather than trusting a separate `overall` field: goal and
/// barrier must be equivalent, subject at least compatible, and no identity-bearing context may be
/// different or unknown. A stored verdict that disagreed with its own dimensions would be a second
/// opinion nobody asked for.
fn verdict_of(dimensions: &Value) -> &'static str {
    let read = |key: &str| dimensions.get(key).and_then(Value::as_str).unwrap_or("unknown");
    let goal = read("goalOrExpectedState");
    let barrier = read("barrierOrUnmetNeed");
    let actor = read("actor");
    let context = read("context");
    if [goal, barrier, actor, context].contains(&"different") {
        return "different";
    }
    if goal == "equivalent"
        && barrier == "equivalent"
        && matches!(actor, "equivalent" | "compatible")
        && context != "unknown"
    {
        return "same";
    }
    "uncertain"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dimensions(actor: &str, goal: &str, barrier: &str, context: &str) -> Value {
        json!({
            "actor":actor,"goalOrExpectedState":goal,
            "barrierOrUnmetNeed":barrier,"context":context
        })
    }

    #[test]
    fn every_core_dimension_must_line_up_before_a_verdict_is_same() {
        assert_eq!(
            verdict_of(&dimensions("equivalent", "equivalent", "equivalent", "equivalent")),
            "same"
        );
        assert_eq!(
            verdict_of(&dimensions("compatible", "equivalent", "equivalent", "compatible")),
            "same",
            "subject may be merely compatible; goal and barrier may not"
        );
    }

    #[test]
    fn a_shared_goal_with_a_different_barrier_is_not_the_same_problem() {
        // The manual's own counter-example: wanting to finish homework on time is one goal, but
        // "cannot read the question" and "cannot get started" are different problems.
        assert_eq!(
            verdict_of(&dimensions("equivalent", "equivalent", "different", "equivalent")),
            "different"
        );
    }

    #[test]
    fn an_unproven_dimension_yields_uncertain_rather_than_a_guess() {
        assert_eq!(
            verdict_of(&dimensions("equivalent", "unknown", "equivalent", "equivalent")),
            "uncertain"
        );
        assert_eq!(
            verdict_of(&dimensions("equivalent", "equivalent", "equivalent", "unknown")),
            "uncertain",
            "an identity-bearing context that nobody established cannot support sameness"
        );
    }

    #[test]
    fn the_key_separates_texts_and_policies_that_were_never_compared_together() {
        let policy = Uuid::new_v4();
        let other_policy = Uuid::new_v4();
        assert_ne!(cache_key("a", "b", policy), cache_key("b", "a", policy));
        assert_ne!(cache_key("a", "b", policy), cache_key("a", "b", other_policy));
        assert_eq!(cache_key("a", "b", policy), cache_key("a", "b", policy));
    }
}
