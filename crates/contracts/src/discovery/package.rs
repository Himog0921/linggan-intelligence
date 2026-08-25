use serde::Deserialize;

use super::query::DiscoveryUnit;
use super::{AcquisitionSpec, DiscoveryCard};

/// A discovery-only package. It is structurally valid input for a later ingress boundary; this
/// contract does not claim that it was received, accepted, persisted, or turned into Evidence.
#[derive(Debug)]
pub struct DiscoveryPackage {
    pub(super) acquisition_spec: AcquisitionSpec,
    pub(super) observed_at: String,
    pub(super) coverage: DiscoveryCoverage,
    pub(super) cards: Vec<DiscoveryCard>,
}

impl DiscoveryPackage {
    pub fn acquisition_spec(&self) -> &AcquisitionSpec {
        &self.acquisition_spec
    }

    pub fn observed_at(&self) -> &str {
        &self.observed_at
    }

    pub fn coverage(&self) -> &DiscoveryCoverage {
        &self.coverage
    }

    pub fn cards(&self) -> &[DiscoveryCard] {
        &self.cards
    }
}

/// Coverage facts in the same visible-card unit as the target. It records what was visible, not
/// an inferred platform total or a completion percentage.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryCoverage {
    pub(super) unit: DiscoveryUnit,
    pub(super) visible_cards: u16,
    pub(super) discovered_cards: u16,
    pub(super) emitted_cards: u16,
    pub(super) failed_cards: u16,
    pub(super) not_attempted_cards: u16,
    pub(super) stopped_reason: DiscoveryStopReason,
}

impl DiscoveryCoverage {
    pub fn visible_cards(&self) -> u16 {
        self.visible_cards
    }

    pub fn discovered_cards(&self) -> u16 {
        self.discovered_cards
    }
    pub fn emitted_cards(&self) -> u16 {
        self.emitted_cards
    }
    pub fn failed_cards(&self) -> u16 {
        self.failed_cards
    }
    pub fn not_attempted_cards(&self) -> u16 {
        self.not_attempted_cards
    }

    pub fn stopped_reason(&self) -> DiscoveryStopReason {
        self.stopped_reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStopReason {
    QuotaReached,
    SurfaceEnded,
    RiskControl,
    ManualStop,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DiscoveryPackageWire {
    pub(super) contract_version: String,
    pub(super) acquisition_spec: AcquisitionSpec,
    pub(super) observed_at: String,
    pub(super) coverage: DiscoveryCoverage,
    pub(super) cards: Vec<DiscoveryCard>,
}
