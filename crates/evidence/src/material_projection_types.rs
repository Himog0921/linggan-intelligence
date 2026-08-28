//! Stable work-level material response envelope shared by focused read modules.

use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibraryProjection {
    pub items: Vec<MaterialLibraryItem>,
    pub query_scope: &'static str,
    pub as_of: String,
    pub cursor: Option<String>,
    pub truncated: bool,
    pub scan_limited: bool,
    pub scanned_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibraryItem {
    pub identity: MaterialIdentity,
    pub display: MaterialDisplay,
    pub preview: MaterialPreview,
    pub lane_summaries: Vec<MaterialLaneSummary>,
    pub summary: MaterialSummary,
    pub inspector: Value,
    pub matched_fields: Vec<&'static str>,
    #[serde(skip)]
    pub(crate) author_external_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialIdentity {
    pub platform: String,
    pub content_external_id: String,
    pub public_ref: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDisplay {
    pub title: Option<String>,
    pub title_state: String,
    pub creator_display_name: Option<String>,
    pub creator_state: String,
    pub published_at: Option<String>,
    pub published_at_source_text: Option<String>,
    pub published_at_state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPreview {
    pub local_asset_url: Option<String>,
    pub slot_purpose: Option<String>,
    pub bytes_state: &'static str,
    pub alt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLaneSummary {
    pub lane: &'static str,
    pub state: &'static str,
    pub observed: Option<i64>,
    pub retained: Option<i64>,
    pub failed: Option<i64>,
    pub known_unattempted: Option<i64>,
    pub maximum_quota: Option<i64>,
    pub value_state: &'static str,
    pub stopped_reason: Option<String>,
    pub limitations: Vec<&'static str>,
    pub latest_observed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSummary {
    pub last_observed_at: String,
    pub primary_limitation: &'static str,
    pub restriction_state: &'static str,
}

pub(crate) fn default_lane_summaries(
    observed_at: &str,
    has_detail: bool,
) -> Vec<MaterialLaneSummary> {
    [
        "discovery",
        "detail",
        "comments",
        "replies",
        "author",
        "media_slots",
        "media_bytes",
        "ocr",
        "asr",
    ]
    .into_iter()
    .map(|lane| MaterialLaneSummary {
        lane,
        state: if lane == "detail" && has_detail {
            "SEARCHABLE"
        } else {
            "UNKNOWN"
        },
        observed: (lane == "detail" && has_detail).then_some(1),
        retained: (lane == "detail" && has_detail).then_some(1),
        failed: None,
        known_unattempted: None,
        maximum_quota: None,
        value_state: if lane == "detail" && has_detail {
            "KNOWN"
        } else {
            "UNKNOWN"
        },
        stopped_reason: None,
        limitations: if lane == "detail" && has_detail {
            vec!["RAW_BODY_NOT_RETURNED"]
        } else {
            vec!["LANE_NOT_EVALUATED"]
        },
        latest_observed_at: (lane == "detail" && has_detail).then(|| observed_at.to_owned()),
    })
    .collect()
}
