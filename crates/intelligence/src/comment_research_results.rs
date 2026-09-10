//! Frozen result revisions and time-window observations for COMMENT-RESEARCH-RESET-001.
//!
//! A revision publishes only after a fully completed Run has assigned every problem-bearing Atom.
//! It computes facts from that frozen input manifest; no tab reads in-flight rows, recomputes an
//! overview, or turns a partial model outcome into a trend.

use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

const MIN_COMPARABLE_COMMENTS: i64 = 3;
const MIN_COMPARABLE_WORKS: i64 = 1;
const MIN_SHARE_DELTA: f64 = 0.10;
const MIN_NEW_PROBLEM_COMMENTS: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultPublicationReceipt {
    pub result_revision_ref: Uuid,
    pub run_ref: Uuid,
    pub current_window_start: String,
    pub current_window_end: String,
    pub published_observations: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchResultError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the research Run is not fully completed")]
    RunNotCompleted,
    #[error("a frozen Run source is no longer readable")]
    SourceUnavailable,
    #[error("one or more problem-bearing Atoms have no current Problem membership")]
    MembershipIncomplete,
    #[error("the persisted run state is invalid for publication")]
    InvalidRun,
}

#[derive(Debug, Clone)]
struct Window {
    baseline_start: String,
    baseline_end: String,
    current_start: String,
    current_end: String,
}

#[derive(Debug, Clone, Copy)]
struct WindowCounts {
    comments: i64,
    comment_denominator: i64,
    works: i64,
    work_denominator: i64,
}

impl WindowCounts {
    fn comment_share(self) -> f64 {
        ratio(self.comments, self.comment_denominator)
    }

    fn work_share(self) -> f64 {
        ratio(self.works, self.work_denominator)
    }
}

#[derive(Debug, Clone, Copy)]
struct ProblemCounts {
    problem_ref: Uuid,
    definition_revision: i32,
    baseline_comments: i64,
    baseline_works: i64,
    current_comments: i64,
    current_works: i64,
}

#[derive(Debug, Clone, Copy)]
enum ObservationKind {
    Rising,
    Falling,
    Spreading,
    NewlyObserved,
}

impl ObservationKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Falling => "falling",
            Self::Spreading => "spreading",
            Self::NewlyObserved => "newly_observed",
        }
    }
}

/// Publishes exactly one immutable result revision for a completed Run. Repeating the call returns
/// the existing revision; it never edits windows, counts, memberships, or observations in place.
pub async fn publish_result_revision(
    database: &Database,
    run_ref: Uuid,
) -> Result<ResultPublicationReceipt, CommentResearchResultError> {
    let mut transaction = database.pool().begin().await?;
    if let Some(existing) = existing_publication(&mut transaction, run_ref).await? {
        transaction.commit().await?;
        return Ok(existing);
    }
    let run = lock_publishable_run(&mut transaction, run_ref).await?;
    let window = publication_window(&mut transaction, run.as_of.clone()).await?;
    let denominators = read_denominators(&mut transaction, run_ref, &window).await?;
    let problems = read_problem_counts(&mut transaction, run_ref, &window).await?;
    let result_revision_ref = insert_building_revision(
        &mut transaction,
        run_ref,
        &run,
        &denominators,
        problems.len(),
    )
    .await?;
    let observation_count = publish_problem_facts(
        &mut transaction,
        result_revision_ref,
        problems,
        &window,
        &denominators,
    )
    .await?;
    publish_revision(&mut transaction, result_revision_ref).await?;
    transaction.commit().await?;
    Ok(ResultPublicationReceipt {
        result_revision_ref,
        run_ref,
        current_window_start: window.current_start,
        current_window_end: window.current_end,
        published_observations: observation_count,
    })
}

struct PublicationRun {
    as_of: String,
    manifest_hash: String,
    extraction_rule_hash: String,
    membership_policy_hash: String,
}

async fn lock_publishable_run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<PublicationRun, CommentResearchResultError> {
    let run = sqlx::query(
        "SELECT run.state,run.as_of::text AS as_of,run.manifest_hash, \
                policy.extraction_rule_hash,policy.membership_policy_hash \
         FROM linggan_comment_research_run run \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
         WHERE run.run_ref=$1 FOR UPDATE OF run",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(CommentResearchResultError::InvalidRun)?;
    if run.get::<String, _>("state") != "completed" {
        return Err(CommentResearchResultError::RunNotCompleted);
    }
    let unreadable: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 \
             FROM linggan_comment_research_run_item item \
             JOIN linggan_comment_research_derivation derivation \
               ON derivation.derivation_ref=item.derivation_ref \
             WHERE item.run_ref=$1 \
               AND NOT EXISTS( \
                   SELECT 1 FROM linggan_comment_research_derivation_readable readable \
                   WHERE readable.derivation_ref=derivation.derivation_ref \
               ) \
         )",
    )
    .bind(run_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if unreadable {
        return Err(CommentResearchResultError::SourceUnavailable);
    }
    let unassigned: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 \
             FROM linggan_comment_research_atom atom \
             JOIN linggan_comment_research_run_item item \
               ON item.run_ref=atom.run_ref AND item.derivation_ref=atom.derivation_ref \
             WHERE atom.run_ref=$1 AND item.state='succeeded' \
               AND atom.kind IN ('problem','need') \
               AND NOT EXISTS( \
                   SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                   WHERE membership.atom_ref=atom.atom_ref AND membership.current \
               ) \
         )",
    )
    .bind(run_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if unassigned {
        return Err(CommentResearchResultError::MembershipIncomplete);
    }

    Ok(PublicationRun {
        as_of: run.get("as_of"),
        manifest_hash: run.get("manifest_hash"),
        extraction_rule_hash: run.get("extraction_rule_hash"),
        membership_policy_hash: run.get("membership_policy_hash"),
    })
}

async fn insert_building_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    run: &PublicationRun,
    denominators: &Denominators,
    problem_count: usize,
) -> Result<Uuid, CommentResearchResultError> {
    let result_revision_ref = Uuid::new_v4();
    let input_counts = json!({
        "analyzedCommentCount":denominators.current.comment_denominator + denominators.baseline.comment_denominator,
        "baselineCommentDenominator":denominators.baseline.comment_denominator,
        "currentCommentDenominator":denominators.current.comment_denominator,
        "baselineWorkDenominator":denominators.baseline.work_denominator,
        "currentWorkDenominator":denominators.current.work_denominator,
        "problemCount":problem_count,
    });
    let policy_hashes = json!({
        "extraction":run.extraction_rule_hash,
        "membership":run.membership_policy_hash,
        "window":"Asia/Shanghai.complete-7d.v1",
    });
    sqlx::query(
        "INSERT INTO linggan_comment_research_result_revision( \
             result_revision_ref,run_ref,state,as_of,manifest_hash,policy_hashes,input_counts \
         ) VALUES($1,$2,'building',$3::timestamptz,$4,$5,$6)",
    )
    .bind(result_revision_ref)
    .bind(run_ref)
    .bind(&run.as_of)
    .bind(&run.manifest_hash)
    .bind(policy_hashes)
    .bind(input_counts)
    .execute(&mut **transaction)
    .await?;
    Ok(result_revision_ref)
}

async fn publish_problem_facts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    result_revision_ref: Uuid,
    problems: Vec<ProblemCounts>,
    window: &Window,
    denominators: &Denominators,
) -> Result<usize, CommentResearchResultError> {
    let mut observation_count = 0usize;
    for problem in problems {
        let baseline = WindowCounts {
            comments: problem.baseline_comments,
            comment_denominator: denominators.baseline.comment_denominator,
            works: problem.baseline_works,
            work_denominator: denominators.baseline.work_denominator,
        };
        let current = WindowCounts {
            comments: problem.current_comments,
            comment_denominator: denominators.current.comment_denominator,
            works: problem.current_works,
            work_denominator: denominators.current.work_denominator,
        };
        insert_stat(
            &mut *transaction,
            result_revision_ref,
            problem,
            "baseline",
            window,
            baseline,
        )
        .await?;
        insert_stat(
            &mut *transaction,
            result_revision_ref,
            problem,
            "current",
            window,
            current,
        )
        .await?;
        observation_count += publish_observations(
            &mut *transaction,
            result_revision_ref,
            problem,
            window,
            baseline,
            current,
        )
        .await?;
    }
    Ok(observation_count)
}

async fn publish_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    result_revision_ref: Uuid,
) -> Result<(), CommentResearchResultError> {
    sqlx::query(
        "UPDATE linggan_comment_research_result_revision \
         SET state='published',published_at=scope_001_now() \
         WHERE result_revision_ref=$1 AND state='building'",
    )
    .bind(result_revision_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

struct Denominators {
    baseline: WindowCounts,
    current: WindowCounts,
}

async fn existing_publication(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<Option<ResultPublicationReceipt>, CommentResearchResultError> {
    let row = sqlx::query(
        "SELECT result_revision_ref,run_ref, \
                (date_trunc('day',as_of AT TIME ZONE 'Asia/Shanghai') - interval '7 days')::text AS current_window_start, \
                date_trunc('day',as_of AT TIME ZONE 'Asia/Shanghai')::text AS current_window_end, \
                (SELECT count(*) FROM linggan_comment_research_change_observation observation \
                 WHERE observation.result_revision_ref=result.result_revision_ref AND observation.status='published') \
                 AS published_observations \
         FROM linggan_comment_research_result_revision result \
         WHERE run_ref=$1 AND state='published'",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(row.map(|row| ResultPublicationReceipt {
        result_revision_ref: row.get("result_revision_ref"),
        run_ref: row.get("run_ref"),
        current_window_start: row.get("current_window_start"),
        current_window_end: row.get("current_window_end"),
        published_observations: usize::try_from(row.get::<i64, _>("published_observations"))
            .unwrap_or(0),
    }))
}

async fn publication_window(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    as_of: String,
) -> Result<Window, CommentResearchResultError> {
    let row = sqlx::query(
        "WITH bounds AS ( \
             SELECT date_trunc('day',$1::timestamptz AT TIME ZONE 'Asia/Shanghai') \
                    AT TIME ZONE 'Asia/Shanghai' AS current_end \
         ) \
         SELECT (current_end - interval '14 days')::text AS baseline_start, \
                (current_end - interval '7 days')::text AS baseline_end, \
                (current_end - interval '7 days')::text AS current_start, \
                current_end::text AS current_end \
         FROM bounds",
    )
    .bind(as_of)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(Window {
        baseline_start: row.get("baseline_start"),
        baseline_end: row.get("baseline_end"),
        current_start: row.get("current_start"),
        current_end: row.get("current_end"),
    })
}

async fn read_denominators(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    window: &Window,
) -> Result<Denominators, CommentResearchResultError> {
    let row = sqlx::query(
        "SELECT \
             count(DISTINCT derivation.source_ref) FILTER (WHERE source.observed_at::timestamptz >= $2::timestamptz AND source.observed_at::timestamptz < $3::timestamptz) AS baseline_comments, \
             count(DISTINCT source.content_public_ref) FILTER (WHERE source.observed_at::timestamptz >= $2::timestamptz AND source.observed_at::timestamptz < $3::timestamptz) AS baseline_works, \
             count(DISTINCT derivation.source_ref) FILTER (WHERE source.observed_at::timestamptz >= $3::timestamptz AND source.observed_at::timestamptz < $4::timestamptz) AS current_comments, \
             count(DISTINCT source.content_public_ref) FILTER (WHERE source.observed_at::timestamptz >= $3::timestamptz AND source.observed_at::timestamptz < $4::timestamptz) AS current_works \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation derivation USING(derivation_ref) \
         JOIN linggan_material_comment source ON source.material_ref=derivation.source_ref \
         WHERE item.run_ref=$1 AND item.state IN ('succeeded','no_signal')",
    )
    .bind(run_ref)
    .bind(&window.baseline_start)
    .bind(&window.current_start)
    .bind(&window.current_end)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(Denominators {
        baseline: WindowCounts {
            comments: 0,
            comment_denominator: row.get("baseline_comments"),
            works: 0,
            work_denominator: row.get("baseline_works"),
        },
        current: WindowCounts {
            comments: 0,
            comment_denominator: row.get("current_comments"),
            works: 0,
            work_denominator: row.get("current_works"),
        },
    })
}

async fn read_problem_counts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    window: &Window,
) -> Result<Vec<ProblemCounts>, CommentResearchResultError> {
    let rows = sqlx::query(
        "SELECT membership.problem_ref,membership.definition_revision, \
             count(DISTINCT derivation.source_ref) FILTER (WHERE source.observed_at::timestamptz >= $2::timestamptz AND source.observed_at::timestamptz < $3::timestamptz) AS baseline_comments, \
             count(DISTINCT source.content_public_ref) FILTER (WHERE source.observed_at::timestamptz >= $2::timestamptz AND source.observed_at::timestamptz < $3::timestamptz) AS baseline_works, \
             count(DISTINCT derivation.source_ref) FILTER (WHERE source.observed_at::timestamptz >= $3::timestamptz AND source.observed_at::timestamptz < $4::timestamptz) AS current_comments, \
             count(DISTINCT source.content_public_ref) FILTER (WHERE source.observed_at::timestamptz >= $3::timestamptz AND source.observed_at::timestamptz < $4::timestamptz) AS current_works \
         FROM linggan_comment_research_atom_problem_membership membership \
         JOIN linggan_comment_research_atom atom ON atom.atom_ref=membership.atom_ref \
         JOIN linggan_comment_research_run_item item \
           ON item.run_ref=atom.run_ref AND item.derivation_ref=atom.derivation_ref \
         JOIN linggan_comment_research_derivation derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         JOIN linggan_material_comment source ON source.material_ref=derivation.source_ref \
         WHERE atom.run_ref=$1 AND item.state='succeeded' AND membership.current \
         GROUP BY membership.problem_ref,membership.definition_revision",
    )
    .bind(run_ref)
    .bind(&window.baseline_start)
    .bind(&window.current_start)
    .bind(&window.current_end)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ProblemCounts {
            problem_ref: row.get("problem_ref"),
            definition_revision: row.get("definition_revision"),
            baseline_comments: row.get("baseline_comments"),
            baseline_works: row.get("baseline_works"),
            current_comments: row.get("current_comments"),
            current_works: row.get("current_works"),
        })
        .collect())
}

async fn insert_stat(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    result_revision_ref: Uuid,
    problem: ProblemCounts,
    kind: &str,
    window: &Window,
    counts: WindowCounts,
) -> Result<(), CommentResearchResultError> {
    let (start, end) = if kind == "baseline" {
        (&window.baseline_start, &window.baseline_end)
    } else {
        (&window.current_start, &window.current_end)
    };
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_window_stat( \
             result_revision_ref,problem_ref,definition_revision,window_kind,window_start,window_end, \
             comment_count,comment_denominator,work_count,work_denominator \
         ) VALUES($1,$2,$3,$4,$5::timestamptz,$6::timestamptz,$7,$8,$9,$10)",
    )
    .bind(result_revision_ref)
    .bind(problem.problem_ref)
    .bind(problem.definition_revision)
    .bind(kind)
    .bind(start)
    .bind(end)
    .bind(counts.comments)
    .bind(counts.comment_denominator)
    .bind(counts.works)
    .bind(counts.work_denominator)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn publish_observations(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    result_revision_ref: Uuid,
    problem: ProblemCounts,
    window: &Window,
    baseline: WindowCounts,
    current: WindowCounts,
) -> Result<usize, CommentResearchResultError> {
    let evidence = observation_evidence(window, baseline, current);
    if baseline.comment_denominator < MIN_COMPARABLE_COMMENTS
        || current.comment_denominator < MIN_COMPARABLE_COMMENTS
        || baseline.work_denominator < MIN_COMPARABLE_WORKS
        || current.work_denominator < MIN_COMPARABLE_WORKS
    {
        insert_observation(
            transaction,
            result_revision_ref,
            problem,
            None,
            "not_comparable",
            "insufficient_window_coverage",
            evidence,
        )
        .await?;
        return Ok(0);
    }

    let mut count = 0usize;
    if current.comments >= MIN_NEW_PROBLEM_COMMENTS
        && baseline.comments == 0
        && first_observed_in_system(transaction, problem, &window.current_start).await?
    {
        insert_observation(
            transaction,
            result_revision_ref,
            problem,
            Some(ObservationKind::NewlyObserved),
            "published",
            "first_observed_in_comparable_history",
            evidence.clone(),
        )
        .await?;
        count += 1;
    }
    let comment_delta = current.comment_share() - baseline.comment_share();
    if comment_delta >= MIN_SHARE_DELTA {
        insert_observation(
            transaction,
            result_revision_ref,
            problem,
            Some(ObservationKind::Rising),
            "published",
            "comment_share_increased",
            evidence.clone(),
        )
        .await?;
        count += 1;
    } else if comment_delta <= -MIN_SHARE_DELTA {
        insert_observation(
            transaction,
            result_revision_ref,
            problem,
            Some(ObservationKind::Falling),
            "published",
            "comment_share_decreased",
            evidence.clone(),
        )
        .await?;
        count += 1;
    }
    if current.work_share() - baseline.work_share() >= MIN_SHARE_DELTA {
        insert_observation(
            transaction,
            result_revision_ref,
            problem,
            Some(ObservationKind::Spreading),
            "published",
            "work_coverage_increased",
            evidence,
        )
        .await?;
        count += 1;
    }
    Ok(count)
}

async fn first_observed_in_system(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    problem: ProblemCounts,
    current_start: &str,
) -> Result<bool, CommentResearchResultError> {
    let earlier: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 \
             FROM linggan_comment_research_atom_problem_membership membership \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_readable derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_material_comment source ON source.material_ref=derivation.source_ref \
             WHERE membership.problem_ref=$1 AND membership.current \
               AND source.observed_at::timestamptz < $2::timestamptz \
         )",
    )
    .bind(problem.problem_ref)
    .bind(current_start)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(!earlier)
}

#[allow(clippy::too_many_arguments)]
async fn insert_observation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    result_revision_ref: Uuid,
    problem: ProblemCounts,
    kind: Option<ObservationKind>,
    status: &str,
    reason_code: &str,
    evidence: Value,
) -> Result<(), CommentResearchResultError> {
    sqlx::query(
        "INSERT INTO linggan_comment_research_change_observation( \
             observation_ref,result_revision_ref,problem_ref,definition_revision,kind,status,reason_code,evidence \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(result_revision_ref)
    .bind(problem.problem_ref)
    .bind(problem.definition_revision)
    .bind(kind.map(ObservationKind::as_db))
    .bind(status)
    .bind(reason_code)
    .bind(evidence)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn observation_evidence(window: &Window, baseline: WindowCounts, current: WindowCounts) -> Value {
    json!({
        "baseline":{
            "start":window.baseline_start,
            "end":window.baseline_end,
            "commentCount":baseline.comments,
            "commentDenominator":baseline.comment_denominator,
            "commentShare":baseline.comment_share(),
            "workCount":baseline.works,
            "workDenominator":baseline.work_denominator,
            "workShare":baseline.work_share(),
        },
        "current":{
            "start":window.current_start,
            "end":window.current_end,
            "commentCount":current.comments,
            "commentDenominator":current.comment_denominator,
            "commentShare":current.comment_share(),
            "workCount":current.works,
            "workDenominator":current.work_denominator,
            "workShare":current.work_share(),
        },
        "thresholds":{
            "minimumComparableComments":MIN_COMPARABLE_COMMENTS,
            "minimumComparableWorks":MIN_COMPARABLE_WORKS,
            "minimumShareDelta":MIN_SHARE_DELTA,
            "minimumNewProblemComments":MIN_NEW_PROBLEM_COMMENTS,
        }
    })
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios_are_zero_without_a_denominator() {
        assert_eq!(ratio(3, 0), 0.0);
        assert_eq!(ratio(1, 4), 0.25);
    }
}
