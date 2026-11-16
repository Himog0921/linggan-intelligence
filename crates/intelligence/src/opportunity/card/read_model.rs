//! Strict decoding of the persisted card before it crosses the HTTP boundary.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Card {
    pub problem: Problem,
    pub as_of: String,
    pub versions: Versions,
    pub demand: Demand,
    pub supply: Supply,
    pub channel: Channel,
    pub flags: Vec<QualityFlag>,
    pub exclusions: Vec<Exclusion>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Problem {
    pub id: Uuid,
    pub statement: String,
    pub level: u8,
    pub parent_id: Option<Uuid>,
    pub redirected_from: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Versions {
    pub taxonomy: i64,
    pub metric_def: i32,
    pub judgment_prompt: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Demand {
    pub groups: Vec<DemandGroup>,
    pub sample_voices: Vec<Voice>,
    pub user_methods: UserMethods,
    pub importance: Importance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DemandGroup {
    pub protocol_version: Option<i32>,
    pub mentioners_total: u64,
    pub evidence_tier: EvidenceTier,
    pub current: Current,
    pub baseline: Baseline,
    pub series: Vec<Period>,
    pub flags: Vec<QualityFlag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EvidenceTier {
    Candidate,
    Established,
    Significant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Current {
    pub period_start: String,
    pub k: u64,
    pub n: u64,
    pub rate_per_1k: Option<f64>,
    pub ci_per_1k: Option<Interval>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Period {
    pub period_start: String,
    pub k: u64,
    pub n: u64,
    pub rate_per_1k: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Baseline {
    pub k: u64,
    pub n: u64,
    pub rate_per_1k: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Interval {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Voice {
    pub comment_ref: CommentRef,
    pub text: Option<String>,
    pub likes: Option<i64>,
    pub work_id: Uuid,
    pub tombstone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CommentRef {
    pub content_ref: Uuid,
    pub comment_external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum UserMethods {
    Unavailable { reason: String },
    Available { items: Vec<UserMethod> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UserMethod {
    pub label: String,
    pub mentioners: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Importance {
    NotCalibrated,
    Calibrated {
        #[serde(rename = "surveyId")]
        survey_id: String,
        importance: f64,
        satisfaction: f64,
        #[serde(rename = "opportunityScore")]
        opportunity_score: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Supply {
    pub state: SupplyState,
    pub judgment_status: Option<JudgmentStatus>,
    pub coverage: Option<Coverage>,
    pub counts: Option<SupplyCounts>,
    pub works: Vec<SupplyWork>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SupplyState {
    NotEvaluated,
    AwaitingUnderstanding,
    InsufficientCoverage,
    Covered,
    SuspectedGap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum JudgmentStatus {
    Verified,
    AiSuggested,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Coverage {
    pub found: u64,
    pub collected: u64,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SupplyCounts {
    pub answers: u64,
    pub mentions: u64,
    pub none: u64,
    pub unjudged: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SupplyWork {
    pub work_id: Uuid,
    pub title: Option<String>,
    pub creator_id: String,
    pub likes: Option<i64>,
    pub label: SupplyLabel,
    pub source: JudgmentSource,
    pub tombstone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SupplyLabel {
    Answers,
    Mentions,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum JudgmentSource {
    Human,
    Ai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Channel {
    pub window_days: u32,
    pub min_n: u32,
    pub groups: Vec<CreatorGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreatorGroup {
    pub protocol_version: Option<i32>,
    pub ranked: Vec<Creator>,
    pub insufficient: Vec<Creator>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Creator {
    pub creator_id: String,
    pub name: Option<String>,
    pub followers: Option<i64>,
    pub k: u64,
    pub n: u64,
    pub rate_per_1k: f64,
    pub ci_per_1k: Interval,
    pub observed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum QualityFlag {
    InsufficientSample,
    NotCalibrated,
    NoBaseline,
    CollectionVolumeShift,
    StaleReadModel,
    PeriodAnchorMissing,
    AuthorUnknownShareHigh,
    EngagementReferenceMissing,
    CollectionBelowTarget,
    CollectionProtocolChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Exclusion {
    pub reason: String,
    pub comments: u64,
}

#[cfg(test)]
mod tests {
    use super::Card;
    use serde_json::Value;

    #[test]
    fn snapshot_round_trips_and_unexpected_identity_fields_are_rejected() {
        let snapshot = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/opportunity_metrics_postgres/p1-card.snapshot.json"
        ));
        let mut raw: Value = serde_json::from_str(snapshot).unwrap();
        let card: Card = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(card).unwrap(), raw);
        raw["demand"]["sampleVoices"][0]["commenterHash"] = "private".into();
        assert!(serde_json::from_value::<Card>(raw).is_err());
    }
}
