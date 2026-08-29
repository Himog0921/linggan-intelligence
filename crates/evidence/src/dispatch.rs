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

use crate::station_read::station_daily_note_usage_in;
use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("dispatch schema is not applied")]
    SchemaUnavailable,
    #[error("no plugin installation with that install key")]
    UnknownInstallation,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 调度对一次「有活吗」的回答。
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchDecision {
    /// 可以派这个任务。
    Dispatch {
        task_id: Uuid,
        lease_ref: Uuid,
        task_spec: Value,
    },
    /// 这个安装还没归位到任何工位，因此不属于任何工位的产能。
    InstallationNotClaimed,
    /// 有覆盖本平台或 lane 的风险暂停。
    RiskPaused { reason: String },
    /// 这台工位今天的额度已经用完。工位没坏，明天自然恢复。
    DailyQuotaReached { quota: i32, used: i64 },
    /// 没有等待派发的任务。
    NothingWaiting,
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
            // 触顶要等次日自然日窗口重置，问得再勤也不会变。
            Self::DailyQuotaReached { .. } => 1800,
            Self::RiskPaused { .. } => 900,
            // 没归位的安装再怎么问也拿不到活，等人认领。
            Self::InstallationNotClaimed => 900,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Dispatch { .. } => "dispatch",
            Self::InstallationNotClaimed => "installation_not_claimed",
            Self::RiskPaused { .. } => "risk_paused",
            Self::DailyQuotaReached { .. } => "daily_quota_reached",
            Self::NothingWaiting => "nothing_waiting",
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
                AND to_regclass('collection_work_order_lease_task') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// 回答一次「有活吗」。
///
/// 检查顺序有意义：先问闸门，再问风险，再问额度，最后才找任务。倒过来会在闸门关着时
/// 仍去翻队列，然后报一个「暂无任务」——那会让人以为是没活可干，而不是根本不许干。
pub async fn decide_dispatch(
    database: &Database,
    install_key: &str,
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
    let (Some(station_ref), Some(quota)) = (station_ref, quota) else {
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
        transaction.commit().await?;
        return Ok(DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
        });
    }

    let used = station_daily_note_usage_in(&mut transaction, station_ref).await?;
    if used >= i64::from(quota) {
        // 触顶的是产能，不是工位。工位仍在线，明天窗口重置后自然恢复（规则文档 §6.3）。
        return Ok(DispatchDecision::DailyQuotaReached { quota, used });
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

    let Some((task_id, lease_ref, task_spec, platform, lane)) = waiting else {
        return Ok(DispatchDecision::NothingWaiting);
    };

    if let Some(reason) = active_risk_pause(&mut transaction, &platform, &lane).await? {
        return Ok(DispatchDecision::RiskPaused { reason });
    }

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
    })
}

/// 覆盖这个平台与 lane 的风险暂停。判据与准入、发租处保持一致——同一条规则有三份实现
/// 而判据不同，正是旧项目「页面显示已达上限但仍在派单」那类故障的来源。
async fn active_risk_pause(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    lane: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT reason FROM collection_risk_pause \
         WHERE lifted_at IS NULL \
           AND (platform IS NULL OR platform = $1) \
           AND (lane IS NULL OR lane = $2) LIMIT 1",
    )
    .bind(platform)
    .bind(lane)
    .fetch_optional(&mut **transaction)
    .await
}
