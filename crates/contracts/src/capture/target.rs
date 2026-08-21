//! Frozen known-set target semantics of `capture.package.v1`.

use serde::Deserialize;

use super::ContentDetailUnit;

/// The frozen known-set target a capture attempt was authorized to deliver.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnownSetTarget {
    #[serde(rename = "basis")]
    _basis: KnownSetBasis,
    #[serde(rename = "unit")]
    _unit: ContentDetailUnit,
    target_manifest_hash: String,
    known_target_count: u32,
}

impl KnownSetTarget {
    pub fn target_manifest_hash(&self) -> &str {
        &self.target_manifest_hash
    }

    pub fn known_target_count(&self) -> u32 {
        self.known_target_count
    }
}

/// What the producer reports for one frozen known-set member in this package.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KnownTargetResult {
    target_ordinal: u32,
    target_external_id: String,
    outcome: KnownTargetOutcome,
    record_ordinal: Option<u32>,
    reason: Option<String>,
}

impl KnownTargetResult {
    pub fn target_ordinal(&self) -> u32 {
        self.target_ordinal
    }

    pub fn target_external_id(&self) -> &str {
        &self.target_external_id
    }

    pub fn outcome(&self) -> KnownTargetOutcome {
        self.outcome
    }

    pub fn record_ordinal(&self) -> Option<u32> {
        self.record_ordinal
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// The closed set of per-target outcomes; there is no aggregate completion ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownTargetOutcome {
    Emitted,
    Failed,
    NotAttempted,
}

impl KnownTargetOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Emitted => "emitted",
            Self::Failed => "failed",
            Self::NotAttempted => "not_attempted",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KnownSetBasis {
    KnownSet,
}
