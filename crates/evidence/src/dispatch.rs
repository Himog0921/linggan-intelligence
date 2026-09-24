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
use crate::execution_input_eligibility::{
    MISSING_EXECUTION_INPUT_REASON, PageReadBudget,
    detail_material_already_accepted_in_transaction,
    record_detail_page_read_failure_in_transaction, requires_signed_execution_source,
    signed_locator_predicate, stop_material_for_missing_execution_input_in_transaction,
    task_content_external_id,
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
/// 一次派发里最多为「输入缺失」停几个成员。
///
/// 停止是廉价的（都停完就终结这张工单），但它是循环的出口：没有上限时，一张成员全部缺输入
/// 的工单会让这一次派发把整批成员挨个停完才开始做别的——在队列很长时那不是「马上」，而是
/// 「这一轮都在收尸」。留一个上限，剩下的下一轮继续停，停过的成员不会重新变成候选。
const MAX_MEMBER_STOPS_PER_DISPATCH: usize = 8;

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

/// 导航前为一个冻结通道登记的交付身份。
///
/// 它是「受权的准备」，不是执行事实：登记它不代表这个通道已经被访问、采集、交付或完成。
/// 插件在打开页面前把它持久化，之后同一条 task 的投递都用这个身份，哪怕最初那份租约
/// 已经结束——服务端仍能认出「这是它自己为这次导航登记过的身份」。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedLaneDelivery {
    pub capability: String,
    pub task_id: Uuid,
    pub attempt_id: Uuid,
    pub task_spec: Value,
    pub lease_expires_at: String,
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
        prepared_lanes: Vec<PreparedLaneDelivery>,
    },
    Replay {
        session_ref: Uuid,
        plan: Value,
        prepared_lanes: Vec<PreparedLaneDelivery>,
    },
    Suppressed {
        session_ref: Uuid,
        reason_code: &'static str,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum DetailPageSessionGrantError {
    #[error("new page access is paused during delivery recovery rollout")]
    UpgradeRecoveryOnly,
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
    #[error("the task no longer has an accepted signed execution locator")]
    ExecutionSourceUnavailable,
    #[error("the signed execution locator changed after the task was dispatched")]
    ExecutionSourceChanged,
    #[error("the frozen plan's lanes could not be registered for delivery before navigation")]
    LanePreparationUnavailable,
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

#[derive(Debug, thiserror::Error)]
pub enum DetailPageRiskSignalError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no active plugin installation with that install key")]
    UnknownInstallation,
    #[error("installation credential is invalid")]
    InvalidCredential,
    #[error("the installation no longer holds that live detail task claim")]
    ClaimNotHeld,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailPageRiskSignalReceipt {
    pub cooldown_active: bool,
    pub cooldown_until: Option<String>,
    pub consecutive_count: i64,
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
    DetailPageUrlInvalid,
    DetailPageSessionGrantUnavailable,
    DetailPageSessionLanePreparationUnavailable,
    DetailPageSessionRecoveryRequired,
    CaptureDeliveryRejected,
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
            "detail_page_url_invalid" => Some(Self::DetailPageUrlInvalid),
            "detail_page_session_grant_unavailable" => {
                Some(Self::DetailPageSessionGrantUnavailable)
            }
            "detail_page_session_lane_preparation_unavailable" => {
                Some(Self::DetailPageSessionLanePreparationUnavailable)
            }
            "detail_page_session_recovery_required" => {
                Some(Self::DetailPageSessionRecoveryRequired)
            }
            "capture_delivery_rejected" => Some(Self::CaptureDeliveryRejected),
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
            Self::DetailPageUrlInvalid => "detail_page_url_invalid",
            Self::DetailPageSessionGrantUnavailable => "detail_page_session_grant_unavailable",
            Self::DetailPageSessionLanePreparationUnavailable => {
                "detail_page_session_lane_preparation_unavailable"
            }
            Self::DetailPageSessionRecoveryRequired => "detail_page_session_recovery_required",
            Self::CaptureDeliveryRejected => "capture_delivery_rejected",
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
    #[error("the claimed detail session has no bound execution-source fingerprint")]
    DetailPageSourceUnbound,
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
            Self::ControlBlocked { reason_code } if reason_code == "installation_risk_cooldown" => {
                1800
            }
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
                "collection_upgrade_recovery_only" => "collection_upgrade_recovery_only",
                "risk_paused" => "risk_paused",
                "installation_risk_cooldown" => "installation_risk_cooldown",
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
                AND to_regclass(format('%I.%I',current_schema(),'collection_detail_page_session_grant_attempt')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_detail_page_session_lane_preparation')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_installation_risk_signal')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_installation_risk_cooldown')) IS NOT NULL \
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
///
/// 这是 v1 的入口：它不登记通道交付身份，行为与升级前完全一致。已确认握手版本的插件走
/// [`grant_detail_page_session_with_lane_deliveries`]。
pub async fn grant_detail_page_session(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    grant_request_id: Uuid,
    execution_source_url: &str,
) -> Result<DetailPageSessionGrant, DetailPageSessionGrantError> {
    grant_detail_page_session_inner(
        database,
        install_key,
        installation_credential,
        task_id,
        grant_request_id,
        execution_source_url,
        false,
    )
    .await
}

/// 新插件的入口：在同一个授权事务里，先为冻结计划中的每个通道登记稳定交付身份，
/// 再给出可持久化的准备回执。
///
/// 登记失败即不发这份授权——「先登记交付身份，再进行页面读取」不是建议。插件在打开页面前
/// 把回执写进本机持久状态，之后各通道按各自的本机 outbox 投递，身份不会因为租约结束
/// 而消失。旧插件不请求这件事，服务端也从不把它当作已经准备完成。
pub async fn grant_detail_page_session_with_lane_deliveries(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    grant_request_id: Uuid,
    execution_source_url: &str,
) -> Result<DetailPageSessionGrant, DetailPageSessionGrantError> {
    grant_detail_page_session_inner(
        database,
        install_key,
        installation_credential,
        task_id,
        grant_request_id,
        execution_source_url,
        true,
    )
    .await
}

async fn grant_detail_page_session_inner(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    grant_request_id: Uuid,
    execution_source_url: &str,
    register_lane_deliveries: bool,
) -> Result<DetailPageSessionGrant, DetailPageSessionGrantError> {
    if !crate::collection_governance_enabled() {
        return Err(DetailPageSessionGrantError::UpgradeRecoveryOnly);
    }
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

    type RawClaimedDetail = (Uuid, Uuid, Uuid, Value);
    let claimed: Option<RawClaimedDetail> = sqlx::query_as::<_, RawClaimedDetail>(
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
    .await?;
    let Some((lease_ref, work_order_ref, content_public_ref, task_spec)) = claimed else {
        return Err(DetailPageSessionGrantError::ClaimNotHeld);
    };
    let Some(plan) = page_session_plan_for_task(&mut transaction, lease_ref, &task_spec).await?
    else {
        return Err(DetailPageSessionGrantError::DetailScopeUnavailable);
    };
    // The browser is allowed to navigate only the locator that was actually
    // dispatched.  Resolve it again inside this transaction and store no raw
    // signed URL: a different discovery result must be claimed afresh rather
    // than silently swapping a page beneath an already-consumed grant.
    let Some(current_execution_source_url) =
        execution_source_url_for_task(&mut transaction, &task_spec).await?
    else {
        return Err(DetailPageSessionGrantError::ExecutionSourceUnavailable);
    };
    if current_execution_source_url != execution_source_url {
        return Err(DetailPageSessionGrantError::ExecutionSourceChanged);
    }
    let execution_source_url_sha256 = execution_source_url_sha256(execution_source_url);
    let plan_hash = Sha256::digest(
        serde_json::to_string(&plan)
            .map_err(|error| sqlx::Error::Protocol(error.to_string()))?
            .as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect::<String>();

    type Existing = (
        Uuid,
        Uuid,
        Uuid,
        String,
        Value,
        bool,
        Option<String>,
        String,
    );
    let existing: Option<Existing> = sqlx::query_as(
        "SELECT session.session_ref,session.owner_installation_ref,session.grant_request_id, \
                session.state,session.plan_snapshot,owner.superseded_at IS NOT NULL, \
                session.execution_source_url_sha256,session.plan_hash \
         FROM collection_detail_page_session session \
         JOIN plugin_installation owner ON owner.installation_ref=session.owner_installation_ref \
         WHERE work_order_ref=$1 \
           AND session.content_public_ref=$2 \
         FOR UPDATE OF session,owner",
    )
    .bind(work_order_ref)
    .bind(content_public_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    // 通道身份要绑定到**这次实际发出的那份计划**：新授权用刚冻结的这份，重放用会话里
    // 存着的那份快照。两者用各自的 plan_hash 记录，不互相借用。
    let mut effective_plan_hash = plan_hash.clone();
    let outcome = match existing {
        None => {
            let session_ref = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO collection_detail_page_session \
                     (session_ref,work_order_ref,content_public_ref,owner_installation_ref, \
                      grant_request_id,initial_lease_ref,plan_snapshot,plan_hash,execution_source_url_sha256,state) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'authorized')",
            )
            .bind(session_ref)
            .bind(work_order_ref)
            .bind(content_public_ref)
            .bind(installation_ref)
            .bind(grant_request_id)
            .bind(lease_ref)
            .bind(&plan)
            .bind(&plan_hash)
            .bind(&execution_source_url_sha256)
            .execute(&mut *transaction)
            .await?;
            DetailPageSessionGrant::Authorized {
                session_ref,
                plan,
                prepared_lanes: Vec::new(),
            }
        }
        Some((
            session_ref,
            owner_installation_ref,
            recorded_request_id,
            state,
            snapshot,
            _,
            source_hash,
            stored_plan_hash,
        )) if owner_installation_ref == installation_ref
            && recorded_request_id == grant_request_id
            && source_hash.as_deref() == Some(execution_source_url_sha256.as_str()) =>
        {
            if state == "stopped" {
                DetailPageSessionGrant::Suppressed {
                    session_ref,
                    reason_code: "session_stopped",
                }
            } else {
                effective_plan_hash = stored_plan_hash;
                DetailPageSessionGrant::Replay {
                    session_ref,
                    plan: snapshot,
                    prepared_lanes: Vec::new(),
                }
            }
        }
        Some((session_ref, _, _, state, _, owner_superseded, _, _)) => {
            if owner_superseded && state != "stopped" {
                sqlx::query(
                    "UPDATE collection_detail_page_session \
                     SET state='stopped',stop_reason='owner_unavailable',finished_at=scope_001_now(), \
                         last_progress_at=scope_001_now() \
                     WHERE session_ref=$1 AND state NOT IN ('finished','stopped')",
                )
                .bind(session_ref)
                .execute(&mut *transaction)
                .await?;
            }
            DetailPageSessionGrant::Suppressed {
                session_ref,
                reason_code: if owner_superseded {
                    "session_owner_unavailable"
                } else {
                    "session_already_authorized"
                },
            }
        }
    };
    // 登记发生在会话已经确定、授权事务仍未提交的时候：此刻计划、工单、租约与工位都还锁着。
    // 只有真的会发出可用导航授权的两种结果才登记——被抑制的授权不留下任何身份，否则
    // 「登记了」会被读成「这次导航获得了许可」。
    let prepared_lanes = match (&outcome, register_lane_deliveries) {
        (
            DetailPageSessionGrant::Authorized {
                session_ref, plan, ..
            },
            true,
        )
        | (
            DetailPageSessionGrant::Replay {
                session_ref, plan, ..
            },
            true,
        ) => {
            let Some(prepared_lanes) = register_lane_delivery_identities(
                &mut transaction,
                *session_ref,
                installation_ref,
                lease_ref,
                plan,
                &effective_plan_hash,
            )
            .await?
            else {
                return Err(DetailPageSessionGrantError::LanePreparationUnavailable);
            };
            prepared_lanes
        }
        _ => Vec::new(),
    };
    let outcome = match outcome {
        DetailPageSessionGrant::Authorized {
            session_ref, plan, ..
        } => DetailPageSessionGrant::Authorized {
            session_ref,
            plan,
            prepared_lanes,
        },
        DetailPageSessionGrant::Replay {
            session_ref, plan, ..
        } => DetailPageSessionGrant::Replay {
            session_ref,
            plan,
            prepared_lanes,
        },
        other => other,
    };
    let (grant_outcome, session_ref) = match &outcome {
        DetailPageSessionGrant::Authorized { session_ref, .. } => {
            ("authorized", Some(*session_ref))
        }
        DetailPageSessionGrant::Replay { session_ref, .. } => ("replay", Some(*session_ref)),
        DetailPageSessionGrant::Suppressed { session_ref, .. } => {
            ("suppressed", Some(*session_ref))
        }
    };
    let attempt_no: i64 = sqlx::query_scalar(
        "SELECT count(*) + 1 FROM collection_detail_page_session_grant_attempt \
         WHERE installation_ref=$1 AND grant_request_id=$2",
    )
    .bind(installation_ref)
    .bind(grant_request_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        "INSERT INTO collection_detail_page_session_grant_attempt \
             (grant_attempt_ref,work_order_ref,task_id,lease_ref,installation_ref,grant_request_id,attempt_no,outcome,session_ref) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(Uuid::new_v4())
    .bind(work_order_ref)
    .bind(task_id)
    .bind(lease_ref)
    .bind(installation_ref)
    .bind(grant_request_id)
    .bind(attempt_no as i32)
    .bind(grant_outcome)
    .bind(session_ref)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(outcome)
}

/// 为冻结计划中的每个通道登记一个稳定交付身份。
///
/// 通道不是插件能加的东西：计划里的每个通道必须在这份租约里已经有一个同目标的任务，
/// 且每个通道只有一个。少一个、多一个、或指向别的目标，都不登记——「受权准备」这句话
/// 得对得上工单真正冻结过的范围。
///
/// 一个会话的一个通道只登记一次：同一 `grant_request_id` 重放拿回同一个 `attempt_id`。
/// 返回 `None` 表示这份计划与租约的任务集合对不上，调用方不得发出导航授权。
///
/// 这里不写 `linggan_runtime_attempt`：那张表是「执行真的开始了」的凭据，界面用它区分
/// 「等着工位来干」和「工位正在干」。把准备写成 Attempt 会让一条尚未打开的通道显示成
/// 正在采集。身份先登记在这里，Attempt 仍在第一次投递时产生。
async fn register_lane_delivery_identities(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_ref: Uuid,
    owner_installation_ref: Uuid,
    lease_ref: Uuid,
    plan: &Value,
    plan_hash: &str,
) -> Result<Option<Vec<PreparedLaneDelivery>>, sqlx::Error> {
    let Some(content_external_id) = plan
        .pointer("/contentExternalId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(lanes) = plan.get("lanes").and_then(Value::as_array) else {
        return Ok(None);
    };
    let frozen: Vec<(Uuid, Option<String>, Value, String)> = sqlx::query_as(
        "SELECT runtime.task_id,runtime.task_spec #>> '{capabilitiesRequested,0}',runtime.task_spec,replace((lease.expires_at AT TIME ZONE 'UTC')::text,' ','T') || 'Z' \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         WHERE task.lease_ref=$1 \
           AND runtime.task_spec #>> '{target,contentExternalId}'=$2",
    )
    .bind(lease_ref)
    .bind(content_external_id)
    .fetch_all(&mut **transaction)
    .await?;
    let recorded: Vec<(String, Uuid, Uuid)> = sqlx::query_as(
        "SELECT capability,task_id,attempt_id \
         FROM collection_detail_page_session_lane_preparation WHERE session_ref=$1",
    )
    .bind(session_ref)
    .fetch_all(&mut **transaction)
    .await?;
    let mut prepared = Vec::with_capacity(lanes.len());
    for lane in lanes {
        let Some(capability) = lane
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let mut candidates = frozen.iter().filter(|(_, frozen_capability, _, _)| {
            frozen_capability.as_deref() == Some(capability)
        });
        let Some((task_id, _, task_spec, lease_expires_at)) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Ok(None);
        }
        let task_id = *task_id;
        let attempt_id = match recorded
            .iter()
            .find(|(recorded_capability, _, _)| recorded_capability == capability)
        {
            Some((_, recorded_task_id, attempt_id)) => {
                // 同一份计划里同一个通道永远指向同一个任务。指到别处，说明这份会话
                // 被另一种计划写过；那种情况下宁可拒绝导航，也不混用两组身份。
                if *recorded_task_id != task_id {
                    return Ok(None);
                }
                *attempt_id
            }
            None => {
                let attempt_id = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO collection_detail_page_session_lane_preparation \
                         (preparation_ref,session_ref,owner_installation_ref,capability,task_id, \
                          lease_ref,attempt_id,plan_hash) \
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
                )
                .bind(Uuid::new_v4())
                .bind(session_ref)
                .bind(owner_installation_ref)
                .bind(capability)
                .bind(task_id)
                .bind(lease_ref)
                .bind(attempt_id)
                .bind(plan_hash)
                .execute(&mut **transaction)
                .await?;
                attempt_id
            }
        };
        prepared.push(PreparedLaneDelivery {
            capability: capability.to_owned(),
            task_id,
            attempt_id,
            task_spec: task_spec.clone(),
            lease_expires_at: lease_expires_at.clone(),
        });
    }
    Ok(Some(prepared))
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

/// Record a conclusive risk-control interstitial from an already-open claimed
/// XHS detail page.  The browser supplies only a closed signal and detector
/// version; page text is never persisted. Two independent observations in
/// thirty minutes open an installation-only twelve-hour circuit breaker.
pub async fn report_detail_page_risk_signal(
    database: &Database,
    install_key: &str,
    installation_credential: &str,
    task_id: Uuid,
    risk_signal_ref: Uuid,
    detector_version: &str,
) -> Result<DetailPageRiskSignalReceipt, DetailPageRiskSignalError> {
    if !dispatch_schema_is_ready(database).await? {
        return Err(DetailPageRiskSignalError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let installation_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation WHERE install_key=$1 AND superseded_at IS NULL FOR UPDATE",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(installation_ref) = installation_ref else {
        return Err(DetailPageRiskSignalError::UnknownInstallation);
    };
    if !validate_installation_credential_in(
        &mut transaction,
        installation_ref,
        installation_credential,
    )
    .await?
    {
        return Err(DetailPageRiskSignalError::InvalidCredential);
    }
    let claimed: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT lease.lease_ref,session.session_ref \
         FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
         JOIN linggan_runtime_task runtime ON runtime.task_id=task.task_id \
         LEFT JOIN collection_detail_page_session session ON session.initial_lease_ref=lease.lease_ref \
         WHERE task.task_id=$1 AND task.claimed_by_installation_ref=$2 AND task.execution_state='in_progress' \
           AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
           AND runtime.platform='xhs' AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
         FOR UPDATE OF task,lease",
    )
    .bind(task_id)
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((lease_ref, session_ref)) = claimed else {
        return Err(DetailPageRiskSignalError::ClaimNotHeld);
    };
    sqlx::query(
        "INSERT INTO collection_installation_risk_signal \
             (risk_signal_ref,installation_ref,task_id,lease_ref,detail_page_session_ref,platform,signal_code,detector_version) \
         VALUES ($1,$2,$3,$4,$5,'xhs','risk_control_interstitial',$6) \
         ON CONFLICT (risk_signal_ref) DO NOTHING",
    )
    .bind(risk_signal_ref)
    .bind(installation_ref)
    .bind(task_id)
    .bind(lease_ref)
    .bind(session_ref)
    .bind(detector_version.trim())
    .execute(&mut *transaction)
    .await?;
    let consecutive_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_installation_risk_signal \
         WHERE installation_ref=$1 AND platform='xhs' AND observed_at>=scope_001_now()-interval '30 minutes'",
    )
    .bind(installation_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if consecutive_count >= 2 {
        sqlx::query(
            "INSERT INTO collection_installation_risk_cooldown \
                 (installation_ref,platform,trigger_signal_ref,trigger_count,opened_at,until_at) \
             VALUES ($1,'xhs',$2,$3,scope_001_now(),scope_001_now()+interval '12 hours') \
             ON CONFLICT (installation_ref,platform) DO UPDATE \
             SET trigger_signal_ref=EXCLUDED.trigger_signal_ref,trigger_count=EXCLUDED.trigger_count, \
                 opened_at=EXCLUDED.opened_at,until_at=EXCLUDED.until_at",
        )
        .bind(installation_ref)
        .bind(risk_signal_ref)
        .bind(consecutive_count as i32)
        .execute(&mut *transaction)
        .await?;
    }
    let cooldown_until: Option<String> = sqlx::query_scalar(
        "SELECT linggan_human_moment(until_at) FROM collection_installation_risk_cooldown \
         WHERE installation_ref=$1 AND platform='xhs' AND until_at>scope_001_now()",
    )
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(DetailPageRiskSignalReceipt {
        cooldown_active: cooldown_until.is_some(),
        cooldown_until,
        consecutive_count,
    })
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
    let Some((current_task_id, lease_ref, work_order_ref, capability, content_external_id)) =
        requeued
    else {
        return Err(DispatchFailureError::ClaimNotHeld);
    };
    if failure_code == DispatchFailureCode::CaptureDeliveryRejected {
        // A Package rejection is terminal for this one immutable task, but is
        // not evidence that the shared page cache is bad. Keep its separately
        // approved lanes eligible so already-read facts can still be delivered
        // without a second page visit.
        let terminalized = sqlx::query(
            "UPDATE collection_work_order_lease_task \
             SET execution_state='unavailable',claimed_at=NULL,claimed_by_installation_ref=NULL \
             WHERE task_id=$1 AND execution_state='pending'",
        )
        .bind(current_task_id)
        .execute(&mut *transaction)
        .await?;
        if terminalized.rows_affected() != 1 {
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
            "unavailable",
        )
        .await?;
        transaction.commit().await?;
        return Ok(DispatchFailureOutcome::Unavailable);
    }
    // 这次上报在台账上落到的位置（只有 `content_detail` 的页面读失败会填它）。它带出去给
    // 下面的退避阶梯用：退避问的是「这个缺口试了几次」，而次数现在按需求范围跨工单累计——
    // 换一张工单就从 60 秒重新开始，等于把同一个缺口的等待时间抹掉。
    let mut page_read_budget: Option<PageReadBudget> = None;
    let terminal_disposition = match (failure_code, content_external_id.as_deref()) {
        (DispatchFailureCode::DetailPageUrlInvalid, Some(content_external_id))
            if capability == "content_detail" =>
        {
            // The final document conclusively rejected this exact signed URL.
            // Bind that fact to the server-owned session fingerprint, never to
            // raw URL text or to the permanent content identity. A future
            // discovery can therefore provide a different signed URL without
            // manually clearing a false "content deleted" state.
            let source_bound: Option<Uuid> = sqlx::query_scalar(
                "UPDATE collection_detail_page_session \
                 SET state='stopped',stop_reason='detail_page_url_invalid', \
                     finished_at=scope_001_now(),last_progress_at=scope_001_now() \
                 WHERE initial_lease_ref=$1 AND execution_source_url_sha256 IS NOT NULL \
                   AND state NOT IN ('finished','stopped') \
                 RETURNING session_ref",
            )
            .bind(lease_ref)
            .fetch_optional(&mut *transaction)
            .await?;
            if source_bound.is_none() {
                return Err(DispatchFailureError::DetailPageSourceUnbound);
            }
            Some(("blocked", content_external_id))
        }
        (DispatchFailureCode::PageUnavailable, Some(content_external_id))
        | (
            DispatchFailureCode::DetailPageSessionLanePreparationUnavailable,
            Some(content_external_id),
        )
        | (DispatchFailureCode::DetailPageSessionRecoveryRequired, Some(content_external_id)) => {
            Some(("unavailable", content_external_id))
        }
        (DispatchFailureCode::PageReadFailed, Some(content_external_id))
            if capability == "content_detail" =>
        {
            // 「同一张工单里最多三次」从前是这里唯一的判据，于是每建一张新工单都从零开始，
            // 同一个缺口可以永远「再试三次」（共享库 2026-09-21：同一篇作品进过 21、13、
            // 20、18 张有匹配任务的工单）。现在预算记在台账的**当前资格行**上：按需求范围
            // 跨工单累计，换工单不清零，刷新同一个地址的签名 token 也不清零。
            //
            // 停止判据取两者中**更严**的一条：台账说用尽就停，已上线的「同一工单三次」
            // 也永远不作废——新规则只能收紧，不能放松既有的保护。
            let budget = if detail_material_already_accepted_in_transaction(
                &mut transaction,
                current_task_id,
            )
            .await?
            {
                // 材料已经被接纳：迟到的失败不回退材料事实，也不许把跨工单预算推到停止——
                // 那个预算问的是「还要不要再试」，而这里已经没有要补的东西了。这次尝试
                // 自己的事实仍按原有的「同一工单三次」收尾。
                None
            } else {
                record_detail_page_read_failure_in_transaction(
                    &mut transaction,
                    current_task_id,
                    failure_ref,
                )
                .await?
            };
            let prior_failures = page_read_failure_count_for_detail_in_transaction(
                &mut transaction,
                work_order_ref,
                content_external_id,
            )
            .await?;
            let effective_failures =
                (prior_failures + 1).max(budget.map_or(0, |budget| budget.count));
            let stopped = effective_failures >= MAX_PAGE_READ_FAILURES_PER_DETAIL
                || budget.is_some_and(|budget| budget.exhausted);
            page_read_budget = budget;
            if stopped {
                Some(("blocked", content_external_id))
            } else {
                None
            }
        }
        _ => None,
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
        page_read_budget.map_or(0, |budget| i32::try_from(budget.count).unwrap_or(i32::MAX)),
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
             WHERE lease_ref=$1 \
               AND execution_state NOT IN ('completed','unavailable','blocked','input_blocked'))",
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

/// 一个成员缺执行输入：记下这一次**停止**，再按剩余有效成员判断工单状态。
///
/// 与 `record_recoverable_dispatch_failure_in_transaction` 的分工，正是这两件事的本质区别：
/// 那边是「读了一次没读成」——退避之后值得再试，所以释放整张租约、把工单放回队列；这边是
/// 「这一篇根本没有可用的执行入口」——重试只是把一件不会变的事再问一遍，所以停它自己，
/// 不重排、不进冷却，**同批其余成员的许可原样保留**。
///
/// 只有当所有通道都终止时才结束租约；还有可跑的成员时租约继续有效，那些成员照常执行。
/// 结束时的队列状态按「有没有任何一条通道真的完成过」判断：全部都是停下的记 `cancelled`
/// ——停止不是完成；有完成过的记 `completed`，与既有终态口径一致。
async fn record_input_blocked_dispatch_failure_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    failure_ref: Uuid,
    task_id: Uuid,
    installation_ref: Uuid,
    lease_ref: Uuid,
    work_order_ref: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task_dispatch_failure \
             (failure_ref,task_id,installation_ref,failure_code,retry_after_seconds,failure_disposition) \
         VALUES ($1,$2,$3,$4,1,'input_blocked')",
    )
    .bind(failure_ref)
    .bind(task_id)
    .bind(installation_ref)
    .bind(MISSING_EXECUTION_INPUT_REASON)
    .execute(&mut **transaction)
    .await?;
    let all_terminal: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS ( \
             SELECT 1 FROM collection_work_order_lease_task \
             WHERE lease_ref=$1 \
               AND execution_state NOT IN ('completed','unavailable','blocked','input_blocked'))",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if !all_terminal {
        return Ok(());
    }
    let any_completed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_work_order_lease_task \
           WHERE lease_ref=$1 AND execution_state='completed')",
    )
    .bind(lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at=scope_001_now(),release_reason='input_blocked' \
         WHERE lease_ref=$1 AND released_at IS NULL",
    )
    .bind(lease_ref)
    .execute(&mut **transaction)
    .await?;
    if any_completed {
        sqlx::query(
            "UPDATE collection_work_order SET queue_state='completed' \
             WHERE work_order_ref=$1 AND queue_state='leased'",
        )
        .bind(work_order_ref)
        .execute(&mut **transaction)
        .await?;
    } else {
        // 一条都没跑成，而且剩下的都因为缺输入停了。记 `completed` 会把「一篇都没取回」
        // 说成「这一单做完了」——那正是包规约禁止的「制造成功」。`cancelled` 说的是实事：
        // 这一轮停了；新的有效输入到了，会由准入另开一张后继工单。
        sqlx::query(
            "UPDATE collection_work_order SET queue_state='cancelled',station_ref=NULL, \
                 installation_ref=NULL,account_ref=NULL,eligibility_ref=NULL \
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
    // 这张工单自己的失败次数只是退避阶梯的**下限**：同一个需求范围的失败按台账跨工单累计，
    // 而等待时间要跟着那个更大的数走。它只把等待变长，不会把任何一次重试提前。
    failure_count_floor: i32,
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
    let effective_failure_count = failure_count.max(failure_count_floor);
    let retry_after_seconds = retry_after_seconds_for_failure_count(effective_failure_count);
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

pub(crate) fn retry_after_seconds_for_failure_count(failure_count: i32) -> u32 {
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
    if !crate::collection_governance_enabled() {
        return Ok(DispatchDecision::ControlBlocked {
            reason_code: "collection_upgrade_recovery_only".to_owned(),
        });
    }
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
    //
    // 循环是为了「停一个成员、当场继续下一个」。一批里有一篇没有执行地址时，停它自己就够
    // 了，同批其余作品应该在同一次轮询里照常派出去；不循环的话它们要等下一轮，而人看到的
    // 是「这一轮什么也没派」——一次针对单篇的停止被读成整批停摆。
    for _ in 0..MAX_MEMBER_STOPS_PER_DISPATCH {
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
                     AND prior.execution_state NOT IN \
                         ('completed','unavailable','blocked','input_blocked')) \
             ORDER BY lease.issued_at, task.sequence_no \
             LIMIT 1 FOR UPDATE OF task SKIP LOCKED",
        )
        .bind(station_ref)
        .fetch_optional(&mut *transaction)
        .await?;

        let Some((task_id, lease_ref, task_spec, _platform, _lane)) = waiting else {
            let Some(claimed) =
                claim_next_queued_work_order(&mut transaction, installation_ref, station_ref)
                    .await?
            else {
                transaction.commit().await?;
                return Ok(DispatchDecision::NothingWaiting);
            };
            match claimed {
                ClaimedWork::StoppedMember => continue,
                ClaimedWork::Decision(decision) => {
                    transaction.commit().await?;
                    return Ok(decision);
                }
            }
        };

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
            // 这个任务还是 pending：浏览器一次都没开始过。停它自己——停的方式是收束它那
            // 一篇的全部通道并记进执行资格台账，**不释放整张租约**。释放整张租约正是那条
            // 连转三小时的老路：同批地址完好的作品被一遍遍放回队列，永远轮不到。
            stop_member_for_missing_execution_input(&mut transaction, installation_ref, task_id)
                .await?;
            continue;
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
        return Ok(DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
            execution_source_url,
            page_session_plan,
        });
    }
    // 一轮里停下的成员多到没停下脚。剩下的下一轮照常处理——这里如实说「这一轮没有可派
    // 的」，而不是把一张还没跑的工单报成别的结论。
    transaction.commit().await?;
    Ok(DispatchDecision::NothingWaiting)
}

/// 停掉一个缺执行输入的成员：台账记原因、收束它那一篇的其余通道、记一条停止事件。
///
/// 三件事一起做，且都不碰同批的其它作品——它们的地址是好的，没有被牵连的理由。
pub(crate) async fn stop_member_for_missing_execution_input(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    task_id: Uuid,
) -> Result<(), sqlx::Error> {
    let work_order_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT lease.work_order_ref FROM collection_work_order_lease_task task \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE task.task_id=$1",
    )
    .bind(task_id)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(work_order_ref) = work_order_ref else {
        return Ok(());
    };
    let lease_ref: Uuid = sqlx::query_scalar(
        "SELECT lease_ref FROM collection_work_order_lease_task WHERE task_id=$1",
    )
    .bind(task_id)
    .fetch_one(&mut **transaction)
    .await?;
    stop_material_for_missing_execution_input_in_transaction(
        transaction,
        task_id,
        MISSING_EXECUTION_INPUT_REASON,
    )
    .await?;
    record_input_blocked_dispatch_failure_in_transaction(
        transaction,
        Uuid::new_v4(),
        task_id,
        installation_ref,
        lease_ref,
        work_order_ref,
    )
    .await
}

/// 一次认领的结果。
///
/// `StoppedMember` 不是失败，也不是「没有活」：它说的是「刚认下的这张租约里，第一个成员
/// 缺执行输入，已经把它停在自己的通道上」。调用方要据此再看一眼同一张租约里的下一个成员，
/// 而不是把整批放回去——后者正是那条永远轮不到好地址作品的老路。
enum ClaimedWork {
    StoppedMember,
    Decision(DispatchDecision),
}

/// Claim one compatible Work Order from the shared queue. The lane fairness
/// rows are a scheduling policy only; the Work Order remains the one durable
/// queue authority and a Lease/RuntimeTask exists only after this succeeds.
async fn claim_next_queued_work_order(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    caller_station_ref: Uuid,
) -> Result<Option<ClaimedWork>, sqlx::Error> {
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
    for (dispatch_lane, weight, concurrent_cap, _virtual_finish) in lanes {
        if concurrent_cap.is_some_and(|cap| {
            active_by_lane.get(&dispatch_lane).copied().unwrap_or(0) >= i64::from(cap)
        }) {
            continue;
        }
        // This is a bounded eligibility probe, not a module queue. A station
        // that cannot run one candidate continues through the lane and then
        // through the other lanes rather than making all later work invisible.
        let candidates: Vec<Candidate> = sqlx::query_as(CANDIDATE_SQL)
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
                return Ok(Some(ClaimedWork::Decision(
                    DispatchDecision::ControlBlocked {
                        reason_code: "capability_missing".to_owned(),
                    },
                )));
            };
            let task_spec: Value =
                sqlx::query_scalar("SELECT task_spec FROM linggan_runtime_task WHERE task_id=$1")
                    .bind(task_id)
                    .fetch_one(&mut **transaction)
                    .await?;
            let execution_source_url =
                execution_source_url_for_task(transaction, &task_spec).await?;
            if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
                // 这里只停**这一个成员**，不释放租约。释放租约等于把同批地址完好的作品一起
                // 放回队列：它们下一轮仍要排在这个缺地址的成员后面，于是每一轮都从头再来，
                // 谁都执行不到。停下来它，租约继续持有，下一个成员当场就能派。
                stop_member_for_missing_execution_input(transaction, installation_ref, task_id)
                    .await?;
                return Ok(Some(ClaimedWork::StoppedMember));
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
            return Ok(Some(ClaimedWork::Decision(DispatchDecision::Dispatch {
                task_id,
                lease_ref: lease.lease_ref,
                task_spec,
                execution_source_url,
                page_session_plan,
            })));
        }
    }
    Ok(deferred_control_block
        .map(|reason_code| {
            Some(ClaimedWork::Decision(DispatchDecision::ControlBlocked {
                reason_code,
            }))
        })
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

/// 候选工单的选取。冻结的材料作用域决定工位需要哪些能力。
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
    " LIMIT 64 FOR NO KEY UPDATE OF work_order SKIP LOCKED",
);

/// 发现链接是可过期的执行定位信息，不是作品身份。每次派发都从最新已接纳的
/// discovery record 读取，而不把 token 冻结进长寿命 TaskSpec 或 Evidence UI。
fn execution_source_url_sha256(execution_source_url: &str) -> String {
    Sha256::digest(execution_source_url.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn execution_source_url_is_rejected(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    execution_source_url: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_detail_page_session \
         WHERE execution_source_url_sha256=$1 AND stop_reason='detail_page_url_invalid')",
    )
    .bind(execution_source_url_sha256(execution_source_url))
    .fetch_one(&mut **transaction)
    .await
}

async fn execution_source_url_for_task(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_spec: &Value,
) -> Result<Option<String>, sqlx::Error> {
    if !requires_signed_execution_source(task_spec) {
        return Ok(None);
    }
    let Some(content_external_id) = task_content_external_id(task_spec) else {
        return Ok(None);
    };
    // 「哪条地址算执行入口」这一条判据住在 `execution_input_eligibility`：候选筛选、准入冻结
    // 与这里问的是同一件事。三处各写一份 URL 形状的判据，正是「候选说能跑、派发说没地址」
    // 这类缺陷的由来。
    let signed_locator = signed_locator_predicate("record.value->'payload'->>'url'");
    // Pick the newest accepted observation before validating its locator. Filtering unsigned
    // records inside the query can resurrect an older signed URL after the latest observation
    // explicitly reported an unusable link.
    let latest_evidence: Option<(Option<String>, bool)> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT record.value->'payload'->>'url', ({signed_locator}) \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') \
              WITH ORDINALITY AS record(value,ordinality) \
         WHERE content.platform='xhs' AND content.content_external_id=$1 \
           AND record.ordinality=finding.record_ordinal+1 \
           AND record.value->'sourceObject'->>'externalId'=$1 \
         ORDER BY package.accepted_at DESC,finding.created_at DESC LIMIT 1",
        )))
        .bind(content_external_id)
        .fetch_optional(&mut **transaction)
        .await?;
    if let Some((Some(url), true)) = latest_evidence {
        return if execution_source_url_is_rejected(transaction, &url).await? {
            Ok(None)
        } else {
            Ok(Some(url))
        };
    }
    Ok(None)
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
