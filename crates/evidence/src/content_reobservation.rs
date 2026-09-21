//! A person-requested, bounded reobservation of one already admitted XHS work resource.
//!
//! This is deliberately a thin adapter over the existing acquisition, authorization, work-order,
//! lease, dispatch and Producer receipt chain. It does not create a second task model and it
//! never reaches a platform: only a claimed Browser Producer may do that later.

use crate::acquisition_chain::{
    DETAIL_WINDOW_COMMENT_LIMIT, DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
    in_flight_work_for_exact_material_scope_in_transaction,
    request_and_admit_material_targets_under_authorization_in_transaction,
};
use crate::work_order_lease::expire_lapsed_leases_in_transaction;
use crate::{
    AcquisitionChainError, LeaseError, MaterialDeepeningTarget, acquisition_chain_schema_is_ready,
    lease_schema_is_ready,
};
use linggan_contracts::AdmissionOutcome;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ContentReobservationError {
    #[error("the requested work resource does not exist")]
    WorkResourceNotFound,
    #[error("the requested work resource is not an XHS work")]
    PlatformNotSupported,
    #[error("this work has no active target-linked deepening authorization")]
    AuthorizedTargetMissing,
    #[error("the strict reobservation merge no longer has the lease it was admitted against")]
    EquivalentLeaseMissing,
    #[error(transparent)]
    Acquisition(#[from] AcquisitionChainError),
    #[error(transparent)]
    Lease(#[from] LeaseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReobservation {
    pub public_ref: Uuid,
    pub target_ref: Uuid,
    pub request_ref: Uuid,
    pub decision_ref: Uuid,
    pub admission: &'static str,
    pub admission_reason: String,
    pub work_order_ref: Option<Uuid>,
    pub lease_ref: Option<Uuid>,
    pub expires_at: Option<String>,
    pub execution: &'static str,
    pub media: ReobservationMediaPolicy,
    pub tasks: Vec<ReobservationTask>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReobservationMediaPolicy {
    pub state: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReobservationTask {
    pub task_id: Uuid,
    pub capability: String,
    pub state: String,
    pub sequence_no: i32,
    pub created_at: String,
    pub claimed_at: Option<String>,
    pub attempt_id: Option<Uuid>,
    pub started_at: Option<String>,
    pub package_ref: Option<Uuid>,
    pub receipt_ref: Option<Uuid>,
    pub accepted_at: Option<String>,
    pub comment_limit: Value,
    pub acquire_media: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReobservationStatus {
    pub public_ref: Uuid,
    pub work_order_ref: Uuid,
    pub lease_ref: Uuid,
    pub lease_state: &'static str,
    pub expires_at: String,
    pub released_at: Option<String>,
    pub release_reason: Option<String>,
    pub media: ReobservationMediaPolicy,
    pub tasks: Vec<ReobservationTask>,
}

/// The single read model used by both the Evidence Library action affordance and the command.
/// `eligible` never means a platform call has happened; it says only that this exact work can
/// enter the existing server-authorized reobservation chain at the time of this read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReobservationEligibility {
    pub supported: bool,
    pub eligible: bool,
    pub reason: &'static str,
}

/// Create a fresh, person-requested bounded work order for exactly one stable XHS content ID.
///
/// The target and purpose are derived only from a prior, still-active authorization that is
/// already linked to this work through accepted discovery or an exact material scope. A title,
/// author display name, URL, monitoring fallback, or payload hash is never used as authority.
pub async fn content_reobservation(
    database: &Database,
    public_ref: Uuid,
) -> Result<ContentReobservation, ContentReobservationError> {
    if !acquisition_chain_schema_is_ready(database).await? {
        return Err(ContentReobservationError::Acquisition(
            AcquisitionChainError::SchemaUnavailable,
        ));
    }
    if !lease_schema_is_ready(database).await? {
        return Err(ContentReobservationError::Lease(
            LeaseError::SchemaUnavailable,
        ));
    }

    let mut transaction = database.pool().begin().await?;
    let platform: Option<String> = sqlx::query_scalar(
        "SELECT platform FROM linggan_material_content WHERE public_ref=$1 FOR SHARE",
    )
    .bind(public_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(platform) = platform else {
        return Err(ContentReobservationError::WorkResourceNotFound);
    };
    if platform != "xhs" {
        return Err(ContentReobservationError::PlatformNotSupported);
    }
    let Some(linked_authorization) =
        linked_authorization_in_transaction(&mut transaction, public_ref).await?
    else {
        return Err(ContentReobservationError::AuthorizedTargetMissing);
    };
    // 复观测读的也是那张已经打开过的详情页，所以窗口与「详情补采」同口径（ADR-0002）：一次
    // 打开带回详情与前 30 条评论、2 层回复。额度只有一处定义，免得两个入口各写一个 30。
    let targets = [MaterialDeepeningTarget {
        content_public_ref: public_ref,
        comment_limit: DETAIL_WINDOW_COMMENT_LIMIT,
        reply_expand_limit: DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
        // Normal reobservation reads existing local assets. It must not create media slots,
        // download bytes, or queue OCR/ASR processing simply because a page could have changed.
        acquire_media: false,
        allow_ocr: false,
        allow_asr: false,
    }];
    // This is one transaction by design. `request_and_admit…` holds the observation-target lock
    // while it writes the exact queued scope, so a concurrent click sees that durable Work Order
    // and merges instead of producing a parallel immediate request. A later eligible station is
    // the only owner allowed to turn it into a Lease.
    expire_lapsed_leases_in_transaction(&mut transaction).await?;
    let outcome = request_and_admit_material_targets_under_authorization_in_transaction(
        &mut transaction,
        linked_authorization.target_ref,
        &linked_authorization.purpose,
        "person",
        &targets,
        linked_authorization.authorization_ref,
    )
    .await?;
    let media = reobservation_media_policy();
    let admission = admission_label(outcome.outcome.code());
    let admission_reason = admission_reason(&outcome.outcome);
    let (work_order_ref, lease_ref, expires_at, execution) =
        if let Some(work_order_ref) = outcome.work_order_ref {
            (Some(work_order_ref), None, None, "QUEUED")
        } else if matches!(&outcome.outcome, AdmissionOutcome::Merge { .. }) {
            let existing = in_flight_work_for_exact_material_scope_in_transaction(
                &mut transaction,
                linked_authorization.target_ref,
                "deep_archive",
                linked_authorization.authorization_ref,
                &targets,
            )
            .await?
            .ok_or(ContentReobservationError::EquivalentLeaseMissing)?;
            let execution = if existing.lease_ref.is_some() {
                "MERGED"
            } else {
                "QUEUED"
            };
            (
                Some(existing.work_order_ref),
                existing.lease_ref,
                existing.expires_at,
                execution,
            )
        } else {
            (None, None, None, "NOT_STARTED")
        };
    transaction.commit().await?;

    let tasks = if let Some(lease_ref) = lease_ref {
        read_content_reobservation(database, public_ref, lease_ref)
            .await?
            .ok_or(ContentReobservationError::WorkResourceNotFound)?
            .tasks
    } else {
        Vec::new()
    };
    Ok(ContentReobservation {
        public_ref,
        target_ref: linked_authorization.target_ref,
        request_ref: outcome.request_ref,
        decision_ref: outcome.decision_ref,
        admission,
        admission_reason,
        work_order_ref,
        lease_ref,
        expires_at,
        execution,
        media,
        tasks,
    })
}

/// Read the canonical reobservation eligibility without opening a request.  The command performs
/// the same authorization lookup again inside its write transaction, so this is never a grant or
/// a TOCTOU bypass.
pub async fn read_content_reobservation_eligibility(
    database: &Database,
    public_ref: Uuid,
) -> Result<Option<ContentReobservationEligibility>, ContentReobservationError> {
    let mut transaction = database.pool().begin().await?;
    let platform: Option<String> =
        sqlx::query_scalar("SELECT platform FROM linggan_material_content WHERE public_ref=$1")
            .bind(public_ref)
            .fetch_optional(&mut *transaction)
            .await?;
    let eligibility = match platform.as_deref() {
        None => None,
        Some("xhs") => {
            let eligible = linked_authorization_in_transaction(&mut transaction, public_ref)
                .await?
                .is_some();
            Some(ContentReobservationEligibility {
                supported: true,
                eligible,
                reason: if eligible {
                    "TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION"
                } else {
                    "TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED"
                },
            })
        }
        Some(_) => Some(ContentReobservationEligibility {
            supported: false,
            eligible: false,
            reason: "PLATFORM_NOT_SUPPORTED",
        }),
    };
    transaction.rollback().await?;
    Ok(eligibility)
}

/// Read the persisted task/attempt/package/receipt state for one exact reobservation lease.
///
/// `collection_work_order_lease_task` is the execution source. The labels here never collapse a
/// claim, a producer attempt, a package, and a receipt into one optimistic "complete" state.
pub async fn read_content_reobservation(
    database: &Database,
    public_ref: Uuid,
    lease_ref: Uuid,
) -> Result<Option<ContentReobservationStatus>, ContentReobservationError> {
    let rows = sqlx::query(
        "SELECT lease.lease_ref,work_order.work_order_ref,lease.expires_at::text AS expires_at, \
                (lease.expires_at <= scope_001_now()) AS lease_expired, \
                lease.released_at::text AS released_at,lease.release_reason, \
                lease_task.task_id,lease_task.sequence_no,lease_task.execution_state, \
                lease_task.claimed_at::text AS claimed_at,runtime.created_at::text AS created_at, \
                runtime.task_spec->'capabilitiesRequested'->>0 AS capability, \
                runtime.task_spec->'commentLimit' AS comment_limit,runtime.task_spec->'acquireMedia' AS acquire_media, \
                runtime_attempt.attempt_id,runtime_attempt.started_at::text AS started_at, \
                package.package_ref,package.accepted_at::text AS accepted_at,receipt.receipt_ref \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order work_order ON work_order.work_order_ref=lease.work_order_ref \
         JOIN collection_work_order_material_target scope ON scope.work_order_ref=work_order.work_order_ref \
         JOIN collection_work_order_lease_task lease_task ON lease_task.lease_ref=lease.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id=lease_task.task_id \
         LEFT JOIN LATERAL ( \
             SELECT attempt_id,started_at FROM linggan_runtime_attempt \
             WHERE task_id=lease_task.task_id ORDER BY started_at DESC,attempt_id DESC LIMIT 1 \
         ) runtime_attempt ON true \
         LEFT JOIN LATERAL ( \
             SELECT capture.package_ref,capture.accepted_at FROM linggan_runtime_capture_package capture \
             WHERE capture.task_id=lease_task.task_id \
               AND (runtime_attempt.attempt_id IS NULL OR capture.attempt_id=runtime_attempt.attempt_id) \
             ORDER BY capture.accepted_at DESC,capture.package_ref DESC LIMIT 1 \
         ) package ON true \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         WHERE scope.content_public_ref=$1 AND lease.lease_ref=$2 \
         ORDER BY lease_task.sequence_no",
    )
    .bind(public_ref)
    .bind(lease_ref)
    .fetch_all(database.pool())
    .await?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let released_at: Option<String> = first.get("released_at");
    let release_reason: Option<String> = first.get("release_reason");
    let lease_expired: bool = first.get("lease_expired");
    let expires_at: String = first.get("expires_at");
    let lease_state = if released_at.is_some() {
        "RELEASED"
    } else if lease_expired {
        "EXPIRED"
    } else {
        "ACTIVE"
    };
    let tasks = rows
        .iter()
        .map(|row| task_from_row(row, released_at.is_some() || lease_expired))
        .collect();
    Ok(Some(ContentReobservationStatus {
        public_ref,
        work_order_ref: first.get("work_order_ref"),
        lease_ref: first.get("lease_ref"),
        lease_state,
        expires_at,
        released_at,
        release_reason,
        media: reobservation_media_policy(),
        tasks,
    }))
}

struct LinkedAuthorization {
    target_ref: Uuid,
    authorization_ref: Uuid,
    purpose: String,
}

async fn linked_authorization_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    public_ref: Uuid,
) -> Result<Option<LinkedAuthorization>, sqlx::Error> {
    let row = sqlx::query(
        "WITH linked_authority AS ( \
             SELECT work_order.target_ref,decision.authorization_ref,scope.created_at AS linked_at \
             FROM collection_work_order_material_target scope \
             JOIN collection_work_order work_order ON work_order.work_order_ref=scope.work_order_ref \
             JOIN collection_admission_decision decision ON decision.decision_ref=work_order.decision_ref \
             WHERE scope.content_public_ref=$1 \
             UNION ALL \
             SELECT work_order.target_ref,decision.authorization_ref,finding.observed_at::timestamptz AS linked_at \
             FROM linggan_material_discovery_finding finding \
             JOIN linggan_runtime_capture_package package USING(package_ref) \
             JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
             JOIN collection_work_order_lease lease ON lease.lease_ref=lease_task.lease_ref \
             JOIN collection_work_order work_order ON work_order.work_order_ref=lease.work_order_ref \
             JOIN collection_admission_decision decision ON decision.decision_ref=work_order.decision_ref \
             WHERE finding.content_public_ref=$1 \
         ) \
         SELECT linked_authority.target_ref,acquisition_auth.authorization_ref,acquisition_auth.purpose \
         FROM linked_authority \
         JOIN collection_acquisition_authorization acquisition_auth \
           ON acquisition_auth.authorization_ref=linked_authority.authorization_ref \
         WHERE acquisition_auth.lane='deep_archive' AND acquisition_auth.revoked_at IS NULL \
           AND acquisition_auth.expires_at > scope_001_now() \
         ORDER BY linked_authority.linked_at DESC LIMIT 1",
    )
    .bind(public_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(row.map(|row| LinkedAuthorization {
        target_ref: row.get("target_ref"),
        authorization_ref: row.get("authorization_ref"),
        purpose: row.get("purpose"),
    }))
}

/// The reader-facing state词表 of one lease task.
///
/// 单独提出来是为了能被单元测试钉住：它只从四个已知量判状态，不碰数据库。判据顺序即语义：
/// 有回执才算交付；生产方确认页面不在、页面读过但没读成、以及**从未拿到执行地址**是三种不同
/// 的失败，不能说成同一个；租约已释放而任务还停在 pending，才是「租约结束，未见回执」。
///
/// `input_blocked` 必须单列。它在 `0097` 之前于 `execution_state` 里根本不存在——缺地址的成员
/// 在浏览器 Attempt 之前就被停下，既没有 Attempt 也没有 Package。旧的 `else` 兜底会把它读成
/// `QUEUED`，那是在告诉操作者「还在排队」：一篇永远不会再被派发的作品，看起来和一篇正等着
/// 派发的作品完全一样。停止必须有停止的说法。
fn task_state(
    execution_state: &str,
    lease_released: bool,
    attempt_id: Option<Uuid>,
    receipt_ref: Option<Uuid>,
) -> &'static str {
    if receipt_ref.is_some() {
        "ACCEPTED"
    } else if execution_state == "completed" {
        "COMPLETED_WITHOUT_RECEIPT"
    } else if execution_state == "unavailable" {
        "PAGE_UNAVAILABLE"
    } else if execution_state == "blocked" {
        "DETAIL_READ_BLOCKED"
    } else if execution_state == "input_blocked" {
        "INPUT_BLOCKED"
    } else if lease_released {
        "EXPIRED_WITHOUT_RECEIPT"
    } else if execution_state == "in_progress" && attempt_id.is_some() {
        "RUNNING"
    } else if execution_state == "in_progress" {
        "CLAIMED"
    } else {
        "QUEUED"
    }
}

fn task_from_row(row: &sqlx::postgres::PgRow, lease_released: bool) -> ReobservationTask {
    let execution_state: String = row.get("execution_state");
    let attempt_id: Option<Uuid> = row.get("attempt_id");
    let package_ref: Option<Uuid> = row.get("package_ref");
    let receipt_ref: Option<Uuid> = row.get("receipt_ref");
    let state = task_state(
        &execution_state,
        lease_released,
        attempt_id,
        receipt_ref,
    );
    ReobservationTask {
        task_id: row.get("task_id"),
        capability: row.get("capability"),
        state: state.to_owned(),
        sequence_no: row.get("sequence_no"),
        created_at: row.get("created_at"),
        claimed_at: row.get("claimed_at"),
        attempt_id,
        started_at: row.get("started_at"),
        package_ref,
        receipt_ref,
        accepted_at: row.get("accepted_at"),
        comment_limit: row.get("comment_limit"),
        acquire_media: row.get("acquire_media"),
    }
}

fn admission_label(value: &str) -> &'static str {
    match value {
        "admitted" => "ADMITTED",
        "reuse" => "REUSE",
        "merge" => "MERGE",
        "defer" => "DEFERRED",
        "refuse" => "REFUSED",
        "decision_required" => "DECISION_REQUIRED",
        _ => "UNKNOWN",
    }
}

fn admission_reason(outcome: &AdmissionOutcome) -> String {
    match outcome {
        AdmissionOutcome::Admitted { .. } => "已在作品原先关联的有效授权范围内准入".to_owned(),
        AdmissionOutcome::Reuse { reason }
        | AdmissionOutcome::Merge { reason }
        | AdmissionOutcome::Defer { reason }
        | AdmissionOutcome::Refuse { reason }
        | AdmissionOutcome::DecisionRequired { reason, .. } => reason.clone(),
    }
}

const fn reobservation_media_policy() -> ReobservationMediaPolicy {
    ReobservationMediaPolicy {
        state: "NOT_REQUESTED",
        reason: "EXISTING_ASSETS_REUSED",
    }
}

#[cfg(test)]
mod tests {
    use super::task_state;
    use uuid::Uuid;

    /// `0097` 的 `input_blocked` 与 `blocked` 都带 `attempt_id = NULL`，也都没有回执：只有
    /// `execution_state` 这一个词能把它们分开。若这个分支缺位，兜底的 `QUEUED` 会把一篇
    /// **永远不会再被派发**的作品说成「还在排队」——操作者等一个不存在的交付。
    #[test]
    fn a_task_stopped_for_missing_input_is_not_reported_as_queued() {
        assert_eq!(
            task_state("input_blocked", true, None, None),
            "INPUT_BLOCKED",
        );
        assert_eq!(task_state("input_blocked", false, None, None), "INPUT_BLOCKED");
    }

    /// 词表其余分支与顺序一并钉住：租约已释放只解释 **pending**，不能把已经说明过原因的
    /// 三种失败（`unavailable`/`blocked`/`input_blocked`）改写成「租约结束，未见回执」。
    #[test]
    fn the_release_reason_never_overrides_a_state_that_already_explains_itself() {
        assert_eq!(
            task_state("unavailable", true, None, None),
            "PAGE_UNAVAILABLE"
        );
        assert_eq!(task_state("blocked", true, None, None), "DETAIL_READ_BLOCKED");
        assert_eq!(
            task_state("pending", true, None, None),
            "EXPIRED_WITHOUT_RECEIPT"
        );
        assert_eq!(task_state("pending", false, None, None), "QUEUED");
        let attempt = Uuid::nil();
        assert_eq!(task_state("in_progress", false, Some(attempt), None), "RUNNING");
        assert_eq!(task_state("in_progress", false, None, None), "CLAIMED");
        let receipt = Uuid::nil();
        assert_eq!(
            task_state("completed", false, Some(attempt), Some(receipt)),
            "ACCEPTED"
        );
        assert_eq!(
            task_state("completed", false, Some(attempt), None),
            "COMPLETED_WITHOUT_RECEIPT"
        );
    }
}
