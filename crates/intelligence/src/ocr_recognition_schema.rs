//! Current image OCR provider response contract.
pub const CONTRACT: &str = "ocr-visual-audit.v1";
use crate::ai_foundation::store::FoundationError;
use linggan_evidence::ocr_source::ImageRole;
use serde::{Deserialize, Serialize};

const BBOX_MIN: f64 = 0.0;
const BBOX_MAX: f64 = 1.0;
const VISUAL_ROLES: &[&str] = &[
    "main_title",
    "body",
    "chapter_heading",
    "subtitle",
    "background",
    "ui",
    "brand",
    "call_to_action",
    "other",
    "uncertain",
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TitlePresence {
    Present,
    Absent,
    Uncertain,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualRole {
    MainTitle,
    Body,
    ChapterHeading,
    Subtitle,
    Background,
    Ui,
    Brand,
    CallToAction,
    Other,
    Uncertain,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Legibility {
    Clear,
    Partial,
    Unreadable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualRegion {
    pub region_ref: String,
    pub text: String,
    pub role: VisualRole,
    pub legibility: Legibility,
    /// [x_min,y_min,x_max,y_max] in 0..=1 original-image coordinates; unknown is null.
    pub bbox: Option<[f64; 4]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualResult {
    pub title_presence: TitlePresence,
    pub title_text: Option<String>,
    pub regions: Vec<VisualRegion>,
    pub unreadable: bool,
}
pub fn qualifies_probe(v: &VisualResult) -> bool {
    !v.unreadable
        && v.regions
            .iter()
            .any(|r| r.legibility == Legibility::Clear && r.text.trim() == "42")
}
/// Stable local diagnostics; rejected text is never normalized into a successful reading.
pub fn parse_visual_diagnostic(
    text: &str,
    finish: Option<&str>,
) -> Result<VisualResult, &'static str> {
    if finish != Some("stop") || text.len() > 131072 || text.trim().is_empty() {
        return Err("ocr_visual_schema_invalid");
    }
    let v: VisualResult = serde_json::from_str(text).map_err(|_| "ocr_visual_schema_invalid")?;
    let mut refs = std::collections::HashSet::new();
    if v.regions.len() > 200
        || v.title_text
            .as_ref()
            .is_some_and(|s| s.chars().count() > 500)
        || (v.title_presence == TitlePresence::Present
            && v.title_text.as_ref().is_none_or(|s| s.trim().is_empty()))
        || (v.title_presence == TitlePresence::Absent && v.title_text.is_some())
        || (v.regions.is_empty() && !v.unreadable && v.title_presence != TitlePresence::Absent)
        || v.regions.iter().any(|r| {
            r.region_ref.is_empty()
                || r.region_ref.len() > 80
                || !refs.insert(&r.region_ref)
                || r.text.chars().count() > 20_000
        })
    {
        return Err("ocr_visual_schema_invalid");
    }
    if v.regions.iter().any(|r| {
        r.bbox.is_some_and(|b| {
            b.iter()
                .any(|n| !n.is_finite() || !(BBOX_MIN..=BBOX_MAX).contains(n))
                || b[0] >= b[2]
                || b[1] >= b[3]
        })
    }) {
        return Err("ocr_visual_coordinates_invalid");
    }
    // Compare visible title characters in reading order; never rewrite the returned text.
    if let Some(title) = &v.title_text {
        let compact = |text: &str| {
            text.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        };
        let candidate = compact(title);
        let title_regions: Vec<_> = v
            .regions
            .iter()
            .filter(|r| r.role == VisualRole::MainTitle)
            .collect();
        let ordered_text = title_regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<String>();
        if !v
            .regions
            .iter()
            .any(|r| r.role == VisualRole::MainTitle && compact(&r.text) == candidate)
            && (candidate.is_empty() || compact(&ordered_text) != candidate)
        {
            return Err("ocr_visual_title_mismatch");
        }
    }
    Ok(v)
}
pub fn parse_visual(text: &str, finish: Option<&str>) -> Result<VisualResult, FoundationError> {
    parse_visual_diagnostic(text, finish).map_err(|_| FoundationError::Invalid)
}

pub fn prompt_for_image(
    instruction: &str,
    version: uuid::Uuid,
    context: Option<(ImageRole, Option<u32>)>,
) -> String {
    serde_json::json!({"contract":CONTRACT,"promptVersion":version,
        "imageContext":context.map(|(role,ordinal)|serde_json::json!({"imageRole":role,"sequenceOrdinal":ordinal})),
        "roleVocabulary":VISUAL_ROLES,
        "legibilityVocabulary":["clear","partial","unreadable"],
        "requiredFields":{"root":["titlePresence","titleText","regions","unreadable"],"region":["regionRef","text","role","legibility","bbox"]},
        "instruction":instruction}).to_string()
}
pub fn response_schema() -> crate::ai_foundation::transport::ResponseSchema {
    use serde_json::json;
    crate::ai_foundation::transport::ResponseSchema {
        name: "ocr_visual_audit".into(),
        schema: json!({
        "type":"object","additionalProperties":false,"required":["titlePresence","titleText","regions","unreadable"],
        "properties":{"titlePresence":{"type":"string","enum":["present","absent","uncertain"]},"titleText":{"type":["string","null"]},"unreadable":{"type":"boolean"},
        "regions":{"type":"array","maxItems":200,"items":{"type":"object","additionalProperties":false,"required":["regionRef","text","role","legibility","bbox"],"properties":{
        "regionRef":{"type":"string"},"text":{"type":"string"},"role":{"type":"string","enum":VISUAL_ROLES},
        "legibility":{"type":"string","enum":["clear","partial","unreadable"]},"bbox":{"type":["array","null"],"items":{"type":"number","minimum":BBOX_MIN,"maximum":BBOX_MAX},"minItems":4,"maxItems":4}}}}}}),
    }
}
