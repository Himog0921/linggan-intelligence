//! COLLECTION-001 · Persisting the four-stage acquisition chain.
//!
//! Request → Authorization → Admission → Work Order. Each stage is written separately so
//! that no earlier stage can be read as a later one having succeeded (INV-36).
//!
//! Nothing here reaches a platform. A Work Order row is a written instruction; execution is
//! a later stage that does not exist yet.

use crate::collection_control::{
    CapacitySelection, evaluate_capacity_in, ready_batch_claim_slots_in, required_capabilities_for,
};
use crate::directory_boundary::{directory_proven_sql, surface_scan_complete_sql};
use crate::work_order_lease::{
    IssuedLease, LeaseError, issue_work_order_lease_in_transaction, lease_schema_is_ready,
};
use linggan_contracts::{
    AdmissionFacts, AdmissionOutcome, AuthorizationBoundaryFailure, Capacity, decide_admission,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AcquisitionChainError {
    #[error("acquisition chain schema is not applied")]
    SchemaUnavailable,
    #[error("no observation target with that reference")]
    UnknownTarget,
    #[error("that target is not in a state where deep archiving can be requested: {state}")]
    TargetNotRequestable { state: String },
    #[error("material deepening needs between 1 and 200 distinct content targets")]
    InvalidMaterialTargets,
    #[error(
        "progressive creator archiving requires an authorization that permits 200 works; current bound is {current_bound}"
    )]
    ProgressiveArchiveAuthorizationTooSmall { current_bound: i32 },
    #[error("progressive creator archiving requires an active matching authorization")]
    ProgressiveArchiveAuthorizationMissing,
    #[error("the existing progressive creator archive has a different purpose")]
    ProgressiveArchivePurposeMismatch,
    #[error("the progressive creator archive cannot advance: {reason}")]
    ProgressiveArchiveNotReady { reason: &'static str },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What a request produced, all the way through admission.
#[derive(Debug)]
pub struct RequestOutcome {
    pub request_ref: Uuid,
    pub decision_ref: Uuid,
    pub outcome: AdmissionOutcome,
    /// Closed machine reason persisted on the Admission decision. Callers must not collapse a
    /// capacity, purpose or authorization boundary into a generic refusal.
    pub reason_code: &'static str,
    /// Present only when the decision admitted the request.
    pub work_order_ref: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestLeaseError {
    #[error(transparent)]
    Acquisition(#[from] AcquisitionChainError),
    #[error(transparent)]
    Lease(#[from] LeaseError),
}

#[derive(Debug)]
pub struct RequestLeaseOutcome {
    pub request: RequestOutcome,
    pub lease: Option<IssuedLease>,
}

#[derive(Debug, Default)]
pub struct ProgressiveArchiveTickSummary {
    /// Newly admitted deep-archive batches waiting in the common browser-work queue.
    pub queued: Vec<Uuid>,
    /// Historical field retained for callers built before batch work entered the shared queue.
    /// New code never records a browser dispatch here; only an eligible installation can do so.
    pub dispatched: Vec<Uuid>,
    pub skipped: Vec<(Uuid, String)>,
}

const PROGRESSIVE_ARCHIVE_VERSION: i32 = 1;
const PROGRESSIVE_ARCHIVE_DIRECTORY_LIMIT: i32 = 200;
const PROGRESSIVE_ARCHIVE_BATCH_SIZE: i64 = 3;
const PROGRESSIVE_ARCHIVE_MAX_GENERATION_PER_TICK: usize = 100;

/// One already-admitted material identity that a person has explicitly selected for deepening.
///
/// This is deliberately not a discovery remainder.  Supplying the identities here is the act
/// that prevents a normal profile scan from silently fanning out into detail/comment/media work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialDeepeningTarget {
    pub content_public_ref: Uuid,
    pub comment_limit: i32,
    pub reply_expand_limit: i32,
    pub acquire_media: bool,
    pub allow_ocr: bool,
    pub allow_asr: bool,
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
    let task_templates = authorization_task_templates(grant.target_kind, grant.lane);
    let dispatch_lanes = authorization_dispatch_lanes(grant.lane);
    let max_work_units = grant.max_works_per_target.unwrap_or(DEFAULT_MAX_WORKS);
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref, platform, target_kind, lane, purpose, max_targets, \
              max_works_per_target, allowed_task_templates,allowed_dispatch_lanes,max_work_units, \
              granted_by, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7,$8,$9,$10, \
                 'person', scope_001_now() + make_interval(days => $11))",
    )
    .bind(authorization_ref)
    .bind(grant.platform)
    .bind(grant.target_kind)
    .bind(grant.lane)
    .bind(grant.purpose)
    .bind(grant.max_targets)
    .bind(grant.max_works_per_target)
    .bind(task_templates)
    .bind(dispatch_lanes)
    .bind(max_work_units)
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
    request_and_admit_inner(database, target_ref, lane, purpose, requested_by, &[], None).await
}

/// Legacy convenience path that additionally tries to claim the just-created queued Work Order.
///
/// New browser work must normally stop after `request_and_admit`: the eligible plugin claims the
/// shared Work Order later.  This helper remains for the bounded progressive-archive callers while
/// they are migrated; it still materialises the same Work Order first and then explicitly changes
/// it to `leased` in the same transaction.
pub async fn request_admit_and_lease(
    database: &Database,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    valid_for_minutes: i32,
) -> Result<RequestLeaseOutcome, RequestLeaseError> {
    request_admit_and_lease_inner(
        database,
        target_ref,
        lane,
        purpose,
        requested_by,
        &[],
        None,
        valid_for_minutes,
    )
    .await
}

/// Start or resume the creator dossier contract used by the Observation Target page.
///
/// This is deliberately separate from the generic deep-archive request.  The visible action
/// promises a 200-Work directory ceiling and subsequent bounded detail batches, so a narrower
/// grant must be reported instead of silently turning that promise into 10 or 20 Works.  It
/// writes an admitted **batch** Work Order and stops there; a plugin installation claims it later
/// through the same fairness and eligibility path as every other browser task.
pub async fn request_progressive_archive(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
) -> Result<RequestOutcome, RequestLeaseError> {
    Ok(
        request_progressive_archive_inner(database, target_ref, purpose, requested_by, 0, false)
            .await?
            .request,
    )
}

/// Legacy convenience path that immediately attaches current capacity after writing a Work
/// Order.  Product routes must use [`request_progressive_archive`] instead so batch work cannot
/// bypass the common queue.  It remains temporarily for protocol-compatibility test fixtures.
pub async fn request_progressive_archive_and_lease(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    valid_for_minutes: i32,
) -> Result<RequestLeaseOutcome, RequestLeaseError> {
    request_progressive_archive_inner(
        database,
        target_ref,
        purpose,
        requested_by,
        valid_for_minutes,
        true,
    )
    .await
}

async fn request_progressive_archive_inner(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    valid_for_minutes: i32,
    issue_lease: bool,
) -> Result<RequestLeaseOutcome, RequestLeaseError> {
    if !acquisition_chain_schema_is_ready(database)
        .await
        .map_err(AcquisitionChainError::from)?
        || !lease_schema_is_ready(database)
            .await
            .map_err(AcquisitionChainError::from)?
    {
        return Err(AcquisitionChainError::SchemaUnavailable.into());
    }
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(AcquisitionChainError::from)?;
    // Every progressive path locks Target before Authorization. The worker uses the same order;
    // keeping it here prevents a person clicking Continue from deadlocking the minute tick.
    let target: Option<(String, String)> = sqlx::query_as(
        "SELECT platform,target_kind FROM collection_observation_target \
         WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(AcquisitionChainError::from)?;
    let Some((platform, target_kind)) = target else {
        return Err(AcquisitionChainError::UnknownTarget.into());
    };
    let qualifying_authorization = match progressive_authorization_in_transaction(
        &mut transaction,
        &platform,
        &target_kind,
        purpose,
    )
    .await
    .map_err(AcquisitionChainError::from)?
    {
        ProgressiveAuthorization::Qualified(authorization_ref) => authorization_ref,
        ProgressiveAuthorization::TooSmall(current_bound) => {
            transaction
                .rollback()
                .await
                .map_err(AcquisitionChainError::from)?;
            return Err(
                AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { current_bound }
                    .into(),
            );
        }
        ProgressiveAuthorization::Missing => {
            transaction
                .rollback()
                .await
                .map_err(AcquisitionChainError::from)?;
            return Err(AcquisitionChainError::ProgressiveArchiveAuthorizationMissing.into());
        }
    };
    if let Some((root_work_order_ref, root_purpose)) =
        canonical_progressive_root_in_transaction(&mut transaction, target_ref)
            .await
            .map_err(AcquisitionChainError::from)?
    {
        match progressive_root_directory_state_in_transaction(&mut transaction, root_work_order_ref)
            .await
            .map_err(AcquisitionChainError::from)?
        {
            ProgressiveRootDirectoryState::Ready => {
                if root_purpose != purpose {
                    transaction
                        .rollback()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    return Err(AcquisitionChainError::ProgressiveArchivePurposeMismatch.into());
                }
                let monitoring_enabled: bool = sqlx::query_scalar(
                    "SELECT monitoring_enabled FROM collection_observation_target WHERE target_ref=$1",
                )
                .bind(target_ref)
                .fetch_one(&mut *transaction)
                .await
                .map_err(AcquisitionChainError::from)?;
                let result = match advance_progressive_archive_in_transaction(
                    &mut transaction,
                    target_ref,
                    root_work_order_ref,
                    qualifying_authorization,
                    purpose,
                    requested_by,
                    monitoring_enabled,
                    valid_for_minutes,
                    issue_lease,
                )
                .await?
                {
                    ProgressiveAdvance::Outcome(outcome) => outcome,
                    ProgressiveAdvance::Skipped(reason) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(AcquisitionChainError::from)?;
                        return Err(
                            AcquisitionChainError::ProgressiveArchiveNotReady { reason }.into()
                        );
                    }
                };
                transaction
                    .commit()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                return Ok(result);
            }
            ProgressiveRootDirectoryState::InProgress => {
                if root_purpose != purpose {
                    transaction
                        .rollback()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    return Err(AcquisitionChainError::ProgressiveArchivePurposeMismatch.into());
                }
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                return Err(AcquisitionChainError::ProgressiveArchiveNotReady {
                    reason: "detail_batch_in_flight",
                }
                .into());
            }
            ProgressiveRootDirectoryState::RebuildRequired => {
                // Keep the old root and its Packages immutable.  It never proved the bounded
                // homepage directory, so the next admitted root is a normal first directory
                // collection, not a continuation of its partial detail batches.
            }
        }
    }
    let request = request_and_admit_in_transaction_with_progressive_resume(
        &mut transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        &[],
        Some(qualifying_authorization),
        true,
    )
    .await?;
    let lease = if let Some(work_order_ref) = request.work_order_ref {
        write_progressive_marker(
            &mut transaction,
            work_order_ref,
            work_order_ref,
            PROGRESSIVE_ARCHIVE_DIRECTORY_LIMIT,
        )
        .await
        .map_err(AcquisitionChainError::from)?;
        if issue_lease {
            assign_current_capacity_to_queued_work_order(
                &mut transaction,
                work_order_ref,
                target_ref,
                "deep_archive",
                &[],
            )
            .await?;
            Some(
                issue_work_order_lease_in_transaction(
                    &mut transaction,
                    work_order_ref,
                    valid_for_minutes,
                )
                .await?,
            )
        } else {
            None
        }
    } else {
        None
    };
    transaction
        .commit()
        .await
        .map_err(AcquisitionChainError::from)?;
    Ok(RequestLeaseOutcome { request, lease })
}

/// Admit one exact, person-selected set of existing materials for bounded deepening.
///
/// It shares the ordinary authorization/admission/work-order chain.  The only extra fact is the
/// exact content set written in the same transaction as the Work Order; no later scheduler query
/// is allowed to replace it with "whatever is newest now".
pub async fn request_and_admit_material_targets(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<RequestOutcome, AcquisitionChainError> {
    validate_material_targets(material_targets)?;
    request_and_admit_inner(
        database,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        material_targets,
        None,
    )
    .await
}

pub async fn request_admit_material_targets_and_lease(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    valid_for_minutes: i32,
) -> Result<RequestLeaseOutcome, RequestLeaseError> {
    validate_material_targets(material_targets)?;
    request_admit_and_lease_inner(
        database,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        material_targets,
        None,
        valid_for_minutes,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn request_admit_and_lease_inner(
    database: &Database,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
    valid_for_minutes: i32,
) -> Result<RequestLeaseOutcome, RequestLeaseError> {
    if !acquisition_chain_schema_is_ready(database)
        .await
        .map_err(AcquisitionChainError::from)?
        || !lease_schema_is_ready(database)
            .await
            .map_err(AcquisitionChainError::from)?
    {
        return Err(AcquisitionChainError::SchemaUnavailable.into());
    }
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(AcquisitionChainError::from)?;
    let request = request_and_admit_in_transaction(
        &mut transaction,
        target_ref,
        lane,
        purpose,
        requested_by,
        material_targets,
        required_authorization_ref,
    )
    .await?;
    let lease = if let Some(work_order_ref) = request.work_order_ref {
        assign_current_capacity_to_queued_work_order(
            &mut transaction,
            work_order_ref,
            target_ref,
            lane,
            material_targets,
        )
        .await?;
        Some(
            issue_work_order_lease_in_transaction(
                &mut transaction,
                work_order_ref,
                valid_for_minutes,
            )
            .await?,
        )
    } else {
        None
    };
    if requested_by == "agent" && lane == "patrol" && lease.is_some() {
        sqlx::query(
            "UPDATE collection_observation_target \
             SET last_patrol_dispatched_at=scope_001_now() WHERE target_ref=$1",
        )
        .bind(target_ref)
        .execute(&mut *transaction)
        .await
        .map_err(AcquisitionChainError::from)?;
    }
    transaction
        .commit()
        .await
        .map_err(AcquisitionChainError::from)?;
    Ok(RequestLeaseOutcome { request, lease })
}

/// Admit one frozen material set under the exact authorization already linked to that material.
///
/// This is intentionally narrower than ordinary admission: a reobservation may not discover a
/// second, broader grant for the same target class and use it as an authorization fallback.
pub async fn request_and_admit_material_targets_under_authorization(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    authorization_ref: Uuid,
) -> Result<RequestOutcome, AcquisitionChainError> {
    validate_material_targets(material_targets)?;
    request_and_admit_inner(
        database,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        material_targets,
        Some(authorization_ref),
    )
    .await
}

/// The transaction-aware form used when the caller must make admission and the next durable
/// execution step indivisible.  Reobservation is such an operation: committing a new Work Order
/// before its Lease exists leaves a window in which another request cannot distinguish a pending
/// instruction from an executable scope.
///
/// The caller owns the surrounding transaction and must have checked the schema before it began.
/// This keeps the target row lock acquired below through the subsequent lease issue.
pub(crate) async fn request_and_admit_material_targets_under_authorization_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    authorization_ref: Uuid,
) -> Result<RequestOutcome, AcquisitionChainError> {
    validate_material_targets(material_targets)?;
    request_and_admit_in_transaction(
        transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        material_targets,
        Some(authorization_ref),
    )
    .await
}

async fn request_and_admit_inner(
    database: &Database,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
) -> Result<RequestOutcome, AcquisitionChainError> {
    if !acquisition_chain_schema_is_ready(database).await? {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    let outcome = request_and_admit_in_transaction(
        &mut transaction,
        target_ref,
        lane,
        purpose,
        requested_by,
        material_targets,
        required_authorization_ref,
    )
    .await?;
    transaction.commit().await?;
    Ok(outcome)
}

pub(crate) async fn request_and_admit_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
) -> Result<RequestOutcome, AcquisitionChainError> {
    request_and_admit_in_transaction_with_progressive_resume(
        transaction,
        target_ref,
        lane,
        purpose,
        requested_by,
        material_targets,
        required_authorization_ref,
        false,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn request_and_admit_in_transaction_with_progressive_resume(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
    allow_progressive_resume: bool,
) -> Result<RequestOutcome, AcquisitionChainError> {
    let target: Option<(String, String, String)> = sqlx::query_as(
        "SELECT platform, target_kind, lifecycle_state \
         FROM collection_observation_target WHERE target_ref = $1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((platform, target_kind, lifecycle_state)) = target else {
        return Err(AcquisitionChainError::UnknownTarget);
    };
    // 前置状态按 lane 分开。
    //
    // **深度建档是一次性的**：只有待决的目标能申请，否则同一个博主会被反复全量建档。
    // **巡检本来就要反复申请**：它的前置是「已经建过档」，因此允许 archiving 与
    // monitoring。产品规则明写「深度建档必须完成之后才能启用监控」，两者的前置本就不同。
    //
    // 此前这里对所有 lane 一律要求 pending_decision，结果是巡检**永远派不出去**——
    // 目标一旦进入 archiving 就再也无法申请。这个缺陷由第一轮 tick 当场暴露。
    let requestable = match lane {
        // A fixed material set is a follow-up to an admitted baseline.  It may deepen an archived
        // or monitored creator, but it still uses the existing deep-archive authorization class.
        "deep_archive" if !material_targets.is_empty() => matches!(
            lifecycle_state.as_str(),
            "archiving" | "archived" | "monitoring" | "paused"
        ),
        "deep_archive" if allow_progressive_resume && target_kind == "creator" => matches!(
            lifecycle_state.as_str(),
            "pending_decision" | "archiving" | "archived" | "monitoring" | "paused"
        ),
        // `archiving` is accepted only so the scheduler can recover an expired bounded baseline.
        // Admission still merges a live lease and the scheduler caps the number of Work Orders.
        "deep_archive" => matches!(lifecycle_state.as_str(), "pending_decision" | "archiving"),
        // Manual observation is an ordinary bounded observation of a resolved
        // target. It may be useful before historical archiving is complete; only
        // an explicitly dismissed target is out of scope.
        "patrol" if requested_by == "person" => lifecycle_state != "dismissed",
        // The scheduler reaches this only through an enabled due rule. The
        // lifecycle check keeps a stopped or dismissed target from running.
        "patrol" => lifecycle_state == "monitoring",
        _ => matches!(lifecycle_state.as_str(), "archiving" | "monitoring"),
    };
    if !requestable {
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
    .execute(&mut **transaction)
    .await?;

    ensure_material_targets_belong_to_platform(&mut *transaction, &platform, material_targets)
        .await?;
    let facts = gather_facts(
        &mut *transaction,
        &platform,
        &target_kind,
        lane,
        purpose,
        requested_by,
        target_ref,
        material_targets,
        required_authorization_ref,
    )
    .await?;
    let outcome = decide_admission(&facts.admission);
    let decision_reason_code = reason_code(&outcome, &facts.admission);

    let decision_ref = Uuid::new_v4();
    let authorization_ref = match &outcome {
        AdmissionOutcome::Admitted { authorization_ref } => Uuid::parse_str(authorization_ref).ok(),
        // A merge points at an existing Work Order rather than creating a new authorization
        // decision.  The persisted decision contract therefore requires this field to remain
        // empty; strict matching is done against that live Work Order's authorization and its
        // complete frozen material scope before `decide_admission` is called.
        _ => None,
    };
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref, request_ref, outcome, unanswered_question, reason_code, reason, \
              authorization_ref,target_ref,station_ref,installation_ref,account_ref,eligibility_ref,monitor_rule_revision_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(outcome.code())
    .bind(outcome.unanswered_question().map(|q| q.number()))
    .bind(decision_reason_code)
    .bind(reason_text(&outcome))
    .bind(authorization_ref)
    .bind(target_ref)
    // A Request/Decision records no fictional assigned station.  Station, account,
    // quota, risk and capability are facts of the later atomic claim.
    .bind(Option::<Uuid>::None)
    .bind(Option::<Uuid>::None)
    .bind(Option::<Uuid>::None)
    .bind(Option::<Uuid>::None)
    .bind(facts.monitor_rule_revision_ref)
    .execute(&mut **transaction)
    .await?;

    let work_order_ref = if outcome.permits_work_order() {
        let work_order_ref = write_work_order(
            &mut *transaction,
            decision_ref,
            target_ref,
            lane,
            authorization_ref,
            facts.monitor_rule_revision_ref,
            requested_by,
            facts.dispatch_lane,
            facts.task_template,
            facts.estimated_work_units,
            material_targets,
        )
        .await?;
        write_material_targets(&mut *transaction, work_order_ref, material_targets).await?;
        Some(work_order_ref)
    } else {
        None
    };

    Ok(RequestOutcome {
        request_ref,
        decision_ref,
        outcome,
        reason_code: decision_reason_code,
        work_order_ref,
    })
}

fn validate_material_targets(
    material_targets: &[MaterialDeepeningTarget],
) -> Result<(), AcquisitionChainError> {
    if material_targets.is_empty() || material_targets.len() > 200 {
        return Err(AcquisitionChainError::InvalidMaterialTargets);
    }
    let mut identities = std::collections::HashSet::new();
    if material_targets.iter().any(|target| {
        !identities.insert(target.content_public_ref)
            || !(0..=30).contains(&target.comment_limit)
            || !(0..=2).contains(&target.reply_expand_limit)
            || (target.comment_limit == 0 && target.reply_expand_limit != 0)
    }) {
        return Err(AcquisitionChainError::InvalidMaterialTargets);
    }
    Ok(())
}

async fn ensure_material_targets_belong_to_platform(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<(), AcquisitionChainError> {
    if material_targets.is_empty() {
        return Ok(());
    }
    let refs: Vec<Uuid> = material_targets
        .iter()
        .map(|target| target.content_public_ref)
        .collect();
    let matched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content \
         WHERE public_ref = ANY($1) AND platform = $2",
    )
    .bind(&refs)
    .bind(platform)
    .fetch_one(&mut **transaction)
    .await?;
    if usize::try_from(matched).ok() != Some(material_targets.len()) {
        return Err(AcquisitionChainError::InvalidMaterialTargets);
    }
    Ok(())
}

async fn write_material_targets(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<(), sqlx::Error> {
    for (index, target) in material_targets.iter().enumerate() {
        sqlx::query(
            "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit, \
              acquire_media,allow_ocr,allow_asr) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(work_order_ref)
        .bind(target.content_public_ref)
        .bind(i32::try_from(index + 1).unwrap_or(i32::MAX))
        .bind(target.comment_limit)
        .bind(target.reply_expand_limit)
        .bind(target.acquire_media)
        .bind(target.allow_ocr)
        .bind(target.allow_asr)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Collect only facts the server can actually establish.
struct GatheredFacts {
    admission: AdmissionFacts,
    monitor_rule_revision_ref: Option<Uuid>,
    dispatch_lane: &'static str,
    task_template: &'static str,
    estimated_work_units: i32,
}

async fn gather_facts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    lane: &str,
    _purpose: &str,
    requested_by: &str,
    target_ref: Uuid,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
) -> Result<GatheredFacts, sqlx::Error> {
    let dispatch_lane = dispatch_lane_for(lane, requested_by);
    let task_template = task_template_for(target_kind, lane, !material_targets.is_empty());
    let estimated_work_units = estimated_work_units_for(task_template, material_targets.len());
    let authorization: Option<(Uuid, Option<i32>)> = sqlx::query_as(
        "SELECT authorization_ref,max_targets FROM collection_acquisition_authorization \
         WHERE platform = $1 AND target_kind = $2 AND lane = $3 \
           AND revoked_at IS NULL AND expires_at > scope_001_now() \
           AND $4=ANY(allowed_task_templates) AND $5=ANY(allowed_dispatch_lanes) \
           AND ($6::uuid IS NULL OR authorization_ref=$6) \
           AND ($7::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $7) \
           AND max_work_units >= $8 \
         ORDER BY expires_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(platform)
    .bind(target_kind)
    .bind(lane)
    .bind(task_template)
    .bind(dispatch_lane)
    .bind(required_authorization_ref)
    .bind(i32::try_from(material_targets.len()).unwrap_or(i32::MAX))
    .bind(estimated_work_units)
    .fetch_optional(&mut **transaction)
    .await?;

    // 「在途」= 还有活着的租约。task 的 pending / in_progress / completed 由租约任务序列
    // 分责；只要整份租约尚未结束，就不能再为同一目标和 lane 复制一份工单。
    //
    // 此前的判据是「存在一行工单」——而工单从不结束，于是第一次巡检之后，后续每一次都被
    // 合并掉，巡检永远只跑一次。一个只置位、从不复位的状态，等于把功能永久关掉。
    let in_flight: bool = if material_targets.is_empty() {
        sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order w \
                 WHERE w.target_ref = $1 AND w.lane = $2 AND w.dispatch_lane=$3 \
                   AND (w.queue_state IN ('queued','leased') OR EXISTS ( \
                       SELECT 1 FROM collection_work_order_lease l \
                       WHERE l.work_order_ref=w.work_order_ref \
                         AND l.released_at IS NULL AND l.expires_at>scope_001_now())))",
        )
        .bind(target_ref)
        .bind(lane)
        .bind(dispatch_lane)
        .fetch_one(&mut **transaction)
        .await?
    } else if let Some(authorization_ref) = required_authorization_ref {
        in_flight_work_for_exact_material_scope_in_transaction(
            transaction,
            target_ref,
            lane,
            authorization_ref,
            material_targets,
        )
        .await?
        .is_some()
    } else {
        let content_refs: Vec<Uuid> = material_targets
            .iter()
            .map(|target| target.content_public_ref)
            .collect();
        sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order w \
                 JOIN collection_work_order_material_target scope ON scope.work_order_ref=w.work_order_ref \
                 WHERE w.target_ref=$1 AND w.lane=$2 AND w.dispatch_lane=$3 \
                   AND scope.content_public_ref=ANY($4) \
                   AND (w.queue_state IN ('queued','leased') OR EXISTS ( \
                       SELECT 1 FROM collection_work_order_lease l \
                       WHERE l.work_order_ref=w.work_order_ref \
                         AND l.released_at IS NULL AND l.expires_at>scope_001_now())))",
        )
        .bind(target_ref)
        .bind(lane)
        .bind(dispatch_lane)
        .bind(&content_refs)
        .fetch_one(&mut **transaction)
        .await?
    };

    // 目标数量上限不再判定（Mog 2026-09-08 决定）。
    //
    // 授权的价值在于「谁批的、批了什么范围、到哪天为止、可以随时撤销」，这几样都留着。
    // 数量上限则是另一回事：它挡住的不是越权，而是「同一类观察多了一个对象」——而
    // 一份授权本就覆盖一类目标，多观察一个同类对象并没有越过人当初批准的范围。
    //
    // 它在真实运行里也只制造了阻塞：线上 8 次 `authorization_target_limit_reached`
    // 全部来自关键词巡查那份 `max_targets=1` 的 canary 遗留授权，而创作者两条通道
    // （`max_targets` 分别是 5 和无限）三个月来从未因它被拒过一次。
    //
    // `max_targets` 列与 `TargetLimitReached` 变体都保留：线上有 8 行历史决定的
    // reason_code 是它，删掉这个概念会让那 8 行无法解释。
    let (authorization_ref, authorization_failure) = if let Some((
        authorization_ref,
        _max_targets,
    )) = authorization
    {
        (Some(authorization_ref), None)
    } else {
        let status: (bool, bool, bool, bool) = sqlx::query_as(
                "SELECT \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND revoked_at IS NULL AND expires_at>scope_001_now() \
                           AND ($4::uuid IS NULL OR authorization_ref=$4)), \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND revoked_at IS NULL AND expires_at>scope_001_now() \
                           AND $5=ANY(allowed_task_templates) AND $6=ANY(allowed_dispatch_lanes) \
                           AND ($4::uuid IS NULL OR authorization_ref=$4) \
                           AND ($7::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $7)), \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND revoked_at IS NULL AND expires_at>scope_001_now() \
                           AND $5=ANY(allowed_task_templates) AND $6=ANY(allowed_dispatch_lanes) \
                           AND ($4::uuid IS NULL OR authorization_ref=$4) \
                           AND ($7::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $7) \
                           AND max_work_units >= $8), \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND ($4::uuid IS NULL OR authorization_ref=$4))",
            )
            .bind(platform)
            .bind(target_kind)
            .bind(lane)
            .bind(required_authorization_ref)
            .bind(task_template)
            .bind(dispatch_lane)
            .bind(i32::try_from(material_targets.len()).unwrap_or(i32::MAX))
            .bind(estimated_work_units)
            .fetch_one(&mut **transaction)
            .await?;
        let failure = if status.0 && !status.1 {
            AuthorizationBoundaryFailure::ScopeMismatch
        } else if status.1 && !status.2 {
            AuthorizationBoundaryFailure::WorkUnitLimitReached
        } else if status.3 {
            AuthorizationBoundaryFailure::ExpiredOrRevoked
        } else {
            AuthorizationBoundaryFailure::Missing
        };
        (None, Some(failure))
    };

    // 巡检面的工单一律冻结目标当前的活跃规则版本，**不分是人点的还是调度发的**。
    //
    // 此前只有 `agent` 发的才绑规则，人点「观察一次」发出的工单规则版本为空。采样口径
    // （排序、下拉次数、取前 N、发布时间窗）住在规则版本上，规则版本为空就等于口径整个
    // 丢失，插件只好按自己的默认走。2026-09-08 实测：同一个「考研自习」目标，定时巡检
    // 下发了 `ranking=most_liked`，人点那次下发的只有一个光秃秃的 `query`——于是「综合
    // 排序」和「最多点赞」两条规则手点出来的结果一模一样。**人点一次要的是「照这条规则
    // 现在跑一遍」，不是「按插件默认跑一遍」。**
    //
    // 绑上规则不会让人点的工单被规则闸门拦住：`reject_if_rule_changed` 里那几条
    // （规则变更、规则缺失、巡检暂停）都只对 `agent` 生效——自动巡检停了，人仍然可以手动
    // 观察一次，这正是这个按钮存在的意义。
    let monitor_rule_revision_ref = if lane == "patrol" {
        sqlx::query_scalar(
            "SELECT active_monitor_rule_revision_ref FROM collection_observation_target \
             WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(&mut **transaction)
        .await?
    } else {
        None
    };

    Ok(GatheredFacts {
        admission: AdmissionFacts {
            authorization_ref: authorization_ref.map(|value| value.to_string()),
            authorization_failure,
            in_flight_work_exists: in_flight,
            // No archive exists yet, so no need can already be satisfied. This becomes a real
            // query once archiving produces results.
            need_already_satisfied: false,
            // The Work Order is queued without selecting a station.  Claim is
            // the atomic point that tests live account/station/risk/quota facts.
            capacity: Capacity::Queueable,
            stop_conditions_expressible: true,
        },
        monitor_rule_revision_ref,
        dispatch_lane,
        task_template,
        estimated_work_units,
    })
}

/// One exact material scope that is either queued or still leased. A shared work ID or a shared
/// content ID is deliberately insufficient: changing comment/reply bounds or media/OCR/ASR policy
/// changes what execution is authorized to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InFlightMaterialScope {
    pub work_order_ref: Uuid,
    pub lease_ref: Option<Uuid>,
    pub expires_at: Option<String>,
}

/// Find an exact frozen material scope in the common queue or under a live lease. A queued work
/// is already sufficient to merge a duplicate person request; requiring a lease here would let
/// two clicks create parallel work during the period before any station claims the first one.
pub(crate) async fn in_flight_work_for_exact_material_scope_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    lane: &str,
    authorization_ref: Uuid,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<Option<InFlightMaterialScope>, sqlx::Error> {
    let rows: Vec<(Uuid, Option<Uuid>, Option<String>, Uuid, i32, i32, bool, bool, bool)> =
        sqlx::query_as(
            "SELECT work_order.work_order_ref,lease.lease_ref,lease.expires_at::text, \
                    scope.content_public_ref,scope.comment_limit,scope.reply_expand_limit, \
                    scope.acquire_media,scope.allow_ocr,scope.allow_asr \
             FROM collection_work_order work_order \
             JOIN collection_admission_decision decision \
               ON decision.decision_ref=work_order.decision_ref \
             LEFT JOIN collection_work_order_lease lease \
               ON lease.work_order_ref=work_order.work_order_ref \
              AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
             JOIN collection_work_order_material_target scope \
               ON scope.work_order_ref=work_order.work_order_ref \
             WHERE work_order.target_ref=$1 AND work_order.lane=$2 \
               AND decision.authorization_ref=$3 \
               AND (work_order.queue_state='queued' OR lease.lease_ref IS NOT NULL) \
             ORDER BY work_order.created_at,work_order.work_order_ref,lease.lease_ref,scope.ordinal",
        )
        .bind(target_ref)
        .bind(lane)
        .bind(authorization_ref)
        .fetch_all(&mut **transaction)
        .await?;

    let requested = normalized_material_scope(material_targets);
    let mut candidates: BTreeMap<
        (Uuid, Option<Uuid>, Option<String>),
        Vec<MaterialDeepeningTarget>,
    > = BTreeMap::new();
    for (
        work_order_ref,
        lease_ref,
        expires_at,
        content_public_ref,
        comment_limit,
        reply_expand_limit,
        acquire_media,
        allow_ocr,
        allow_asr,
    ) in rows
    {
        candidates
            .entry((work_order_ref, lease_ref, expires_at))
            .or_default()
            .push(MaterialDeepeningTarget {
                content_public_ref,
                comment_limit,
                reply_expand_limit,
                acquire_media,
                allow_ocr,
                allow_asr,
            });
    }
    Ok(candidates
        .into_iter()
        .find_map(|((work_order_ref, lease_ref, expires_at), scope)| {
            (normalized_material_scope(&scope) == requested).then_some(InFlightMaterialScope {
                work_order_ref,
                lease_ref,
                expires_at,
            })
        }))
}

fn normalized_material_scope(
    material_targets: &[MaterialDeepeningTarget],
) -> Vec<MaterialDeepeningTarget> {
    let mut normalized = material_targets.to_vec();
    normalized.sort_by_key(|target| target.content_public_ref);
    normalized
}

/// 只读地问一次第 5 问。不写任何东西，也不产生任何决定。
///
/// 执行工位页用它回答「系统现在有没有能力接活」。**页面与准入必须读同一个
/// `establish_capacity`**：两处各写一份判据，正是旧项目「页面显示已达上限但仍在派单」
/// 的成因，配额那段 SQL 已经因为同一个理由只留了一份。
///
/// 事务开了又回滚，不是浪费：`establish_capacity` 的签名要求事务内的一致快照，四个分项
/// 必须读同一个瞬间——否则页面可能显示「有在岗工位」的同时显示「因为没有工位而接不了活」。
pub async fn read_capacity(
    database: &Database,
    platform: &str,
    target_kind: &str,
    lane: &str,
) -> Result<Capacity, sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let capacity =
        establish_capacity(&mut transaction, platform, target_kind, lane, None, &[]).await?;
    transaction.rollback().await?;
    Ok(capacity.capacity)
}

/// Answer question 5 against real rows: staffed station, capabilities, budget, risk pause.
///
/// Asked in that order on purpose. A missing station makes the capability question moot, and
/// reporting "quota exhausted" when nothing is even installed would send the reader to the
/// wrong place.
async fn establish_capacity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    lane: &str,
    target_ref: Option<Uuid>,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<CapacitySelection, sqlx::Error> {
    let required = required_capabilities_for(
        target_kind,
        lane,
        !material_targets.is_empty(),
        material_targets
            .iter()
            .any(|target| target.comment_limit > 0),
        material_targets
            .iter()
            .any(|target| target.reply_expand_limit > 0),
        material_targets.iter().any(|target| target.acquire_media),
    );
    evaluate_capacity_in(
        transaction,
        platform,
        target_kind,
        lane,
        &required,
        target_ref,
    )
    .await
}

/// Transitional compatibility for callers that still ask for immediate leasing.
/// The durable instruction is already queued at this point; this helper makes a
/// separate, explicit claim decision before the old lease materialisation API is
/// called. Normal manual and scheduled flows intentionally do not use it.
async fn assign_current_capacity_to_queued_work_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    target_ref: Uuid,
    lane: &str,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<(), LeaseError> {
    let (platform, target_kind): (String, String) = sqlx::query_as(
        "SELECT platform,target_kind FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await?;
    let selection = establish_capacity(
        transaction,
        &platform,
        &target_kind,
        lane,
        Some(target_ref),
        material_targets,
    )
    .await?;
    let (Some(station_ref), Some(installation_ref)) =
        (selection.station_ref, selection.installation_ref)
    else {
        return Err(LeaseError::ControlBlocked {
            reason_code: selection.capacity.reason_code().to_owned(),
        });
    };
    let changed = sqlx::query(
        "UPDATE collection_work_order SET station_ref=$2,installation_ref=$3,account_ref=$4, \
             eligibility_ref=$5,queue_state='leased' \
         WHERE work_order_ref=$1 AND queue_state='queued'",
    )
    .bind(work_order_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(selection.account_ref)
    .bind(selection.eligibility_ref)
    .execute(&mut **transaction)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(LeaseError::AlreadyLeased);
    }
    Ok(())
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

/// Authorizations name stable execution scope. `purpose` stays on the request
/// and grant for audit; it is not a fragile exact-match switch for scheduling.
fn authorization_task_templates(target_kind: &str, lane: &str) -> Vec<&'static str> {
    match (target_kind, lane) {
        ("creator", "patrol") => vec!["creator_patrol"],
        ("keyword", "patrol") => vec!["keyword_patrol"],
        ("creator", "deep_archive") => vec!["creator_archive", "material_deepening"],
        _ => vec!["keyword_archive", "material_deepening"],
    }
}

fn authorization_dispatch_lanes(lane: &str) -> Vec<&'static str> {
    match lane {
        "patrol" => vec!["immediate", "scheduled"],
        _ => vec!["immediate", "batch"],
    }
}

fn dispatch_lane_for(lane: &str, requested_by: &str) -> &'static str {
    match (lane, requested_by) {
        ("patrol", "agent") => "scheduled",
        ("deep_archive", "agent") => "batch",
        _ => "immediate",
    }
}

fn task_template_for(target_kind: &str, lane: &str, has_material_targets: bool) -> &'static str {
    if has_material_targets {
        return "material_deepening";
    }
    match (target_kind, lane) {
        ("creator", "patrol") => "creator_patrol",
        ("keyword", "patrol") => "keyword_patrol",
        ("creator", "deep_archive") => "creator_archive",
        _ => "keyword_archive",
    }
}

fn estimated_work_units_for(task_template: &str, material_count: usize) -> i32 {
    match task_template {
        // Profile identity + one bounded profile-discovery scan. It is not an
        // implicit historic archive.
        "creator_patrol" => 2,
        "material_deepening" => i32::try_from(material_count).unwrap_or(i32::MAX).max(1),
        _ => 1,
    }
}

fn reason_code(outcome: &AdmissionOutcome, facts: &AdmissionFacts) -> &'static str {
    match outcome {
        AdmissionOutcome::Admitted { .. } => "within_authorization",
        AdmissionOutcome::Reuse { .. } => "need_already_satisfied",
        AdmissionOutcome::Merge { .. } => "in_flight_work_covers_it",
        AdmissionOutcome::Defer { .. } => facts.capacity.reason_code(),
        AdmissionOutcome::Refuse { .. } => facts
            .authorization_failure
            .map(AuthorizationBoundaryFailure::as_str)
            .unwrap_or("authorization_missing"),
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
    monitor_rule_revision_ref: Option<Uuid>,
    requested_by: &str,
    dispatch_lane: &str,
    task_template: &str,
    estimated_work_units: i32,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<Uuid, sqlx::Error> {
    let work_order_ref = Uuid::new_v4();
    let max_works = max_works_for(transaction, authorization_ref).await?;
    let dedupe_key = format!(
        "{dispatch_lane}:{target_ref}:{task_template}:{}:{}",
        monitor_rule_revision_ref.unwrap_or(Uuid::nil()),
        material_scope_dedupe_fragment(material_targets),
    );
    let dispatch_group_key = (dispatch_lane == "batch").then(|| format!("target:{target_ref}"));
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, station_ref, \
             installation_ref,account_ref,eligibility_ref,monitor_rule_revision_ref,stop_conditions, \
             dispatch_lane,queue_state,scheduled_for,dedupe_key,estimated_work_units, \
             dispatch_group_key,retry_not_before_at) \
         VALUES ($1, $2, $3, $4, $5, NULL, NULL,NULL,NULL,$6,$7,$8,'queued', \
                 scope_001_now(),$9,$10,$11,scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(max_works)
    .bind(monitor_rule_revision_ref)
    .bind(json!({
        "maximumQuota": max_works,
        // A quota's shortfall is not a set of real objects (contract §3.1), so the order
        // states what stops it rather than what it expects to find.
        "stopOn": ["maximum_quota", "surface_ended", "risk_stop", "time_budget"],
    }))
    .bind(dispatch_lane)
    .bind(dedupe_key)
    .bind(estimated_work_units)
    .bind(dispatch_group_key)
    .execute(&mut **transaction)
    .await?;

    // 只有深度建档会推进生命周期。**巡检不改状态**：它是一个已建档目标的常规动作，
    // 每跑一次就改一次状态，会把「这个目标处于什么阶段」变成「它最近被派过一次」。
    //
    // **而关键词连这一步都不做**：`0042` 的 CHECK 禁止关键词进入 `archiving`，理由是
    // 「Keyword observation has a monitor lifecycle, not a creator archive lifecycle」。
    // 这条 UPDATE 原先不分目标类型，于是关键词的建档请求在准入通过之后必然撞上那条
    // CHECK，整个事务回滚——**结果是关键词根本发不出建档工单**，而页面只会显示一句
    // 「上一次动作没有完成」，看不出是被一条数据库不变量挡住的。
    //
    // 关键词「正在建档 / 建过档了」不靠生命周期字段表达，而是从证据里查
    // （`collection_control::keyword_baseline_qualified`），所以这里跳过它是完整的，
    // 不是少做了一步。
    if lane == "deep_archive" {
        let moved = sqlx::query(
            "UPDATE collection_observation_target \
             SET lifecycle_state = 'archiving', lifecycle_changed_at = scope_001_now() \
             WHERE target_ref = $1 AND lifecycle_state='pending_decision' \
               AND target_kind <> 'keyword'",
        )
        .bind(target_ref)
        .execute(&mut **transaction)
        .await?
        .rows_affected();
        if moved == 1 {
            sqlx::query(
                "INSERT INTO collection_observation_target_transition \
                     (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
                 VALUES ($1, $2, 'pending_decision', 'archiving', $3, 'rule_baseline_started', $4)",
            )
            .bind(Uuid::new_v4())
            .bind(target_ref)
            .bind(if requested_by == "agent" {
                "system"
            } else {
                "person"
            })
            .bind(format!("工单 {work_order_ref} 已创建"))
            .execute(&mut **transaction)
            .await?;
        }
    }

    Ok(work_order_ref)
}

/// The active WorkOrder dedupe index protects an exact browser read scope, not
/// every future batch for the same target.  Persisting a digest keeps the key
/// compact while still separating different content/comment/media contracts.
fn material_scope_dedupe_fragment(material_targets: &[MaterialDeepeningTarget]) -> String {
    if material_targets.is_empty() {
        return "target".to_owned();
    }
    let mut normalized = material_targets.to_vec();
    normalized.sort_by_key(|target| target.content_public_ref);
    let input = normalized
        .iter()
        .map(|target| {
            format!(
                "{}:{}:{}:{}:{}:{}",
                target.content_public_ref,
                target.comment_limit,
                target.reply_expand_limit,
                target.acquire_media,
                target.allow_ocr,
                target.allow_asr
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    Sha256::digest(input.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn write_progressive_marker(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    root_work_order_ref: Uuid,
    work_order_limit: i32,
) -> Result<(), sqlx::Error> {
    let marker = json!({
        "version": PROGRESSIVE_ARCHIVE_VERSION,
        "rootWorkOrderRef": root_work_order_ref,
        "maxDirectoryWorks": PROGRESSIVE_ARCHIVE_DIRECTORY_LIMIT,
        "batchSize": PROGRESSIVE_ARCHIVE_BATCH_SIZE,
        "commentLimit": 30,
        "replyExpandLimit": 2,
        "acquireMedia": true,
        "allowOcr": true,
        "allowAsr": true,
    });
    sqlx::query(
        "UPDATE collection_work_order \
         SET max_works=$2, \
             stop_conditions=jsonb_set( \
               jsonb_set(stop_conditions,'{maximumQuota}',to_jsonb($2::integer),true), \
               '{progressiveArchive}',$3::jsonb,true) \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .bind(work_order_limit)
    .bind(marker)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

enum ProgressiveAuthorization {
    Qualified(Uuid),
    TooSmall(i32),
    Missing,
}

async fn progressive_authorization_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    purpose: &str,
) -> Result<ProgressiveAuthorization, sqlx::Error> {
    let qualified: Option<Uuid> = sqlx::query_scalar(
        "SELECT authorization_ref FROM collection_acquisition_authorization \
         WHERE platform=$1 AND target_kind=$2 AND lane='deep_archive' AND purpose=$3 \
           AND revoked_at IS NULL AND expires_at>scope_001_now() \
           AND (max_works_per_target IS NULL OR max_works_per_target >= $4) \
         ORDER BY expires_at DESC,authorization_ref LIMIT 1 FOR UPDATE",
    )
    .bind(platform)
    .bind(target_kind)
    .bind(purpose)
    .bind(PROGRESSIVE_ARCHIVE_DIRECTORY_LIMIT)
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some(authorization_ref) = qualified {
        return Ok(ProgressiveAuthorization::Qualified(authorization_ref));
    }
    let smaller_bound: Option<i32> = sqlx::query_scalar(
        "SELECT max(max_works_per_target) FROM collection_acquisition_authorization \
         WHERE platform=$1 AND target_kind=$2 AND lane='deep_archive' AND purpose=$3 \
           AND revoked_at IS NULL AND expires_at>scope_001_now()",
    )
    .bind(platform)
    .bind(target_kind)
    .bind(purpose)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(smaller_bound.map_or(
        ProgressiveAuthorization::Missing,
        ProgressiveAuthorization::TooSmall,
    ))
}

async fn canonical_progressive_root_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<Option<(Uuid, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT work_order.work_order_ref,request.purpose \
         FROM collection_work_order work_order \
         JOIN collection_admission_decision decision USING(decision_ref) \
         JOIN collection_acquisition_request request USING(request_ref) \
         WHERE work_order.target_ref=$1 AND work_order.lane='deep_archive' \
           AND work_order.stop_conditions #>> '{progressiveArchive,version}'=$2 \
           AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
         ORDER BY work_order.created_at DESC,work_order.work_order_ref DESC \
         LIMIT 1 FOR UPDATE OF work_order",
    )
    .bind(target_ref)
    .bind(PROGRESSIVE_ARCHIVE_VERSION.to_string())
    .fetch_optional(&mut **transaction)
    .await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProgressiveRootDirectoryState {
    Ready,
    InProgress,
    RebuildRequired,
}

/// A root is a usable directory only when its own accepted discovery Package says that the
/// producer reached the page end, or that it actually acquired the contract's 200 entries.
/// A partial package with a 200 limit is evidence, but it is not a directory baseline.
async fn progressive_root_directory_state_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    root_work_order_ref: Uuid,
) -> Result<ProgressiveRootDirectoryState, sqlx::Error> {
    let (directory_ready, work_in_progress): (bool, bool) = sqlx::query_as(
        concat!(
            "SELECT \
           EXISTS ( \
             SELECT 1 FROM collection_work_order root_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE root_order.work_order_ref=$1 \
               AND package.package_kind='profile_discovery' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND receipt.material_admission='ACCEPTED' \
               AND ",
            directory_proven_sql!(),
            ") AS directory_ready, \
           EXISTS ( \
             SELECT 1 FROM collection_work_order work_order \
             LEFT JOIN collection_work_order_lease lease USING(work_order_ref) \
             WHERE (work_order.work_order_ref=$1 \
                    OR work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=$1::text) \
               AND (work_order.queue_state='queued' \
                    OR (lease.released_at IS NULL AND lease.expires_at>scope_001_now()))) AS work_in_progress",
        ),
    )
    .bind(root_work_order_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(if directory_ready {
        ProgressiveRootDirectoryState::Ready
    } else if work_in_progress {
        ProgressiveRootDirectoryState::InProgress
    } else {
        ProgressiveRootDirectoryState::RebuildRequired
    })
}

enum ProgressiveAdvance {
    Outcome(RequestLeaseOutcome),
    Skipped(&'static str),
}

async fn advance_progressive_archive_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    root_work_order_ref: Uuid,
    authorization_ref: Uuid,
    purpose: &str,
    requested_by: &str,
    _monitoring_enabled: bool,
    valid_for_minutes: i32,
    issue_lease: bool,
) -> Result<ProgressiveAdvance, RequestLeaseError> {
    let content_refs: Vec<Uuid> = sqlx::query_scalar(
        concat!(
            "WITH canonical_directory_package AS ( \
             SELECT package.package_ref,package.accepted_at \
             FROM collection_work_order root_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             WHERE root_order.work_order_ref=$2 \
               AND package.package_kind='profile_discovery' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND receipt.material_admission='ACCEPTED' \
             ORDER BY package.accepted_at DESC,package.package_ref DESC LIMIT 1 \
         ), canonical_directory AS ( \
             SELECT finding.content_public_ref,directory.accepted_at AS first_seen \
             FROM canonical_directory_package directory \
             JOIN linggan_material_discovery_finding finding USING(package_ref) \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=finding.package_ref \
              AND disposition.record_ordinal=finding.record_ordinal \
             WHERE finding.discovery_kind='profile_discovery' \
               AND disposition.disposition='accepted_for_library_discovery' \
         ), qualified_patrol_packages AS ( \
             SELECT DISTINCT package.package_ref,package.accepted_at \
             FROM collection_work_order patrol_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE patrol_order.target_ref=$1 AND patrol_order.lane='patrol' \
               AND package.package_kind='profile_discovery' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND receipt.material_admission='ACCEPTED' \
               AND layer->>'capability'='profile_discovery' \
               AND ",
            surface_scan_complete_sql!(),
            " \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref \
                                 AND disposition.disposition='quarantined') \
         ), patrol_additions AS ( \
             SELECT finding.content_public_ref,patrol.accepted_at AS first_seen \
             FROM qualified_patrol_packages patrol \
             JOIN linggan_material_discovery_finding finding USING(package_ref) \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=finding.package_ref \
              AND disposition.record_ordinal=finding.record_ordinal \
             WHERE finding.discovery_kind='profile_discovery' \
               AND disposition.disposition='accepted_for_library_discovery' \
         ), current_directory AS ( \
             SELECT content_public_ref,min(first_seen) AS first_seen FROM ( \
               SELECT * FROM canonical_directory UNION ALL SELECT * FROM patrol_additions \
             ) works GROUP BY content_public_ref \
         ) \
         SELECT current_directory.content_public_ref FROM current_directory \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM linggan_material_content_detail detail \
             WHERE detail.content_public_ref=current_directory.content_public_ref) \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_work_order scoped_order \
             JOIN collection_work_order_material_target scope USING(work_order_ref) \
             LEFT JOIN collection_work_order_lease scoped_lease USING(work_order_ref) \
             WHERE scoped_order.target_ref=$1 \
               AND scope.content_public_ref=current_directory.content_public_ref \
               AND (scoped_order.queue_state='queued' \
                    OR (scoped_lease.released_at IS NULL \
                        AND scoped_lease.expires_at>scope_001_now()))) \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_work_order unavailable_order \
             JOIN collection_work_order_material_target unavailable_scope \
               ON unavailable_scope.work_order_ref=unavailable_order.work_order_ref \
             JOIN linggan_material_content unavailable_content \
               ON unavailable_content.public_ref=unavailable_scope.content_public_ref \
             JOIN collection_work_order_lease unavailable_lease \
               ON unavailable_lease.work_order_ref=unavailable_order.work_order_ref \
             JOIN collection_work_order_lease_task unavailable_task \
               ON unavailable_task.lease_ref=unavailable_lease.lease_ref \
             JOIN linggan_runtime_task unavailable_runtime \
               ON unavailable_runtime.task_id=unavailable_task.task_id \
             WHERE unavailable_order.target_ref=$1 \
               AND unavailable_content.public_ref=current_directory.content_public_ref \
               -- An explicitly absent page and a bounded pre-Attempt read
               -- failure both remain unresolved details. Neither may be
               -- silently reintroduced by the automatic progressive worker;
               -- a later retry requires a new, explicit acquisition decision.
               AND unavailable_task.execution_state IN ('unavailable','blocked') \
               AND unavailable_runtime.task_spec #>> '{target,contentExternalId}'=unavailable_content.content_external_id) \
         ORDER BY current_directory.first_seen,current_directory.content_public_ref \
         LIMIT $3",
        ),
    )
    .bind(target_ref)
    .bind(root_work_order_ref)
    .bind(PROGRESSIVE_ARCHIVE_BATCH_SIZE)
    .fetch_all(&mut **transaction)
    .await
    .map_err(AcquisitionChainError::from)?;
    if content_refs.is_empty() {
        // 「一篇都挑不出来」有两种完全不同的原因，必须分开告诉用户。
        //
        // 候选查询同时排除了「已经有详情的」和「已经在别的批次里在途的」。若只报
        // `no_missing_accepted_work`，正在跑的批次会被说成「没有可继续的内容」——人看到的
        // 是「点了没反应」，而实际上活正在进行。此前 `detail_batch_in_flight` 这个理由在
        // 全仓库没有任何一处会产生，页面上那条「建档进行中」的提示永远不会出现。
        let in_flight: bool = sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order work_order \
                 JOIN collection_work_order_material_target scope USING (work_order_ref) \
                 LEFT JOIN collection_work_order_lease lease USING (work_order_ref) \
                 WHERE work_order.target_ref=$1 \
                   AND (work_order.queue_state='queued' \
                        OR (lease.released_at IS NULL \
                            AND lease.expires_at>scope_001_now())))",
        )
        .bind(target_ref)
        .fetch_one(&mut **transaction)
        .await
        .map_err(AcquisitionChainError::from)?;
        return Ok(ProgressiveAdvance::Skipped(if in_flight {
            "detail_batch_in_flight"
        } else {
            "no_missing_accepted_work"
        }));
    }
    let material_targets = content_refs
        .iter()
        .map(|content_public_ref| MaterialDeepeningTarget {
            content_public_ref: *content_public_ref,
            comment_limit: 30,
            reply_expand_limit: 2,
            acquire_media: true,
            allow_ocr: true,
            allow_asr: true,
        })
        .collect::<Vec<_>>();
    let request = request_and_admit_in_transaction_with_progressive_resume(
        transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        &material_targets,
        Some(authorization_ref),
        true,
    )
    .await?;
    let Some(work_order_ref) = request.work_order_ref else {
        return Ok(ProgressiveAdvance::Outcome(RequestLeaseOutcome {
            request,
            lease: None,
        }));
    };
    write_progressive_marker(
        transaction,
        work_order_ref,
        root_work_order_ref,
        i32::try_from(content_refs.len()).unwrap_or(i32::MAX),
    )
    .await
    .map_err(AcquisitionChainError::from)?;
    let lease = if issue_lease {
        assign_current_capacity_to_queued_work_order(
            transaction,
            work_order_ref,
            target_ref,
            "deep_archive",
            &material_targets,
        )
        .await?;
        Some(
            issue_work_order_lease_in_transaction(transaction, work_order_ref, valid_for_minutes)
                .await?,
        )
    } else {
        None
    };
    Ok(ProgressiveAdvance::Outcome(RequestLeaseOutcome {
        request,
        lease,
    }))
}

/// Keep versioned creator dossier plans supplied with bounded material batches.
///
/// The scheduler never downloads or parses platform data. It only turns already accepted,
/// target-scoped directory facts into explicitly bounded WorkOrders. Exact content scopes still
/// dedupe under the target-row transaction. A source may keep at most `currently eligible
/// claimants × lane policy multiplier` queued or leased batch orders, so the worker neither
/// starves ten ready stations nor builds an unbounded archive backlog.
pub async fn run_progressive_archives(
    database: &Database,
) -> Result<ProgressiveArchiveTickSummary, RequestLeaseError> {
    if !acquisition_chain_schema_is_ready(database)
        .await
        .map_err(AcquisitionChainError::from)?
        || !lease_schema_is_ready(database)
            .await
            .map_err(AcquisitionChainError::from)?
    {
        return Err(AcquisitionChainError::SchemaUnavailable.into());
    }
    let plans: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "WITH roots AS ( \
           SELECT work_order.target_ref,work_order.work_order_ref,request.purpose,work_order.created_at, \
                  target.last_scheduler_considered_at, \
                  row_number() OVER (PARTITION BY work_order.target_ref \
                                     ORDER BY work_order.created_at DESC,work_order.work_order_ref DESC) AS root_rank \
           FROM collection_work_order work_order \
           JOIN collection_admission_decision decision \
             ON decision.decision_ref=work_order.decision_ref \
           JOIN collection_acquisition_request request \
             ON request.request_ref=decision.request_ref \
           JOIN collection_observation_target target \
             ON target.target_ref=work_order.target_ref \
           WHERE work_order.lane='deep_archive' \
             AND work_order.stop_conditions #>> '{progressiveArchive,version}'=$1 \
             AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
             AND decision.authorization_ref IS NOT NULL \
             AND target.lifecycle_state <> 'dismissed') \
         SELECT target_ref,work_order_ref,purpose FROM roots WHERE root_rank=1 \
         ORDER BY last_scheduler_considered_at NULLS FIRST,created_at,work_order_ref LIMIT 50",
    )
    .bind(PROGRESSIVE_ARCHIVE_VERSION.to_string())
    .fetch_all(database.pool())
    .await
    .map_err(AcquisitionChainError::from)?;

    let mut summary = ProgressiveArchiveTickSummary::default();
    let mut generated = 0_usize;
    while generated < PROGRESSIVE_ARCHIVE_MAX_GENERATION_PER_TICK {
        let mut advanced_any = false;
        // A pass takes at most one batch per source. Repeating passes only
        // while a source remains below its persisted cap yields round-robin
        // production without an in-memory source cursor.
        for (target_ref, root_work_order_ref, purpose) in &plans {
            if generated >= PROGRESSIVE_ARCHIVE_MAX_GENERATION_PER_TICK {
                break;
            }
            let mut transaction = database
                .pool()
                .begin()
                .await
                .map_err(AcquisitionChainError::from)?;
            let state: Option<(String, String, String, bool)> = sqlx::query_as(
                "SELECT platform,target_kind,lifecycle_state,monitoring_enabled \
                 FROM collection_observation_target \
                 WHERE target_ref=$1 FOR UPDATE",
            )
            .bind(*target_ref)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AcquisitionChainError::from)?;
            let Some((platform, target_kind, lifecycle_state, monitoring_enabled)) = state else {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary
                    .skipped
                    .push((*target_ref, "target_not_found".to_owned()));
                continue;
            };
            // Reuse the Target's existing scheduler fairness fact.  A bounded
            // root page must not keep selecting its first fifty sources while
            // later dossiers wait forever; writing this under the same target
            // lock makes the next tick start with sources considered least
            // recently, without inventing a progressive-only cursor object.
            sqlx::query(
                "UPDATE collection_observation_target \
                 SET last_scheduler_considered_at=scope_001_now() WHERE target_ref=$1",
            )
            .bind(*target_ref)
            .execute(&mut *transaction)
            .await
            .map_err(AcquisitionChainError::from)?;
            if lifecycle_state == "dismissed" {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary
                    .skipped
                    .push((*target_ref, "target_dismissed".to_owned()));
                continue;
            }
            if progressive_root_directory_state_in_transaction(
                &mut transaction,
                *root_work_order_ref,
            )
            .await
            .map_err(AcquisitionChainError::from)?
                != ProgressiveRootDirectoryState::Ready
            {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary
                    .skipped
                    .push((*target_ref, "directory_baseline_not_ready".to_owned()));
                continue;
            }
            let authorization_ref = match progressive_authorization_in_transaction(
                &mut transaction,
                &platform,
                &target_kind,
                purpose,
            )
            .await
            .map_err(AcquisitionChainError::from)?
            {
                ProgressiveAuthorization::Qualified(authorization_ref) => authorization_ref,
                ProgressiveAuthorization::TooSmall(_) | ProgressiveAuthorization::Missing => {
                    transaction
                        .rollback()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    summary
                        .skipped
                        .push((*target_ref, "authorization_expired_or_too_small".to_owned()));
                    continue;
                }
            };
            let required =
                required_capabilities_for(&target_kind, "deep_archive", true, true, true, true);
            let (ready_claimants, ready_work_multiplier) =
                ready_batch_claim_slots_in(&mut transaction, &platform, &required)
                    .await
                    .map_err(AcquisitionChainError::from)?;
            let ready_cap = ready_claimants.saturating_mul(i64::from(ready_work_multiplier));
            let active_group_work: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM collection_work_order \
                 WHERE dispatch_lane='batch' AND dispatch_group_key=$1 \
                   AND queue_state IN ('queued','leased')",
            )
            .bind(format!("target:{target_ref}"))
            .fetch_one(&mut *transaction)
            .await
            .map_err(AcquisitionChainError::from)?;
            if ready_cap == 0 || active_group_work >= ready_cap {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                // Capacity is a deliberate scheduler decision, not an
                // invisible absence of the root.  Persisted ready capacity
                // can be zero while every compatible station is busy or
                // unavailable; reporting this lets the Runtime distinguish
                // it from a malformed or missing progressive plan.
                summary.skipped.push((
                    *target_ref,
                    if ready_cap == 0 {
                        "no_ready_batch_capacity".to_owned()
                    } else {
                        "batch_source_cap_reached".to_owned()
                    },
                ));
                continue;
            }
            let advance = advance_progressive_archive_in_transaction(
                &mut transaction,
                *target_ref,
                *root_work_order_ref,
                authorization_ref,
                purpose,
                "agent",
                monitoring_enabled,
                180,
                false,
            )
            .await?;
            match advance {
                ProgressiveAdvance::Skipped(reason) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    summary.skipped.push((*target_ref, reason.to_owned()));
                }
                ProgressiveAdvance::Outcome(outcome)
                    if outcome.request.work_order_ref.is_some() =>
                {
                    transaction
                        .commit()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    summary.queued.push(*target_ref);
                    generated += 1;
                    advanced_any = true;
                }
                ProgressiveAdvance::Outcome(outcome) => {
                    let reason = outcome.request.reason_code.to_owned();
                    transaction
                        .commit()
                        .await
                        .map_err(AcquisitionChainError::from)?;
                    summary.skipped.push((*target_ref, reason));
                }
            }
        }
        if !advanced_any {
            break;
        }
    }
    Ok(summary)
}
