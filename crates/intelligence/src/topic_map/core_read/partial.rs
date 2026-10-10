//! An accepted resolution exposes its own frozen discussion, never the remaining draft.
use super::*;
use crate::topic_map_research_analysis::{Discussion, ResearchOutput};

fn matches_source(unit: &Value, id: &str, source: &Discussion) -> bool {
    let unit_ref = id.get(..32).and_then(|s| Uuid::parse_str(s).ok());
    unit_ref.is_some()
        && unit["unitRef"] == json!(unit_ref)
        && unit["resolutionRef"]
            .as_str()
            .is_some_and(|s| Uuid::parse_str(s).is_ok())
        && unit["label"] == source.label
        && unit["statement"] == source.statement
        && unit["speakerRole"] == source.speaker_role
        && unit["evidenceRole"] == source.evidence_role
        && unit["rationale"] == source.rationale
        && unit["evidence"] == json!(source.evidence)
        && valid_membership(unit)
        && unit["relations"].is_array()
        && unit["reason"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
}

fn valid_membership(unit: &Value) -> bool {
    let Some(assignments) = unit["assignments"].as_array() else {
        return false;
    };
    let cardinality = match unit["status"].as_str() {
        Some("matched") => (1..=4).contains(&assignments.len()),
        Some("new") => assignments.len() == 1,
        Some("uncertain" | "out_of_scope") => assignments.is_empty(),
        _ => false,
    };
    cardinality
        && assignments.iter().all(|a| {
            ["topicRef", "definitionRef"]
                .iter()
                .all(|key| a[*key].as_str().is_some_and(|s| Uuid::parse_str(s).is_ok()))
        })
        && unit["comparedDefinitionRefs"].as_array().is_some_and(|ds| {
            ds.iter()
                .all(|d| d.as_str().is_some_and(|s| Uuid::parse_str(s).is_ok()))
        })
}

pub(super) fn accepted_units(
    draft: &ResearchOutput,
    input: &ResearchInput,
    accepted: &Value,
) -> Vec<(Discussion, Value)> {
    let sources = crate::topic_map_core::units(input, draft);
    let mut seen = HashSet::new();
    accepted
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|unit| {
            let id = unit["unitId"].as_str()?;
            let (_, source) = sources.iter().find(|(key, _)| key == id)?;
            (matches_source(unit, id, source) && seen.insert(id.to_owned()))
                .then(|| (source.clone(), unit.clone()))
        })
        .collect()
}

pub(super) fn pending_journey() -> Value {
    json!({"mainStage":"unclear","involvedStages":[],"overlays":[],"path":"unknown",
        "rationale":"当前已接纳讨论不足以判定作者旅程。","evidence":[]})
}

pub(super) fn payload(
    draft: &ResearchOutput,
    input: &ResearchInput,
    accepted: &Value,
) -> Option<(Value, Value)> {
    let units = accepted_units(draft, input, accepted);
    if units.is_empty() {
        return None;
    }
    let output = json!({"contract":draft.contract,"outcome":"analyzed",
        "discussions":units.iter().map(|(d,_)|d).collect::<Vec<_>>(),
        "journey":pending_journey(),"scenes":[],"responseMatches":[],"angles":[],"productOpportunities":[],
        "limitations":["当前仅显示已完成归属判断的讨论；本窗口其余讨论和综合研究仍未完成。"]});
    let core = json!({"units":units.into_iter().map(|(_,u)|u).collect::<Vec<_>>()});
    Some((output, core))
}
