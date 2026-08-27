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
    /// Does an in-flight Work already cover this target and lane?
    pub in_flight_work_exists: bool,
    /// Has this target already been archived to the standard the purpose needs?
    pub need_already_satisfied: bool,
    /// Whether the server can currently establish worker, account, budget and risk headroom.
    ///
    /// This is `false` for as long as no worker model exists. That is not a placeholder: the
    /// contract forbids answering question 5 by assuming capacity, so until workers are a
    /// real object the honest answer is "cannot be established".
    pub capacity_is_establishable: bool,
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
            reason: "没有覆盖该平台、目标类型与 lane 的有效采集授权".to_owned(),
        };
    };

    // 5 资源与风险
    if !facts.capacity_is_establishable {
        return AdmissionOutcome::DecisionRequired {
            question: AdmissionQuestion::ResourcesAndRisk,
            reason: "工位、账号与预算模型尚不存在，无法如实回答是否有资源与风险余量".to_owned(),
        };
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
            in_flight_work_exists: false,
            need_already_satisfied: false,
            capacity_is_establishable: true,
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
    fn an_unanswerable_question_is_named_not_swallowed() {
        // Today's real state: no worker model exists, so question 5 cannot be answered. The
        // contract requires saying so rather than assuming capacity.
        let outcome = decide_admission(&AdmissionFacts {
            capacity_is_establishable: false,
            ..facts()
        });
        assert_eq!(
            outcome.unanswered_question(),
            Some(AdmissionQuestion::ResourcesAndRisk)
        );
        assert!(!outcome.permits_work_order());
        assert_eq!(outcome.code(), "decision_required");
    }

    #[test]
    fn a_satisfied_need_never_reaches_the_capacity_question() {
        // Reuse is asked first so an already-met need cannot consume capacity on its way to
        // being rejected for lack of it.
        let outcome = decide_admission(&AdmissionFacts {
            need_already_satisfied: true,
            capacity_is_establishable: false,
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
    fn the_six_questions_keep_the_contracts_numbering() {
        assert_eq!(AdmissionQuestion::Reuse.number(), 1);
        assert_eq!(AdmissionQuestion::ResourcesAndRisk.number(), 5);
        assert_eq!(AdmissionQuestion::Explainability.number(), 6);
    }
}
