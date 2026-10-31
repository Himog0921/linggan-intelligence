//! CONTENT-WORKBENCH-001 typed, versioned document and command boundary.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Note,
    Comment,
    Ocr,
    Transcript,
    FrameText,
    OpsPublication,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceField {
    Title,
    Body,
    Ocr,
    Tags,
    Transcript,
    FrameText,
    CommentBody,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    pub block_ref: String,
    pub as_of: String,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub field: SourceField,
    pub source_version: String,
    pub start: usize,
    pub end: usize,
    pub text_hash: String,
    pub location: Option<OcrLocation>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OcrLocation {
    pub unit_ref: Uuid,
    pub reading_ref: Uuid,
    pub block_ref: String,
    pub slot_key: String,
    pub bbox: [f64; 4],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Paragraph {
    pub id: Uuid,
    pub text: String,
}
/// Provenance of a whole paragraph containing a personal experience or an edit derived from it.
/// The paragraph text is projected from erasable storage and is not embedded in revisions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperienceSlot {
    pub paragraph_id: Uuid,
    pub experience_ref: Uuid,
    pub experience_revision: i32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Excerpt {
    pub id: Uuid,
    pub citation: Citation,
    pub selected_text: Option<String>,
    pub text_hash: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasGroup {
    Inbox,
    Opening,
    Argument,
    Example,
    Ending,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanvasBlock {
    pub id: Uuid,
    pub excerpt_id: Option<Uuid>,
    pub group: CanvasGroup,
    pub note: String,
    pub expression: String,
    pub expression_revision: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Cover,
    Opinion,
    Voice,
    Steps,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Palette {
    Cream,
    Sage,
    Ink,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageStyle {
    #[serde(default)]
    pub alignment: Alignment,
    pub palette: Palette,
    pub font_family: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageBlock {
    pub id: Uuid,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImagePage {
    pub id: Uuid,
    pub kind: PageKind,
    pub template_version: String,
    pub title: String,
    pub subtitle: String,
    pub blocks: Vec<PageBlock>,
    pub asset_id: Option<Uuid>,
    pub crop: Crop,
    pub style: PageStyle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UseTarget {
    Title,
    Angle,
    Outline {
        index: usize,
    },
    Tag {
        index: usize,
    },
    CanvasNote {
        #[serde(rename = "canvasId")]
        canvas_id: Uuid,
    },
    CanvasExpression {
        #[serde(rename = "canvasId")]
        canvas_id: Uuid,
    },
    PageTitle {
        #[serde(rename = "pageId")]
        page_id: Uuid,
    },
    PageSubtitle {
        #[serde(rename = "pageId")]
        page_id: Uuid,
    },
    Paragraph {
        #[serde(rename = "paragraphId")]
        paragraph_id: Uuid,
    },
    PageBlock {
        #[serde(rename = "pageId")]
        page_id: Uuid,
        #[serde(rename = "blockId")]
        block_id: Uuid,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UseState {
    Current,
    Modified,
    NeedsReview,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentOrigin {
    Excerpt,
    Expression,
}
/// Exact placement coordinates; source identity remains the linked Citation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    pub text_hash: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialUse {
    #[serde(default)]
    pub source_span: Option<SourceSpan>,
    pub id: Uuid,
    pub canvas_id: Uuid,
    pub expression_revision: u32,
    pub excerpt_id: Option<Uuid>,
    pub target: UseTarget,
    pub placed_text_hash: String,
    pub state: UseState,
    pub content_origin: ContentOrigin,
}
/// An output target derived from a selected source, including paraphrases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DerivedSource {
    pub target: UseTarget,
    pub citation: Citation,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialSelection {
    pub document_id: String,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub source_version: String,
    pub added_by: String,
    pub match_reasons: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaterialPreparation {
    pub method_version: String,
    pub query: String,
    pub terms: Vec<String>,
    pub state: String,
    pub gaps: Vec<String>,
}
/// Whose voice a bridge sentence is written in; the account identity decides which group applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeGroup {
    Parent,
    Coach,
}
/// The library sentence a draft uses. `text` is what this draft says and may be rewritten
/// locally (`edited`), after which it no longer follows the library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeUse {
    pub sentence_code: String,
    pub sentence_revision: i32,
    pub group: BridgeGroup,
    pub text: String,
    #[serde(default)]
    pub edited: bool,
}
/// The bridge slot of a draft (CREATION-ASSETS-001). `skip` is "这篇不植入"; an absent
/// sentence with `skip == false` is an undecided slot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeChoice {
    #[serde(default)]
    pub skip: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<BridgeUse>,
    /// The bridge follows this paragraph; absent or no longer present means the default slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_paragraph: Option<Uuid>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkDocument {
    /// Non-sensitive ordering metadata for erasable experience-bound source uses; never client input.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experience_use_order: Vec<Uuid>,
    /// Changes when the server discards stale editor state following a withdrawal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experience_epoch: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experience_slots: Vec<ExperienceSlot>,
    /// Paragraphs explicitly reserved for a real experience that has not yet been supplied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub experience_gaps: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<BridgeChoice>,
    /// Whether the cover image is AI-made; absent means not recorded, never "no".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_ai: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<MaterialSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_preparation: Option<MaterialPreparation>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Manually entered alternatives, kept with the draft revision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cover_title_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_title_candidates: Vec<String>,
    pub contract: String,
    pub title: String,
    pub account_ref: Option<Uuid>,
    pub topic_ref: Option<Uuid>,
    /// Creation idea identity is distinct from topic_ref, which belongs to the research graph.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea_ref: Option<Uuid>,
    /// Only the idea's anchor work owns its name; its angle holds the idea's intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idea_name: Option<String>,
    pub angle: String,
    pub outline: Vec<String>,
    pub paragraphs: Vec<Paragraph>,
    pub excerpts: Vec<Excerpt>,
    pub canvas: Vec<CanvasBlock>,
    pub pages: Vec<ImagePage>,
    pub uses: Vec<MaterialUse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_sources: Vec<DerivedSource>,
}
impl Default for WorkDocument {
    fn default() -> Self {
        Self {
            contract: "content-workbench.v1".into(),
            experience_epoch: None,
            experience_use_order: vec![],
            experience_slots: vec![],
            experience_gaps: vec![],
            bridge: None,
            cover_ai: None,
            materials: vec![],
            material_preparation: None,
            tags: vec![],
            cover_title_candidates: vec![],
            note_title_candidates: vec![],
            title: String::new(),
            account_ref: None,
            topic_ref: None,
            idea_ref: None,
            idea_name: None,
            angle: String::new(),
            outline: vec![],
            paragraphs: vec![],
            excerpts: vec![],
            canvas: vec![],
            pages: vec![],
            uses: vec![],
            derived_sources: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Revision {
    pub id: Uuid,
    pub number: i64,
    pub saved_at: String,
    pub document: WorkDocument,
}
/// Metadata only; a current reference format still requires a qualified detail read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionSummary {
    pub id: Uuid,
    pub number: i64,
    pub saved_at: String,
    pub reference_format: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub document: WorkDocument,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRequest {
    pub request_id: Uuid,
    pub domain_ref: Uuid,
    pub document: WorkDocument,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub restore_revision: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetWithdrawnRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlaceRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub canvas_id: Uuid,
    pub targets: Vec<UseTarget>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReceipt {
    pub request_id: Uuid,
    pub work_id: Uuid,
    pub outcome: String,
    pub replayed: bool,
    pub revision: Revision,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub excerpt_id: Uuid,
    pub state: String,
    pub code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkEnvelope {
    pub work_id: Uuid,
    pub domain_ref: Uuid,
    pub saved_by: String,
    pub revision: Revision,
    pub experience_reset_required: bool,
    pub source_statuses: Vec<SourceStatus>,
    pub exports: Vec<ExportRecord>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    pub work_id: Uuid,
    pub domain_ref: Uuid,
    pub title: String,
    pub revision_id: Uuid,
    pub saved_at: String,
    /// The current revision if confirmed copied to the clipboard; this is not a publication.
    pub copied_revision_id: Option<Uuid>,
    /// Linked Operations record and the revision it describes, never inferred from clipboard state.
    pub published_revision_id: Option<Uuid>,
    pub publication_unavailable: bool,
    pub idea_ref: Option<Uuid>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeaBoxSummary {
    pub idea_ref: Uuid,
    pub anchor_work_id: Uuid,
    pub anchor_revision_id: Uuid,
    pub saved_at: String,
    pub name: String,
    pub intent: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub sha256: String,
    pub byte_size: i64,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FreezeRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub renderer_version: String,
    pub font_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPage {
    pub page_id: Uuid,
    pub ordinal: u32,
    pub sha256: Option<String>,
    pub byte_size: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub work_id: Uuid,
    pub revision_id: Uuid,
    pub state: String,
    pub failure_code: Option<String>,
    pub renderer_version: String,
    pub font_version: String,
    pub pages: Vec<ExportPage>,
    pub package_sha256: Option<String>,
    pub package_byte_size: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FinalizeRequest {
    pub request_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportFailureRequest {
    pub request_id: Uuid,
    pub code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdoptionLink {
    pub suggestion_id: Uuid,
    pub action: String,
    pub input_revision: Uuid,
    pub model_invocation_ref: Uuid,
    pub method_version: String,
}
