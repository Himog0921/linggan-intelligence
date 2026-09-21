//! 一步的结果：三值结局 + 「什么样的错误算没轮到」。
//!
//! 与账本（`scheduler_tick`）分开，是因为这一层是**值**：它不碰数据库、不认识 run。写行是
//! 账本那一层的事。分开也是为了让「三个结局」和 `0098` 的 CHECK 挨着住——两边是同一套词，
//! 改一处必须改另一处。
//!
//! 失败一律压成受限码（`error_class` / `skipped_reason`，字符集 `[a-z0-9_]`、最长 32 字符）：
//! 原始报文可能带着连接串与页面文本，而这几列是给人看、给页面读的持久事实。

use std::time::Duration;

use crate::runtime_event::bounded_code;

/// 这一步的结局。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    /// 跑完了。三个计数各自可选：这一步不统计的那一项留空。
    Ok {
        considered: Option<i64>,
        produced: Option<i64>,
        skipped: Option<i64>,
    },
    /// 跑了并失败。`error_class` 是受限码（如 `sqlstate_42p01`），不是报文。
    Failed { error_class: String },
    /// 没轮到。`reason` 是受限码（如 `schema_unavailable`）。
    Skipped { reason: &'static str },
}

impl StepOutcome {
    /// 只说产出的步骤（媒体投影这类不数「考虑过多少」的）。
    pub fn produced(count: i64) -> Self {
        Self::Ok {
            considered: None,
            produced: Some(count),
            skipped: None,
        }
    }

    /// 计数齐全的步骤。
    pub fn counted(considered: i64, produced: i64, skipped: i64) -> Self {
        Self::Ok {
            considered: Some(considered),
            produced: Some(produced),
            skipped: Some(skipped),
        }
    }

    pub fn failed(error_class: impl Into<String>) -> Self {
        Self::Failed {
            error_class: error_class.into(),
        }
    }

    pub fn skipped(reason: &'static str) -> Self {
        Self::Skipped { reason }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Ok { .. } => "ok",
            Self::Failed { .. } => "failed",
            Self::Skipped { .. } => "skipped",
        }
    }

    pub fn error_class(&self) -> Option<&str> {
        match self {
            Self::Failed { error_class } => Some(error_class),
            _ => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Skipped { reason } => Some(reason),
            _ => None,
        }
    }

    /// 这一步报出来的产出数；没报就是 `None`（不是 0）。
    pub fn produced_count(&self) -> Option<i64> {
        match self {
            Self::Ok { produced, .. } => *produced,
            _ => None,
        }
    }
}

/// 收尾后的一步：写好或写失败都留在调用方手里，收轮时据此算整轮结局。
#[derive(Debug, Clone)]
pub struct StepReport {
    pub step_key: &'static str,
    pub outcome: StepOutcome,
    pub duration: Duration,
}

/// 一步的失败怎么变成受限码。
///
/// 表只有这一张，按错误的真实类型逐变体写满——**不留兜底臂**：留了兜底，将来新增一个错误
/// 变体时编译器不会拦，它会悄悄落进通用桶；而一个压平的原因码比没有原因码更坏，它看起来
/// 像个答案。
pub trait StepFailure {
    /// 这一步**没轮到**（它自己那张表不齐，不是故障）→ 受限原因码；真失败 → `None`。
    fn skip_reason(&self) -> Option<&'static str>;
    /// 受限错误分类：一律 `[a-z0-9_]`、最长 32 字符。
    fn error_class(&self) -> String;
}

pub(crate) fn outcome_of_failure<E: StepFailure>(error: &E) -> StepOutcome {
    match error.skip_reason() {
        Some(reason) => StepOutcome::skipped(reason),
        None => StepOutcome::failed(error.error_class()),
    }
}

/// 数据库错误的分类只留 **SQLSTATE**（五个字符的类别码），不留报文——报文可能带着连接串。
fn sqlstate_class(error: &sqlx::Error) -> String {
    error
        .as_database_error()
        .and_then(|database_error| database_error.code())
        .map(|code| format!("sqlstate_{}", code.to_ascii_lowercase()))
        .unwrap_or_else(|| "database_error".to_owned())
}

impl StepFailure for sqlx::Error {
    fn skip_reason(&self) -> Option<&'static str> {
        None
    }

    fn error_class(&self) -> String {
        sqlstate_class(self)
    }
}

impl StepFailure for crate::patrol_scheduler::PatrolStepError {
    fn skip_reason(&self) -> Option<&'static str> {
        match self {
            Self::SchemaUnavailable => Some("schema_unavailable"),
            Self::Database(_) => None,
        }
    }

    fn error_class(&self) -> String {
        match self {
            Self::SchemaUnavailable => "schema_unavailable".to_owned(),
            Self::Database(error) => sqlstate_class(error),
        }
    }
}

impl StepFailure for crate::media_acquisition::MediaAcquisitionError {
    fn skip_reason(&self) -> Option<&'static str> {
        match self {
            Self::SchemaUnavailable => Some("schema_unavailable"),
            _ => None,
        }
    }

    fn error_class(&self) -> String {
        match self {
            Self::SchemaUnavailable => "schema_unavailable".to_owned(),
            Self::InvalidCredential => "invalid_credential".to_owned(),
            Self::Database(error) => sqlstate_class(error),
        }
    }
}

impl StepFailure for crate::acquisition_chain::AcquisitionChainError {
    fn skip_reason(&self) -> Option<&'static str> {
        match self {
            Self::SchemaUnavailable => Some("schema_unavailable"),
            _ => None,
        }
    }

    fn error_class(&self) -> String {
        match self {
            Self::Database(error) => sqlstate_class(error),
            // 准入链的原因码词表已经存在（调度写进 decision 的那一套），这里共用一份，
            // 不为同一个错误再编第二套词。
            other => crate::patrol_scheduler::acquisition_failure_code(other).to_owned(),
        }
    }
}

impl StepFailure for crate::acquisition_chain::RequestLeaseError {
    fn skip_reason(&self) -> Option<&'static str> {
        match self {
            Self::Acquisition(error) => error.skip_reason(),
            Self::Lease(error) => error.skip_reason(),
        }
    }

    fn error_class(&self) -> String {
        match self {
            Self::Acquisition(error) => error.error_class(),
            Self::Lease(error) => error.error_class(),
        }
    }
}

impl StepFailure for crate::work_order_lease::LeaseError {
    fn skip_reason(&self) -> Option<&'static str> {
        match self {
            Self::SchemaUnavailable => Some("schema_unavailable"),
            _ => None,
        }
    }

    fn error_class(&self) -> String {
        match self {
            Self::SchemaUnavailable => "schema_unavailable".to_owned(),
            Self::UnknownWorkOrder => "unknown_work_order".to_owned(),
            Self::AlreadyLeased => "already_leased".to_owned(),
            Self::InvalidLeaseDuration => "invalid_lease_duration".to_owned(),
            Self::NoStation => "no_station".to_owned(),
            Self::StationUnavailable => "station_unavailable".to_owned(),
            Self::FrozenControlMissing => "frozen_control_missing".to_owned(),
            Self::WorkOrderAlreadySatisfied => "work_order_already_satisfied".to_owned(),
            Self::OnlyStoppedMembersRemain => "only_stopped_members_remain".to_owned(),
            // 控制面已经给出它自己的受限原因码，它比「被控制面挡住」这句话更精确。
            Self::ControlBlocked { reason_code } => bounded_code(reason_code),
            Self::AuthorizationLapsed => "authorization_lapsed".to_owned(),
            Self::RiskPaused { .. } => "risk_paused".to_owned(),
            Self::TaskSpecInvalid(_) => "task_spec_invalid".to_owned(),
            Self::Database(error) => sqlstate_class(error),
        }
    }
}
