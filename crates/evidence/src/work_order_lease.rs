//! COLLECTION-001 · 把一张工单变成一次有界、可撤销的执行许可。
//!
//! 发租**不是执行**。这里不访问任何平台，也不通知任何插件；它只写下「谁、在什么范围内、
//! 到什么时候为止，被允许执行这张工单」。插件是否真的去跑，是后面的事。
//!
//! 授权在发租那一刻**重新检查**，而不是沿用准入时的结论：准入与执行之间隔着时间，
//! 工位可能已经掉线、授权可能已经撤销、风险暂停可能已经生效。

use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    #[error("work order lease schema is not applied")]
    SchemaUnavailable,
    #[error("no work order with that reference")]
    UnknownWorkOrder,
    #[error("that work order already has a live lease")]
    AlreadyLeased,
    #[error("the work order names no station, so there is nothing to lease to")]
    NoStation,
    #[error("that station no longer has a live plugin installation")]
    StationUnavailable,
    #[error("the authorization behind this work order is no longer valid")]
    AuthorizationLapsed,
    #[error("a risk pause covering this work is in effect: {reason}")]
    RiskPaused { reason: String },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn lease_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT to_regclass('collection_work_order_lease') IS NOT NULL")
        .fetch_one(database.pool())
        .await
}

/// 发出的租约。
#[derive(Debug)]
pub struct IssuedLease {
    pub lease_ref: Uuid,
    /// 派发任务。目前恒为 `None`：租约是许可，派发是另一件事，而派发目前被合同挡着。
    pub task_id: Option<Uuid>,
    pub station_ref: Uuid,
    pub expires_at: String,
}

/// 发租主体的行形状：工单、工位、lane、篇数上限、平台、目标类型、目标标识、授权。
type SubjectRow = (
    Uuid,
    Option<Uuid>,
    String,
    i32,
    String,
    String,
    String,
    Option<Uuid>,
);

/// 工单发租所需的、发租那一刻的事实。
struct LeaseSubject {
    target_ref: Uuid,
    station_ref: Uuid,
    lane: String,
    max_works: i32,
    platform: String,
    target_kind: String,
    identity_key: String,
    authorization_ref: Option<Uuid>,
}

/// 给一张工单发租。
///
/// `valid_for_minutes` 必须有限：没有到期时间的租约一旦发出就再也收不回来。
pub async fn issue_work_order_lease(
    database: &Database,
    work_order_ref: Uuid,
    valid_for_minutes: i32,
) -> Result<IssuedLease, LeaseError> {
    if !lease_schema_is_ready(database).await? {
        return Err(LeaseError::SchemaUnavailable);
    }
    // 先把过期租约收回。过期的租约不该挡住新租约，而「它是正常到期还是一直挂着没人管」
    // 必须留下记录才分得清。
    expire_lapsed_leases(database).await?;

    let mut transaction = database.pool().begin().await?;

    let subject = load_subject(&mut transaction, work_order_ref).await?;
    reject_if_already_leased(&mut transaction, work_order_ref).await?;
    reject_if_risk_paused(&mut transaction, &subject).await?;
    reject_if_station_unstaffed(&mut transaction, subject.station_ref).await?;
    reject_if_authorization_lapsed(&mut transaction, &subject).await?;

    let lease_ref = Uuid::new_v4();
    let capture_identity = freeze_capture_identity(&subject);
    let expires_at: String = sqlx::query_scalar(
        "INSERT INTO collection_work_order_lease \
             (lease_ref, work_order_ref, station_ref, task_id, capture_identity, expires_at) \
         VALUES ($1, $2, $3, NULL, $4, scope_001_now() + make_interval(mins => $5)) \
         RETURNING to_char(expires_at, 'YYYY-MM-DD HH24:MI')",
    )
    .bind(lease_ref)
    .bind(work_order_ref)
    .bind(subject.station_ref)
    .bind(&capture_identity)
    .bind(valid_for_minutes)
    .fetch_one(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(IssuedLease {
        lease_ref,
        station_ref: subject.station_ref,
        expires_at,
        // 派发任务尚未生成，原因见 `dispatch_is_blocked_by`。
        task_id: None,
    })
}

/// 结束一份租约。到期与被撤销分开记：前者是正常边界，后者是有人踩了刹车。
pub async fn release_work_order_lease(
    database: &Database,
    lease_ref: Uuid,
    reason: &str,
) -> Result<(), LeaseError> {
    if !lease_schema_is_ready(database).await? {
        return Err(LeaseError::SchemaUnavailable);
    }
    let affected = sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = $2 \
         WHERE lease_ref = $1 AND released_at IS NULL",
    )
    .bind(lease_ref)
    .bind(reason)
    .execute(database.pool())
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(LeaseError::UnknownWorkOrder);
    }
    Ok(())
}

/// 把已过期但仍标为未结束的租约收回。
///
/// 过期是时间到了这个事实，不是一次状态变更；但**必须落成记录**，否则「它是正常到期还是
/// 一直挂着没人管」在事后无法分辨。
pub async fn expire_lapsed_leases(database: &Database) -> Result<u64, LeaseError> {
    if !lease_schema_is_ready(database).await? {
        return Err(LeaseError::SchemaUnavailable);
    }
    Ok(sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = expires_at, release_reason = 'expired' \
         WHERE released_at IS NULL AND expires_at <= scope_001_now()",
    )
    .execute(database.pool())
    .await?
    .rows_affected())
}

async fn load_subject(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<LeaseSubject, LeaseError> {
    let row: Option<SubjectRow> = sqlx::query_as(
        "SELECT w.target_ref, w.station_ref, w.lane, w.max_works, \
                t.platform, t.target_kind, t.identity_key, d.authorization_ref \
         FROM collection_work_order w \
         JOIN collection_observation_target t ON t.target_ref = w.target_ref \
         JOIN collection_admission_decision d ON d.decision_ref = w.decision_ref \
         WHERE w.work_order_ref = $1 FOR UPDATE OF w",
    )
    .bind(work_order_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(row) = row else {
        return Err(LeaseError::UnknownWorkOrder);
    };
    let Some(station_ref) = row.1 else {
        return Err(LeaseError::NoStation);
    };
    Ok(LeaseSubject {
        target_ref: row.0,
        station_ref,
        lane: row.2,
        max_works: row.3,
        platform: row.4,
        target_kind: row.5,
        identity_key: row.6,
        authorization_ref: row.7,
    })
}

async fn reject_if_already_leased(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<(), LeaseError> {
    let live: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_work_order_lease \
                        WHERE work_order_ref = $1 AND released_at IS NULL \
                          AND expires_at > scope_001_now())",
    )
    .bind(work_order_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if live {
        return Err(LeaseError::AlreadyLeased);
    }
    Ok(())
}

async fn reject_if_risk_paused(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject: &LeaseSubject,
) -> Result<(), LeaseError> {
    let pause: Option<String> = sqlx::query_scalar(
        "SELECT reason FROM collection_risk_pause \
         WHERE lifted_at IS NULL \
           AND (platform IS NULL OR platform = $1) \
           AND (lane IS NULL OR lane = $2) LIMIT 1",
    )
    .bind(&subject.platform)
    .bind(&subject.lane)
    .fetch_optional(&mut **transaction)
    .await?;
    match pause {
        Some(reason) => Err(LeaseError::RiskPaused { reason }),
        None => Ok(()),
    }
}

async fn reject_if_station_unstaffed(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    station_ref: Uuid,
) -> Result<(), LeaseError> {
    let staffed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM execution_station s \
                        JOIN plugin_installation i ON i.station_ref = s.station_ref \
                                                  AND i.superseded_at IS NULL \
                        WHERE s.station_ref = $1 AND s.retired_at IS NULL)",
    )
    .bind(station_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if !staffed {
        return Err(LeaseError::StationUnavailable);
    }
    Ok(())
}

async fn reject_if_authorization_lapsed(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    subject: &LeaseSubject,
) -> Result<(), LeaseError> {
    let Some(authorization_ref) = subject.authorization_ref else {
        return Err(LeaseError::AuthorizationLapsed);
    };
    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_acquisition_authorization \
                        WHERE authorization_ref = $1 AND revoked_at IS NULL \
                          AND expires_at > scope_001_now())",
    )
    .bind(authorization_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if !valid {
        return Err(LeaseError::AuthorizationLapsed);
    }
    Ok(())
}

/// 为什么租约还没有派发任务。
///
/// 试着按插件自己的任务规格合同生成派发任务时，解析器挡住了两处——挡得对，记录在此，
/// 免得下次有人绕开校验硬塞：
///
/// 1. **一个任务只能请求一个能力**（`capabilities_requested.len() != 1`）。因此创作者
///    深度建档不是一个任务，而是一串：作者档案 → 作品清单 → 逐篇详情。这是合同「只负责
///    这一小步做什么」的设计，不是缺陷；派发要先有把一张工单展开成任务序列的东西。
/// 2. **`riskPolicy` 目前只接受 `local_trusted_user_initiated`**，而服务端派发的任务
///    没有对应取值——尽管 `source` 早已允许 `scheduled`。合同在这里不自洽。把服务端派的
///    任务谎报成「本机用户发起」，等于让追责链从第一步就指向错误的人。
///
/// 补一个取值是跨插件与服务端的合同变更，需要人来定，不由实现者顺手加上。
pub const DISPATCH_BLOCKED_REASON: &str =
    "派发未接通：任务规格合同要求一个任务只请求一个能力，且没有服务端派发对应的风险策略取值";

/// 发租那一刻的执行身份。执行期间它不跟着目标或授权的后续变化漂移——事后追责依据的是
/// 当时批准的那份。
fn freeze_capture_identity(subject: &LeaseSubject) -> Value {
    json!({
        "platform": subject.platform,
        "targetRef": subject.target_ref,
        "targetKind": subject.target_kind,
        "identityKey": subject.identity_key,
        "lane": subject.lane,
        "maxWorks": subject.max_works,
        "authorizationRef": subject.authorization_ref,
    })
}
