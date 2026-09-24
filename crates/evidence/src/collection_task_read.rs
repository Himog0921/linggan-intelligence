//! Read-only task execution history for the Collection Tasks surface.
//!
//! A queue state, a producer Attempt, an immutable Package and a Submission
//! Receipt are deliberately different facts.  This projection keeps all four
//! visible together, so a page never turns "a task was claimed" into "the
//! task finished", or hides an earlier dispatch-start failure after a retry
//! later succeeds.

use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// One task and the newest execution facts that belong to it.
#[derive(Debug, Clone)]
pub struct CollectionTaskExecution {
    pub task_id: Uuid,
    pub source: String,
    pub platform: String,
    pub page_type: String,
    pub capabilities: String,
    pub created_at: String,
    pub sequence_no: Option<i32>,
    pub queue_state: Option<String>,
    pub claimed_at: Option<String>,
    /// `false` means the task belongs to a released or expired lease.  That
    /// is historical queue state, not a producer that is still executing.
    pub has_live_lease: Option<bool>,
    pub target_display_name: Option<String>,
    pub target_identity_key: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub attempt_started_at: Option<String>,
    pub package_kind: Option<String>,
    pub package_ref: Option<Uuid>,
    pub package_accepted_at: Option<String>,
    pub receipt_ref: Option<Uuid>,
    pub receipt_received_at: Option<String>,
    pub execution_effect: Option<String>,
    pub material_admission: Option<String>,
    /// The most recent pre-Attempt dispatch failure.  It remains visible even
    /// after a successful retry; an append-only failure is still a fact.
    pub last_dispatch_failure_code: Option<String>,
    pub last_dispatch_failure_at: Option<String>,
}

/// The bounded Task surface.  Counts describe this exact returned timeline,
/// not an unbounded all-history total.
#[derive(Debug, Clone)]
pub struct CollectionTaskTimeline {
    pub tasks: Vec<CollectionTaskExecution>,
    pub accepted_count: i64,
    pub active_count: i64,
    pub expired_lease_count: i64,
}

/// Read the most recently active or changed tasks.  It is a local projection:
/// this function does not claim, retry, release, or contact a platform.
pub async fn read_collection_task_timeline(
    database: &Database,
    limit: i64,
) -> Result<CollectionTaskTimeline, sqlx::Error> {
    let rows = sqlx::query(TASK_TIMELINE_SQL)
        .bind(limit.clamp(1, 200))
        .fetch_all(database.pool())
        .await?;
    let tasks = rows
        .into_iter()
        .map(|row| CollectionTaskExecution {
            task_id: row.get("task_id"),
            source: row.get("source"),
            platform: row.get("platform"),
            page_type: row.get("page_type"),
            capabilities: row.get("capabilities"),
            created_at: row.get("created_at"),
            sequence_no: row.get("sequence_no"),
            queue_state: row.get("queue_state"),
            claimed_at: row.get("claimed_at"),
            has_live_lease: row.get("has_live_lease"),
            target_display_name: row.get("target_display_name"),
            target_identity_key: row.get("target_identity_key"),
            attempt_id: row.get("attempt_id"),
            attempt_started_at: row.get("attempt_started_at"),
            package_kind: row.get("package_kind"),
            package_ref: row.get("package_ref"),
            package_accepted_at: row.get("package_accepted_at"),
            receipt_ref: row.get("receipt_ref"),
            receipt_received_at: row.get("receipt_received_at"),
            execution_effect: row.get("execution_effect"),
            material_admission: row.get("material_admission"),
            last_dispatch_failure_code: row.get("last_dispatch_failure_code"),
            last_dispatch_failure_at: row.get("last_dispatch_failure_at"),
        })
        .collect::<Vec<_>>();

    let accepted_count = tasks
        .iter()
        .filter(|task| task.material_admission.as_deref() == Some("ACCEPTED"))
        .count() as i64;
    let active_count = tasks
        .iter()
        .filter(|task| {
            task.receipt_ref.is_none()
                && (matches!(
                    task.queue_state.as_deref(),
                    Some("pending") | Some("in_progress")
                ) && task.has_live_lease == Some(true)
                    || task.attempt_id.is_some())
        })
        .count() as i64;
    let expired_lease_count = tasks
        .iter()
        .filter(|task| {
            task.receipt_ref.is_none()
                && task.has_live_lease == Some(false)
                && matches!(
                    task.queue_state.as_deref(),
                    Some("pending") | Some("in_progress")
                )
        })
        .count() as i64;

    Ok(CollectionTaskTimeline {
        tasks,
        accepted_count,
        active_count,
        expired_lease_count,
    })
}

/// 一个已授权详情页会话的交付结论。
///
/// 「包到没到服务端」与「材料有没有通过资格判定」是两件事，这里只回答前者：有回执
/// 不等于材料合格，没有回执也不等于本地数据丢了。会话自己记的 `state` 不直接变成
/// 页面上的话——`delivery_pending` 的会话如果冻结通道都已拿到回执，它的结论是
/// 已交付，而不是继续留在待交付里。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeliveryConclusion {
    /// 冻结通道的包都收到了回执。
    Delivered,
    /// 读得到冻结通道的交付身份，其中还有通道没有回执。
    AwaitingDelivery,
    /// 会话说自己在等交付，却读不到它的冻结通道。此时只能显示未知与最后观察时间，
    /// 不能据此推断「没采到」。
    RecoveryUnverified,
    /// 会话已终结（`finished` / `stopped`）。晚到的 progress 不会把它降回待交付。
    Closed,
}

/// 交付对账里的一行：一个已进入交付阶段的详情页会话。
#[derive(Debug, Clone)]
pub struct DetailDeliveryReconciliation {
    pub session_ref: Uuid,
    pub conclusion: DeliveryConclusion,
    /// 会话自己记的状态（机器码，页面上只作旁注）。
    pub state: String,
    pub stop_reason: Option<String>,
    pub platform: Option<String>,
    pub content_external_id: Option<String>,
    pub target_display_name: Option<String>,
    /// 冻结计划登记过的通道数，以及其中已有回执的通道数。两个数一起看才知道欠的是
    /// 哪一段：0 个通道，是读不到身份；3 个里到 1 个，是还有两个包在路上。
    pub prepared_lanes: i64,
    pub delivered_lanes: i64,
    pub last_observed_at: String,
    pub closed_at: Option<String>,
}

/// 只读的交付对账投影。它不接纳、不重投、不清理本地 outbox，也不重开页面。
///
/// 进对账的是**已消费导航的会话**：只有 `authorized` 的会话还没有任何包可以交付，把它列进
/// 「待交付」等于把「还没打开页面」说成「欠着几个包」。已消费导航的会话即使还写着
/// `navigation_committed`，它的通道也已经是可对账的身份了。
pub async fn read_detail_delivery_reconciliation(
    database: &Database,
    limit: i64,
) -> Result<Vec<DetailDeliveryReconciliation>, sqlx::Error> {
    let rows = sqlx::query(DETAIL_DELIVERY_RECONCILIATION_SQL)
        .bind(limit.clamp(1, 200))
        .fetch_all(database.pool())
        .await?;
    let mut reconciliations = Vec::with_capacity(rows.len());
    for row in rows {
        let state: String = row.get("state");
        let prepared_lanes: i64 = row.get("prepared_lanes");
        let delivered_lanes: i64 = row.get("delivered_lanes");
        reconciliations.push(DetailDeliveryReconciliation {
            session_ref: row.get("session_ref"),
            conclusion: delivery_conclusion(&state, prepared_lanes, delivered_lanes),
            state,
            stop_reason: row.get("stop_reason"),
            platform: row.get("platform"),
            content_external_id: row.get("content_external_id"),
            target_display_name: row.get("target_display_name"),
            prepared_lanes,
            delivered_lanes,
            last_observed_at: row.get("last_observed_at"),
            closed_at: row.get("closed_at"),
        });
    }
    Ok(reconciliations)
}

/// 会话落点与通道回执合成一个交付结论。终结压过一切：一个已经终结的会话，
/// 之后再来多少条 progress 都改不回 `delivery_pending`（写入路径同样守着这条），
/// 这里的读法只是如实复述那个终态。
fn delivery_conclusion(
    state: &str,
    prepared_lanes: i64,
    delivered_lanes: i64,
) -> DeliveryConclusion {
    match state {
        "finished" | "stopped" => DeliveryConclusion::Closed,
        _ if prepared_lanes == 0 => DeliveryConclusion::RecoveryUnverified,
        _ if delivered_lanes == prepared_lanes => DeliveryConclusion::Delivered,
        _ => DeliveryConclusion::AwaitingDelivery,
    }
}

const DETAIL_DELIVERY_RECONCILIATION_SQL: &str = r#"
SELECT
    session.session_ref,
    session.state,
    session.stop_reason,
    linggan_human_moment(session.last_progress_at) AS last_observed_at,
    linggan_human_moment(session.finished_at) AS closed_at,
    content.platform,
    content.content_external_id,
    target.display_name AS target_display_name,
    COALESCE(lanes.prepared_lanes, 0) AS prepared_lanes,
    COALESCE(lanes.delivered_lanes, 0) AS delivered_lanes
FROM collection_detail_page_session session
LEFT JOIN collection_work_order work_order
       ON work_order.work_order_ref = session.work_order_ref
LEFT JOIN collection_observation_target target
       ON target.target_ref = work_order.target_ref
LEFT JOIN linggan_material_content content
       ON content.public_ref = session.content_public_ref
LEFT JOIN LATERAL (
    SELECT count(*) AS prepared_lanes,
           count(receipt.receipt_ref) AS delivered_lanes
    FROM collection_detail_page_session_lane_preparation lane
    LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.attempt_id = lane.attempt_id
    WHERE lane.session_ref = session.session_ref
) lanes ON true
WHERE session.state IN (
    'navigation_started','navigation_committed','delivery_pending','finished','stopped'
)
ORDER BY session.last_progress_at DESC
LIMIT $1
"#;

const TASK_TIMELINE_SQL: &str = r#"
SELECT
    task.task_id,
    task.source,
    task.platform,
    task.page_type,
    COALESCE(
        array_to_string(
            ARRAY(SELECT jsonb_array_elements_text(task.task_spec->'capabilitiesRequested')),
            ' · '
        ),
        'UNKNOWN'
    ) AS capabilities,
    task.created_at::text AS created_at,
    lease_task.sequence_no,
    lease_task.execution_state AS queue_state,
    lease_task.claimed_at::text AS claimed_at,
    CASE
        WHEN lease.lease_ref IS NULL THEN NULL
        ELSE (lease.released_at IS NULL AND lease.expires_at > scope_001_now())
    END AS has_live_lease,
    COALESCE(linked_target.display_name, fallback_target.display_name) AS target_display_name,
    COALESCE(linked_target.identity_key, fallback_target.identity_key) AS target_identity_key,
    attempt.attempt_id,
    attempt.started_at::text AS attempt_started_at,
    package.package_kind,
    package.package_ref,
    package.accepted_at::text AS package_accepted_at,
    receipt.receipt_ref,
    receipt.received_at::text AS receipt_received_at,
    receipt.execution_effect,
    receipt.material_admission,
    failure.failure_code AS last_dispatch_failure_code,
    failure.occurred_at::text AS last_dispatch_failure_at
FROM linggan_runtime_task task
LEFT JOIN collection_work_order_lease_task lease_task ON lease_task.task_id = task.task_id
LEFT JOIN collection_work_order_lease lease ON lease.lease_ref = lease_task.lease_ref
LEFT JOIN collection_work_order work_order ON work_order.work_order_ref = lease.work_order_ref
LEFT JOIN collection_observation_target linked_target
       ON linked_target.target_ref = work_order.target_ref
LEFT JOIN LATERAL (
    SELECT target.display_name, target.identity_key
    FROM collection_observation_target target
    WHERE target.platform = task.platform
      AND (
          (task.task_spec #>> '{target,authorExternalId}' IS NOT NULL
           AND target.target_kind = 'creator'
           AND target.identity_key = task.task_spec #>> '{target,authorExternalId}')
          OR
          (task.task_spec #>> '{target,query}' IS NOT NULL
           AND target.target_kind = 'keyword'
           AND target.identity_key = task.task_spec #>> '{target,query}')
      )
    ORDER BY target.first_stored_at DESC
    LIMIT 1
) fallback_target ON true
LEFT JOIN LATERAL (
    SELECT candidate.attempt_id, candidate.started_at
    FROM linggan_runtime_attempt candidate
    WHERE candidate.task_id = task.task_id
    ORDER BY candidate.started_at DESC
    LIMIT 1
) attempt ON true
LEFT JOIN linggan_runtime_capture_package package ON package.attempt_id = attempt.attempt_id
LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.attempt_id = attempt.attempt_id
LEFT JOIN LATERAL (
    SELECT dispatch_failure.failure_code, dispatch_failure.occurred_at
    FROM collection_work_order_lease_task_dispatch_failure dispatch_failure
    WHERE dispatch_failure.task_id = task.task_id
    ORDER BY dispatch_failure.occurred_at DESC
    LIMIT 1
) failure ON true
ORDER BY COALESCE(
    receipt.received_at,
    package.accepted_at,
    attempt.started_at,
    failure.occurred_at,
    task.created_at
) DESC
LIMIT $1
"#;
