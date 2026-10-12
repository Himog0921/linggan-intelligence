use super::{Discussion, RESOLVE_CONTRACT, text};
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
    let values = topics.as_array().ok_or("resolution_candidates_not_array")?;
    if values.len() > 64 {
        return Err("resolution_candidate_count_exceeded");
    }
    let mut candidates = HashMap::new();
    for candidate in values {
        let topic = candidate["topicRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .ok_or("resolution_candidate_topic_invalid")?;
        let definition = candidate["definitionRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .ok_or("resolution_candidate_definition_invalid")?;
        if candidates.insert(topic, definition).is_some() {
            return Err("duplicate_resolution_candidate");
        }
    }
    Ok(candidates)
}

/// These checks establish contract validity; model semantics still require evidence review.
pub fn validate_resolution(
    output: &ResolutionOutput,
    units: &[(String, Discussion)],
    topics: &Value,
) -> Result<(), &'static str> {
    if output.contract != RESOLVE_CONTRACT {
        return Err("resolution_contract_mismatch");
    }
    if output.decisions.len() != units.len() {
        return Err("resolution_unit_coverage_mismatch");
    }
    if units.len() > super::MAX_DISCUSSIONS {
        return Err("resolution_unit_count_exceeded");
    }
    let unit_ids: HashSet<_> = units.iter().map(|(id, _)| id.as_str()).collect();
    if unit_ids.len() != units.len() {
        return Err("resolution_unit_identity_duplicate");
    }
    if unit_ids.iter().any(|id| !text(id, 200)) {
        return Err("resolution_unit_identity_invalid");
    }
    let candidates = candidate_versions(topics)?;
    let mut seen_units = HashSet::new();
    for decision in &output.decisions {
        if !unit_ids.contains(decision.unit_id.as_str()) {
            return Err("resolution_unit_not_requested");
        }
        if !seen_units.insert(decision.unit_id.as_str()) {
            return Err("resolution_unit_decision_duplicate");
        }
        if !text(&decision.reason, 500) {
            return Err("resolution_decision_reason_invalid");
        }
        if decision.matches.len() > 4 {
            return Err("resolution_match_count_exceeded");
        }
        if decision.relations.len() > 64 {
            return Err("resolution_relation_count_exceeded");
        }
        let matches = validate_matches(decision, &candidates)?;
        let relations = validate_relations(decision, &candidates, &matches)?;
        validate_status(decision, units, &candidates, &matches, &relations)?;
        if let Some(child) = &decision.proposed_topic {
            validate_parent_candidate_reuse(child, topics)?;
        }
    }
    Ok(())
}

fn validate_matches(
    decision: &UnitDecision,
    candidates: &HashMap<Uuid, Uuid>,
) -> Result<HashSet<Uuid>, &'static str> {
    let mut matches = HashSet::new();
    for matched in &decision.matches {
        if !candidates.contains_key(&matched.topic_ref) {
            return Err("resolution_match_not_candidate");
        }
        if candidates.get(&matched.topic_ref) != Some(&matched.definition_ref) {
            return Err("resolution_match_version_mismatch");
        }
        if !matches.insert(matched.topic_ref) {
            return Err("resolution_match_duplicate");
        }
        if !text(&matched.reason, 500) {
            return Err("resolution_match_reason_invalid");
        }
    }
    Ok(matches)
}

fn validate_relations<'a>(
    decision: &'a UnitDecision,
    candidates: &HashMap<Uuid, Uuid>,
    matches: &HashSet<Uuid>,
) -> Result<HashMap<Uuid, &'a str>, &'static str> {
    let mut relations = HashMap::new();
    for relation in &decision.relations {
        if !candidates.contains_key(&relation.topic_ref) {
            return Err("resolution_relation_not_candidate");
        }
        if candidates.get(&relation.topic_ref) != Some(&relation.definition_ref) {
            return Err("resolution_relation_version_mismatch");
        }
        if !RELATIONS.contains(&relation.relation.as_str()) {
            return Err("resolution_relation_kind_invalid");
        }
        if !text(&relation.reason, 500) {
            return Err("resolution_relation_reason_invalid");
        }
        if relations
            .insert(relation.topic_ref, relation.relation.as_str())
            .is_some()
        {
            return Err("resolution_relation_duplicate");
        }
        if matches.contains(&relation.topic_ref)
            && ["distinct", "uncertain"].contains(&relation.relation.as_str())
        {
            return Err("resolution_match_relation_conflict");
        }
    }
    Ok(relations)
}

fn validate_status(
    decision: &UnitDecision,
    units: &[(String, Discussion)],
    candidates: &HashMap<Uuid, Uuid>,
    matches: &HashSet<Uuid>,
    relations: &HashMap<Uuid, &str>,
) -> Result<(), &'static str> {
    if relations.iter().any(|(topic, relation)| {
        *relation == "equivalent" && (decision.status != "matched" || !matches.contains(topic))
    }) {
        return Err("new_topic_equivalent_requires_match");
    }
    match decision.status.as_str() {
        "matched" => {
            if matches.is_empty() {
                return Err("resolution_matched_without_match");
            }
            if decision.proposed_topic.is_some() {
                return Err("resolution_matched_has_proposal");
            }
        }
        "new" => {
            if !matches.is_empty() {
                return Err("resolution_new_has_match");
            }
            let proposed = decision
                .proposed_topic
                .as_ref()
                .ok_or("missing_proposed_topic")?;
            if relations.len() != candidates.len() {
                return Err("new_topic_candidate_coverage_missing");
            }
            if relations.values().any(|r| *r == "equivalent") {
                return Err("new_topic_equivalent_requires_match");
            }
            validate_concept(
                &proposed.label,
                &proposed.definition,
                &proposed.inclusion_criteria,
                &proposed.exclusion_criteria,
                &proposed.domain_fit,
                &proposed.domain_reason,
                &proposed.abstraction_reason,
                false,
            )?;
            let statement = &units
                .iter()
                .find(|(id, _)| id == &decision.unit_id)
                .unwrap()
                .1
                .statement;
            if proposed.definition.trim() == statement.trim() {
                return Err("new_topic_definition_copies_statement");
            }
            validate_parent(proposed, candidates, relations)?;
        }
        "uncertain" | "out_of_scope" => {
            if !matches.is_empty() {
                return Err("resolution_unassigned_has_match");
            }
            if decision.proposed_topic.is_some() {
                return Err("resolution_unassigned_has_proposal");
            }
        }
        _ => return Err("resolution_status_unknown"),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_concept(
    label: &str,
    definition: &str,
    inclusion: &[String],
    exclusion: &[String],
    fit: &str,
    domain: &str,
    abstraction: &str,
    parent: bool,
) -> Result<(), &'static str> {
    for (valid, child_code, parent_code) in [
        (
            text(label, 120),
            "new_topic_label_invalid",
            "parent_topic_label_invalid",
        ),
        (
            text(definition, 1000),
            "new_topic_definition_invalid",
            "parent_topic_definition_invalid",
        ),
        (
            fit == "in_scope",
            "new_topic_domain_not_in_scope",
            "parent_topic_domain_not_in_scope",
        ),
        (
            text(domain, 500),
            "new_topic_domain_reason_invalid",
            "parent_topic_domain_reason_invalid",
        ),
        (
            text(abstraction, 500),
            "new_topic_abstraction_reason_invalid",
            "parent_topic_abstraction_reason_invalid",
        ),
    ] {
        if !valid {
            return Err(if parent { parent_code } else { child_code });
        }
    }
    validate_criteria(inclusion, exclusion, parent)
}

fn validate_criteria(
    inclusion: &[String],
    exclusion: &[String],
    parent: bool,
) -> Result<(), &'static str> {
    let mut seen = HashSet::new();
    for (items, side) in [(inclusion, true), (exclusion, false)] {
        let code = match (parent, side) {
            (false, true) => [
                "new_topic_inclusion_empty",
                "new_topic_inclusion_count_exceeded",
                "new_topic_inclusion_item_empty",
                "new_topic_inclusion_item_too_long",
            ],
            (false, false) => [
                "new_topic_exclusion_empty",
                "new_topic_exclusion_count_exceeded",
                "new_topic_exclusion_item_empty",
                "new_topic_exclusion_item_too_long",
            ],
            (true, true) => [
                "parent_topic_inclusion_empty",
                "parent_topic_inclusion_count_exceeded",
                "parent_topic_inclusion_item_empty",
                "parent_topic_inclusion_item_too_long",
            ],
            (true, false) => [
                "parent_topic_exclusion_empty",
                "parent_topic_exclusion_count_exceeded",
                "parent_topic_exclusion_item_empty",
                "parent_topic_exclusion_item_too_long",
            ],
        };
        if items.is_empty() {
            return Err(code[0]);
        }
        if items.len() > 8 {
            return Err(code[1]);
        }
        for item in items {
            if item.trim().is_empty() {
                return Err(code[2]);
            }
            if item.chars().count() > 300 {
                return Err(code[3]);
            }
            if !seen.insert(item.trim().to_lowercase()) {
                return Err(if parent {
                    "parent_topic_criteria_duplicate"
                } else {
                    "new_topic_criteria_duplicate"
                });
            }
        }
    }
    Ok(())
}

/// Identity reuse must not skip the existing-parent version and containment decision.
fn validate_parent_candidate_reuse(
    child: &ProposedTopic,
    topics: &Value,
) -> Result<(), &'static str> {
    if child.parent.kind != "proposed" {
        return Ok(());
    }
    let parent = child
        .parent
        .proposal
        .as_ref()
        .ok_or("topic_parent_proposal_missing")?;
    let proposed_identity = crate::topic_map_core::concept_identity(
        &parent.definition,
        &parent.inclusion_criteria,
        &parent.exclusion_criteria,
    );
    for candidate in topics.as_array().into_iter().flatten() {
        let Some(definition) = candidate["definition"].as_str() else {
            continue;
        };
        let criteria = |key: &str| {
            candidate[key]
                .as_array()?
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        };
        let (Some(included), Some(excluded)) =
            (criteria("inclusionCriteria"), criteria("exclusionCriteria"))
        else {
            continue;
        };
        if crate::topic_map_core::concept_identity(definition, &included, &excluded)
            == proposed_identity
        {
            return Err("topic_parent_proposal_matches_candidate");
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
        return Err("topic_parent_reason_invalid");
    }
    match parent.kind.as_str() {
        "root" => {
            if parent.topic_ref.is_some()
                || parent.definition_ref.is_some()
                || parent.proposal.is_some()
            {
                return Err("topic_parent_root_has_reference");
            }
        }
        "existing" => {
            let (topic, definition) = parent
                .topic_ref
                .zip(parent.definition_ref)
                .ok_or("topic_parent_existing_reference_missing")?;
            if parent.proposal.is_some() {
                return Err("topic_parent_existing_has_proposal");
            }
            if !candidates.contains_key(&topic) {
                return Err("topic_parent_not_candidate");
            }
            if candidates.get(&topic) != Some(&definition) {
                return Err("topic_parent_version_mismatch");
            }
            if relations.get(&topic) != Some(&"narrower") {
                return Err("topic_parent_containment_missing");
            }
        }
        "proposed" => {
            if parent.topic_ref.is_some() || parent.definition_ref.is_some() {
                return Err("topic_parent_proposed_has_reference");
            }
            let proposal = parent
                .proposal
                .as_ref()
                .ok_or("topic_parent_proposal_missing")?;
            validate_concept(
                &proposal.label,
                &proposal.definition,
                &proposal.inclusion_criteria,
                &proposal.exclusion_criteria,
                &proposal.domain_fit,
                &proposal.domain_reason,
                &proposal.abstraction_reason,
                true,
            )?;
            if proposal.label.trim() == child.label.trim() {
                return Err("topic_parent_same_label");
            }
            if crate::topic_map_core::concept_identity(
                &proposal.definition,
                &proposal.inclusion_criteria,
                &proposal.exclusion_criteria,
            ) == crate::topic_map_core::concept_identity(
                &child.definition,
                &child.inclusion_criteria,
                &child.exclusion_criteria,
            ) {
                return Err("topic_parent_same_concept");
            }
        }
        _ => return Err("topic_parent_kind_unknown"),
    }
    Ok(())
}
