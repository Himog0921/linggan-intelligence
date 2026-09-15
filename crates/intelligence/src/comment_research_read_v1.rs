//! Read models for the single COMMENT-RESEARCH-RESET-001 semantic kernel.
//!
//! Changes reads one published ResultRevision only. Overview and Problems separately read current
//! accepted memberships: a valid atom-to-problem admission becomes cumulative knowledge at once,
//! while a ResultRevision remains the only source for shares, ranks, and change conclusions.
//! The projections intentionally do not expose embedding queues or recompute trends from mutable
//! membership state.
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
    pub problem_view: Option<ProblemReadView>,
}

/// A filter on one current-state read model. `all` deliberately does not mean that every row is
/// a stable Problem: deferred signals carry their own item kind and are excluded from cumulative
/// Problem statistics.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemReadView {
    #[default]
    All,
    Confirmed,
    Deferred,
}

impl ProblemReadView {
    fn as_db(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Confirmed => "confirmed",
            Self::Deferred => "deferred",
        }
    }
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
                      AND attname='derivation_input_hash' AND NOT attisdropped) \
                AND EXISTS(SELECT 1 FROM pg_attribute \
                    WHERE attrelid='linggan_comment_research_atom'::regclass \
                      AND attname='problem_frame' AND NOT attisdropped) \
                AND EXISTS(SELECT 1 FROM pg_attribute \
                    WHERE attrelid='linggan_comment_research_problem_resolution'::regclass \
                      AND attname='decision_kind' AND NOT attisdropped)",
    )
    .fetch_one(database.pool())
    .await?)
}

/// Overview reports cumulative, currently readable accepted memberships. A latest statistics
/// revision is optional metadata: it never gates confirmed membership visibility or adds mutable
/// facts to its frozen window metrics.
pub async fn read_overview(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let mut transaction = begin_read(database).await?;
    let (summary, problems) = read_cumulative_problem_state(&mut transaction, 8, 0).await?;
    let statistics = resolve_optional_result(&mut transaction, query.result_revision_ref).await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"overview",
        "cumulative":summary,
        "currentProblems":problems,
        "statisticsResult":statistics.map(|context|context.envelope()),
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

/// Problems are current accepted Definitions and their cumulative evidence counts. They have no
/// shares, ranks, or frozen-window fields; callers must use a published ResultRevision through
/// Changes for statistical conclusions.
pub async fn read_problems(
    database: &Database,
    query: &CommentResearchV1ReadQuery,
) -> Result<Value, CommentResearchV1ReadError> {
    let (limit, offset) = query.page()?;
    let mut transaction = begin_read(database).await?;
    let (summary, _) = read_cumulative_problem_state(&mut transaction, 1, 0).await?;
    let confirmed_total = summary["problemCount"].as_i64().unwrap_or_default();
    let deferred_total = read_deferred_signal_total(&mut transaction).await?;
    let problem_view = query.problem_view.unwrap_or_default();
    let total = match problem_view {
        ProblemReadView::All => confirmed_total + deferred_total,
        ProblemReadView::Confirmed => confirmed_total,
        ProblemReadView::Deferred => deferred_total,
    };
    let items = read_problem_items(&mut transaction, problem_view, limit, offset).await?;
    let statistics = resolve_optional_result(&mut transaction, query.result_revision_ref).await?;
    transaction.commit().await?;
    Ok(json!({
        "view":"problems",
        "cumulative":summary,
        "facets":{"all":confirmed_total + deferred_total,"confirmed":confirmed_total,"deferred":deferred_total},
        "problemView":problem_view.as_db(),
        "statisticsResult":statistics.map(|context|context.envelope()),
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
             'researchHealth',jsonb_build_object( \
                 'researchSignalCount',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref), \
                 'problemBearingAtomCount',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref AND atom.kind IN ('problem','need')), \
                 'organizedProblemAtomCount',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref AND atom.kind IN ('problem','need') AND EXISTS( \
                     SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                     WHERE membership.atom_ref=atom.atom_ref AND membership.current)), \
                 'backlogProblemAtomCount',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref AND atom.kind IN ('problem','need') AND NOT EXISTS( \
                     SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                     WHERE membership.atom_ref=atom.atom_ref AND membership.current)), \
                 'organizationCoverage',jsonb_build_object( \
                     'numerator',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref AND atom.kind IN ('problem','need') AND EXISTS( \
                         SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                         WHERE membership.atom_ref=atom.atom_ref AND membership.current)), \
                     'denominator',(SELECT count(*) FROM linggan_comment_research_atom atom WHERE atom.run_ref=run.run_ref AND atom.kind IN ('problem','need')) \
                 ), \
                 'activatedBacklogAtomCount',(SELECT count(*) FROM linggan_comment_research_problem_resolution_execution execution \
                    JOIN linggan_comment_research_atom atom ON atom.atom_ref=execution.atom_ref \
                    WHERE execution.run_ref=run.run_ref AND atom.run_ref<>run.run_ref), \
                 'activatedBacklogResolvedAtomCount',(SELECT count(*) FROM linggan_comment_research_problem_resolution_execution execution \
                    JOIN linggan_comment_research_atom atom ON atom.atom_ref=execution.atom_ref \
                    JOIN linggan_comment_research_atom_problem_membership membership ON membership.atom_ref=atom.atom_ref AND membership.current \
                    WHERE execution.run_ref=run.run_ref AND atom.run_ref<>run.run_ref), \
                 'activatedBacklogPendingAtomCount',(SELECT count(*) FROM linggan_comment_research_problem_resolution_execution execution \
                    JOIN linggan_comment_research_atom atom ON atom.atom_ref=execution.atom_ref \
                    WHERE execution.run_ref=run.run_ref AND atom.run_ref<>run.run_ref \
                      AND execution.state IN ('pending','running','retryable')), \
                 'activatedBacklogFailedAtomCount',(SELECT count(*) FROM linggan_comment_research_problem_resolution_execution execution \
                    JOIN linggan_comment_research_atom atom ON atom.atom_ref=execution.atom_ref \
                    WHERE execution.run_ref=run.run_ref AND atom.run_ref<>run.run_ref \
                      AND execution.state IN ('model_failed','incompatible')) \
             ), \
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

async fn read_deferred_signal_total(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
) -> Result<i64, CommentResearchV1ReadError> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) \
         FROM linggan_comment_research_problem_resolution resolution \
         JOIN linggan_comment_research_atom atom USING(atom_ref) \
         JOIN linggan_comment_research_derivation_current derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         WHERE resolution.state='succeeded' \
           AND resolution.decision_kind IN ('deferred_novel','deferred_ambiguous','deferred_context') \
           AND derivation.derivation_version=$1 \
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                          WHERE membership.atom_ref=atom.atom_ref AND membership.current)",
    )
    .bind(DERIVATION_VERSION)
    .fetch_one(&mut **transaction)
    .await?)
}

/// One ordered stream for the existing Problems surface. Confirmed Problems and deferred
/// signals have different semantics, but `all` remains page-able instead of silently showing a
/// convenient subset. A deferred row never contributes to `cumulative` above.
async fn read_problem_items(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    view: ProblemReadView,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, CommentResearchV1ReadError> {
    let items: Vec<Value> = sqlx::query_scalar(
        "WITH accepted AS ( \
             SELECT membership.problem_ref,membership.definition_revision,membership.atom_ref, \
                    membership.created_at,source.material_ref,source.content_public_ref \
             FROM linggan_comment_research_atom_problem_membership membership \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_current derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
             WHERE membership.current AND derivation.derivation_version=$1 \
         ), grouped AS ( \
             SELECT problem_ref,definition_revision,count(DISTINCT atom_ref) AS atom_count, \
                    count(DISTINCT material_ref) AS comment_count,count(DISTINCT content_public_ref) AS work_count, \
                    min(created_at) AS first_confirmed_at,max(created_at) AS last_confirmed_at \
             FROM accepted GROUP BY problem_ref,definition_revision \
         ), confirmed AS ( \
             SELECT jsonb_build_object( \
                 'itemKind','confirmed', \
                 'problemRef',definition.problem_ref,'definitionRevision',definition.revision, \
                 'name',definition.name,'meaning',definition.meaning, \
                 'confirmedAtomCount',grouped.atom_count,'confirmedCommentCount',grouped.comment_count, \
                 'confirmedWorkCount',grouped.work_count,'firstConfirmedAt',grouped.first_confirmed_at, \
                 'lastConfirmedAt',grouped.last_confirmed_at \
             ) AS item,grouped.last_confirmed_at AS sort_at,0 AS kind_order,definition.name AS tie_breaker \
             FROM grouped \
             JOIN linggan_comment_research_problem_definition definition \
               ON definition.problem_ref=grouped.problem_ref AND definition.revision=grouped.definition_revision \
         ), deferred AS ( \
             SELECT jsonb_build_object( \
                 'itemKind','deferred', \
                 'proposition',atom.proposition, \
                 'problemFrame',atom.problem_frame, \
                 'decisionKind',resolution.decision_kind, \
                 'decision',resolution.decision_payload, \
                 'recheckConditions',resolution.recheck_conditions, \
                 'updatedAt',resolution.updated_at, \
                 'candidateSnapshot',COALESCE(( \
                     SELECT jsonb_agg(jsonb_build_object( \
                         'candidateIndex',candidate->'candidateIndex', \
                         'definition',candidate->'definition', \
                         'retrievalCosine',candidate->'cosine' \
                     ) ORDER BY (candidate->>'candidateIndex')::integer) \
                     FROM jsonb_array_elements(resolution.candidate_set) candidate \
                 ),'[]'::jsonb), \
                 'executionHistory',COALESCE(( \
                     SELECT jsonb_agg(jsonb_build_object( \
                         'state',history.state, \
                         'attempts',history.attempts, \
                         'failureCode',history.failure_code, \
                         'lastAttemptAt',history.last_attempt_at, \
                         'finishedAt',history.finished_at \
                     ) ORDER BY history.updated_at DESC,history.run_ref DESC) \
                     FROM linggan_comment_research_problem_resolution_execution history \
                     WHERE history.atom_ref=atom.atom_ref \
                 ),'[]'::jsonb) \
             ) AS item,resolution.updated_at AS sort_at,1 AS kind_order,atom.atom_ref::text AS tie_breaker \
             FROM linggan_comment_research_problem_resolution resolution \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_current derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
             WHERE resolution.state='succeeded' \
               AND resolution.decision_kind IN ('deferred_novel','deferred_ambiguous','deferred_context') \
               AND derivation.derivation_version=$1 \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                              WHERE membership.atom_ref=atom.atom_ref AND membership.current) \
         ), visible AS ( \
             SELECT * FROM confirmed WHERE $2 IN ('all','confirmed') \
             UNION ALL \
             SELECT * FROM deferred WHERE $2 IN ('all','deferred') \
         ) SELECT item FROM visible \
           ORDER BY sort_at DESC,kind_order,tie_breaker \
           LIMIT $3 OFFSET $4",
    )
    .bind(DERIVATION_VERSION)
    .bind(view.as_db())
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(items)
}

async fn read_cumulative_problem_state(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    limit: i64,
    offset: i64,
) -> Result<(Value, Vec<Value>), CommentResearchV1ReadError> {
    let summary: Value = sqlx::query_scalar(
        "WITH accepted AS ( \
             SELECT membership.problem_ref,membership.definition_revision,membership.atom_ref, \
                    membership.created_at,membership.membership_ref,source.material_ref,source.content_public_ref \
             FROM linggan_comment_research_atom_problem_membership membership \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_current derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
             WHERE membership.current AND derivation.derivation_version=$1 \
         ) \
         SELECT jsonb_build_object( \
             'kind','current_confirmed_membership', \
             'problemCount',count(DISTINCT (problem_ref,definition_revision)), \
             'confirmedAtomCount',count(DISTINCT atom_ref), \
             'confirmedCommentCount',count(DISTINCT material_ref), \
             'confirmedWorkCount',count(DISTINCT content_public_ref), \
             'lastConfirmedAt',max(created_at) \
         ) FROM accepted",
    )
    .bind(DERIVATION_VERSION)
    .fetch_one(&mut **transaction)
    .await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "WITH accepted AS ( \
             SELECT membership.problem_ref,membership.definition_revision,membership.atom_ref, \
                    membership.created_at,source.material_ref,source.content_public_ref \
             FROM linggan_comment_research_atom_problem_membership membership \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_current derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
             WHERE membership.current AND derivation.derivation_version=$1 \
         ), grouped AS ( \
             SELECT problem_ref,definition_revision,count(DISTINCT atom_ref) AS atom_count, \
                    count(DISTINCT material_ref) AS comment_count,count(DISTINCT content_public_ref) AS work_count, \
                    min(created_at) AS first_confirmed_at,max(created_at) AS last_confirmed_at \
             FROM accepted GROUP BY problem_ref,definition_revision \
         ) SELECT jsonb_build_object( \
             'problemRef',definition.problem_ref,'definitionRevision',definition.revision, \
             'name',definition.name,'meaning',definition.meaning, \
             'confirmedAtomCount',grouped.atom_count,'confirmedCommentCount',grouped.comment_count, \
             'confirmedWorkCount',grouped.work_count,'firstConfirmedAt',grouped.first_confirmed_at, \
             'lastConfirmedAt',grouped.last_confirmed_at \
         ) FROM grouped \
         JOIN linggan_comment_research_problem_definition definition \
           ON definition.problem_ref=grouped.problem_ref AND definition.revision=grouped.definition_revision \
         ORDER BY grouped.last_confirmed_at DESC,definition.name,definition.problem_ref \
         LIMIT $2 OFFSET $3",
    )
    .bind(DERIVATION_VERSION)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut **transaction)
    .await?;
    Ok((summary, items))
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

async fn resolve_optional_result(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    requested: Option<Uuid>,
) -> Result<Option<ResultContext>, CommentResearchV1ReadError> {
    match resolve_result(transaction, requested).await {
        Ok(context) => Ok(Some(context)),
        Err(CommentResearchV1ReadError::ResultUnavailable) if requested.is_none() => Ok(None),
        Err(error) => Err(error),
    }
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
