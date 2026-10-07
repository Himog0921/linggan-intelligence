//! Read-only projections for the clean comment-study lifecycle.
//!
//! An empty clean layer is an honest empty state, not a reason to relabel historical derived
//! results as a new StudyRun.

use crate::comment_study_catalog::cursor::{self, CommentPosition, EntryPosition};
use crate::comment_study_catalog::{
    CatalogStudyState, CatalogSummaryQuery, CatalogVoiceRole, CommentDetailQuery,
    StudyCatalogError, read_catalog_summary, read_comment_detail,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 100;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentStudyReadQuery {
    pub domain: Option<Uuid>,
    pub run_ref: Option<Uuid>,
    pub kind: Option<String>,
    pub work_ref: Option<Uuid>,
    pub comment_external_id: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub q: Option<String>,
    pub state: Option<String>,
    pub problem_ref: Option<Uuid>,
}

impl CommentStudyReadQuery {
    fn limit(&self) -> Result<i64, CommentStudyReadError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(CommentStudyReadError::InvalidQuery);
        }
        Ok(limit)
    }
}

#[derive(Debug, Error)]
pub enum CommentStudyReadError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the requested query is invalid")]
    InvalidQuery,
    #[error("the requested cursor is invalid")]
    InvalidCursor,
    #[error("the requested cursor belongs to another query")]
    CursorScopeMismatch,
    #[error("the clean comment-study schema is unavailable")]
    SchemaUnavailable,
    #[error("the requested run is absent or outside the selected domain")]
    RunUnavailable,
    #[error("the requested problem is absent or outside the selected domain")]
    ProblemUnavailable,
    #[error("the requested model request is absent or outside the selected domain")]
    RequestUnavailable,
}

struct ReadPage {
    limit: i64,
    as_of: String,
    after: Option<EntryPosition>,
    scope_hash: String,
}

async fn read_page(
    database: &Database,
    query: &CommentStudyReadQuery,
    resource: &'static str,
    scope: Value,
    order: &'static str,
) -> Result<ReadPage, CommentStudyReadError> {
    let scope_hash = cursor::scope_hash(&json!({
        "scope":scope,
        "resource":resource,
        "order":order
    }))
    .map_err(map_cursor_error)?;
    let (as_of, after) = if let Some(value) = query.cursor.as_deref() {
        let decoded = cursor::decode_for::<EntryPosition>(resource, value, &scope_hash)
            .map_err(map_cursor_error)?;
        (decoded.as_of, Some(decoded.last))
    } else {
        let as_of = sqlx::query_scalar::<_, String>(
            "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC', \
                    'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
        )
        .fetch_one(database.pool())
        .await?;
        (as_of, None)
    };
    Ok(ReadPage {
        limit: query.limit()?,
        as_of,
        after,
        scope_hash,
    })
}

fn map_cursor_error(error: StudyCatalogError) -> CommentStudyReadError {
    match error {
        StudyCatalogError::InvalidCursor => CommentStudyReadError::InvalidCursor,
        StudyCatalogError::CursorScopeMismatch => CommentStudyReadError::CursorScopeMismatch,
        _ => CommentStudyReadError::InvalidQuery,
    }
}

fn encode_next_cursor(
    page: &ReadPage,
    resource: &'static str,
    created_at: String,
    reference: Uuid,
) -> Result<String, CommentStudyReadError> {
    cursor::encode_for(
        resource,
        &page.scope_hash,
        &page.as_of,
        EntryPosition {
            created_at,
            reference,
        },
    )
    .map_err(map_cursor_error)
}

pub async fn schema_ready(database: &Database) -> Result<bool, CommentStudyReadError> {
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_active_policy') IS NOT NULL \
                AND to_regclass('linggan_comment_study_run') IS NOT NULL \
                AND to_regclass('linggan_comment_study_target') IS NOT NULL \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                  WHERE table_schema=current_schema() AND table_name='linggan_comment_study_work' \
                    AND column_name='observation_role') \
                AND to_regclass('linggan_comment_study_signal') IS NOT NULL \
                AND to_regclass('linggan_comment_study_problem') IS NOT NULL \
                AND to_regclass('linggan_comment_study_resolution') IS NOT NULL \
                AND to_regclass('linggan_comment_study_effective_target') IS NOT NULL \
                AND to_regclass('linggan_comment_study_effective_signal') IS NOT NULL \
                AND to_regclass('linggan_comment_study_current_problem') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    Ok(ready)
}

/// Shows configuration, a truthful 56-day observation series, and the latest Run state without
/// inferring study success from source material counts. Each count remains a distinct lifecycle
/// fact; the browser only slices the server projection into 7 / 28 / 56 day views.
pub async fn read_overview(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let domain = domain_ref.ok_or(CommentStudyReadError::InvalidQuery)?;
    let policy: Option<Value> = match domain_ref {
        Some(domain_ref) => {
            sqlx::query_scalar(
                "SELECT jsonb_build_object( \
                 'policyRef',policy.policy_ref,'domainRef',policy.domain_ref, \
                 'contract',policy.contract,'commentBudget',policy.comment_budget, \
                 'contextCharacterBudget',policy.context_character_budget, \
                 'modelConfigured',policy.model_config_ref IS NOT NULL \
             ) \
             FROM linggan_comment_study_active_policy active \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             WHERE active.domain_ref=$1 AND policy.domain_ref=$1",
            )
            .bind(domain_ref)
            .fetch_optional(database.pool())
            .await?
        }
        None => None,
    };
    let latest_run: Option<Value> = match domain_ref {
        Some(domain_ref) => sqlx::query_scalar(
            "SELECT jsonb_build_object( \
                 'runRef',run.run_ref,'asOf',run.as_of,'state',run.state, \
                 'createdAt',run.created_at,'finishedAt',run.finished_at, \
                 'selectedWorkCount',(SELECT count(*) FROM linggan_comment_study_work work WHERE work.run_ref=run.run_ref), \
                 'primaryWorkCount',(SELECT count(*) FROM linggan_comment_study_work work WHERE work.run_ref=run.run_ref AND work.observation_role='primary'), \
                 'referenceWorkCount',(SELECT count(*) FROM linggan_comment_study_work work WHERE work.run_ref=run.run_ref AND work.observation_role='reference'), \
                 'targetStates',COALESCE((SELECT jsonb_object_agg(grouped.state,grouped.count) \
                   FROM (SELECT target.state,count(*) AS count FROM linggan_comment_study_target target \
                         WHERE target.run_ref=run.run_ref GROUP BY target.state) grouped),'{}'::jsonb), \
                 'signalStates',COALESCE((SELECT jsonb_object_agg(grouped.eligibility_state,grouped.count) \
                   FROM (SELECT signal.eligibility_state,count(*) AS count \
                         FROM linggan_comment_study_signal signal \
                         JOIN linggan_comment_study_target target USING(target_ref) \
                         WHERE target.run_ref=run.run_ref GROUP BY signal.eligibility_state) grouped),'{}'::jsonb), \
                 'resolutionStates',COALESCE((SELECT jsonb_object_agg(grouped.state,grouped.count) \
                   FROM (SELECT resolution.state,count(*) AS count \
                         FROM linggan_comment_study_resolution resolution \
                         JOIN linggan_comment_study_signal signal USING(signal_ref) \
                         JOIN linggan_comment_study_target target USING(target_ref) \
                         WHERE target.run_ref=run.run_ref GROUP BY resolution.state) grouped),'{}'::jsonb), \
                 'semanticSummary',COALESCE((WITH attempts AS ( \
                    SELECT attempt.target_ref,attempt.attempt_ordinal,attempt.state,attempt.rejection_code,attempt.model_invocation_ref \
                    FROM linggan_comment_study_semantic_attempt attempt \
                    JOIN linggan_comment_study_target target USING(target_ref) \
                    WHERE target.run_ref=run.run_ref \
                  ), final_attempt AS ( \
                    SELECT DISTINCT ON (target_ref) target_ref,state,rejection_code \
                    FROM attempts ORDER BY target_ref,attempt_ordinal DESC \
                  ) SELECT jsonb_build_object( \
                    'modelInvocationCount',(SELECT count(DISTINCT model_invocation_ref) FROM attempts), \
                    'semanticAttemptCount',(SELECT count(*) FROM attempts), \
                    'firstAttemptAcceptedTargetCount',(SELECT count(*) FROM attempts WHERE state='accepted' AND attempt_ordinal=1), \
                    'retryRecoveredTargetCount',(SELECT count(*) FROM (SELECT target_ref FROM attempts WHERE state='accepted' GROUP BY target_ref HAVING min(attempt_ordinal)>1) recovered), \
                    'finalSemanticContractFailureCount',(SELECT count(*) FROM final_attempt final JOIN linggan_comment_study_target target USING(target_ref) WHERE target.state='failed' AND final.rejection_code IN ('semantic_json_schema','semantic_contract','unsupported_problem_frame','semantic_batch_contract','semantic_target_missing')), \
                    'finalEvidenceFailureCount',(SELECT count(*) FROM final_attempt final JOIN linggan_comment_study_target target USING(target_ref) WHERE target.state='failed' AND final.rejection_code IN ('evidence_not_contiguous','evidence_ambiguous')), \
                    'acceptedEvidenceSpanMismatchCount',(SELECT count(*) FROM linggan_comment_study_signal signal JOIN linggan_comment_study_target target USING(target_ref) JOIN linggan_material_comment source ON source.material_ref=target.source_ref WHERE target.run_ref=run.run_ref AND substring(source.body_text FROM signal.evidence_start+1 FOR signal.evidence_end-signal.evidence_start)<>signal.evidence) \
                  ) ),'{}'::jsonb), \
                 'problemMembershipCount',(SELECT count(*) FROM linggan_comment_study_problem_membership membership \
                    JOIN linggan_comment_study_signal signal USING(signal_ref) \
                    JOIN linggan_comment_study_target target USING(target_ref) WHERE target.run_ref=run.run_ref) \
             ) \
             FROM linggan_comment_study_run run \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             WHERE policy.domain_ref=$1 ORDER BY run.created_at DESC,run.run_ref DESC LIMIT 1",
        )
        .bind(domain_ref)
        .fetch_optional(database.pool())
        .await?,
        None => None,
    };
    let observation_series = match domain_ref {
        Some(domain_ref) => Some(
            crate::comment_study_observation::read_comment_observation_series(database, domain_ref)
                .await?,
        ),
        None => None,
    };
    let as_of: String = sqlx::query_scalar(
        "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC', \
         'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(database.pool())
    .await?;
    let corpus = if let Some(domain) = domain_ref {
        read_catalog_summary(
            database,
            &CatalogSummaryQuery {
                domain,
                q: None,
                work_ref: None,
                voice_role: CatalogVoiceRole::ReaderAndUnknown,
                study_state: CatalogStudyState::All,
            },
        )
        .await
        .ok()
    } else {
        None
    };
    let corpus_summary = corpus.as_ref().map(|value| value["summary"].clone());
    let index_coverage = corpus.as_ref().map(|value| value["indexCoverage"].clone());
    let research_summary: Value = sqlx::query_scalar(
        "WITH studied AS ( \
           SELECT head.state,head.content_public_ref,head.comment_external_id \
           FROM linggan_comment_study_effective_target head \
           JOIN linggan_comment_study_target target ON target.target_ref=head.target_ref \
           JOIN linggan_material_comment studied_source ON studied_source.material_ref=head.source_ref \
           JOIN linggan_comment_study_current_comment current_comment \
             ON current_comment.content_public_ref=head.content_public_ref \
            AND current_comment.comment_external_id=head.comment_external_id \
           JOIN linggan_material_content_author work_author \
             ON work_author.content_public_ref=head.content_public_ref \
           WHERE head.domain_ref=$1 \
             AND current_comment.body_state='KNOWN' AND current_comment.body_text IS NOT NULL \
             AND studied_source.body_state='KNOWN' AND studied_source.body_text IS NOT NULL \
             AND current_comment.body_text=studied_source.body_text \
             AND NULLIF(btrim(current_comment.author_external_id),'') IS NOT NULL \
             AND NULLIF(btrim(work_author.author_external_id),'') IS NOT NULL \
             AND btrim(current_comment.author_external_id)<>btrim(work_author.author_external_id) \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
               WHERE restriction.content_public_ref=head.content_public_ref \
                 AND restriction.comment_external_id=head.comment_external_id) \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment parent \
               JOIN linggan_material_comment_restriction restriction \
                 ON restriction.content_public_ref=parent.content_public_ref \
                AND restriction.comment_external_id=parent.comment_external_id \
               WHERE parent.material_ref=target.parent_source_ref) \
         ), current_signal AS ( \
           SELECT signal.signal_ref FROM linggan_comment_study_effective_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           WHERE policy.domain_ref=$1 \
         ) \
         SELECT jsonb_build_object( \
           'studiedCommentCount',(SELECT count(*) FROM studied), \
           'succeededCommentCount',(SELECT count(*) FROM studied WHERE state='succeeded'), \
           'noSignalCommentCount',(SELECT count(*) FROM studied WHERE state='no_signal'), \
           'currentSignalCount',(SELECT count(*) FROM current_signal))",
    )
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    let mut knowledge_summary: Value = sqlx::query_scalar(
        "WITH current_signal AS ( \
           SELECT signal.* FROM linggan_comment_study_effective_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           WHERE policy.domain_ref=$1 \
         ), current_resolution AS ( \
           SELECT resolution.state FROM linggan_comment_study_resolution resolution \
           JOIN current_signal signal USING(signal_ref) \
           WHERE resolution.domain_ref=$1 AND signal.eligibility_state='eligible' \
         ), current_pair AS ( \
           SELECT pair.state FROM linggan_comment_study_problem_pair pair \
           JOIN current_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
             AND first_signal.eligibility_state='eligible' \
           JOIN current_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
             AND second_signal.eligibility_state='eligible' \
         ) \
         SELECT jsonb_build_object( \
           'problemCount',(SELECT count(*) FROM linggan_comment_study_current_problem WHERE domain_ref=$1), \
           'activeProblemCount',(SELECT count(*) FROM linggan_comment_study_current_problem \
             WHERE domain_ref=$1 AND state='active'), \
           'supportInsufficientProblemCount',(SELECT count(*) FROM linggan_comment_study_current_problem \
             WHERE domain_ref=$1 AND state='support_insufficient'), \
           'assignedSignalCount',(SELECT count(*) FROM current_resolution WHERE state='assigned'), \
           'pendingResolutionCount',(SELECT count(*) FROM current_resolution WHERE state='pending'), \
           'retrievalIncompleteCount',(SELECT count(*) FROM current_resolution WHERE state='retrieval_incomplete'), \
           'deferredNovelCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_novel'), \
           'deferredAmbiguousCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_ambiguous'), \
           'deferredContextCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_context'), \
           'budgetStoppedCount',(SELECT count(*) FROM current_resolution WHERE state='budget_stopped'), \
           'protocolRejectedCount',(SELECT count(*) FROM current_resolution WHERE state='protocol_rejected'), \
           'pendingPairCount',(SELECT count(*) FROM current_pair WHERE state='pending'), \
           'solutionSignalCount',(SELECT count(*) FROM current_signal WHERE kind='solution'), \
           'experienceSignalCount',(SELECT count(*) FROM current_signal WHERE kind='experience'), \
           'solutionWorkCount',(SELECT count(DISTINCT content_public_ref) FROM current_signal WHERE kind='solution'), \
           'experienceWorkCount',(SELECT count(DISTINCT content_public_ref) FROM current_signal WHERE kind='experience'))",
    ).bind(domain_ref).fetch_one(database.pool()).await?;
    knowledge_summary["supportWindowDays"] = json!(28);
    knowledge_summary["problemSupportPreview"] =
        read_problem_support_preview(database, domain, &as_of).await?;
    knowledge_summary["voicePreview"] = read_signal_voice_preview(database, domain, None).await?;
    knowledge_summary["solutionPreview"] =
        read_signal_voice_preview(database, domain, Some("solution")).await?;
    knowledge_summary["experiencePreview"] =
        read_signal_voice_preview(database, domain, Some("experience")).await?;
    Ok(json!({
        "contract":"comment-study.read.v2",
        "domainRef":domain_ref,
        "asOf":as_of,
        "corpusSummary":corpus_summary,
        "corpusSummaryState":if corpus.is_some() { "known" } else { "unavailable" },
        "indexCoverage":index_coverage,
        "researchSummary":research_summary,
        "knowledgeSummary":knowledge_summary,
        "policy":policy,
        "latestRun":latest_run,
        "observationSeries":observation_series,
        "cleanLayerState": if policy.is_some() { "configured" } else { "not_configured" }
    }))
}

async fn read_signal_voice_preview(
    database: &Database,
    domain_ref: Uuid,
    kind: Option<&str>,
) -> Result<Value, CommentStudyReadError> {
    let rows = sqlx::query(
        "WITH voices AS ( \
           SELECT signal.signal_ref,signal.target_ref,signal.kind,signal.proposition,signal.evidence, \
                  signal.content_public_ref,signal.comment_external_id,signal.created_at, \
                  current_comment.body_text,current_comment.author_display_name, \
                  row_number() OVER (PARTITION BY signal.content_public_ref,signal.comment_external_id \
                    ORDER BY signal.created_at DESC,signal.signal_ref DESC) AS voice_rank \
           FROM linggan_comment_study_effective_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           JOIN linggan_comment_study_current_comment current_comment \
             ON current_comment.content_public_ref=signal.content_public_ref \
            AND current_comment.comment_external_id=signal.comment_external_id \
           WHERE policy.domain_ref=$1 AND ($2::text IS NULL OR signal.kind=$2) \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
               WHERE restriction.content_public_ref=signal.content_public_ref \
                 AND restriction.comment_external_id=signal.comment_external_id) \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment parent \
               JOIN linggan_material_comment_restriction restriction \
                 ON restriction.content_public_ref=parent.content_public_ref \
                AND restriction.comment_external_id=parent.comment_external_id \
               WHERE parent.material_ref=target.parent_source_ref) \
         ) \
         SELECT voice.*,detail.title AS work_title \
         FROM voices voice \
         LEFT JOIN LATERAL ( \
           SELECT detail.title FROM linggan_material_content_detail detail \
           JOIN linggan_runtime_capture_package package ON package.package_ref=detail.package_ref \
           WHERE detail.content_public_ref=voice.content_public_ref \
             AND detail.title_state='KNOWN' AND package.accepted_at IS NOT NULL \
           ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC,detail.material_ref DESC \
           LIMIT 1 \
         ) detail ON true \
         WHERE voice.voice_rank=1 \
         ORDER BY voice.created_at DESC,voice.signal_ref DESC LIMIT 3",
    ).bind(domain_ref).bind(kind).fetch_all(database.pool()).await?;
    Ok(json!(
        rows.into_iter()
            .map(|row| json!({
                "signalRef":row.get::<Uuid,_>("signal_ref"),
                "targetRef":row.get::<Uuid,_>("target_ref"),
                "kind":row.get::<String,_>("kind"),
                "proposition":row.get::<String,_>("proposition"),
                "evidence":row.get::<String,_>("evidence"),
                "commentText":row.get::<String,_>("body_text"),
                "authorDisplayName":row.get::<Option<String>,_>("author_display_name"),
                "commentKey":{"workRef":row.get::<Uuid,_>("content_public_ref"),
                              "commentExternalId":row.get::<String,_>("comment_external_id")},
                "workRef":row.get::<Uuid,_>("content_public_ref"),
                "workTitle":row.get::<Option<String>,_>("work_title"),
                "sourceState":"known"
            }))
            .collect::<Vec<_>>()
    ))
}

/// Domain-wide current Signal rows. The overview preview groups voices by comment, whereas this
/// page keeps every effective Signal so its pagination and total use the same unit.
pub async fn read_current_signals(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain)
        .await?
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    let kind = query
        .kind
        .as_deref()
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    if !matches!(kind, "solution" | "experience") {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    let page = read_page(
        database,
        query,
        "signals",
        json!({"domainRef":domain_ref,"kind":kind}),
        "created_at_desc.signal_ref_desc.v1",
    )
    .await?;
    let rows = sqlx::query(
        "WITH eligible AS MATERIALIZED ( \
           SELECT signal.signal_ref,signal.target_ref,target.run_ref,signal.kind, \
                  signal.proposition,signal.evidence,signal.content_public_ref, \
                  signal.comment_external_id,signal.created_at, \
                  current_comment.body_text,current_comment.author_display_name \
           FROM linggan_comment_study_effective_signal signal \
           JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           JOIN linggan_comment_study_current_comment current_comment \
             ON current_comment.content_public_ref=signal.content_public_ref \
            AND current_comment.comment_external_id=signal.comment_external_id \
           WHERE policy.domain_ref=$1 AND signal.domain_ref=$1 AND signal.kind=$2 \
             AND signal.created_at <= $3::text::timestamptz \
         ), total AS (SELECT count(*) AS total_count FROM eligible), page AS ( \
           SELECT * FROM eligible \
           WHERE $4::text IS NULL OR created_at < $4::text::timestamptz \
             OR (created_at=$4::text::timestamptz AND signal_ref<$5::uuid) \
           ORDER BY created_at DESC,signal_ref DESC LIMIT $6 \
         ) \
         SELECT page.signal_ref,page.target_ref,page.run_ref,page.kind,page.proposition, \
                page.evidence,page.content_public_ref,page.comment_external_id, \
                to_char(page.created_at AT TIME ZONE 'UTC', \
                  'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                page.body_text,page.author_display_name,total.total_count, \
                detail.title AS work_title \
         FROM total LEFT JOIN page ON true \
         LEFT JOIN LATERAL ( \
           SELECT detail.title FROM linggan_material_content_detail detail \
           JOIN linggan_runtime_capture_package package ON package.package_ref=detail.package_ref \
           WHERE detail.content_public_ref=page.content_public_ref \
             AND detail.title_state='KNOWN' AND package.accepted_at IS NOT NULL \
           ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC,detail.material_ref DESC \
           LIMIT 1 \
         ) detail ON true \
         ORDER BY page.created_at DESC,page.signal_ref DESC",
    )
    .bind(domain_ref)
    .bind(kind)
    .bind(&page.as_of)
    .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
    .bind(page.after.as_ref().map(|position| position.reference))
    .bind(page.limit + 1)
    .fetch_all(database.pool())
    .await?;
    let total_count: i64 = rows.first().map(|row| row.get("total_count")).unwrap_or(0);
    let mut items: Vec<_> = rows
        .into_iter()
        .filter(|row| row.get::<Option<Uuid>, _>("signal_ref").is_some())
        .collect();
    let has_more = items.len() > page.limit as usize;
    items.truncate(page.limit as usize);
    let next_cursor = if has_more {
        items
            .last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "signals",
                    row.get("created_at"),
                    row.get("signal_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.current-signals.v1","domainRef":domain_ref,"kind":kind,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,
            "asOf":page.as_of,"totalCount":total_count},
        "signals":items.into_iter().map(|row| json!({
            "signalRef":row.get::<Uuid,_>("signal_ref"),"targetRef":row.get::<Uuid,_>("target_ref"),
            "runRef":row.get::<Uuid,_>("run_ref"),"kind":row.get::<String,_>("kind"),
            "proposition":row.get::<String,_>("proposition"),"evidence":row.get::<String,_>("evidence"),
            "commentText":row.get::<String,_>("body_text"),
            "authorDisplayName":row.get::<Option<String>,_>("author_display_name"),
            "commentKey":{"workRef":row.get::<Uuid,_>("content_public_ref"),
                "commentExternalId":row.get::<String,_>("comment_external_id")},
            "workRef":row.get::<Uuid,_>("content_public_ref"),
            "workTitle":row.get::<Option<String>,_>("work_title"),
            "sourceState":"known","createdAt":row.get::<String,_>("created_at")
        })).collect::<Vec<_>>()
    }))
}

/// Current related results for one stable comment. This keeps the catalog's domain and source
/// decision, then reads only the effective head; historical Run rows remain under /signals.
pub async fn read_comment_related(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain)
        .await?
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    let work_ref = query.work_ref.ok_or(CommentStudyReadError::InvalidQuery)?;
    let comment_external_id = query
        .comment_external_id
        .as_deref()
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    if work_ref.is_nil()
        || comment_external_id.is_empty()
        || comment_external_id.len() > 512
        || comment_external_id.contains('\0')
    {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    let detail = read_comment_detail(
        database,
        &CommentDetailQuery {
            domain: domain_ref,
            work_ref,
            comment_external_id: comment_external_id.to_owned(),
        },
    )
    .await
    .map_err(|error| match error {
        StudyCatalogError::Database(database_error) => {
            CommentStudyReadError::Database(database_error)
        }
        _ => CommentStudyReadError::InvalidQuery,
    })?;
    let source_state = detail["source"]["sourceState"]
        .as_str()
        .unwrap_or("unavailable");
    let page = read_page(
        database,
        query,
        "signals",
        json!({"domainRef":domain_ref,"workRef":work_ref,"commentExternalId":comment_external_id}),
        "created_at_desc.signal_ref_desc.v1",
    )
    .await?;
    if source_state != "known" {
        return Ok(json!({
            "contract":"comment-study.comment-related.v1","domainRef":domain_ref,
            "commentKey":{"workRef":work_ref,"commentExternalId":comment_external_id},
            "sourceState":source_state,
            "page":{"limit":page.limit,"hasMore":false,"nextCursor":null,"asOf":page.as_of},
            "signals":[]
        }));
    }
    let mut rows = sqlx::query(
        "SELECT signal.signal_ref,signal.target_ref,target.run_ref,signal.kind, \
                signal.proposition,signal.evidence, \
                resolution.state AS resolution_state,resolution.resolved_problem_ref, \
                to_jsonb(membership)->>'problem_revision_ref' AS problem_revision_ref, \
                to_char(signal.created_at AT TIME ZONE 'UTC', \
                  'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at \
         FROM linggan_comment_study_effective_signal signal \
         JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
         LEFT JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         LEFT JOIN linggan_comment_study_problem_membership membership USING(signal_ref) \
         WHERE signal.domain_ref=$1 AND signal.content_public_ref=$2 \
           AND signal.comment_external_id=$3 AND signal.created_at<=$4::text::timestamptz \
           AND ($5::text IS NULL OR signal.created_at<$5::text::timestamptz \
             OR (signal.created_at=$5::text::timestamptz AND signal.signal_ref<$6::uuid)) \
         ORDER BY signal.created_at DESC,signal.signal_ref DESC LIMIT $7",
    )
    .bind(domain_ref)
    .bind(work_ref)
    .bind(comment_external_id)
    .bind(&page.as_of)
    .bind(
        page.after
            .as_ref()
            .map(|position| position.created_at.as_str()),
    )
    .bind(page.after.as_ref().map(|position| position.reference))
    .bind(page.limit + 1)
    .fetch_all(database.pool())
    .await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "signals",
                    row.get("created_at"),
                    row.get("signal_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.comment-related.v1","domainRef":domain_ref,
        "commentKey":{"workRef":work_ref,"commentExternalId":comment_external_id},
        "sourceState":"known",
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "signals":rows.into_iter().map(|row| json!({
            "signalRef":row.get::<Uuid,_>("signal_ref"),
            "targetRef":row.get::<Uuid,_>("target_ref"),"runRef":row.get::<Uuid,_>("run_ref"),
            "kind":row.get::<String,_>("kind"),"proposition":row.get::<String,_>("proposition"),
            "evidence":row.get::<String,_>("evidence"),
            "resolutionState":row.get::<Option<String>,_>("resolution_state"),
            "resolvedProblemRef":row.get::<Option<Uuid>,_>("resolved_problem_ref"),
            "problemRevisionRef":row.get::<Option<String>,_>("problem_revision_ref"),
            "createdAt":row.get::<String,_>("created_at")
        })).collect::<Vec<_>>()
    }))
}

async fn read_problem_support_preview(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
) -> Result<Value, CommentStudyReadError> {
    let rows = sqlx::query(
        "WITH first_membership AS ( \
           SELECT membership.problem_ref,source.content_public_ref,source.comment_external_id, \
                  min(membership.created_at) AS first_added_at \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
           GROUP BY membership.problem_ref,source.content_public_ref,source.comment_external_id \
         ), current_membership AS ( \
           SELECT DISTINCT membership.problem_ref,signal.content_public_ref,signal.comment_external_id \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
           WHERE signal.eligibility_state='eligible' \
         ), current_voices AS ( \
           SELECT member.problem_ref,member.content_public_ref,member.comment_external_id, \
                  first.first_added_at,current_comment.body_text,current_comment.author_display_name \
           FROM current_membership member \
           JOIN first_membership first USING(problem_ref,content_public_ref,comment_external_id) \
           JOIN linggan_comment_study_current_comment current_comment \
             ON current_comment.content_public_ref=member.content_public_ref \
            AND current_comment.comment_external_id=member.comment_external_id \
           WHERE NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
             WHERE restriction.content_public_ref=member.content_public_ref \
               AND restriction.comment_external_id=member.comment_external_id) \
         ) \
         SELECT problem.problem_ref,revision.title, \
                count(*) AS support_comment_count, \
                count(*) FILTER (WHERE voice.first_added_at >= $2::text::timestamptz - interval '28 days') \
                  AS added_support_comment_count, \
                max(voice.first_added_at) AS latest_added_at, \
                (array_agg(jsonb_build_object( \
                  'commentKey',jsonb_build_object('workRef',voice.content_public_ref, \
                    'commentExternalId',voice.comment_external_id), \
                  'workRef',voice.content_public_ref,'commentText',voice.body_text, \
                  'authorDisplayName',voice.author_display_name,'sourceState','known') \
                  ORDER BY voice.first_added_at DESC,voice.content_public_ref,voice.comment_external_id))[1] \
                  AS voice \
         FROM linggan_comment_study_current_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         JOIN current_voices voice ON voice.problem_ref=problem.problem_ref \
         WHERE problem.domain_ref=$1 \
         GROUP BY problem.problem_ref,revision.title \
         HAVING count(*) FILTER (WHERE voice.first_added_at >= $2::text::timestamptz - interval '28 days')>0 \
         ORDER BY latest_added_at DESC,problem.problem_ref DESC LIMIT 3",
    ).bind(domain_ref).bind(as_of).fetch_all(database.pool()).await?;
    Ok(json!(
        rows.into_iter()
            .map(|row| json!({
                "problemRef":row.get::<Uuid,_>("problem_ref"),
                "title":row.get::<String,_>("title"),
                "supportCommentCount":row.get::<i64,_>("support_comment_count"),
                "addedSupportCommentCount":row.get::<i64,_>("added_support_comment_count"),
                "voice":row.get::<Value,_>("voice")
            }))
            .collect::<Vec<_>>()
    ))
}

pub async fn read_runs(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let page = read_page(
        database,
        query,
        "runs",
        json!({"domainRef":domain_ref}),
        "created_at_desc.run_ref_desc.v1",
    )
    .await?;
    // Aggregate each child relation independently after bounding the run page. Joining both
    // children on run_ref would count every target once for each selected work.
    let mut rows = sqlx::query(include_str!("comment_study_read/runs.sql"))
        .bind(domain_ref)
        .bind(&page.as_of)
        .bind(
            page.after
                .as_ref()
                .map(|position| position.created_at.as_str()),
        )
        .bind(page.after.as_ref().map(|position| position.reference))
        .bind(page.limit + 1)
        .fetch_all(database.pool())
        .await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "runs",
                    row.get("cursor_created_at"),
                    row.get("run_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    let run_refs: Vec<Uuid> = rows.iter().map(|row| row.get("run_ref")).collect();
    let extra_rows = sqlx::query(
        "SELECT run.run_ref,to_jsonb(policy)->>'method_name' AS method_name, \
           (SELECT count(*) FROM linggan_comment_study_resolution resolution \
            JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
            JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
            WHERE target.run_ref=run.run_ref AND signal.eligibility_state='eligible' \
              AND resolution.state='pending') AS pending_resolution_count, \
           (SELECT count(*) FROM linggan_comment_study_problem_pair pair \
            JOIN linggan_comment_study_effective_signal first_signal \
              ON first_signal.signal_ref=pair.first_signal_ref \
             AND first_signal.eligibility_state='eligible' \
            JOIN linggan_comment_study_effective_signal second_signal \
              ON second_signal.signal_ref=pair.second_signal_ref \
             AND second_signal.eligibility_state='eligible' \
            JOIN linggan_comment_study_target first_target \
              ON first_target.target_ref=first_signal.target_ref \
            JOIN linggan_comment_study_target second_target \
              ON second_target.target_ref=second_signal.target_ref \
            WHERE first_target.run_ref=run.run_ref AND second_target.run_ref=run.run_ref \
              AND pair.state='pending') AS pending_pair_count \
         FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE run.run_ref=ANY($1::uuid[])",
    )
    .bind(&run_refs)
    .fetch_all(database.pool())
    .await?;
    let extras: HashMap<Uuid, (Option<String>, i64, i64)> = extra_rows
        .into_iter()
        .map(|row| {
            (
                row.get("run_ref"),
                (
                    row.get("method_name"),
                    row.get("pending_resolution_count"),
                    row.get("pending_pair_count"),
                ),
            )
        })
        .collect();
    let has_start_request: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_start_request') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    let origins: HashMap<Uuid, String> = if has_start_request {
        sqlx::query(
            "SELECT run_ref,origin FROM linggan_comment_study_start_request \
             WHERE run_ref=ANY($1::uuid[])",
        )
        .bind(&run_refs)
        .fetch_all(database.pool())
        .await?
        .into_iter()
        .map(|row| (row.get("run_ref"), row.get("origin")))
        .collect()
    } else {
        HashMap::new()
    };
    Ok(json!({
        "contract":"comment-study.read.v1",
        "domainRef":domain_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "runs":rows.into_iter().map(|row| {
          let extra = extras.get(&row.get::<Uuid,_>("run_ref"));
          json!({
            "runRef":row.get::<Uuid,_>("run_ref"),"asOf":row.get::<String,_>("as_of"),
            "policyRef":row.get::<Uuid,_>("policy_ref"),
            "methodName":extra.and_then(|item| item.0.as_deref()),
            "origin":origins.get(&row.get::<Uuid,_>("run_ref")),
            "pendingResolutionCount":extra.map(|item| item.1),
            "pendingPairCount":extra.map(|item| item.2),
            "limits":{"commentBudget":row.get::<Option<i32>,_>("comment_budget"),
                "contextCharacterBudget":row.get::<Option<i32>,_>("context_character_budget"),
                "tokenLimit":row.get::<Option<i64>,_>("token_limit")},
            "state":row.get::<String,_>("state"),"createdAt":row.get::<String,_>("created_at"),
            "finishedAt":row.get::<Option<String>,_>("finished_at"),
            "selectionContract":row.get::<Option<String>,_>("selection_contract"),
            "recoverySourceRunRef":row.get::<Option<String>,_>("recovery_source_run_ref"),
            "dispatchState":row.get::<String,_>("dispatch_state"),
            "dispatchReason":row.get::<Option<String>,_>("dispatch_reason"),
            "controlVersion":row.get::<i64,_>("control_version"),
            "workCount":row.get::<i64,_>("work_count"),
            "primaryWorkCount":row.get::<i64,_>("primary_work_count"),
            "referenceWorkCount":row.get::<i64,_>("reference_work_count"),
            "targetCount":row.get::<i64,_>("target_count"),
            "pendingCount":row.get::<i64,_>("pending_count"),
            "succeededCount":row.get::<i64,_>("succeeded_count"),"noSignalCount":row.get::<i64,_>("no_signal_count"),
            "needsContextCount":row.get::<i64,_>("needs_context_count"),"failedCount":row.get::<i64,_>("failed_count"),
            "excludedCount":row.get::<i64,_>("excluded_count"),
            "cancelledCount":row.get::<i64,_>("cancelled_count")
        })}).collect::<Vec<_>>()
    }))
}

/// A Run is addressed directly so an older Run remains reachable after it leaves the list page.
pub async fn read_run_detail(
    database: &Database,
    query: &CommentStudyReadQuery,
    run_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let mut scoped = query.clone();
    scoped.run_ref = Some(run_ref);
    required_run(database, &scoped).await?;
    let mut detail: Value = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'runRef',run.run_ref,'domainRef',policy.domain_ref,'policyRef',run.policy_ref, \
           'asOf',run.as_of,'state',run.state,'createdAt',run.created_at,'finishedAt',run.finished_at, \
           'limits',jsonb_build_object('commentBudget',run.comment_budget, \
             'contextCharacterBudget',run.context_character_budget,'tokenLimit',run.token_limit), \
           'selectionContract',run.selection_manifest->>'contract', \
           'scope',run.selection_manifest, \
           'recoverySourceRunRef',run.selection_manifest->>'recoverySourceRunRef', \
           'dispatchState',run.dispatch_state,'dispatchReason',run.dispatch_reason, \
           'controlVersion',run.control_version, \
           'method',jsonb_build_object('name',policy.method_name,'manifest',policy.method_manifest, \
             'hash',policy.method_hash), \
           'workCount',(SELECT count(*) FROM linggan_comment_study_work work WHERE work.run_ref=run.run_ref), \
           'primaryWorkCount',(SELECT count(*) FROM linggan_comment_study_work work \
             WHERE work.run_ref=run.run_ref AND work.observation_role='primary'), \
           'referenceWorkCount',(SELECT count(*) FROM linggan_comment_study_work work \
             WHERE work.run_ref=run.run_ref AND work.observation_role='reference'), \
           'targetStates',COALESCE((SELECT jsonb_object_agg(state,count) FROM ( \
             SELECT state,count(*) AS count FROM linggan_comment_study_target \
             WHERE run_ref=run.run_ref GROUP BY state) grouped),'{}'::jsonb), \
           'signalStates',COALESCE((SELECT jsonb_object_agg(eligibility_state,count) FROM ( \
             SELECT signal.eligibility_state,count(*) AS count FROM linggan_comment_study_signal signal \
             JOIN linggan_comment_study_target target USING(target_ref) \
             WHERE target.run_ref=run.run_ref GROUP BY signal.eligibility_state) grouped),'{}'::jsonb) \
         ) FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE run.run_ref=$1 AND policy.domain_ref=$2",
    ).bind(run_ref).bind(query.domain).fetch_one(database.pool()).await?;
    detail["semanticSummary"] = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'targetCount',(SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1), \
           'succeededTargetCount',(SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='succeeded'), \
           'noSignalTargetCount',(SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='no_signal'), \
           'needsContextTargetCount',(SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='needs_context'), \
           'failedTargetCount',(SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='failed'), \
           'excludedTargetCount',(SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND state='excluded'), \
           'attemptCount',(SELECT count(*) FROM linggan_comment_study_semantic_attempt attempt \
             JOIN linggan_comment_study_target target USING(target_ref) WHERE target.run_ref=$1), \
           'signalCount',(SELECT count(*) FROM linggan_comment_study_signal signal \
             JOIN linggan_comment_study_target target USING(target_ref) WHERE target.run_ref=$1), \
           'currentSignalCount',(SELECT count(*) FROM linggan_comment_study_effective_signal signal \
             JOIN linggan_comment_study_target target USING(target_ref) WHERE target.run_ref=$1))",
    ).bind(run_ref).fetch_one(database.pool()).await?;
    detail["knowledgeSummary"] = sqlx::query_scalar(
        "WITH current_signal AS ( \
           SELECT signal.signal_ref FROM linggan_comment_study_effective_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           WHERE target.run_ref=$1 AND signal.eligibility_state='eligible' \
         ), current_resolution AS ( \
           SELECT resolution.state FROM linggan_comment_study_resolution resolution \
           JOIN current_signal signal USING(signal_ref) \
         ) SELECT jsonb_build_object( \
           'eligibleSignalCount',(SELECT count(*) FROM current_signal), \
           'assignedSignalCount',(SELECT count(*) FROM current_resolution WHERE state='assigned'), \
           'pendingResolutionCount',(SELECT count(*) FROM current_resolution WHERE state='pending'), \
           'deferredNovelCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_novel'), \
           'deferredAmbiguousCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_ambiguous'), \
           'deferredContextCount',(SELECT count(*) FROM current_resolution WHERE state='deferred_context'), \
           'retrievalIncompleteCount',(SELECT count(*) FROM current_resolution WHERE state='retrieval_incomplete'), \
           'budgetStoppedCount',(SELECT count(*) FROM current_resolution WHERE state='budget_stopped'), \
           'protocolRejectedCount',(SELECT count(*) FROM current_resolution WHERE state='protocol_rejected'), \
           'failedResolutionCount',(SELECT count(*) FROM current_resolution WHERE state='failed'), \
           'pendingPairCount',(SELECT count(*) FROM linggan_comment_study_problem_pair pair \
             JOIN current_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
             JOIN current_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
             WHERE pair.state='pending'), \
           'linkedProblemCount',(SELECT count(DISTINCT pair.created_problem_ref) \
             FROM linggan_comment_study_problem_pair pair \
             JOIN linggan_comment_study_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
             JOIN linggan_comment_study_target first_target ON first_target.target_ref=first_signal.target_ref \
             JOIN linggan_comment_study_signal second_signal ON second_signal.signal_ref=pair.second_signal_ref \
             JOIN linggan_comment_study_target second_target ON second_target.target_ref=second_signal.target_ref \
             WHERE (first_target.run_ref=$1 OR second_target.run_ref=$1) \
               AND pair.created_problem_ref IS NOT NULL))",
    ).bind(run_ref).fetch_one(database.pool()).await?;
    detail["costSummary"] = sqlx::query_scalar(
        "WITH ledger AS ( \
           SELECT request.invocation_ref,request.dispatch_started_at,invocation.input_tokens,invocation.output_tokens, \
                  invocation.charged_tokens \
           FROM linggan_comment_study_model_request request \
           JOIN linggan_model_invocation invocation USING(invocation_ref) \
           WHERE request.run_ref=$1 \
         ), run_contract AS ( \
           SELECT COALESCE(selection_manifest->>'contract'='comment-study.run-selection.v2',false) \
             AS ledger_complete \
           FROM linggan_comment_study_run WHERE run_ref=$1 \
         ) SELECT jsonb_build_object( \
           'recordingState',CASE WHEN run_contract.ledger_complete THEN 'recorded' \
             WHEN count(ledger.invocation_ref)=0 THEN 'unrecorded' ELSE 'partial' END, \
           'requestCount',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE count(ledger.invocation_ref) END, \
           'dispatchedRequestCount',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE count(ledger.invocation_ref) FILTER (WHERE dispatch_started_at IS NOT NULL) END, \
           'usageKnownRequestCount',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE count(ledger.invocation_ref) FILTER \
               (WHERE input_tokens IS NOT NULL AND output_tokens IS NOT NULL) END, \
           'usageUnknownRequestCount',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE count(ledger.invocation_ref) FILTER \
               (WHERE input_tokens IS NULL OR output_tokens IS NULL) END, \
           'knownInputTokens',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE COALESCE(sum(input_tokens),0) END, \
           'knownOutputTokens',CASE WHEN NOT run_contract.ledger_complete AND count(ledger.invocation_ref)=0 \
             THEN NULL ELSE COALESCE(sum(output_tokens),0) END, \
           'totalInputTokens',CASE WHEN run_contract.ledger_complete AND count(ledger.invocation_ref) FILTER \
             (WHERE input_tokens IS NULL OR output_tokens IS NULL)=0 \
             THEN COALESCE(sum(input_tokens),0) ELSE NULL END, \
           'totalOutputTokens',CASE WHEN run_contract.ledger_complete AND count(ledger.invocation_ref) FILTER \
             (WHERE input_tokens IS NULL OR output_tokens IS NULL)=0 \
             THEN COALESCE(sum(output_tokens),0) ELSE NULL END, \
           'chargedTokens',CASE WHEN run_contract.ledger_complete \
             THEN COALESCE(sum(charged_tokens),0) ELSE NULL END, \
           'billingAmount',NULL,'billingCurrency',NULL,'billingState','not_recorded') \
         FROM run_contract LEFT JOIN ledger ON true GROUP BY run_contract.ledger_complete",
    ).bind(run_ref).fetch_one(database.pool()).await?;
    Ok(json!({"contract":"comment-study.run-detail.v1","domainRef":query.domain,"run":detail}))
}

/// The request ledger exposes status and usage, not provider prompts or stored raw responses.
pub async fn read_run_requests(
    database: &Database,
    query: &CommentStudyReadQuery,
    run_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let mut scoped = query.clone();
    scoped.run_ref = Some(run_ref);
    required_run(database, &scoped).await?;
    let page = read_page(
        database,
        query,
        "run-requests",
        json!({"domainRef":query.domain,"runRef":run_ref}),
        "created_at_desc.invocation_ref_desc.v1",
    )
    .await?;
    let mut rows = sqlx::query(
        "SELECT request.invocation_ref,request.stage,request.batch_ref,request.resolution_ref, \
                request.pair_ref,request.attempt_ordinal,request.dispatch_started_at IS NOT NULL AS dispatched, \
                invocation.state,invocation.failure_code,invocation.input_tokens,invocation.output_tokens, \
                invocation.model_ref,invocation.config_ref,invocation.connection_version_ref, \
                model.model_id, \
                invocation.reserved_tokens,invocation.charged_tokens,invocation.elapsed_ms, \
                invocation.result->'diagnostic' AS diagnostic, \
                to_char(request.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                to_char(request.dispatch_started_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS dispatch_started_at, \
                to_char(request.deadline_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deadline_at, \
                to_char(invocation.finished_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS finished_at \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         LEFT JOIN linggan_model_entry model ON model.model_ref=invocation.model_ref \
         WHERE request.run_ref=$1 AND request.created_at<=$2::text::timestamptz \
           AND ($3::text IS NULL OR request.created_at<$3::text::timestamptz \
             OR (request.created_at=$3::text::timestamptz AND request.invocation_ref<$4::uuid)) \
         ORDER BY request.created_at DESC,request.invocation_ref DESC LIMIT $5",
    ).bind(run_ref).bind(&page.as_of)
     .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
     .bind(page.after.as_ref().map(|position| position.reference))
     .bind(page.limit+1).fetch_all(database.pool()).await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "run-requests",
                    row.get("created_at"),
                    row.get("invocation_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.run-requests.v1","domainRef":query.domain,"runRef":run_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "requests":rows.into_iter().map(|row| json!({
            "invocationRef":row.get::<Uuid,_>("invocation_ref"),
            "stage":row.get::<String,_>("stage"),
            "batchRef":row.get::<Option<Uuid>,_>("batch_ref"),
            "resolutionRef":row.get::<Option<Uuid>,_>("resolution_ref"),
            "pairRef":row.get::<Option<Uuid>,_>("pair_ref"),
            "attemptOrdinal":row.get::<i32,_>("attempt_ordinal"),
            "state":row.get::<String,_>("state"),
            "dispatched":row.get::<bool,_>("dispatched"),
            "createdAt":row.get::<String,_>("created_at"),
            "dispatchStartedAt":row.get::<Option<String>,_>("dispatch_started_at"),
            "deadlineAt":row.get::<String,_>("deadline_at"),
            "finishedAt":row.get::<Option<String>,_>("finished_at"),
            "failureCode":row.get::<Option<String>,_>("failure_code"),
            "modelIdentity":{"modelRef":row.get::<Option<Uuid>,_>("model_ref"),
                             "connectionVersionRef":row.get::<Uuid,_>("connection_version_ref"),
                             "modelId":row.get::<Option<String>,_>("model_id")},
            "modelConfigRef":row.get::<Option<Uuid>,_>("config_ref"),
            "usageKnown":row.get::<Option<i64>,_>("input_tokens").is_some()
                && row.get::<Option<i64>,_>("output_tokens").is_some(),
            "inputTokens":row.get::<Option<i64>,_>("input_tokens"),
            "outputTokens":row.get::<Option<i64>,_>("output_tokens"),
            "reservedTokens":row.get::<i64,_>("reserved_tokens"),
            "chargedTokens":row.get::<i64,_>("charged_tokens"),
            "elapsedMs":row.get::<Option<i64>,_>("elapsed_ms"),
            "diagnostic":row.get::<Option<Value>,_>("diagnostic")
        })).collect::<Vec<_>>()
    }))
}

/// The exact recorded provider request is available only while every referenced source is
/// readable. This includes the frozen parent and all candidate definition seeds.
pub async fn read_request_detail(
    database: &Database,
    query: &CommentStudyReadQuery,
    invocation_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain)
        .await?
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    let row = sqlx::query(
        "SELECT request.run_ref,request.stage,request.request_manifest, \
                request.dispatch_started_at IS NOT NULL AS dispatched, \
                to_char(request.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                to_char(request.dispatch_started_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS dispatch_started_at, \
                invocation.state,invocation.failure_code,invocation.input_tokens, \
                invocation.model_ref,invocation.config_ref,invocation.connection_version_ref, \
                model.model_id, \
                invocation.output_tokens,invocation.reserved_tokens,invocation.charged_tokens \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         LEFT JOIN linggan_model_entry model ON model.model_ref=invocation.model_ref \
         JOIN linggan_comment_study_run run ON run.run_ref=request.run_ref \
         JOIN linggan_comment_study_policy policy ON policy.policy_ref=request.policy_ref \
         WHERE request.invocation_ref=$1 AND policy.domain_ref=$2",
    ).bind(invocation_ref).bind(domain_ref).fetch_optional(database.pool()).await?
     .ok_or(CommentStudyReadError::RequestUnavailable)?;
    let request_manifest: Value = row.get("request_manifest");
    let stage: String = row.get("stage");
    // Candidate manifests on a Resolution can be refreshed. Read permission for an old
    // invocation must follow the immutable provider prompt, not that mutable current row.
    let frozen_candidate_revisions = if stage == "resolution" {
        crate::comment_study_request_ledger::frozen_candidate_revision_refs(&request_manifest)
    } else {
        Some(Vec::new())
    };
    let snapshot_complete = frozen_candidate_revisions.is_some();
    let frozen_candidate_revisions = frozen_candidate_revisions.unwrap_or_default();
    let gate = sqlx::query(
        "WITH request AS ( \
           SELECT * FROM linggan_comment_study_model_request WHERE invocation_ref=$1 \
         ), touched_target AS ( \
           SELECT target.target_ref,target.source_ref,target.parent_source_ref \
           FROM request JOIN linggan_comment_study_batch_target member \
             ON member.batch_ref=request.batch_ref \
           JOIN linggan_comment_study_target target USING(target_ref) \
           WHERE request.stage='semantic' \
           UNION \
           SELECT target.target_ref,target.source_ref,target.parent_source_ref \
           FROM request JOIN linggan_comment_study_resolution resolution \
             ON resolution.resolution_ref=request.resolution_ref \
           JOIN linggan_comment_study_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target USING(target_ref) \
           WHERE request.stage='resolution' \
           UNION \
           SELECT target.target_ref,target.source_ref,target.parent_source_ref \
           FROM request CROSS JOIN LATERAL unnest($2::uuid[]) candidate(revision_ref) \
           JOIN linggan_comment_study_problem_revision revision \
             ON revision.revision_ref=candidate.revision_ref \
           CROSS JOIN LATERAL unnest(revision.seed_signal_refs) seed(signal_ref) \
           JOIN linggan_comment_study_signal signal ON signal.signal_ref=seed.signal_ref \
           JOIN linggan_comment_study_target target USING(target_ref) \
           WHERE request.stage='resolution' \
           UNION \
           SELECT target.target_ref,target.source_ref,target.parent_source_ref \
           FROM request JOIN linggan_comment_study_problem_pair pair \
             ON pair.pair_ref=request.pair_ref \
           JOIN linggan_comment_study_signal signal \
             ON signal.signal_ref IN (pair.first_signal_ref,pair.second_signal_ref) \
           JOIN linggan_comment_study_target target USING(target_ref) \
           WHERE request.stage='pair' \
         ) \
         SELECT EXISTS(SELECT 1 FROM touched_target target \
             JOIN linggan_material_comment source \
               ON source.material_ref IN (target.source_ref,target.parent_source_ref) \
             JOIN linggan_material_comment_restriction restriction \
               ON restriction.content_public_ref=source.content_public_ref \
              AND restriction.comment_external_id=source.comment_external_id) AS restricted, \
           $3::boolean AND EXISTS(SELECT 1 FROM touched_target) \
             AND NOT EXISTS(SELECT 1 FROM unnest($2::uuid[]) candidate(revision_ref) \
               LEFT JOIN linggan_comment_study_problem_revision revision \
                 ON revision.revision_ref=candidate.revision_ref \
               WHERE revision.revision_ref IS NULL) \
             AND NOT EXISTS(SELECT 1 FROM touched_target target \
               JOIN linggan_material_comment source \
                 ON source.material_ref IN (target.source_ref,target.parent_source_ref) \
               WHERE source.body_state<>'KNOWN' OR source.body_text IS NULL) AS readable \
         FROM request",
    )
    .bind(invocation_ref)
    .bind(&frozen_candidate_revisions)
    .bind(snapshot_complete)
    .fetch_one(database.pool())
    .await?;
    let restricted: bool = gate.get("restricted");
    let readable: bool = gate.get("readable");
    let source_state = if restricted {
        "restricted"
    } else if !readable {
        "unavailable"
    } else {
        "known"
    };
    Ok(json!({
        "contract":"comment-study.request-detail.v1","domainRef":domain_ref,
        "request":{
            "invocationRef":invocation_ref,"runRef":row.get::<Uuid,_>("run_ref"),
            "stage":stage,"state":row.get::<String,_>("state"),
            "dispatched":row.get::<bool,_>("dispatched"),
            "createdAt":row.get::<String,_>("created_at"),
            "dispatchStartedAt":row.get::<Option<String>,_>("dispatch_started_at"),
            "failureCode":row.get::<Option<String>,_>("failure_code"),
            "modelIdentity":{"modelRef":row.get::<Option<Uuid>,_>("model_ref"),
                             "connectionVersionRef":row.get::<Uuid,_>("connection_version_ref"),
                             "modelId":row.get::<Option<String>,_>("model_id")},
            "modelConfigRef":row.get::<Option<Uuid>,_>("config_ref"),
            "usageKnown":row.get::<Option<i64>,_>("input_tokens").is_some()
                && row.get::<Option<i64>,_>("output_tokens").is_some(),
            "inputTokens":row.get::<Option<i64>,_>("input_tokens"),
            "outputTokens":row.get::<Option<i64>,_>("output_tokens"),
            "reservedTokens":row.get::<i64,_>("reserved_tokens"),
            "chargedTokens":row.get::<i64,_>("charged_tokens"),
            "recordingState":"recorded","sourceState":source_state,
            "requestManifest":if source_state=="known" {
                request_manifest
            } else { Value::Null }
        }
    }))
}

/// Current, readable expressions that have not become a durable Problem. This query is
/// independent of the Problem page so a domain with zero Problems still has a real list.
pub async fn read_deferred_expressions(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain)
        .await?
        .ok_or(CommentStudyReadError::InvalidQuery)?;
    let state = query.state.as_deref().unwrap_or("all");
    if !matches!(
        state,
        "all"
            | "pending"
            | "deferred_novel"
            | "deferred_ambiguous"
            | "deferred_context"
            | "retrieval_incomplete"
            | "budget_stopped"
            | "protocol_rejected"
            | "failed"
    ) {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    if query.q.is_some() {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    let page = read_page(
        database,
        query,
        "deferred-expressions",
        json!({"domainRef":domain_ref,"state":state}),
        "created_at_desc.resolution_ref_desc.v1",
    )
    .await?;
    let mut rows = sqlx::query(
        "SELECT resolution.resolution_ref,signal.signal_ref,signal.target_ref,target.run_ref, \
                signal.kind,signal.proposition,signal.evidence,resolution.state, \
                signal.content_public_ref,signal.comment_external_id, \
                current_comment.material_ref AS source_ref,current_comment.body_text AS comment_text, \
                current_comment.author_display_name,current_comment.author_external_id, \
                COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                    'pairRef',pair.pair_ref,'state',pair.state, \
                    'decisionReason',pair.pair_manifest->'decision'->>'code', \
                    'selection',CASE WHEN pair.pair_manifest ? 'selection' THEN jsonb_build_object( \
                      'recallRank',pair.pair_manifest->'selection'->'recallRank', \
                      'admissibleRank',pair.pair_manifest->'selection'->'admissibleRank') ELSE NULL END \
                  ) ORDER BY pair.created_at,pair.pair_ref) \
                  FROM linggan_comment_study_problem_pair pair \
                  JOIN linggan_comment_study_effective_signal partner \
                    ON partner.signal_ref=CASE WHEN pair.first_signal_ref=signal.signal_ref \
                      THEN pair.second_signal_ref ELSE pair.first_signal_ref END \
                   AND partner.domain_ref=signal.domain_ref \
                   AND partner.eligibility_state='eligible' \
                  WHERE (pair.first_signal_ref=signal.signal_ref \
                    OR pair.second_signal_ref=signal.signal_ref) \
                    AND pair.created_at<=$3::text::timestamptz), '[]'::jsonb) AS pair_outcomes, \
                to_char(resolution.created_at AT TIME ZONE 'UTC', \
                  'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at \
         FROM linggan_comment_study_resolution resolution \
         JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
         JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
         JOIN linggan_comment_study_current_comment current_comment \
           ON current_comment.content_public_ref=signal.content_public_ref \
          AND current_comment.comment_external_id=signal.comment_external_id \
         WHERE resolution.domain_ref=$1 AND signal.eligibility_state='eligible' \
           AND resolution.state IN ('pending','deferred_novel','deferred_ambiguous', \
                                    'deferred_context','retrieval_incomplete', \
                                    'budget_stopped','protocol_rejected','failed') \
           AND ($2='all' OR resolution.state=$2) \
           AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
             WHERE restriction.content_public_ref=signal.content_public_ref \
               AND restriction.comment_external_id=signal.comment_external_id) \
           AND NOT EXISTS(SELECT 1 FROM linggan_material_comment parent \
             JOIN linggan_material_comment_restriction restriction \
               ON restriction.content_public_ref=parent.content_public_ref \
              AND restriction.comment_external_id=parent.comment_external_id \
             WHERE parent.material_ref=target.parent_source_ref) \
           AND resolution.created_at<=$3::text::timestamptz \
           AND ($4::text IS NULL OR resolution.created_at<$4::text::timestamptz \
             OR (resolution.created_at=$4::text::timestamptz \
               AND resolution.resolution_ref<$5::uuid)) \
         ORDER BY resolution.created_at DESC,resolution.resolution_ref DESC LIMIT $6",
    ).bind(domain_ref).bind(state).bind(&page.as_of)
     .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
     .bind(page.after.as_ref().map(|position| position.reference))
     .bind(page.limit+1).fetch_all(database.pool()).await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "deferred-expressions",
                    row.get("created_at"),
                    row.get("resolution_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.deferred-expressions.v1","domainRef":domain_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "expressions":rows.into_iter().map(|row| json!({
            "resolutionRef":row.get::<Uuid,_>("resolution_ref"),
            "signalRef":row.get::<Uuid,_>("signal_ref"),
            "targetRef":row.get::<Uuid,_>("target_ref"),
            "runRef":row.get::<Uuid,_>("run_ref"),
            "state":row.get::<String,_>("state"),
            "pairOutcomes":row.get::<Value,_>("pair_outcomes"),
            "kind":row.get::<String,_>("kind"),
            "proposition":row.get::<String,_>("proposition"),
            "evidence":row.get::<String,_>("evidence"),
            "commentText":row.get::<String,_>("comment_text"),
            "authorDisplayName":row.get::<Option<String>,_>("author_display_name"),
            "authorExternalId":row.get::<Option<String>,_>("author_external_id"),
            "commentKey":{"workRef":row.get::<Uuid,_>("content_public_ref"),
                          "commentExternalId":row.get::<String,_>("comment_external_id")},
            "workRef":row.get::<Uuid,_>("content_public_ref"),
            "sourceRef":row.get::<Uuid,_>("source_ref"),
            "sourceState":"known",
            "createdAt":row.get::<String,_>("created_at")
        })).collect::<Vec<_>>()
    }))
}

pub async fn read_targets(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let run_ref = required_run(database, query).await?;
    let page = read_page(
        database,
        query,
        "targets",
        json!({"domainRef":query.domain,"runRef":run_ref}),
        "created_at_asc.target_ref_asc.v1",
    )
    .await?;
    let mut rows = sqlx::query(
        "SELECT target.target_ref,target.source_ref,target.parent_source_ref,target.research_text, \
                target.input_manifest,target.dependency_state,target.state,target.exclusion_reason, \
                target.terminal_reason,to_char(target.created_at AT TIME ZONE 'UTC', \
                    'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                work.context_state,work.context_manifest,work.content_public_ref,work.observation_role, \
                comment.body_text,comment.body_state,comment.comment_external_id,comment.parent_comment_external_id, \
                EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                  WHERE restriction.content_public_ref=comment.content_public_ref \
                    AND restriction.comment_external_id=comment.comment_external_id) AS source_restricted, \
                EXISTS(SELECT 1 FROM linggan_material_comment parent_comment \
                  JOIN linggan_material_comment_restriction restriction \
                    ON restriction.content_public_ref=parent_comment.content_public_ref \
                   AND restriction.comment_external_id=parent_comment.comment_external_id \
                  WHERE parent_comment.material_ref=target.parent_source_ref) AS parent_restricted, \
                (SELECT jsonb_build_object( \
                    'attemptOrdinal',attempt.attempt_ordinal,'state',attempt.state, \
                    'rejectionCode',attempt.rejection_code, \
                    'retryStrategy',attempt.output_manifest->>'retryStrategy', \
                    'modelReason',CASE WHEN attempt.output_manifest->>'outcome'='needs_context' \
                      THEN attempt.output_manifest->>'reason' ELSE NULL END, \
                    'providerFailureCode',COALESCE(invocation.result->>'failureCode',invocation.failure_code), \
                    'stage',invocation.result->'diagnostic'->>'stage', \
                    'responseLimit',invocation.result->'diagnostic'->>'limitKind', \
                    'httpStatus',invocation.result->'diagnostic'->>'httpStatus', \
                    'receivedBytes',invocation.result->'diagnostic'->'receivedBytes', \
                    'terminalReceived',invocation.result->'diagnostic'->'terminalReceived', \
                    'usageKnown',invocation.input_tokens IS NOT NULL AND invocation.output_tokens IS NOT NULL, \
                    'inputTokens',invocation.input_tokens,'outputTokens',invocation.output_tokens, \
                    'reservedTokens',invocation.reserved_tokens,'chargedTokens',invocation.charged_tokens \
                  ) FROM linggan_comment_study_semantic_attempt attempt \
                  LEFT JOIN linggan_model_invocation invocation \
                    ON invocation.invocation_ref=attempt.model_invocation_ref \
                  WHERE attempt.target_ref=target.target_ref \
                  ORDER BY attempt.attempt_ordinal DESC LIMIT 1) AS latest_attempt, \
                (SELECT count(*) FROM linggan_comment_study_semantic_attempt attempt \
                  WHERE attempt.target_ref=target.target_ref) AS attempt_count, \
                (SELECT count(*) FROM linggan_comment_study_signal signal WHERE signal.target_ref=target.target_ref) AS signal_count, \
                (SELECT resolution.state FROM linggan_comment_study_resolution resolution \
                   JOIN linggan_comment_study_signal signal USING(signal_ref) \
                   WHERE signal.target_ref=target.target_ref ORDER BY resolution.created_at DESC LIMIT 1) AS resolution_state \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_work work ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE target.run_ref=$1 AND target.created_at <= $2::text::timestamptz \
           AND ($3::text IS NULL OR target.created_at > $3::text::timestamptz \
             OR (target.created_at=$3::text::timestamptz AND target.target_ref>$4::uuid)) \
         ORDER BY target.created_at,target.target_ref LIMIT $5",
    )
    .bind(run_ref)
    .bind(&page.as_of)
    .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
    .bind(page.after.as_ref().map(|position| position.reference))
    .bind(page.limit + 1)
    .fetch_all(database.pool())
    .await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "targets",
                    row.get("created_at"),
                    row.get("target_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.read.v1","runRef":run_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "targets":rows.into_iter().map(|row| {
            let parent_restricted: bool = row.get("parent_restricted");
            let restricted: bool = row.get::<bool,_>("source_restricted") || parent_restricted;
            let body_state: String = row.get("body_state");
            let body_text: Option<String> = row.get("body_text");
            let (comment_text, source_state) = if restricted {
                (None, "restricted")
            } else if body_state == "KNOWN" {
                (body_text, "known")
            } else {
                (None, "unknown")
            };
            let parent_manifest: Value = row.get("input_manifest");
            let source_parent: Option<String> = row.get("parent_comment_external_id");
            let frozen_parent_ref: Option<Uuid> = row.get("parent_source_ref");
            let frozen_parent = if restricted {
                json!({"state":"restricted","sourceRef":frozen_parent_ref})
            } else if let Some(source_ref) = frozen_parent_ref {
                let manifest = &parent_manifest["parentContext"];
                if let Some(text) = manifest["researchText"].as_str() {
                    json!({"state":"available","sourceRef":source_ref,"researchText":text})
                } else {
                    json!({"state":"missing","sourceRef":source_ref})
                }
            } else if source_parent.is_some() {
                let manifest = &parent_manifest["parentContext"];
                if manifest["state"] == "missing" {
                    json!({"state":"missing"})
                } else {
                    json!({"state":"not_included"})
                }
            } else {
                json!({"state":"none"})
            };
            let mut latest_attempt: Option<Value> = row.get("latest_attempt");
            let reason_readable = source_state == "known" && !parent_restricted;
            if !reason_readable {
                if let Some(attempt) = latest_attempt.as_mut() {
                    attempt["modelReason"] = Value::Null;
                }
            }
            let model_reason = if reason_readable {
                latest_attempt
                    .as_ref()
                    .and_then(|attempt| attempt["modelReason"].as_str().map(str::to_owned))
            } else {
                None
            };
            json!({
            "targetRef":row.get::<Uuid,_>("target_ref"),"sourceRef":row.get::<Uuid,_>("source_ref"),
            "parentSourceRef":row.get::<Option<Uuid>,_>("parent_source_ref"),"workRef":row.get::<Uuid,_>("content_public_ref"),
            "commentKey":{"workRef":row.get::<Uuid,_>("content_public_ref"),
                          "commentExternalId":row.get::<Option<String>,_>("comment_external_id")},
            "observationRole":row.get::<String,_>("observation_role"),
            "commentText":comment_text,"sourceState":source_state,
            "researchText":if source_state == "known" { json!(row.get::<String,_>("research_text")) } else { Value::Null },
            "dependencyState":row.get::<String,_>("dependency_state"),
            "contextState":row.get::<String,_>("context_state"),"state":row.get::<String,_>("state"),
            "workContext":if restricted { Value::Null } else { row.get::<Value,_>("context_manifest") },"parentContext":frozen_parent,
            "modelReason":model_reason,
            "latestAttempt":latest_attempt,"attemptCount":row.get::<i64,_>("attempt_count"),
            "terminalReason":row.get::<Option<String>,_>("terminal_reason"),
            "exclusionReason":row.get::<Option<String>,_>("exclusion_reason"),"signalCount":row.get::<i64,_>("signal_count"),
            "resolutionState":row.get::<Option<String>,_>("resolution_state"),"createdAt":row.get::<String,_>("created_at")
        })}).collect::<Vec<_>>()
    }))
}

pub async fn read_signals(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let run_ref = required_run(database, query).await?;
    let page = read_page(
        database,
        query,
        "signals",
        json!({"domainRef":query.domain,"runRef":run_ref}),
        "created_at_desc.signal_ref_desc.v1",
    )
    .await?;
    let mut rows = sqlx::query(
        "SELECT signal.signal_ref,signal.target_ref,signal.kind,signal.proposition,signal.evidence,work.observation_role, \
                signal.problem_frame,signal.eligibility_state,signal.eligibility_reason, \
                to_char(signal.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                resolution.resolution_ref,resolution.state AS resolution_state,resolution.resolved_problem_ref, \
                membership.membership_ref,to_jsonb(membership)->>'problem_revision_ref' AS problem_revision_ref, \
                COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                    'pairRef',pair.pair_ref,'state',pair.state, \
                    'decisionReason',pair.pair_manifest->'decision'->>'code', \
                    'selection',CASE WHEN pair.pair_manifest ? 'selection' THEN jsonb_build_object( \
                        'recallRank',pair.pair_manifest->'selection'->'recallRank', \
                        'admissibleRank',pair.pair_manifest->'selection'->'admissibleRank' \
                    ) ELSE NULL END \
                  ) ORDER BY pair.created_at,pair.pair_ref) \
                  FROM linggan_comment_study_problem_pair pair \
                  WHERE pair.first_signal_ref=signal.signal_ref OR pair.second_signal_ref=signal.signal_ref), \
                 '[]'::jsonb) AS pair_outcomes, \
                EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                  WHERE restriction.content_public_ref=comment.content_public_ref \
                    AND restriction.comment_external_id=comment.comment_external_id) AS source_restricted, \
                EXISTS(SELECT 1 FROM linggan_material_comment parent_comment \
                  JOIN linggan_material_comment_restriction restriction \
                    ON restriction.content_public_ref=parent_comment.content_public_ref \
                   AND restriction.comment_external_id=parent_comment.comment_external_id \
                  WHERE parent_comment.material_ref=target.parent_source_ref) AS parent_restricted \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_comment_study_work work ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         LEFT JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         LEFT JOIN linggan_comment_study_problem_membership membership USING(signal_ref) \
         WHERE target.run_ref=$1 AND signal.created_at <= $2::text::timestamptz \
           AND ($3::text IS NULL OR signal.created_at < $3::text::timestamptz \
             OR (signal.created_at=$3::text::timestamptz AND signal.signal_ref<$4::uuid)) \
         ORDER BY signal.created_at DESC,signal.signal_ref DESC LIMIT $5",
    )
    .bind(run_ref)
    .bind(&page.as_of)
    .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
    .bind(page.after.as_ref().map(|position| position.reference))
    .bind(page.limit + 1)
    .fetch_all(database.pool())
    .await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "signals",
                    row.get("created_at"),
                    row.get("signal_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.read.v1","runRef":run_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "signals":rows.into_iter().map(|row| {
            let restricted: bool = row.get::<bool,_>("source_restricted") || row.get::<bool,_>("parent_restricted");
            let source_state = if restricted { "restricted" } else { "known" };
            let (proposition, evidence, problem_frame) = if restricted {
                (None, None, None)
            } else {
                (
                    Some(row.get::<String, _>("proposition")),
                    Some(row.get::<String, _>("evidence")),
                    row.get::<Option<Value>,_>("problem_frame"),
                )
            };
            json!({
            "signalRef":row.get::<Uuid,_>("signal_ref"),"targetRef":row.get::<Uuid,_>("target_ref"),
            "observationRole":row.get::<String,_>("observation_role"),
            "kind":row.get::<String,_>("kind"),"proposition":proposition,
            "evidence":evidence,"sourceState":source_state,"problemFrame":problem_frame,
            "eligibilityState":row.get::<String,_>("eligibility_state"),
            "eligibilityReason":if restricted { None } else { row.get::<Option<String>,_>("eligibility_reason") },
            "resolutionRef":row.get::<Option<Uuid>,_>("resolution_ref"),"resolutionState":row.get::<Option<String>,_>("resolution_state"),
            "resolvedProblemRef":row.get::<Option<Uuid>,_>("resolved_problem_ref"),"membershipRef":row.get::<Option<Uuid>,_>("membership_ref"),
            "problemRevisionRef":row.get::<Option<String>,_>("problem_revision_ref"),
            "pairOutcomes":row.get::<Value,_>("pair_outcomes"),
            "createdAt":row.get::<String,_>("created_at")
        })}).collect::<Vec<_>>()
    }))
}

pub async fn read_problems(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let state = query.state.as_deref().unwrap_or("current");
    if !matches!(
        state,
        "current" | "all" | "active" | "support_insufficient" | "merged" | "retired"
    ) {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    let search = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > 100) {
        return Err(CommentStudyReadError::InvalidQuery);
    }
    let search = search.map(|value| {
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    });
    let page = read_page(
        database,
        query,
        "problems",
        json!({"domainRef":domain_ref,"q":search,"state":state,"problemRef":query.problem_ref}),
        "created_at_desc.problem_ref_desc.v1",
    )
    .await?;
    let mut rows = sqlx::query(
        "SELECT problem.problem_ref,problem.domain_ref,revision.revision_ref,revision.title, \
                revision.definition,revision.core_frame,revision.inclusions,revision.exclusions, \
                problem.state, \
                to_char(problem.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                to_char(problem.retired_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS retired_at, \
                support.membership_count,support.support_comment_count, \
                support.support_author_count,support.support_work_count, \
                to_char(problem.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cursor_created_at, \
                NOT EXISTS(SELECT 1 FROM unnest(revision.seed_signal_refs) seed(signal_ref) \
                  JOIN linggan_comment_study_signal seed_signal USING(signal_ref) \
                  JOIN linggan_comment_study_target seed_target ON seed_target.target_ref=seed_signal.target_ref \
                  JOIN linggan_material_comment seed_source ON seed_source.material_ref=seed_target.source_ref \
                  WHERE EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                    WHERE restriction.content_public_ref=seed_source.content_public_ref \
                      AND restriction.comment_external_id=seed_source.comment_external_id) \
                     OR EXISTS(SELECT 1 FROM linggan_material_comment parent \
                       JOIN linggan_material_comment_restriction restriction \
                         ON restriction.content_public_ref=parent.content_public_ref \
                        AND restriction.comment_external_id=parent.comment_external_id \
                       WHERE parent.material_ref=seed_target.parent_source_ref)) AS definition_readable, \
                NOT EXISTS(SELECT 1 FROM unnest(revision.seed_signal_refs) seed(signal_ref) \
                  WHERE NOT EXISTS(SELECT 1 FROM linggan_comment_study_effective_signal current_seed \
                    WHERE current_seed.signal_ref=seed.signal_ref \
                      AND current_seed.eligibility_state='eligible')) AS definition_current \
         FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         CROSS JOIN LATERAL (SELECT count(*) AS membership_count, \
           count(DISTINCT (signal.content_public_ref,signal.comment_external_id)) AS support_comment_count, \
           count(DISTINCT (content.platform,current_comment.author_external_id)) \
             FILTER (WHERE current_comment.author_external_id IS NOT NULL AND btrim(current_comment.author_external_id)<>'') AS support_author_count, \
           count(DISTINCT signal.content_public_ref) AS support_work_count \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_current_comment current_comment \
             ON current_comment.content_public_ref=signal.content_public_ref \
            AND current_comment.comment_external_id=signal.comment_external_id \
           JOIN linggan_material_content content ON content.public_ref=signal.content_public_ref \
           WHERE membership.problem_ref=problem.problem_ref \
             AND signal.eligibility_state='eligible') support \
         WHERE problem.domain_ref=$1 AND ($8::uuid IS NULL OR problem.problem_ref=$8) \
           AND ($2='all' OR ($2='current' AND problem.state IN ('active','support_insufficient')) \
                OR problem.state=$2) \
           AND ($3::text IS NULL OR (NOT EXISTS( \
                 SELECT 1 FROM unnest(revision.seed_signal_refs) seed(signal_ref) \
                 JOIN linggan_comment_study_signal seed_signal USING(signal_ref) \
                 JOIN linggan_comment_study_target seed_target ON seed_target.target_ref=seed_signal.target_ref \
                 JOIN linggan_material_comment seed_source ON seed_source.material_ref=seed_target.source_ref \
                 WHERE EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                   WHERE restriction.content_public_ref=seed_source.content_public_ref \
                     AND restriction.comment_external_id=seed_source.comment_external_id) \
                    OR EXISTS(SELECT 1 FROM linggan_material_comment parent \
                      JOIN linggan_material_comment_restriction restriction \
                        ON restriction.content_public_ref=parent.content_public_ref \
                       AND restriction.comment_external_id=parent.comment_external_id \
                      WHERE parent.material_ref=seed_target.parent_source_ref)) \
               AND (revision.title ILIKE '%'||$3||'%' ESCAPE E'\\\\' \
                 OR revision.definition ILIKE '%'||$3||'%' ESCAPE E'\\\\'))) \
           AND problem.created_at <= $4::text::timestamptz \
           AND ($5::text IS NULL OR problem.created_at < $5::text::timestamptz \
             OR (problem.created_at=$5::text::timestamptz AND problem.problem_ref<$6::uuid)) \
         ORDER BY problem.created_at DESC,problem.problem_ref DESC LIMIT $7",
    )
    .bind(domain_ref)
    .bind(state)
    .bind(search)
    .bind(&page.as_of)
    .bind(page.after.as_ref().map(|position| position.created_at.as_str()))
    .bind(page.after.as_ref().map(|position| position.reference))
    .bind(page.limit + 1)
    .bind(query.problem_ref)
    .fetch_all(database.pool())
    .await?;
    let has_more = rows.len() > page.limit as usize;
    rows.truncate(page.limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                encode_next_cursor(
                    &page,
                    "problems",
                    row.get("cursor_created_at"),
                    row.get("problem_ref"),
                )
            })
            .transpose()?
    } else {
        None
    };
    let problem_refs: Vec<Uuid> = rows.iter().map(|row| row.get("problem_ref")).collect();
    let recent_rows = sqlx::query(
        "WITH first_membership AS ( \
           SELECT membership.problem_ref,source.content_public_ref,source.comment_external_id, \
                  min(membership.created_at) AS first_added_at \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
           WHERE membership.problem_ref=ANY($1::uuid[]) AND policy.domain_ref=$3 \
           GROUP BY membership.problem_ref,source.content_public_ref,source.comment_external_id \
         ), current_support AS ( \
           SELECT DISTINCT membership.problem_ref,signal.content_public_ref,signal.comment_external_id \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
           WHERE membership.problem_ref=ANY($1::uuid[]) AND signal.domain_ref=$3 \
             AND signal.eligibility_state='eligible' \
         ) \
         SELECT current_support.problem_ref, \
                count(*) FILTER (WHERE first.first_added_at >= \
                  $2::text::timestamptz - interval '28 days') AS recent_count, \
                to_char(max(first.first_added_at) FILTER (WHERE first.first_added_at >= \
                  $2::text::timestamptz - interval '28 days') AT TIME ZONE 'UTC', \
                  'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recent_added_at \
         FROM current_support JOIN first_membership first USING( \
           problem_ref,content_public_ref,comment_external_id) \
         GROUP BY current_support.problem_ref",
    )
    .bind(&problem_refs)
    .bind(&page.as_of)
    .bind(domain_ref)
    .fetch_all(database.pool())
    .await?;
    let recent: HashMap<Uuid, (i64, Option<String>)> = recent_rows
        .into_iter()
        .map(|row| {
            (
                row.get("problem_ref"),
                (row.get("recent_count"), row.get("recent_added_at")),
            )
        })
        .collect();
    Ok(json!({
        "contract":"comment-study.read.v1","domainRef":domain_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "problems":rows.into_iter().map(|row| {
            let recent_support = recent.get(&row.get::<Uuid,_>("problem_ref"));
            json!({
            "problemRef":row.get::<Uuid,_>("problem_ref"),"domainRef":row.get::<Uuid,_>("domain_ref"),
            "revisionRef":row.get::<Uuid,_>("revision_ref"),
            "title":if row.get::<bool,_>("definition_readable") { Some(row.get::<String,_>("title")) } else { None },
            "definition":if row.get::<bool,_>("definition_readable") { Some(row.get::<String,_>("definition")) } else { None },
            "definitionReadable":row.get::<bool,_>("definition_readable"),
            "definitionCurrent":row.get::<bool,_>("definition_current"),
            "definitionState":if !row.get::<bool,_>("definition_readable") { "seed_restricted" }
                else if !row.get::<bool,_>("definition_current") { "seed_superseded" }
                else { "current" },
            "stableIdentity":if row.get::<bool,_>("definition_readable") { Some(row.get::<Value,_>("core_frame")) } else { None },
            "includeCriteria":if row.get::<bool,_>("definition_readable") { Some(row.get::<Value,_>("inclusions")) } else { None },
            "excludeCriteria":if row.get::<bool,_>("definition_readable") { Some(row.get::<Value,_>("exclusions")) } else { None },
            "state":row.get::<String,_>("state"),"membershipCount":row.get::<i64,_>("membership_count"),
            "supportCommentCount":row.get::<i64,_>("support_comment_count"),
            "supportAuthorCount":row.get::<i64,_>("support_author_count"),
            "supportWorkCount":row.get::<i64,_>("support_work_count"),
            "recentAddedSupportCommentCount":recent_support.map(|item| item.0).unwrap_or(0),
            "recentAddedAt":recent_support.and_then(|item| item.1.as_deref()),
            "supportState":if !row.get::<bool,_>("definition_current") { "definition_stale" }
                else if row.get::<i64,_>("support_author_count") >= 2 { "supported" }
                else { "support_insufficient" },
            "createdAt":row.get::<String,_>("created_at"),
            "retiredAt":row.get::<Option<String>,_>("retired_at")
        })}).collect::<Vec<_>>()
    }))
}

/// One long-lived Problem, with each historical revision checked against its own seed lineage.
pub async fn read_problem_detail(
    database: &Database,
    query: &CommentStudyReadQuery,
    problem_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    let mut scoped = query.clone();
    scoped.problem_ref = Some(problem_ref);
    scoped.state = Some("all".to_owned());
    scoped.q = None;
    scoped.cursor = None;
    scoped.limit = Some(1);
    let list = read_problems(database, &scoped).await?;
    let problem = list["problems"]
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .ok_or(CommentStudyReadError::ProblemUnavailable)?;
    let revisions = sqlx::query(
        "SELECT revision.revision_ref,revision.identity_version,revision.title, \
                revision.definition,revision.core_frame,revision.inclusions,revision.exclusions, \
                to_char(revision.created_at AT TIME ZONE 'UTC', \
                  'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at, \
                cardinality(revision.seed_signal_refs)>0 AND NOT EXISTS( \
                  SELECT 1 FROM unnest(revision.seed_signal_refs) seed(signal_ref) \
                  LEFT JOIN linggan_comment_study_signal signal ON signal.signal_ref=seed.signal_ref \
                  LEFT JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
                  LEFT JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
                  LEFT JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
                  LEFT JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
                  WHERE source.material_ref IS NULL \
                     OR policy.domain_ref IS DISTINCT FROM revision.domain_ref \
                     OR EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                       WHERE restriction.content_public_ref=source.content_public_ref \
                         AND restriction.comment_external_id=source.comment_external_id) \
                     OR EXISTS(SELECT 1 FROM linggan_material_comment parent \
                       JOIN linggan_material_comment_restriction restriction \
                         ON restriction.content_public_ref=parent.content_public_ref \
                        AND restriction.comment_external_id=parent.comment_external_id \
                       WHERE parent.material_ref=target.parent_source_ref) \
                ) AS definition_readable \
         FROM linggan_comment_study_problem_revision revision \
         WHERE revision.problem_ref=$1 AND revision.domain_ref=$2 \
         ORDER BY revision.identity_version DESC",
    )
    .bind(problem_ref)
    .bind(query.domain)
    .fetch_all(database.pool())
    .await?;
    let source_distribution = read_problem_source_distribution(
        database,
        problem_ref,
        query.domain.ok_or(CommentStudyReadError::InvalidQuery)?,
    )
    .await?;
    Ok(json!({
        "contract":"comment-study.problem-detail.v1",
        "domainRef":problem["domainRef"],"problem":problem,
        "revisionHistory":revisions.into_iter().map(|row| {
            let readable: bool = row.get("definition_readable");
            json!({
                "revisionRef":row.get::<Uuid,_>("revision_ref"),
                "identityVersion":row.get::<i32,_>("identity_version"),
                "createdAt":row.get::<String,_>("created_at"),
                "definitionReadable":readable,
                "title":if readable { Some(row.get::<String,_>("title")) } else { None },
                "definition":if readable { Some(row.get::<String,_>("definition")) } else { None },
                "stableIdentity":if readable { Some(row.get::<Value,_>("core_frame")) } else { None },
                "includeCriteria":if readable { Some(row.get::<Value,_>("inclusions")) } else { None },
                "excludeCriteria":if readable { Some(row.get::<Value,_>("exclusions")) } else { None }
            })
        }).collect::<Vec<_>>(),
        "sourceDistribution":source_distribution
    }))
}

async fn read_problem_source_distribution(
    database: &Database,
    problem_ref: Uuid,
    domain_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    let distribution: Value = sqlx::query_scalar(
        "WITH first_membership AS ( \
           SELECT source.content_public_ref,source.comment_external_id, \
                  min(membership.created_at) AS added_at \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_comment_study_policy policy ON policy.policy_ref=run.policy_ref \
           JOIN linggan_material_comment source ON source.material_ref=target.source_ref \
           WHERE membership.problem_ref=$1 AND policy.domain_ref=$2 \
           GROUP BY source.content_public_ref,source.comment_external_id \
         ), support AS MATERIALIZED ( \
           SELECT DISTINCT signal.content_public_ref,signal.comment_external_id,first.added_at \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
           JOIN first_membership first \
             ON first.content_public_ref=signal.content_public_ref \
            AND first.comment_external_id=signal.comment_external_id \
           WHERE membership.problem_ref=$1 AND signal.domain_ref=$2 \
             AND signal.eligibility_state='eligible' \
         ), by_work AS ( \
           SELECT content_public_ref,count(*) AS comment_count,max(added_at) AS latest_added_at \
           FROM support GROUP BY content_public_ref \
         ), by_day AS ( \
           SELECT to_char(added_at AT TIME ZONE 'UTC','YYYY-MM-DD') AS day, \
                  count(*) AS comment_count FROM support GROUP BY 1 \
         ) \
         SELECT jsonb_build_object( \
           'supportCommentCount',(SELECT count(*) FROM support), \
           'works',COALESCE((SELECT jsonb_agg(jsonb_build_object( \
             'workRef',work.content_public_ref,'workTitle',detail.title, \
             'commentCount',work.comment_count, \
             'latestAddedAt',to_char(work.latest_added_at AT TIME ZONE 'UTC', \
               'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')) \
             ORDER BY work.comment_count DESC,work.content_public_ref) \
             FROM by_work work LEFT JOIN LATERAL ( \
               SELECT detail.title FROM linggan_material_content_detail detail \
               JOIN linggan_runtime_capture_package package \
                 ON package.package_ref=detail.package_ref \
               WHERE detail.content_public_ref=work.content_public_ref \
                 AND detail.title_state='KNOWN' AND package.accepted_at IS NOT NULL \
               ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC,detail.material_ref DESC \
               LIMIT 1) detail ON true),'[]'::jsonb), \
           'timeBuckets',COALESCE((SELECT jsonb_agg(jsonb_build_object( \
             'date',day,'commentCount',comment_count) ORDER BY day DESC) \
             FROM by_day),'[]'::jsonb))",
    )
    .bind(problem_ref)
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    Ok(distribution)
}

/// Current evidence is paged by stable comment identity. Multiple Signals from one comment are
/// grouped under one original voice; historical Run Signals remain readable through /signals.
pub async fn read_problem_evidence(
    database: &Database,
    query: &CommentStudyReadQuery,
    problem_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_problem \
         WHERE problem_ref=$1 AND domain_ref=$2)",
    )
    .bind(problem_ref)
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    if !exists {
        return Err(CommentStudyReadError::ProblemUnavailable);
    }
    let limit = query.limit()?;
    let resource = "problem-evidence";
    let scope_hash = cursor::scope_hash(&json!({
        "domainRef":domain_ref,"problemRef":problem_ref,"resource":resource,
        "order":"added_at_desc.work_ref_desc.comment_id_desc.v1"
    }))
    .map_err(map_cursor_error)?;
    let (as_of, after) = if let Some(value) = query.cursor.as_deref() {
        let decoded = cursor::decode_for::<CommentPosition>(resource, value, &scope_hash)
            .map_err(map_cursor_error)?;
        (decoded.as_of, Some(decoded.last))
    } else {
        let now: String = sqlx::query_scalar(
            "SELECT to_char(statement_timestamp() AT TIME ZONE 'UTC', \
             'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
        )
        .fetch_one(database.pool())
        .await?;
        (now, None)
    };
    let mut rows = sqlx::query(
        "WITH support AS MATERIALIZED ( \
           SELECT signal.content_public_ref,signal.comment_external_id, \
                  max(membership.created_at) AS added_at \
           FROM linggan_comment_study_problem_membership membership \
           JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
           WHERE membership.problem_ref=$1 AND membership.created_at <= $2::text::timestamptz \
             AND signal.eligibility_state='eligible' \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
               WHERE restriction.content_public_ref=signal.content_public_ref \
                 AND restriction.comment_external_id=signal.comment_external_id) \
             AND NOT EXISTS(SELECT 1 FROM linggan_material_comment parent \
               JOIN linggan_material_comment_restriction restriction \
                 ON restriction.content_public_ref=parent.content_public_ref \
                AND restriction.comment_external_id=parent.comment_external_id \
               WHERE parent.material_ref=target.parent_source_ref) \
           GROUP BY signal.content_public_ref,signal.comment_external_id \
         ), page AS ( \
           SELECT * FROM support \
           WHERE $3::text IS NULL OR added_at < $3::text::timestamptz \
             OR (added_at=$3::text::timestamptz AND (content_public_ref<$4::uuid \
               OR (content_public_ref=$4::uuid AND comment_external_id COLLATE \"C\" < $5::text COLLATE \"C\"))) \
           ORDER BY added_at DESC,content_public_ref DESC,comment_external_id COLLATE \"C\" DESC LIMIT $6 \
         ) \
         SELECT page.content_public_ref,page.comment_external_id, \
           to_char(page.added_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS added_at, \
           current_comment.material_ref AS source_ref,current_comment.body_text AS comment_text, \
           current_comment.author_external_id,current_comment.author_display_name, \
           COALESCE((SELECT jsonb_agg(jsonb_build_object( \
             'signalRef',signal.signal_ref,'kind',signal.kind,'proposition',signal.proposition, \
             'evidence',signal.evidence,'problemRevisionRef',to_jsonb(membership)->>'problem_revision_ref') \
             ORDER BY signal.created_at,signal.signal_ref) \
             FROM linggan_comment_study_problem_membership membership \
             JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
             JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref \
             WHERE membership.problem_ref=$1 AND membership.created_at <= $2::text::timestamptz \
               AND signal.eligibility_state='eligible' \
               AND signal.content_public_ref=page.content_public_ref \
               AND signal.comment_external_id=page.comment_external_id \
               AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                 WHERE restriction.content_public_ref=signal.content_public_ref \
                   AND restriction.comment_external_id=signal.comment_external_id) \
               AND NOT EXISTS(SELECT 1 FROM linggan_material_comment parent \
                 JOIN linggan_material_comment_restriction restriction \
                   ON restriction.content_public_ref=parent.content_public_ref \
                  AND restriction.comment_external_id=parent.comment_external_id \
                 WHERE parent.material_ref=target.parent_source_ref)), '[]'::jsonb) AS signals \
         FROM page JOIN linggan_comment_study_current_comment current_comment \
           ON current_comment.content_public_ref=page.content_public_ref \
          AND current_comment.comment_external_id=page.comment_external_id \
         WHERE NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
           WHERE restriction.content_public_ref=page.content_public_ref \
             AND restriction.comment_external_id=page.comment_external_id) \
         ORDER BY page.added_at DESC,page.content_public_ref DESC,page.comment_external_id COLLATE \"C\" DESC",
    ).bind(problem_ref).bind(&as_of)
     .bind(after.as_ref().map(|position| position.received_at.as_str()))
     .bind(after.as_ref().map(|position| position.work_ref))
     .bind(after.as_ref().map(|position| position.comment_external_id.as_str()))
     .bind(limit + 1).fetch_all(database.pool()).await?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if has_more {
        rows.last()
            .map(|row| {
                cursor::encode_for(
                    resource,
                    &scope_hash,
                    &as_of,
                    CommentPosition {
                        received_at: row.get("added_at"),
                        work_ref: row.get("content_public_ref"),
                        comment_external_id: row.get("comment_external_id"),
                    },
                )
                .map_err(map_cursor_error)
            })
            .transpose()?
    } else {
        None
    };
    Ok(json!({
        "contract":"comment-study.problem-evidence.v1","domainRef":domain_ref,
        "problemRef":problem_ref,
        "page":{"limit":limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":as_of},
        "evidence":rows.into_iter().map(|row| json!({
            "commentKey":{"workRef":row.get::<Uuid,_>("content_public_ref"),
                          "commentExternalId":row.get::<String,_>("comment_external_id")},
            "workRef":row.get::<Uuid,_>("content_public_ref"),
            "sourceRef":row.get::<Uuid,_>("source_ref"),
            "sourceState":"known",
            "commentText":row.get::<String,_>("comment_text"),
            "authorDisplayName":row.get::<Option<String>,_>("author_display_name"),
            "authorExternalId":row.get::<Option<String>,_>("author_external_id"),
            "addedAt":row.get::<String,_>("added_at"),
            "signals":row.get::<Value,_>("signals")
        })).collect::<Vec<_>>()
    }))
}

async fn ensure_schema(database: &Database) -> Result<(), CommentStudyReadError> {
    schema_ready(database)
        .await?
        .then_some(())
        .ok_or(CommentStudyReadError::SchemaUnavailable)
}

async fn resolved_domain(
    database: &Database,
    requested: Option<Uuid>,
) -> Result<Option<Uuid>, CommentStudyReadError> {
    let requested = requested.ok_or(CommentStudyReadError::InvalidQuery)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(requested)
            .fetch_one(database.pool())
            .await?;
    exists
        .then_some(Some(requested))
        .ok_or(CommentStudyReadError::InvalidQuery)
}

async fn required_run(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Uuid, CommentStudyReadError> {
    let requested = query.run_ref.ok_or(CommentStudyReadError::InvalidQuery)?;
    let domain_ref = query.domain.ok_or(CommentStudyReadError::InvalidQuery)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE run.run_ref=$1 AND policy.domain_ref=$2)",
    )
    .bind(requested)
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    exists
        .then_some(requested)
        .ok_or(CommentStudyReadError::RunUnavailable)
}
