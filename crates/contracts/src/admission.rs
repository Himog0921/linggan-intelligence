//! COLLECTION-001 · Admission, per `capture-control-contract` §2.1.
//!
//! Admission is not a priority scorer and not an AI decision. Before a Work Order may be
//! written, the server must be able to *explain* six things. If any one of them cannot be
//! answered honestly, the contract requires refusing, waiting, merging, downgrading to a
//! suggestion, or marking `DECISION_REQUIRED` — never "looks relevant, go ahead".
//!
//! This module encodes those six questions as a type, so that "we skipped one" is not
//! expressible rather than merely discouraged.

use std::fmt;

/// Stable control reason shared by Admission, Lease, dispatch and read models.
/// There is deliberately no free-form/platform-provided variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapacityReasonCode {
    RiskPaused,
    StationUnavailable,
    StationNotAccepting,
    InstallationCredentialMissing,
    PluginVersionUnsupported,
    InstallationStale,
    CapabilityMissing,
    AccountUnbound,
    AccountBindingChanged,
    AccountBindingExpired,
    AccountEligibilityStale,
    AccountCooling,
    AccountNeedsLogin,
    AccountRestricted,
    AccountUnknown,
    AccountBusy,
    StationDailyBudgetReached,
    CapacityUnknown,
}

impl CapacityReasonCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RiskPaused => "risk_paused",
            Self::StationUnavailable => "station_unavailable",
            Self::StationNotAccepting => "station_not_accepting",
            Self::InstallationCredentialMissing => "installation_credential_missing",
            Self::PluginVersionUnsupported => "plugin_version_unsupported",
            Self::InstallationStale => "installation_stale",
            Self::CapabilityMissing => "capability_missing",
            Self::AccountUnbound => "account_unbound",
            Self::AccountBindingChanged => "account_binding_changed",
            Self::AccountBindingExpired => "account_binding_expired",
            Self::AccountEligibilityStale => "account_eligibility_stale",
            Self::AccountCooling => "account_cooling",
            Self::AccountNeedsLogin => "account_needs_login",
            Self::AccountRestricted => "account_restricted",
            Self::AccountUnknown => "account_unknown",
            Self::AccountBusy => "account_busy",
            Self::StationDailyBudgetReached => "station_daily_budget_reached",
            Self::CapacityUnknown => "capacity_unknown",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuthorizationBoundaryFailure {
    Missing,
    ScopeMismatch,
    PurposeMismatch,
    TargetLimitReached,
    WorkUnitLimitReached,
    ExpiredOrRevoked,
}

impl AuthorizationBoundaryFailure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "authorization_missing",
            Self::ScopeMismatch => "authorization_scope_mismatch",
            Self::PurposeMismatch => "authorization_purpose_mismatch",
            Self::TargetLimitReached => "authorization_target_limit_reached",
            Self::WorkUnitLimitReached => "authorization_work_unit_limit_reached",
            Self::ExpiredOrRevoked => "authorization_expired_or_revoked",
        }
    }

    pub const fn zh_reason(self) -> &'static str {
        match self {
            Self::Missing => "没有覆盖该平台、目标类型与 lane 的采集授权",
            Self::ScopeMismatch => "现有授权不覆盖这类任务或派发通道",
            Self::PurposeMismatch => "请求用途与现有授权用途不一致",
            Self::TargetLimitReached => "授权允许的目标数量已经用尽",
            Self::WorkUnitLimitReached => "授权允许的工作单元不足以覆盖这次任务",
            Self::ExpiredOrRevoked => "匹配授权已经过期或被撤销",
        }
    }
}

/// What question 5 established about resources and risk.
///
/// The contract asks about worker, account, budget and risk headroom together. Keeping them
/// as separate variants means a deferral names the one thing that is missing rather than
/// reporting a vague "no capacity".
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Capacity {
    /// Admission may write a bounded Work Order into the shared queue.  Station,
    /// account, quota, risk and capability are re-evaluated by the installation
    /// that later claims the order; this is not a claim that a particular
    /// station is available now.
    Queueable,
    /// A compatible station is in place, within budget, with no risk pause in effect.
    Available { station_ref: String },
    /// No station has a live plugin installation claimed to it.
    NoStaffedStation,
    /// A staffed station exists but none of them can do what this lane needs.
    MissingCapabilities { missing: Vec<String> },
    /// Every compatible station has already committed its day to other work orders.
    DailyQuotaCommitted { quota: i32, committed: i32 },
    /// A risk pause covering this platform or lane is still in effect.
    RiskPaused { reason: String },
    /// The collection-control evaluator closed the gate with one stable machine reason.
    ///
    /// Package 2 keeps legacy variants readable, while every new account, credential,
    /// freshness and station-acceptance gate flows through this bounded reason code.
    Unavailable {
        reason_code: CapacityReasonCode,
        reason: String,
    },
}

impl Capacity {
    /// `None` when question 5 can be answered "yes".
    pub fn blocking_reason(&self) -> Option<String> {
        match self {
            Self::Queueable => None,
            Self::Available { .. } => None,
            Self::NoStaffedStation => {
                Some("没有任何工位有在岗插件安装，因此没有可执行这次采集的资源".to_owned())
            }
            Self::MissingCapabilities { missing } => Some(format!(
                "在岗工位都不具备本 lane 需要的能力：缺 {}",
                missing.join("、")
            )),
            Self::DailyQuotaCommitted { quota, committed } => Some(format!(
                "当天额度已被既有工单占满：每日 {quota} 篇，已下发 {committed} 篇"
            )),
            Self::RiskPaused { reason } => Some(format!("风险暂停仍在生效：{reason}")),
            Self::Unavailable { reason, .. } => Some(reason.clone()),
        }
    }

    pub fn station_ref(&self) -> Option<&str> {
        match self {
            Self::Available { station_ref } => Some(station_ref),
            _ => None,
        }
    }

    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Queueable => "queueable",
            Self::Available { .. } => "available",
            Self::NoStaffedStation => "station_unavailable",
            Self::MissingCapabilities { .. } => "capability_missing",
            Self::DailyQuotaCommitted { .. } => "station_daily_budget_reached",
            Self::RiskPaused { .. } => "risk_paused",
            Self::Unavailable { reason_code, .. } => reason_code.as_str(),
        }
    }
}

/// The six questions, in the contract's order. The numbers are load-bearing: a decision that
/// stops early records *which* question it stopped on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdmissionQuestion {
    /// 1 复用 — is an existing evidence or in-flight Work already satisfying the need?
    Reuse = 1,
    /// 2 差额 — if not, what exactly is missing?
    Shortfall = 2,
    /// 3 时间 — would delay measurably lose the information this purpose needs?
    Timing = 3,
    /// 4 边界 — still inside the granted authorization, purpose, lane and object range?
    Bounds = 4,
    /// 5 资源与风险 — is there a compatible worker, account, budget and risk headroom?
    ResourcesAndRisk = 5,
    /// 6 可解释性 — can the target semantics, coverage unit and stop conditions be written
    /// into the Work Order?
    Explainability = 6,
}

impl AdmissionQuestion {
    pub fn number(self) -> i16 {
        self as i16
    }

    pub fn describe(self) -> &'static str {
        match self {
            Self::Reuse => "复用：已有合格材料或在途工作是否已满足需要",
            Self::Shortfall => "差额：若不满足，缺的究竟是哪一层",
            Self::Timing => "时间：延迟是否会明显损失本次目的所需的信息",
            Self::Bounds => "边界：是否仍在获批授权、用途、lane 与对象范围内",
            Self::ResourcesAndRisk => "资源与风险：是否有兼容工位、账号、预算与风险余量",
            Self::Explainability => "可解释性：目标语义、Coverage 单位与停止条件能否写入工单",
        }
    }
}

/// What Admission concluded.
///
/// Wider than accept/reject on purpose. An admission stage with two outcomes cannot express
/// "already satisfied" or "we cannot answer this honestly yet", and would be forced to push
/// both of those into a false accept.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AdmissionOutcome {
    /// Proceed. A Work Order may be written against the named authorization.
    Admitted { authorization_ref: String },
    /// Existing evidence already satisfies the need — no new platform access.
    Reuse { reason: String },
    /// An in-flight Work already covers this; wait for it rather than opening a second one.
    Merge { reason: String },
    /// Not now. May be decided again later.
    Defer { reason: String },
    /// Will not do this.
    Refuse { reason: String },
    /// One of the six questions cannot be answered honestly. Names which one.
    DecisionRequired {
        question: AdmissionQuestion,
        reason: String,
    },
}

impl AdmissionOutcome {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Admitted { .. } => "admitted",
            Self::Reuse { .. } => "reuse",
            Self::Merge { .. } => "merge",
            Self::Defer { .. } => "defer",
            Self::Refuse { .. } => "refuse",
            Self::DecisionRequired { .. } => "decision_required",
        }
    }

    /// Only an admitted outcome may lead to a Work Order. Every other outcome, including the
    /// ones that are not failures, must leave no instruction behind.
    pub fn permits_work_order(&self) -> bool {
        matches!(self, Self::Admitted { .. })
    }

    pub fn unanswered_question(&self) -> Option<AdmissionQuestion> {
        match self {
            Self::DecisionRequired { question, .. } => Some(*question),
            _ => None,
        }
    }
}

/// The facts Admission is allowed to reason over. Every field is something the server can
/// actually establish — nothing here is inferred from "it looks related".
#[derive(Clone, Debug)]
pub struct AdmissionFacts {
    /// Is there a live, unexpired, unrevoked grant covering this platform/kind/lane?
    pub authorization_ref: Option<String>,
    /// Closed explanation when no authorization reference is usable.
    pub authorization_failure: Option<AuthorizationBoundaryFailure>,
    /// Does an in-flight Work already cover this target and lane?
    pub in_flight_work_exists: bool,
    /// Has this target already been archived to the standard the purpose needs?
    pub need_already_satisfied: bool,
    /// What question 5 found. A queued order deliberately defers this check to
    /// the atomic station claim; otherwise a scheduler would have to bind work
    /// to whichever station happened to be healthy when it woke up.
    pub capacity: Capacity,
    /// Whether stop conditions and coverage semantics can be written into the order.
    pub stop_conditions_expressible: bool,
}

/// Run the six questions in order, stopping at the first that cannot be answered.
///
/// Order matters: reuse is asked first so that a need already met never consumes capacity,
/// and bounds are asked before resources so an out-of-scope request is refused rather than
/// merely queued behind capacity.
pub fn decide_admission(facts: &AdmissionFacts) -> AdmissionOutcome {
    // 1 复用
    if facts.need_already_satisfied {
        return AdmissionOutcome::Reuse {
            reason: "已有材料满足本次目的，不需要新的平台访问".to_owned(),
        };
    }

    // 2 差额 — an in-flight Work covering the same ground is a merge, not a second order.
    if facts.in_flight_work_exists {
        return AdmissionOutcome::Merge {
            reason: "已有在途工作覆盖同一目标与 lane，等待它而不是再开一个".to_owned(),
        };
    }

    // 3 时间 is currently subsumed by the caller's decision to request at all; there is no
    // freshness model yet to reason with, and inventing one would be the "把发布 7 天硬编码
    // 成宇宙事实" the contract warns against.

    // 4 边界
    let Some(authorization_ref) = facts.authorization_ref.clone() else {
        return AdmissionOutcome::Refuse {
            reason: facts
                .authorization_failure
                .unwrap_or(AuthorizationBoundaryFailure::Missing)
                .zh_reason()
                .to_owned(),
        };
    };

    // 5 资源与风险
    if let Some(reason) = facts.capacity.blocking_reason() {
        // A missing resource is a *wait*, not a refusal: the request is legitimate and
        // becomes runnable once the resource exists. Refusing would tell the caller to stop
        // asking, which is the wrong instruction.
        return AdmissionOutcome::Defer { reason };
    }

    // 6 可解释性
    if !facts.stop_conditions_expressible {
        return AdmissionOutcome::DecisionRequired {
            question: AdmissionQuestion::Explainability,
            reason: "停止条件与 Coverage 单位无法写入工单".to_owned(),
        };
    }

    AdmissionOutcome::Admitted { authorization_ref }
}

impl fmt::Display for AdmissionOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> AdmissionFacts {
        AdmissionFacts {
            authorization_ref: Some("auth-1".to_owned()),
            authorization_failure: None,
            in_flight_work_exists: false,
            need_already_satisfied: false,
            capacity: Capacity::Available {
                station_ref: "station-1".to_owned(),
            },
            stop_conditions_expressible: true,
        }
    }

    #[test]
    fn without_a_grant_it_refuses_rather_than_queues() {
        // Being out of scope is not "wait your turn" — it is a refusal with a reason.
        let outcome = decide_admission(&AdmissionFacts {
            authorization_ref: None,
            ..facts()
        });
        assert!(matches!(outcome, AdmissionOutcome::Refuse { .. }));
        assert!(!outcome.permits_work_order());
    }

    #[test]
    fn a_missing_resource_defers_and_says_which_one() {
        // Question 5 must name the missing resource: a bare "no capacity" sends the reader
        // hunting through four subsystems.
        let outcome = decide_admission(&AdmissionFacts {
            capacity: Capacity::NoStaffedStation,
            ..facts()
        });
        // A missing resource defers rather than refuses: the request stays legitimate and
        // becomes runnable once a station is staffed.
        assert_eq!(outcome.code(), "defer");
        assert!(!outcome.permits_work_order());
    }

    #[test]
    fn queueable_admission_does_not_fabricate_a_station() {
        let outcome = decide_admission(&AdmissionFacts {
            capacity: Capacity::Queueable,
            ..facts()
        });
        assert!(outcome.permits_work_order());
        assert_eq!(Capacity::Queueable.station_ref(), None);
    }

    #[test]
    fn a_satisfied_need_never_reaches_the_capacity_question() {
        // Reuse is asked first so an already-met need cannot consume capacity on its way to
        // being rejected for lack of it.
        let outcome = decide_admission(&AdmissionFacts {
            need_already_satisfied: true,
            capacity: Capacity::NoStaffedStation,
            authorization_ref: None,
            ..facts()
        });
        assert!(matches!(outcome, AdmissionOutcome::Reuse { .. }));
    }

    #[test]
    fn in_flight_work_is_merged_not_duplicated() {
        let outcome = decide_admission(&AdmissionFacts {
            in_flight_work_exists: true,
            ..facts()
        });
        assert!(matches!(outcome, AdmissionOutcome::Merge { .. }));
        assert!(!outcome.permits_work_order());
    }

    #[test]
    fn only_admitted_permits_a_work_order() {
        assert!(decide_admission(&facts()).permits_work_order());
        for outcome in [
            AdmissionOutcome::Reuse {
                reason: String::new(),
            },
            AdmissionOutcome::Merge {
                reason: String::new(),
            },
            AdmissionOutcome::Defer {
                reason: String::new(),
            },
            AdmissionOutcome::Refuse {
                reason: String::new(),
            },
            AdmissionOutcome::DecisionRequired {
                question: AdmissionQuestion::Timing,
                reason: String::new(),
            },
        ] {
            assert!(
                !outcome.permits_work_order(),
                "{} must not permit a work order",
                outcome.code()
            );
        }
    }

    #[test]
    fn each_capacity_gap_names_itself() {
        // Every blocked variant must say which resource is missing, and the available one
        // must block nothing.
        assert!(
            Capacity::Available {
                station_ref: "s".to_owned()
            }
            .blocking_reason()
            .is_none()
        );
        for capacity in [
            Capacity::NoStaffedStation,
            Capacity::MissingCapabilities {
                missing: vec!["content_detail".to_owned()],
            },
            Capacity::DailyQuotaCommitted {
                quota: 200,
                committed: 200,
            },
            Capacity::RiskPaused {
                reason: "演练".to_owned(),
            },
        ] {
            let reason = capacity
                .blocking_reason()
                .expect("blocked capacity states a reason");
            assert!(!reason.trim().is_empty());
        }
    }

    #[test]
    fn the_six_questions_keep_the_contracts_numbering() {
        assert_eq!(AdmissionQuestion::Reuse.number(), 1);
        assert_eq!(AdmissionQuestion::ResourcesAndRisk.number(), 5);
        assert_eq!(AdmissionQuestion::Explainability.number(), 6);
    }
}
