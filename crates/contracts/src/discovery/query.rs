use serde::Deserialize;

const FIRST_DISCOVERY_QUERY: &str = "ADHD";
const FIRST_DISCOVERY_MAXIMUM_QUOTA: u16 = 20;

/// A future real-world observation instruction. It is deliberately a different type from
/// [`EvidenceQuery`]: creating or parsing it never performs the described platform request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcquisitionSpec {
    pub(super) platform: DiscoveryPlatform,
    pub(super) query: String,
    pub(super) sort: DiscoverySort,
    pub(super) target: DiscoveryTarget,
}

impl AcquisitionSpec {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn maximum_quota(&self) -> u16 {
        self.target.maximum_quota
    }

    pub(super) fn is_first_canary(&self) -> bool {
        matches!(self.platform, DiscoveryPlatform::Xhs)
            && self.query == FIRST_DISCOVERY_QUERY
            && matches!(self.sort, DiscoverySort::Comprehensive)
            && self.target.maximum_quota == FIRST_DISCOVERY_MAXIMUM_QUOTA
            && matches!(self.target.basis, TargetBasis::MaximumQuota)
            && matches!(self.target.unit, DiscoveryUnit::VisibleSearchCard)
    }
}

/// A read-only query over materials that Linggan has already accepted. It has no platform,
/// plugin, capture command, or acquisition target fields.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceQuery {
    text: Option<String>,
    scope: EvidenceQueryScope,
    window: PublishedWindow,
    sort: EvidenceQuerySort,
}

impl EvidenceQuery {
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn scope(&self) -> EvidenceQueryScope {
        self.scope
    }

    pub fn window(&self) -> PublishedWindow {
        self.window
    }

    pub fn sort(&self) -> EvidenceQuerySort {
        self.sort
    }
}

/// Evidence Library's V1 search scope. The values only describe local matching fields; they do
/// not request missing OCR, ASR, comments, or a new platform capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceQueryScope {
    AllAcceptedMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceQuerySort {
    Relevance,
    LatestDiscovery,
}

/// `WINDOW` is only a ContentItem published-time window. A missing source publication time is
/// not silently included in this window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishedWindow {
    #[serde(rename = "last_7_days")]
    Last7Days,
    #[serde(rename = "last_30_days")]
    Last30Days,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DiscoveryTarget {
    pub(super) basis: TargetBasis,
    pub(super) unit: DiscoveryUnit,
    pub(super) maximum_quota: u16,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TargetBasis {
    MaximumQuota,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiscoveryUnit {
    VisibleSearchCard,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiscoveryPlatform {
    Xhs,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiscoverySort {
    Comprehensive,
}
