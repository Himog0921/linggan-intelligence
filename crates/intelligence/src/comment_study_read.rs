//! Read-only projections for the clean comment-study lifecycle.
//!
//! An empty clean layer is an honest empty state, not a reason to relabel historical derived
//! results as a new StudyRun.

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
    #[error("the clean comment-study schema is unavailable")]
    SchemaUnavailable,
    #[error("the requested run is absent or outside the selected domain")]
    RunUnavailable,
}

pub async fn schema_ready(database: &Database) -> Result<bool, CommentStudyReadError> {
    let ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_active_policy') IS NOT NULL \
                AND to_regclass('linggan_comment_study_run') IS NOT NULL \
                AND to_regclass('linggan_comment_study_target') IS NOT NULL \
                AND to_regclass('linggan_comment_study_signal') IS NOT NULL \
                AND to_regclass('linggan_comment_study_problem') IS NOT NULL \
                AND to_regclass('linggan_comment_study_resolution') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    Ok(ready)
}

/// Shows configuration and the latest run state without inferring study success from source
/// material counts.  Each count is a distinct lifecycle fact.
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
             WHERE active.singleton AND policy.domain_ref=$1",
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
    Ok(json!({
        "contract":"comment-study.read.v1",
        "domainRef":domain_ref,
        "policy":policy,
        "latestRun":latest_run,
        "cleanLayerState": if policy.is_some() { "configured" } else { "not_configured" }
    }))
}

pub async fn read_runs(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let limit = query.limit()?;
    let domain_ref = resolved_domain(database, query.domain).await?;
    let rows = sqlx::query(
        "SELECT run.run_ref,run.as_of::text AS as_of,run.state,run.created_at::text AS created_at,run.finished_at::text AS finished_at, \
                count(DISTINCT work.content_public_ref) AS work_count, \
                count(target.target_ref) AS target_count, \
                count(target.target_ref) FILTER (WHERE target.state='succeeded') AS succeeded_count, \
                count(target.target_ref) FILTER (WHERE target.state='no_signal') AS no_signal_count, \
                count(target.target_ref) FILTER (WHERE target.state='needs_context') AS needs_context_count, \
                count(target.target_ref) FILTER (WHERE target.state='failed') AS failed_count, \
                count(target.target_ref) FILTER (WHERE target.state='excluded') AS excluded_count \
         FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         LEFT JOIN linggan_comment_study_work work ON work.run_ref=run.run_ref \
         LEFT JOIN linggan_comment_study_target target ON target.run_ref=run.run_ref \
         WHERE ($1::uuid IS NULL OR policy.domain_ref=$1) \
         GROUP BY run.run_ref ORDER BY run.created_at DESC,run.run_ref DESC LIMIT $2",
    )
    .bind(domain_ref)
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(json!({
        "contract":"comment-study.read.v1",
        "domainRef":domain_ref,
        "runs":rows.into_iter().map(|row| json!({
            "runRef":row.get::<Uuid,_>("run_ref"),"asOf":row.get::<String,_>("as_of"),
            "state":row.get::<String,_>("state"),"createdAt":row.get::<String,_>("created_at"),
            "finishedAt":row.get::<Option<String>,_>("finished_at"),
            "workCount":row.get::<i64,_>("work_count"),"targetCount":row.get::<i64,_>("target_count"),
            "succeededCount":row.get::<i64,_>("succeeded_count"),"noSignalCount":row.get::<i64,_>("no_signal_count"),
            "needsContextCount":row.get::<i64,_>("needs_context_count"),"failedCount":row.get::<i64,_>("failed_count"),
            "excludedCount":row.get::<i64,_>("excluded_count")
        })).collect::<Vec<_>>()
    }))
}

pub async fn read_targets(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Value, CommentStudyReadError> {
    ensure_schema(database).await?;
    let limit = query.limit()?;
    let run_ref = required_run(database, query).await?;
    let rows = sqlx::query(
        "SELECT target.target_ref,target.source_ref,target.parent_source_ref,target.research_text, \
                target.dependency_state,target.state,target.exclusion_reason,target.created_at::text AS created_at, \
                work.context_state,work.content_public_ref, \
                comment.body_text,comment.body_state, \
                EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                  WHERE restriction.content_public_ref=comment.content_public_ref \
                    AND restriction.comment_external_id=comment.comment_external_id) AS source_restricted, \
                (SELECT count(*) FROM linggan_comment_study_signal signal WHERE signal.target_ref=target.target_ref) AS signal_count, \
                (SELECT resolution.state FROM linggan_comment_study_resolution resolution \
                   JOIN linggan_comment_study_signal signal USING(signal_ref) \
                   WHERE signal.target_ref=target.target_ref ORDER BY resolution.created_at DESC LIMIT 1) AS resolution_state \
         FROM linggan_comment_study_target target \
         JOIN linggan_comment_study_work work ON work.run_ref=target.run_ref AND work.content_public_ref=target.content_public_ref \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         WHERE target.run_ref=$1 ORDER BY target.created_at,target.target_ref LIMIT $2",
    )
    .bind(run_ref)
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(json!({
        "contract":"comment-study.read.v1","runRef":run_ref,
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
            json!({
            "targetRef":row.get::<Uuid,_>("target_ref"),"sourceRef":row.get::<Uuid,_>("source_ref"),
            "parentSourceRef":row.get::<Option<Uuid>,_>("parent_source_ref"),"workRef":row.get::<Uuid,_>("content_public_ref"),
            "commentText":comment_text,"sourceState":source_state,
            "researchText":row.get::<String,_>("research_text"),"dependencyState":row.get::<String,_>("dependency_state"),
            "contextState":row.get::<String,_>("context_state"),"state":row.get::<String,_>("state"),
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
    let limit = query.limit()?;
    let run_ref = required_run(database, query).await?;
    let rows = sqlx::query(
        "SELECT signal.signal_ref,signal.target_ref,signal.kind,signal.proposition,signal.evidence, \
                signal.problem_frame,signal.eligibility_state,signal.eligibility_reason,signal.created_at::text AS created_at, \
                resolution.resolution_ref,resolution.state AS resolution_state,resolution.resolved_problem_ref, \
                membership.membership_ref, \
                EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                  WHERE restriction.content_public_ref=comment.content_public_ref \
                    AND restriction.comment_external_id=comment.comment_external_id) AS source_restricted \
         FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         JOIN linggan_material_comment comment ON comment.material_ref=target.source_ref \
         LEFT JOIN linggan_comment_study_resolution resolution USING(signal_ref) \
         LEFT JOIN linggan_comment_study_problem_membership membership USING(signal_ref) \
         WHERE target.run_ref=$1 ORDER BY signal.created_at,signal.signal_ref LIMIT $2",
    )
    .bind(run_ref)
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(json!({
        "contract":"comment-study.read.v1","runRef":run_ref,
        "signals":rows.into_iter().map(|row| {
            let restricted: bool = row.get("source_restricted");
            let source_state = if restricted { "restricted" } else { "known" };
            let (proposition, evidence) = if restricted {
                (None, None)
            } else {
                (
                    Some(row.get::<String, _>("proposition")),
                    Some(row.get::<String, _>("evidence")),
                )
            };
            json!({
            "signalRef":row.get::<Uuid,_>("signal_ref"),"targetRef":row.get::<Uuid,_>("target_ref"),
            "kind":row.get::<String,_>("kind"),"proposition":proposition,
            "evidence":evidence,"sourceState":source_state,"problemFrame":row.get::<Option<Value>,_>("problem_frame"),
            "eligibilityState":row.get::<String,_>("eligibility_state"),"eligibilityReason":row.get::<Option<String>,_>("eligibility_reason"),
            "resolutionRef":row.get::<Option<Uuid>,_>("resolution_ref"),"resolutionState":row.get::<Option<String>,_>("resolution_state"),
            "resolvedProblemRef":row.get::<Option<Uuid>,_>("resolved_problem_ref"),"membershipRef":row.get::<Option<Uuid>,_>("membership_ref"),
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
    if requested.is_some() {
        return Ok(requested);
    }
    Ok(sqlx::query_scalar(
        "SELECT policy.domain_ref FROM linggan_comment_study_active_policy active \
         JOIN linggan_comment_study_policy policy USING(policy_ref) WHERE active.singleton",
    )
    .fetch_optional(database.pool())
    .await?)
}

async fn required_run(
    database: &Database,
    query: &CommentStudyReadQuery,
) -> Result<Uuid, CommentStudyReadError> {
    let requested = query.run_ref.ok_or(CommentStudyReadError::InvalidQuery)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_run run \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE run.run_ref=$1 AND ($2::uuid IS NULL OR policy.domain_ref=$2))",
    )
    .bind(requested)
    .bind(query.domain)
    .fetch_one(database.pool())
    .await?;
    exists
        .then_some(requested)
        .ok_or(CommentStudyReadError::RunUnavailable)
}
