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
    /// Single-pass only: the proposed parent's own parent, so a referenced chain is accepted whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ancestor: Option<Box<ProposedParent>>,
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
        let matches = validate_matches(decision, &candidates)?;
        let relations = validate_relations(decision, &candidates, &matches)?;
        validate_status(decision, units, &candidates, &matches, &relations, topics)?;
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
    topics: &Value,
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
            validate_parent_chain(proposed, candidates, relations, topics)?;
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

/// Normalized comparison is deterministic; it does not claim semantic containment.
fn normalized_criteria(items: &[String]) -> Vec<String> {
    let mut normalized: Vec<_> = items
        .iter()
        .map(|item| {
            item.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase()
        })
        .collect();
    normalized.sort();
    normalized
}

/// A proposed parent must not recreate a boundary the catalog already holds.
fn validate_candidate_identity_reuse(
    proposal: &ConceptDefinition,
    topics: &Value,
) -> Result<(), &'static str> {
    let proposed_identity = crate::topic_map_core::concept_identity(
        &proposal.definition,
        &proposal.inclusion_criteria,
        &proposal.exclusion_criteria,
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

/// Walks the whole declared parent chain: an ancestor referenced only through `ancestor`
/// must satisfy the same shape, identity, criteria, and candidate-version rules as a
/// directly assigned parent. The direct parent alone also needs the containment relation;
/// a deeper existing ancestor has no decision-level relation to require.
fn validate_parent_chain(
    child: &ProposedTopic,
    candidates: &HashMap<Uuid, Uuid>,
    relations: &HashMap<Uuid, &str>,
    topics: &Value,
) -> Result<(), &'static str> {
    let mut descendant_label = child.label.trim();
    let mut descendant_identity = crate::topic_map_core::concept_identity(
        &child.definition,
        &child.inclusion_criteria,
        &child.exclusion_criteria,
    );
    let mut descendant_inclusion = normalized_criteria(&child.inclusion_criteria);
    let mut descendant_exclusion = normalized_criteria(&child.exclusion_criteria);
    let mut seen_labels = HashSet::from([descendant_label]);
    let mut seen_identities = HashSet::from([descendant_identity.clone()]);
    let mut node = &child.parent;
    let mut direct = true;
    loop {
        if !text(&node.reason, 500) {
            return Err("topic_parent_reason_invalid");
        }
        match node.kind.as_str() {
            "root" => {
                if node.topic_ref.is_some()
                    || node.definition_ref.is_some()
                    || node.proposal.is_some()
                {
                    return Err("topic_parent_root_has_reference");
                }
                if node.ancestor.is_some() {
                    return Err("topic_parent_ancestor_unexpected");
                }
                return Ok(());
            }
            "existing" => {
                let (topic, definition) = node
                    .topic_ref
                    .zip(node.definition_ref)
                    .ok_or("topic_parent_existing_reference_missing")?;
                if node.proposal.is_some() {
                    return Err("topic_parent_existing_has_proposal");
                }
                if node.ancestor.is_some() {
                    return Err("topic_parent_ancestor_unexpected");
                }
                if !candidates.contains_key(&topic) {
                    return Err("topic_parent_not_candidate");
                }
                if candidates.get(&topic) != Some(&definition) {
                    return Err("topic_parent_version_mismatch");
                }
                if direct && relations.get(&topic) != Some(&"narrower") {
                    return Err("topic_parent_containment_missing");
                }
                return Ok(());
            }
            "proposed" => {
                if node.topic_ref.is_some() || node.definition_ref.is_some() {
                    return Err("topic_parent_proposed_has_reference");
                }
                let proposal = node
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
                if proposal.label.trim() == descendant_label {
                    return Err("topic_parent_same_label");
                }
                let identity = crate::topic_map_core::concept_identity(
                    &proposal.definition,
                    &proposal.inclusion_criteria,
                    &proposal.exclusion_criteria,
                );
                if identity == descendant_identity {
                    return Err("topic_parent_same_concept");
                }
                if !seen_labels.insert(proposal.label.trim())
                    || !seen_identities.insert(identity.clone())
                {
                    return Err("topic_parent_cycle");
                }
                let inclusion = normalized_criteria(&proposal.inclusion_criteria);
                let exclusion = normalized_criteria(&proposal.exclusion_criteria);
                if inclusion == descendant_inclusion && exclusion == descendant_exclusion {
                    return Err("topic_parent_criteria_repeated");
                }
                validate_candidate_identity_reuse(proposal, topics)?;
                descendant_label = proposal.label.trim();
                descendant_identity = identity;
                descendant_inclusion = inclusion;
                descendant_exclusion = exclusion;
                let Some(ancestor) = node.ancestor.as_deref() else {
                    return Ok(());
                };
                node = ancestor;
                direct = false;
            }
            _ => return Err("topic_parent_kind_unknown"),
        }
    }
}
