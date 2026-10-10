use super::{Discussion, RESOLVE_CONTRACT, criteria, text};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicMatch {
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposedTopic {
    pub label: String,
    pub definition: String,
    pub inclusion_criteria: Vec<String>,
    pub exclusion_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopicRelation {
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
    pub relation: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnitDecision {
    pub unit_id: String,
    pub status: String,
    pub matches: Vec<TopicMatch>,
    #[serde(deserialize_with = "required_nullable_topic")]
    pub proposed_topic: Option<ProposedTopic>,
    pub relations: Vec<TopicRelation>,
    pub reason: String,
}

fn required_nullable_topic<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ProposedTopic>, D::Error> {
    Option::<ProposedTopic>::deserialize(deserializer)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolutionOutput {
    pub contract: String,
    pub decisions: Vec<UnitDecision>,
}

pub const RELATIONS: [&str; 6] = [
    "equivalent",
    "broader",
    "narrower",
    "related",
    "distinct",
    "uncertain",
];

fn candidate_versions(topics: &Value) -> Result<HashMap<Uuid, Uuid>, &'static str> {
    let Some(candidate_values) = topics.as_array() else {
        return Err("invalid_resolution_candidates");
    };
    if candidate_values.len() > 64 {
        return Err("invalid_resolution_candidates");
    }
    let mut candidates = HashMap::new();
    for candidate in candidate_values {
        let topic = candidate["topicRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok());
        let definition = candidate["definitionRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok());
        let (Some(topic), Some(definition)) = (topic, definition) else {
            return Err("invalid_resolution_candidates");
        };
        if candidates.insert(topic, definition).is_some() {
            return Err("duplicate_resolution_candidate");
        }
    }
    Ok(candidates)
}

/// Validates coverage, explicit concept boundaries, and exact candidate versions.
/// It cannot prove that a model's semantic comparison is correct.
pub fn validate_resolution(
    output: &ResolutionOutput,
    units: &[(String, Discussion)],
    topics: &Value,
) -> Result<(), &'static str> {
    if output.contract != RESOLVE_CONTRACT
        || output.decisions.len() != units.len()
        || units.len() > super::MAX_DISCUSSIONS
    {
        return Err("invalid_resolution_structure");
    }
    let unit_ids: HashSet<_> = units.iter().map(|(id, _)| id.as_str()).collect();
    if unit_ids.len() != units.len() || unit_ids.iter().any(|id| !text(id, 200)) {
        return Err("invalid_resolution_units");
    }
    let candidates = candidate_versions(topics)?;
    let mut seen_units = HashSet::new();
    for decision in &output.decisions {
        if !unit_ids.contains(decision.unit_id.as_str())
            || !seen_units.insert(decision.unit_id.as_str())
            || !text(&decision.reason, 500)
            || decision.matches.len() > 4
            || decision.relations.len() > 64
        {
            return Err("invalid_resolution_decision");
        }
        let mut matches = HashSet::new();
        for matched in &decision.matches {
            if candidates.get(&matched.topic_ref) != Some(&matched.definition_ref)
                || !matches.insert(matched.topic_ref)
                || !text(&matched.reason, 500)
            {
                return Err("invalid_resolution_match");
            }
        }
        let mut relations = HashMap::new();
        for relation in &decision.relations {
            if candidates.get(&relation.topic_ref) != Some(&relation.definition_ref)
                || !RELATIONS.contains(&relation.relation.as_str())
                || !text(&relation.reason, 500)
                || relations
                    .insert(relation.topic_ref, relation.relation.as_str())
                    .is_some()
                || (matches.contains(&relation.topic_ref)
                    && ["distinct", "uncertain"].contains(&relation.relation.as_str()))
            {
                return Err("invalid_resolution_relation");
            }
        }
        match decision.status.as_str() {
            "matched" if !matches.is_empty() && decision.proposed_topic.is_none() => {}
            "new" if matches.is_empty() => {
                let Some(proposed) = &decision.proposed_topic else {
                    return Err("missing_proposed_topic");
                };
                if !text(&proposed.label, 120)
                    || !text(&proposed.definition, 1000)
                    || !criteria(&proposed.inclusion_criteria, &proposed.exclusion_criteria)
                    || relations.len() != candidates.len()
                    || relations.values().any(|r| *r == "equivalent")
                {
                    return Err("invalid_new_topic_boundary");
                }
            }
            "uncertain" | "out_of_scope"
                if matches.is_empty() && decision.proposed_topic.is_none() => {}
            _ => return Err("invalid_resolution_status"),
        }
    }
    Ok(())
}
