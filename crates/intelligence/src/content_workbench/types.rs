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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkDocument {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<MaterialSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_preparation: Option<MaterialPreparation>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub contract: String,
    pub title: String,
    pub account_ref: Option<Uuid>,
    pub topic_ref: Option<Uuid>,
    pub angle: String,
    pub outline: Vec<String>,
    pub paragraphs: Vec<Paragraph>,
    pub excerpts: Vec<Excerpt>,
    pub canvas: Vec<CanvasBlock>,
    pub pages: Vec<ImagePage>,
    pub uses: Vec<MaterialUse>,
}
impl Default for WorkDocument {
    fn default() -> Self {
        Self {
            contract: "content-workbench.v1".into(),
            materials: vec![],
            material_preparation: None,
            tags: vec![],
            title: String::new(),
            account_ref: None,
            topic_ref: None,
            angle: String::new(),
            outline: vec![],
            paragraphs: vec![],
            excerpts: vec![],
            canvas: vec![],
            pages: vec![],
            uses: vec![],
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
