//! COLLECTION-001 · 调度决策：一台工位来问「现在有我能做的活吗」。
//!
//! 这里**只做决策，不做执行**（规则文档：调度决策与重执行必须分开，tick 只做轻量状态推进
//! 与入队）。它不下载、不转录、不访问任何平台，只回答「可以派 / 不可以派，以及为什么」。
//!
//! 拒绝的理由必须具体。一个笼统的「暂无任务」会让人分不清是「今天额度用完了」还是
//! 「风险暂停中」——这两件事的处置完全不同。
//!
//! 真实执行闸门已于 2026-08-28 删除（migration 0013）：一个必须由人反复上弦的开关，
//! 装不进一个必须无人值守运行的系统。稳态的控制是每日额度、风险暂停与授权到期，它们
//! 默认允许、有界、自己复位。

use crate::collection_control::{
    evaluate_claiming_installation_capacity_in, required_capabilities_for,
    revalidate_frozen_capacity_in, validate_installation_credential_in,
};
use crate::work_order_lease::{
    LeaseError, claim_queued_work_order_in_transaction, expire_lapsed_leases_in_transaction,
    recover_released_orphaned_work_orders_in_transaction,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The next retry after a locally reported execution-start failure.  Chrome
/// alarms cannot run more frequently than once per minute, and a retry must
/// not immediately reopen a page that just failed its readiness probe.
pub const DISPATCH_FAILURE_RETRY_AFTER_SECONDS: u32 = 60;
const MAX_DISPATCH_FAILURE_RETRY_AFTER_SECONDS: u32 = 900;
/// A browser page-read failure has no producer Attempt, so repeated automatic
/// retries only repeat the same uncertain pre-execution state.  Keep two
/// bounded recoveries, then make that exact frozen detail material visibly
/// blocked until a later, explicit recovery decision is made.
const MAX_PAGE_READ_FAILURES_PER_DETAIL: i64 = 3;
const CLAIM_LEASE_MINUTES: i32 = 30;

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no plugin installation with that install key")]
    UnknownInstallation,
    #[error("installation credential is invalid")]
    InvalidCredential,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The only server response that may be paired with a browser's durable local
/// navigation-consumption record.  It is intentionally separate from task
/// claim: a committed claim may be replayed, while this grant is idempotent by
/// a browser-persisted request id and one Work Order/material session.
#[derive(Debug, Clone, PartialEq)]
pub enum DetailPageSessionGrant {
    Authorized {
        session_ref: Uuid,
        plan: Value,
    },
    Replay {
        session_ref: Uuid,
        plan: Value,
    },
    Suppressed {
        session_ref: Uuid,
        reason_code: &'static str,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DetailPageSessionGrantError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no active plugin installation with that install key")]
    UnknownInstallation,
    #[error("installation credential is invalid")]
    InvalidCredential,
    #[error("the installation no longer holds that live detail task claim")]
    ClaimNotHeld,
    #[error("the task is not a material detail-page task")]
    DetailScopeUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A navigation observation is deliberately separate from grant issuance.
/// It advances the one session row and never creates another execution ledger.
#[derive(Debug, thiserror::Error)]
pub enum DetailPageSessionNavigationError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no active plugin installation with that install key")]
    UnknownInstallation,
    #[error("installation credential is invalid")]
    InvalidCredential,
    #[error("the installation does not own an active detail-page session for that task")]
    SessionNotHeld,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A constrained session-progress vocabulary. None of these transitions can
/// issue, renew, or transfer a navigation grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailPageSessionProgress {
    NavigationObserved,
    DeliveryPending,
    Stopped { reason: &'static str },
}

/// A bounded failure vocabulary for a claimed browser task before a producer
/// Attempt exists.  These are local execution facts, not source Evidence and
/// never contain raw platform/browser text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchFailureCode {
    CapabilityNotExecutableHere,
    TargetIncomplete,
    TabUnavailable,
    PageTimeout,
    PageUnavailable,
    PageReceiptMissing,
    PageReceiptIdentityMismatch,
    PageReadFailed,
    AccountObservationBlocked,
}

impl DispatchFailureCode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "capability_not_executable_here" => Some(Self::CapabilityNotExecutableHere),
            "target_incomplete" => Some(Self::TargetIncomplete),
            "tab_unavailable" => Some(Self::TabUnavailable),
            "page_timeout" => Some(Self::PageTimeout),
            "page_unavailable" => Some(Self::PageUnavailable),
            "page_receipt_missing" => Some(Self::PageReceiptMissing),
            "page_receipt_identity_mismatch" => Some(Self::PageReceiptIdentityMismatch),
            "page_read_failed" => Some(Self::PageReadFailed),
            "account_observation_blocked" => Some(Self::AccountObservationBlocked),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapabilityNotExecutableHere => "capability_not_executable_here",
            Self::TargetIncomplete => "target_incomplete",
            Self::TabUnavailable => "tab_unavailable",
            Self::PageTimeout => "page_timeout",
            Self::PageUnavailable => "page_unavailable",
            Self::PageReceiptMissing => "page_receipt_missing",
            Self::PageReceiptIdentityMismatch => "page_receipt_identity_mismatch",
            Self::PageReadFailed => "page_read_failed",
            Self::AccountObservationBlocked => "account_observation_blocked",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchFailureError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no active plugin installation with that install key")]
    UnknownInstallation,
    #[error("installation credential is invalid")]
    InvalidCredential,
    #[error("the installation no longer holds that live task claim")]
    ClaimNotHeld,
    #[error("failure id was already used for a different task, installation, or failure code")]
    FailureIdentityConflict,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A local execution failure has either moved the current task back to the
/// queue, or replayed a response that had already done so.  Neither variant is
/// a producer Attempt or a Capture Package outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchFailureOutcome {
    Requeued {
        retry_after_seconds: u32,
    },
    /// The claimed `content_detail` page was explicitly unavailable.  The
    /// approved material remains missing; only the dependent lanes for that
    /// same material become terminal so later works in the bounded batch can
    /// still proceed.
    Unavailable,
    /// The same frozen detail material reached its bounded page-read retry
    /// limit.  This is not page unavailability and does not create an
    /// Attempt, CapturePackage, Receipt, or Evidence.
    Blocked,
    Replay {
        retry_after_seconds: u32,
    },
}

/// 调度对一次「有活吗」的回答。
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchDecision {
    /// 可以派这个任务。
    Dispatch {
        task_id: Uuid,
        lease_ref: Uuid,
        task_spec: Value,
        /// 一次派发所需的短期页面定位信息。它不是 Task 身份，不写回不可变 TaskSpec。
        execution_source_url: Option<String>,
        /// 同一详情页内已经由 Work Order 批准的读取范围。它只用于减少重复开页，
        /// 不替代各 lane 自己的 TaskSpec，也不授权提前提交尚未领取的 Package。
        page_session_plan: Option<Value>,
    },
    /// 这个安装还没归位到任何工位，因此不属于任何工位的产能。
    InstallationNotClaimed,
    /// 有覆盖本平台或 lane 的风险暂停。
    RiskPaused { reason: String },
    /// 这台工位今天的额度已经用完。工位没坏，明天自然恢复。
    DailyQuotaReached { quota: i32, used: i64 },
    /// 没有等待派发的任务。
    NothingWaiting,
    /// 任务存在，但当下没有符合资格的页面执行定位信息。
    ExecutionLocatorUnavailable { reason: String },
    /// A frozen control fact changed after the Lease was issued.
    ControlBlocked { reason_code: String },
}

impl DispatchDecision {
    /// 下次隔多久再来问。
    ///
    /// **节奏由服务端给，不由插件自己定。**插件定的话，想调就得重新发一版插件；而且十个
    /// 插件会各自按自己的常量敲门，服务端对总量毫无控制。
    ///
    /// 取值按「这个答案多久可能变一次」来定：额度触顶要等次日窗口重置，没必要频繁问；
    /// 有活可派时立刻再来，因为一次派发通常意味着后面还有。
    pub fn next_poll_after_seconds(&self) -> u32 {
        match self {
            // 刚派出一个，后面很可能还有——立刻再来。
            Self::Dispatch { .. } => 0,
            Self::NothingWaiting => 300,
            Self::ExecutionLocatorUnavailable { .. } => 300,
            Self::ControlBlocked { .. } => 300,
            // 触顶要等次日自然日窗口重置，问得再勤也不会变。
            Self::DailyQuotaReached { .. } => 1800,
            Self::RiskPaused { .. } => 900,
            // A person may claim the installation from the local station page
            // at any moment, but the page has no channel to wake a Chrome
            // service worker.  Recheck at Chrome's one-minute minimum so a
            // newly claimed install does not appear disconnected for 15 min.
            Self::InstallationNotClaimed => 60,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Dispatch { .. } => "dispatch",
            Self::InstallationNotClaimed => "installation_not_claimed",
            Self::RiskPaused { .. } => "risk_paused",
            Self::DailyQuotaReached { .. } => "daily_quota_reached",
            Self::NothingWaiting => "nothing_waiting",
            Self::ExecutionLocatorUnavailable { .. } => "execution_locator_unavailable",
            Self::ControlBlocked { reason_code } => match reason_code.as_str() {
                "risk_paused" => "risk_paused",
                "station_unavailable" => "station_unavailable",
                "installation_credential_missing" => "installation_credential_missing",
                "plugin_version_unsupported" => "plugin_version_unsupported",
                "installation_stale" => "installation_stale",
                "capability_missing" => "capability_missing",
                "account_unbound" => "account_unbound",
                "account_binding_changed" => "account_binding_changed",
                "account_binding_expired" => "account_binding_expired",
                "account_eligibility_stale" => "account_eligibility_stale",
                "account_cooling" => "account_cooling",
                "account_needs_login" => "account_needs_login",
                "account_restricted" => "account_restricted",
                "account_unknown" => "account_unknown",
                "account_busy" => "account_busy",
                "station_busy" => "station_busy",
                "station_daily_budget_reached" => "station_daily_budget_reached",
                "platform_concurrency_reached" => "platform_concurrency_reached",
                "rule_revision_changed" => "rule_revision_changed",
                "monitoring_paused" => "monitoring_paused",
                "authorization_expired_or_revoked" => "authorization_expired_or_revoked",
                "target_not_requestable" => "target_not_requestable",
                _ => "capacity_unknown",
            },
        }
    }

    /// 只有一种回答允许插件去访问平台。
    pub fn permits_execution(&self) -> bool {
        matches!(self, Self::Dispatch { .. })
    }
}

/// 记下「上一次这台工位来问活，我们的回答是什么」。
///
/// 2026-09-08 停摆的那一整天里，服务端每 5 分钟都给出过一个明确的回答，而这个回答只发给
/// 插件，没有在任何人能看见的地方留下痕迹。人能看到的只有「等待 5 单」——**给出了回答却
/// 不留痕，等于没有回答**。
///
/// 只留最后一次，不做流水账：一天 288 次问活，其中 287 次是同一句话。人要看的是「现在
/// 为什么不动」，不是「过去三天每五分钟分别为什么不动」。
///
/// 这是一条**只供显示**的记录：它不参与任何准入或派发判断，写失败也不能影响这次派发的
/// 结果——所以它单独一次写入，而不是挤进 `decide_dispatch` 的事务里。调用方据此可以放心
/// 忽略它的错误。
pub async fn record_dispatch_answer(
    database: &Database,
    install_key: &str,
    decision: &DispatchDecision,
) -> Result<(), sqlx::Error> {
    // 「被拦住了」与「被什么拦住」是两个事实，分开存才拆得开。
    let reason_code = match decision {
        DispatchDecision::ControlBlocked { reason_code } => Some(reason_code.as_str()),
        _ => None,
    };
    sqlx::query(
        "UPDATE execution_station SET last_dispatch_answer_at=scope_001_now(), \
             last_dispatch_answer_code=$2,last_dispatch_answer_reason=$3 \
         WHERE station_ref=(SELECT station_ref FROM plugin_installation \
                            WHERE install_key=$1 AND superseded_at IS NULL)",
    )
    .bind(install_key)
    .bind(decision.code())
    .bind(reason_code)
    .execute(database.pool())
    .await?;
    Ok(())
}

pub async fn dispatch_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass(format('%I.%I',current_schema(),'collection_work_order_lease')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_work_order_lease_task')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_work_order_lease_task_dispatch_failure')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_dispatch_lane_fairness')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_platform_dispatch_policy')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_detail_page_session')) IS NOT NULL \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='collection_work_order' \
                              AND column_name='retry_not_before_at') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='collection_work_order_lease_task_dispatch_failure' \
                              AND column_name='failure_disposition')",
    )
    .fetch_one(database.pool())
    .await
}

/// Authorize the one page-owning `content_detail` task for a material only
/// after the Browser Producer has persisted `grant_request_id` locally.
///
/// PostgreSQL makes the Work Order/material session unique.  Retrying the
/// same request id returns the same grant; another request id, another
/// installation, a stopped session, or an unheld task never becomes a second
/// navigation authorization.  This function does not observe Chrome and must
/// not claim that a page was actually opened.
pub async fn grant_detail_page_session(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    grant_request_id: Uuid,
) -> Result<DetailPageSessionGrant, DetailPageSessionGrantError> {
    if !dispatch_schema_is_ready(database).await? {
        return Err(DetailPageSessionGrantError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let installation_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation \
         WHERE install_key=$1 AND superseded_at IS NULL FOR UPDATE",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(installation_ref) = installation_ref else {
        return Err(DetailPageSessionGrantError::UnknownInstallation);
    };
    if !validate_installation_credential_in(
        &mut transaction,
        installation_ref,
        installation_credential,
    )
    .await?
    {
        return Err(DetailPageSessionGrantError::InvalidCredential);
    }

    type ClaimedDetail = (Uuid, Uuid, Option<Uuid>, Option<Uuid>, Value);
    type RawClaimedDetail = (Uuid, Uuid, Uuid, Value);
    let material_claimed: Option<ClaimedDetail> = sqlx::query_as::<_, RawClaimedDetail>(
        "SELECT lease.lease_ref,lease.work_order_ref,target.content_public_ref,runtime.task_spec \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         JOIN collection_work_order_material_target target ON target.work_order_ref=lease.work_order_ref \
         JOIN linggan_material_content content ON content.public_ref=target.content_public_ref \
         WHERE task.task_id=$1 AND task.execution_state='in_progress' \
           AND task.claimed_by_installation_ref=$2 \
           AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
           AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
           AND content.content_external_id=runtime.task_spec #>> '{target,contentExternalId}' \
         FOR UPDATE OF task,lease",
    )
    .bind(task_id)
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .map(|(lease_ref, work_order_ref, content_public_ref, task_spec)| {
        (lease_ref, work_order_ref, Some(content_public_ref), None, task_spec)
    });
    let cross_industry_claimed: Option<ClaimedDetail> = if material_claimed.is_none()
        && sqlx::query_scalar::<_, bool>(
            "SELECT to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
        )
        .fetch_one(&mut *transaction)
        .await?
    {
        sqlx::query_as::<_, RawClaimedDetail>(
            "SELECT lease.lease_ref,lease.work_order_ref,sample.sample_ref,runtime.task_spec \
             FROM collection_work_order_lease_task task \
             JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
             JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
             JOIN collection_work_order_cross_industry_target target \
               ON target.work_order_ref=lease.work_order_ref \
             JOIN cross_industry_sample sample ON sample.sample_ref=target.sample_ref \
             WHERE task.task_id=$1 AND task.execution_state='in_progress' \
               AND task.claimed_by_installation_ref=$2 \
               AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
               AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
               AND sample.content_external_id=runtime.task_spec #>> '{target,contentExternalId}' \
             FOR UPDATE OF task,lease",
        )
        .bind(task_id)
        .bind(installation_ref)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|(lease_ref, work_order_ref, sample_ref, task_spec)| {
            (lease_ref, work_order_ref, None, Some(sample_ref), task_spec)
        })
    } else {
        None
    };
    let Some((lease_ref, work_order_ref, content_public_ref, cross_industry_sample_ref, task_spec)) =
        material_claimed.or(cross_industry_claimed)
    else {
        return Err(DetailPageSessionGrantError::ClaimNotHeld);
    };
    let Some(plan) = page_session_plan_for_task(&mut transaction, lease_ref, &task_spec).await?
    else {
        return Err(DetailPageSessionGrantError::DetailScopeUnavailable);
    };
    let plan_hash = Sha256::digest(
        serde_json::to_string(&plan)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?
            .as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();

    type Existing = (Uuid, Uuid, Uuid, String, Value);
    let existing: Option<Existing> = sqlx::query_as(
        "SELECT session_ref,owner_installation_ref,grant_request_id,state,plan_snapshot \
         FROM collection_detail_page_session \
         WHERE work_order_ref=$1 \
           AND content_public_ref IS NOT DISTINCT FROM $2 \
           AND cross_industry_sample_ref IS NOT DISTINCT FROM $3 \
         FOR UPDATE",
    )
    .bind(work_order_ref)
    .bind(content_public_ref)
    .bind(cross_industry_sample_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let outcome = match existing {
        None => {
            let session_ref = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO collection_detail_page_session \
                     (session_ref,work_order_ref,content_public_ref,cross_industry_sample_ref,owner_installation_ref, \
                      grant_request_id,initial_lease_ref,plan_snapshot,plan_hash,state) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'authorized')",
            )
            .bind(session_ref)
            .bind(work_order_ref)
            .bind(content_public_ref)
            .bind(cross_industry_sample_ref)
            .bind(installation_ref)
            .bind(grant_request_id)
            .bind(lease_ref)
            .bind(&plan)
            .bind(&plan_hash)
            .execute(&mut *transaction)
            .await?;
            DetailPageSessionGrant::Authorized { session_ref, plan }
        }
        Some((session_ref, owner_installation_ref, recorded_request_id, state, snapshot))
            if owner_installation_ref == installation_ref
                && recorded_request_id == grant_request_id =>
        {
            if state == "stopped" {
                DetailPageSessionGrant::Suppressed {
                    session_ref,
                    reason_code: "session_stopped",
                }
            } else {
                DetailPageSessionGrant::Replay {
                    session_ref,
                    plan: snapshot,
                }
            }
        }
        Some((session_ref, _, _, _, _)) => DetailPageSessionGrant::Suppressed {
            session_ref,
            reason_code: "session_already_authorized",
        },
    };
    transaction.commit().await?;
    Ok(outcome)
}

/// Record that Chrome observed the one locally consumed detail tab.  This is
/// not a navigation command and cannot reopen a page; a missing report stays
/// an honest unknown instead of being inferred from authorization alone.
pub async fn record_detail_page_session_navigation(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    session_ref: Uuid,
) -> Result<(), DetailPageSessionNavigationError> {
    record_detail_page_session_progress(
        database,
        install_key,
        installation_credential,
        task_id,
        session_ref,
        DetailPageSessionProgress::NavigationObserved,
    )
    .await
}

/// Persist the latest execution fact for one owner-held session. A stopped
/// session remains replayable only as `suppressed`; it can never be used to
/// revive an automatic browser navigation.
pub async fn record_detail_page_session_progress(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    session_ref: Uuid,
    progress: DetailPageSessionProgress,
) -> Result<(), DetailPageSessionNavigationError> {
    if !dispatch_schema_is_ready(database).await? {
        return Err(DetailPageSessionNavigationError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let installation_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation \
         WHERE install_key=$1 AND superseded_at IS NULL FOR UPDATE",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(installation_ref) = installation_ref else {
        return Err(DetailPageSessionNavigationError::UnknownInstallation);
    };
    if !validate_installation_credential_in(
        &mut transaction,
        installation_ref,
        installation_credential,
    )
    .await?
    {
        return Err(DetailPageSessionNavigationError::InvalidCredential);
    }
    let recorded: Option<Uuid> = match progress {
        DetailPageSessionProgress::NavigationObserved => {
            sqlx::query_scalar(
                "UPDATE collection_detail_page_session session \
             SET state=CASE WHEN state='authorized' THEN 'navigation_committed' ELSE state END, \
                 navigation_observed_at=COALESCE(navigation_observed_at,scope_001_now()), \
                 last_progress_at=scope_001_now() \
             WHERE session.session_ref=$1 AND session.owner_installation_ref=$2 \
               AND session.state NOT IN ('finished','stopped') \
               AND EXISTS (SELECT 1 FROM collection_work_order_lease_task task \
                           WHERE task.lease_ref=session.initial_lease_ref AND task.task_id=$3) \
             RETURNING session.session_ref",
            )
            .bind(session_ref)
            .bind(installation_ref)
            .bind(task_id)
            .fetch_optional(&mut *transaction)
            .await?
        }
        DetailPageSessionProgress::DeliveryPending => {
            sqlx::query_scalar(
                "UPDATE collection_detail_page_session session \
             SET state='delivery_pending',last_progress_at=scope_001_now() \
             WHERE session.session_ref=$1 AND session.owner_installation_ref=$2 \
               AND session.state NOT IN ('finished','stopped') \
               AND EXISTS (SELECT 1 FROM collection_work_order_lease_task task \
                           WHERE task.lease_ref=session.initial_lease_ref AND task.task_id=$3) \
             RETURNING session.session_ref",
            )
            .bind(session_ref)
            .bind(installation_ref)
            .bind(task_id)
            .fetch_optional(&mut *transaction)
            .await?
        }
        DetailPageSessionProgress::Stopped { reason } => {
            sqlx::query_scalar(
                "UPDATE collection_detail_page_session session \
             SET state='stopped',stop_reason=$4,finished_at=scope_001_now(), \
                 last_progress_at=scope_001_now() \
             WHERE session.session_ref=$1 AND session.owner_installation_ref=$2 \
               AND session.state NOT IN ('finished','stopped') \
               AND EXISTS (SELECT 1 FROM collection_work_order_lease_task task \
                           WHERE task.lease_ref=session.initial_lease_ref AND task.task_id=$3) \
             RETURNING session.session_ref",
            )
            .bind(session_ref)
            .bind(installation_ref)
            .bind(task_id)
            .bind(reason)
            .fetch_optional(&mut *transaction)
            .await?
        }
    };
    if recorded.is_none() {
        return Err(DetailPageSessionNavigationError::SessionNotHeld);
    }
    transaction.commit().await?;
    Ok(())
}

/// Release a *current* scheduled-task claim after the plugin could not start
/// its browser work.  The failure is appended before the task becomes pending
/// again, so retries keep their frozen TaskSpec while the original failed
/// claim remains auditable.
///
/// Only the active installation that currently holds the live claim may make
/// this transition.  A delayed failure cannot reopen a task which has since
/// produced a Package, been superseded, revoked, or expired.
pub async fn requeue_failed_dispatch(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    failure_ref: Uuid,
    failure_code: DispatchFailureCode,
) -> Result<DispatchFailureOutcome, DispatchFailureError> {
    if !dispatch_schema_is_ready(database).await? {
        return Err(DispatchFailureError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let installation_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation \
         WHERE install_key=$1 AND superseded_at IS NULL FOR UPDATE",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(installation_ref) = installation_ref else {
        return Err(DispatchFailureError::UnknownInstallation);
    };
    if !validate_installation_credential_in(
        &mut transaction,
        installation_ref,
        installation_credential,
    )
    .await?
    {
        return Err(DispatchFailureError::InvalidCredential);
    }

    let replay: Option<(Uuid, Uuid, String, i32, String)> = sqlx::query_as(
        "SELECT task_id,installation_ref,failure_code,retry_after_seconds,failure_disposition \
         FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_ref=$1 FOR UPDATE",
    )
    .bind(failure_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((
        recorded_task_id,
        recorded_installation_ref,
        recorded_code,
        retry_after_seconds,
        recorded_disposition,
    )) = replay
    {
        if recorded_task_id != task_id
            || recorded_installation_ref != installation_ref
            || recorded_code != failure_code.as_str()
        {
            return Err(DispatchFailureError::FailureIdentityConflict);
        }
        transaction.commit().await?;
        match recorded_disposition.as_str() {
            "unavailable" => return Ok(DispatchFailureOutcome::Unavailable),
            "blocked" => return Ok(DispatchFailureOutcome::Blocked),
            "requeued" => {}
            _ => return Err(DispatchFailureError::FailureIdentityConflict),
        }
        return Ok(DispatchFailureOutcome::Replay {
            retry_after_seconds: u32::try_from(retry_after_seconds)
                .unwrap_or(DISPATCH_FAILURE_RETRY_AFTER_SECONDS),
        });
    }

    // Take the task row lock before changing eligibility.  A concurrent
    // Package receipt finishes the same `in_progress` row instead, causing
    // this update to affect zero rows; it must never be put back into pending.
    let requeued: Option<(Uuid, Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        "UPDATE collection_work_order_lease_task task \
         SET execution_state='pending',claimed_at=NULL,claimed_by_installation_ref=NULL \
         FROM collection_work_order_lease lease, linggan_runtime_task runtime \
         WHERE task.task_id=$1 \
           AND task.execution_state='in_progress' \
           AND task.claimed_by_installation_ref=$2 \
           AND lease.lease_ref=task.lease_ref \
           AND runtime.task_id=task.task_id \
           AND lease.released_at IS NULL \
           AND lease.expires_at>scope_001_now() \
         RETURNING task.task_id,lease.lease_ref,lease.work_order_ref, \
                   runtime.task_spec #>> '{capabilitiesRequested,0}', \
                   NULLIF(runtime.task_spec #>> '{target,contentExternalId}','')",
    )
    .bind(task_id)
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((_task_id, lease_ref, work_order_ref, capability, content_external_id)) = requeued
    else {
        return Err(DispatchFailureError::ClaimNotHeld);
    };
    let terminal_disposition = if capability == "content_detail" {
        match (failure_code, content_external_id.as_deref()) {
            (DispatchFailureCode::PageUnavailable, Some(content_external_id)) => {
                Some(("unavailable", content_external_id))
            }
            (DispatchFailureCode::PageReadFailed, Some(content_external_id)) => {
                let prior_failures = page_read_failure_count_for_detail_in_transaction(
                    &mut transaction,
                    work_order_ref,
                    content_external_id,
                )
                .await?;
                if prior_failures + 1 >= MAX_PAGE_READ_FAILURES_PER_DETAIL {
                    Some(("blocked", content_external_id))
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };
    if let Some((disposition, content_external_id)) = terminal_disposition {
        // The generic update above restores the current task to `pending`.
        // Terminalizing all still-pending lanes for the same frozen material
        // prevents its auxiliary work from trapping later, unrelated material
        // behind an unresolved detail read. The failure record below contains
        // only our bounded code/disposition, never raw browser text.
        let terminalized = sqlx::query(
            "UPDATE collection_work_order_lease_task sibling \
             SET execution_state=$3,claimed_at=NULL,claimed_by_installation_ref=NULL \
             FROM linggan_runtime_task sibling_runtime \
             WHERE sibling.lease_ref=$1 \
               AND sibling.task_id=sibling_runtime.task_id \
               AND sibling.execution_state='pending' \
               AND sibling_runtime.task_spec #>> '{target,contentExternalId}'=$2",
        )
        .bind(lease_ref)
        .bind(content_external_id)
        .bind(disposition)
        .execute(&mut *transaction)
        .await?;
        if terminalized.rows_affected() == 0 {
            return Err(DispatchFailureError::ClaimNotHeld);
        }
        record_terminal_dispatch_failure_in_transaction(
            &mut transaction,
            failure_ref,
            task_id,
            installation_ref,
            lease_ref,
            work_order_ref,
            failure_code.as_str(),
            disposition,
        )
        .await?;
        transaction.commit().await?;
        return Ok(if disposition == "unavailable" {
            DispatchFailureOutcome::Unavailable
        } else {
            DispatchFailureOutcome::Blocked
        });
    }
    let retry_after_seconds = record_recoverable_dispatch_failure_in_transaction(
        &mut transaction,
        failure_ref,
        task_id,
        installation_ref,
        lease_ref,
        work_order_ref,
        failure_code.as_str(),
        "dispatch_start_failed",
    )
    .await?;
    transaction.commit().await?;
    Ok(DispatchFailureOutcome::Requeued {
        retry_after_seconds,
    })
}

async fn record_terminal_dispatch_failure_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    failure_ref: Uuid,
    task_id: Uuid,
    installation_ref: Uuid,
    lease_ref: Uuid,
    work_order_ref: Uuid,
    failure_code: &str,
    failure_disposition: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task_dispatch_failure \
             (failure_ref,task_id,installation_ref,failure_code,retry_after_seconds,failure_disposition) \
         VALUES ($1,$2,$3,$4,1,$5)",
    )
    .bind(failure_ref)
    .bind(task_id)
    .bind(installation_ref)
    .bind(failure_code)
    .bind(failure_disposition)
    .execute(&mut **transaction)
    .await?;
    let all_terminal: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM collection_work_order_lease_task \
             WHERE lease_ref=$1 AND execution_state NOT IN ('completed','unavailable','blocked'))",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if all_terminal {
        sqlx::query(
            "UPDATE collection_work_order_lease \
             SET released_at=scope_001_now(),release_reason='partial' \
             WHERE lease_ref=$1 AND released_at IS NULL",
        )
        .bind(lease_ref)
        .execute(&mut **transaction)
        .await?;
        sqlx::query(
            "UPDATE collection_work_order SET queue_state='completed' \
             WHERE work_order_ref=$1 AND queue_state='leased'",
        )
        .bind(work_order_ref)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Count only the pre-Attempt `page_read_failed` events for one frozen detail
/// material in one WorkOrder.  A batch may contain many works and several
/// lanes per work: failures for other works, other lanes, or another request
/// must not consume this material's retry budget.
async fn page_read_failure_count_for_detail_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
    content_external_id: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) \
         FROM collection_work_order_lease_task_dispatch_failure failure \
         JOIN collection_work_order_lease_task task ON task.task_id=failure.task_id \
         JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         WHERE lease.work_order_ref=$1 \
           AND failure.failure_code='page_read_failed' \
           AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
           AND runtime.task_spec #>> '{target,contentExternalId}'=$2",
    )
    .bind(work_order_ref)
    .bind(content_external_id)
    .fetch_one(&mut **transaction)
    .await
}

/// Return the same WorkOrder to the common queue without erasing the old
/// Lease/Task.  Both producer-reported failures and a server-detected missing
/// locator use this path, so neither leaves a deceptive live permission behind.
async fn record_recoverable_dispatch_failure_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    failure_ref: Uuid,
    task_id: Uuid,
    installation_ref: Uuid,
    lease_ref: Uuid,
    work_order_ref: Uuid,
    failure_code: &str,
    release_reason: &str,
) -> Result<u32, sqlx::Error> {
    let failure_count: i32 = sqlx::query_scalar(
        "UPDATE collection_work_order \
         SET dispatch_failure_count=dispatch_failure_count+1 \
         WHERE work_order_ref=$1 AND queue_state='leased' \
         RETURNING dispatch_failure_count",
    )
    .bind(work_order_ref)
    .fetch_one(&mut **transaction)
    .await?;
    let retry_after_seconds = retry_after_seconds_for_failure_count(failure_count);
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task_dispatch_failure \
             (failure_ref,task_id,installation_ref,failure_code,retry_after_seconds,failure_disposition) \
         VALUES ($1,$2,$3,$4,$5,'requeued')",
    )
    .bind(failure_ref)
    .bind(task_id)
    .bind(installation_ref)
    .bind(failure_code)
    .bind(i32::try_from(retry_after_seconds).unwrap_or(i32::MAX))
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE collection_work_order_lease SET released_at=scope_001_now(), \
             release_reason=$2 WHERE lease_ref=$1 AND released_at IS NULL",
    )
    .bind(lease_ref)
    .bind(release_reason)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='queued',station_ref=NULL, \
             installation_ref=NULL,account_ref=NULL,eligibility_ref=NULL, \
             retry_not_before_at=scope_001_now()+make_interval(secs=>$2) \
         WHERE work_order_ref=$1 AND queue_state='leased'",
    )
    .bind(work_order_ref)
    .bind(i32::try_from(retry_after_seconds).unwrap_or(i32::MAX))
    .execute(&mut **transaction)
    .await?;
    Ok(retry_after_seconds)
}

fn retry_after_seconds_for_failure_count(failure_count: i32) -> u32 {
    let exponent = u32::try_from((failure_count - 1).clamp(0, 4)).unwrap_or(0);
    (DISPATCH_FAILURE_RETRY_AFTER_SECONDS.saturating_mul(1_u32 << exponent))
        .min(MAX_DISPATCH_FAILURE_RETRY_AFTER_SECONDS)
}

/// 回答一次「有活吗」。
///
/// 检查顺序有意义：先问闸门，再问风险，再问额度，最后才找任务。倒过来会在闸门关着时
/// 仍去翻队列，然后报一个「暂无任务」——那会让人以为是没活可干，而不是根本不许干。
pub async fn decide_dispatch(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
) -> Result<DispatchDecision, DispatchError> {
    if !dispatch_schema_is_ready(database).await? {
        return Err(DispatchError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;

    let station: Option<(Uuid, Option<Uuid>, Option<i32>)> = sqlx::query_as(
        "SELECT i.installation_ref, i.station_ref, s.daily_work_quota \
         FROM plugin_installation i \
         LEFT JOIN execution_station s \
                ON s.station_ref = i.station_ref AND s.retired_at IS NULL \
         WHERE i.install_key = $1 AND i.superseded_at IS NULL \
         FOR UPDATE OF i",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((installation_ref, station_ref, quota)) = station else {
        return Err(DispatchError::UnknownInstallation);
    };
    if !validate_installation_credential_in(
        &mut transaction,
        installation_ref,
        installation_credential,
    )
    .await?
    {
        return Err(DispatchError::InvalidCredential);
    }
    // A lapsed lease is historical execution state, not permanent ownership of
    // the queue entry. Requeue it before this installation evaluates new work.
    expire_lapsed_leases_in_transaction(&mut transaction).await?;
    // Older releases created before the common release/requeue boundary can
    // have no live lease but still be marked `leased`.  Recover only orders
    // with runnable work; terminal historical orders never reopen.
    recover_released_orphaned_work_orders_in_transaction(&mut transaction).await?;
    // 过了保质期的工单在这里终结，而不是靠上面那条候选查询把它们滤掉了事。
    // 只滤掉的话，它们会永远以「等待」的样子留在队列里——页面显示等待 5 单，其中 3 单
    // 永远不会跑，这正是 2026-09-08 那一天人看着队列却什么也看不出来的原因之一。
    expire_stale_queued_work_orders_in_transaction(&mut transaction).await?;
    let (Some(station_ref), Some(_quota)) = (station_ref, quota) else {
        // Expiry recovery above is durable even when this caller has not yet
        // claimed a station.  Returning without a commit would silently roll
        // that recovery back whenever an unclaimed installation happened to
        // be the next poller.
        transaction.commit().await?;
        return Ok(DispatchDecision::InstallationNotClaimed);
    };

    // A committed claim response can be lost. Serializing on the installation row and replaying
    // its same live task makes retry idempotent; without this, the task remains in progress until
    // lease expiry or a concurrent poll claims unrelated work for the same installation.
    let in_progress: Option<(Uuid, Uuid, Value)> = sqlx::query_as(
        "SELECT task.task_id, lease.lease_ref, runtime.task_spec \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease ON lease.lease_ref = task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id = task.task_id \
         WHERE task.claimed_by_installation_ref = $1 \
           AND task.execution_state = 'in_progress' \
           AND lease.released_at IS NULL \
           AND lease.expires_at > scope_001_now() \
         ORDER BY task.claimed_at, task.sequence_no \
         LIMIT 1 FOR UPDATE OF task",
    )
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((task_id, lease_ref, task_spec)) = in_progress {
        if let Some(reason_code) =
            revalidate_dispatch_task(&mut transaction, installation_ref, lease_ref, &task_spec)
                .await?
        {
            transaction.commit().await?;
            return Ok(DispatchDecision::ControlBlocked { reason_code });
        }
        let execution_source_url =
            execution_source_url_for_task(&mut transaction, &task_spec).await?;
        if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
            // Do not release an in-progress task: an earlier response may
            // have been lost after the browser actually began its Attempt.
            // We still commit any unrelated expired-lease recovery above.
            transaction.commit().await?;
            return Ok(DispatchDecision::ExecutionLocatorUnavailable {
                reason: "这篇作品当前没有带 xsec_token 的已接纳发现链接，未交给插件执行。"
                    .to_owned(),
            });
        }
        let page_session_plan =
            page_session_plan_for_task(&mut transaction, lease_ref, &task_spec).await?;
        transaction.commit().await?;
        return Ok(DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
            execution_source_url,
            page_session_plan,
        });
    }

    // 先取候选，因为风险暂停是**按平台与 lane 限定范围**的：不知道这次要做的是哪条
    // lane，就无法判断覆盖它的暂停是否生效。
    //
    // 顺序保证「有任务却被拦」不会被报成「没有任务」：只有确实没有候选时才回
    // `NothingWaiting`。
    let waiting: Option<(Uuid, Uuid, Value, String, String)> = sqlx::query_as(
        "SELECT task.task_id, lease.lease_ref, runtime.task_spec, runtime.platform, work_order.lane \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease ON lease.lease_ref = task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id = task.task_id \
         JOIN collection_work_order work_order ON work_order.work_order_ref = lease.work_order_ref \
         WHERE lease.station_ref = $1 \
           AND lease.released_at IS NULL \
           AND lease.expires_at > scope_001_now() \
           AND task.execution_state = 'pending' \
           AND NOT EXISTS ( \
               SELECT 1 FROM collection_work_order_lease_task prior \
               WHERE prior.lease_ref = task.lease_ref \
                 AND prior.sequence_no < task.sequence_no \
                 AND prior.execution_state NOT IN ('completed','unavailable','blocked')) \
         ORDER BY lease.issued_at, task.sequence_no \
         LIMIT 1 FOR UPDATE OF task SKIP LOCKED",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;

    let Some((task_id, lease_ref, task_spec, _platform, _lane)) = waiting else {
        if let Some(decision) =
            claim_next_queued_work_order(&mut transaction, installation_ref, station_ref).await?
        {
            transaction.commit().await?;
            return Ok(decision);
        }
        transaction.commit().await?;
        return Ok(DispatchDecision::NothingWaiting);
    };

    if let Some(reason_code) =
        revalidate_dispatch_task(&mut transaction, installation_ref, lease_ref, &task_spec).await?
    {
        transaction.commit().await?;
        return Ok(DispatchDecision::ControlBlocked { reason_code });
    }

    let execution_source_url = execution_source_url_for_task(&mut transaction, &task_spec).await?;
    if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
        // This task is still pending: no browser Attempt has begun.  Release
        // the permission now rather than allowing a locator defect to consume
        // the station/account/platform slot until the lease naturally expires.
        let work_order_ref: Uuid = sqlx::query_scalar(
            "SELECT work_order_ref FROM collection_work_order_lease WHERE lease_ref=$1",
        )
        .bind(lease_ref)
        .fetch_one(&mut *transaction)
        .await?;
        let _retry_after_seconds = record_recoverable_dispatch_failure_in_transaction(
            &mut transaction,
            Uuid::new_v4(),
            task_id,
            installation_ref,
            lease_ref,
            work_order_ref,
            "execution_locator_unavailable",
            "execution_locator_unavailable",
        )
        .await?;
        transaction.commit().await?;
        return Ok(DispatchDecision::ExecutionLocatorUnavailable {
            reason: "这篇作品当前没有带 xsec_token 的已接纳发现链接；已释放许可并进入冷却重试。"
                .to_owned(),
        });
    }
    let page_session_plan =
        page_session_plan_for_task(&mut transaction, lease_ref, &task_spec).await?;

    let claimed = sqlx::query(
        "UPDATE collection_work_order_lease_task \
         SET execution_state = 'in_progress', claimed_at = scope_001_now(), \
             claimed_by_installation_ref = $2 \
         WHERE task_id = $1 AND execution_state = 'pending'",
    )
    .bind(task_id)
    .bind(installation_ref)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    if claimed != 1 {
        transaction.commit().await?;
        return Ok(DispatchDecision::NothingWaiting);
    }

    transaction.commit().await?;
    Ok(DispatchDecision::Dispatch {
        task_id,
        lease_ref,
        task_spec,
        execution_source_url,
        page_session_plan,
    })
}

/// Claim one compatible Work Order from the shared queue. The lane fairness
/// rows are a scheduling policy only; the Work Order remains the one durable
/// queue authority and a Lease/RuntimeTask exists only after this succeeds.
async fn claim_next_queued_work_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    caller_station_ref: Uuid,
) -> Result<Option<DispatchDecision>, sqlx::Error> {
    type Lane = (String, i32, Option<i32>, f64);
    type Candidate = (
        Uuid,
        String,
        String,
        String,
        bool,
        bool,
        bool,
        bool,
        i32,
        String,
    );
    let mut lanes: Vec<Lane> = sqlx::query_as(
        "SELECT dispatch_lane,weight,concurrent_cap,virtual_finish \
         FROM collection_dispatch_lane_fairness FOR UPDATE",
    )
    .fetch_all(&mut **transaction)
    .await?;
    if lanes.is_empty() {
        return Ok(None);
    }
    let active: Vec<(String, i64)> = sqlx::query_as(
        "SELECT dispatch_lane,count(*) FROM collection_work_order \
         WHERE queue_state='leased' GROUP BY dispatch_lane",
    )
    .fetch_all(&mut **transaction)
    .await?;
    let active_by_lane = active
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    // `virtual_finish` already stores estimated work / weight. Dividing by
    // weight again made high-weight lanes receive a double preference.
    lanes.sort_by(|left, right| {
        left.3
            .total_cmp(&right.3)
            .then_with(|| left.0.cmp(&right.0))
    });

    // A ready WorkOrder which this exact installation cannot claim is not the
    // same thing as an empty queue.  Keep the first stable control reason as
    // a fallback, but continue looking: another lane/platform may still be
    // runnable by this worker during the same poll.
    let mut deferred_control_block: Option<String> = None;
    // 跨行业作用域表（`0074`）不在每一套 schema 里：控制面的证明库只装了它需要的那一段
    // 迁移，跨行业那一串依赖的 `0044` 不在其中。派发必须在两种库上都跑得起来，所以
    // 这里先问一次表在不在，而不是让整条派发在缺表时 42P01。
    let cross_industry_scope_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    for (dispatch_lane, weight, concurrent_cap, _virtual_finish) in lanes {
        if concurrent_cap.is_some_and(|cap| {
            active_by_lane.get(&dispatch_lane).copied().unwrap_or(0) >= i64::from(cap)
        }) {
            continue;
        }
        // This is a bounded eligibility probe, not a module queue. A station
        // that cannot run one candidate continues through the lane and then
        // through the other lanes rather than making all later work invisible.
        let candidates: Vec<Candidate> = sqlx::query_as(if cross_industry_scope_ready {
            CANDIDATE_SQL_WITH_CROSS_INDUSTRY_SCOPE
        } else {
            CANDIDATE_SQL
        })
        .bind(&dispatch_lane)
        .fetch_all(&mut **transaction)
        .await?;
        let mut seen_batch_groups = std::collections::BTreeSet::new();
        for (
            work_order_ref,
            platform,
            target_kind,
            lane,
            has_material_scope,
            collects_comments,
            collects_replies,
            acquires_media,
            units,
            dispatch_group_key,
        ) in candidates
        {
            // A bounded locked probe may contain several ready batches from
            // one source. Consider only its oldest here; next claim starts at
            // the least-recently leased group, yielding durable round-robin
            // without another queue or in-memory cursor.
            if dispatch_lane == "batch" && !seen_batch_groups.insert(dispatch_group_key) {
                continue;
            }
            let required = required_capabilities_for(
                &target_kind,
                &lane,
                has_material_scope,
                collects_comments,
                collects_replies,
                acquires_media,
            );
            let selection = evaluate_claiming_installation_capacity_in(
                transaction,
                installation_ref,
                &platform,
                &lane,
                &required,
            )
            .await?;
            let (Some(station_ref), Some(bound_installation_ref)) =
                (selection.station_ref, selection.installation_ref)
            else {
                deferred_control_block
                    .get_or_insert_with(|| selection.capacity.reason_code().to_owned());
                continue;
            };
            if station_ref != caller_station_ref || bound_installation_ref != installation_ref {
                continue;
            }
            let lease = match claim_queued_work_order_in_transaction(
                transaction,
                work_order_ref,
                station_ref,
                installation_ref,
                selection.account_ref,
                selection.eligibility_ref,
                CLAIM_LEASE_MINUTES,
            )
            .await
            {
                Ok(lease) => lease,
                // 发租被拦住说的是**这一张工单**此刻不能发，不是整条队列没活。此前这里直接
                // 返回，等于让队首那一张挡住它后面所有人：2026-09-08 实测 immediate 队首是
                // 一张 09-05 授权已被撤销的建档工单，此后每一次领活都停在它身上，当天两张
                // 人工观察工单一次都没有被看过。与上面容量不足的处理保持一致——记下第一个
                // 稳定原因当兜底，然后继续往下看。
                Err(LeaseError::ControlBlocked { reason_code }) => {
                    undo_failed_queue_claim(transaction, work_order_ref).await?;
                    deferred_control_block.get_or_insert(reason_code);
                    continue;
                }
                // 授权没了就不会自己回来，这张工单永远不可能再执行。放回队列等于每一轮都
                // 重试一次注定失败的事，而且它还占着队列位置。终结它。
                //
                // 终结原因不另存一列：工单经 `decision_ref → authorization_ref` 指着那份
                // 授权，撤销原因与到期时间都还在那里，比在这里复述一遍更不容易走样。
                Err(LeaseError::AuthorizationLapsed) => {
                    cancel_work_order_without_authorization(transaction, work_order_ref).await?;
                    deferred_control_block
                        .get_or_insert_with(|| "authorization_expired_or_revoked".to_owned());
                    continue;
                }
                Err(LeaseError::Database(error)) => return Err(error),
                Err(_) => {
                    undo_failed_queue_claim(transaction, work_order_ref).await?;
                    continue;
                }
            };
            let Some(task_id) = lease.task_ids.first().copied() else {
                return Ok(Some(DispatchDecision::ControlBlocked {
                    reason_code: "capability_missing".to_owned(),
                }));
            };
            let task_spec: Value =
                sqlx::query_scalar("SELECT task_spec FROM linggan_runtime_task WHERE task_id=$1")
                    .bind(task_id)
                    .fetch_one(&mut **transaction)
                    .await?;
            let execution_source_url =
                execution_source_url_for_task(transaction, &task_spec).await?;
            if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
                let _retry_after_seconds = record_recoverable_dispatch_failure_in_transaction(
                    transaction,
                    Uuid::new_v4(),
                    task_id,
                    installation_ref,
                    lease.lease_ref,
                    work_order_ref,
                    "execution_locator_unavailable",
                    "execution_locator_unavailable",
                )
                .await?;
                return Ok(Some(DispatchDecision::ExecutionLocatorUnavailable {
                    reason: "已认领的详情任务缺少仍有效的签名执行链接；许可已释放并进入冷却重试。"
                        .to_owned(),
                }));
            }
            let page_session_plan =
                page_session_plan_for_task(transaction, lease.lease_ref, &task_spec).await?;
            let claimed = sqlx::query(
                "UPDATE collection_work_order_lease_task \
                 SET execution_state='in_progress',claimed_at=scope_001_now(), \
                     claimed_by_installation_ref=$2 \
                 WHERE task_id=$1 AND execution_state='pending'",
            )
            .bind(task_id)
            .bind(installation_ref)
            .execute(&mut **transaction)
            .await?
            .rows_affected();
            if claimed != 1 {
                return Ok(None);
            }
            sqlx::query(
                "UPDATE collection_dispatch_lane_fairness \
                 SET virtual_finish=virtual_finish + ($2::double precision/$3::double precision), \
                     updated_at=scope_001_now() WHERE dispatch_lane=$1",
            )
            .bind(&dispatch_lane)
            .bind(units)
            .bind(weight)
            .execute(&mut **transaction)
            .await?;
            return Ok(Some(DispatchDecision::Dispatch {
                task_id,
                lease_ref: lease.lease_ref,
                task_spec,
                execution_source_url,
                page_session_plan,
            }));
        }
    }
    Ok(deferred_control_block
        .map(|reason_code| Some(DispatchDecision::ControlBlocked { reason_code }))
        .unwrap_or(None))
}

async fn undo_failed_queue_claim(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='queued',station_ref=NULL, \
             installation_ref=NULL,account_ref=NULL,eligibility_ref=NULL \
         WHERE work_order_ref=$1 AND queue_state='leased' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_lease \
                           WHERE work_order_ref=$1 AND released_at IS NULL)",
    )
    .bind(work_order_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// 终结过了保质期的排队工单。
///
/// 一张昨天的工单回答的是昨天的问题。人可以再点一次、巡检下一轮会再来、建档可以重新
/// 发起——每条通道都有自己的再生方式，没有哪一张值得无限期等下去；而只要它还在队列里，
/// 它就一直是最老的那一张，一直排在最前面。
///
/// 只动 `queued`：`leased` 表示它此刻真的在跑，一次派发扫描不该把正在跑的活取消掉。
/// 它的租约结束后会回到队列，那时这条规则才轮到它。
async fn expire_stale_queued_work_orders_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE collection_work_order \
         SET queue_state='cancelled',station_ref=NULL,installation_ref=NULL, \
             account_ref=NULL,eligibility_ref=NULL \
         WHERE queue_state='queued' AND expires_at IS NOT NULL \
           AND expires_at<=scope_001_now()",
    )
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// 终结一张授权已经不在了的工单。
///
/// 与 `undo_failed_queue_claim` 的唯一差别是终点：那边是「这次没轮到你，回队列等」，
/// 这边是「你等的那个条件已经不存在了」。同样只动没有存活租约的那一行——真在跑的活
/// 不能被一次派发扫描顺手取消。
async fn cancel_work_order_without_authorization(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_order_ref: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE collection_work_order SET queue_state='cancelled',station_ref=NULL, \
             installation_ref=NULL,account_ref=NULL,eligibility_ref=NULL \
         WHERE work_order_ref=$1 AND queue_state='leased' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_lease \
                           WHERE work_order_ref=$1 AND released_at IS NULL)",
    )
    .bind(work_order_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn revalidate_dispatch_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    caller_installation_ref: Uuid,
    lease_ref: Uuid,
    task_spec: &Value,
) -> Result<Option<String>, sqlx::Error> {
    type DispatchControlRow = (
        String,
        String,
        Uuid,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        Option<bool>,
        String,
        String,
        Option<Uuid>,
    );
    let control: Option<DispatchControlRow> = sqlx::query_as(
        "SELECT target.platform,work_order.lane,lease.station_ref, \
                work_order.installation_ref,work_order.account_ref, \
                work_order.monitor_rule_revision_ref,owning_rule.active_revision_ref, \
                current_revision.automatic_enabled,target.lifecycle_state,request.requested_by, \
                decision.authorization_ref \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order work_order USING(work_order_ref) \
         JOIN collection_observation_target target USING(target_ref) \
         JOIN collection_admission_decision decision USING(decision_ref) \
         JOIN collection_acquisition_request request USING(request_ref) \
         -- 与发租那一处同一个判据：比的是**同一条规则**的当前版本。
         LEFT JOIN collection_monitor_rule_revision frozen \
           ON frozen.rule_revision_ref=work_order.monitor_rule_revision_ref \
         LEFT JOIN collection_monitor_rule owning_rule \
           ON owning_rule.rule_ref=frozen.rule_ref AND owning_rule.retired_at IS NULL \
         LEFT JOIN collection_monitor_rule_revision current_revision \
           ON current_revision.rule_revision_ref=owning_rule.active_revision_ref \
         WHERE lease.lease_ref=$1 AND lease.released_at IS NULL \
           AND lease.expires_at>scope_001_now() FOR UPDATE OF lease,target",
    )
    .bind(lease_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((
        platform,
        lane,
        station_ref,
        installation_ref,
        account_ref,
        frozen_rule_ref,
        active_rule_ref,
        automatic_enabled,
        lifecycle_state,
        requested_by,
        authorization_ref,
    )) = control
    else {
        return Ok(Some("station_unavailable".to_owned()));
    };
    let Some(installation_ref) = installation_ref else {
        return Ok(Some("station_unavailable".to_owned()));
    };
    if installation_ref != caller_installation_ref {
        return Ok(Some("station_unavailable".to_owned()));
    }
    let authorization_valid: bool = if let Some(authorization_ref) = authorization_ref {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM collection_acquisition_authorization \
             WHERE authorization_ref=$1 AND revoked_at IS NULL \
               AND expires_at>scope_001_now())",
        )
        .bind(authorization_ref)
        .fetch_one(&mut **transaction)
        .await?
    } else {
        false
    };
    if !authorization_valid {
        return Ok(Some("authorization_expired_or_revoked".to_owned()));
    }
    if rule_revision_blocks_dispatch(&requested_by, &lane, frozen_rule_ref, active_rule_ref) {
        return Ok(Some("rule_revision_changed".to_owned()));
    }
    if requested_by == "agent"
        && lane == "patrol"
        && (frozen_rule_ref.is_none()
            || automatic_enabled != Some(true)
            || lifecycle_state != "monitoring")
    {
        return Ok(Some("monitoring_paused".to_owned()));
    }
    if requested_by == "person" && lifecycle_state == "dismissed" {
        return Ok(Some("target_not_requestable".to_owned()));
    }
    let Some(capability) = task_spec
        .get("capabilitiesRequested")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
    else {
        return Ok(Some("capability_missing".to_owned()));
    };
    let capacity = revalidate_frozen_capacity_in(
        transaction,
        &platform,
        &lane,
        &[capability],
        station_ref,
        installation_ref,
        account_ref,
        Some(lease_ref),
        true,
    )
    .await?;
    Ok((!matches!(
        capacity.capacity,
        linggan_contracts::Capacity::Available { .. }
    ))
    .then(|| capacity.capacity.reason_code().to_owned()))
}

/// 把已经冻结在 Work Order 中的同页读取范围交给 `content_detail` 执行。
///
/// 这里没有创造复合 Task：四个 lane 仍按自己的 TaskSpec、Attempt、Package、Receipt
/// 逐一完成。计划只允许插件在第一次打开详情页时顺手读取后续已批准的数据并暂存，避免
/// 同一作品为了媒体卡槽、评论和回复重复打开四次。
async fn page_session_plan_for_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lease_ref: Uuid,
    task_spec: &Value,
) -> Result<Option<Value>, sqlx::Error> {
    let capability = task_spec
        .get("capabilitiesRequested")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str);
    if capability != Some("content_detail") {
        return Ok(None);
    }
    let Some(content_external_id) = task_spec
        .pointer("/target/contentExternalId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let scope: Option<(i32, i32, bool, i64)> = sqlx::query_as(
        "SELECT target.comment_limit,target.reply_expand_limit,target.acquire_media, \
                GREATEST(1,FLOOR(EXTRACT(EPOCH FROM (lease.expires_at-scope_001_now()))))::bigint \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order_material_target target \
           ON target.work_order_ref=lease.work_order_ref \
         JOIN linggan_material_content content \
           ON content.public_ref=target.content_public_ref \
         WHERE lease.lease_ref=$1 AND content.content_external_id=$2 LIMIT 1",
    )
    .bind(lease_ref)
    .bind(content_external_id)
    .fetch_optional(&mut **transaction)
    .await?;
    // 跨行业参照物的作用域住另一张表（`0074`/`0087`）。漏掉这一侧，关键词的跨行业详情补采
    // 虽然会各自派出 `comments` 任务，却是每样东西再打开一次详情页——「一次打开顺手读完」
    // 这件事就只发生在证据侧，而 Mog 要的恰好是两侧一致。
    let scope = match scope {
        Some(scope) => Some(scope),
        None => load_cross_industry_page_scope(transaction, lease_ref, content_external_id).await?,
    };
    let Some((comment_limit, reply_expand_limit, acquire_media, ttl_seconds)) = scope else {
        return Ok(None);
    };

    let mut lanes = vec!["content_detail"];
    if acquire_media {
        lanes.push("media_slots");
    }
    if comment_limit > 0 {
        lanes.push("comments");
    }
    if reply_expand_limit > 0 {
        lanes.push("replies");
    }
    Ok(Some(serde_json::json!({
        "contractVersion": "linggan.detail-page-session.v1",
        "contentExternalId": content_external_id,
        "lanes": lanes,
        "commentLimit": comment_limit,
        "replyExpandLimit": reply_expand_limit,
        "cacheTtlSeconds": ttl_seconds,
    })))
}

/// 跨行业参照物的同页读取范围。与证据侧那一段同一个问题、另一张表。
///
/// 媒体恒为 false：跨行业作用域没有媒体授权这一项，它不是「没查」，是这一侧不存在这件事。
async fn load_cross_industry_page_scope(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    lease_ref: Uuid,
    content_external_id: &str,
) -> Result<Option<(i32, i32, bool, i64)>, sqlx::Error> {
    // 跨行业作用域表不在每一套 schema 里，与派发挑候选时同一个理由（`0074` 依赖的 `0044`
    // 不在控制面证明库中）。缺表就不是 None 的另一种说法，所以这里先问再读。
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await?;
    if !schema_ready {
        return Ok(None);
    }
    sqlx::query_as(
        "SELECT scope.comment_limit,scope.reply_expand_limit,false, \
                GREATEST(1,FLOOR(EXTRACT(EPOCH FROM (lease.expires_at-scope_001_now()))))::bigint \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order_cross_industry_target scope \
           ON scope.work_order_ref=lease.work_order_ref \
         JOIN cross_industry_sample sample USING(sample_ref) \
         WHERE lease.lease_ref=$1 AND sample.content_external_id=$2 LIMIT 1",
    )
    .bind(lease_ref)
    .bind(content_external_id)
    .fetch_optional(&mut **transaction)
    .await
}

/// 「这张工单现在就可以派」。
///
/// 派发与「我排在第几」必须问同一个问题：还没到点、还在退避、已经过期的工单不占队列
/// 位置，把它们算进去会让人看到一个永远不减的数字。
macro_rules! dispatch_ready_predicate_sql {
    () => {
        "work_order.queue_state='queued' \
           AND work_order.retry_not_before_at<=scope_001_now() \
           AND work_order.scheduled_for<=scope_001_now() \
           AND (work_order.expires_at IS NULL OR work_order.expires_at>scope_001_now())"
    };
}

/// 派发的排序键。`$lane` 是当前通道的 SQL 表达式（派发时是绑定参数，读取时是列本身）。
///
/// **排队位置必须用这把尺子量**，否则界面上的「前面还有 2 个」与真正的出队顺序无关——
/// batch 通道那一项轮转公平（同一来源上次发租越久远越靠前）会让直觉上的「先来先到」
/// 完全不成立。
macro_rules! dispatch_order_sql {
    ($lane:expr) => {
        concat!(
            "CASE WHEN ",
            $lane,
            "='batch' THEN COALESCE(( \
                        SELECT EXTRACT(EPOCH FROM max(prior_lease.issued_at))::bigint \
                        FROM collection_work_order prior \
                        JOIN collection_work_order_lease prior_lease USING(work_order_ref) \
                        WHERE prior.dispatch_lane='batch' \
                          AND prior.dispatch_group_key=work_order.dispatch_group_key \
                    ),-1) ELSE 0 END, \
                    work_order.scheduled_for,work_order.created_at,work_order.work_order_ref"
        )
    };
}

pub(crate) use {dispatch_order_sql, dispatch_ready_predicate_sql};

/// 候选工单的选取。两份的差别只有一处：**要不要问跨行业那一侧的作用域**。
///
/// 这几问决定要求工位具备哪些能力（冻结了就要 `content_detail`，冻结的额度里有评论就还要
/// `comments`/`replies`，没冻结就是发现面）。跨行业详情补采把作用域放在 `0074` 那张表上，
/// 漏问它会让补详情的工单被当成发现任务派给一个不具备详情能力的工位；`0087` 给那张表补上
/// 额度列之后，**「这一单要不要读评论」也必须同时问两侧**，否则一张要读评论的工单会被派给
/// 一个读不了评论的工位。媒体不在此列：跨行业作用域没有媒体授权这一项。
const CANDIDATE_SQL: &str = concat!(
    "SELECT work_order.work_order_ref,target.platform,target.target_kind,work_order.lane, \
                    EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                            WHERE scope.work_order_ref=work_order.work_order_ref), \
                    EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                            WHERE scope.work_order_ref=work_order.work_order_ref \
                              AND scope.comment_limit>0), \
                    EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                            WHERE scope.work_order_ref=work_order.work_order_ref \
                              AND scope.reply_expand_limit>0), \
                    EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                            WHERE scope.work_order_ref=work_order.work_order_ref \
                              AND scope.acquire_media), \
                    work_order.estimated_work_units, \
                    COALESCE(work_order.dispatch_group_key, \
                             concat('work:',work_order.work_order_ref::text)) \
             FROM collection_work_order work_order \
             JOIN collection_observation_target target USING(target_ref) \
             WHERE work_order.dispatch_lane=$1 AND ",
    dispatch_ready_predicate_sql!(),
    " ORDER BY ",
    dispatch_order_sql!("$1"),
    " LIMIT 64 FOR UPDATE OF work_order SKIP LOCKED",
);

const CANDIDATE_SQL_WITH_CROSS_INDUSTRY_SCOPE: &str = concat!(
    "SELECT work_order.work_order_ref,target.platform,target.target_kind,work_order.lane, \
                    (EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                             WHERE scope.work_order_ref=work_order.work_order_ref) \
                     OR EXISTS (SELECT 1 FROM collection_work_order_cross_industry_target scope \
                                WHERE scope.work_order_ref=work_order.work_order_ref)), \
                    (EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                             WHERE scope.work_order_ref=work_order.work_order_ref \
                               AND scope.comment_limit>0) \
                     OR EXISTS (SELECT 1 FROM collection_work_order_cross_industry_target scope \
                                WHERE scope.work_order_ref=work_order.work_order_ref \
                                  AND scope.comment_limit>0)), \
                    (EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                             WHERE scope.work_order_ref=work_order.work_order_ref \
                               AND scope.reply_expand_limit>0) \
                     OR EXISTS (SELECT 1 FROM collection_work_order_cross_industry_target scope \
                                WHERE scope.work_order_ref=work_order.work_order_ref \
                                  AND scope.reply_expand_limit>0)), \
                    EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                            WHERE scope.work_order_ref=work_order.work_order_ref \
                              AND scope.acquire_media), \
                    work_order.estimated_work_units, \
                    COALESCE(work_order.dispatch_group_key, \
                             concat('work:',work_order.work_order_ref::text)) \
             FROM collection_work_order work_order \
             JOIN collection_observation_target target USING(target_ref) \
             WHERE work_order.dispatch_lane=$1 AND ",
    dispatch_ready_predicate_sql!(),
    " ORDER BY ",
    dispatch_order_sql!("$1"),
    " LIMIT 64 FOR UPDATE OF work_order SKIP LOCKED",
);

fn requires_signed_execution_source(task_spec: &Value) -> bool {
    task_spec.get("platform").and_then(Value::as_str) == Some("xhs")
        && task_spec
            .get("capabilitiesRequested")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .is_some_and(|capability| {
                matches!(
                    capability,
                    "content_detail" | "media_slots" | "comments" | "replies"
                )
            })
}

/// 发现链接是可过期的执行定位信息，不是作品身份。每次派发都从最新已接纳的
/// discovery record 读取，而不把 token 冻结进长寿命 TaskSpec 或 Evidence UI。
async fn execution_source_url_for_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_spec: &Value,
) -> Result<Option<String>, sqlx::Error> {
    if !requires_signed_execution_source(task_spec) {
        return Ok(None);
    }
    let Some(content_external_id) = task_spec
        .get("target")
        .and_then(|target| target.get("contentExternalId"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let from_evidence: Option<String> = sqlx::query_scalar(
        "SELECT record.value->'payload'->>'url' \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') \
              WITH ORDINALITY AS record(value,ordinality) \
         WHERE content.platform='xhs' AND content.content_external_id=$1 \
           AND record.ordinality=finding.record_ordinal+1 \
           AND record.value->'sourceObject'->>'externalId'=$1 \
           AND record.value->'payload'->>'url' LIKE 'https://www.xiaohongshu.com/%' \
           AND position('xsec_token=' IN record.value->'payload'->>'url') > 0 \
         ORDER BY package.accepted_at DESC,finding.created_at DESC LIMIT 1",
    )
    .bind(content_external_id)
    .fetch_optional(&mut **transaction)
    .await?
    .flatten();
    if from_evidence.is_some() {
        return Ok(from_evidence);
    }
    // 跨行业参照物不在证据侧，上面那一查对它必然落空。
    //
    // 外部领域的材料按 `0044` 的隔离住在 `cross_industry_sample`，那张表上的
    // `source_url` 正是列表面当时平台返回的那条签名链接（`signed_source_url` 只收
    // 平台真给过的，从不由作品 ID 拼）。没有这一步，关键词建档的详情补采会在派发这一关
    // 被判为「没有可用执行入口」——活派不出去，而原因看上去像是链接过期。
    let cross_industry_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(&mut **transaction)
            .await?;
    if !cross_industry_ready {
        return Ok(None);
    }
    sqlx::query_scalar(
        "SELECT sample.source_url FROM cross_industry_sample sample \
         WHERE sample.platform='xhs' AND sample.content_external_id=$1 \
           AND sample.source_url LIKE 'https://www.xiaohongshu.com/%' \
           AND position('xsec_token=' IN sample.source_url) > 0 \
         ORDER BY sample.last_observed_at DESC,sample.sample_ref LIMIT 1",
    )
    .bind(content_external_id)
    .fetch_optional(&mut **transaction)
    .await
}

/// Does a changed monitor rule revision stop this dispatch?
///
/// Only a patrol freezes a rule revision, so only a patrol can be compared against one. The
/// write side (`acquisition_chain`) has always frozen `monitor_rule_revision_ref` for `agent`
/// + `patrol` alone; this check omitted the lane, so every agent-issued deep archive carried
/// NULL, never equalled the target's active revision, and was blocked forever. No retry could
/// clear it, because a rule revision does not travel backwards.
///
/// Observed on 2026-09-06: a progressive archive finished its person-issued directory step and
/// then stalled indefinitely on every agent-issued batch behind it. The person-issued
/// "continue" button also appeared dead, because the stuck batches still held those works.
///
/// A rule revision describes patrol cadence. A deep archive's scope is a frozen list of works,
/// decided when the batch was admitted; changing how often a profile is swept says nothing
/// about whether those works may still be captured. The adjacent `monitoring_paused` check
/// scopes itself to `patrol` for the same reason.
fn rule_revision_blocks_dispatch(
    requested_by: &str,
    lane: &str,
    frozen_rule_ref: Option<Uuid>,
    active_rule_ref: Option<Uuid>,
) -> bool {
    requested_by == "agent" && lane == "patrol" && frozen_rule_ref != active_rule_ref
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_is_persistent_bounded_and_never_immediate() {
        assert_eq!(retry_after_seconds_for_failure_count(1), 60);
        assert_eq!(retry_after_seconds_for_failure_count(2), 120);
        assert_eq!(retry_after_seconds_for_failure_count(3), 240);
        assert_eq!(retry_after_seconds_for_failure_count(4), 480);
        assert_eq!(retry_after_seconds_for_failure_count(5), 900);
        assert_eq!(retry_after_seconds_for_failure_count(999), 900);
    }

    /// An agent-issued deep archive never freezes a rule revision, so comparing it against one
    /// can only ever block it. This is the 2026-09-06 stall: a progressive archive that could
    /// not advance a single batch, with no retry able to clear it.
    #[test]
    fn a_changed_rule_revision_only_stops_a_patrol() {
        let active = Some(Uuid::new_v4());

        // The regression: agent deep archives carry NULL and must still dispatch.
        assert!(!rule_revision_blocks_dispatch(
            "agent",
            "deep_archive",
            None,
            active
        ));
        assert!(!rule_revision_blocks_dispatch(
            "agent",
            "deep_archive",
            Some(Uuid::new_v4()),
            active,
        ));

        // Patrol keeps the guard: a superseded rule must not keep sweeping on old cadence.
        assert!(rule_revision_blocks_dispatch(
            "agent", "patrol", None, active
        ));
        assert!(rule_revision_blocks_dispatch(
            "agent",
            "patrol",
            Some(Uuid::new_v4()),
            active,
        ));
        assert!(!rule_revision_blocks_dispatch(
            "agent", "patrol", active, active
        ));

        // A person acts under their own authority and is never gated on rule cadence.
        assert!(!rule_revision_blocks_dispatch(
            "person", "patrol", None, active
        ));
    }
}
