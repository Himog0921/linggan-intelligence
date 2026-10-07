//! Shared P3 request ledger and dispatch fence for the resolution and pair stages.
//!
//! The semantic worker has a batch lease and a specialized dispatcher. Resolution and pair
//! requests use the same immutable request table and Run-level token budget, with the common lock
//! order Run -> stage subject -> request/invocation.

use crate::comment_study_policy::{StudyModelIdentity, json_hash};
use crate::comment_study_run::close_run_if_settled;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

const MAX_PROBLEM_STAGE_RECOVERIES_PER_TICK: i64 = 32;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProblemStageSubject {
    Resolution(Uuid),
    Pair(Uuid),
}

impl ProblemStageSubject {
    fn stage(self) -> &'static str {
        match self {
            Self::Resolution(_) => "resolution",
            Self::Pair(_) => "pair",
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum RequestLedgerError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the owning StudyRun is not enabled for model dispatch")]
    RunUnavailable,
    #[error("the StudyRun has active calls settling before this reservation")]
    BudgetDeferred,
    #[error("the StudyRun token budget is exhausted")]
    BudgetExhausted,
    #[error("the immutable stage request could not be recorded")]
    SnapshotUnavailable,
}

pub(crate) fn frozen_candidate_revision_refs(request_manifest: &Value) -> Option<Vec<Uuid>> {
    request_manifest["prompt"]
        .as_str()
        .and_then(|prompt| serde_json::from_str::<Value>(prompt).ok())
        .and_then(|payload| {
            payload
                .pointer("/input/candidates")
                .and_then(Value::as_array)
                .cloned()
        })
        .and_then(|candidates| {
            candidates
                .into_iter()
                .map(|candidate| {
                    candidate["problemRevisionRef"]
                        .as_str()
                        .and_then(|value| Uuid::parse_str(value).ok())
                })
                .collect::<Option<Vec<_>>>()
        })
        .filter(|revisions| !revisions.is_empty())
}

/// Builds and hashes the exact prompt and provider request that a problem-stage worker will send.
pub(crate) fn problem_stage_request_manifest(
    stage: &'static str,
    contract: &'static str,
    method_hash: &str,
    stage_hash: &str,
    config_ref: Uuid,
    model_identity: &StudyModelIdentity,
    timeout_seconds: i32,
    output_token_limit: i32,
    system_instruction: &str,
    output_schema: &Value,
    input: &Value,
) -> Result<(String, String, String, Value), serde_json::Error> {
    let provider_payload = json!({
        "contract":contract,
        "input":input,
        "outputSchema":output_schema,
    });
    let prompt = serde_json::to_string(&provider_payload)?;
    let request_manifest = json!({
        "contract":"comment-study.model-request.v1",
        "stage":stage,
        "methodHash":method_hash,
        "stageHash":stage_hash,
        "modelConfigRef":config_ref,
        "modelIdentity":model_identity,
        "parameters":{
            "operation":"analyze",
            "timeoutMs":i64::from(timeout_seconds)*1000,
            "maxOutputTokens":output_token_limit,
        },
        "systemInstruction":system_instruction,
        "prompt":prompt,
        "outputSchema":output_schema,
    });
    let request_hash = json_hash(&request_manifest).map_err(|_| {
        serde_json::Error::io(std::io::Error::other("invalid canonical request manifest"))
    })?;
    let input_context_hash =
        json_hash(&json!({"stageHash":stage_hash,"input":input})).map_err(|_| {
            serde_json::Error::io(std::io::Error::other("invalid canonical input context"))
        })?;
    Ok((prompt, request_hash, input_context_hash, request_manifest))
}

/// Reserves one stage request under the already-held Run lock. The SQL sums all request stages,
/// so semantic, resolution, and pair work consume the same frozen Run budget.
pub(crate) async fn reserve_problem_stage_call(
    transaction: &mut Transaction<'_, Postgres>,
    subject: ProblemStageSubject,
    run_ref: Uuid,
    policy_ref: Uuid,
    config_ref: Uuid,
    connection_version_ref: Uuid,
    model_ref: Uuid,
    reserved_tokens: i64,
    attempt_ordinal: i32,
    input_context_hash: &str,
    request_manifest: &Value,
    request_hash: &str,
    timeout_seconds: i32,
) -> Result<Uuid, RequestLedgerError> {
    let run = sqlx::query(
        "SELECT to_jsonb(run)->'selection_manifest' AS selection_manifest, \
                to_jsonb(run)->>'dispatch_state' AS dispatch_state, \
                to_jsonb(run)->>'dispatch_reason' AS dispatch_reason, \
                to_jsonb(run)->>'token_limit' AS token_limit \
         FROM linggan_comment_study_run run WHERE run.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(RequestLedgerError::RunUnavailable)?;
    let selection_manifest: Option<Value> = run.try_get("selection_manifest")?;
    if selection_manifest
        .as_ref()
        .and_then(|value| value["contract"].as_str())
        != Some("comment-study.run-selection.v2")
        || run
            .try_get::<Option<String>, _>("dispatch_state")?
            .as_deref()
            != Some("enabled")
        || run
            .try_get::<Option<String>, _>("dispatch_reason")?
            .is_some()
    {
        return Err(RequestLedgerError::RunUnavailable);
    }
    let token_limit = run
        .try_get::<Option<String>, _>("token_limit")?
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(RequestLedgerError::RunUnavailable)?;
    let spent_or_reserved: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN invocation.state='running' \
                                 AND (invocation.input_tokens IS NULL OR invocation.output_tokens IS NULL) \
                                 THEN GREATEST(invocation.reserved_tokens,COALESCE(invocation.charged_tokens,0)) \
                                 ELSE COALESCE(invocation.charged_tokens,invocation.reserved_tokens) END),0)::bigint \
         FROM linggan_model_invocation invocation \
         WHERE invocation.invocation_ref IN ( \
           SELECT request.invocation_ref FROM linggan_comment_study_model_request request WHERE request.run_ref=$1 \
           UNION SELECT resolution.model_invocation_ref \
             FROM linggan_comment_study_resolution resolution \
             JOIN linggan_comment_study_signal signal USING(signal_ref) \
             JOIN linggan_comment_study_target target USING(target_ref) \
             JOIN linggan_comment_study_run owner ON owner.run_ref=target.run_ref \
             WHERE target.run_ref=$1 AND owner.selection_manifest->>'contract'='comment-study.run-selection.v2' \
               AND resolution.model_invocation_ref IS NOT NULL \
           UNION SELECT pair.model_invocation_ref \
             FROM linggan_comment_study_problem_pair pair \
             JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
             JOIN linggan_comment_study_target target USING(target_ref) \
             JOIN linggan_comment_study_run owner ON owner.run_ref=target.run_ref \
             WHERE target.run_ref=$1 AND owner.selection_manifest->>'contract'='comment-study.run-selection.v2' \
               AND pair.model_invocation_ref IS NOT NULL \
           UNION SELECT legacy.invocation_ref FROM linggan_model_invocation legacy \
             WHERE legacy.result->>'legacyRequestLedgerMissing'='true' \
               AND legacy.result->>'legacyRunRef'=$1::text \
         )",
    )
    .bind(run_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if spent_or_reserved.saturating_add(reserved_tokens) > token_limit {
        let in_flight: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_model_invocation invocation \
             WHERE invocation.state='running' AND invocation.invocation_ref IN ( \
               SELECT request.invocation_ref FROM linggan_comment_study_model_request request WHERE request.run_ref=$1 \
               UNION SELECT resolution.model_invocation_ref \
                 FROM linggan_comment_study_resolution resolution \
                 JOIN linggan_comment_study_signal signal USING(signal_ref) \
                 JOIN linggan_comment_study_target target USING(target_ref) \
                 JOIN linggan_comment_study_run owner ON owner.run_ref=target.run_ref \
                 WHERE target.run_ref=$1 AND owner.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                   AND resolution.model_invocation_ref IS NOT NULL \
               UNION SELECT pair.model_invocation_ref \
                 FROM linggan_comment_study_problem_pair pair \
                 JOIN linggan_comment_study_signal signal ON signal.signal_ref=pair.first_signal_ref \
                 JOIN linggan_comment_study_target target USING(target_ref) \
                 JOIN linggan_comment_study_run owner ON owner.run_ref=target.run_ref \
                 WHERE target.run_ref=$1 AND owner.selection_manifest->>'contract'='comment-study.run-selection.v2' \
                   AND pair.model_invocation_ref IS NOT NULL \
               UNION SELECT legacy.invocation_ref FROM linggan_model_invocation legacy \
                 WHERE legacy.result->>'legacyRequestLedgerMissing'='true' \
                   AND legacy.result->>'legacyRunRef'=$1::text \
             )",
        )
        .bind(run_ref)
        .fetch_one(&mut **transaction)
        .await?;
        if in_flight != 0 {
            return Err(RequestLedgerError::BudgetDeferred);
        }
        sqlx::query(
            "UPDATE linggan_comment_study_run SET dispatch_state='stopped', \
               dispatch_reason='budget_exhausted',control_version=control_version+1 \
             WHERE run_ref=$1 AND dispatch_state='enabled'",
        )
        .bind(run_ref)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_target SET state='cancelled', \
               finished_at=scope_001_now(),terminal_reason='budget_exhausted' \
             WHERE run_ref=$1 AND state IN ('queued','running')",
        )
        .bind(run_ref)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_resolution resolution \
             SET state='budget_stopped',model_invocation_ref=NULL, \
                 decision_manifest=jsonb_build_object('reason','budget_exhausted'), \
                 resolved_at=scope_001_now() \
             FROM linggan_comment_study_signal signal \
             JOIN linggan_comment_study_target target USING(target_ref) \
             WHERE resolution.signal_ref=signal.signal_ref AND target.run_ref=$1 \
               AND resolution.state='pending'",
        )
        .bind(run_ref)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "UPDATE linggan_comment_study_problem_pair pair \
             SET state='failed',model_invocation_ref=NULL,resolved_at=scope_001_now(), \
                 pair_manifest=jsonb_set(pair.pair_manifest,'{decision}', \
                   jsonb_build_object('code','budget_exhausted'),true) \
             FROM linggan_comment_study_signal signal \
             JOIN linggan_comment_study_target target USING(target_ref) \
             WHERE pair.first_signal_ref=signal.signal_ref AND target.run_ref=$1 \
               AND pair.state='pending'",
        )
        .bind(run_ref)
        .execute(&mut **transaction)
        .await?;
        close_run_if_settled(transaction, run_ref).await?;
        return Err(RequestLedgerError::BudgetExhausted);
    }

    let invocation_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_invocation( \
           invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash, \
           state,reserved_tokens,charged_tokens,result \
         ) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,0,$7)",
    )
    .bind(invocation_ref)
    .bind(connection_version_ref)
    .bind(model_ref)
    .bind(config_ref)
    .bind(request_hash)
    .bind(reserved_tokens)
    .bind(json!({"runRef":run_ref,"stage":subject.stage(),"callStarted":false}))
    .execute(&mut **transaction)
    .await?;

    let (resolution_ref, pair_ref) = match subject {
        ProblemStageSubject::Resolution(reference) => (Some(reference), None),
        ProblemStageSubject::Pair(reference) => (None, Some(reference)),
    };
    let inserted = sqlx::query(
        "INSERT INTO linggan_comment_study_model_request( \
           invocation_ref,run_ref,policy_ref,stage,resolution_ref,pair_ref,attempt_ordinal, \
           input_context_hash,request_manifest,request_hash,deadline_at \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10, \
           scope_001_now()+make_interval(secs=>$11)) ON CONFLICT DO NOTHING",
    )
    .bind(invocation_ref)
    .bind(run_ref)
    .bind(policy_ref)
    .bind(subject.stage())
    .bind(resolution_ref)
    .bind(pair_ref)
    .bind(attempt_ordinal)
    .bind(input_context_hash)
    .bind(request_manifest)
    .bind(request_hash)
    .bind(i64::from(timeout_seconds.saturating_add(15)))
    .execute(&mut **transaction)
    .await?;
    if inserted.rows_affected() != 1 {
        return Err(RequestLedgerError::SnapshotUnavailable);
    }
    let updated = match subject {
        ProblemStageSubject::Resolution(reference) => {
            sqlx::query(
                "UPDATE linggan_comment_study_resolution SET model_invocation_ref=$2 \
                 WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
            )
            .bind(reference)
            .bind(invocation_ref)
            .execute(&mut **transaction)
            .await?
        }
        ProblemStageSubject::Pair(reference) => {
            sqlx::query(
                "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=$2 \
                 WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref IS NULL",
            )
            .bind(reference)
            .bind(invocation_ref)
            .execute(&mut **transaction)
            .await?
        }
    };
    if updated.rows_affected() != 1 {
        return Err(RequestLedgerError::SnapshotUnavailable);
    }
    Ok(invocation_ref)
}

/// The final fence runs after credentials/request construction and immediately before adapter I/O.
pub(crate) async fn mark_problem_stage_dispatch_started(
    database: &linggan_storage_postgres::Database,
    subject: ProblemStageSubject,
    invocation_ref: Uuid,
) -> Result<(), sqlx::Error> {
    let started = Instant::now();
    let mut transaction = database.pool().begin().await?;
    let pool_wait = started.elapsed();
    let request_row = sqlx::query(
        "SELECT run_ref,request_manifest FROM linggan_comment_study_model_request \
         WHERE invocation_ref=$1 AND stage=$2",
    )
    .bind(invocation_ref)
    .bind(subject.stage())
    .fetch_optional(&mut *transaction)
    .await?;
    let request_row = request_row.ok_or(sqlx::Error::RowNotFound)?;
    let run_ref: Uuid = request_row.get("run_ref");
    let frozen_candidate_revisions = match subject {
        ProblemStageSubject::Resolution(_) => {
            frozen_candidate_revision_refs(&request_row.get::<Value, _>("request_manifest"))
                .ok_or(sqlx::Error::RowNotFound)?
        }
        ProblemStageSubject::Pair(_) => Vec::new(),
    };
    let subject_ref = match subject {
        ProblemStageSubject::Resolution(reference) | ProblemStageSubject::Pair(reference) => {
            reference
        }
    };
    let request_read_wait = started.elapsed().saturating_sub(pool_wait);
    let locked: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let run_lock_wait = started
        .elapsed()
        .saturating_sub(pool_wait + request_read_wait);
    let subject_locked = match subject {
        ProblemStageSubject::Resolution(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT resolution_ref FROM linggan_comment_study_resolution \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
        ProblemStageSubject::Pair(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT pair_ref FROM linggan_comment_study_problem_pair \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
    };
    if subject_locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let subject_lock_wait = started
        .elapsed()
        .saturating_sub(pool_wait + request_read_wait + run_lock_wait);
    let fenced = match subject {
        ProblemStageSubject::Resolution(_) => {
            sqlx::query_scalar::<_, bool>(
                "UPDATE linggan_comment_study_model_request request \
                 SET dispatch_started_at=scope_001_now() \
                 FROM linggan_comment_study_run run \
                 WHERE request.invocation_ref=$1 AND request.run_ref=run.run_ref \
                   AND request.stage='resolution' AND request.resolution_ref=$2 \
                   AND request.dispatch_started_at IS NULL \
                   AND request.deadline_at>scope_001_now() \
                   AND run.dispatch_state='enabled' AND run.dispatch_reason IS NULL \
                   AND EXISTS ( \
                     SELECT 1 FROM linggan_comment_study_resolution resolution \
                     JOIN linggan_comment_study_effective_signal signal USING(signal_ref) \
                     WHERE resolution.resolution_ref=request.resolution_ref \
                       AND signal.eligibility_state='eligible' \
                       AND cardinality($3::uuid[])>0 \
                       AND NOT EXISTS ( \
                         SELECT 1 FROM unnest($3::uuid[]) candidate(revision_ref) \
                         LEFT JOIN linggan_comment_study_current_problem problem \
                           ON problem.current_revision_ref=candidate.revision_ref \
                         WHERE problem.problem_ref IS NULL)) \
                 RETURNING request.dispatch_started_at < request.deadline_at",
            )
            .bind(invocation_ref)
            .bind(subject_ref)
            .bind(&frozen_candidate_revisions)
            .fetch_optional(&mut *transaction)
            .await?
        }
        ProblemStageSubject::Pair(_) => {
            // The effective_signal view computes the latest head for every comment. Joining it
            // twice inside UPDATE took 380 seconds on the local corpus. Resolve only this pair's
            // two frozen signals, using the same head/current-comment ordering and restrictions.
            sqlx::query_scalar::<_, bool>(
                r#"WITH wanted AS MATERIALIZED (
                     SELECT signal.signal_ref, signal.eligibility_state, target.target_ref,
                            target.content_public_ref, target.parent_source_ref, target.state,
                            policy.domain_ref, studied.comment_external_id,
                            studied.content_public_ref AS studied_work_ref,
                            studied.body_state AS studied_body_state,
                            studied.body_text AS studied_body_text
                     FROM linggan_comment_study_problem_pair pair
                     CROSS JOIN LATERAL (VALUES (pair.first_signal_ref),
                                                (pair.second_signal_ref)) refs(signal_ref)
                     JOIN linggan_comment_study_signal signal ON signal.signal_ref=refs.signal_ref
                     JOIN linggan_comment_study_target target ON target.target_ref=signal.target_ref
                     JOIN linggan_comment_study_run owner ON owner.run_ref=target.run_ref
                     JOIN linggan_comment_study_policy policy ON policy.policy_ref=owner.policy_ref
                     JOIN linggan_material_comment studied ON studied.material_ref=target.source_ref
                     WHERE pair.pair_ref=$2
                   ), eligible AS (
                     SELECT wanted.signal_ref, wanted.domain_ref,
                            current_comment.author_external_id
                     FROM wanted
                     JOIN LATERAL (
                       SELECT newer.target_ref, newer.state
                       FROM linggan_comment_study_target newer
                       JOIN linggan_material_comment source ON source.material_ref=newer.source_ref
                       JOIN linggan_comment_study_run newer_run ON newer_run.run_ref=newer.run_ref
                       JOIN linggan_comment_study_policy newer_policy
                         ON newer_policy.policy_ref=newer_run.policy_ref
                       WHERE newer.content_public_ref=wanted.content_public_ref
                         AND source.content_public_ref=newer.content_public_ref
                         AND source.comment_external_id=wanted.comment_external_id
                         AND newer_policy.domain_ref=wanted.domain_ref
                         AND newer.state IN ('succeeded','no_signal')
                       ORDER BY newer.created_at DESC, newer.target_ref DESC LIMIT 1
                     ) head ON head.target_ref=wanted.target_ref AND head.state='succeeded'
                     JOIN LATERAL (
                       SELECT comment.author_external_id, comment.body_state, comment.body_text
                       FROM linggan_material_comment comment
                       JOIN linggan_runtime_capture_package package
                         ON package.package_ref=comment.package_ref
                       WHERE comment.content_public_ref=wanted.content_public_ref
                         AND comment.comment_external_id=wanted.comment_external_id
                         AND package.accepted_at IS NOT NULL
                       ORDER BY comment.observed_at::timestamptz DESC,
                                comment.created_at DESC, comment.material_ref DESC LIMIT 1
                     ) current_comment ON true
                     WHERE wanted.state='succeeded'
                       AND wanted.eligibility_state='eligible'
                       AND wanted.comment_external_id IS NOT NULL
                       AND wanted.content_public_ref=wanted.studied_work_ref
                       AND wanted.studied_body_state='KNOWN'
                       AND wanted.studied_body_text IS NOT NULL
                       AND current_comment.body_state='KNOWN'
                       AND current_comment.body_text IS NOT NULL
                       AND current_comment.body_text=wanted.studied_body_text
                       AND NULLIF(btrim(current_comment.author_external_id),'') IS NOT NULL
                       AND EXISTS (
                         SELECT 1 FROM linggan_material_content_author work_author
                         WHERE work_author.content_public_ref=wanted.content_public_ref
                           AND NULLIF(btrim(work_author.author_external_id),'') IS NOT NULL
                           AND btrim(current_comment.author_external_id)<>
                               btrim(work_author.author_external_id))
                       AND NOT EXISTS (
                         SELECT 1 FROM linggan_material_comment_restriction restriction
                         WHERE restriction.content_public_ref=wanted.content_public_ref
                           AND restriction.comment_external_id=wanted.comment_external_id)
                       AND NOT EXISTS (
                         SELECT 1 FROM linggan_material_comment parent
                         JOIN linggan_material_comment_restriction restriction
                           ON restriction.content_public_ref=parent.content_public_ref
                          AND restriction.comment_external_id=parent.comment_external_id
                         WHERE parent.material_ref=wanted.parent_source_ref)
                   )
                   UPDATE linggan_comment_study_model_request request
                   SET dispatch_started_at=scope_001_now()
                   FROM linggan_comment_study_run run
                   WHERE request.invocation_ref=$1 AND request.run_ref=run.run_ref
                     AND request.stage='pair' AND request.pair_ref=$2
                     AND request.dispatch_started_at IS NULL
                     AND request.deadline_at>scope_001_now()
                     AND run.dispatch_state='enabled' AND run.dispatch_reason IS NULL
                     AND (SELECT count(DISTINCT signal_ref)=2
                                   AND count(DISTINCT domain_ref)=1
                                   AND count(DISTINCT btrim(author_external_id))=2
                          FROM eligible)
                   RETURNING request.dispatch_started_at < request.deadline_at"#,
            )
            .bind(invocation_ref)
            .bind(subject_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
    };
    let eligibility_wait = started
        .elapsed()
        .saturating_sub(pool_wait + request_read_wait + run_lock_wait + subject_lock_wait);
    if started.elapsed() >= Duration::from_secs(1) || fenced == Some(false) {
        eprintln!(
            "linggan worker: problem stage fence timing stage={} pool_ms={} request_ms={} run_lock_ms={} subject_lock_ms={} eligibility_ms={} late={}",
            subject.stage(),
            pool_wait.as_millis(),
            request_read_wait.as_millis(),
            run_lock_wait.as_millis(),
            subject_lock_wait.as_millis(),
            eligibility_wait.as_millis(),
            fenced == Some(false)
        );
    }
    // The WHERE deadline predicate can be evaluated before expensive eligibility work. The
    // timestamp written by SET is authoritative; rollback if it crossed the deadline meanwhile.
    if fenced != Some(true) {
        return Err(sqlx::Error::RowNotFound);
    }
    let marked = sqlx::query(
        "UPDATE linggan_model_invocation SET result=COALESCE(result,'{}'::jsonb)||'{\"callStarted\":true}'::jsonb \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .execute(&mut *transaction)
    .await?;
    if marked.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    transaction.commit().await?;
    Ok(())
}

/// An invocation that never crossed the durable dispatch fence must not consume the budget.
pub(crate) async fn release_problem_stage_before_dispatch(
    database: &linggan_storage_postgres::Database,
    subject: ProblemStageSubject,
    invocation_ref: Uuid,
    failure_code: &str,
) -> Result<(), sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let run_ref: Uuid = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_model_request \
         WHERE invocation_ref=$1 AND stage=$2 AND dispatch_started_at IS NULL",
    )
    .bind(invocation_ref)
    .bind(subject.stage())
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(sqlx::Error::RowNotFound)?;
    let locked: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let subject_locked = match subject {
        ProblemStageSubject::Resolution(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT resolution_ref FROM linggan_comment_study_resolution \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
        ProblemStageSubject::Pair(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT pair_ref FROM linggan_comment_study_problem_pair \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
    };
    if subject_locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let failed = sqlx::query(
        "UPDATE linggan_model_invocation SET state='failed',charged_tokens=0, \
           failure_code=$2,result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('ok',false,'failureCode',$2), \
           finished_at=scope_001_now() \
         WHERE invocation_ref=$1 AND state='running' AND EXISTS( \
           SELECT 1 FROM linggan_comment_study_model_request request \
           WHERE request.invocation_ref=$1 AND request.dispatch_started_at IS NULL)",
    )
    .bind(invocation_ref)
    .bind(failure_code)
    .execute(&mut *transaction)
    .await?;
    if failed.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    let released = match subject {
        ProblemStageSubject::Resolution(reference) => {
            sqlx::query(
                "UPDATE linggan_comment_study_resolution SET model_invocation_ref=NULL \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2",
            )
            .bind(reference)
            .bind(invocation_ref)
            .execute(&mut *transaction)
            .await?
        }
        ProblemStageSubject::Pair(reference) => {
            sqlx::query(
                "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=NULL \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2",
            )
            .bind(reference)
            .bind(invocation_ref)
            .execute(&mut *transaction)
            .await?
        }
    };
    if released.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    if let ProblemStageSubject::Resolution(reference) = subject {
        // A frozen candidate whose revision or seed became unavailable cannot be retried with
        // its old definition. Keep the reason visible in the unfiled expression queue.
        sqlx::query(
            "UPDATE linggan_comment_study_resolution resolution \
             SET state='retrieval_incomplete', \
                 decision_manifest=jsonb_build_object('reason','candidate_revision_unavailable'), \
                 resolved_at=scope_001_now() \
             WHERE resolution.resolution_ref=$1 AND resolution.state='pending' \
               AND jsonb_typeof(resolution.candidate_manifest->'candidateProblemRevisions')='array' \
               AND EXISTS ( \
                 SELECT 1 FROM jsonb_to_recordset(resolution.candidate_manifest->'candidateProblemRevisions') \
                   AS candidate(\"problemRef\" uuid,\"problemRevisionRef\" uuid) \
                 LEFT JOIN linggan_comment_study_current_problem problem \
                   ON problem.problem_ref=candidate.\"problemRef\" \
                  AND problem.current_revision_ref=candidate.\"problemRevisionRef\" \
                 WHERE problem.problem_ref IS NULL)",
        )
        .bind(reference)
        .execute(&mut *transaction)
        .await?;
    }
    close_run_if_settled(&mut transaction, run_ref).await?;
    transaction.commit().await?;
    Ok(())
}

/// Reopens only a still-pending stage after a dispatched request failed. The immutable invocation
/// remains charged at measured usage or its reservation; the configured attempt limit closes the
/// stage with an explicit failure state.
pub(crate) async fn release_problem_stage_after_failure(
    database: &linggan_storage_postgres::Database,
    subject: ProblemStageSubject,
    invocation_ref: Uuid,
    failure_code: &str,
) -> Result<(), sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let row = sqlx::query(
        "SELECT request.run_ref,request.attempt_ordinal,config.max_attempts \
         FROM linggan_comment_study_model_request request \
         JOIN linggan_model_invocation invocation USING(invocation_ref) \
         JOIN linggan_model_config config ON config.config_ref=invocation.config_ref \
         WHERE request.invocation_ref=$1 AND request.stage=$2",
    )
    .bind(invocation_ref)
    .bind(subject.stage())
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(sqlx::Error::RowNotFound)?;
    let run_ref: Uuid = row.get("run_ref");
    let attempt_ordinal: i32 = row.get("attempt_ordinal");
    let max_attempts: i32 = row.get("max_attempts");
    let locked: Option<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_study_run WHERE run_ref=$1 FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    let subject_locked = match subject {
        ProblemStageSubject::Resolution(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT resolution_ref FROM linggan_comment_study_resolution \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
        ProblemStageSubject::Pair(reference) => {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT pair_ref FROM linggan_comment_study_problem_pair \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
        }
    };
    if subject_locked.is_none() {
        return Err(sqlx::Error::RowNotFound);
    }
    apply_problem_stage_failure(
        &mut transaction,
        subject,
        invocation_ref,
        attempt_ordinal,
        max_attempts,
        failure_code,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
}

async fn apply_problem_stage_failure(
    transaction: &mut Transaction<'_, Postgres>,
    subject: ProblemStageSubject,
    invocation_ref: Uuid,
    attempt_ordinal: i32,
    max_attempts: i32,
    failure_code: &str,
) -> Result<(), sqlx::Error> {
    let changed = match (subject, attempt_ordinal < max_attempts) {
        (ProblemStageSubject::Resolution(reference), true) => sqlx::query(
            "UPDATE linggan_comment_study_resolution SET model_invocation_ref=NULL \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2",
        )
        .bind(reference)
        .bind(invocation_ref)
        .execute(&mut **transaction)
        .await?,
        (ProblemStageSubject::Resolution(reference), false) => sqlx::query(
            "UPDATE linggan_comment_study_resolution SET state='failed', \
               decision_manifest=jsonb_build_object('reason','attempts_exhausted','lastFailureCode',$3), \
               resolved_at=scope_001_now() \
             WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$2",
        )
        .bind(reference)
        .bind(invocation_ref)
        .bind(&failure_code)
        .execute(&mut **transaction)
        .await?,
        (ProblemStageSubject::Pair(reference), true) => sqlx::query(
            "UPDATE linggan_comment_study_problem_pair SET model_invocation_ref=NULL \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2",
        )
        .bind(reference)
        .bind(invocation_ref)
        .execute(&mut **transaction)
        .await?,
        (ProblemStageSubject::Pair(reference), false) => sqlx::query(
            "UPDATE linggan_comment_study_problem_pair \
             SET state='failed',resolved_at=scope_001_now(), \
                 pair_manifest=jsonb_set(pair_manifest,'{decision}', \
                   jsonb_build_object('code','attempts_exhausted','lastFailureCode',$3),true) \
             WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$2",
        )
        .bind(reference)
        .bind(invocation_ref)
        .bind(failure_code)
        .execute(&mut **transaction)
        .await?,
    };
    if changed.rows_affected() != 1 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

/// Recovers expired resolution/pair invocations at the beginning of a worker tick. A dispatched
/// request with unknown usage keeps its reservation; a request that never crossed the fence is
/// safely settled at zero. Late responses cannot reclaim the now-closed invocation.
pub(crate) async fn recover_expired_problem_stage_requests(
    database: &linggan_storage_postgres::Database,
) -> Result<u64, sqlx::Error> {
    let has_request_table: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_model_request') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !has_request_table {
        return Ok(0);
    }
    let pair_failure_state_supported = pair_failure_state_supported(database).await?;
    // Recover a bounded page per stage. Keep the older no-ledger receipts inside the same
    // per-stage allowance below, so legacy cleanup cannot double the intended tick budget.
    let recoverable: Vec<(Uuid, Uuid, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "WITH eligible AS ( \
           SELECT request.run_ref,request.invocation_ref,request.stage, \
                  request.resolution_ref,request.pair_ref,request.deadline_at \
           FROM linggan_comment_study_model_request request \
           JOIN linggan_model_invocation invocation USING(invocation_ref) \
           WHERE request.stage IN ('resolution','pair') \
             AND ($1 OR request.stage<>'pair') AND ( \
             (invocation.state='running' AND request.deadline_at<=scope_001_now()) OR \
             (invocation.state='failed' AND ( \
               (request.stage='resolution' AND EXISTS(SELECT 1 \
                 FROM linggan_comment_study_resolution resolution \
                 WHERE resolution.resolution_ref=request.resolution_ref \
                   AND resolution.state='pending' AND resolution.model_invocation_ref=request.invocation_ref)) OR \
               (request.stage='pair' AND EXISTS(SELECT 1 \
                 FROM linggan_comment_study_problem_pair pair \
                 WHERE pair.pair_ref=request.pair_ref \
                   AND pair.state='pending' AND pair.model_invocation_ref=request.invocation_ref))))) \
         ), ranked AS ( \
           SELECT eligible.*,row_number() OVER (PARTITION BY stage ORDER BY deadline_at,invocation_ref) AS stage_ordinal \
           FROM eligible \
         ) \
         SELECT run_ref,invocation_ref,stage,resolution_ref,pair_ref \
         FROM ranked WHERE stage_ordinal<=$2 ORDER BY deadline_at,invocation_ref",
    )
    .bind(pair_failure_state_supported)
    .bind(MAX_PROBLEM_STAGE_RECOVERIES_PER_TICK)
    .fetch_all(database.pool())
    .await?;
    let mut recovered = 0_u64;
    let mut recovered_resolution = 0_u64;
    let mut recovered_pair = 0_u64;
    for (run_ref, invocation_ref, stage, resolution_ref, pair_ref) in recoverable {
        let mut transaction = database.pool().begin().await?;
        let locked_run: Option<Uuid> = sqlx::query_scalar(
            "SELECT run_ref FROM linggan_comment_study_run \
             WHERE run_ref=$1 FOR UPDATE SKIP LOCKED",
        )
        .bind(run_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        if locked_run.is_none() {
            transaction.rollback().await?;
            continue;
        }
        let subject = match (stage.as_str(), resolution_ref, pair_ref) {
            ("resolution", Some(reference), None) => sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_comment_study_resolution \
                     WHERE resolution_ref=$1 AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|state| (ProblemStageSubject::Resolution(reference), state)),
            ("pair", None, Some(reference)) => sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_comment_study_problem_pair \
                     WHERE pair_ref=$1 AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(reference)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|state| (ProblemStageSubject::Pair(reference), state)),
            _ => None,
        };
        let Some((subject, subject_state)) = subject else {
            transaction.rollback().await?;
            continue;
        };
        let invocation = sqlx::query(
            "SELECT invocation.state,invocation.failure_code,invocation.reserved_tokens,invocation.charged_tokens, \
                    invocation.input_tokens,invocation.output_tokens,request.dispatch_started_at::text AS dispatch_started_at, \
                    request.attempt_ordinal,config.max_attempts,request.deadline_at<=scope_001_now() AS expired \
             FROM linggan_model_invocation invocation \
             JOIN linggan_comment_study_model_request request USING(invocation_ref) \
             JOIN linggan_model_config config ON config.config_ref=invocation.config_ref \
             WHERE invocation.invocation_ref=$1 AND request.run_ref=$2 \
               AND request.stage=$3 FOR UPDATE OF invocation",
        )
        .bind(invocation_ref)
        .bind(run_ref)
        .bind(subject.stage())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(invocation) = invocation else {
            transaction.rollback().await?;
            continue;
        };
        let invocation_state: String = invocation.get("state");
        if invocation_state == "failed" {
            if subject_state != "pending" {
                transaction.rollback().await?;
                continue;
            }
            let attempt: i32 = invocation.get("attempt_ordinal");
            let failure_code: String = invocation
                .get::<Option<String>, _>("failure_code")
                .unwrap_or_else(|| "provider_failed".to_owned());
            apply_problem_stage_failure(
                &mut transaction,
                subject,
                invocation_ref,
                attempt,
                invocation.get::<i32, _>("max_attempts"),
                &failure_code,
            )
            .await?;
            close_run_if_settled(&mut transaction, run_ref).await?;
            transaction.commit().await?;
            recovered = recovered.saturating_add(1);
            match stage.as_str() {
                "resolution" => recovered_resolution = recovered_resolution.saturating_add(1),
                "pair" => recovered_pair = recovered_pair.saturating_add(1),
                _ => {}
            }
            continue;
        }
        if invocation_state != "running" || !invocation.get::<bool, _>("expired") {
            transaction.rollback().await?;
            continue;
        }
        let dispatched = invocation
            .get::<Option<String>, _>("dispatch_started_at")
            .is_some();
        let reserved: i64 = invocation.get("reserved_tokens");
        let charged: Option<i64> = invocation.get("charged_tokens");
        let input_tokens: Option<i64> = invocation.get("input_tokens");
        let output_tokens: Option<i64> = invocation.get("output_tokens");
        let settled_charge = if !dispatched {
            0
        } else if let (Some(input), Some(output)) = (input_tokens, output_tokens) {
            input.saturating_add(output)
        } else {
            reserved.max(charged.unwrap_or(0))
        };
        let changed = sqlx::query(
            "UPDATE linggan_model_invocation SET state='failed',charged_tokens=$2, \
               failure_code='request_deadline_expired', \
               result=COALESCE(result,'{}'::jsonb)||jsonb_build_object( \
                   'ok',false,'failureCode','request_deadline_expired'), \
               finished_at=scope_001_now() \
             WHERE invocation_ref=$1 AND state='running'",
        )
        .bind(invocation_ref)
        .bind(settled_charge)
        .execute(&mut *transaction)
        .await?;
        if changed.rows_affected() != 1 {
            transaction.rollback().await?;
            continue;
        }
        let attempt: i32 = invocation.get("attempt_ordinal");
        if subject_state == "pending" {
            apply_problem_stage_failure(
                &mut transaction,
                subject,
                invocation_ref,
                attempt,
                invocation.get::<i32, _>("max_attempts"),
                "request_deadline_expired",
            )
            .await?;
        }
        close_run_if_settled(&mut transaction, run_ref).await?;
        transaction.commit().await?;
        recovered = recovered.saturating_add(1);
        match stage.as_str() {
            "resolution" => recovered_resolution = recovered_resolution.saturating_add(1),
            "pair" => recovered_pair = recovered_pair.saturating_add(1),
            _ => {}
        }
    }
    recovered = recovered.saturating_add(
        recover_pre_ledger_v2_problem_stage_invocations(
            database,
            pair_failure_state_supported,
            (MAX_PROBLEM_STAGE_RECOVERIES_PER_TICK as u64).saturating_sub(recovered_resolution)
                as i64,
            (MAX_PROBLEM_STAGE_RECOVERIES_PER_TICK as u64).saturating_sub(recovered_pair) as i64,
        )
        .await?,
    );
    Ok(recovered)
}

/// New P3 starts require migration 0111, but a partially applied sequence may have 0108's request
/// ledger without the terminal Pair state. Keep P3 work fenced until both Pair constraints exist.
pub(crate) async fn pair_failure_state_supported(
    database: &linggan_storage_postgres::Database,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('linggan_comment_study_model_request') IS NOT NULL \
           AND to_regclass('linggan_comment_study_problem_pair') IS NOT NULL \
           AND EXISTS(SELECT 1 FROM pg_trigger \
             WHERE tgrelid=to_regclass('linggan_comment_study_model_request') \
               AND tgname='cs_request_immutable' AND NOT tgisinternal) \
           AND EXISTS(SELECT 1 FROM pg_trigger \
             WHERE tgrelid=to_regclass('linggan_comment_study_model_request') \
               AND tgname='cs_request_no_truncate' AND NOT tgisinternal) \
           AND EXISTS(SELECT 1 FROM pg_constraint \
             WHERE conrelid=to_regclass('linggan_comment_study_problem_pair') \
               AND conname='cs_problem_pair_state_ck' AND contype='c' \
               AND pg_get_constraintdef(oid) LIKE '%failed%') \
           AND EXISTS(SELECT 1 FROM pg_constraint \
             WHERE conrelid=to_regclass('linggan_comment_study_problem_pair') \
               AND conname='cs_problem_pair_terminal_ck' AND contype='c' \
               AND pg_get_constraintdef(oid) LIKE '%resolved_at%')",
    )
    .fetch_one(database.pool())
    .await
}

/// Closes pre-ledger P3 claims left by the earlier v2 worker. Those receipts have no immutable
/// request snapshot or deadline, so pending subjects are terminalized instead of silently replayed
/// with an invented attempt ordinal. Running receipts are conservatively charged after the frozen
/// model timeout plus a recovery grace period.
async fn recover_pre_ledger_v2_problem_stage_invocations(
    database: &linggan_storage_postgres::Database,
    pair_failure_state_supported: bool,
    resolution_remaining: i64,
    pair_remaining: i64,
) -> Result<u64, sqlx::Error> {
    let orphans: Vec<(Uuid, String, Uuid, Uuid)> = sqlx::query_as(
        "WITH candidates AS ( \
           SELECT target.run_ref,'resolution'::text AS stage,resolution.resolution_ref AS subject_ref, \
                  invocation.invocation_ref \
           FROM linggan_comment_study_resolution resolution \
           JOIN linggan_comment_study_signal signal USING(signal_ref) \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_comment_study_run run ON run.run_ref=target.run_ref \
           JOIN linggan_model_invocation invocation ON invocation.invocation_ref=resolution.model_invocation_ref \
           LEFT JOIN linggan_model_config config ON config.config_ref=invocation.config_ref \
           WHERE run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
             AND (invocation.state='failed' OR (invocation.state='running' AND \
               invocation.created_at+make_interval(secs=>COALESCE(config.timeout_seconds,30)+30)<=scope_001_now())) \
             AND COALESCE(invocation.result->>'legacyRequestLedgerMissing','false')<>'true' \
             AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
               WHERE request.invocation_ref=invocation.invocation_ref) \
           UNION ALL \
           SELECT first_target.run_ref,'pair'::text,pair.pair_ref,invocation.invocation_ref \
           FROM linggan_comment_study_problem_pair pair \
           JOIN linggan_comment_study_signal first_signal ON first_signal.signal_ref=pair.first_signal_ref \
           JOIN linggan_comment_study_target first_target USING(target_ref) \
           JOIN linggan_comment_study_run run ON run.run_ref=first_target.run_ref \
           JOIN linggan_model_invocation invocation ON invocation.invocation_ref=pair.model_invocation_ref \
           LEFT JOIN linggan_model_config config ON config.config_ref=invocation.config_ref \
           WHERE $1 AND run.selection_manifest->>'contract'='comment-study.run-selection.v2' \
             AND (invocation.state='failed' OR (invocation.state='running' AND \
               invocation.created_at+make_interval(secs=>COALESCE(config.timeout_seconds,30)+30)<=scope_001_now())) \
             AND COALESCE(invocation.result->>'legacyRequestLedgerMissing','false')<>'true' \
             AND NOT EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
               WHERE request.invocation_ref=invocation.invocation_ref) \
         ), ranked AS ( \
           SELECT candidates.*,row_number() OVER (PARTITION BY stage ORDER BY run_ref,subject_ref,invocation_ref) AS stage_ordinal \
           FROM candidates \
         ) \
         SELECT run_ref,stage,subject_ref,invocation_ref FROM ranked \
         WHERE stage_ordinal<=CASE stage WHEN 'resolution' THEN $2 ELSE $3 END \
         ORDER BY run_ref,stage,subject_ref,invocation_ref",
    )
    .bind(pair_failure_state_supported)
    .bind(resolution_remaining)
    .bind(pair_remaining)
    .fetch_all(database.pool())
    .await?;
    let mut recovered = 0_u64;
    for (run_ref, stage, subject_ref, invocation_ref) in orphans {
        let mut transaction = database.pool().begin().await?;
        let locked_run: Option<Uuid> = sqlx::query_scalar(
            "SELECT run_ref FROM linggan_comment_study_run \
             WHERE run_ref=$1 AND selection_manifest->>'contract'='comment-study.run-selection.v2' \
             FOR UPDATE SKIP LOCKED",
        )
        .bind(run_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        if locked_run.is_none() {
            transaction.rollback().await?;
            continue;
        }
        let subject_record = match stage.as_str() {
            "resolution" => sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_comment_study_resolution \
                 WHERE resolution_ref=$1 AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(subject_ref)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|state| (ProblemStageSubject::Resolution(subject_ref), state)),
            "pair" => sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_comment_study_problem_pair \
                 WHERE pair_ref=$1 AND model_invocation_ref=$2 FOR UPDATE",
            )
            .bind(subject_ref)
            .bind(invocation_ref)
            .fetch_optional(&mut *transaction)
            .await?
            .map(|state| (ProblemStageSubject::Pair(subject_ref), state)),
            _ => None,
        };
        let Some((subject, subject_state)) = subject_record else {
            transaction.rollback().await?;
            continue;
        };
        let invocation = sqlx::query(
            "SELECT invocation.state,invocation.reserved_tokens,invocation.charged_tokens,config.max_attempts, \
                    invocation.input_tokens,invocation.output_tokens,invocation.failure_code, \
                    invocation.created_at+make_interval(secs=>COALESCE(config.timeout_seconds,30)+30) \
                      <=scope_001_now() AS recovery_elapsed, \
                    EXISTS(SELECT 1 FROM linggan_comment_study_model_request request \
                      WHERE request.invocation_ref=invocation.invocation_ref) AS has_request \
             FROM linggan_model_invocation invocation \
             LEFT JOIN linggan_model_config config ON config.config_ref=invocation.config_ref \
             WHERE invocation.invocation_ref=$1 FOR UPDATE OF invocation",
        )
        .bind(invocation_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(invocation) = invocation else {
            transaction.rollback().await?;
            continue;
        };
        if invocation.get::<bool, _>("has_request") {
            transaction.rollback().await?;
            continue;
        }
        let invocation_state: String = invocation.get("state");
        if invocation_state != "failed"
            && (invocation_state != "running" || !invocation.get::<bool, _>("recovery_elapsed"))
        {
            transaction.rollback().await?;
            continue;
        }
        let reserved: i64 = invocation.get("reserved_tokens");
        let charged: i64 = invocation.get("charged_tokens");
        let input: Option<i64> = invocation.get("input_tokens");
        let output: Option<i64> = invocation.get("output_tokens");
        let settled_charge = input
            .zip(output)
            .map(|(input, output)| input.saturating_add(output))
            .unwrap_or_else(|| reserved.max(charged));
        let failure_code: String = invocation
            .get::<Option<String>, _>("failure_code")
            .unwrap_or_else(|| "legacy_request_ledger_missing".to_owned());
        sqlx::query(
            "UPDATE linggan_model_invocation SET state='failed',charged_tokens=$2, \
               failure_code=COALESCE(failure_code,'legacy_request_ledger_missing'), \
               result=COALESCE(result,'{}'::jsonb)||jsonb_build_object( \
                 'ok',false,'failureCode',$3,'legacyRequestLedgerMissing',true, \
                 'legacyRunRef',$4::text,'legacyStage',$5,'legacySubjectRef',$6::text), \
               finished_at=COALESCE(finished_at,scope_001_now()) \
             WHERE invocation_ref=$1 AND state IN ('running','failed')",
        )
        .bind(invocation_ref)
        .bind(settled_charge)
        .bind(&failure_code)
        .bind(run_ref)
        .bind(&stage)
        .bind(subject_ref)
        .execute(&mut *transaction)
        .await?;
        if subject_state == "pending" {
            match subject {
                ProblemStageSubject::Resolution(reference) => {
                    sqlx::query(
                        "UPDATE linggan_comment_study_resolution SET state='failed', \
                           decision_manifest=jsonb_build_object( \
                             'reason','legacy_request_ledger_missing','lastFailureCode',$2), \
                           resolved_at=scope_001_now() \
                         WHERE resolution_ref=$1 AND state='pending' AND model_invocation_ref=$3",
                    )
                    .bind(reference)
                    .bind(&failure_code)
                    .bind(invocation_ref)
                    .execute(&mut *transaction)
                    .await?;
                }
                ProblemStageSubject::Pair(reference) => {
                    sqlx::query(
                        "UPDATE linggan_comment_study_problem_pair SET state='failed', \
                           resolved_at=scope_001_now(),pair_manifest=jsonb_set(pair_manifest,'{decision}', \
                             jsonb_build_object('code','legacy_request_ledger_missing', \
                               'lastFailureCode',$2),true) \
                         WHERE pair_ref=$1 AND state='pending' AND model_invocation_ref=$3",
                    )
                    .bind(reference)
                    .bind(&failure_code)
                    .bind(invocation_ref)
                    .execute(&mut *transaction)
                    .await?;
                }
            }
        }
        close_run_if_settled(&mut transaction, run_ref).await?;
        transaction.commit().await?;
        recovered = recovered.saturating_add(1);
    }
    Ok(recovered)
}
