//! Bounded semantic naming for a persisted cluster candidate.
//!
//! The model may say `same` or `independent`; it cannot merge external Topics,
//! browse data, or overwrite an existing group. Every accepted decision is an
//! append-only group/membership/lineage record tied to this review attempt.

use super::decision::{
    ReviewDecision, ReviewExpression, ReviewSampleRoles, review_prompt, review_system, sample_roles,
};
use super::reconciliation::{reconcile_ready_run, reconcile_run_if_complete};
use crate::{
    comment_execution_budget::{BudgetPurpose, check_budget_in},
    comment_research::comment_source_hash,
    model_invocation::{checkpoint_invocation_usage, connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::{PiAdapter, PiResponse},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

const REVIEW_INPUT_MAX: i32 = 8000;
const REVIEW_OUTPUT_MAX: i32 = 2000;
const REVIEW_CALLS_PER_DAY_MAX: i64 = 60;

pub(crate) async fn run_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if draining(drain) {
        return Ok(false);
    }
    recover(db).await?;
    let Some(job) = reserve(db, drain).await? else {
        return reconcile_ready_run(db).await;
    };
    if draining(drain) {
        finish_without_call(db, &job, "worker_draining").await?;
        return Ok(false);
    }
    let mut called = false;
    let outcome = async {
        let mut request = connection_request(db, store, job.connection_version_ref).await?;
        request.operation = "analyze".into();
        request.model_id = job.model_id.clone();
        request.timeout_ms = job.timeout_seconds as u64 * 1000;
        request.max_output_tokens = job.output_limit;
        request.system = review_system().into();
        request.prompt = job.prompt.clone();
        if draining(drain) {
            return Err(ModelError::Disabled);
        }
        let allowed: bool = sqlx::query_scalar(
            "SELECT cfg.config_ref=$1 AND connection.enabled \
             FROM linggan_model_config cfg JOIN linggan_model_entry model USING(model_ref) \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             JOIN linggan_model_connection connection USING(connection_ref) \
             WHERE cfg.config_ref=$1",
        )
        .bind(job.config_ref)
        .fetch_optional(db.pool())
        .await?
        .unwrap_or(false);
        if !allowed {
            return Err(ModelError::Disabled);
        }
        called = true;
        adapter.call(&request).await
    }
    .await;
    let response = outcome.as_ref().ok();
    if called {
        checkpoint_invocation_usage(db, job.invocation_ref, response).await?;
    }
    finish(
        db,
        &job,
        response,
        outcome.as_ref().err().map(ModelError::code),
        called,
    )
    .await?;
    Ok(true)
}

#[derive(Debug)]
struct ReviewJob {
    review_ref: Uuid,
    run_ref: Uuid,
    cluster_key: String,
    algorithm: String,
    space_ref: Uuid,
    run_snapshot_hash: String,
    sample_refs: Vec<Uuid>,
    sample_roles: ReviewSampleRoles,
    config_ref: Uuid,
    connection_version_ref: Uuid,
    model_id: String,
    input_limit: i32,
    output_limit: i32,
    timeout_seconds: i32,
    invocation_ref: Uuid,
    prompt: String,
}

struct PendingReview {
    review_ref: Uuid,
    run_ref: Uuid,
    cluster_key: String,
    algorithm: String,
    kind: String,
    space_ref: Uuid,
    run_snapshot_hash: String,
    sample_refs: Vec<Uuid>,
    receipt: Value,
    config_ref: Uuid,
    connection_version_ref: Uuid,
    model_ref: Uuid,
    model_id: String,
    configured_input_limit: i32,
    configured_output_limit: i32,
    timeout_seconds: i32,
}

async fn reserve(
    db: &Database,
    drain: Option<&ModelWorkerDrain>,
) -> Result<Option<ReviewJob>, ModelError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *transaction)
        .await?;
    if daily_limit_reached(&mut transaction).await? || draining(drain) {
        transaction.rollback().await?;
        return Ok(None);
    }
    let Some(pending) = select_pending_review(&mut transaction).await? else {
        transaction.rollback().await?;
        return Ok(None);
    };
    if pending.sample_refs.is_empty() || pending.sample_refs.len() > 12 {
        transaction.rollback().await?;
        return Err(ModelError::InvalidOutput);
    }
    let Some(sample_roles) = sample_roles(&pending.receipt, &pending.sample_refs) else {
        mark_invalid_sample_roles(&mut transaction, &pending).await?;
        transaction.commit().await?;
        return Ok(None);
    };
    let expressions =
        current_expressions(&mut transaction, &pending.sample_refs, &pending.kind).await?;
    if expressions.len() != pending.sample_refs.len() {
        sqlx::query(
            "UPDATE linggan_ci_cluster_review SET state='independent',receipt=receipt||$2 WHERE review_ref=$1",
        )
        .bind(pending.review_ref)
        .bind(json!({"reason":"current_atom_withdrawn"}))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        return Ok(None);
    }
    let input_limit = pending.configured_input_limit.min(REVIEW_INPUT_MAX);
    let output_limit = pending.configured_output_limit.min(REVIEW_OUTPUT_MAX);
    if input_limit < 1024 || output_limit < 128 {
        transaction.rollback().await?;
        return Err(ModelError::NotQualified);
    }
    let prompt = review_prompt(
        pending.kind.clone(),
        pending.algorithm.clone(),
        pending.cluster_key.clone(),
        &expressions,
        &sample_roles,
    );
    if prompt.len() + 512 > input_limit as usize {
        transaction.rollback().await?;
        return Err(ModelError::InputLimit);
    }
    let Some(invocation_ref) = reserve_review_invocation(
        &mut transaction,
        &pending,
        &prompt,
        input_limit,
        output_limit,
        drain,
    )
    .await?
    else {
        transaction.rollback().await?;
        return Ok(None);
    };
    transaction.commit().await?;
    Ok(Some(ReviewJob {
        review_ref: pending.review_ref,
        run_ref: pending.run_ref,
        cluster_key: pending.cluster_key,
        algorithm: pending.algorithm,
        space_ref: pending.space_ref,
        run_snapshot_hash: pending.run_snapshot_hash,
        sample_refs: pending.sample_refs,
        sample_roles,
        config_ref: pending.config_ref,
        connection_version_ref: pending.connection_version_ref,
        model_id: pending.model_id,
        input_limit,
        output_limit,
        timeout_seconds: pending.timeout_seconds,
        invocation_ref,
        prompt,
    }))
}

async fn mark_invalid_sample_roles(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pending: &PendingReview,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_ci_cluster_review SET state='insufficient',lease_until=NULL,receipt=receipt||$2 \
         WHERE review_ref=$1 AND state='pending'",
    )
    .bind(pending.review_ref)
    .bind(json!({"failureCode":"invalid_sample_roles"}))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn daily_limit_reached(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, ModelError> {
    let daily_calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation \
         WHERE operation='analyze' AND result->>'task'='semantic_cluster_review' \
         AND created_at >= date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai' \
         AND created_at < (date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') + interval '1 day') AT TIME ZONE 'Asia/Shanghai'",
    )
    .fetch_one(&mut **transaction)
    .await?;
    Ok(daily_calls >= REVIEW_CALLS_PER_DAY_MAX)
}

async fn select_pending_review(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<PendingReview>, ModelError> {
    let row = sqlx::query(
        "SELECT review.review_ref,review.run_ref,review.cluster_key,review.algorithm,review.sample_refs,review.receipt,run.kind,run.space_ref,run.snapshot_hash, \
                config.config_ref,version.version_ref,model.model_ref,model.model_id,config.input_token_limit,config.output_token_limit,config.timeout_seconds \
         FROM linggan_ci_cluster_review review \
         JOIN linggan_ci_cluster_run run ON run.run_ref=review.run_ref AND run.state='reviewing' \
         JOIN linggan_model_workspace workspace ON workspace.singleton \
         JOIN linggan_model_config config ON config.config_ref=workspace.default_config_ref \
         JOIN linggan_model_entry model ON model.model_ref=config.model_ref \
         JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE review.state='pending' AND (review.next_attempt_at IS NULL OR review.next_attempt_at<=scope_001_now()) \
           AND connection.enabled \
         ORDER BY review.created_at,review.review_ref FOR UPDATE OF review SKIP LOCKED LIMIT 1",
    )
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(row.map(|row| PendingReview {
        review_ref: row.get("review_ref"),
        run_ref: row.get("run_ref"),
        cluster_key: row.get("cluster_key"),
        algorithm: row.get("algorithm"),
        kind: row.get("kind"),
        space_ref: row.get("space_ref"),
        run_snapshot_hash: row.get("snapshot_hash"),
        sample_refs: row.get("sample_refs"),
        receipt: row.get("receipt"),
        config_ref: row.get("config_ref"),
        connection_version_ref: row.get("version_ref"),
        model_ref: row.get("model_ref"),
        model_id: row.get("model_id"),
        configured_input_limit: row.get("input_token_limit"),
        configured_output_limit: row.get("output_token_limit"),
        timeout_seconds: row.get("timeout_seconds"),
    }))
}

async fn current_expressions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    sample_refs: &[Uuid],
    kind: &str,
) -> Result<Vec<ReviewExpression>, ModelError> {
    let mut rows = sqlx::query(
        "SELECT atom_ref,meaning,target,position FROM linggan_ci_semantic_atom_current WHERE atom_ref=ANY($1) ORDER BY atom_ref",
    )
    .bind(sample_refs)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .map(|row| {
        (
            row.get::<Uuid, _>("atom_ref"),
            ReviewExpression {
                atom_ref: row.get("atom_ref"),
                meaning: row.get("meaning"),
                target: row.get("target"),
                position: row.get("position"),
            },
        )
    })
    .collect::<std::collections::BTreeMap<_, _>>();
    let expressions = sample_refs
        .iter()
        .filter_map(|atom_ref| rows.remove(atom_ref))
        .collect::<Vec<_>>();
    if expressions.iter().any(|expression| {
        if kind == "stance" {
            expression.target.as_deref().is_none_or(str::is_empty)
                || expression.position.as_deref().is_none_or(str::is_empty)
        } else {
            expression.target.is_some() || expression.position.is_some()
        }
    }) {
        return Err(ModelError::InvalidOutput);
    }
    Ok(expressions)
}

async fn reserve_review_invocation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pending: &PendingReview,
    prompt: &str,
    input_limit: i32,
    output_limit: i32,
    drain: Option<&ModelWorkerDrain>,
) -> Result<Option<Uuid>, ModelError> {
    let reserved_tokens = i64::from(input_limit + output_limit);
    let budget =
        check_budget_in(transaction, BudgetPurpose::Semantic, reserved_tokens, false).await?;
    if !budget.allowed || draining(drain) {
        return Ok(None);
    }
    let invocation_ref = Uuid::new_v4();
    let mut metadata = budget.ledger_metadata;
    metadata["task"] = json!("semantic_cluster_review");
    metadata["reviewRef"] = json!(pending.review_ref);
    metadata["runRef"] = json!(pending.run_ref);
    sqlx::query(
        "INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) \
         VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)",
    )
    .bind(invocation_ref)
    .bind(pending.connection_version_ref)
    .bind(pending.model_ref)
    .bind(pending.config_ref)
    .bind(comment_source_hash(prompt))
    .bind(reserved_tokens)
    .bind(metadata)
    .execute(&mut **transaction)
    .await?;
    let updated = sqlx::query(
        "UPDATE linggan_ci_cluster_review SET state='running',attempts=attempts+1,invocation_ref=$2,lease_until=scope_001_now()+interval '76 seconds' \
         WHERE review_ref=$1 AND state='pending'",
    )
    .bind(pending.review_ref)
    .bind(invocation_ref)
    .execute(&mut **transaction)
    .await?
    .rows_affected();
    if updated != 1 || draining(drain) {
        return Ok(None);
    }
    Ok(Some(invocation_ref))
}

fn draining(drain: Option<&ModelWorkerDrain>) -> bool {
    drain.is_some_and(ModelWorkerDrain::is_requested)
}

async fn finish_without_call(db: &Database, job: &ReviewJob, code: &str) -> Result<(), ModelError> {
    let mut transaction = db.pool().begin().await?;
    sqlx::query(
        "UPDATE linggan_ci_cluster_review SET state='pending',attempts=GREATEST(attempts-1,0),next_attempt_at=NULL,lease_until=NULL,receipt=receipt||$2 \
         WHERE review_ref=$1 AND state='running'",
    )
    .bind(job.review_ref)
    .bind(json!({"failureCode":code,"callStarted":false}))
    .execute(&mut *transaction)
    .await?;
    finish_invocation_in(
        &mut transaction,
        job.invocation_ref,
        None,
        false,
        Some(code),
        &json!({"task":"semantic_cluster_review","callStarted":false,"usageUnknown":false}),
    )
    .await?;
    sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=0,input_tokens=0,output_tokens=0 WHERE invocation_ref=$1")
        .bind(job.invocation_ref).execute(&mut *transaction).await?;
    transaction.commit().await?;
    Ok(())
}

async fn finish(
    db: &Database,
    job: &ReviewJob,
    response: Option<&PiResponse>,
    error: Option<&str>,
    called: bool,
) -> Result<(), ModelError> {
    let parsed = response
        .filter(|response| response.ok)
        .and_then(|response| response.text.as_deref())
        .and_then(|text| serde_json::from_str::<ReviewDecision>(text).ok());
    let decision = parsed
        .as_ref()
        .filter(|decision| valid_decision(decision, job));
    let failure = review_failure(response, error, decision, job);
    let mut transaction = db.pool().begin().await?;
    let valid: bool = sqlx::query_scalar(
        "SELECT review.state='running' AND review.invocation_ref=$2 AND review.lease_until>scope_001_now() \
         AND run.state='reviewing' AND run.space_ref=$3 AND run.snapshot_hash=$4 \
         FROM linggan_ci_cluster_review review \
         JOIN linggan_ci_cluster_run run ON run.run_ref=review.run_ref \
         WHERE review.review_ref=$1 FOR UPDATE OF review,run",
    )
    .bind(job.review_ref)
    .bind(job.invocation_ref)
    .bind(job.space_ref)
    .bind(&job.run_snapshot_hash)
    .fetch_optional(&mut *transaction)
    .await?
    .unwrap_or(false);
    if !valid {
        return Err(ModelError::Conflict);
    }
    let invocation_succeeded = settle_review(
        &mut transaction,
        job,
        response,
        called,
        decision,
        failure.as_deref(),
    )
    .await?;
    reconcile_run_if_complete(&mut transaction, job.run_ref).await?;
    finish_invocation_in(
        &mut transaction,
        job.invocation_ref,
        response,
        invocation_succeeded,
        failure.as_deref(),
        &json!({"task":"semantic_cluster_review","callStarted":called,"usageUnknown":called&&response.is_none()}),
    )
    .await?;
    if !called {
        sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=0,input_tokens=0,output_tokens=0 WHERE invocation_ref=$1")
            .bind(job.invocation_ref).execute(&mut *transaction).await?;
    }
    transaction.commit().await?;
    Ok(())
}

async fn settle_review(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ReviewJob,
    response: Option<&PiResponse>,
    called: bool,
    decision: Option<&ReviewDecision>,
    failure: Option<&str>,
) -> Result<bool, ModelError> {
    let mut receipt = json!({
        "callStarted":called,
        "response":response.map(crate::pi_adapter::safe_result),
        "decision":decision,
        "sampleRefs":job.sample_refs,
        "sampleRoles":job.sample_roles
    });
    let current_cluster =
        if failure.is_none() && decision.is_some_and(|decision| decision.decision == "same") {
            cluster_is_current(transaction, job).await?
        } else {
            true
        };
    let admissible = failure.is_none() && decision.is_some() && current_cluster;
    // `coreAtomRefs` are bounded review witnesses, not the full cluster. A
    // successful same decision therefore applies to the quality-qualified
    // algorithm cluster, except explicit `relatedAtomRefs`; those remain a
    // visible, non-same relation. The receipt makes that distinction durable.
    if let Some(decision) = decision.filter(|_| failure.is_none()) {
        let review_state = if decision.decision == "same" && !admissible {
            receipt["adoption"] = json!("preserved_independent_current_input_changed");
            receipt["failureCode"] = json!("current_input_changed");
            "independent"
        } else if decision.decision == "same" {
            let same_scope = decision.same_scope.as_deref().unwrap_or("cluster");
            receipt["adoptionScope"] = json!({
                "sameMembership":if same_scope=="core" {"reviewed_core_only_boundary_not_confirmed"} else {"whole_quality_passing_algorithm_cluster_except_explicit_related"},
                "sampleReview":"bounded_representatives_not_per_member_llm_verification",
                "sameScope":same_scope,
                "relatedAtomRefs":decision.related_atom_refs
            });
            "accepted"
        } else {
            "independent"
        };
        sqlx::query(
            "UPDATE linggan_ci_cluster_review SET state=$2,lease_until=NULL,receipt=$3,next_attempt_at=NULL WHERE review_ref=$1",
        )
        .bind(job.review_ref)
        .bind(review_state)
        .bind(&receipt)
        .execute(&mut **transaction)
        .await?;
    } else {
        let terminal = failure.unwrap_or("invalid_semantic_review_output");
        sqlx::query(
            "UPDATE linggan_ci_cluster_review SET state=CASE WHEN attempts>=2 THEN 'failed' ELSE 'pending' END, \
             next_attempt_at=CASE WHEN attempts>=2 THEN NULL ELSE scope_001_now()+interval '24 hours' END,lease_until=NULL,receipt=receipt||$2 WHERE review_ref=$1",
        )
        .bind(job.review_ref)
        .bind(json!({"failureCode":terminal,"callStarted":called,"response":response.map(crate::pi_adapter::safe_result),"sampleRoles":job.sample_roles}))
        .execute(&mut **transaction)
        .await?;
    }
    // A current-input fence can preserve the atoms as independent after a
    // valid provider reply. That is an adoption decision, not unknown cost.
    Ok(failure.is_none() && decision.is_some())
}

async fn cluster_is_current(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    job: &ReviewJob,
) -> Result<bool, ModelError> {
    cluster_is_current_for(
        transaction,
        job.run_ref,
        &job.algorithm,
        &job.cluster_key,
        job.space_ref,
    )
    .await
}

pub(super) async fn cluster_is_current_for(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    algorithm: &str,
    cluster_key: &str,
    space_ref: Uuid,
) -> Result<bool, ModelError> {
    sqlx::query_scalar(
        "WITH cluster AS ( \
           SELECT input.atom_ref,input.definition_hash \
           FROM linggan_ci_cluster_assignment assignment \
           JOIN linggan_ci_cluster_input input ON input.run_ref=assignment.run_ref AND input.atom_ref=assignment.atom_ref \
           WHERE assignment.run_ref=$1 AND assignment.algorithm=$2 AND assignment.cluster_key=$3 \
         ), active_space AS ( \
           SELECT EXISTS(SELECT 1 \
             FROM linggan_embedding_settings setting \
             JOIN linggan_embedding_config config USING(config_ref) \
             JOIN linggan_model_entry model USING(model_ref) \
             JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
             JOIN linggan_model_connection connection USING(connection_ref) \
             JOIN linggan_ci_semantic_space space ON space.space_ref=$4 \
             WHERE setting.singleton AND config.qualified AND config.enabled AND connection.enabled \
               AND space.normalization='l2' AND space.dimensions=config.dimensions \
               AND space.provider_api=version.api AND space.endpoint=version.base_url \
               AND space.model_id=model.model_id) AS active \
         ) \
         SELECT (SELECT active FROM active_space) AND count(*)>0 \
           AND count(*)=count(current_atom.atom_ref) AND count(*)=count(vector.definition_hash) \
         FROM cluster \
         LEFT JOIN linggan_ci_semantic_atom_current current_atom \
           ON current_atom.atom_ref=cluster.atom_ref AND current_atom.definition_hash=cluster.definition_hash \
         LEFT JOIN linggan_ci_atom_vector vector \
           ON vector.space_ref=$4 AND vector.definition_hash=cluster.definition_hash",
    )
    .bind(run_ref)
    .bind(algorithm)
    .bind(cluster_key)
    .bind(space_ref)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ModelError::from)
}

fn review_failure(
    response: Option<&PiResponse>,
    error: Option<&str>,
    decision: Option<&ReviewDecision>,
    job: &ReviewJob,
) -> Option<String> {
    let malformed = decision
        .is_none()
        .then(|| "invalid_semantic_review_output".to_owned());
    let provider_failure = response
        .filter(|response| !response.ok)
        .and_then(|response| response.failure_code.clone());
    let over_budget = response
        .filter(|response| {
            response
                .usage
                .input_tokens
                .is_some_and(|tokens| tokens > i64::from(job.input_limit))
                || response
                    .usage
                    .output_tokens
                    .is_some_and(|tokens| tokens > i64::from(job.output_limit))
        })
        .map(|_| "model_budget_overrun".to_owned());
    over_budget
        .or_else(|| error.map(str::to_owned))
        .or(provider_failure)
        .or(malformed)
}

fn valid_decision(decision: &ReviewDecision, job: &ReviewJob) -> bool {
    if decision.decision == "independent" {
        return decision.name.is_none()
            && decision.definition.is_none()
            && decision.same_scope.is_none()
            && decision.core_atom_refs.is_empty()
            && decision.related_atom_refs.is_empty();
    }
    let Some(name) = decision.name.as_deref() else {
        return false;
    };
    let Some(definition) = decision.definition.as_deref() else {
        return false;
    };
    let allowed = job
        .sample_refs
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let core_allowed = job
        .sample_roles
        .core_atom_refs
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let related_allowed = job
        .sample_roles
        .core_atom_refs
        .iter()
        .chain(&job.sample_roles.boundary_atom_refs)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let references = decision
        .core_atom_refs
        .iter()
        .chain(&decision.related_atom_refs)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    decision.decision == "same"
        && (1..=120).contains(&name.chars().count())
        && (1..=1000).contains(&definition.chars().count())
        && (1..=5).contains(&decision.core_atom_refs.len())
        && decision.related_atom_refs.len() <= 3
        && decision
            .same_scope
            .as_deref()
            .is_none_or(|scope| matches!(scope, "cluster" | "core"))
        && references.len() == decision.core_atom_refs.len() + decision.related_atom_refs.len()
        && references.is_subset(&allowed)
        && decision
            .core_atom_refs
            .iter()
            .all(|atom_ref| core_allowed.contains(atom_ref))
        && decision
            .related_atom_refs
            .iter()
            .all(|atom_ref| related_allowed.contains(atom_ref))
}

#[derive(Clone)]
pub(super) struct ExistingGroup {
    pub(super) group_ref: Uuid,
    pub(super) problem_ref: Option<Uuid>,
    pub(super) name: String,
    pub(super) definition: String,
    pub(super) current_same_atoms: BTreeSet<Uuid>,
}

pub(super) struct AcceptedCandidate {
    pub(super) review_ref: Uuid,
    pub(super) invocation_ref: Uuid,
    pub(super) cluster_key: String,
    pub(super) algorithm: String,
    pub(super) name: String,
    pub(super) definition: String,
    pub(super) same_scope: String,
    pub(super) same_atoms: BTreeSet<Uuid>,
    pub(super) related_atoms: BTreeSet<Uuid>,
}

pub(super) struct MaterializedGroup {
    pub(super) candidate_index: usize,
    pub(super) group_ref: Uuid,
    pub(super) problem_ref: Option<Uuid>,
    pub(super) reused: bool,
    pub(super) lineage_kind: &'static str,
    pub(super) lineage_reason: &'static str,
}

async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut transaction = db.pool().begin().await?;
    let invocations: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE linggan_ci_cluster_review SET state=CASE WHEN attempts>=2 THEN 'failed' ELSE 'pending' END, \
         next_attempt_at=CASE WHEN attempts>=2 THEN NULL ELSE scope_001_now()+interval '24 hours' END,lease_until=NULL,receipt=receipt||jsonb_build_object('recovered',true,'failureCode','worker_interrupted') \
         WHERE state='running' AND lease_until<=scope_001_now() RETURNING invocation_ref",
    )
    .fetch_all(&mut *transaction)
    .await?;
    sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(),result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('callStarted',true,'usageUnknown',input_tokens IS NULL OR output_tokens IS NULL) WHERE invocation_ref=ANY($1) AND state='running'")
        .bind(invocations).execute(&mut *transaction).await?;
    transaction.commit().await?;
    Ok(())
}
