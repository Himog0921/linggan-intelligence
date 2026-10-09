//! Canonical Topic Map projection and explicit, versioned human decisions.
#[path = "topic_map/read.rs"]
mod read;
#[path = "topic_map/saved.rs"]
mod saved;
#[path = "topic_map/store.rs"]
mod store;
pub use read::read_topic_map;
pub use saved::{
    TopicMapSavedAlternative, TopicMapSavedFragment, TopicMapSavedRange, read_saved_alternative,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub(crate) use store::{accept_topic_map_candidates_in, append_work_annotation_in};
pub use store::{append_work_annotation, save_topic_map_command};
use uuid::Uuid;

pub const METHOD_VERSION: &str = "topic-map.v41.1";
pub const MAIN_STAGES: [&str; 5] = [
    "discover_understand",
    "seek_assessment",
    "choose_support",
    "begin_practice",
    "long_term_manage",
];
pub const OTHER_STAGES: [&str; 4] = ["cross_stage", "general_background", "unclear", "pending"];
pub const OVERLAYS: [&str; 2] = ["obstruction_recurrence", "transition_handoff"];
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicMapQuery {
    pub domain_ref: Option<Uuid>,
    pub topic_ref: Option<Uuid>,
    pub platform: Option<String>,
    pub window_days: Option<i32>,
    pub reference_window_days: Option<i32>,
    pub overlay: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub include_reference: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapSnapshot {
    pub domains: Vec<Value>,
    pub scope: Value,
    pub topics: Vec<TopicMapTopic>,
    pub works: Vec<TopicMapWork>,
    pub statistics: Value,
    pub journey: Value,
    pub sources: Vec<Value>,
    pub candidates: Vec<Value>,
    pub changes: Vec<Value>,
    pub alternatives: Vec<Value>,
    pub own_creators: Vec<Value>,
    pub method_version: &'static str,
    pub source_boundary: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapTopic {
    pub topic_ref: Uuid,
    pub canonical_key: String,
    pub domain_ref: Option<Uuid>,
    pub definition_ref: Uuid,
    pub definition_version: i32,
    pub display_name: String,
    pub definition_text: String,
    pub lifecycle_state: String,
    pub parent_topic_ref: Option<Uuid>,
    pub binding_version: Option<i32>,
    pub direct_work_refs: Vec<Uuid>,
    pub work_refs: Vec<Uuid>,
    pub statistics: Value,
    pub journey: Value,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapWork {
    pub work_ref: Uuid,
    pub platform: String,
    pub content_external_id: String,
    pub title: Option<String>,
    pub title_source: String,
    pub author_external_id: Option<String>,
    pub creator_display_name: Option<String>,
    pub published_at: Option<String>,
    pub published_at_source_text: Option<String>,
    pub published_at_precision: String,
    pub likes: Option<i64>,
    pub comments: Option<i64>,
    pub collects: Option<i64>,
    pub shares: Option<i64>,
    pub follower_count: Option<i64>,
    pub own: bool,
    pub own_breakout: bool,
    pub readable: bool,
    pub usage_roles: Vec<String>,
    pub topic_refs: Vec<Uuid>,
    pub main_stage: String,
    pub involved_stages: Vec<String>,
    pub overlays: Vec<String>,
    pub path: String,
    pub annotation: Option<Value>,
    pub research: Option<Value>,
    pub media: Value,
    pub preview: Value,
    pub evidence_fragment: Option<Value>,
    pub source_manifest: Value,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicMapCitation {
    pub fragment_id: String,
    pub source_ref: Uuid,
    pub field: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicMapAnnotationRequest {
    pub idempotency_key: String,
    pub domain_ref: Uuid,
    pub work_public_ref: Uuid,
    pub topic_ref: Option<Uuid>,
    pub definition_ref: Option<Uuid>,
    pub method_version: String,
    pub main_stage: String,
    pub involved_stages: Vec<String>,
    pub overlays: Vec<String>,
    pub path: String,
    pub rationale: String,
    pub evidence_citations: Vec<TopicMapCitation>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "action",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TopicMapCommand {
    OwnCreator {
        idempotency_key: String,
        domain_ref: Uuid,
        platform: String,
        author_external_id: String,
        active: bool,
    },
    Breakout {
        idempotency_key: String,
        domain_ref: Uuid,
        work_ref: Uuid,
        marked: bool,
    },
    SaveAlternative {
        idempotency_key: String,
        domain_ref: Uuid,
        topic_ref: Uuid,
        definition_ref: Uuid,
        title: String,
        angle: String,
        rationale: String,
        evidence_work_refs: Vec<Uuid>,
        method_version: String,
        #[serde(default)]
        research_result_ref: Option<Uuid>,
        #[serde(default)]
        research_angle_index: Option<usize>,
        #[serde(default)]
        research_opportunity_index: Option<usize>,
    },
    BindTopic {
        idempotency_key: String,
        domain_ref: Uuid,
        topic_ref: Uuid,
        parent_topic_ref: Option<Uuid>,
        expected_version: Option<i32>,
    },
    PerformanceRule {
        idempotency_key: String,
        domain_ref: Uuid,
        platform: String,
        like_threshold: i64,
        expected_version: Option<i32>,
    },
    Viewed {
        idempotency_key: String,
        domain_ref: Uuid,
        topic_ref: Uuid,
        definition_ref: Uuid,
        #[serde(default)]
        work_refs: Vec<Uuid>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapReceipt {
    pub receipt_ref: Uuid,
    pub action: String,
    pub subject_ref: Option<Uuid>,
    pub revision: i32,
    pub persisted_at: String,
}
#[derive(Debug, thiserror::Error)]
pub enum TopicMapError {
    #[error("invalid Topic Map request: {0}")]
    Invalid(&'static str),
    #[error("Topic Map resource not found")]
    NotFound,
    #[error("Topic Map version or idempotency conflict")]
    Conflict,
    #[error("Topic Map source unavailable: {0}")]
    Source(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub(crate) struct TopicMapCandidateRequest {
    pub label: String,
    pub topic_ref: Option<Uuid>,
    pub evidence_citations: Vec<TopicMapCitation>,
}

pub(crate) fn canonical_source_manifest(resource: &linggan_evidence::WorkResource) -> Value {
    use sha2::{Digest, Sha256};
    let derived = serde_json::to_vec(&resource.inspector["derivatives"]).unwrap_or_default();
    let derived_hash: String = Sha256::digest(derived)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    serde_json::json!({"contract":"topic-map.source-manifest.v1","workRef":resource.identity.public_ref,"fieldSources":resource.inspector.pointer("/overview/fields"),"derivedVersionHash":derived_hash,"titleSource":resource.display.title_source,"fragmentSourceRef":resource.evidence_fragment.as_ref().and_then(|f|f.source_ref),"fragmentSourceKind":resource.evidence_fragment.as_ref().map(|f|f.source_kind)})
}

/// Public and saved alternative manifests retain identities and version fingerprints only.
/// Accepted comment propositions remain behind their current-source-qualified research reader.
pub(crate) fn safe_research_manifest(value: &Value) -> Value {
    let input = &value["inputRefs"];
    let topics:Vec<Value>=input["topics"].as_array().into_iter().flatten().map(|t|serde_json::json!({"topicRef":t["topicRef"],"definitionRef":t["definitionRef"],"version":t["version"]})).collect();
    serde_json::json!({"resultRef":value["resultRef"],"angleIndex":value["angleIndex"],"opportunityIndex":value["opportunityIndex"],"citations":value["citations"],"performanceRules":value["performanceRules"],"inputRefs":{"workRef":input["workRef"],"contextWorkRefs":input["contextWorkRefs"],"methodVersion":input["methodVersion"],"domain":input["domain"],"fragments":input["fragments"],"topics":topics}})
}

#[cfg(test)]
mod manifest_tests {
    use super::*;
    #[test]
    fn restricted_propositions_cannot_escape_through_alternative_metadata() {
        let result = serde_json::json!({"resultRef":"retained-ref","inputRefs":{"commentStudy":[{"signals":[{"proposition":"restricted-private-comment"}]}],"fragments":[{"fragmentId":"comment-ref","textHash":"immutable-fingerprint"}],"topics":[{"topicRef":"topic","definitionRef":"definition","definitionText":"private-derived-claim"}]}});
        let frozen = safe_research_manifest(&result);
        assert_eq!(frozen["resultRef"], "retained-ref");
        assert_eq!(
            frozen["inputRefs"]["fragments"][0]["fragmentId"],
            "comment-ref"
        );
        let wire = frozen.to_string();
        assert!(!wire.contains("restricted-private-comment"));
        assert!(!wire.contains("private-derived-claim"));
    }
}
