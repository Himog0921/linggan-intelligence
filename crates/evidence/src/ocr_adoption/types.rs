use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageIdentity {
    pub work_ref: uuid::Uuid,
    pub image_ref: String,
    pub blob_sha256: String,
    pub width: u32,
    pub height: u32,
}

/// Source metadata, never inferred from title-like text. FirstImage means known ordinal 1,
/// not the first available member of an incomplete or unordered set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRole {
    ExplicitCover,
    FirstImage,
    Interior,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pipeline {
    pub version: String,
    pub raw_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageInput {
    pub schema_version: u8,
    pub image: ImageIdentity,
    pub image_role: ImageRole,
    pub sequence_ordinal: Option<u32>,
    pub layout: SavedLayout,
    pub anchors: SavedAnchors,
    #[serde(default)]
    pub rechecks: Vec<LocalRecheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedLayout {
    pub image: ImageIdentity,
    pub pipeline: Pipeline,
    pub completed: bool,
    pub blocks: Vec<Block>,
    pub terminations: Vec<Termination>,
    #[serde(default)]
    pub require_termination: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedAnchors {
    pub image: ImageIdentity,
    pub pipeline: Pipeline,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Block {
    pub block_ref: String,
    pub reading_order: u32,
    /// Original upstream label, kept separately from the business role.
    pub label: String,
    pub text: String,
    /// Original-image pixel rectangle [left, top, right, bottom]. Block granularity.
    pub bbox: [f64; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Line {
    pub line_ref: String,
    pub reading_order: u32,
    pub text: String,
    pub confidence: f64,
    /// Original-image pixels. No fabricated per-character location.
    pub bbox: [f64; 4],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Termination {
    pub block_ref: String,
    pub output_sha256: String,
    /// Absent if not saved. A whole-image "ok" or a sequential server log cannot fill it.
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub request_ref: Option<String>,
    #[serde(default)]
    pub raw_output_sha256: Option<String>,
    #[serde(default)]
    pub generated_tokens: Option<u64>,
    #[serde(default)]
    pub raw_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecheckKind {
    LineCrop,
    VlCrop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalRecheck {
    pub block_ref: String,
    pub image_blob_sha256: String,
    pub crop_bbox: [f64; 4],
    pub crop_sha256: String,
    pub raw_sha256: String,
    pub pipeline_version: String,
    pub kind: RecheckKind,
    pub text: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdoptionState {
    AdoptedBlock,
    /// Only exact source ranges are usable; the original block still needs review.
    AdoptedPartialBlock,
    AdoptedLineFallback,
    Pending,
    Excluded,
}

impl AdoptionState {
    pub fn has_unresolved_text(self) -> bool {
        matches!(self, Self::Pending | Self::AdoptedPartialBlock)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Supported,
    NeedsReview,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    StructuredText,
    TextCandidate,
    ImageHeading,
    CoverTitle,
    Context,
    Background,
    LayoutMetadata,
    OtherTitleGroup,
    NonNarrative,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    ExecutionIncomplete,
    FinishReasonMissing,
    TerminationIncomplete,
    OutputTruncated,
    ExcessiveRepetition,
    DecorationLike,
    ImageRegionText,
    StructuredLayoutUnverified,
    MathematicalLayoutUnverified,
    NoLineSupport,
    AnchorDisagreement,
    MatchingLineText,
    EmptyText,
    NonNarrativeLabel,
    SmallEdgeMetadata,
    NoProminentGroup,
    ProminentConnectedGroup,
    CompetingTitleGroups,
    SecondaryDistantGroup,
    InteriorImage,
    UnknownCoverIdentity,
    UnsupportedCandidate,
    LineFallback,
    FallbackNotConfirmed,
    CropLineAgreement,
    CropVlAgreement,
    CropSupportsFallback,
    ScreenshotUiContext,
    ScreenshotTextFiltered,
    ConflictingLocalReadings,
    UnverifiedSymbol,
    PartialLiteralAgreement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Block,
    Line,
}

/// Offsets are Unicode scalar half-open ranges into the original block/line text.
/// bbox is the actual source granularity, not a box estimated for this substring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub source_kind: SourceKind,
    pub source_ref: String,
    pub start: usize,
    pub end: usize,
    pub bbox: [f64; 4],
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionAssessment {
    /// Original Unicode scalar length. None for older stored assessments.
    #[serde(default)]
    pub source_char_count: Option<usize>,
    pub block_ref: String,
    pub reading_order: u32,
    pub bbox: [f64; 4],
    pub model_label: String,
    pub quality: Quality,
    pub adoption_state: AdoptionState,
    pub role: Role,
    pub reasons: Vec<Reason>,
    pub finish_reason: Option<String>,
    pub support_line_refs: Vec<String>,
    /// Diagnostic match only: punctuation/case normalized, never quote correctness.
    pub normalized_text_matches: bool,
    pub median_line_height_norm: Option<f64>,
    /// Only supported source text or explicitly identified line fallback. These remain
    /// text candidates; context/author/use eligibility must still be checked downstream.
    pub usable_fragments: Vec<Fragment>,
    pub fallback_candidates: Vec<Fragment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleState {
    Usable,
    None,
    Uncertain,
    NotCover,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleCandidate {
    pub block_refs: Vec<String>,
    pub text: String,
    pub fragments: Vec<Fragment>,
    pub bbox: [f64; 4],
    pub line_height_norm: f64,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverTitle {
    pub state: TitleState,
    pub text: Option<String>,
    pub fragments: Vec<Fragment>,
    pub reasons: Vec<Reason>,
    pub candidates: Vec<TitleCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub rule_version: String,
    pub input_fingerprint: String,
    pub image: ImageIdentity,
    pub image_role: ImageRole,
    pub sequence_ordinal: Option<u32>,
    pub pipeline: Pipeline,
    pub anchor_pipeline: Pipeline,
    pub execution_completed: bool,
    pub cover_title: CoverTitle,
    pub regions: Vec<RegionAssessment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplaySource {
    PlatformTitle,
    CoverOcr,
    Unknown,
}
#[derive(Debug, Serialize)]
pub struct DisplayTitle<'a> {
    pub source: DisplaySource,
    pub text: Option<&'a str>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InputError {
    #[error("schema_or_bounds_invalid")]
    Bounds,
    #[error("image_identity_mismatch")]
    Identity,
    #[error("source_hash_or_version_invalid")]
    Source,
    #[error("duplicate_or_invalid_reference_or_order")]
    Reference,
    #[error("geometry_or_confidence_invalid")]
    Geometry,
    #[error("region_receipt_mismatch")]
    Receipt,
    #[error("input_encoding_invalid")]
    Encoding,
}
