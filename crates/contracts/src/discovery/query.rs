use serde::Deserialize;
use uuid::Uuid;

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
    #[serde(rename = "window")]
    time_view: EvidenceTimeView,
    sort: EvidenceQuerySort,
    #[serde(default)]
    lane: Option<EvidenceMaterialLane>,
    #[serde(default)]
    lane_state: Option<EvidenceLaneState>,
    #[serde(default)]
    media_kind: Option<EvidenceMediaKind>,
    #[serde(default)]
    restriction: Option<EvidenceRestriction>,
    #[serde(default)]
    cursor: Option<String>,
    /// Required by the Corpus adapter; optional in the shared query contract for older internal
    /// callers that operate over an already-scoped material set.
    #[serde(default)]
    domain_ref: Option<Uuid>,
}

impl EvidenceQuery {
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn scope(&self) -> EvidenceQueryScope {
        self.scope
    }

    pub fn time_view(&self) -> EvidenceTimeView {
        self.time_view
    }

    /// A publication-time filter exists only for an explicit published-time view.  The default
    /// accepted-discovery view is intentionally not allowed to smuggle first-seen or observed
    /// time into this return value.
    pub fn published_window(&self) -> Option<PublishedWindow> {
        match self.time_view {
            EvidenceTimeView::LatestAcceptedDiscovery => None,
            EvidenceTimeView::PublishedLast7Days => Some(PublishedWindow::Last7Days),
            EvidenceTimeView::PublishedLast30Days => Some(PublishedWindow::Last30Days),
        }
    }

    pub fn sort(&self) -> EvidenceQuerySort {
        self.sort
    }

    pub fn lane(&self) -> Option<EvidenceMaterialLane> {
        self.lane
    }

    pub fn lane_state(&self) -> Option<EvidenceLaneState> {
        self.lane_state
    }

    pub fn media_kind(&self) -> Option<EvidenceMediaKind> {
        self.media_kind
    }

    pub fn restriction(&self) -> Option<EvidenceRestriction> {
        self.restriction
    }

    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    pub fn domain_ref(&self) -> Option<Uuid> {
        self.domain_ref
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMaterialLane {
    Discovery,
    Detail,
    Comments,
    Replies,
    Author,
    MediaSlots,
    MediaBytes,
    Ocr,
    Asr,
}

impl EvidenceMaterialLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discovery => "discovery",
            Self::Detail => "detail",
            Self::Comments => "comments",
            Self::Replies => "replies",
            Self::Author => "author",
            Self::MediaSlots => "media_slots",
            Self::MediaBytes => "media_bytes",
            Self::Ocr => "ocr",
            Self::Asr => "asr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceLaneState {
    NotRequested,
    Queued,
    NotObserved,
    Observed,
    Partial,
    Acquired,
    Processing,
    NotEnabled,
    Searchable,
    Failed,
    RiskControl,
    BytesCleaned,
    WithdrawnOrRestricted,
    Unknown,
}

impl EvidenceLaneState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "NOT_REQUESTED",
            Self::Queued => "QUEUED",
            Self::NotObserved => "NOT_OBSERVED",
            Self::Observed => "OBSERVED",
            Self::Partial => "PARTIAL",
            Self::Acquired => "ACQUIRED",
            Self::Processing => "PROCESSING",
            Self::NotEnabled => "NOT_ENABLED",
            Self::Searchable => "SEARCHABLE",
            Self::Failed => "FAILED",
            Self::RiskControl => "RISK_CONTROL",
            Self::BytesCleaned => "BYTES_CLEANED",
            Self::WithdrawnOrRestricted => "WITHDRAWN_OR_RESTRICTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMediaKind {
    Cover,
    Image,
    Video,
    LivePhoto,
}

impl EvidenceMediaKind {
    pub fn as_purpose(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Image => "body_image",
            Self::Video => "video",
            Self::LivePhoto => "live_photo",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceRestriction {
    Unrestricted,
    Restricted,
    BytesCleaned,
    WithdrawnOrRestricted,
    Unknown,
}

impl EvidenceRestriction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unrestricted => "UNRESTRICTED",
            Self::Restricted | Self::WithdrawnOrRestricted => "WITHDRAWN_OR_RESTRICTED",
            Self::BytesCleaned => "BYTES_CLEANED",
            Self::Unknown => "UNKNOWN",
        }
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

/// Evidence Library's bounded time/reading view.
///
/// The wire name remains `window` for the local URL/API contract, but
/// `latest_accepted_discovery` is deliberately a view, not a publication-time window.  It
/// surfaces accepted Discovery material in latest-discovery order and keeps publication time
/// unknown when the producer did not supply it.  The two `published_*` values remain strict
/// `ContentItem.published_at` filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTimeView {
    LatestAcceptedDiscovery,
    #[serde(rename = "last_7_days")]
    PublishedLast7Days,
    #[serde(rename = "last_30_days")]
    PublishedLast30Days,
}

/// A source-published-time window. This is never inferred from an accepted/observed timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishedWindow {
    Last7Days,
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
