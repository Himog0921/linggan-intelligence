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
pub struct ConceptDefinition {
    pub label: String,
    pub definition: String,
    pub inclusion_criteria: Vec<String>,
    pub exclusion_criteria: Vec<String>,
    pub domain_fit: String,
    pub domain_reason: String,
    pub abstraction_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposedTopic {
    pub label: String,
    pub definition: String,
    pub inclusion_criteria: Vec<String>,
    pub exclusion_criteria: Vec<String>,
    pub domain_fit: String,
    pub domain_reason: String,
    pub abstraction_reason: String,
    pub parent: ProposedParent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProposedParent {
    pub kind: String,
    #[serde(deserialize_with = "required_nullable_uuid")]
    pub topic_ref: Option<Uuid>,
    #[serde(deserialize_with = "required_nullable_uuid")]
    pub definition_ref: Option<Uuid>,
    #[serde(deserialize_with = "required_nullable_concept")]
    pub proposal: Option<ConceptDefinition>,
    pub reason: String,
}

fn required_nullable_uuid<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Uuid>, D::Error> {
    Option::<Uuid>::deserialize(deserializer)
}
fn required_nullable_concept<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ConceptDefinition>, D::Error> {
    Option::<ConceptDefinition>::deserialize(deserializer)
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
                    || proposed.domain_fit != "in_scope"
                    || !text(&proposed.domain_reason, 500)
                    || !text(&proposed.abstraction_reason, 500)
                    || proposed.definition.trim()
                        == units
                            .iter()
                            .find(|(id, _)| id == &decision.unit_id)
                            .unwrap()
                            .1
                            .statement
                            .trim()
                {
                    return Err("invalid_new_topic_boundary");
                }
                validate_parent(proposed, &candidates, &relations)?;
            }
            "uncertain" | "out_of_scope"
                if matches.is_empty() && decision.proposed_topic.is_none() => {}
            _ => return Err("invalid_resolution_status"),
        }
    }
    Ok(())
}

fn validate_parent(
    child: &ProposedTopic,
    candidates: &HashMap<Uuid, Uuid>,
    relations: &HashMap<Uuid, &str>,
) -> Result<(), &'static str> {
    let parent = &child.parent;
    if !text(&parent.reason, 500) {
        return Err("invalid_topic_parent");
    }
    let valid = match parent.kind.as_str() {
        "root" => {
            parent.topic_ref.is_none()
                && parent.definition_ref.is_none()
                && parent.proposal.is_none()
        }
        "existing" => {
            parent
                .topic_ref
                .zip(parent.definition_ref)
                .is_some_and(|(topic, definition)| {
                    candidates.get(&topic) == Some(&definition)
                        && relations.get(&topic) == Some(&"narrower")
                })
                && parent.proposal.is_none()
        }
        "proposed" => {
            parent.topic_ref.is_none()
                && parent.definition_ref.is_none()
                && parent.proposal.as_ref().is_some_and(|proposal| {
                    text(&proposal.label, 120)
                        && text(&proposal.definition, 1000)
                        && criteria(&proposal.inclusion_criteria, &proposal.exclusion_criteria)
                        && proposal.domain_fit == "in_scope"
                        && text(&proposal.domain_reason, 500)
                        && text(&proposal.abstraction_reason, 500)
                        && proposal.label.trim() != child.label.trim()
                        && crate::topic_map_core::concept_identity(
                            &proposal.definition,
                            &proposal.inclusion_criteria,
                            &proposal.exclusion_criteria,
                        ) != crate::topic_map_core::concept_identity(
                            &child.definition,
                            &child.inclusion_criteria,
                            &child.exclusion_criteria,
                        )
                })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("invalid_topic_parent")
    }
}
