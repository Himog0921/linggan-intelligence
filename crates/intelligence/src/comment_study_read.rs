//! Read-only projections for the clean comment-study lifecycle.
//!
//! An empty clean layer is an honest empty state, not a reason to relabel historical derived
//! results as a new StudyRun.

use crate::comment_study_catalog::StudyCatalogError;
use crate::comment_study_catalog::cursor::{self, EntryPosition};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 100;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentStudyReadQuery {
    pub domain: Option<Uuid>,
    pub run_ref: Option<Uuid>,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
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
                AND to_regclass('linggan_comment_study_resolution') IS NOT NULL",
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
    Ok(json!({
        "contract":"comment-study.read.v1",
        "domainRef":domain_ref,
        "policy":policy,
        "latestRun":latest_run,
        "observationSeries":observation_series,
        "cleanLayerState": if policy.is_some() { "configured" } else { "not_configured" }
    }))
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
    Ok(json!({
        "contract":"comment-study.read.v1",
        "domainRef":domain_ref,
        "page":{"limit":page.limit,"hasMore":has_more,"nextCursor":next_cursor,"asOf":page.as_of},
        "runs":rows.into_iter().map(|row| json!({
            "runRef":row.get::<Uuid,_>("run_ref"),"asOf":row.get::<String,_>("as_of"),
            "policyRef":row.get::<Uuid,_>("policy_ref"),
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
                comment.body_text,comment.body_state,comment.parent_comment_external_id, \
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
            let restricted: bool = row.get("source_restricted");
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
            let parent_restricted: bool = row.get("parent_restricted");
            let frozen_parent = if parent_restricted {
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
            "observationRole":row.get::<String,_>("observation_role"),
            "commentText":comment_text,"sourceState":source_state,
            "researchText":if source_state == "known" { json!(row.get::<String,_>("research_text")) } else { Value::Null },
            "dependencyState":row.get::<String,_>("dependency_state"),
            "contextState":row.get::<String,_>("context_state"),"state":row.get::<String,_>("state"),
            "workContext":row.get::<Value,_>("context_manifest"),"parentContext":frozen_parent,
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
            "eligibilityState":row.get::<String,_>("eligibility_state"),"eligibilityReason":row.get::<Option<String>,_>("eligibility_reason"),
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
    let limit = query.limit()?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let rows = sqlx::query(
        "SELECT problem.problem_ref,problem.domain_ref,revision.definition,revision.core_frame, \
                revision.inclusions,revision.exclusions,problem.state,problem.created_at::text AS created_at,problem.retired_at::text AS retired_at, \
                count(membership.membership_ref) AS membership_count \
         FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         LEFT JOIN linggan_comment_study_problem_membership membership \
           ON membership.problem_ref=problem.problem_ref \
         WHERE ($1::uuid IS NULL OR problem.domain_ref=$1) \
         GROUP BY problem.problem_ref,revision.revision_ref \
         ORDER BY problem.created_at DESC,problem.problem_ref DESC LIMIT $2",
    )
    .bind(domain_ref)
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(json!({
        "contract":"comment-study.read.v1","domainRef":domain_ref,
        "problems":rows.into_iter().map(|row| json!({
            "problemRef":row.get::<Uuid,_>("problem_ref"),"domainRef":row.get::<Uuid,_>("domain_ref"),
            "definition":row.get::<String,_>("definition"),"stableIdentity":row.get::<Value,_>("core_frame"),
            "includeCriteria":row.get::<Value,_>("inclusions"),"excludeCriteria":row.get::<Value,_>("exclusions"),
            "state":row.get::<String,_>("state"),"membershipCount":row.get::<i64,_>("membership_count"),
            "createdAt":row.get::<String,_>("created_at"),"retiredAt":row.get::<Option<String>,_>("retired_at")
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
