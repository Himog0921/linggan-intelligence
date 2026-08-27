//! COLLECTION-001 · Persisting the four-stage acquisition chain.
//!
//! Request → Authorization → Admission → Work Order. Each stage is written separately so
//! that no earlier stage can be read as a later one having succeeded (INV-36).
//!
//! Nothing here reaches a platform. A Work Order row is a written instruction; execution is
//! a later stage that does not exist yet.

use linggan_contracts::{AdmissionFacts, AdmissionOutcome, Capacity, decide_admission};
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
        // 准入认定了哪台工位，工单就记哪台。没有这一步，每日额度算不出来。
        let station_ref = facts
            .capacity
            .station_ref()
            .and_then(|value| Uuid::parse_str(value).ok());
        Some(
            write_work_order(
                &mut transaction,
                decision_ref,
                target_ref,
                lane,
                authorization_ref,
                station_ref,
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

    let capacity = establish_capacity(transaction, platform, target_kind, lane).await?;

    Ok(AdmissionFacts {
        authorization_ref: authorization_ref.map(|value| value.to_string()),
        in_flight_work_exists: in_flight,
        // No archive exists yet, so no need can already be satisfied. This becomes a real
        // query once archiving produces results.
        need_already_satisfied: false,
        capacity,
        stop_conditions_expressible: true,
    })
}

/// What each lane actually needs a plugin to be able to do.
///
/// These strings are the plugin's own capability vocabulary, taken from what the Linggan
/// browser path really implements — not invented for this check. Requiring a capability the
/// plugin never declares would make the gate unpassable; inventing a name the plugin happens
/// to echo back would make it theatre.
fn required_capabilities(target_kind: &str, lane: &str) -> &'static [&'static str] {
    match (target_kind, lane) {
        // Deep archiving a creator means: who they are, which works exist, and each work.
        ("creator", "deep_archive") => &["author_profile", "profile_discovery", "content_detail"],
        ("keyword", "deep_archive") => &["discovery_search", "content_detail"],
        ("creator", _) => &["profile_discovery"],
        _ => &["discovery_search"],
    }
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
) -> Result<Capacity, sqlx::Error> {
    // 风险暂停优先：正在停的时候，工位是否充足并不重要。
    let pause: Option<String> = sqlx::query_scalar(
        "SELECT reason FROM collection_risk_pause \
         WHERE lifted_at IS NULL \
           AND (platform IS NULL OR platform = $1) \
           AND (lane IS NULL OR lane = $2) \
         ORDER BY paused_at DESC LIMIT 1",
    )
    .bind(platform)
    .bind(lane)
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some(reason) = pause {
        return Ok(Capacity::RiskPaused { reason });
    }

    // 在岗 = 已登记且未停用的工位上，有一个未被取代的插件安装。
    let staffed: Vec<(Uuid, serde_json::Value, i32)> = sqlx::query_as(
        "SELECT s.station_ref, i.capabilities, s.daily_work_quota \
         FROM execution_station s \
         JOIN plugin_installation i \
           ON i.station_ref = s.station_ref AND i.superseded_at IS NULL \
         WHERE s.retired_at IS NULL \
         ORDER BY s.registered_at",
    )
    .fetch_all(&mut **transaction)
    .await?;
    if staffed.is_empty() {
        return Ok(Capacity::NoStaffedStation);
    }

    let required = required_capabilities(target_kind, lane);
    let mut missing_for_all: Option<Vec<String>> = None;
    let mut quota_blocked: Option<(i32, i32)> = None;

    for (station_ref, capabilities, quota) in staffed {
        let declared: Vec<String> = capabilities
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let missing: Vec<String> = required
            .iter()
            .filter(|needed| !declared.iter().any(|have| have == *needed))
            .map(|needed| (*needed).to_owned())
            .collect();
        if !missing.is_empty() {
            // 记下缺得最少的那台，报给人看的应该是「最接近可用的工位差什么」。
            if missing_for_all
                .as_ref()
                .is_none_or(|current| missing.len() < current.len())
            {
                missing_for_all = Some(missing);
            }
            continue;
        }

        // 预算算的是**已下发的承诺**，不是已经采到的数量——本项目还没有任何执行。
        // 按承诺算才拦得住「一天下发三张 200 篇的工单」这种超发。
        let committed: Option<i64> = sqlx::query_scalar(
            "SELECT sum(w.max_works)::bigint FROM collection_work_order w \
             WHERE w.station_ref = $1 AND w.created_at >= date_trunc('day', scope_001_now())",
        )
        .bind(station_ref)
        .fetch_one(&mut **transaction)
        .await?;
        let committed = i32::try_from(committed.unwrap_or(0)).unwrap_or(i32::MAX);
        if committed >= quota {
            quota_blocked = Some((quota, committed));
            continue;
        }
        return Ok(Capacity::Available {
            station_ref: station_ref.to_string(),
        });
    }

    if let Some(missing) = missing_for_all {
        return Ok(Capacity::MissingCapabilities { missing });
    }
    let (quota, committed) = quota_blocked.unwrap_or((0, 0));
    Ok(Capacity::DailyQuotaCommitted { quota, committed })
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
    station_ref: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let work_order_ref = Uuid::new_v4();
    let max_works = max_works_for(transaction, authorization_ref).await?;
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref, decision_ref, target_ref, lane, max_works, station_ref, \
              stop_conditions) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(max_works)
    .bind(station_ref)
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
