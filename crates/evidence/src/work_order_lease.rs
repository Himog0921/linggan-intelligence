//! COLLECTION-001 · 把一张工单变成一次有界、可撤销的执行许可。
//!
//! 发租**不是执行**。这里不访问任何平台，也不通知任何插件；它只写下「谁、在什么范围内、
//! 到什么时候为止，被允许执行这张工单」。插件是否真的去跑，是后面的事。
//!
//! 授权在发租那一刻**重新检查**，而不是沿用准入时的结论：准入与执行之间隔着时间，
//! 工位可能已经掉线、授权可能已经撤销、风险暂停可能已经生效。

use linggan_contracts::{
    PRODUCER_TASK_SPEC_VERSION, ProducerTaskSpec, SERVER_LEASED_RISK_POLICY,
    parse_producer_task_spec,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
    #[error("the task specification could not be built: {0}")]
    TaskSpecInvalid(String),
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
    /// 本次发租生成的派发任务，按执行顺序排列。逐篇详情不在其中，原因见
    /// `DETAIL_STEP_DEFERRED_REASON`。
    pub task_ids: Vec<Uuid>,
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

    let tasks = expand_into_tasks(&subject)?;
    for task in &tasks {
        insert_scheduled_task(&mut transaction, task).await?;
    }

    let lease_ref = Uuid::new_v4();
    let capture_identity = freeze_capture_identity(&subject);
    let expires_at: String = sqlx::query_scalar(
        "INSERT INTO collection_work_order_lease \
             (lease_ref, work_order_ref, station_ref, task_id, capture_identity, expires_at) \
         VALUES ($1, $2, $3, $4, $5, scope_001_now() + make_interval(mins => $6)) \
         RETURNING to_char(expires_at, 'YYYY-MM-DD HH24:MI')",
    )
    .bind(lease_ref)
    .bind(work_order_ref)
    .bind(subject.station_ref)
    .bind(tasks.first().map(ProducerTaskSpec::task_id))
    .bind(&capture_identity)
    .bind(valid_for_minutes)
    .fetch_one(&mut *transaction)
    .await?;

    transaction.commit().await?;
    Ok(IssuedLease {
        lease_ref,
        station_ref: subject.station_ref,
        expires_at,
        task_ids: tasks.iter().map(ProducerTaskSpec::task_id).collect(),
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

/// 把一张工单展开成派发任务序列。
///
/// **一个任务只能请求一个能力**（任务规格合同）。深度建档因此不是一个任务，而是一串：
/// 先认清这个人是谁，再拿到他的作品清单，最后逐篇取详情。
///
/// 序列**不能一次性生成完**：`content_detail` 需要 `contentExternalId`，而那要等作品
/// 清单跑完才知道。凭空造一批占位 id 会让「已派发 200 篇」变成一句假话。因此这里只生成
/// 目标已经确定的步骤，逐篇详情等清单回来后再生成。
fn expand_into_tasks(subject: &LeaseSubject) -> Result<Vec<ProducerTaskSpec>, LeaseError> {
    let steps: Vec<(&str, Value, i32)> = match subject.target_kind.as_str() {
        "creator" => vec![
            // 作者档案只取一份，配额固定为 1，不受工单篇数上限影响。
            (
                "author_profile",
                json!({ "authorExternalId": subject.identity_key }),
                1,
            ),
            (
                "profile_discovery",
                json!({ "authorExternalId": subject.identity_key }),
                subject.max_works,
            ),
        ],
        _ => vec![(
            "discovery_search",
            json!({ "query": subject.identity_key }),
            subject.max_works,
        )],
    };

    steps
        .into_iter()
        .map(|(capability, target, quota)| build_task_spec(subject, capability, target, quota))
        .collect()
}

/// 逐篇详情为什么不在发租时生成。
pub const DETAIL_STEP_DEFERRED_REASON: &str =
    "逐篇详情要等作品清单跑完才知道每篇的标识，发租时凭空造占位 id 会让「已派发」变成假话";

fn build_task_spec(
    subject: &LeaseSubject,
    capability: &str,
    target: Value,
    quota: i32,
) -> Result<ProducerTaskSpec, LeaseError> {
    let page_type = match capability {
        "author_profile" | "profile_discovery" => "profile",
        "discovery_search" => "search_results",
        _ => "note_detail",
    };
    let raw = json!({
        "contractVersion": PRODUCER_TASK_SPEC_VERSION,
        "taskId": Uuid::new_v4(),
        // 服务端派发。它与 riskPolicy 必须配对，插件与服务端同一条规则。
        "source": "scheduled",
        "platform": subject.platform,
        "pageType": page_type,
        "target": target,
        "capabilitiesRequested": [capability],
        "maximumQuota": quota,
        // 评论与媒体不在深度建档首片范围内。显式写 not_requested，而不是省略——
        // 省略会让「没要」与「忘了写」无从分辨。
        "commentLimit": "not_requested",
        "acquireMedia": "not_requested",
        "riskPolicy": SERVER_LEASED_RISK_POLICY,
        // time_budget 就是租约：租约到期即止损，执行端不得自行放宽。
        "stopConditions": ["maximum_quota", "surface_ended", "time_budget"],
    });
    parse_producer_task_spec(&raw.to_string())
        .map_err(|error| LeaseError::TaskSpecInvalid(error.to_string()))
}

async fn insert_scheduled_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task: &ProducerTaskSpec,
) -> Result<(), LeaseError> {
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
             (task_id, task_spec_hash, task_spec, source, platform, page_type) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(task.task_id())
    .bind(sha256_hex(&task.raw().to_string()))
    .bind(task.raw())
    .bind(task.source())
    .bind(task.platform())
    .bind(task.page_type())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

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
