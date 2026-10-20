//! Admission and round decisions contain no I/O, dispatch, or implicit switch recovery.
use super::{RuleError, TaskKey};
use linggan_contracts::ai_limits::AI_LIMITS;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "mode",
    content = "tokens",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BudgetPolicy {
    Unconfigured,
    Unlimited,
    Capped(#[serde(with = "super::number::u64_wire")] u64),
}
impl BudgetPolicy {
    pub fn admits(&self, already_held: u64, reservation: u64) -> bool {
        let Some(total) = already_held
            .checked_add(reservation)
            .filter(|v| *v <= i64::MAX as u64)
        else {
            return false;
        };
        match self {
            Self::Unconfigured => false,
            Self::Unlimited => true,
            Self::Capped(limit) => total <= *limit,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Admission {
    pub global_on: bool,
    pub executor_on: bool,
    pub task_on: bool,
    pub task_connected: bool,
    pub config_ready: bool,
    pub capability_ready: bool,
    pub egress_allowed: bool,
    pub material_qualified: bool,
    pub budget: BudgetPolicy,
    pub already_held: u64,
    pub reservation: u64,
}
impl Admission {
    pub fn denied_reasons(&self) -> Vec<&'static str> {
        let mut reasons = vec![];
        for (allowed, code) in [
            (self.global_on, "global_paused"),
            (self.executor_on, "executor_paused"),
            (self.task_on, "task_paused"),
            (self.task_connected, "task_not_connected"),
            (self.config_ready, "task_not_configured"),
            (self.capability_ready, "model_capability_unknown"),
            (self.egress_allowed, "egress_not_approved"),
            (self.material_qualified, "source_unavailable"),
        ] {
            if !allowed {
                reasons.push(code);
            }
        }
        if !self.budget.admits(self.already_held, self.reservation) {
            reasons.push(if self.budget == BudgetPolicy::Unconfigured {
                "budget_not_configured"
            } else {
                "daily_budget_exhausted"
            });
        }
        reasons
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrakeReason {
    FailureRatio,
    NoUsableOutput,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletedRound {
    pub attempted: u32,
    pub failed: u32,
    pub usable: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub struct RoundDecision {
    pub brake: Option<BrakeReason>,
    pub consecutive_without_usable: u32,
}
impl CompletedRound {
    pub fn evaluate(
        self,
        task: TaskKey,
        previous_without_usable: u32,
    ) -> Result<RoundDecision, RuleError> {
        if self.attempted > AI_LIMITS.round_size
            || self.failed > self.attempted
            || self.usable > self.attempted - self.failed
        {
            return Err(RuleError::Round);
        }
        if !task.uses_automatic_brake() || self.attempted == 0 {
            return Ok(RoundDecision {
                brake: None,
                consecutive_without_usable: previous_without_usable,
            });
        }
        let streak = if self.usable > 0 {
            0
        } else {
            previous_without_usable.saturating_add(1)
        };
        let brake = if self.failed * 100 > self.attempted * 30 {
            Some(BrakeReason::FailureRatio)
        } else if streak >= 2 {
            Some(BrakeReason::NoUsableOutput)
        } else {
            None
        };
        Ok(RoundDecision {
            brake,
            consecutive_without_usable: streak,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_switch_and_qualification_is_required_even_for_unlimited_budget() {
        let ready = Admission {
            global_on: true,
            executor_on: true,
            task_on: true,
            task_connected: true,
            config_ready: true,
            capability_ready: true,
            egress_allowed: true,
            material_qualified: true,
            budget: BudgetPolicy::Unlimited,
            already_held: 0,
            reservation: 100,
        };
        assert!(ready.denied_reasons().is_empty());
        for field in [
            "globalOn",
            "executorOn",
            "taskOn",
            "taskConnected",
            "configReady",
            "capabilityReady",
            "egressAllowed",
            "materialQualified",
        ] {
            let mut value = serde_json::to_value(&ready).unwrap();
            value[field] = false.into();
            assert_eq!(
                serde_json::from_value::<Admission>(value)
                    .unwrap()
                    .denied_reasons()
                    .len(),
                1,
                "{field}"
            );
        }
        assert!(!BudgetPolicy::Unconfigured.admits(0, 0));
        assert!(BudgetPolicy::Capped(100).admits(80, 20));
        assert!(!BudgetPolicy::Capped(100).admits(80, 21));
        assert!(!BudgetPolicy::Unlimited.admits(u64::MAX, 1));
    }
    #[test]
    fn strictly_over_thirty_percent_and_empty_rounds_cannot_mask_the_brake() {
        let task = TaskKey::NoteInterpretation;
        assert_eq!(
            CompletedRound {
                attempted: 50,
                failed: 15,
                usable: 35
            }
            .evaluate(task, 0)
            .unwrap()
            .brake,
            None
        );
        assert_eq!(
            CompletedRound {
                attempted: 50,
                failed: 16,
                usable: 34
            }
            .evaluate(task, 0)
            .unwrap()
            .brake,
            Some(BrakeReason::FailureRatio)
        );
        let first = CompletedRound {
            attempted: 50,
            failed: 0,
            usable: 0,
        }
        .evaluate(task, 0)
        .unwrap();
        assert_eq!(first.consecutive_without_usable, 1);
        let empty = CompletedRound {
            attempted: 0,
            failed: 0,
            usable: 0,
        }
        .evaluate(task, 1)
        .unwrap();
        assert_eq!(empty.consecutive_without_usable, 1);
        assert_eq!(empty.brake, None);
        assert_eq!(
            CompletedRound {
                attempted: 1,
                failed: 0,
                usable: 0
            }
            .evaluate(task, empty.consecutive_without_usable)
            .unwrap()
            .brake,
            Some(BrakeReason::NoUsableOutput)
        );
        assert_eq!(
            CompletedRound {
                attempted: 50,
                failed: 50,
                usable: 0
            }
            .evaluate(TaskKey::WritingAssistant, 0)
            .unwrap()
            .brake,
            None
        );
        assert!(
            CompletedRound {
                attempted: 1,
                failed: 1,
                usable: 1
            }
            .evaluate(task, 0)
            .is_err()
        );
    }
}
