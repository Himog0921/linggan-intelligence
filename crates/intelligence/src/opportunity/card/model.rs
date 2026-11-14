//! JSON contract for the versioned opportunity read model.
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Card {
    pub problem: Problem,
    pub as_of: String,
    pub versions: Versions,
    pub demand: Demand,
    pub supply: Supply,
    pub channel: Channel,
    pub flags: Vec<&'static str>,
    pub exclusions: Vec<Exclusion>,
    pub limitations: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Problem {
    pub id: Uuid,
    pub statement: String,
    pub level: i32,
    pub parent_id: Option<Uuid>,
    pub redirected_from: Option<Uuid>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Versions {
    pub taxonomy: i64,
    pub metric_def: i32,
    pub judgment_prompt: Option<String>,
    pub model: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Demand {
    pub groups: Vec<DemandGroup>,
    pub sample_voices: Vec<Voice>,
    pub user_methods: UnavailableMethods,
    pub importance: NotCalibrated,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DemandGroup {
    pub protocol_version: Option<i32>,
    pub mentioners_total: i32,
    pub evidence_tier: String,
    pub current: CurrentPeriod,
    pub baseline: Baseline,
    pub series: Vec<Period>,
    pub flags: Vec<&'static str>,
    #[serde(skip)]
    pub window_has_samples: bool,
    #[serde(skip)]
    pub previous_n: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentPeriod {
    pub period_start: String,
    pub k: i32,
    pub n: i32,
    pub rate_per_1k: Option<f64>,
    pub ci_per_1k: Option<Interval>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Period {
    pub period_start: String,
    pub k: i32,
    pub n: i32,
    pub rate_per_1k: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Baseline {
    pub k: i32,
    pub n: i32,
    pub rate_per_1k: Option<f64>,
}

#[derive(Serialize)]
pub(super) struct Interval {
    pub low: f64,
    pub high: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Voice {
    pub comment_ref: CommentRef,
    pub text: Option<String>,
    pub likes: Option<i64>,
    pub work_id: Uuid,
    pub tombstone: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CommentRef {
    pub content_ref: Uuid,
    pub comment_external_id: String,
}

#[derive(Serialize)]
pub(super) struct UnavailableMethods {
    pub status: &'static str,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub(super) struct NotCalibrated {
    pub status: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Supply {
    pub state: &'static str,
    pub judgment_status: Option<&'static str>,
    pub coverage: Option<()>,
    pub counts: Option<()>,
    pub works: Vec<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Channel {
    pub window_days: u32,
    pub min_n: u32,
    pub groups: Vec<CreatorGroup>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CreatorGroup {
    pub protocol_version: Option<i32>,
    pub ranked: Vec<Creator>,
    pub insufficient: Vec<Creator>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Creator {
    pub creator_id: String,
    pub name: Option<String>,
    pub followers: Option<i64>,
    pub k: i32,
    pub n: i32,
    pub rate_per_1k: f64,
    pub ci_per_1k: Interval,
    pub observed: bool,
}

#[derive(Serialize)]
pub(super) struct Exclusion {
    pub reason: String,
    pub comments: i64,
}
