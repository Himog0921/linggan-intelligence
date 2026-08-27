//! COLLECTION-001 · 调度决策：一台工位来问「现在有我能做的活吗」。
//!
//! 这里**只做决策，不做执行**（规则文档：调度决策与重执行必须分开，tick 只做轻量状态推进
//! 与入队）。它不下载、不转录、不访问任何平台，只回答「可以派 / 不可以派，以及为什么」。
//!
//! 拒绝的理由必须具体。一个笼统的「暂无任务」会让人分不清是「今天额度用完了」「风险
//! 暂停中」还是「闸门根本没开」——这三件事的处置完全不同。

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
    /// 真实执行闸门没开。这是**默认状态**，不是故障。
    GateClosed,
    /// 有覆盖本平台或 lane 的风险暂停。
    RiskPaused { reason: String },
    /// 这台工位今天的额度已经用完。工位没坏，明天自然恢复。
    DailyQuotaReached { quota: i32, used: i64 },
    /// 没有等待派发的任务。
    NothingWaiting,
}

impl DispatchDecision {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Dispatch { .. } => "dispatch",
            Self::InstallationNotClaimed => "installation_not_claimed",
            Self::GateClosed => "execution_gate_closed",
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
    sqlx::query_scalar::<_, bool>("SELECT to_regclass('collection_execution_gate') IS NOT NULL")
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

    let station: Option<(Option<Uuid>, Option<i32>)> = sqlx::query_as(
        "SELECT i.station_ref, s.daily_work_quota \
         FROM plugin_installation i \
         LEFT JOIN execution_station s \
                ON s.station_ref = i.station_ref AND s.retired_at IS NULL \
         WHERE i.install_key = $1 AND i.superseded_at IS NULL",
    )
    .bind(install_key)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((station_ref, quota)) = station else {
        return Err(DispatchError::UnknownInstallation);
    };
    let (Some(station_ref), Some(quota)) = (station_ref, quota) else {
        return Ok(DispatchDecision::InstallationNotClaimed);
    };

    // 闸门先问。关着的时候，队列里有什么并不重要。
    if !execution_gate_is_open(&mut transaction).await? {
        return Ok(DispatchDecision::GateClosed);
    }
    if let Some(reason) = active_risk_pause(&mut transaction).await? {
        return Ok(DispatchDecision::RiskPaused { reason });
    }

    let used = station_daily_note_usage_in(&mut transaction, station_ref).await?;
    if used >= i64::from(quota) {
        // 触顶的是产能，不是工位。工位仍在线，明天窗口重置后自然恢复（规则文档 §6.3）。
        return Ok(DispatchDecision::DailyQuotaReached { quota, used });
    }

    let waiting: Option<(Uuid, Uuid, Value)> = sqlx::query_as(
        "SELECT t.task_id, l.lease_ref, t.task_spec \
         FROM collection_work_order_lease l \
         JOIN linggan_runtime_task t ON t.task_id = l.task_id \
         LEFT JOIN linggan_runtime_attempt a ON a.task_id = t.task_id \
         WHERE l.station_ref = $1 \
           AND l.released_at IS NULL \
           AND l.expires_at > scope_001_now() \
           AND a.attempt_id IS NULL \
         ORDER BY l.issued_at \
         LIMIT 1 FOR UPDATE OF l",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;

    let decision = match waiting {
        Some((task_id, lease_ref, task_spec)) => DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
        },
        None => DispatchDecision::NothingWaiting,
    };
    transaction.commit().await?;
    Ok(decision)
}

/// 闸门是否开着。空表即关闭——与风险暂停同一个思路：空表本身就是有效答案。
async fn execution_gate_is_open(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_execution_gate \
                        WHERE closed_at IS NULL AND expires_at > scope_001_now())",
    )
    .fetch_one(&mut **transaction)
    .await
}

async fn active_risk_pause(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT reason FROM collection_risk_pause WHERE lifted_at IS NULL LIMIT 1")
        .fetch_optional(&mut **transaction)
        .await
}
