//! Exact business-field and source rules. Failures never edit the proposal.
use super::{
    Angle, Citation, Discussion, Fragment, Journey, ProductOpportunity, ResponseMatch, STAGES,
    Uuid, citations, criteria, discussion_roles, fragment_role, text,
};

pub(super) fn citation_rule(
    c: &[Citation],
    fragments: &[Fragment],
    required: bool,
) -> Result<(), &'static str> {
    if required && c.is_empty() {
        return Err("citation_missing");
    }
    if c.len() > 12 {
        return Err("citation_count_exceeded");
    }
    for r in c {
        let f = fragments
            .iter()
            .find(|f| f.fragment_id == r.fragment_id)
            .ok_or("citation_fragment_not_found")?;
        if r.start < f.start || r.end <= r.start || r.end > f.end {
            return Err("citation_range_invalid");
        }
        if r.end - r.start > 1000 {
            return Err("citation_length_exceeded");
        }
    }
    Ok(())
}
pub(super) fn validate_discussion(
    d: &Discussion,
    fragments: &[Fragment],
    topic_refs: &[Uuid],
    modern: bool,
) -> Result<(), &'static str> {
    if !text(&d.label, 120) {
        return Err("discussion_label_invalid");
    }
    if d.topic_ref.is_some_and(|r| !topic_refs.contains(&r)) {
        return Err("discussion_topic_not_allowed");
    }
    citation_rule(&d.evidence, fragments, true)?;
    if !modern {
        return Ok(());
    }
    if d.topic_ref.is_some() {
        return Err("discussion_topic_not_allowed");
    }
    if !text(&d.statement, 1000) {
        return Err("discussion_statement_invalid");
    }
    if !text(&d.definition, 1000) {
        return Err("discussion_definition_invalid");
    }
    for (items, code) in [
        (&d.inclusion_criteria, "discussion_inclusion_invalid"),
        (&d.exclusion_criteria, "discussion_exclusion_invalid"),
    ] {
        if items.is_empty() || items.len() > 8 || items.iter().any(|v| !text(v, 300)) {
            return Err(code);
        }
    }
    if !criteria(&d.inclusion_criteria, &d.exclusion_criteria) {
        return Err("discussion_criteria_duplicate");
    }
    if !text(&d.rationale, 500) {
        return Err("discussion_rationale_invalid");
    }
    if !["author", "commenter", "quoted", "unknown"].contains(&d.speaker_role.as_str()) {
        return Err("discussion_speaker_role_invalid");
    }
    if !["support", "challenge", "context"].contains(&d.evidence_role.as_str()) {
        return Err("discussion_evidence_role_invalid");
    }
    if !discussion_roles(d, fragments) {
        return Err("discussion_source_role_mismatch");
    }
    Ok(())
}
pub(super) fn validate_journey(j: &Journey, fragments: &[Fragment]) -> Result<(), &'static str> {
    if !STAGES.contains(&j.main_stage.as_str()) {
        return Err("journey_stage_invalid");
    }
    if !["family", "adult", "both", "unknown"].contains(&j.path.as_str()) {
        return Err("journey_path_invalid");
    }
    if j.involved_stages.len() > 5 {
        return Err("journey_involved_stage_count");
    }
    if j.involved_stages
        .iter()
        .any(|s| !STAGES[..5].contains(&s.as_str()))
    {
        return Err("journey_involved_stage_invalid");
    }
    if j.overlays.len() > 2 {
        return Err("journey_overlay_count");
    }
    if j.overlays
        .iter()
        .any(|s| !["obstruction_recurrence", "transition_handoff"].contains(&s.as_str()))
    {
        return Err("journey_overlay_invalid");
    }
    if !text(&j.rationale, 500) {
        return Err("journey_rationale_invalid");
    }
    if j.main_stage != "unclear" && j.evidence.is_empty() {
        return Err("journey_evidence_missing");
    }
    if !citations(&j.evidence, fragments, j.main_stage != "unclear") {
        return Err("journey_evidence_invalid");
    }
    Ok(())
}

pub(super) fn validate_response(
    response: &ResponseMatch,
    fragments: &[Fragment],
    modern: bool,
) -> Result<(), &'static str> {
    if ![
        "direct",
        "partial",
        "not_applicable",
        "unmatched",
        "unknown",
    ]
    .contains(&response.status.as_str())
    {
        return Err("response_status_invalid");
    }
    if response.unanswered.len() > 8 {
        return Err("response_unanswered_count_exceeded");
    }
    if response.unanswered.iter().any(|value| !text(value, 300)) {
        return Err("response_unanswered_text_invalid");
    }
    citation_rule(&response.evidence, fragments, response.status != "unknown")?;
    if modern && ["direct", "partial", "unmatched"].contains(&response.status.as_str()) {
        for (role, code) in [
            ("author", "response_author_evidence_missing"),
            ("commenter", "response_commenter_evidence_missing"),
        ] {
            if !response.evidence.iter().any(|citation| {
                fragments.iter().any(|fragment| {
                    fragment.fragment_id == citation.fragment_id && fragment_role(fragment) == role
                })
            }) {
                return Err(code);
            }
        }
    }
    Ok(())
}

pub(super) fn validate_angle(angle: &Angle, fragments: &[Fragment]) -> Result<(), &'static str> {
    for (value, max, code) in [
        (&angle.label, 120, "angle_label_invalid"),
        (&angle.title, 120, "angle_title_invalid"),
        (&angle.answer_task, 500, "angle_answer_task_invalid"),
    ] {
        if !text(value, max) {
            return Err(code);
        }
    }
    citation_rule(&angle.evidence, fragments, true)
}

pub(super) fn validate_opportunity(
    opportunity: &ProductOpportunity,
    fragments: &[Fragment],
) -> Result<(), &'static str> {
    for (value, code) in [
        (&opportunity.need, "opportunity_need_invalid"),
        (&opportunity.hypothesis, "opportunity_hypothesis_invalid"),
        (
            &opportunity.verification_question,
            "opportunity_verification_question_invalid",
        ),
        (
            &opportunity.alternative_explanation,
            "opportunity_alternative_explanation_invalid",
        ),
    ] {
        if !text(value, 500) {
            return Err(code);
        }
    }
    citation_rule(&opportunity.evidence, fragments, true)
}
