//! LOCAL-001 / 001C-0's closed discovery and local retrieval boundary.
//!
//! This module intentionally models neither Evidence acceptance nor a plugin command. A
//! validated [`DiscoveryPackage`] only says what a discovery surface reported as visible. It is
//! not a detail capture, media acquisition, Observation, or a permission to perform a platform
//! request.

use serde::Deserialize;
use thiserror::Error;

const FIRST_DISCOVERY_CONTRACT_VERSION: &str = "xhs.discovery.visible-card.v1";
const FIRST_DISCOVERY_QUERY: &str = "ADHD";
const FIRST_DISCOVERY_MAXIMUM_QUOTA: u16 = 20;

#[derive(Debug, Error)]
pub enum DiscoveryContractError {
    #[error("discovery contract is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("discovery contract schema is invalid: {0}")]
    SchemaInvalid(String),
    #[error("unsupported discovery contract version: {0}")]
    UnsupportedContractVersion(String),
    #[error("LOCAL-001's first discovery canary must remain the fixed XHS ADHD first-20 plan")]
    FirstCanaryChanged,
    #[error("discovery coverage does not match the delivered visible cards")]
    CoverageDoesNotMatchCards,
    #[error("a discovery occurrence does not match the package observation context")]
    OccurrenceContextMismatch,
}

/// A future real-world observation instruction. It is deliberately a different type from
/// [`EvidenceQuery`]: creating or parsing it never performs the described platform request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcquisitionSpec {
    platform: DiscoveryPlatform,
    query: String,
    sort: DiscoverySort,
    target: DiscoveryTarget,
}

impl AcquisitionSpec {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn maximum_quota(&self) -> u16 {
        self.target.maximum_quota
    }

    fn is_first_canary(&self) -> bool {
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

/// A discovery-only package. It is structurally valid input for a later ingress boundary; this
/// contract does not claim that it was received, accepted, persisted, or turned into Evidence.
#[derive(Debug)]
pub struct DiscoveryPackage {
    acquisition_spec: AcquisitionSpec,
    observed_at: String,
    coverage: DiscoveryCoverage,
    cards: Vec<DiscoveryCard>,
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

/// The only target meaning in this first discovery canary. A quota is not a frozen object set,
/// so `20 - visibleCards` must never be represented as named missing ContentItems.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscoveryTarget {
    basis: TargetBasis,
    unit: DiscoveryUnit,
    maximum_quota: u16,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TargetBasis {
    MaximumQuota,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiscoveryUnit {
    VisibleSearchCard,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiscoveryPlatform {
    Xhs,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DiscoverySort {
    Comprehensive,
}

/// Coverage facts in the same visible-card unit as the target. It records what was visible, not
/// an inferred platform total or a completion percentage.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryCoverage {
    unit: DiscoveryUnit,
    visible_cards: u16,
    stopped_reason: DiscoveryStopReason,
}

impl DiscoveryCoverage {
    pub fn visible_cards(&self) -> u16 {
        self.visible_cards
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

/// One platform content identity together with a distinct discovery occurrence. The occurrence
/// records why, when, and at which result position this card was displayed; position is not a
/// ContentItem or Observation field.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryCard {
    content: VisibleContentCard,
    occurrence: DiscoveryOccurrence,
}

impl DiscoveryCard {
    pub fn content(&self) -> &VisibleContentCard {
        &self.content
    }

    pub fn occurrence(&self) -> &DiscoveryOccurrence {
        &self.occurrence
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibleContentCard {
    platform_content_id: String,
    title: Option<String>,
    creator_display_name: Option<String>,
    published_at_source_text: Option<String>,
    cover_candidate: Option<MediaCandidate>,
}

impl VisibleContentCard {
    pub fn platform_content_id(&self) -> &str {
        &self.platform_content_id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn creator_display_name(&self) -> Option<&str> {
        self.creator_display_name.as_deref()
    }

    pub fn published_at_source_text(&self) -> Option<&str> {
        self.published_at_source_text.as_deref()
    }

    pub fn cover_candidate(&self) -> Option<&MediaCandidate> {
        self.cover_candidate.as_ref()
    }
}

/// A visible external media address is provenance only. It cannot produce a page display URL;
/// a later media-acquisition contract must first obtain and verify a MediaBlob and local replica.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaCandidate {
    observed_external_uri: String,
}

impl MediaCandidate {
    pub fn observed_external_uri(&self) -> &str {
        &self.observed_external_uri
    }

    pub fn presentation_state(&self) -> CoverPresentationState {
        CoverPresentationState::MediaNotAcquired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPresentationState {
    MediaNotAcquired,
}

/// The search-specific context for a single returned card. Its complete context is repeated on
/// the occurrence deliberately: a card can be discovered again through another query or sort.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryOccurrence {
    query: String,
    sort: DiscoverySort,
    observed_at: String,
    result_position: u16,
}

impl DiscoveryOccurrence {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn observed_at(&self) -> &str {
        &self.observed_at
    }

    pub fn result_position(&self) -> u16 {
        self.result_position
    }
}

pub fn parse_discovery_package(input: &str) -> Result<DiscoveryPackage, DiscoveryContractError> {
    let wire: DiscoveryPackageWire = serde_json::from_str(input)
        .map_err(|error| DiscoveryContractError::SchemaInvalid(error.to_string()))?;

    if wire.contract_version != FIRST_DISCOVERY_CONTRACT_VERSION {
        return Err(DiscoveryContractError::UnsupportedContractVersion(
            wire.contract_version,
        ));
    }
    if !wire.acquisition_spec.is_first_canary() {
        return Err(DiscoveryContractError::FirstCanaryChanged);
    }
    if wire.coverage.visible_cards != wire.cards.len() as u16
        || wire.coverage.visible_cards > wire.acquisition_spec.maximum_quota()
        || !matches!(wire.coverage.unit, DiscoveryUnit::VisibleSearchCard)
    {
        return Err(DiscoveryContractError::CoverageDoesNotMatchCards);
    }
    if wire.cards.iter().any(|card| {
        card.occurrence.query != wire.acquisition_spec.query
            || !matches!(card.occurrence.sort, DiscoverySort::Comprehensive)
            || card.occurrence.observed_at != wire.observed_at
            || card.occurrence.result_position == 0
            || card.occurrence.result_position > wire.acquisition_spec.maximum_quota()
    }) {
        return Err(DiscoveryContractError::OccurrenceContextMismatch);
    }

    Ok(DiscoveryPackage {
        acquisition_spec: wire.acquisition_spec,
        observed_at: wire.observed_at,
        coverage: wire.coverage,
        cards: wire.cards,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DiscoveryPackageWire {
    contract_version: String,
    acquisition_spec: AcquisitionSpec,
    observed_at: String,
    coverage: DiscoveryCoverage,
    cards: Vec<DiscoveryCard>,
}
