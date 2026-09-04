//! COLLECTION-001 · 把一张工单变成一次有界、可撤销的执行许可。
//!
//! 发租**不是执行**。这里不访问任何平台，也不通知任何插件；它只写下「谁、在什么范围内、
//! 到什么时候为止，被允许执行这张工单」。插件是否真的去跑，是后面的事。
//!
//! 授权在发租那一刻**重新检查**，而不是沿用准入时的结论：准入与执行之间隔着时间，
//! 工位可能已经掉线、授权可能已经撤销、风险暂停可能已经生效。

use crate::collection_control::{
    creator_baseline_qualified, required_capabilities_for, revalidate_frozen_capacity_in,
};
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
    #[error("the work order did not freeze an installation and observation account")]
    FrozenControlMissing,
    #[error("collection control closed lease issuance: {reason_code}")]
    ControlBlocked { reason_code: String },
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
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_work_order_lease') IS NOT NULL \
                AND to_regclass('collection_work_order_lease_task') IS NOT NULL",
    )
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
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
    Option<bool>,
    String,
    String,
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
    installation_ref: Uuid,
    account_ref: Uuid,
    monitor_rule_revision_ref: Option<Uuid>,
    active_monitor_rule_revision_ref: Option<Uuid>,
    active_rule_automatic_enabled: Option<bool>,
    lifecycle_state: String,
    requested_by: String,
}

struct MaterialTarget {
    content_external_id: String,
    comment_limit: i32,
    reply_expand_limit: i32,
    acquire_media: bool,
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
    let mut transaction = database.pool().begin().await?;
    // 先把过期租约收回。过期的租约不该挡住新租约，而「它是正常到期还是一直挂着没人管」
    // 必须留下记录才分得清。和接下来的 live-lease 判定放在同一事务，避免在两次读取之间
    // 由另一个请求看见不同的执行事实。
    expire_lapsed_leases_in_transaction(&mut transaction).await?;
    let issued =
        issue_work_order_lease_in_transaction(&mut transaction, work_order_ref, valid_for_minutes)
            .await?;
    transaction.commit().await?;
    Ok(issued)
}

/// The transaction-aware issue path.  It deliberately does not open or commit a transaction:
/// callers that create a Work Order and lease it as one domain operation keep the target lock,
/// admission decision, frozen material scope, lease, and scheduled tasks all-or-nothing.
pub(crate) async fn issue_work_order_lease_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    valid_for_minutes: i32,
) -> Result<IssuedLease, LeaseError> {
    let subject = load_subject(transaction, work_order_ref).await?;
    reject_if_already_leased(transaction, work_order_ref).await?;
    reject_if_authorization_lapsed(transaction, &subject).await?;
    reject_if_rule_changed(&subject)?;

    // Busy is a cross-lease predicate. Serializing on the exact frozen account makes the later
    // recheck a hard exclusion: two concurrent issuers cannot both observe the account as idle.
    sqlx::query(
        "SELECT account_ref FROM platform_observation_account WHERE account_ref=$1 FOR UPDATE",
    )
    .bind(subject.account_ref)
    .execute(&mut **transaction)
    .await?;

    let material_targets = load_material_targets(transaction, work_order_ref).await?;
    let required_capabilities = required_capabilities_for(
        &subject.target_kind,
        &subject.lane,
        !material_targets.is_empty(),
        material_targets.iter().any(|target| target.acquire_media),
    );
    let capacity = revalidate_frozen_capacity_in(
        transaction,
        &subject.platform,
        &subject.lane,
        &required_capabilities,
        subject.station_ref,
        subject.installation_ref,
        subject.account_ref,
        None,
        true,
    )
    .await?;
    if !matches!(
        capacity.capacity,
        linggan_contracts::Capacity::Available { .. }
    ) {
        return Err(LeaseError::ControlBlocked {
            reason_code: capacity.capacity.reason_code().to_owned(),
        });
    }
    let tasks = expand_into_tasks(&subject, &material_targets)?;
    for task in &tasks {
        insert_scheduled_task(transaction, task).await?;
    }

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
    .fetch_one(&mut **transaction)
    .await?;
    insert_lease_tasks(transaction, lease_ref, &tasks).await?;

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

/// 在 Package 与 Receipt 的同一事务中完成 scheduled task。
///
/// 按 task 找租约而不是按工单：插件交回来的只有 task 与 attempt，它不知道自己属于哪张
/// 工单，也不该知道。调用者已经在同一事务内锁定并复核 live lease 与领取安装；这里不得
/// 自行提交，否则 Package 已接纳但 lease 未收尾的断点会再次出现。
pub(crate) async fn complete_lease_for_task_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let completed: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "UPDATE collection_work_order_lease_task task \
         SET execution_state = 'completed', completed_at = scope_001_now() \
         FROM collection_work_order_lease lease, collection_work_order work_order \
         WHERE task.task_id = $1 \
           AND task.execution_state = 'in_progress' \
           AND lease.lease_ref = task.lease_ref \
           AND lease.released_at IS NULL \
           AND work_order.work_order_ref = lease.work_order_ref \
         RETURNING lease.lease_ref, work_order.target_ref, work_order.lane",
    )
    .bind(task_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((lease_ref, target_ref, lane)) = completed else {
        return Ok(false);
    };
    let all_completed: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM collection_work_order_lease_task \
             WHERE lease_ref = $1 AND execution_state <> 'completed')",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if !all_completed {
        return Ok(true);
    }
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = 'completed' \
         WHERE lease_ref = $1 AND released_at IS NULL",
    )
    .bind(lease_ref)
    .execute(&mut **transaction)
    .await?;
    if lane == "patrol" {
        sqlx::query(
            "UPDATE collection_observation_target \
             SET last_patrol_succeeded_at = scope_001_now() WHERE target_ref = $1",
        )
        .bind(target_ref)
        .execute(&mut **transaction)
        .await?;
    } else if lane == "deep_archive" {
        if !creator_baseline_qualified(transaction, target_ref).await? {
            // Task/Package/Receipt completion remains durable, but empty, zero, unknown,
            // scan-limited or quarantined coverage is not a creator baseline. Keeping the target
            // in archiving makes the partial result explicit and recoverable.
            return Ok(true);
        }
        let monitoring_enabled: Option<bool> = sqlx::query_scalar(
            "SELECT monitoring_enabled FROM collection_observation_target \
             WHERE target_ref=$1 AND lifecycle_state='archiving' FOR UPDATE",
        )
        .bind(target_ref)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(monitoring_enabled) = monitoring_enabled {
            let to_state = if monitoring_enabled {
                "monitoring"
            } else {
                "archived"
            };
            sqlx::query(
                "UPDATE collection_observation_target SET lifecycle_state=$2, \
                     lifecycle_changed_at=scope_001_now() \
                 WHERE target_ref=$1 AND lifecycle_state='archiving'",
            )
            .bind(target_ref)
            .bind(to_state)
            .execute(&mut **transaction)
            .await?;
            sqlx::query(
                "INSERT INTO collection_observation_target_transition \
                     (transition_ref,target_ref,from_state,to_state,actor,reason_code,reason) \
                 VALUES ($1,$2,'archiving',$3,'system','baseline_receipts_completed',$4)",
            )
            .bind(Uuid::new_v4())
            .bind(target_ref)
            .bind(to_state)
            .bind(format!("租约 {lease_ref} 的全部基线步骤已接纳"))
            .execute(&mut **transaction)
            .await?;
        }
    }
    Ok(true)
}

/// 把已过期但仍标为未结束的租约收回。
///
/// 过期是时间到了这个事实，不是一次状态变更；但**必须落成记录**，否则「它是正常到期还是
/// 一直挂着没人管」在事后无法分辨。
pub async fn expire_lapsed_leases(database: &Database) -> Result<u64, LeaseError> {
    if !lease_schema_is_ready(database).await? {
        return Err(LeaseError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let expired = expire_lapsed_leases_in_transaction(&mut transaction).await?;
    transaction.commit().await?;
    Ok(expired)
}

/// Apply expiry while preserving the caller's consistency boundary.  In particular, an admission
/// that is about to decide whether a frozen scope is already executable must observe the same
/// expiry result as the lease it may issue immediately afterwards.
pub(crate) async fn expire_lapsed_leases_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<u64, sqlx::Error> {
    Ok(sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = expires_at, release_reason = 'expired' \
         WHERE released_at IS NULL AND expires_at <= scope_001_now()",
    )
    .execute(&mut **transaction)
    .await?
    .rows_affected())
}

async fn load_subject(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<LeaseSubject, LeaseError> {
    // Lock the same target row used by admission.  A pre-existing Work Order may be leased by a
    // scheduler while a person requests reobservation; without this shared serialization point,
    // each path could make its live-scope decision before seeing the other's lease.
    let row: Option<SubjectRow> = sqlx::query_as(
        "SELECT w.target_ref, w.station_ref, w.lane, w.max_works, \
                t.platform, t.target_kind, t.identity_key, d.authorization_ref, \
                w.installation_ref,w.account_ref,w.monitor_rule_revision_ref, \
                t.active_monitor_rule_revision_ref,active_rule.automatic_enabled,t.lifecycle_state, \
                request.requested_by \
         FROM collection_work_order w \
         JOIN collection_observation_target t ON t.target_ref = w.target_ref \
         JOIN collection_admission_decision d ON d.decision_ref = w.decision_ref \
         JOIN collection_acquisition_request request ON request.request_ref=d.request_ref \
         LEFT JOIN collection_monitor_rule_revision active_rule \
           ON active_rule.rule_revision_ref=t.active_monitor_rule_revision_ref \
         WHERE w.work_order_ref = $1 FOR UPDATE OF w, t",
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
    let (Some(installation_ref), Some(account_ref)) = (row.8, row.9) else {
        return Err(LeaseError::FrozenControlMissing);
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
        installation_ref,
        account_ref,
        monitor_rule_revision_ref: row.10,
        active_monitor_rule_revision_ref: row.11,
        active_rule_automatic_enabled: row.12,
        lifecycle_state: row.13,
        requested_by: row.14,
    })
}

fn reject_if_rule_changed(subject: &LeaseSubject) -> Result<(), LeaseError> {
    if subject.requested_by == "agent"
        && subject.monitor_rule_revision_ref.is_some()
        && subject.monitor_rule_revision_ref != subject.active_monitor_rule_revision_ref
    {
        return Err(LeaseError::ControlBlocked {
            reason_code: "rule_revision_changed".to_owned(),
        });
    }
    if subject.requested_by == "agent" && subject.lane == "patrol" {
        if subject.monitor_rule_revision_ref.is_none() {
            return Err(LeaseError::ControlBlocked {
                reason_code: "rule_missing".to_owned(),
            });
        }
        if subject.active_rule_automatic_enabled != Some(true)
            || subject.lifecycle_state != "monitoring"
        {
            return Err(LeaseError::ControlBlocked {
                reason_code: "monitoring_paused".to_owned(),
            });
        }
    } else if subject.requested_by == "person" && subject.lifecycle_state == "dismissed" {
        return Err(LeaseError::ControlBlocked {
            reason_code: "target_not_requestable".to_owned(),
        });
    }
    Ok(())
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
/// 普通发现序列只生成当下已经确定的步骤；它不能凭空预建逐篇详情。只有上游在工单事务中
/// 冻结了真实 Material 身份时，这里才按固定集合展开详情、媒体、评论与回复。
fn expand_into_tasks(
    subject: &LeaseSubject,
    material_targets: &[MaterialTarget],
) -> Result<Vec<ProducerTaskSpec>, LeaseError> {
    if !material_targets.is_empty() {
        let mut tasks = Vec::new();
        for material in material_targets {
            let target = json!({ "contentExternalId": material.content_external_id });
            tasks.push(build_task_spec(
                subject,
                "content_detail",
                target.clone(),
                1,
                json!("not_requested"),
                json!("not_requested"),
            )?);
            if material.acquire_media {
                tasks.push(build_task_spec(
                    subject,
                    "media_slots",
                    target.clone(),
                    1,
                    json!("not_requested"),
                    json!("slots"),
                )?);
            }
            tasks.push(build_task_spec(
                subject,
                "comments",
                target.clone(),
                material.comment_limit,
                json!(material.comment_limit),
                json!("not_requested"),
            )?);
            if material.reply_expand_limit > 0 {
                let reply_target = json!({
                    "contentExternalId": material.content_external_id,
                    "replyExpandLimit": material.reply_expand_limit,
                });
                tasks.push(build_task_spec(
                    subject,
                    "replies",
                    reply_target,
                    material.comment_limit,
                    json!(material.comment_limit),
                    json!("not_requested"),
                )?);
            }
        }
        return Ok(tasks);
    }

    let steps: Vec<(&str, Value, i32)> = match (subject.target_kind.as_str(), subject.lane.as_str())
    {
        ("creator", "deep_archive") => vec![
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
        // Recurring creator patrols only need the current discovery surface. Re-reading the
        // stable profile every cycle adds platform load without improving change detection.
        ("creator", _) => vec![(
            "profile_discovery",
            json!({ "authorExternalId": subject.identity_key }),
            subject.max_works,
        )],
        _ => vec![(
            "discovery_search",
            json!({ "query": subject.identity_key }),
            subject.max_works,
        )],
    };

    steps
        .into_iter()
        .map(|(capability, target, quota)| {
            build_task_spec(
                subject,
                capability,
                target,
                quota,
                json!("not_requested"),
                json!("not_requested"),
            )
        })
        .collect()
}

/// 逐篇详情为什么不在发租时生成。
pub const DETAIL_STEP_DEFERRED_REASON: &str =
    "普通发现任务不会自动深化；只有工单已经冻结真实作品标识时才会展开逐篇详情";

fn build_task_spec(
    subject: &LeaseSubject,
    capability: &str,
    target: Value,
    quota: i32,
    comment_limit: Value,
    acquire_media: Value,
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
        // 每条任务仍只请求一个能力；没有请求的维度显式写 not_requested。
        "commentLimit": comment_limit,
        "acquireMedia": acquire_media,
        "riskPolicy": SERVER_LEASED_RISK_POLICY,
        // time_budget 就是租约：租约到期即止损，执行端不得自行放宽。
        "stopConditions": ["maximum_quota", "surface_ended", "time_budget"],
    });
    parse_producer_task_spec(&raw.to_string())
        .map_err(|error| LeaseError::TaskSpecInvalid(error.to_string()))
}

async fn load_material_targets(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<Vec<MaterialTarget>, LeaseError> {
    let rows: Vec<(String, i32, i32, bool)> = sqlx::query_as(
        "SELECT content.content_external_id,target.comment_limit,target.reply_expand_limit, \
                target.acquire_media \
         FROM collection_work_order_material_target target \
         JOIN linggan_material_content content ON content.public_ref=target.content_public_ref \
         WHERE target.work_order_ref=$1 ORDER BY target.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| MaterialTarget {
            content_external_id: row.0,
            comment_limit: row.1,
            reply_expand_limit: row.2,
            acquire_media: row.3,
        })
        .collect())
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

async fn insert_lease_tasks(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lease_ref: Uuid,
    tasks: &[ProducerTaskSpec],
) -> Result<(), LeaseError> {
    for (index, task) in tasks.iter().enumerate() {
        sqlx::query(
            "INSERT INTO collection_work_order_lease_task \
                 (lease_ref, task_id, sequence_no, execution_state) \
             VALUES ($1, $2, $3, 'pending')",
        )
        .bind(lease_ref)
        .bind(task.task_id())
        .bind(i32::try_from(index + 1).map_err(|error| {
            LeaseError::TaskSpecInvalid(format!("lease task sequence overflow: {error}"))
        })?)
        .execute(&mut **transaction)
        .await?;
    }
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

#[cfg(test)]
mod tests {
    use super::{LeaseSubject, MaterialTarget, expand_into_tasks};
    use uuid::Uuid;

    fn subject() -> LeaseSubject {
        LeaseSubject {
            target_ref: Uuid::new_v4(),
            station_ref: Uuid::new_v4(),
            lane: "material_deepening".to_owned(),
            max_works: 12,
            platform: "xhs".to_owned(),
            target_kind: "creator".to_owned(),
            identity_key: "creator-fixture".to_owned(),
            authorization_ref: Some(Uuid::new_v4()),
            installation_ref: Uuid::new_v4(),
            account_ref: Uuid::new_v4(),
            monitor_rule_revision_ref: None,
            active_monitor_rule_revision_ref: None,
            active_rule_automatic_enabled: None,
            lifecycle_state: "archived".to_owned(),
            requested_by: "person".to_owned(),
        }
    }

    #[test]
    fn fixed_material_scope_expands_to_one_capability_per_bounded_step() {
        let tasks = expand_into_tasks(
            &subject(),
            &[MaterialTarget {
                content_external_id: "note-fixture".to_owned(),
                comment_limit: 20,
                reply_expand_limit: 2,
                acquire_media: true,
            }],
        )
        .expect("fixed material tasks must be valid");

        let capabilities = tasks
            .iter()
            .map(|task| {
                task.raw()["capabilitiesRequested"][0]
                    .as_str()
                    .expect("one string capability")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            capabilities,
            ["content_detail", "media_slots", "comments", "replies"]
        );
        assert!(tasks.iter().all(|task| {
            task.raw()["target"]["contentExternalId"] == "note-fixture"
                && task.raw()["capabilitiesRequested"]
                    .as_array()
                    .is_some_and(|values| values.len() == 1)
        }));
        assert_eq!(tasks[2].raw()["commentLimit"], 20);
        assert_eq!(tasks[3].raw()["target"]["replyExpandLimit"], 2);
    }

    #[test]
    fn fixed_material_policy_can_omit_media_and_reply_steps_without_changing_scope() {
        let tasks = expand_into_tasks(
            &subject(),
            &[MaterialTarget {
                content_external_id: "note-text-only".to_owned(),
                comment_limit: 10,
                reply_expand_limit: 0,
                acquire_media: false,
            }],
        )
        .expect("text-only fixed material tasks must be valid");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].raw()["capabilitiesRequested"][0], "content_detail");
        assert_eq!(tasks[1].raw()["capabilitiesRequested"][0], "comments");
    }
}
