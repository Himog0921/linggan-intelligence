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
