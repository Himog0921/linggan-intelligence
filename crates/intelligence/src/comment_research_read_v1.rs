//! Read models for the single COMMENT-RESEARCH-RESET-001 semantic kernel.
//!
//! Overview, Problems, and Changes each read one published ResultRevision only.  They
//! intentionally do not join the legacy comment-intelligence projections, expose embedding queue
//! state, or recompute trends from a mutable overview.  A caller may ask for a particular
//! readable revision; otherwise the latest readable published revision is selected.
//!
//! Voices answers a different question: what currently readable ordinary-user evidence is
//! available to research?  It reads the current derivation head directly and is deliberately
//! available before a ResultRevision exists.  This is a read-only evidence view: it never derives
//! a comment, claims work, or calls a model.

use crate::comment_research_kernel::DERIVATION_VERSION;
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Row, Transaction};
use uuid::Uuid;

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 50;
const MAX_OFFSET: i64 = 10_000;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentResearchV1ReadQuery {
    pub result_revision_ref: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl CommentResearchV1ReadQuery {
    fn page(&self) -> Result<(i64, i64), CommentResearchV1ReadError> {
        let limit = self.limit.unwrap_or(DEFAULT_PAGE_SIZE);
        let offset = self.offset.unwrap_or(0);
        if !(1..=MAX_PAGE_SIZE).contains(&limit) || !(0..=MAX_OFFSET).contains(&offset) {
            return Err(CommentResearchV1ReadError::InvalidQuery);
        }
        Ok((limit, offset))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchV1ReadError {
    #[error("invalid comment research V1 read query")]
    InvalidQuery,
    #[error("the requested published research result is not readable")]
    ResultUnavailable,
    #[error("comment research V1 schema is unavailable")]
    SchemaUnavailable,
    #[error("comment research V1 storage failure")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
struct ResultContext {
    result_revision_ref: Uuid,
    run_ref: Uuid,
    as_of: String,
    published_at: String,
    current_window_start: String,
    current_window_end: String,
    baseline_window_start: String,
    baseline_window_end: String,
    input_counts: Value,
}

impl ResultContext {
    fn envelope(&self) -> Value {
        json!({
            "resultRevisionRef":self.result_revision_ref,
            "runRef":self.run_ref,
            "asOf":self.as_of,
            "publishedAt":self.published_at,
            "inputCounts":self.input_counts,
            "comparison":{
                "timeZone":"Asia/Shanghai",
                "baseline":{"start":self.baseline_window_start,"end":self.baseline_window_end},
                "current":{"start":self.current_window_start,"end":self.current_window_end},
            },
        })
    }
}

/// The API checks this before serving a V1 endpoint, so a partially migrated runtime never
/// returns a synthetic empty research screen.
pub async fn schema_ready(database: &Database) -> Result<bool, CommentResearchV1ReadError> {
    Ok(sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_research_result_revision_readable') IS NOT NULL \
                AND to_regclass('linggan_comment_research_problem_window_stat') IS NOT NULL \
                AND to_regclass('linggan_comment_research_change_observation') IS NOT NULL \
                AND to_regclass('linggan_comment_research_atom_problem_membership') IS NOT NULL \
                AND to_regclass('linggan_comment_daily_batch') IS NULL \
                AND to_regclass('linggan_ci_problem') IS NULL \
                AND EXISTS(SELECT 1 FROM pg_attribute \
                    WHERE attrelid='linggan_comment_research_derivation'::regclass \
                      AND attname='derivation_input_hash' AND NOT attisdropped)",
    )
    .fetch_one(database.pool())
    .await?)
}

/// Overview deliberately reports current research coverage and currently represented Problems.
/// It has no change observations: the Changes endpoint owns that different user question.
pub async fn read_overview(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let mut transaction = begin_read(database).await?;
    let context = resolve_result(&mut transaction, query.result_revision_ref).await?;
    let problems: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'problemRef',definition.problem_ref, \
             'definitionRevision',definition.revision, \
             'name',definition.name, \
             'meaning',definition.meaning, \
             'currentCommentCount',current_stat.comment_count, \
             'currentCommentShare',CASE WHEN current_stat.comment_denominator=0 THEN 0 ELSE current_stat.comment_count::double precision/current_stat.comment_denominator END, \
             'currentWorkCount',current_stat.work_count, \
             'currentWorkShare',CASE WHEN current_stat.work_denominator=0 THEN 0 ELSE current_stat.work_count::double precision/current_stat.work_denominator END, \
             'baselineCommentCount',baseline_stat.comment_count, \
             'baselineWorkCount',baseline_stat.work_count \
         ) \
         FROM linggan_comment_research_problem_window_stat current_stat \
         JOIN linggan_comment_research_problem_window_stat baseline_stat \
           ON baseline_stat.result_revision_ref=current_stat.result_revision_ref \
          AND baseline_stat.problem_ref=current_stat.problem_ref \
          AND baseline_stat.definition_revision=current_stat.definition_revision \
          AND baseline_stat.window_kind='baseline' \
         JOIN linggan_comment_research_problem_definition definition \
           ON definition.problem_ref=current_stat.problem_ref \
          AND definition.revision=current_stat.definition_revision \
         WHERE current_stat.result_revision_ref=$1 AND current_stat.window_kind='current' \
         ORDER BY CASE WHEN current_stat.comment_denominator=0 THEN baseline_stat.comment_count ELSE current_stat.comment_count END DESC, \
                  CASE WHEN current_stat.work_denominator=0 THEN baseline_stat.work_count ELSE current_stat.work_count END DESC, \
                  definition.name,definition.problem_ref \
         LIMIT 8",
    )
    .bind(context.result_revision_ref)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"overview",
        "result":context.envelope(),
        "currentProblems":problems,
    }))
}

/// Reads current, readable ordinary-user evidence independently of ResultRevision publication.
///
/// A row carries the raw evidence and separately derived research text.  `researchStatus` is the
/// most recent RunItem state for this exact current derivation; it is `unresearched` when this
/// derivation has not entered a Run.  It is not a mutable overview, and this function never
/// derives comments, claims work, or calls a model.  Author replies, identity-unknown comments,
/// and deterministically dropped/anomalous entries remain excluded by the `eligible` filter.
/// `derivation_current` retains an immutable head for every historical derivation version, so this
/// view explicitly selects only the canonical V1 version that new Runs can read.
pub async fn read_voices(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let (limit, offset) = query.page()?;
    let mut transaction = begin_read(database).await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) \
         FROM linggan_comment_research_derivation_current derivation \
         WHERE derivation.derivation_version=$1 \
           AND derivation.author_role='ordinary_user' AND derivation.eligibility='eligible'",
    )
    .bind(DERIVATION_VERSION)
    .fetch_one(&mut *transaction)
    .await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'sourceRef',source.material_ref, \
             'workRef',source.content_public_ref, \
             'workTitle',work.title, \
             'commentText',source.body_text, \
             'researchText',derivation.research_text, \
             'researchStatus',COALESCE(latest_item.state,'unresearched'), \
             'researchFailureCode',latest_item.failure_code, \
             'observedAt',source.observed_at, \
             'isReply',source.is_reply \
         ) \
         FROM linggan_comment_research_derivation_current derivation \
         JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
         LEFT JOIN LATERAL ( \
             SELECT detail.title \
             FROM linggan_material_content_detail detail \
             WHERE detail.content_public_ref=source.content_public_ref \
             ORDER BY detail.created_at DESC,detail.material_ref DESC \
             LIMIT 1 \
         ) work ON true \
         LEFT JOIN LATERAL ( \
             SELECT item.state,item.failure_code \
             FROM linggan_comment_research_run_item item \
             WHERE item.derivation_ref=derivation.derivation_ref \
             ORDER BY item.updated_at DESC,item.run_ref DESC \
             LIMIT 1 \
         ) latest_item ON true \
         WHERE derivation.derivation_version=$1 \
           AND derivation.author_role='ordinary_user' AND derivation.eligibility='eligible' \
         ORDER BY source.observed_at::timestamptz DESC,source.material_ref DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(DERIVATION_VERSION)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"voices",
        "source":{"kind":"current_readable_ordinary_user"},
        "page":{"total":total,"limit":limit,"offset":offset,"items":items},
    }))
}

/// Problems are stable Definitions and their frozen window facts.  This endpoint does not fold
/// in ChangeObservation rows; callers should use `read_changes` for signals and reasons.
pub async fn read_problems(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let (limit, offset) = query.page()?;
    let mut transaction = begin_read(database).await?;
    let context = resolve_result(&mut transaction, query.result_revision_ref).await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_problem_window_stat \
         WHERE result_revision_ref=$1 AND window_kind='current'",
    )
    .bind(context.result_revision_ref)
    .fetch_one(&mut *transaction)
    .await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'problemRef',definition.problem_ref, \
             'definitionRevision',definition.revision, \
             'name',definition.name, \
             'meaning',definition.meaning, \
             'current',jsonb_build_object( \
                'commentCount',current_stat.comment_count, \
                'commentDenominator',current_stat.comment_denominator, \
                'commentShare',CASE WHEN current_stat.comment_denominator=0 THEN 0 ELSE current_stat.comment_count::double precision/current_stat.comment_denominator END, \
                'workCount',current_stat.work_count, \
                'workDenominator',current_stat.work_denominator, \
                'workShare',CASE WHEN current_stat.work_denominator=0 THEN 0 ELSE current_stat.work_count::double precision/current_stat.work_denominator END \
             ), \
             'baseline',jsonb_build_object( \
                'commentCount',baseline_stat.comment_count, \
                'commentDenominator',baseline_stat.comment_denominator, \
                'commentShare',CASE WHEN baseline_stat.comment_denominator=0 THEN 0 ELSE baseline_stat.comment_count::double precision/baseline_stat.comment_denominator END, \
                'workCount',baseline_stat.work_count, \
                'workDenominator',baseline_stat.work_denominator, \
                'workShare',CASE WHEN baseline_stat.work_denominator=0 THEN 0 ELSE baseline_stat.work_count::double precision/baseline_stat.work_denominator END \
             ), \
             'evidenceAtomCount',( \
                SELECT count(*) FROM linggan_comment_research_atom_problem_membership membership \
                JOIN linggan_comment_research_atom atom USING(atom_ref) \
                WHERE membership.current AND membership.problem_ref=definition.problem_ref \
                  AND membership.definition_revision=definition.revision AND atom.run_ref=$2 \
             ) \
         ) \
         FROM linggan_comment_research_problem_window_stat current_stat \
         JOIN linggan_comment_research_problem_window_stat baseline_stat \
           ON baseline_stat.result_revision_ref=current_stat.result_revision_ref \
          AND baseline_stat.problem_ref=current_stat.problem_ref \
          AND baseline_stat.definition_revision=current_stat.definition_revision \
          AND baseline_stat.window_kind='baseline' \
         JOIN linggan_comment_research_problem_definition definition \
           ON definition.problem_ref=current_stat.problem_ref \
          AND definition.revision=current_stat.definition_revision \
         WHERE current_stat.result_revision_ref=$1 AND current_stat.window_kind='current' \
         ORDER BY CASE WHEN current_stat.comment_denominator=0 THEN baseline_stat.comment_count ELSE current_stat.comment_count END DESC, \
                  CASE WHEN current_stat.work_denominator=0 THEN baseline_stat.work_count ELSE current_stat.work_count END DESC, \
                  definition.name,definition.problem_ref \
         LIMIT $3 OFFSET $4",
    )
    .bind(context.result_revision_ref)
    .bind(context.run_ref)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"problems",
        "result":context.envelope(),
        "page":{"total":total,"limit":limit,"offset":offset,"items":items},
    }))
}

/// Change observations are a separate, evidence-bearing answer to “what changed?”  A Problem
/// may legitimately have several signals (for example newly observed, rising, and spreading),
/// while a not-comparable entry tells the UI exactly why it must not manufacture a trend.
pub async fn read_changes(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    query.page()?;
    let mut transaction = begin_read(database).await?;
    let context = resolve_result(&mut transaction, query.result_revision_ref).await?;
    let published: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'observationRef',observation.observation_ref, \
             'problemRef',definition.problem_ref, \
             'definitionRevision',definition.revision, \
             'problemName',definition.name, \
             'problemMeaning',definition.meaning, \
             'kind',observation.kind, \
             'reasonCode',observation.reason_code, \
             'evidence',observation.evidence \
         ) \
         FROM linggan_comment_research_change_observation observation \
         JOIN linggan_comment_research_problem_definition definition \
           ON definition.problem_ref=observation.problem_ref \
          AND definition.revision=observation.definition_revision \
         WHERE observation.result_revision_ref=$1 AND observation.status='published' \
         ORDER BY definition.name,observation.kind,observation.observation_ref",
    )
    .bind(context.result_revision_ref)
    .fetch_all(&mut *transaction)
    .await?;
    let not_comparable: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'problemRef',definition.problem_ref, \
             'definitionRevision',definition.revision, \
             'problemName',definition.name, \
             'reasonCode',observation.reason_code, \
             'evidence',observation.evidence \
         ) \
         FROM linggan_comment_research_change_observation observation \
         JOIN linggan_comment_research_problem_definition definition \
           ON definition.problem_ref=observation.problem_ref \
          AND definition.revision=observation.definition_revision \
         WHERE observation.result_revision_ref=$1 AND observation.status='not_comparable' \
         ORDER BY definition.name,observation.observation_ref",
    )
    .bind(context.result_revision_ref)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"changes",
        "result":context.envelope(),
        "observations":published,
        "notComparable":not_comparable,
    }))
}

/// Run history contains operational truth only.  It reports the V1 Run state, frozen source
/// count, safe invocation-ledger aggregates and whether its already-published result remains
/// readable; it never treats a failed run as an empty successful analysis. Prompt material,
/// model output, provider diagnostics, secrets and invocation IDs remain private.
pub async fn read_runs(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let (limit, offset) = query.page()?;
    let mut transaction = begin_read(database).await?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_run")
        .fetch_one(&mut *transaction)
        .await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
             'runRef',run.run_ref, \
             'state',run.state, \
             'asOf',run.as_of, \
             'createdAt',run.created_at, \
             'finishedAt',run.finished_at, \
             'selectedSources',(SELECT count(*) FROM linggan_comment_research_run_item item WHERE item.run_ref=run.run_ref), \
             'itemStates',COALESCE(( \
                 SELECT jsonb_object_agg(state,count) \
                 FROM (SELECT item.state,count(*) FROM linggan_comment_research_run_item item WHERE item.run_ref=run.run_ref GROUP BY item.state) grouped \
             ),'{}'::jsonb), \
             'itemFailureCounts',COALESCE(( \
                 SELECT jsonb_object_agg(failure_code,count) \
                 FROM (SELECT item.failure_code,count(*) FROM linggan_comment_research_run_item item \
                       WHERE item.run_ref=run.run_ref AND item.failure_code IS NOT NULL \
                       GROUP BY item.failure_code) grouped \
             ),'{}'::jsonb), \
             'exclusionCounts',run.exclusion_counts, \
             'failureCounts',run.failure_counts, \
             'modelExecution',( \
                 SELECT jsonb_build_object( \
                     'callCount',count(*), \
                     'startedCallCount',count(*) FILTER(WHERE invocation.result->>'callStarted'='true'), \
                     'succeededCallCount',count(*) FILTER(WHERE invocation.state='succeeded'), \
                     'failedCallCount',count(*) FILTER(WHERE invocation.state='failed'), \
                     'runningCallCount',count(*) FILTER(WHERE invocation.state='running'), \
                     'stateCounts',COALESCE(( \
                         SELECT jsonb_object_agg(state,call_count) FROM ( \
                             SELECT ledger.state,count(*) AS call_count \
                             FROM linggan_model_invocation ledger \
                             WHERE ledger.result->>'runRef'=run.run_ref::text \
                             GROUP BY ledger.state \
                         ) grouped_states \
                     ),'{}'::jsonb), \
                     'stageCounts',COALESCE(( \
                         SELECT jsonb_object_agg(stage,call_count) FROM ( \
                             SELECT COALESCE(ledger.result->>'stage','unknown') AS stage,count(*) AS call_count \
                             FROM linggan_model_invocation ledger \
                             WHERE ledger.result->>'runRef'=run.run_ref::text \
                             GROUP BY COALESCE(ledger.result->>'stage','unknown') \
                         ) grouped_stages \
                     ),'{}'::jsonb), \
                     'failureCounts',COALESCE(( \
                         SELECT jsonb_object_agg(failure_code,call_count) FROM ( \
                             SELECT ledger.failure_code,count(*) AS call_count \
                             FROM linggan_model_invocation ledger \
                             WHERE ledger.result->>'runRef'=run.run_ref::text \
                               AND ledger.failure_code IS NOT NULL \
                             GROUP BY ledger.failure_code \
                         ) grouped_failures \
                     ),'{}'::jsonb), \
                     'elapsedMs',CASE WHEN count(invocation.elapsed_ms)=0 THEN NULL ELSE sum(invocation.elapsed_ms) END, \
                     'elapsedMeasuredCallCount',count(invocation.elapsed_ms), \
                     'inputTokens',CASE WHEN count(invocation.input_tokens)=0 THEN NULL ELSE sum(invocation.input_tokens) END, \
                     'outputTokens',CASE WHEN count(invocation.output_tokens)=0 THEN NULL ELSE sum(invocation.output_tokens) END, \
                     'usageMeasuredCallCount',count(*) FILTER(WHERE invocation.input_tokens IS NOT NULL AND invocation.output_tokens IS NOT NULL), \
                     'chargedTokens',CASE WHEN count(*)=0 THEN NULL ELSE sum(invocation.charged_tokens) END \
                 ) \
                 FROM linggan_model_invocation invocation \
                 WHERE invocation.result->>'runRef'=run.run_ref::text \
             ), \
             'publishedResult',COALESCE(( \
                 SELECT jsonb_build_object('resultRevisionRef',result.result_revision_ref,'publishedAt',result.published_at,'inputCounts',result.input_counts) \
                 FROM linggan_comment_research_result_revision_readable result \
                 WHERE result.run_ref=run.run_ref \
             ),'null'::jsonb) \
         ) \
         FROM linggan_comment_research_run run \
         ORDER BY run.created_at DESC,run.run_ref DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"runs",
        "page":{"total":total,"limit":limit,"offset":offset,"items":items},
    }))
}

async fn begin_read(
    database: &Database,
) -> Result<Transaction<'_, sqlx::Postgres>, CommentResearchV1ReadError> {
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *transaction)
        .await?;
    Ok(transaction)
}

async fn resolve_result(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    requested: Option<Uuid>,
) -> Result<ResultContext, CommentResearchV1ReadError> {
    let row = sqlx::query(
        "SELECT result.result_revision_ref,result.run_ref,result.as_of::text AS as_of, \
                result.published_at::text AS published_at,result.input_counts, \
                ((date_trunc('day',result.as_of AT TIME ZONE 'Asia/Shanghai') - interval '14 days') \
                  AT TIME ZONE 'Asia/Shanghai')::text AS baseline_window_start, \
                ((date_trunc('day',result.as_of AT TIME ZONE 'Asia/Shanghai') - interval '7 days') \
                  AT TIME ZONE 'Asia/Shanghai')::text AS baseline_window_end, \
                ((date_trunc('day',result.as_of AT TIME ZONE 'Asia/Shanghai') - interval '7 days') \
                  AT TIME ZONE 'Asia/Shanghai')::text AS current_window_start, \
                (date_trunc('day',result.as_of AT TIME ZONE 'Asia/Shanghai') \
                  AT TIME ZONE 'Asia/Shanghai')::text AS current_window_end \
         FROM linggan_comment_research_result_revision_readable result \
         WHERE ($1::uuid IS NULL OR result.result_revision_ref=$1) \
         ORDER BY result.published_at DESC,result.result_revision_ref DESC \
         LIMIT 1",
    )
    .bind(requested)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(CommentResearchV1ReadError::ResultUnavailable)?;
    Ok(ResultContext {
        result_revision_ref: row.get("result_revision_ref"),
        run_ref: row.get("run_ref"),
        as_of: row.get("as_of"),
        published_at: row.get("published_at"),
        baseline_window_start: row.get("baseline_window_start"),
        baseline_window_end: row.get("baseline_window_end"),
        current_window_start: row.get("current_window_start"),
        current_window_end: row.get("current_window_end"),
        input_counts: row.get("input_counts"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_pagination_outside_the_read_budget() {
        assert!(
            CommentResearchV1ReadQuery {
                limit: Some(51),
                ..Default::default()
            }
            .page()
            .is_err()
        );
        assert!(
            CommentResearchV1ReadQuery {
                offset: Some(-1),
                ..Default::default()
            }
            .page()
            .is_err()
        );
    }
}
