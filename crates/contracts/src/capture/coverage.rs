//! Coverage and terminal facts of `capture.package.v1`.

use serde::Deserialize;

use super::ContentDetailUnit;

/// What this attempt actually covered, in the same unit as the target.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    #[serde(rename = "unit")]
    _unit: ContentDetailUnit,
    attempted: u32,
    emitted: u32,
    failed: u32,
    known_not_attempted: Option<u32>,
    remaining_scope: RemainingScope,
}

impl Coverage {
    pub fn attempted(&self) -> u32 {
        self.attempted
    }

    pub fn emitted(&self) -> u32 {
        self.emitted
    }

    pub fn failed(&self) -> u32 {
        self.failed
    }

    pub fn known_not_attempted(&self) -> Option<u32> {
        self.known_not_attempted
    }

    pub fn remaining_scope(&self) -> RemainingScope {
        self.remaining_scope
    }
}

/// Remaining scope is either the frozen known members or explicitly unknown; a difference
/// between counts never manufactures `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingScope {
    KnownMembers,
    Unknown,
}

impl RemainingScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KnownMembers => "known_members",
            Self::Unknown => "unknown",
        }
    }
}

/// Why the producer stopped. This is not the same fact as package acceptance.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    reason: TerminalReason,
}

impl Terminal {
    pub fn reason(&self) -> TerminalReason {
        self.reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalReason {
    TargetReached,
    RiskControl,
}

impl TerminalReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TargetReached => "target_reached",
            Self::RiskControl => "risk_control",
        }
    }
}
