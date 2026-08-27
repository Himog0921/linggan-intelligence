//! COLLECTION-001 · Persisting the four-stage acquisition chain.
//!
//! Request → Authorization → Admission → Work Order. Each stage is written separately so
//! that no earlier stage can be read as a later one having succeeded (INV-36).
//!
//! Nothing here reaches a platform. A Work Order row is a written instruction; execution is
//! a later stage that does not exist yet.

use linggan_contracts::{AdmissionFacts, AdmissionOutcome, decide_admission};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AcquisitionChainError {
    #[error("acquisition chain schema is not applied")]
    SchemaUnavailable,
    #[error("no observation target with that reference")]
    UnknownTarget,
    #[error("that target is not in a state where deep archiving can be requested: {state}")]
    TargetNotRequestable { state: String },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What a request produced, all the way through admission.
#[derive(Debug)]
pub struct RequestOutcome {
    pub request_ref: Uuid,
    pub decision_ref: Uuid,
    pub outcome: AdmissionOutcome,
    /// Present only when the decision admitted the request.
    pub work_order_ref: Option<Uuid>,
}

pub async fn acquisition_chain_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_acquisition_authorization') IS NOT NULL \
             AND to_regclass('collection_acquisition_request') IS NOT NULL \
             AND to_regclass('collection_admission_decision') IS NOT NULL \
             AND to_regclass('collection_work_order') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// Grant an acquisition authorization. Only a person may do this.
///
/// The grant covers a *class* of targets. Asking a person to approve every individual
/// creator would turn the control into a rubber stamp — the bounds are what keep a class
/// grant meaningful.
#[derive(Debug, Clone)]
pub struct AuthorizationGrant<'a> {
    pub platform: &'a str,
    pub target_kind: &'a str,
    pub lane: &'a str,
    /// Why this is being granted. Without it, admission cannot later check whether a request
    /// is still inside the approved purpose.
    pub purpose: &'a str,
    pub max_targets: Option<i32>,
    pub max_works_per_target: Option<i32>,
    /// Expiry is required, not optional: an unbounded-in-time grant is indistinguishable
    /// from no control at all.
    pub valid_for_days: i32,
}

pub async fn grant_authorization(
    database: &Database,
    grant: &AuthorizationGrant<'_>,
) -> Result<Uuid, AcquisitionChainError> {
    if !acquisition_chain_schema_is_ready(database).await? {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, purpose, max_targets, \
              max_works_per_target, granted_by, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'person', scope_001_now() + make_interval(days => $8))",
    )
    .bind(authorization_ref)
    .bind(grant.platform)
    .bind(grant.target_kind)
    .bind(grant.lane)
    .bind(grant.purpose)
    .bind(grant.max_targets)
    .bind(grant.max_works_per_target)
    .bind(grant.valid_for_days)
    .execute(database.pool())
    .await?;
    Ok(authorization_ref)
}

/// Request an acquisition and run it through admission in one transaction.
///
/// The request is recorded **whatever admission concludes** — including refusal. A refused
/// request that leaves no trace cannot later answer "why didn't this ever run".
pub async fn request_and_admit(
    database: &Database,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
) -> Result<RequestOutcome, AcquisitionChainError> {
    if !acquisition_chain_schema_is_ready(database).await? {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    let target: Option<(String, String, String)> = sqlx::query_as(
        "SELECT platform, target_kind, lifecycle_state \
         FROM collection_observation_target WHERE target_ref = $1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((platform, target_kind, lifecycle_state)) = target else {
        return Err(AcquisitionChainError::UnknownTarget);
    };
    // Only a pending target may be requested for archiving. Anything else means the caller
    // is acting on a stale view of the target.
    if lifecycle_state != "pending_decision" {
        return Err(AcquisitionChainError::TargetNotRequestable {
            state: lifecycle_state,
        });
    }

    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref, target_ref, lane, purpose, requested_by) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(request_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(purpose)
    .bind(requested_by)
    .execute(&mut *transaction)
    .await?;

    let facts = gather_facts(&mut transaction, &platform, &target_kind, lane, target_ref).await?;
    let outcome = decide_admission(&facts);

    let decision_ref = Uuid::new_v4();
    let authorization_ref = match &outcome {
        AdmissionOutcome::Admitted { authorization_ref } => Uuid::parse_str(authorization_ref).ok(),
        _ => None,
    };
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref, request_ref, outcome, unanswered_question, reason_code, reason, \
              authorization_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(outcome.code())
    .bind(outcome.unanswered_question().map(|q| q.number()))
    .bind(reason_code(&outcome))
    .bind(reason_text(&outcome))
    .bind(authorization_ref)
    .execute(&mut *transaction)
    .await?;

    let work_order_ref = if outcome.permits_work_order() {
        Some(
            write_work_order(
                &mut transaction,
                decision_ref,
                target_ref,
                lane,
                authorization_ref,
            )
            .await?,
        )
    } else {
        None
    };

    transaction.commit().await?;
    Ok(RequestOutcome {
        request_ref,
        decision_ref,
        outcome,
        work_order_ref,
    })
}

/// Collect only facts the server can actually establish.
async fn gather_facts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    lane: &str,
    target_ref: Uuid,
) -> Result<AdmissionFacts, sqlx::Error> {
    let authorization_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT authorization_ref FROM collection_acquisition_authorization \
         WHERE platform = $1 AND target_kind = $2 AND lane = $3 \
           AND revoked_at IS NULL AND expires_at > scope_001_now() \
         ORDER BY expires_at DESC LIMIT 1",
    )
    .bind(platform)
    .bind(target_kind)
    .bind(lane)
    .fetch_optional(&mut **transaction)
    .await?;

    let in_flight: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_work_order \
                        WHERE target_ref = $1 AND lane = $2)",
    )
    .bind(target_ref)
    .bind(lane)
    .fetch_one(&mut **transaction)
    .await?;

    Ok(AdmissionFacts {
        authorization_ref: authorization_ref.map(|value| value.to_string()),
        in_flight_work_exists: in_flight,
        // No archive exists yet, so no need can already be satisfied. This becomes a real
        // query once archiving produces results.
        need_already_satisfied: false,
        // Question 5 cannot be answered while no worker, account or budget model exists. The
        // contract forbids answering it by assuming capacity, so the honest value is false.
        // This flips to a real check when workers become an object — not before.
        capacity_is_establishable: false,
        stop_conditions_expressible: true,
    })
}

async fn max_works_for(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    authorization_ref: Option<Uuid>,
) -> Result<i32, sqlx::Error> {
    let Some(authorization_ref) = authorization_ref else {
        return Ok(DEFAULT_MAX_WORKS);
    };
    let bound: Option<i32> = sqlx::query_scalar(
        "SELECT max_works_per_target FROM collection_acquisition_authorization \
         WHERE authorization_ref = $1",
    )
    .bind(authorization_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .flatten();
    Ok(bound.unwrap_or(DEFAULT_MAX_WORKS))
}

/// The deep-archive ceiling from the product rules (§3.1): an upper bound, never a target.
const DEFAULT_MAX_WORKS: i32 = 200;

fn reason_code(outcome: &AdmissionOutcome) -> &'static str {
    match outcome {
        AdmissionOutcome::Admitted { .. } => "within_authorization",
        AdmissionOutcome::Reuse { .. } => "need_already_satisfied",
        AdmissionOutcome::Merge { .. } => "in_flight_work_covers_it",
        AdmissionOutcome::Defer { .. } => "deferred",
        AdmissionOutcome::Refuse { .. } => "no_valid_authorization",
        AdmissionOutcome::DecisionRequired { .. } => "question_unanswerable",
    }
}

fn reason_text(outcome: &AdmissionOutcome) -> String {
    match outcome {
        AdmissionOutcome::Admitted { .. } => "在有效授权范围内".to_owned(),
        AdmissionOutcome::Reuse { reason }
        | AdmissionOutcome::Merge { reason }
        | AdmissionOutcome::Defer { reason }
        | AdmissionOutcome::Refuse { reason }
        | AdmissionOutcome::DecisionRequired { reason, .. } => reason.clone(),
    }
}

/// Write the bounded instruction and move the target into archiving.
///
/// Both happen together: a target sitting in `archiving` with no order behind it, or an
/// order with the target still `pending_decision`, would each be a lie about what stage the
/// chain reached.
async fn write_work_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    decision_ref: Uuid,
    target_ref: Uuid,
    lane: &str,
    authorization_ref: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let work_order_ref = Uuid::new_v4();
    let max_works = max_works_for(transaction, authorization_ref).await?;
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, stop_conditions) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(max_works)
    .bind(json!({
        "maximumQuota": max_works,
        // A quota's shortfall is not a set of real objects (contract §3.1), so the order
        // states what stops it rather than what it expects to find.
        "stopOn": ["maximum_quota", "surface_ended", "risk_stop", "time_budget"],
    }))
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        "UPDATE collection_observation_target \
         SET lifecycle_state = 'archiving', lifecycle_changed_at = scope_001_now() \
         WHERE target_ref = $1",
    )
    .bind(target_ref)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
         VALUES ($1, $2, 'pending_decision', 'archiving', 'person', 'work_order_created', $3)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(format!("工单 {work_order_ref} 已创建"))
    .execute(&mut **transaction)
    .await?;

    Ok(work_order_ref)
}
