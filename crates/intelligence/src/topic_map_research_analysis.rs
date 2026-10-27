//! Typed topic-map analysis acceptance. Quotes never become a second source of raw text.
use linggan_evidence::creator_discovery::Fragment;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

mod resolution;
mod schema;
mod transport;
mod validation;
use validation::{
    citation_rule, validate_angle, validate_discussion, validate_journey, validate_opportunity,
    validate_response,
};
pub mod verification;
pub use resolution::{
    ConceptDefinition, ProposedParent, ProposedTopic, ResolutionOutput, TopicMatch, TopicRelation,
    UnitDecision, validate_resolution,
};
pub use schema::{output_schema, resolution_schema};
pub use transport::{
    COMPACT_EXTRACT_CONTRACT, CompactDecodeError, compact_field_names, compact_output_schema,
    decode_compact_output,
};

pub const METHOD_VERSION: &str = "topic-map.research.v6";
pub const EXTRACT_CONTRACT: &str = "topic-map.research.v6";
pub const RESOLVE_CONTRACT: &str = "topic-map.resolve.v2";
pub const MAX_DISCUSSIONS: usize = 24;
pub const TOPIC_TRANSPORT_BYTE_LIMIT: usize = 1_048_576;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    pub fragment_id: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Discussion {
    pub label: String,
    pub topic_ref: Option<Uuid>,
    pub evidence: Vec<Citation>,
    // Defaults decode historical v1 results; current extraction requires every value.
    #[serde(default)]
    pub statement: String,
    #[serde(default)]
    pub definition: String,
    #[serde(default)]
    pub inclusion_criteria: Vec<String>,
    #[serde(default)]
    pub exclusion_criteria: Vec<String>,
    #[serde(default)]
    pub speaker_role: String,
    #[serde(default)]
    pub evidence_role: String,
    #[serde(default)]
    pub rationale: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene {
    pub label: String,
    pub evidence: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Journey {
    pub main_stage: String,
    pub involved_stages: Vec<String>,
    pub overlays: Vec<String>,
    pub path: String,
    pub rationale: String,
    pub evidence: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponseMatch {
    pub status: String,
    pub unanswered: Vec<String>,
    pub evidence: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Angle {
    pub label: String,
    pub title: String,
    pub answer_task: String,
    pub evidence: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductOpportunity {
    pub need: String,
    pub hypothesis: String,
    pub verification_question: String,
    pub alternative_explanation: String,
    pub evidence: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchOutput {
    pub contract: String,
    pub outcome: String,
    pub discussions: Vec<Discussion>,
    pub scenes: Vec<Scene>,
    pub journey: Journey,
    pub response_matches: Vec<ResponseMatch>,
    pub angles: Vec<Angle>,
    pub product_opportunities: Vec<ProductOpportunity>,
    pub limitations: Vec<String>,
}
pub const STAGES: [&str; 8] = [
    "discover_understand",
    "seek_assessment",
    "choose_support",
    "begin_practice",
    "long_term_manage",
    "cross_stage",
    "general_background",
    "unclear",
];
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max
}
fn criteria(inclusion: &[String], exclusion: &[String]) -> bool {
    let mut seen = HashSet::new();
    [inclusion, exclusion].iter().all(|items| {
        !items.is_empty()
            && items.len() <= 8
            && items
                .iter()
                .all(|v| text(v, 300) && seen.insert(v.trim().to_lowercase()))
    })
}
fn valid_fragments(fragments: &[Fragment]) -> bool {
    let mut ids = HashSet::new();
    fragments.iter().all(|f| {
        !f.fragment_id.trim().is_empty()
            && ids.insert(&f.fragment_id)
            && f.end.checked_sub(f.start) == Some(f.text.chars().count())
    })
}
pub(crate) fn fragment_role(fragment: &Fragment) -> &'static str {
    match fragment.field.as_str() {
        "title" | "body" | "tags" | "ocr" | "transcript" | "frame_text" => {
            match fragment.speaker.as_str() {
                "author" => "author",
                "other" => "quoted",
                _ => "unknown",
            }
        }
        "studied_comment" | "unresearched_comment" => "commenter",
        "parent_comment_context" => "context",
        field if field.starts_with("work_context:") => "context",
        _ => "unknown",
    }
}
fn discussion_roles(d: &Discussion, fragments: &[Fragment]) -> bool {
    let roles: Vec<_> = d
        .evidence
        .iter()
        .filter_map(|c| {
            fragments
                .iter()
                .find(|f| f.fragment_id == c.fragment_id)
                .map(fragment_role)
        })
        .collect();
    if !["support", "challenge", "context"].contains(&d.evidence_role.as_str())
        || (d.evidence_role != "context" && roles.iter().all(|r| *r == "context"))
    {
        return false;
    }
    match d.speaker_role.as_str() {
        "author" => roles.iter().all(|r| *r == "author"),
        "commenter" => {
            roles.contains(&"commenter")
                && roles.iter().all(|r| ["commenter", "context"].contains(r))
        }
        "quoted" | "unknown" => true,
        _ => false,
    }
}

/// A parent's valid text is not evidence that it belongs to every child in a batch.
/// The input preparation freezes precise child -> parent-range identities.
pub fn validate_context_links(
    output: &ResearchOutput,
    fragments: &[Fragment],
    comment_study: &serde_json::Value,
) -> Result<(), &'static str> {
    for citations in output
        .discussions
        .iter()
        .map(|d| d.evidence.as_slice())
        .chain(output.scenes.iter().map(|s| s.evidence.as_slice()))
        .chain(std::iter::once(output.journey.evidence.as_slice()))
        .chain(
            output
                .response_matches
                .iter()
                .map(|r| r.evidence.as_slice()),
        )
        .chain(output.angles.iter().map(|a| a.evidence.as_slice()))
        .chain(
            output
                .product_opportunities
                .iter()
                .map(|p| p.evidence.as_slice()),
        )
    {
        for parent in citations.iter().filter(|citation| {
            fragments.iter().any(|f| {
                f.fragment_id == citation.fragment_id && f.field == "parent_comment_context"
            })
        }) {
            let related = citations
                .iter()
                .filter(|citation| {
                    fragments.iter().any(|f| {
                        f.fragment_id == citation.fragment_id
                            && matches!(
                                f.field.as_str(),
                                "studied_comment" | "unresearched_comment"
                            )
                    })
                })
                .any(|child| {
                    comment_study
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|comment| {
                            comment["fragmentId"].as_str() == Some(child.fragment_id.as_str())
                                && comment["parentFragmentIds"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .any(|id| id.as_str() == Some(parent.fragment_id.as_str()))
                        })
                });
            if !related {
                return Err("invalid_comment_parent_context");
            }
        }
    }
    Ok(())
}

/// A reproducible estimate, not a tokenizer count or a guarantee for every provider.
/// Keep this formula in sync with estimateTopicInputTokens in the Pi adapter.
pub fn estimate_input_tokens(system: &str, prompt: &str) -> i64 {
    crate::ai_foundation::transport::estimate_input_tokens(system, prompt) as i64
}
fn citations(c: &[Citation], fragments: &[Fragment], required: bool) -> bool {
    (!required || !c.is_empty())
        && c.len() <= 12
        && c.iter().all(|r| {
            fragments.iter().any(|f| {
                f.fragment_id == r.fragment_id
                    && r.start >= f.start
                    && r.end > r.start
                    && r.end <= f.end
                    && r.end - r.start <= 1000
            })
        })
}

pub fn validate_output(
    output: &ResearchOutput,
    fragments: &[Fragment],
    topic_refs: &[Uuid],
) -> Result<(), &'static str> {
    let modern = matches!(
        output.contract.as_str(),
        EXTRACT_CONTRACT
            | "topic-map.research.v2"
            | "topic-map.research.v3"
            | "topic-map.research.v4"
            | "topic-map.research.v5"
    );
    if (!modern && output.contract != "topic-map.research.v1")
        || !["analyzed", "no_signal", "insufficient"].contains(&output.outcome.as_str())
        || output.discussions.len() > if modern { MAX_DISCUSSIONS } else { 8 }
        || output.scenes.len() > 8
        || output.angles.len() > 5
        || output.product_opportunities.len() > 5
        || output.response_matches.len() > 10
        || output.limitations.len() > 12
        || output.limitations.iter().any(|v| !text(v, 500))
    {
        return Err("invalid_output_structure");
    }
    if modern && !valid_fragments(fragments) {
        return Err("invalid_unicode_fragment");
    }
    let j = &output.journey;
    validate_journey(j, fragments)?;
    for d in &output.discussions {
        validate_discussion(d, fragments, topic_refs, modern)?;
    }
    for scene in &output.scenes {
        if !text(&scene.label, 300) {
            return Err("scene_label_invalid");
        }
        citation_rule(&scene.evidence, fragments, true)?;
    }
    for response in &output.response_matches {
        validate_response(response, fragments, modern)?;
    }
    for angle in &output.angles {
        validate_angle(angle, fragments)?;
    }
    for opportunity in &output.product_opportunities {
        validate_opportunity(opportunity, fragments)?;
    }
    for field in 0..3 {
        let keys: HashSet<_> = output
            .angles
            .iter()
            .map(|a| match field {
                0 => a.label.trim().to_lowercase(),
                1 => a.title.trim().to_lowercase(),
                _ => a.answer_task.trim().to_lowercase(),
            })
            .collect();
        if keys.len() != output.angles.len() {
            return Err("duplicate_angle_task");
        }
    }
    let unique: HashSet<_> = j.involved_stages.iter().collect();
    if unique.len() != j.involved_stages.len() {
        return Err("duplicate_journey_stage");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
