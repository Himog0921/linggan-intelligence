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
    revalidate_frozen_capacity_in, validate_installation_credential_in,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

/// The next retry after a locally reported execution-start failure.  Chrome
/// alarms cannot run more frequently than once per minute, and a retry must
/// not immediately reopen a page that just failed its readiness probe.
pub const DISPATCH_FAILURE_RETRY_AFTER_SECONDS: u32 = 60;

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
    Requeued { retry_after_seconds: u32 },
    Replay { retry_after_seconds: u32 },
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
                "station_daily_budget_reached" => "station_daily_budget_reached",
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

pub async fn dispatch_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_work_order_lease') IS NOT NULL \
                AND to_regclass('collection_work_order_lease_task') IS NOT NULL \
                AND to_regclass('collection_work_order_lease_task_dispatch_failure') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
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

    let replay: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT task_id,installation_ref,failure_code \
         FROM collection_work_order_lease_task_dispatch_failure \
         WHERE failure_ref=$1 FOR UPDATE",
    )
    .bind(failure_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((recorded_task_id, recorded_installation_ref, recorded_code)) = replay {
        if recorded_task_id != task_id
            || recorded_installation_ref != installation_ref
            || recorded_code != failure_code.as_str()
        {
            return Err(DispatchFailureError::FailureIdentityConflict);
        }
        transaction.commit().await?;
        return Ok(DispatchFailureOutcome::Replay {
            retry_after_seconds: DISPATCH_FAILURE_RETRY_AFTER_SECONDS,
        });
    }

    // Take the task row lock before changing eligibility.  A concurrent
    // Package receipt finishes the same `in_progress` row instead, causing
    // this update to affect zero rows; it must never be put back into pending.
    let requeued: Option<Uuid> = sqlx::query_scalar(
        "UPDATE collection_work_order_lease_task task \
         SET execution_state='pending',claimed_at=NULL,claimed_by_installation_ref=NULL \
         FROM collection_work_order_lease lease \
         WHERE task.task_id=$1 \
           AND task.execution_state='in_progress' \
           AND task.claimed_by_installation_ref=$2 \
           AND lease.lease_ref=task.lease_ref \
           AND lease.released_at IS NULL \
           AND lease.expires_at>scope_001_now() \
         RETURNING task.task_id",
    )
    .bind(task_id)
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if requeued.is_none() {
        return Err(DispatchFailureError::ClaimNotHeld);
    }
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task_dispatch_failure \
             (failure_ref,task_id,installation_ref,failure_code) \
         VALUES ($1,$2,$3,$4)",
    )
    .bind(failure_ref)
    .bind(task_id)
    .bind(installation_ref)
    .bind(failure_code.as_str())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(DispatchFailureOutcome::Requeued {
        retry_after_seconds: DISPATCH_FAILURE_RETRY_AFTER_SECONDS,
    })
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
    let (Some(station_ref), Some(_quota)) = (station_ref, quota) else {
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
            return Ok(DispatchDecision::ControlBlocked { reason_code });
        }
        let execution_source_url =
            execution_source_url_for_task(&mut transaction, &task_spec).await?;
        if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
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
                 AND prior.execution_state <> 'completed') \
         ORDER BY lease.issued_at, task.sequence_no \
         LIMIT 1 FOR UPDATE OF task SKIP LOCKED",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;

    let Some((task_id, lease_ref, task_spec, _platform, _lane)) = waiting else {
        return Ok(DispatchDecision::NothingWaiting);
    };

    if let Some(reason_code) =
        revalidate_dispatch_task(&mut transaction, installation_ref, lease_ref, &task_spec).await?
    {
        return Ok(DispatchDecision::ControlBlocked { reason_code });
    }

    let execution_source_url = execution_source_url_for_task(&mut transaction, &task_spec).await?;
    if requires_signed_execution_source(&task_spec) && execution_source_url.is_none() {
        return Ok(DispatchDecision::ExecutionLocatorUnavailable {
            reason: "这篇作品当前没有带 xsec_token 的已接纳发现链接，任务保持等待。".to_owned(),
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
                work_order.monitor_rule_revision_ref,target.active_monitor_rule_revision_ref, \
                active_rule.automatic_enabled,target.lifecycle_state,request.requested_by, \
                decision.authorization_ref \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order work_order USING(work_order_ref) \
         JOIN collection_observation_target target USING(target_ref) \
         JOIN collection_admission_decision decision USING(decision_ref) \
         JOIN collection_acquisition_request request USING(request_ref) \
         LEFT JOIN collection_monitor_rule_revision active_rule \
           ON active_rule.rule_revision_ref=target.active_monitor_rule_revision_ref \
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
    let (Some(installation_ref), Some(account_ref)) = (installation_ref, account_ref) else {
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
    if requested_by == "agent" && frozen_rule_ref != active_rule_ref {
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
        false,
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
    sqlx::query_scalar(
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
    .await
}
