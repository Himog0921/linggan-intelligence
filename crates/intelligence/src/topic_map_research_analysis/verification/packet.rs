//! Program-selected review obligations; the proposer cannot choose what gets reviewed.
use super::super::{Citation, Discussion, ResearchOutput, ResolutionOutput};
use super::{
    MAX_VERIFY_CHECKS, VERIFY_CONTRACT,
    evidence::{EvidenceTable, outcome_context},
};
use linggan_evidence::creator_discovery::Fragment;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;

fn cited<T: Serialize>(
    value: &T,
    citations: &[Citation],
    evidence: &mut EvidenceTable<'_>,
) -> Result<Value, &'static str> {
    let mut value = serde_json::to_value(value).map_err(|_| "semantic_review_proposal_invalid")?;
    let object = value
        .as_object_mut()
        .ok_or("semantic_review_proposal_invalid")?;
    object.remove("evidence");
    object.insert("evidenceIds".into(), json!(evidence.citations(citations)?));
    Ok(value)
}

fn check(id: impl Into<String>, rule: &'static str, subject: impl Into<String>) -> Value {
    json!({"checkId":id.into(),"rule":rule,"subject":subject.into()})
}

fn bounded(checks: &[Value]) -> Result<(), &'static str> {
    if checks.is_empty() || checks.len() > MAX_VERIFY_CHECKS {
        Err("semantic_review_check_limit")
    } else {
        Ok(())
    }
}

pub fn extraction_review(
    output: &ResearchOutput,
    fragments: &[Fragment],
) -> Result<Value, &'static str> {
    let mut evidence = EvidenceTable::new(fragments)?;
    let mut checks = vec![check("o0", "semantic_outcome_unsupported", "outcome")];
    let mut discussions = Vec::new();
    for (index, d) in output.discussions.iter().enumerate() {
        discussions.push(cited(d, &d.evidence, &mut evidence)?);
        let subject = format!("discussions.{index}");
        checks.push(check(
            format!("d{index}.s"),
            "semantic_discussion_unsupported",
            &subject,
        ));
        checks.push(check(
            format!("d{index}.a"),
            "semantic_discussion_non_atomic",
            subject,
        ));
    }
    let mut scenes = Vec::new();
    for (index, scene) in output.scenes.iter().enumerate() {
        scenes.push(cited(scene, &scene.evidence, &mut evidence)?);
        checks.push(check(
            format!("s{index}"),
            "semantic_scene_unsupported",
            format!("scenes.{index}"),
        ));
    }
    let journey = cited(&output.journey, &output.journey.evidence, &mut evidence)?;
    checks.push(check("j0", "semantic_journey_unsupported", "journey"));
    let mut responses = Vec::new();
    for (index, response) in output.response_matches.iter().enumerate() {
        responses.push(cited(response, &response.evidence, &mut evidence)?);
        checks.push(check(
            format!("r{index}"),
            "semantic_response_unsupported",
            format!("responseMatches.{index}"),
        ));
    }
    let mut angles = Vec::new();
    for (index, angle) in output.angles.iter().enumerate() {
        angles.push(cited(angle, &angle.evidence, &mut evidence)?);
        checks.push(check(
            format!("a{index}"),
            "semantic_angle_unsupported",
            format!("angles.{index}"),
        ));
    }
    let mut opportunities = Vec::new();
    for (index, opportunity) in output.product_opportunities.iter().enumerate() {
        opportunities.push(cited(opportunity, &opportunity.evidence, &mut evidence)?);
        checks.push(check(
            format!("p{index}"),
            "semantic_opportunity_unsupported",
            format!("productOpportunities.{index}"),
        ));
    }
    bounded(&checks)?;
    Ok(
        json!({"contract":VERIFY_CONTRACT,"phase":"extraction","checks":checks,
        "evidence":evidence.entries,"proposal":{"outcome":output.outcome,"discussions":discussions,
            "scenes":scenes,"journey":journey,"responseMatches":responses,"angles":angles,
            "productOpportunities":opportunities,"limitations":output.limitations},
        "outcomeContext":if output.outcome!="analyzed" || output.discussions.is_empty() {
            outcome_context(fragments)
        } else {json!([])}}),
    )
}

fn candidate_table(candidates: &Value) -> Result<Value, &'static str> {
    let source = candidates
        .as_array()
        .ok_or("semantic_review_candidate_invalid")?;
    let mut ids = HashSet::new();
    let mut selected = Vec::new();
    for candidate in source {
        let topic = candidate["topicRef"]
            .as_str()
            .ok_or("semantic_review_candidate_invalid")?;
        let definition = candidate["definitionRef"]
            .as_str()
            .ok_or("semantic_review_candidate_invalid")?;
        if !ids.insert(topic) || definition.is_empty() || topic.is_empty() {
            return Err("semantic_review_candidate_invalid");
        }
        let mut row = serde_json::Map::new();
        for field in [
            "topicRef",
            "definitionRef",
            "label",
            "definition",
            "inclusionCriteria",
            "exclusionCriteria",
            "qualityState",
            "inductionMethod",
            "conceptRole",
            "parentTopicRef",
        ] {
            if let Some(value) = candidate.get(field) {
                row.insert(field.into(), value.clone());
            }
        }
        selected.push(Value::Object(row));
    }
    Ok(json!(selected))
}

pub fn resolution_review(
    units: &[(String, Discussion)],
    resolution: &ResolutionOutput,
    candidates: &Value,
    fragments: &[Fragment],
) -> Result<Value, &'static str> {
    let mut evidence = EvidenceTable::new(fragments)?;
    let mut unit_ids = HashSet::new();
    let mut selected_units = Vec::new();
    for (id, discussion) in units {
        if !unit_ids.insert(id) {
            return Err("semantic_review_proposal_invalid");
        }
        selected_units.push(
            json!({"unitId":id,"discussion":cited(discussion,&discussion.evidence,&mut evidence)?}),
        );
    }
    let mut checks = Vec::new();
    let mut seen = HashSet::new();
    for (index, decision) in resolution.decisions.iter().enumerate() {
        if !unit_ids.contains(&decision.unit_id) || !seen.insert(&decision.unit_id) {
            return Err("semantic_review_proposal_invalid");
        }
        let subject = format!("decisions.{index}");
        checks.push(check(
            format!("u{index}.b"),
            "semantic_assignment_boundary",
            &subject,
        ));
        checks.push(check(
            format!("u{index}.r"),
            "semantic_identity_relation",
            &subject,
        ));
        if decision.status == "new" {
            checks.push(check(
                format!("u{index}.c"),
                "semantic_concept_boundary",
                &subject,
            ));
            checks.push(check(
                format!("u{index}.p"),
                "semantic_parent_containment",
                subject,
            ));
        }
    }
    if seen.len() != units.len() {
        return Err("semantic_review_proposal_invalid");
    }
    bounded(&checks)?;
    Ok(
        json!({"contract":VERIFY_CONTRACT,"phase":"resolution","checks":checks,
        "evidence":evidence.entries,"units":selected_units,"decisions":resolution.decisions,
        "candidates":candidate_table(candidates)?}),
    )
}
