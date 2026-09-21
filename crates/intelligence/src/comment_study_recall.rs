//! Choosing what a Signal gets compared against.
//!
//! Recall proposes; it never decides. Distance orders the queue of things worth asking a model
//! about and carries no verdict of its own — there is deliberately no cosine threshold here,
//! because "close" is not "the same problem" and "far" is not "a new one".
//!
//! Three paths, each bounded, each able to say it came up short:
//!   A. the identical canonical sentence, as a shortcut into an existing Problem;
//!   B. Problem cores — and, as supplementary reach, a few member Signals of each;
//!   C. the unmerged pool, so the very first eligible Signal has something to be compared with.
//!
//! The failure this module exists to prevent is the quiet one: reporting "no candidates" when the
//! catalogue was simply not searchable. That reads exactly like "this is a new problem" and
//! creates a duplicate.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

/// Budget parameters, not semantic truths. Raising them widens what a model is asked about; it
/// never changes what counts as the same problem.
const MAX_PROBLEM_CANDIDATES: i64 = 8;
const MAX_POOL_CANDIDATES: i64 = 16;
const MAX_REPRESENTATIVES_PER_PROBLEM: i64 = 3;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum RecallCompleteness {
    /// Every active Problem in the domain was searchable and the pool query ran.
    Complete,
    /// Recall could not cover the catalogue. The caller must resolve as `retrieval_incomplete`
    /// rather than treating an empty candidate list as evidence of novelty.
    Incomplete { reason: &'static str },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallCandidates {
    /// Problems whose current core is the identical canonical sentence. A shortcut into the
    /// comparison queue, never an admission: source and independence are still checked.
    pub identity_problem_refs: Vec<Uuid>,
    /// Ordered by ascending distance, already folded to one entry per Problem.
    pub problem_refs: Vec<Uuid>,
    /// Eligible Signals that no Problem supports yet.
    pub pool_signal_refs: Vec<Uuid>,
    pub completeness: RecallCompleteness,
}

/// Collects candidates for one eligible Signal under the active embedding profile.
///
/// `profile_ref` is passed in rather than looked up here so that a caller can never mix distances
/// from two profiles by accident: one call, one space.
pub async fn recall_candidates(
    database: &Database,
    profile_ref: Uuid,
    signal_ref: Uuid,
) -> Result<RecallCandidates, sqlx::Error> {
    let Some(target) = target_signal(database, signal_ref).await? else {
        return Ok(empty("signal_not_eligible_or_unvectorised"));
    };
    // An active Problem whose core was never encoded is invisible to path B. Continuing would
    // report a smaller catalogue than exists, so the whole recall is marked incomplete instead.
    let uncovered_cores = uncovered_problem_cores(database, profile_ref, target.domain_ref).await?;
    let identity_problem_refs =
        identity_matches(database, target.domain_ref, &target.canonical_hash).await?;
    let problem_refs = nearest_problems(
        database,
        profile_ref,
        target.domain_ref,
        &target.canonical_hash,
    )
    .await?;
    let pool_signal_refs = unmerged_pool(
        database,
        profile_ref,
        target.domain_ref,
        signal_ref,
        &target.canonical_hash,
    )
    .await?;
    Ok(RecallCandidates {
        identity_problem_refs,
        problem_refs,
        pool_signal_refs,
        completeness: if uncovered_cores > 0 {
            RecallCompleteness::Incomplete {
                reason: "problem_core_vectors_incomplete",
            }
        } else {
            RecallCompleteness::Complete
        },
    })
}

struct TargetSignal {
    domain_ref: Uuid,
    canonical_hash: String,
}

fn empty(reason: &'static str) -> RecallCandidates {
    RecallCandidates {
        identity_problem_refs: Vec::new(),
        problem_refs: Vec::new(),
        pool_signal_refs: Vec::new(),
        completeness: RecallCompleteness::Incomplete { reason },
    }
}

/// The Signal must be eligible, must carry canonical text, and that text must already be encoded.
/// A Signal without a vector is not a Signal without matches.
async fn target_signal(
    database: &Database,
    signal_ref: Uuid,
) -> Result<Option<TargetSignal>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT work.domain_ref,signal.canonical_hash \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_work work \
           ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         WHERE signal.signal_ref=$1 AND signal.eligibility_state='eligible' \
           AND signal.canonical_hash IS NOT NULL",
    )
    .bind(signal_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| TargetSignal {
        domain_ref: row.get("domain_ref"),
        canonical_hash: row.get("canonical_hash"),
    }))
}

async fn uncovered_problem_cores(
    database: &Database,
    profile_ref: Uuid,
    domain_ref: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.domain_ref=$1 AND problem.state='active' \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_study_embedding_cache cache \
             WHERE cache.profile_ref=$2 AND cache.canonical_hash=revision.canonical_hash)",
    )
    .bind(domain_ref)
    .bind(profile_ref)
    .fetch_one(database.pool())
    .await
}

async fn identity_matches(
    database: &Database,
    domain_ref: Uuid,
    canonical_hash: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT problem.problem_ref FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.domain_ref=$1 AND problem.state='active' \
           AND revision.canonical_hash=$2 \
         ORDER BY problem.created_at,problem.problem_ref",
    )
    .bind(domain_ref)
    .bind(canonical_hash)
    .fetch_all(database.pool())
    .await
}

/// The representative ranking, defined once.
///
/// Both the recall query and the accessor tests observe it through read the same text: a second
/// copy would let the tested ordering and the shipped ordering drift apart while every assertion
/// stayed green — the same failure the canonical template is protected against.
///
/// Placeholders are `$1` profile, `$2` target canonical hash, `$3` domain.
///
/// A macro rather than a `const` because the repository refuses runtime-formatted SQL — rightly —
/// so the two call sites splice this in with `concat!` at compile time. The shipped string is
/// still a literal, and there is still exactly one copy of the ordering.
macro_rules! representative_ranking {
    () => {
        r#"
SELECT candidate.problem_ref,candidate.signal_ref,candidate.distance,
       /* Slots two and three answer different questions, so one ORDER BY cannot fill both. Slot
          two asks who covers ground the lead does not — a different account, a different note.
          Slot three asks who sits furthest from the lead, so the three together span the Problem
          instead of crowding around it. */
       CASE WHEN candidate.reach_rank<=2 THEN candidate.reach_rank
            ELSE 2+row_number() OVER (
              PARTITION BY candidate.problem_ref,(candidate.reach_rank<=2)
              ORDER BY candidate.lead_distance DESC,
                       candidate.created_at,candidate.signal_ref)
       END AS rank
FROM (
  SELECT member.*,
         row_number() OVER (
           PARTITION BY member.problem_ref
           ORDER BY member.is_lead DESC,
                    (member.author_external_id IS DISTINCT FROM member.lead_author) DESC,
                    (member.content_public_ref IS DISTINCT FROM member.lead_content) DESC,
                    member.created_at,member.signal_ref) AS reach_rank
  FROM (
  SELECT membership.problem_ref,signal.signal_ref,signal.created_at,
         source.author_external_id,study_target.content_public_ref,
         cache.embedding OPERATOR(public.<=>) (SELECT embedding FROM target) AS distance,
         (signal.signal_ref = first_value(signal.signal_ref) OVER lead_window) AS is_lead,
         first_value(source.author_external_id) OVER lead_window AS lead_author,
         first_value(study_target.content_public_ref) OVER lead_window AS lead_content,
         cache.embedding OPERATOR(public.<=>)
           first_value(cache.embedding) OVER lead_window AS lead_distance
  FROM linggan_comment_study_problem_membership membership
  JOIN linggan_comment_study_problem problem USING(problem_ref)
  JOIN linggan_comment_study_problem_revision revision
    ON revision.revision_ref=problem.current_revision_ref
  JOIN linggan_comment_study_signal signal ON signal.signal_ref=membership.signal_ref
  JOIN linggan_comment_study_target study_target ON study_target.target_ref=signal.target_ref
  JOIN linggan_material_comment source ON source.material_ref=study_target.source_ref
  JOIN linggan_comment_study_embedding_cache cache
    ON cache.profile_ref=$1 AND cache.canonical_hash=signal.canonical_hash
  WHERE problem.domain_ref=$3 AND problem.state='active'
  WINDOW lead_window AS (
    PARTITION BY membership.problem_ref
    ORDER BY (signal.signal_ref = ANY(revision.seed_signal_refs)) DESC,
             signal.created_at,signal.signal_ref
    ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING)
  ) member
) candidate
"#
    };
}

/// The Signals a Problem is additionally reachable through, in the order the ranking chose them.
/// Exposed so the ordering rules can be asserted directly rather than inferred from which Problem
/// happened to surface.
pub async fn problem_representatives(
    database: &Database,
    profile_ref: Uuid,
    target_canonical_hash: &str,
    domain_ref: Uuid,
    maximum: i64,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "WITH target AS ( \
           SELECT embedding FROM linggan_comment_study_embedding_cache \
           WHERE profile_ref=$1 AND canonical_hash=$2), \
         ranked AS (",
        representative_ranking!(),
        ") SELECT signal_ref FROM ranked WHERE rank<=$4 ORDER BY rank"
    ))
    .bind(profile_ref)
    .bind(target_canonical_hash)
    .bind(domain_ref)
    .bind(maximum)
    .fetch_all(database.pool())
    .await
}

/// Path B. Cores and representatives are ranked in one space and then folded per Problem by the
/// smaller distance, so a Problem reached through a member Signal competes on equal terms with one
/// reached through its core — and a Problem still appears exactly once.
///
/// Representatives are supplementary: a Problem with none stays in the catalogue through its core.
async fn nearest_problems(
    database: &Database,
    profile_ref: Uuid,
    domain_ref: Uuid,
    canonical_hash: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "WITH target AS ( \
           SELECT embedding FROM linggan_comment_study_embedding_cache \
           WHERE profile_ref=$1 AND canonical_hash=$2), \
         candidate AS ( \
           SELECT problem.problem_ref, \
                  cache.embedding OPERATOR(public.<=>) (SELECT embedding FROM target) AS distance \
           FROM linggan_comment_study_problem problem \
           JOIN linggan_comment_study_problem_revision revision \
             ON revision.revision_ref=problem.current_revision_ref \
           JOIN linggan_comment_study_embedding_cache cache \
             ON cache.profile_ref=$1 AND cache.canonical_hash=revision.canonical_hash \
           WHERE problem.domain_ref=$3 AND problem.state='active' \
           UNION ALL \
           SELECT representative.problem_ref,representative.distance FROM (",
        representative_ranking!(),
        ") representative WHERE representative.rank<=$4) \
         SELECT problem_ref FROM candidate \
         GROUP BY problem_ref ORDER BY min(distance),problem_ref LIMIT $5"
    ))
    .bind(profile_ref)
    .bind(canonical_hash)
    .bind(domain_ref)
    .bind(MAX_REPRESENTATIVES_PER_PROBLEM)
    .bind(MAX_PROBLEM_CANDIDATES)
    .fetch_all(database.pool())
    .await
}

/// Path C. Without it a first eligible Signal in an empty catalogue would have nothing to be
/// compared with, and every later one would look equally novel.
///
/// The target itself is excluded, but a different Signal with the same canonical sentence stays
/// eligible for recall.  Canonical equality is evidence that two expressions deserve comparison,
/// not evidence that they came from the same person: source and author independence are enforced
/// by the pairing admission boundary after recall has ranked the pool.
async fn unmerged_pool(
    database: &Database,
    profile_ref: Uuid,
    domain_ref: Uuid,
    signal_ref: Uuid,
    canonical_hash: &str,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "WITH target AS ( \
           SELECT embedding FROM linggan_comment_study_embedding_cache \
           WHERE profile_ref=$1 AND canonical_hash=$2) \
         SELECT signal.signal_ref FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target study_target USING(target_ref) \
         JOIN linggan_comment_study_work work \
           ON work.run_ref=study_target.run_ref \
          AND work.content_public_ref=study_target.content_public_ref \
         JOIN linggan_comment_study_embedding_cache cache \
           ON cache.profile_ref=$1 AND cache.canonical_hash=signal.canonical_hash \
         WHERE work.domain_ref=$3 AND signal.eligibility_state='eligible' \
           AND signal.signal_ref<>$4 \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_study_problem_membership membership \
             WHERE membership.signal_ref=signal.signal_ref) \
         ORDER BY cache.embedding OPERATOR(public.<=>) (SELECT embedding FROM target),signal.signal_ref LIMIT $5",
    )
    .bind(profile_ref)
    .bind(canonical_hash)
    .bind(domain_ref)
    .bind(signal_ref)
    .bind(MAX_POOL_CANDIDATES)
    .fetch_all(database.pool())
    .await
}
