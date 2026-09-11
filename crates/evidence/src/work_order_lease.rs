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
use crate::directory_boundary::surface_scan_complete_sql;
use linggan_contracts::{
    PRODUCER_TASK_SPEC_VERSION, ProducerTaskSpec, SERVER_LEASED_RISK_POLICY,
    parse_producer_task_spec,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const EXPIRED_LEASE_RETRY_AFTER_SECONDS: i32 = 60;

#[derive(Debug, thiserror::Error)]
pub enum LeaseError {
    #[error("work order lease schema is not applied")]
    SchemaUnavailable,
    #[error("no work order with that reference")]
    UnknownWorkOrder,
    #[error("that work order already has a live lease")]
    AlreadyLeased,
    #[error("a work order lease must have a positive duration")]
    InvalidLeaseDuration,
    #[error("the work order names no station, so there is nothing to lease to")]
    NoStation,
    #[error("that station no longer has a live plugin installation")]
    StationUnavailable,
    #[error("the work order did not freeze an installation")]
    FrozenControlMissing,
    #[error("every step this work order froze is already done")]
    WorkOrderAlreadySatisfied,
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
                AND to_regclass('collection_work_order_lease_task') IS NOT NULL \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_name='collection_work_order' \
                              AND column_name='retry_not_before_at')",
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
    account_ref: Option<Uuid>,
    monitor_rule_revision_ref: Option<Uuid>,
    active_monitor_rule_revision_ref: Option<Uuid>,
    active_rule_automatic_enabled: Option<bool>,
    lifecycle_state: String,
    requested_by: String,
    /// 这次采集的采样口径，取自**工单冻结的那一版规则**而不是目标当前的活跃版本。
    /// 工单发出时约定的口径才是这一轮的依据；中途有人改了规则，也不该改写已发出的这一单。
    sampling: SamplingPolicy,
}

/// 一次关键词搜索的取样方式。四项都可能缺失——缺了就不往任务里写那一项，
/// 让插件按自己的默认走，而不是替它编一个数。
#[derive(Default)]
struct SamplingPolicy {
    ranking: Option<String>,
    scroll_rounds: Option<i32>,
    top_by_likes: Option<i32>,
    published_within_days: Option<i32>,
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
    if valid_for_minutes <= 0 {
        return Err(LeaseError::InvalidLeaseDuration);
    }
    let subject = load_subject(transaction, work_order_ref).await?;
    reject_if_already_leased(transaction, work_order_ref).await?;
    reject_if_authorization_lapsed(transaction, &subject).await?;
    reject_if_rule_changed(&subject)?;

    // One installation may hold only one active Lease. Serialize on its installation first so an
    // unobserved account still has a safe first task; when an identity is known, also serialize on
    // that account to preserve the cross-installation exclusion.
    sqlx::query(
        "SELECT installation_ref FROM plugin_installation WHERE installation_ref=$1 FOR UPDATE",
    )
    .bind(subject.installation_ref)
    .execute(&mut **transaction)
    .await?;
    if let Some(account_ref) = subject.account_ref {
        sqlx::query(
            "SELECT account_ref FROM platform_observation_account WHERE account_ref=$1 FOR UPDATE",
        )
        .bind(account_ref)
        .execute(&mut **transaction)
        .await?;
    }

    let material_targets = load_material_targets(transaction, work_order_ref).await?;
    let required_capabilities = required_capabilities_for(
        &subject.target_kind,
        &subject.lane,
        !material_targets.is_empty(),
        material_targets
            .iter()
            .any(|target| target.comment_limit > 0),
        material_targets
            .iter()
            .any(|target| target.reply_expand_limit > 0),
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
    // Pre-0036 callers may still create a legacy Work Order and issue a Lease
    // directly. Record that it is leased as soon as the same control checks
    // succeed, so completion/expiry/failure recovery use one queue lifecycle.
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='leased', \
             dispatch_lane=CASE WHEN dispatch_lane='legacy' \
                                THEN CASE WHEN lane='patrol' THEN 'immediate' ELSE 'batch' END \
                                ELSE dispatch_lane END, \
             scheduled_for=COALESCE(scheduled_for,scope_001_now()) \
         WHERE work_order_ref=$1 AND queue_state IN ('legacy','queued')",
    )
    .bind(work_order_ref)
    .execute(&mut **transaction)
    .await?;
    let tasks =
        remaining_steps_for_work_order(transaction, work_order_ref, &subject, &material_targets)
            .await?;
    if tasks.is_empty() {
        // Nothing left to do. Issuing an empty Lease would hand a station a permit with no work,
        // and it would sit there until it expired — putting the Work Order straight back into the
        // queue to be claimed again. Close it instead; `undo_failed_queue_claim` is guarded on
        // `queue_state='leased'`, so this completion survives the caller's rollback.
        sqlx::query(
            "UPDATE collection_work_order SET queue_state='completed' \
             WHERE work_order_ref=$1 AND queue_state='leased'",
        )
        .bind(work_order_ref)
        .execute(&mut **transaction)
        .await?;
        return Err(LeaseError::WorkOrderAlreadySatisfied);
    }
    for task in &tasks {
        insert_scheduled_task(transaction, task).await?;
    }

    let lease_ref = Uuid::new_v4();
    let capture_identity = freeze_capture_identity(&subject);
    let expires_at: String = sqlx::query_scalar(
        "INSERT INTO collection_work_order_lease \
             (lease_ref, work_order_ref, station_ref, task_id, capture_identity, expires_at) \
         VALUES ($1, $2, $3, NULL, $4, scope_001_now() + make_interval(mins => $5)) \
         RETURNING linggan_human_moment(expires_at)",
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

/// Materialise the Lease for a Work Order claimed from the shared queue.
///
/// The caller has already locked the queued Work Order and evaluated the
/// *calling* installation. Updating the frozen route and changing
/// `queued → leased` happen before task expansion, in the same transaction;
/// no scheduler can accidentally reserve a station on behalf of a plugin that
/// did not claim this order.
pub(crate) async fn claim_queued_work_order_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    station_ref: Uuid,
    installation_ref: Uuid,
    account_ref: Option<Uuid>,
    eligibility_ref: Option<Uuid>,
    valid_for_minutes: i32,
) -> Result<IssuedLease, LeaseError> {
    let claimed = sqlx::query(
        "UPDATE collection_work_order \
         SET station_ref=$2,installation_ref=$3,account_ref=$4,eligibility_ref=$5, \
             queue_state='leased' \
         WHERE work_order_ref=$1 AND queue_state='queued'",
    )
    .bind(work_order_ref)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(account_ref)
    .bind(eligibility_ref)
    .execute(&mut **transaction)
    .await?
    .rows_affected();
    if claimed != 1 {
        return Err(LeaseError::AlreadyLeased);
    }
    issue_work_order_lease_in_transaction(transaction, work_order_ref, valid_for_minutes).await
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
    let completed: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "UPDATE collection_work_order_lease_task task \
         SET execution_state = 'completed', completed_at = scope_001_now() \
         FROM collection_work_order_lease lease, collection_work_order work_order \
         WHERE task.task_id = $1 \
           AND task.execution_state = 'in_progress' \
           AND lease.lease_ref = task.lease_ref \
           AND lease.released_at IS NULL \
           AND work_order.work_order_ref = lease.work_order_ref \
         RETURNING lease.lease_ref, lease.work_order_ref, work_order.target_ref, work_order.lane",
    )
    .bind(task_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((lease_ref, work_order_ref, target_ref, lane)) = completed else {
        return Ok(false);
    };
    let all_terminal: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM collection_work_order_lease_task \
             WHERE lease_ref = $1 AND execution_state NOT IN ('completed','unavailable','blocked'))",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if !all_terminal {
        return Ok(true);
    }
    let has_non_completed_terminal: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_work_order_lease_task \
           WHERE lease_ref=$1 AND execution_state IN ('unavailable','blocked'))",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at = scope_001_now(), release_reason = $2 \
         WHERE lease_ref = $1 AND released_at IS NULL",
    )
    .bind(lease_ref)
    .bind(if has_non_completed_terminal {
        "partial"
    } else {
        "completed"
    })
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='completed' \
         WHERE work_order_ref=$1 AND queue_state='leased'",
    )
    .bind(work_order_ref)
    .execute(&mut **transaction)
    .await?;
    if lane == "patrol" && patrol_completion_qualified(transaction, lease_ref, target_ref).await? {
        sqlx::query(
            "UPDATE collection_observation_target \
             SET last_patrol_succeeded_at = scope_001_now() WHERE target_ref = $1",
        )
        .bind(target_ref)
        .execute(&mut **transaction)
        .await?;
    } else if lane == "deep_archive" {
        // 这里只推进**创作者**的状态机。关键词没有建档生命周期——`0042` 明写
        // 「Keyword observation has a monitor lifecycle, not a creator archive lifecycle」
        // 并用 CHECK 禁止关键词进入 `archiving`/`archived`，还把历史上误入这两个状态的
        // 关键词修回了 monitoring/paused。
        //
        // 关键词照样可以建档（`deep_archive` 的准入对它开放），只是**建过没建过是读取时
        // 查出来的，不是存下来的一个状态**——与本仓库对生命周期的一贯理解一致：
        // 「A lifecycle point is not a second material fact. It is a read-time combination.」
        // 判据见 `collection_control::keyword_baseline_qualified`。
        //
        // 下面这段对关键词天然是空操作：它只更新处于 `archiving` 的目标，而关键词永远
        // 不会在那个状态里。
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

/// A patrol lease completing is an operation fact; a successful patrol additionally requires a
/// target-bound, live Receipt with complete Coverage. Zero new records is valid when the producer
/// proves the surface ended. Quarantine, unknown scope and risk stops remain completed attempts,
/// but must not advance the target's last-success clock.
async fn patrol_completion_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lease_ref: Uuid,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "SELECT EXISTS ( \
           SELECT 1 FROM collection_work_order_lease lease \
           JOIN collection_work_order work_order USING(work_order_ref) \
           JOIN collection_observation_target target USING(target_ref) \
           JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
           JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
           JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
           JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
           CROSS JOIN LATERAL jsonb_array_elements( \
             CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                  THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
           WHERE lease.lease_ref=$1 AND work_order.target_ref=$2 AND work_order.lane='patrol' \
             AND package.package_kind=CASE WHEN target.target_kind='creator' \
                                           THEN 'profile_discovery' ELSE 'discovery_search' END \
             AND package.platform=task.platform \
             AND task.task_spec->'capabilitiesRequested' ? package.package_kind \
             -- 归属由 package.task_id=task.task_id 保证。此前这里还要求包的 target 包含
             -- 任务 target 的每一个键，而采样口径是下发的指令、不是回执的事实，插件只
             -- 回显身份——任务一带口径就必然为假，关键词巡检因此从未被记成成功。
             AND receipt.material_admission='ACCEPTED' \
             AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
             AND layer->>'capability'=package.package_kind \
             AND ",
        surface_scan_complete_sql!(),
        " \
             AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                             WHERE disposition.package_ref=package.package_ref \
                               AND disposition.disposition='quarantined') \
             AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                  WHERE disposition.package_ref=package.package_ref \
                    AND disposition.disposition='accepted_for_library_discovery') = \
                 COALESCE((layer->>'acquired')::integer,-1))",
    ))
    .bind(lease_ref)
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await
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
    let expired: Vec<Uuid> = sqlx::query_scalar(
        "UPDATE collection_work_order_lease \
         SET released_at = expires_at, release_reason = 'expired' \
         WHERE released_at IS NULL AND expires_at <= scope_001_now() \
         RETURNING work_order_ref",
    )
    .fetch_all(&mut **transaction)
    .await?;
    requeue_work_orders_after_release(transaction, &expired).await?;
    Ok(u64::try_from(expired.len()).unwrap_or(u64::MAX))
}

/// 释放一份租约之后，把它的工单交还给共享队列。
///
/// **每一条释放租约的路径都必须走这里。** 释放租约和交还工单是同一件事的两半：只做前一半，
/// 工单就永远停在 `leased`、名下却没有活租约——共享 claim 只找 `queued`，于是没有任何一台
/// 工位能再碰它，也没有任何巡检会回收它。它成为永久僵尸，还继续占着该来源的批量并发额度。
///
/// 2026-09-06 实测过这个后果：插件顶替（`teardown_installation`）当时只释放了租约，工单
/// `f7942504` 就此卡死；渐进建档的候选查询只排除「排队中」和「有活租约」两种状态，看不见
/// 这第三种，于是它冻结的作品被重新选进了新工单——同两篇作品因此被冻结了两次。
///
/// 过期与顶替共用同一条延迟：一个已经死掉的浏览器不该在 claim/expire 的紧循环里反复吃掉
/// 账号与平台名额。租约本身和它名下的任务都保持为不可变历史，这里只动工单的队列态。
pub(crate) async fn requeue_work_orders_after_release(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_refs: &[Uuid],
) -> Result<(), sqlx::Error> {
    if work_order_refs.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='queued',station_ref=NULL, \
             installation_ref=NULL,account_ref=NULL,eligibility_ref=NULL, \
             retry_not_before_at=GREATEST(retry_not_before_at, \
                scope_001_now()+make_interval(secs=>$2)) \
         WHERE work_order_ref=ANY($1) AND queue_state='leased' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_lease live \
                           WHERE live.work_order_ref=collection_work_order.work_order_ref \
                             AND live.released_at IS NULL)",
    )
    .bind(work_order_refs)
    .bind(EXPIRED_LEASE_RETRY_AFTER_SECONDS)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Recover historical WorkOrders whose Lease was already released but whose
/// queue state predates the invariant that a release must return the WorkOrder
/// to `queued`.  This is intentionally narrow: a still-live lease and an
/// order whose every task is terminal are never reopened.
pub async fn recover_released_orphaned_work_orders(database: &Database) -> Result<u64, LeaseError> {
    if !lease_schema_is_ready(database).await? {
        return Err(LeaseError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let recovered = recover_released_orphaned_work_orders_in_transaction(&mut transaction).await?;
    transaction.commit().await?;
    Ok(recovered)
}

pub(crate) async fn recover_released_orphaned_work_orders_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<u64, sqlx::Error> {
    let work_order_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT work_order.work_order_ref \
         FROM collection_work_order work_order \
         WHERE work_order.queue_state='leased' \
           AND EXISTS (SELECT 1 FROM collection_work_order_lease released \
                       WHERE released.work_order_ref=work_order.work_order_ref \
                         AND released.released_at IS NOT NULL) \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_lease live \
                           WHERE live.work_order_ref=work_order.work_order_ref \
                             AND live.released_at IS NULL \
                             AND live.expires_at>scope_001_now()) \
           AND EXISTS (SELECT 1 FROM collection_work_order_lease_task task \
                       JOIN collection_work_order_lease lease USING(lease_ref) \
                       WHERE lease.work_order_ref=work_order.work_order_ref \
                         AND task.execution_state IN ('pending','in_progress')) \
         ORDER BY work_order.created_at,work_order.work_order_ref \
         LIMIT 100 FOR UPDATE OF work_order SKIP LOCKED",
    )
    .fetch_all(&mut **transaction)
    .await?;
    requeue_work_orders_after_release(&mut *transaction, &work_order_refs).await?;
    Ok(u64::try_from(work_order_refs.len()).unwrap_or(u64::MAX))
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
    let Some(installation_ref) = row.8 else {
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
        account_ref: row.9,
        monitor_rule_revision_ref: row.10,
        active_monitor_rule_revision_ref: row.11,
        active_rule_automatic_enabled: row.12,
        lifecycle_state: row.13,
        requested_by: row.14,
        sampling: load_sampling_policy(transaction, row.10).await?,
    })
}

/// 读工单冻结的那一版规则里的采样口径。
///
/// 单独一次查询而不是并进上面那条：主查询的列已经到了 sqlx 元组绑定的上限，更重要的是
/// 口径本来就是规则的属性，从规则读它比从工单那条大 JOIN 里捎带出来更说得清。
///
/// 工单没有绑定规则（人工发起的一次性请求）时没有口径，返回全空——那是真话，不是缺省值。
/// 一次关键词搜索要告诉插件的东西。
///
/// **`query` 此前传的是身份键原文**（`adhd::comprehensive`）——那是「词 + 排序」拼出来的
/// 内部标识，不是一个能拿去搜索框搜的词。插件因此从来不按它搜，只读当前页面上碰巧
/// 可见的内容，采回来的东西没有可信的采样口径。这里把它拆开：`query` 只放真正的词，
/// 排序与取样方式各自成字段。
///
/// 保留 `query` 这个键名而不是改叫 `keyword`：`discovery_search` 的契约校验要求它非空，
/// 且旧版插件仍靠它搜索。新增的几项是纯追加，认不出它们的插件行为不变。
///
/// 口径缺失的项一概不写进去，让插件按自己的默认走——编一个数会让回执里出现一份
/// 从未被约定过的口径。
fn keyword_search_target(subject: &LeaseSubject) -> Value {
    let mut target = serde_json::Map::new();
    target.insert(
        "query".to_owned(),
        json!(search_term(
            &subject.identity_key,
            subject.sampling.ranking.as_deref()
        )),
    );
    for (key, value) in sampling_directives(&subject.sampling) {
        if let Some(value) = value {
            target.insert(key.to_owned(), value);
        }
    }
    Value::Object(target)
}

/// 关键词任务 target 里属于**采样口径**的键——即「怎么取」，而不是「取谁」。
///
/// 这份名单是唯一真源：[`keyword_search_target`] 按它下发，
/// [`crate::material_contract_validation::task_package_binding_valid`] 按它豁免。
/// 加新口径时改这一处，两边同时生效；`sampling_directives_are_all_declared_exempt`
/// 会在漏改时变红。
pub(crate) const SAMPLING_DIRECTIVE_KEYS: [&str; 4] = [
    "ranking",
    "scrollRounds",
    "topByLikes",
    "publishedWithinDays",
];

/// 口径缺失的项返回 `None`，调用方不写进去，让插件按自己的默认走。
fn sampling_directives(sampling: &SamplingPolicy) -> [(&'static str, Option<Value>); 4] {
    [
        (
            "ranking",
            sampling.ranking.as_deref().map(|value| json!(value)),
        ),
        (
            "scrollRounds",
            sampling.scroll_rounds.map(|value| json!(value)),
        ),
        (
            "topByLikes",
            sampling.top_by_likes.map(|value| json!(value)),
        ),
        (
            "publishedWithinDays",
            sampling.published_within_days.map(|value| json!(value)),
        ),
    ]
}

/// 首次建档要下发给插件的东西。
///
/// 与巡检口径的区别只有三处，但每一处都是建档之所以是建档的原因：
/// **不限发布时间**（历史全要）、**不设取前 N**（采到多少交多少）、**排序固定按点赞**
/// （历史里值得挖的就是高赞那批）。
///
/// 下拉次数给足：建档是一次性的，把这个词能翻到的表面翻完，而不是翻三屏就停。真正的
/// 上限由工单的篇数（授权给的 200）和插件自己的轮次上限一起兜住，不靠这个数收口。
///
/// **不设取前 N 是有意的**：截断只会把第 N+1 名之后的永久扔掉，而「点赞最高的前 100」
/// 是读取时按点赞排序就能得到的事，不必在采集时先砍一刀。
fn keyword_archive_target(subject: &LeaseSubject) -> Value {
    json!({
        "query": search_term(&subject.identity_key, Some(ARCHIVE_RANKING)),
        "ranking": ARCHIVE_RANKING,
        "scrollRounds": ARCHIVE_SCROLL_ROUNDS,
    })
}

/// 建档按点赞排序。这是建档的目的，不是可配项。
pub(crate) const ARCHIVE_RANKING: &str = "most_liked";

/// 建档下发的下拉次数。插件会按自己的策略把它放大成实际轮次上限，采够工单篇数即停。
const ARCHIVE_SCROLL_ROUNDS: i32 = 10;

/// 从关键词目标的身份键里取出真正的搜索词。
///
/// 身份键由 `TargetIdentity::keyword` 拼成 `{词}::{排序}`。优先按规则里的排序去掉后缀——
/// 两处对得上才剥，对不上就用 `rsplit_once` 兜底（排序不含 `::`，所以从右边切是对的）。
/// 两者都不成立时原样返回：宁可搜一个怪词，也不要凭空猜出半截。
fn search_term(identity_key: &str, ranking: Option<&str>) -> String {
    if let Some(ranking) = ranking.map(str::trim).filter(|value| !value.is_empty()) {
        if let Some(term) = identity_key.strip_suffix(&format!("::{ranking}")) {
            return term.to_owned();
        }
    }
    identity_key
        .rsplit_once("::")
        .map_or_else(|| identity_key.to_owned(), |(term, _)| term.to_owned())
}

async fn load_sampling_policy(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rule_revision_ref: Option<Uuid>,
) -> Result<SamplingPolicy, LeaseError> {
    let Some(rule_revision_ref) = rule_revision_ref else {
        return Ok(SamplingPolicy::default());
    };
    let row: Option<(Option<String>, Option<i32>, Option<i32>, Option<i32>)> = sqlx::query_as(
        "SELECT ranking_key,scroll_rounds,top_by_likes,published_within_days \
         FROM collection_monitor_rule_revision WHERE rule_revision_ref=$1",
    )
    .bind(rule_revision_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(
        row.map_or_else(SamplingPolicy::default, |row| SamplingPolicy {
            ranking: row.0,
            scroll_rounds: row.1,
            top_by_likes: row.2,
            published_within_days: row.3,
        }),
    )
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

/// 这张工单还剩哪些步骤没做完。
///
/// 任务在模型上属于**租约**（主键 `(lease_ref, sequence_no)`），而租约一旦释放，它名下的
/// pending 任务就永久失效——派发查询硬性要求租约仍然存活。所以重新发租只能重造一套任务，
/// 这是模型选择的结果，不是遗漏。
///
/// 但「重造」不等于「重做」。工单冻结的作品清单不会变，已完成的步骤是不可变历史；两者
/// 相减才是这次租约真正要干的事。此前这里无条件全量展开，于是每次租约过期都把整批步骤
/// 原样再跑一遍：2026-09-06 实测一张三篇作品的工单发了 9 次租约、生成 108 个步骤去完成
/// 本该 12 步的活，同一篇正文被真实重复抓取 9 次——每一次都真的访问了平台、真的扣了配额。
///
/// 去重的键是「作品 + 能力」而不是任务 id：任务 id 每次生成都是新的随机值，正因如此
/// `linggan_runtime_task` 上那条 `UNIQUE(task_spec_hash)` 约束从来拦不住重复（哈希的
/// 输入里就含着这个随机 id）。
///
/// 不带作品身份的步骤（`author_profile` / `profile_discovery` / `discovery_search`）不参与
/// 去重：它们描述的是「再看一眼当前的发现面」，本就该每轮重做。
async fn remaining_steps_for_work_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    subject: &LeaseSubject,
    material_targets: &[MaterialTarget],
) -> Result<Vec<ProducerTaskSpec>, LeaseError> {
    let finished: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT runtime.task_spec->'target'->>'contentExternalId', \
                runtime.task_spec->'capabilitiesRequested'->>0 \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order_lease_task task USING(lease_ref) \
         JOIN linggan_runtime_task runtime ON runtime.task_id = task.task_id \
         WHERE lease.work_order_ref = $1 AND task.execution_state = 'completed'",
    )
    .bind(work_order_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let done: std::collections::HashSet<(String, String)> = finished
        .into_iter()
        .filter_map(|(content, capability)| Some((content?, capability?)))
        .collect();

    Ok(expand_into_tasks(subject, material_targets)?
        .into_iter()
        .filter(|task| {
            let content = task
                .raw()
                .pointer("/target/contentExternalId")
                .and_then(Value::as_str);
            let capability = task
                .raw()
                .pointer("/capabilitiesRequested/0")
                .and_then(Value::as_str);
            match (content, capability) {
                (Some(content), Some(capability)) => {
                    !done.contains(&(content.to_owned(), capability.to_owned()))
                }
                _ => true,
            }
        })
        .collect())
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
            // `comment_limit = 0` is an explicit detail-only authorization, not an empty
            // comment result.  Creating an empty comments task would widen the Work Order.
            if material.comment_limit > 0 {
                tasks.push(build_task_spec(
                    subject,
                    "comments",
                    target.clone(),
                    material.comment_limit,
                    json!(material.comment_limit),
                    json!("not_requested"),
                )?);
            }
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
        // The first product contract for a creator patrol is intentionally
        // fixed and small: verify the author profile, then inspect a bounded
        // current directory. Historical completeness is not inferred here.
        ("creator", "patrol") => vec![
            (
                "author_profile",
                json!({ "authorExternalId": subject.identity_key }),
                1,
            ),
            (
                "profile_discovery",
                json!({ "authorExternalId": subject.identity_key }),
                subject.max_works.min(30),
            ),
        ],
        ("creator", _) => vec![(
            "profile_discovery",
            json!({ "authorExternalId": subject.identity_key }),
            subject.max_works,
        )],
        // 关键词的首次建档：把这个词的历史高赞挖一遍，**一个词只做一次**。
        //
        // 它不读监控规则的口径。规则描述的是「每周该怎么看这个词」——近 7 天、取前 20；
        // 建档要的恰恰相反：不限时间、能取多少取多少。共用一份口径时，走了建档通道拿回来
        // 的仍然是最近一周的 20 条，历史那一段等于没做。
        //
        // 排序固定为 most_liked：历史里最值得挖的就是高赞那批，这是建档的目的本身，
        // 不是一个每次都要人选的参数。
        ("keyword", "deep_archive") => vec![(
            "discovery_search",
            keyword_archive_target(subject),
            subject.max_works,
        )],
        _ => vec![(
            "discovery_search",
            keyword_search_target(subject),
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
    use linggan_contracts::ProducerTaskSpec;
    use serde_json::Value;
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
            account_ref: Some(Uuid::new_v4()),
            monitor_rule_revision_ref: None,
            active_monitor_rule_revision_ref: None,
            active_rule_automatic_enabled: None,
            lifecycle_state: "archived".to_owned(),
            requested_by: "person".to_owned(),
            sampling: super::SamplingPolicy::default(),
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

    #[test]
    fn fixed_material_policy_can_authorize_detail_without_comments() {
        let tasks = expand_into_tasks(
            &subject(),
            &[MaterialTarget {
                content_external_id: "note-detail-only".to_owned(),
                comment_limit: 0,
                reply_expand_limit: 0,
                acquire_media: false,
            }],
        )
        .expect("detail-only fixed scope must be valid");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].raw()["capabilitiesRequested"][0], "content_detail");
        assert_eq!(tasks[0].raw()["commentLimit"], "not_requested");
    }

    /// 重新发租只补未完成的步骤，不把整批重跑一遍。
    ///
    /// 任务属于租约，租约释放后它名下的 pending 任务就永久失效，所以重新发租必须重造一套。
    /// 但「重造」不等于「重做」：已完成的步骤是不可变历史，减掉它们才是这次真正要干的活。
    /// 2026-09-06 实测缺这一步的后果：一张三篇作品的工单发了 9 次租约、生成 108 个步骤去
    /// 完成本该 12 步的活，同一篇正文被真实重复抓取 9 次，每次都实扣平台配额。
    #[test]
    fn remaining_steps_skip_what_this_work_order_already_finished() {
        let subject = subject();
        let targets = [MaterialTarget {
            content_external_id: "note-fixture".to_owned(),
            comment_limit: 20,
            reply_expand_limit: 2,
            acquire_media: true,
        }];
        let all = expand_into_tasks(&subject, &targets).expect("tasks");
        assert_eq!(all.len(), 4, "一篇作品展开为四个 lane");

        let step = |task: &ProducerTaskSpec| {
            (
                task.raw()
                    .pointer("/target/contentExternalId")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                task.raw()
                    .pointer("/capabilitiesRequested/0")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            )
        };
        // 每一步都带得出「作品 + 能力」——这正是去重用的键。用任务 id 做不到：它每次生成
        // 都是新的随机值，`UNIQUE(task_spec_hash)` 也因此从来拦不住重复。
        for task in &all {
            let (content, capability) = step(task);
            assert_eq!(content.as_deref(), Some("note-fixture"));
            assert!(capability.is_some(), "每一步只请求一个能力");
        }
        let capabilities: Vec<_> = all.iter().filter_map(|task| step(task).1).collect();
        assert_eq!(
            capabilities,
            ["content_detail", "media_slots", "comments", "replies"],
        );

        // 模拟「详情已完成」：按同一个键过滤，应当只剩后三步，且顺序不变。
        let done: std::collections::HashSet<(String, String)> =
            [("note-fixture".to_owned(), "content_detail".to_owned())].into();
        let remaining: Vec<_> = all
            .iter()
            .filter(|task| match step(task) {
                (Some(content), Some(capability)) => !done.contains(&(content, capability)),
                _ => true,
            })
            .filter_map(|task| step(task).1)
            .collect();
        assert_eq!(remaining, ["media_slots", "comments", "replies"]);
    }
}

#[cfg(test)]
mod keyword_search_target_tests {
    use super::*;

    fn subject(identity_key: &str, sampling: SamplingPolicy) -> LeaseSubject {
        LeaseSubject {
            target_ref: Uuid::nil(),
            station_ref: Uuid::nil(),
            lane: "patrol".to_owned(),
            max_works: 20,
            platform: "xhs".to_owned(),
            target_kind: "keyword".to_owned(),
            identity_key: identity_key.to_owned(),
            authorization_ref: None,
            installation_ref: Uuid::nil(),
            account_ref: Some(Uuid::nil()),
            monitor_rule_revision_ref: None,
            active_monitor_rule_revision_ref: None,
            active_rule_automatic_enabled: None,
            lifecycle_state: "monitoring".to_owned(),
            requested_by: "person".to_owned(),
            sampling,
        }
    }

    /// 此前 `query` 传的是身份键原文，插件拿它根本搜不出东西。
    #[test]
    fn the_query_carries_the_term_alone_not_the_identity_key() {
        let target = keyword_search_target(&subject(
            "考研自习::most_liked",
            SamplingPolicy {
                ranking: Some("most_liked".to_owned()),
                scroll_rounds: Some(3),
                top_by_likes: Some(20),
                published_within_days: Some(7),
            },
        ));
        assert_eq!(target["query"], json!("考研自习"));
        assert_eq!(target["ranking"], json!("most_liked"));
        assert_eq!(target["scrollRounds"], json!(3));
        assert_eq!(target["topByLikes"], json!(20));
        assert_eq!(target["publishedWithinDays"], json!(7));
    }

    /// 没有口径就不写那几项，让插件按自己的默认走——编一个数会让回执里出现一份
    /// 从未被约定过的口径。`query` 仍必须是能搜的词。
    #[test]
    fn a_rule_without_a_policy_only_sends_the_term() {
        let target =
            keyword_search_target(&subject("adhd::comprehensive", SamplingPolicy::default()));
        assert_eq!(target["query"], json!("adhd"));
        for absent in [
            "ranking",
            "scrollRounds",
            "topByLikes",
            "publishedWithinDays",
        ] {
            assert!(target.get(absent).is_none(), "{absent} 不该被编出来");
        }
    }

    /// 建档与巡检读的必须是两套口径。共用一份时，走了建档通道拿回来的仍然是最近一周的
    /// 20 条——历史那一段等于没做，而这正是关键词建档存在的理由。
    #[test]
    fn archiving_a_keyword_ignores_the_patrol_sampling_policy() {
        let patrol_policy = SamplingPolicy {
            ranking: Some("most_liked".to_owned()),
            scroll_rounds: Some(3),
            top_by_likes: Some(20),
            published_within_days: Some(7),
        };
        let target = keyword_archive_target(&subject("考研自习::most_liked", patrol_policy));

        assert_eq!(target["query"], json!("考研自习"));
        assert_eq!(target["ranking"], json!(ARCHIVE_RANKING));
        // 建档要历史全量：**一天都不能限**，否则挖不到这个词真正的高赞。
        assert!(
            target.get("publishedWithinDays").is_none(),
            "建档不限发布时间"
        );
        // 也不截断：截断只会把第 N+1 名之后的永久扔掉，而「前 100」是读取时排序的事。
        assert!(target.get("topByLikes").is_none(), "建档不设取前 N");
    }

    /// 巡检那一路照旧读规则口径，不受建档改动影响。
    #[test]
    fn patrolling_a_keyword_still_follows_the_rule() {
        let target = keyword_search_target(&subject(
            "考研自习::most_liked",
            SamplingPolicy {
                ranking: Some("most_liked".to_owned()),
                scroll_rounds: Some(3),
                top_by_likes: Some(20),
                published_within_days: Some(7),
            },
        ));
        assert_eq!(target["publishedWithinDays"], json!(7));
        assert_eq!(target["topByLikes"], json!(20));
    }

    /// 名单漏改会让被漏掉的那个口径重新参与身份比对，而插件不会回显它——
    /// 于是那一类关键词采集重新整包隔离，且没有任何报错。这条测试是那件事的唯一防线。
    #[test]
    fn sampling_directives_are_all_declared_exempt() {
        let sampling = SamplingPolicy {
            ranking: Some("most_liked".to_owned()),
            scroll_rounds: Some(3),
            top_by_likes: Some(20),
            published_within_days: Some(7),
        };
        let emitted: Vec<&str> = sampling_directives(&sampling)
            .into_iter()
            .map(|(key, value)| {
                assert!(value.is_some(), "{key} 在口径填满时必须下发");
                key
            })
            .collect();
        assert_eq!(emitted, SAMPLING_DIRECTIVE_KEYS);

        let target = keyword_search_target(&subject("考研自习::most_liked", sampling));
        let mut carried: Vec<&str> = target
            .as_object()
            .expect("target is an object")
            .keys()
            .map(String::as_str)
            .filter(|key| *key != "query")
            .collect();
        carried.sort_unstable();
        let mut exempt = SAMPLING_DIRECTIVE_KEYS.to_vec();
        exempt.sort_unstable();
        assert_eq!(
            carried, exempt,
            "下发的口径键必须与豁免名单逐项一致，否则关键词采集会被整包隔离"
        );
    }

    /// 词里含 `::` 时从右边切，排序不含 `::`，所以切得对。
    #[test]
    fn a_term_containing_the_separator_still_resolves() {
        assert_eq!(search_term("c::b::latest", Some("latest")), "c::b");
        assert_eq!(search_term("c::b::latest", None), "c::b");
    }

    /// 规则里的排序与身份键对不上时不猜：原样返回好过凭空切出半截。
    #[test]
    fn a_mismatched_ranking_falls_back_instead_of_guessing() {
        assert_eq!(search_term("考研自习", Some("most_liked")), "考研自习");
    }
}
