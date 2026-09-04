//! COLLECTION-001 · Persisting the four-stage acquisition chain.
//!
//! Request → Authorization → Admission → Work Order. Each stage is written separately so
//! that no earlier stage can be read as a later one having succeeded (INV-36).
//!
//! Nothing here reaches a platform. A Work Order row is a written instruction; execution is
//! a later stage that does not exist yet.

use crate::collection_control::{
    CapacitySelection, evaluate_capacity_in, required_capabilities_for,
};
use crate::work_order_lease::{
    IssuedLease, LeaseError, issue_work_order_lease_in_transaction, lease_schema_is_ready,
};
use linggan_contracts::{
    AdmissionFacts, AdmissionOutcome, AuthorizationBoundaryFailure, Capacity, decide_admission,
};
use linggan_storage_postgres::Database;
use serde_json::json;
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
    pub dispatched: Vec<Uuid>,
    pub skipped: Vec<(Uuid, String)>,
}

const PROGRESSIVE_ARCHIVE_VERSION: i32 = 1;
const PROGRESSIVE_ARCHIVE_DIRECTORY_LIMIT: i32 = 200;
const PROGRESSIVE_ARCHIVE_BATCH_SIZE: i64 = 3;

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
    request_and_admit_inner(database, target_ref, lane, purpose, requested_by, &[], None).await
}

/// Create Request → Decision → Work Order → Lease under one transaction. A failed Lease check
/// rolls the preceding writes back, so an "admitted" row cannot be mistaken for executable work.
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
/// grant must be reported instead of silently turning that promise into 10 or 20 Works.
pub async fn request_progressive_archive_and_lease(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
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
        )
        .await?
        {
            ProgressiveAdvance::Outcome(outcome) => outcome,
            ProgressiveAdvance::Skipped(reason) => {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                return Err(AcquisitionChainError::ProgressiveArchiveNotReady { reason }.into());
            }
        };
        transaction
            .commit()
            .await
            .map_err(AcquisitionChainError::from)?;
        return Ok(result);
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
        "patrol" if requested_by == "person" => {
            matches!(
                lifecycle_state.as_str(),
                "archived" | "monitoring" | "paused"
            ) || (target_kind == "keyword" && lifecycle_state == "pending_decision")
        }
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
    .bind(facts.capacity.station_ref)
    .bind(facts.capacity.installation_ref)
    .bind(facts.capacity.account_ref)
    .bind(facts.capacity.eligibility_ref)
    .bind(facts.monitor_rule_revision_ref)
    .execute(&mut **transaction)
    .await?;

    let work_order_ref = if outcome.permits_work_order() {
        // 准入认定了哪台工位，工单就记哪台。没有这一步，每日额度算不出来。
        let station_ref = facts.capacity.station_ref;
        let work_order_ref = write_work_order(
            &mut *transaction,
            decision_ref,
            target_ref,
            lane,
            authorization_ref,
            station_ref,
            facts.capacity.installation_ref,
            facts.capacity.account_ref,
            facts.capacity.eligibility_ref,
            facts.monitor_rule_revision_ref,
            requested_by,
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
            || !(1..=30).contains(&target.comment_limit)
            || !(0..=2).contains(&target.reply_expand_limit)
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
    capacity: CapacitySelection,
    monitor_rule_revision_ref: Option<Uuid>,
}

async fn gather_facts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    lane: &str,
    purpose: &str,
    requested_by: &str,
    target_ref: Uuid,
    material_targets: &[MaterialDeepeningTarget],
    required_authorization_ref: Option<Uuid>,
) -> Result<GatheredFacts, sqlx::Error> {
    let authorization: Option<(Uuid, Option<i32>)> = sqlx::query_as(
        "SELECT authorization_ref,max_targets FROM collection_acquisition_authorization \
         WHERE platform = $1 AND target_kind = $2 AND lane = $3 \
           AND revoked_at IS NULL AND expires_at > scope_001_now() \
           AND purpose=$4 AND ($5::uuid IS NULL OR authorization_ref=$5) \
           AND ($6::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $6) \
         ORDER BY expires_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(platform)
    .bind(target_kind)
    .bind(lane)
    .bind(purpose)
    .bind(required_authorization_ref)
    .bind(i32::try_from(material_targets.len()).unwrap_or(i32::MAX))
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
                 JOIN collection_work_order_lease l ON l.work_order_ref = w.work_order_ref \
                 WHERE w.target_ref = $1 AND w.lane = $2 \
                   AND l.released_at IS NULL AND l.expires_at > scope_001_now())",
        )
        .bind(target_ref)
        .bind(lane)
        .fetch_one(&mut **transaction)
        .await?
    } else if let Some(authorization_ref) = required_authorization_ref {
        live_lease_for_exact_material_scope_in_transaction(
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
                 JOIN collection_work_order_lease l ON l.work_order_ref=w.work_order_ref \
                 WHERE w.target_ref=$1 AND w.lane=$2 AND scope.content_public_ref=ANY($3) \
                   AND l.released_at IS NULL AND l.expires_at > scope_001_now())",
        )
        .bind(target_ref)
        .bind(lane)
        .bind(&content_refs)
        .fetch_one(&mut **transaction)
        .await?
    };

    let (authorization_ref, authorization_failure) = if let Some((authorization_ref, max_targets)) =
        authorization
    {
        let target_count: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT work_order.target_ref) \
             FROM collection_work_order work_order \
             JOIN collection_admission_decision decision USING(decision_ref) \
             WHERE decision.authorization_ref=$1 AND work_order.target_ref<>$2",
        )
        .bind(authorization_ref)
        .bind(target_ref)
        .fetch_one(&mut **transaction)
        .await?;
        if max_targets.is_some_and(|limit| target_count >= i64::from(limit)) {
            (None, Some(AuthorizationBoundaryFailure::TargetLimitReached))
        } else {
            (Some(authorization_ref), None)
        }
    } else {
        let status: (bool, bool) = sqlx::query_as(
                "SELECT \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND revoked_at IS NULL AND expires_at>scope_001_now() \
                           AND purpose<>$4 \
                           AND ($5::uuid IS NULL OR authorization_ref=$5) \
                           AND ($6::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $6)), \
                 EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                         WHERE platform=$1 AND target_kind=$2 AND lane=$3 \
                           AND ($5::uuid IS NULL OR authorization_ref=$5) \
                           AND ($6::integer=0 OR max_works_per_target IS NULL OR max_works_per_target >= $6))",
            )
            .bind(platform)
            .bind(target_kind)
            .bind(lane)
            .bind(purpose)
            .bind(required_authorization_ref)
            .bind(i32::try_from(material_targets.len()).unwrap_or(i32::MAX))
            .fetch_one(&mut **transaction)
            .await?;
        let failure = if status.0 {
            AuthorizationBoundaryFailure::PurposeMismatch
        } else if status.1 {
            AuthorizationBoundaryFailure::ExpiredOrRevoked
        } else {
            AuthorizationBoundaryFailure::Missing
        };
        (None, Some(failure))
    };

    let capacity = establish_capacity(
        transaction,
        platform,
        target_kind,
        lane,
        Some(target_ref),
        material_targets,
    )
    .await?;

    let monitor_rule_revision_ref = if requested_by == "agent" && lane == "patrol" {
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
            capacity: capacity.capacity.clone(),
            stop_conditions_expressible: true,
        },
        capacity,
        monitor_rule_revision_ref,
    })
}

/// Find a currently executable lease only when its frozen authorization and full material policy
/// are identical to the requested scope.  A shared work ID or a shared content ID is deliberately
/// insufficient: changing comment/reply bounds or media/OCR/ASR policy changes what execution is
/// authorized to do.
pub(crate) async fn live_lease_for_exact_material_scope_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    lane: &str,
    authorization_ref: Uuid,
    material_targets: &[MaterialDeepeningTarget],
) -> Result<Option<Uuid>, sqlx::Error> {
    let rows: Vec<(Uuid, Uuid, i32, i32, bool, bool, bool)> = sqlx::query_as(
        "SELECT lease.lease_ref,scope.content_public_ref,scope.comment_limit, \
                scope.reply_expand_limit,scope.acquire_media,scope.allow_ocr,scope.allow_asr \
         FROM collection_work_order work_order \
         JOIN collection_admission_decision decision \
           ON decision.decision_ref=work_order.decision_ref \
         JOIN collection_work_order_lease lease \
           ON lease.work_order_ref=work_order.work_order_ref \
         JOIN collection_work_order_material_target scope \
           ON scope.work_order_ref=work_order.work_order_ref \
         WHERE work_order.target_ref=$1 AND work_order.lane=$2 \
           AND decision.authorization_ref=$3 \
           AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
         ORDER BY lease.lease_ref,scope.ordinal",
    )
    .bind(target_ref)
    .bind(lane)
    .bind(authorization_ref)
    .fetch_all(&mut **transaction)
    .await?;

    let requested = normalized_material_scope(material_targets);
    let mut candidates: BTreeMap<Uuid, Vec<MaterialDeepeningTarget>> = BTreeMap::new();
    for (
        lease_ref,
        content_public_ref,
        comment_limit,
        reply_expand_limit,
        acquire_media,
        allow_ocr,
        allow_asr,
    ) in rows
    {
        candidates
            .entry(lease_ref)
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
    Ok(candidates.into_iter().find_map(|(lease_ref, scope)| {
        (normalized_material_scope(&scope) == requested).then_some(lease_ref)
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
    station_ref: Option<Uuid>,
    installation_ref: Option<Uuid>,
    account_ref: Option<Uuid>,
    eligibility_ref: Option<Uuid>,
    monitor_rule_revision_ref: Option<Uuid>,
    requested_by: &str,
) -> Result<Uuid, sqlx::Error> {
    let work_order_ref = Uuid::new_v4();
    let max_works = max_works_for(transaction, authorization_ref).await?;
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, station_ref, \
             installation_ref,account_ref,eligibility_ref,monitor_rule_revision_ref,stop_conditions) \
         VALUES ($1, $2, $3, $4, $5, $6, $7,$8,$9,$10,$11)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(max_works)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .bind(eligibility_ref)
    .bind(monitor_rule_revision_ref)
    .bind(json!({
        "maximumQuota": max_works,
        // A quota's shortfall is not a set of real objects (contract §3.1), so the order
        // states what stops it rather than what it expects to find.
        "stopOn": ["maximum_quota", "surface_ended", "risk_stop", "time_budget"],
    }))
    .execute(&mut **transaction)
    .await?;

    // 只有深度建档会推进生命周期。**巡检不改状态**：它是一个已建档目标的常规动作，
    // 每跑一次就改一次状态，会把「这个目标处于什么阶段」变成「它最近被派过一次」。
    if lane == "deep_archive" {
        let moved = sqlx::query(
            "UPDATE collection_observation_target \
             SET lifecycle_state = 'archiving', lifecycle_changed_at = scope_001_now() \
             WHERE target_ref = $1 AND lifecycle_state='pending_decision'",
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
         ORDER BY work_order.created_at,work_order.work_order_ref \
         LIMIT 1 FOR UPDATE OF work_order",
    )
    .bind(target_ref)
    .bind(PROGRESSIVE_ARCHIVE_VERSION.to_string())
    .fetch_optional(&mut **transaction)
    .await
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
    monitoring_enabled: bool,
    valid_for_minutes: i32,
) -> Result<ProgressiveAdvance, RequestLeaseError> {
    let live_material_scope: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
           SELECT 1 FROM collection_work_order candidate \
           JOIN collection_work_order_material_target scope USING (work_order_ref) \
           JOIN collection_work_order_lease lease USING (work_order_ref) \
           WHERE candidate.target_ref=$1 AND candidate.lane='deep_archive' \
             AND lease.released_at IS NULL AND lease.expires_at>scope_001_now())",
    )
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await
    .map_err(AcquisitionChainError::from)?;
    if live_material_scope {
        return Ok(ProgressiveAdvance::Skipped("detail_batch_in_flight"));
    }

    let content_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT discovered.content_public_ref FROM ( \
           SELECT finding.content_public_ref,min(finding.created_at) AS first_seen \
           FROM linggan_material_discovery_finding finding \
           JOIN linggan_runtime_capture_package package USING (package_ref) \
           JOIN linggan_runtime_submission_receipt receipt USING (package_ref) \
           JOIN linggan_runtime_record_disposition disposition \
             ON disposition.package_ref=finding.package_ref \
            AND disposition.record_ordinal=finding.record_ordinal \
           JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
           JOIN collection_work_order_lease lease USING (lease_ref) \
           JOIN collection_work_order source_order USING (work_order_ref) \
           WHERE source_order.target_ref=$1 \
             AND finding.discovery_kind='profile_discovery' \
             AND (source_order.lane='deep_archive' OR $2) \
             AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
             AND receipt.material_admission='ACCEPTED' \
             AND disposition.disposition <> 'quarantined' \
             AND NOT EXISTS ( \
               SELECT 1 FROM linggan_material_content_detail detail \
               WHERE detail.content_public_ref=finding.content_public_ref) \
             AND NOT EXISTS ( \
               SELECT 1 FROM collection_work_order scoped_order \
               JOIN collection_work_order_material_target scope USING (work_order_ref) \
               JOIN collection_work_order_lease scoped_lease USING (work_order_ref) \
               WHERE scoped_order.target_ref=$1 \
                 AND scope.content_public_ref=finding.content_public_ref \
                 AND scoped_lease.released_at IS NULL \
                 AND scoped_lease.expires_at>scope_001_now()) \
           GROUP BY finding.content_public_ref \
         ) discovered \
         ORDER BY discovered.first_seen,discovered.content_public_ref \
         LIMIT $3",
    )
    .bind(target_ref)
    .bind(monitoring_enabled)
    .bind(PROGRESSIVE_ARCHIVE_BATCH_SIZE)
    .fetch_all(&mut **transaction)
    .await
    .map_err(AcquisitionChainError::from)?;
    if content_refs.is_empty() {
        return Ok(ProgressiveAdvance::Skipped("no_missing_accepted_work"));
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
    let lease =
        issue_work_order_lease_in_transaction(transaction, work_order_ref, valid_for_minutes)
            .await?;
    Ok(ProgressiveAdvance::Outcome(RequestLeaseOutcome {
        request,
        lease: Some(lease),
    }))
}

/// Advance versioned creator dossier plans by one small, frozen material batch per target.
///
/// The scheduler never downloads or parses platform data. It only turns already accepted,
/// target-scoped directory facts into the next explicitly bounded Work Order. Re-running it is
/// safe: the target row lock and the live-scope exclusion are held in the same transaction that
/// writes the child scope and Lease.
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
                  row_number() OVER (PARTITION BY work_order.target_ref \
                                     ORDER BY work_order.created_at,work_order.work_order_ref) AS root_rank \
           FROM collection_work_order work_order \
           JOIN collection_admission_decision decision USING (decision_ref) \
           JOIN collection_acquisition_request request USING (request_ref) \
           JOIN collection_observation_target target USING (target_ref) \
           WHERE work_order.lane='deep_archive' \
             AND work_order.stop_conditions #>> '{progressiveArchive,version}'=$1 \
             AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
             AND decision.authorization_ref IS NOT NULL \
             AND target.lifecycle_state <> 'dismissed') \
         SELECT target_ref,work_order_ref,purpose FROM roots WHERE root_rank=1 \
         ORDER BY created_at,work_order_ref LIMIT 50",
    )
    .bind(PROGRESSIVE_ARCHIVE_VERSION.to_string())
    .fetch_all(database.pool())
    .await
    .map_err(AcquisitionChainError::from)?;

    let mut summary = ProgressiveArchiveTickSummary::default();
    for (target_ref, root_work_order_ref, purpose) in plans {
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
        .bind(target_ref)
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
                .push((target_ref, "target_not_found".to_owned()));
            continue;
        };
        if lifecycle_state == "dismissed" {
            transaction
                .rollback()
                .await
                .map_err(AcquisitionChainError::from)?;
            summary
                .skipped
                .push((target_ref, "target_dismissed".to_owned()));
            continue;
        }
        let authorization_ref = match progressive_authorization_in_transaction(
            &mut transaction,
            &platform,
            &target_kind,
            &purpose,
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
                    .push((target_ref, "authorization_expired_or_too_small".to_owned()));
                continue;
            }
        };
        let advance = advance_progressive_archive_in_transaction(
            &mut transaction,
            target_ref,
            root_work_order_ref,
            authorization_ref,
            &purpose,
            "agent",
            monitoring_enabled,
            180,
        )
        .await?;
        match advance {
            ProgressiveAdvance::Skipped(reason) => {
                transaction
                    .rollback()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary.skipped.push((target_ref, reason.to_owned()));
            }
            ProgressiveAdvance::Outcome(outcome) if outcome.lease.is_some() => {
                transaction
                    .commit()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary.dispatched.push(target_ref);
            }
            ProgressiveAdvance::Outcome(outcome) => {
                let reason = outcome.request.reason_code.to_owned();
                transaction
                    .commit()
                    .await
                    .map_err(AcquisitionChainError::from)?;
                summary.skipped.push((target_ref, reason));
            }
        }
    }
    Ok(summary)
}
