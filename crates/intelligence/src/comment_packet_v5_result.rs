//! Strict v5 atomic-result acceptance. This is separate from the frozen v4 parser so a v5
//! candidate can never reinterpret a historical v4 result.
use super::{
    ContextQuote, EvidenceBasis, Outcome, Quote, ResearchPacket, normalized_envelope,
    semantic_context_for_comment,
};
use crate::{
    comment_analysis::{CommentAnalysisOutput, CommentAnalysisSpan, validate_comment_analysis},
    comment_cleaning::{clean, outbound},
    comment_research::{ResearchBasis, ResearchDimension, ResearchFacet, comment_source_hash},
    comment_research_rules::AtomicKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct V5Envelope {
    comments: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct V5Item {
    comment_ref: String,
    outcome: Outcome,
    // Each raw atom is decoded independently. A malformed sibling must not discard a valid
    // atom that already has target-comment evidence.
    atoms: Vec<Value>,
    context_missing: Value,
    uncertainty_reason: Value,
    limitations: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct V5Atom {
    ordinal: u8,
    kind: AtomicKind,
    meaning: String,
    context: Option<String>,
    basis: EvidenceBasis,
    evidence: Vec<Quote>,
    context_evidence: Vec<ContextQuote>,
    target: Option<String>,
    position: Option<V5StancePosition>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum V5StancePosition {
    Support,
    Oppose,
    Concern,
    Mixed,
    Neutral,
}

pub(super) fn normalization_kind(raw: &str) -> Option<&'static str> {
    normalized_envelope(raw)
        .ok()
        .map(|normalized| normalized.kind)
}

pub(super) fn parse(
    packet: &ResearchPacket,
    raw: &str,
) -> Result<Vec<Result<Value, &'static str>>, &'static str> {
    let normalized = normalized_envelope(raw)?;
    let envelope: V5Envelope =
        serde_json::from_value(normalized.value).map_err(|_| "schema_invalid")?;
    let matching = validate_envelope(packet, &envelope)?;
    Ok((0..packet.inputs.len())
        .map(|index| match matching.get(&format!("C{:03}", index + 1)) {
            None => Err("missing_comment"),
            Some(raw) => serde_json::from_value::<V5Item>((*raw).clone())
                .map_err(|_| "item_schema_invalid")
                .and_then(|item| accept_item(packet, index, raw, &item))
                .and_then(accept_nonempty_item),
        })
        .collect())
}

fn accept_nonempty_item(value: Value) -> Result<Value, &'static str> {
    if value["semantic"]["acceptance"] == "rejected" {
        Err("all_fields_rejected")
    } else {
        Ok(value)
    }
}

pub(super) fn validation_diagnostics(packet: &ResearchPacket, raw: &str) -> Vec<Value> {
    let normalized = match normalized_envelope(raw) {
        Ok(value) => value,
        Err(code) => return vec![diagnostic(None, code, "$.comments", raw.len())],
    };
    let envelope: V5Envelope = match serde_json::from_value(normalized.value) {
        Ok(value) => value,
        Err(_) => return vec![diagnostic(None, "schema_invalid", "$.comments", 0)],
    };
    let matching = match validate_envelope(packet, &envelope) {
        Ok(value) => value,
        Err(code) => {
            return vec![diagnostic(
                None,
                code,
                "$.comments",
                envelope.comments.len(),
            )];
        }
    };
    (0..packet.inputs.len())
        .flat_map(|index| {
            let comment_ref = format!("C{:03}", index + 1);
            let raw = match matching.get(&comment_ref) {
                Some(value) => *value,
                None => {
                    return vec![diagnostic(
                        Some(&comment_ref),
                        "missing_comment",
                        "$.comments",
                        0,
                    )];
                }
            };
            let response_index = envelope
                .comments
                .iter()
                .position(|value| value["commentRef"].as_str() == Some(comment_ref.as_str()))
                .unwrap_or_default();
            match serde_json::from_value::<V5Item>(raw.clone()) {
                Ok(item) => diagnostics_for_item(packet, index, raw, &item, response_index),
                Err(_) => vec![diagnostic(
                    Some(&comment_ref),
                    "item_schema_invalid",
                    "$.comments[]",
                    raw.as_object().map_or(0, |value| value.len()),
                )],
            }
        })
        .collect()
}

fn diagnostics_for_item(
    packet: &ResearchPacket,
    index: usize,
    raw: &Value,
    item: &V5Item,
    response_index: usize,
) -> Vec<Value> {
    let comment_ref = format!("C{:03}", index + 1);
    match accept_item(packet, index, raw, item) {
        Ok(value) => value["semantic"]["rejectedFields"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|rejected| {
                diagnostic(
                    Some(&comment_ref),
                    rejected["code"].as_str().unwrap_or("field_schema_invalid"),
                    &format!(
                        "$.comments[{response_index}].{}",
                        rejected["path"].as_str().unwrap_or("atoms")
                    ),
                    item.atoms.len(),
                )
            })
            .collect(),
        Err(code) => vec![diagnostic(
            Some(&comment_ref),
            code,
            "$.comments[].atoms",
            item.atoms.len(),
        )],
    }
}

fn diagnostic(comment_ref: Option<&str>, code: &str, path: &str, count: usize) -> Value {
    json!({"commentRef":comment_ref,"code":code,"path":path,"expected":"comment-extraction.schema.v5 的固定字段、原子顺序和精确证据","actual":{"type":"bounded_summary","count":count}})
}

fn validate_envelope<'a>(
    packet: &ResearchPacket,
    envelope: &'a V5Envelope,
) -> Result<BTreeMap<String, &'a Value>, &'static str> {
    if envelope.comments.len() > 100 {
        return Err("output_bounds");
    }
    let expected = (0..packet.inputs.len())
        .map(|index| format!("C{:03}", index + 1))
        .collect::<Vec<_>>();
    let mut matching = BTreeMap::new();
    for raw in &envelope.comments {
        let reference = raw
            .get("commentRef")
            .and_then(Value::as_str)
            .ok_or("item_schema_invalid")?;
        if !expected.iter().any(|expected| expected == reference) {
            return Err("unknown_comment");
        }
        if matching.insert(reference.to_owned(), raw).is_some() {
            return Err("duplicate_comment");
        }
    }
    Ok(matching)
}

fn accept_item(
    packet: &ResearchPacket,
    index: usize,
    _raw: &Value,
    item: &V5Item,
) -> Result<Value, &'static str> {
    if item.comment_ref != format!("C{:03}", index + 1) {
        return Err("item_identity_mismatch");
    }
    validate_item_shape(item)?;
    let mut rejected = Vec::new();
    let context_missing =
        accepted_auxiliary_list(&item.context_missing, "contextMissing", &mut rejected);
    let limitations = accepted_auxiliary_list(&item.limitations, "limitations", &mut rejected);
    let uncertainty_reason = accepted_uncertainty_reason(&item.uncertainty_reason, &mut rejected);
    if item.outcome == Outcome::Uncertain && uncertainty_reason.is_none() {
        return Err("uncertainty_reason_missing");
    }
    let input = &packet.inputs[index];
    let mut spans = Vec::new();
    let mut accepted = Vec::new();
    let mut uncertain_atoms = Vec::new();
    for (atom_index, raw_atom) in item.atoms.iter().enumerate() {
        let checkpoint = spans.len();
        match accept_atom(packet, index, atom_index, raw_atom) {
            Ok((value, _atom_spans, basis)) if basis == EvidenceBasis::Uncertain => {
                if uncertainty_reason.is_none() {
                    rejected.push(atom_rejection(atom_index, "uncertainty_reason_missing"));
                } else {
                    uncertain_atoms.push(value);
                }
            }
            Ok((value, atom_spans, _)) => {
                spans.extend(atom_spans);
                accepted.push(value);
            }
            Err(code) => {
                spans.truncate(checkpoint);
                rejected.push(atom_rejection(atom_index, code));
            }
        }
    }
    let all_rejected =
        item.outcome == Outcome::Interpretable && accepted.is_empty() && uncertain_atoms.is_empty();
    let outcome = if item.outcome == Outcome::Interpretable
        && accepted.is_empty()
        && !uncertain_atoms.is_empty()
    {
        Outcome::Uncertain
    } else {
        item.outcome
    };
    let legacy = CommentAnalysisOutput {
        source_ref: input.source_ref,
        source_sha256: input.source_sha256.clone(),
        spans,
        limitations,
    };
    let mut result =
        validate_comment_analysis(input, &legacy).map_err(|_| "facets_or_evidence_invalid")?;
    result["schemaVersion"] = json!("comment-extraction.schema.v5");
    result["semantic"] = json!({"outcome":outcome,"atoms":accepted,"uncertainAtoms":uncertain_atoms,"contextMissing":context_missing,"uncertaintyReason":uncertainty_reason,"rejectedFields":rejected,"acceptance":if all_rejected {"rejected"} else if rejected.is_empty() {"complete"} else {"partial"}});
    result["candidateSnapshot"] = json!([]);
    result["contextFingerprint"] = json!(comment_source_hash(
        &semantic_context_for_comment(&input.context, &packet.cleaned[index].text).to_string()
    ));
    result["contextRefs"]["researchSourceRefs"] = json!(packet.context_refs);
    result["contextRefs"]["workRef"] = input
        .context
        .pointer("/workUrl")
        .and_then(Value::as_str)
        .and_then(|url| url.rsplit('/').next())
        .map_or(Value::Null, |reference| json!(reference));
    result["contextRefs"]["mediaJobs"] = json!(
        input.context["derivatives"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|value| value["state"] == "ACQUIRED" && value["displayText"].is_string())
            .map(|value| value["jobRef"].clone())
            .collect::<Vec<_>>()
    );
    Ok(result)
}

/// The field-repair runner never asks a provider to regenerate already accepted atoms.  It
/// accepts one explicitly addressed replacement atom under the same v5 evidence rules, while
/// retaining the original ordinal rather than treating the repair as a fresh one-item packet.
pub(crate) fn accept_repair_atom(
    packet: &ResearchPacket,
    index: usize,
    expected_ordinal: u8,
    raw: &Value,
) -> Result<Value, &'static str> {
    if !(1..=8).contains(&expected_ordinal) {
        return Err("repair_slot_invalid");
    }
    let (value, _, _) = accept_atom(packet, index, usize::from(expected_ordinal - 1), raw)?;
    Ok(value)
}

fn validate_item_shape(item: &V5Item) -> Result<(), &'static str> {
    if item.atoms.len() > 8
        || (item.outcome != Outcome::Interpretable && !item.atoms.is_empty())
        || (item.outcome == Outcome::Interpretable && item.atoms.is_empty())
    {
        return Err("outcome_conflict");
    }
    Ok(())
}

fn accepted_auxiliary_list(raw: &Value, field: &str, rejected: &mut Vec<Value>) -> Vec<String> {
    let Some(values) = raw.as_array() else {
        rejected.push(json!({"path":field,"code":"field_schema_invalid"}));
        return Vec::new();
    };
    if values.len() > 8 {
        rejected.push(json!({"path":field,"code":"auxiliary_items_bounds"}));
        return Vec::new();
    }
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| match value.as_str() {
            Some(value) if bounded(value, 200) => Some(value.to_owned()),
            Some(_) => {
                rejected.push(
                    json!({"path":format!("{field}[{index}]"),"code":"auxiliary_text_bounds"}),
                );
                None
            }
            None => {
                rejected.push(
                    json!({"path":format!("{field}[{index}]"),"code":"field_schema_invalid"}),
                );
                None
            }
        })
        .collect()
}

fn accepted_uncertainty_reason(raw: &Value, rejected: &mut Vec<Value>) -> Option<String> {
    match raw {
        Value::Null => None,
        Value::String(value) if bounded(value, 200) => Some(value.to_owned()),
        Value::String(_) => {
            rejected.push(json!({"path":"uncertaintyReason","code":"auxiliary_text_bounds"}));
            None
        }
        _ => {
            rejected.push(json!({"path":"uncertaintyReason","code":"field_schema_invalid"}));
            None
        }
    }
}

fn accept_atom(
    packet: &ResearchPacket,
    index: usize,
    atom_index: usize,
    raw: &Value,
) -> Result<(Value, Vec<CommentAnalysisSpan>, EvidenceBasis), &'static str> {
    let atom: V5Atom = serde_json::from_value(raw.clone()).map_err(|_| "field_schema_invalid")?;
    validate_atom_shape(&atom, atom_index)?;
    if !packet
        .rule_snapshot
        .field_definitions
        .iter()
        .any(|field| field.kind == atom.kind)
    {
        return Err("atom_kind_not_enabled");
    }
    let stance = stance_fields(raw, &atom)?;
    let (evidence, spans) = resolve_evidence(packet, index, &atom)?;
    let context_evidence = resolve_context_evidence(packet, index, &atom)?;
    let mut value = json!({"ordinal":atom.ordinal,"kind":atom.kind,"meaning":atom.meaning,"context":atom.context,"basis":atom.basis,"evidence":evidence,"contextEvidence":context_evidence});
    if let Some(stance) = stance {
        value["target"] = stance["target"].clone();
        value["position"] = stance["position"].clone();
    }
    Ok((value, spans, atom.basis))
}

fn validate_atom_shape(atom: &V5Atom, atom_index: usize) -> Result<(), &'static str> {
    if atom.ordinal as usize != atom_index + 1
        || !bounded(&atom.meaning, 200)
        || atom
            .context
            .as_deref()
            .is_some_and(|value| !bounded(value, 200))
    {
        return Err("atom_bounds");
    }
    Ok(())
}

fn stance_fields(raw: &Value, atom: &V5Atom) -> Result<Option<Value>, &'static str> {
    let fields_present = raw.get("target").is_some() || raw.get("position").is_some();
    if atom.kind != AtomicKind::Stance {
        return (!fields_present)
            .then_some(None)
            .ok_or("stance_fields_forbidden");
    }
    let target = atom.target.as_deref().ok_or("stance_target_required")?;
    if !bounded(target, 100) {
        return Err("stance_target_bounds");
    }
    let position = atom.position.ok_or("stance_position_required")?;
    Ok(Some(json!({"target":target,"position":position})))
}

fn atom_rejection(index: usize, code: &str) -> Value {
    let suffix = match code {
        "stance_target_required" | "stance_target_bounds" => "target",
        "stance_position_required" => "position",
        "stance_fields_forbidden" => "target",
        "evidence_bounds" | "quote_missing_or_ambiguous" => "evidence",
        "context_evidence_required_or_conflicting"
        | "context_fragment_unknown"
        | "context_quote_missing_or_ambiguous" => "contextEvidence",
        "atom_bounds" => "meaning",
        _ => "",
    };
    let path = if suffix.is_empty() {
        format!("atoms[{index}]")
    } else {
        format!("atoms[{index}].{suffix}")
    };
    json!({"path":path,"code":code})
}

fn resolve_evidence(
    packet: &ResearchPacket,
    index: usize,
    atom: &V5Atom,
) -> Result<(Vec<Value>, Vec<CommentAnalysisSpan>), &'static str> {
    if atom.evidence.is_empty() || atom.evidence.len() > 4 {
        return Err("evidence_bounds");
    }
    let input = &packet.inputs[index];
    let mut refs = Vec::new();
    let mut spans = Vec::new();
    for quote in &atom.evidence {
        let (start_char, end_char, exact) =
            packet.cleaned[index].resolve(&quote.quote, &input.body)?;
        refs.push(json!({"sourceRef":input.source_ref,"startChar":start_char,"endChar":end_char}));
        if atom.basis != EvidenceBasis::Uncertain {
            spans.push(CommentAnalysisSpan {
                source_ref: input.source_ref,
                start_char,
                end_char,
                quote: exact,
                facets: vec![ResearchFacet {
                    dimension: dimension(atom.kind),
                    label: atom.meaning.clone(),
                    basis: if atom.basis == EvidenceBasis::Explicit {
                        ResearchBasis::Explicit
                    } else {
                        ResearchBasis::Inferred
                    },
                }],
            });
        }
    }
    Ok((refs, spans))
}

fn resolve_context_evidence(
    packet: &ResearchPacket,
    index: usize,
    atom: &V5Atom,
) -> Result<Vec<Value>, &'static str> {
    if atom.context_evidence.len() > 4
        || (atom.basis == EvidenceBasis::Explicit && !atom.context_evidence.is_empty())
        || (atom.basis == EvidenceBasis::ContextResolved && atom.context_evidence.is_empty())
    {
        return Err("context_evidence_required_or_conflicting");
    }
    atom.context_evidence
        .iter()
        .map(|quote| resolve_context_quote(packet, index, quote))
        .collect()
}

fn resolve_context_quote(
    packet: &ResearchPacket,
    index: usize,
    quote: &ContextQuote,
) -> Result<Value, &'static str> {
    let fragments = super::context_selection::fragments(
        &packet.inputs[index].context,
        &packet.cleaned[index].text,
    );
    let fragment = fragments
        .as_array()
        .into_iter()
        .flatten()
        .find(|fragment| fragment["fragmentRef"] == quote.fragment_ref)
        .ok_or("context_fragment_unknown")?;
    let text = fragment["text"]
        .as_str()
        .ok_or("context_fragment_unknown")?;
    if quote.quote.is_empty()
        || quote.quote.contains('█')
        || text.match_indices(&quote.quote).count() != 1
    {
        return Err("context_quote_missing_or_ambiguous");
    }
    let source_path = fragment["sourcePath"]
        .as_str()
        .ok_or("context_fragment_unknown")?;
    let original = packet.inputs[index]
        .context
        .pointer(source_path)
        .and_then(Value::as_str)
        .ok_or("context_fragment_unknown")?;
    let cleaned = outbound(clean(original));
    let offset = text
        .find(&quote.quote)
        .ok_or("context_quote_missing_or_ambiguous")?;
    let start = fragment["cleanStartChar"]
        .as_u64()
        .ok_or("context_fragment_unknown")? as usize
        + text[..offset].chars().count();
    let end = start + quote.quote.chars().count();
    let start_char = cleaned
        .offsets
        .get(start)
        .ok_or("context_quote_missing_or_ambiguous")?
        .0;
    let end_char = cleaned
        .offsets
        .get(end - 1)
        .ok_or("context_quote_missing_or_ambiguous")?
        .1;
    Ok(
        json!({"fragmentRef":quote.fragment_ref,"kind":fragment["kind"],"sourcePath":source_path,"sourceRef":fragment["sourceRef"],"startChar":start_char,"endChar":end_char}),
    )
}

fn dimension(kind: AtomicKind) -> ResearchDimension {
    match kind {
        AtomicKind::Problem => ResearchDimension::Problem,
        AtomicKind::Need => ResearchDimension::Expectation,
        AtomicKind::Solution => ResearchDimension::TriedMethod,
        AtomicKind::Stance | AtomicKind::Quote => ResearchDimension::Expression,
        AtomicKind::Story => ResearchDimension::Scene,
        AtomicKind::Emotion => ResearchDimension::Emotion,
    }
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment_packet::synthetic_packet;
    use crate::comment_research_rules::{
        EditableFieldDefinition, RuleCreationSource, RulePurpose, RuleVersion, canonical_hash,
        program_contract,
    };
    use uuid::Uuid;

    fn v5_packet() -> ResearchPacket {
        let mut packet = synthetic_packet();
        let mut rule = packet.rule_snapshot.clone();
        rule.rule_revision_ref = Uuid::new_v4();
        rule.rule_version = RuleVersion::V5;
        rule.parent_rule_revision_ref = Some(rule.rule_revision_ref);
        rule.purpose = RulePurpose {
            title: "困难".into(),
            instruction: "提取评论者直接表达的困难。".into(),
        };
        rule.field_definitions = vec![EditableFieldDefinition {
            kind: AtomicKind::Problem,
            name: "困难".into(),
            definition: "评论者直接表达的困难。".into(),
        }];
        rule.examples = vec![];
        rule.creation_source = RuleCreationSource::UserCandidate;
        let contract = program_contract(RuleVersion::V5);
        rule.schema_version = contract.schema_version.into();
        rule.validator_version = contract.validator_version.into();
        rule.selector_version = contract.selector_version.into();
        rule.canonical_hash = canonical_hash(&rule);
        packet.inputs[0].rule_version = rule.rule_version.as_str().into();
        packet.rule_snapshot = rule;
        packet
    }

    fn item(packet: &ResearchPacket) -> Value {
        json!({"commentRef":"C001","outcome":"interpretable","atoms":[{"ordinal":1,"kind":"problem","meaning":"开始作业很费精力","context":null,"basis":"explicit","evidence":[{"quote":packet.cleaned[0].text}],"contextEvidence":[]}],"contextMissing":[],"uncertaintyReason":null,"limitations":[]})
    }

    fn packet_with_comments(count: usize) -> ResearchPacket {
        let mut packet = v5_packet();
        let cleaned = packet.cleaned[0].clone();
        packet.inputs = (0..count)
            .map(|_| {
                let mut input = crate::model_invocation::synthetic_input();
                input.rule_version = packet.rule_snapshot.rule_version.as_str().into();
                input
            })
            .collect();
        packet.cleaned = (0..count).map(|_| cleaned.clone()).collect();
        packet
    }

    fn item_for(packet: &ResearchPacket, index: usize) -> Value {
        let mut value = item(packet);
        value["commentRef"] = json!(format!("C{:03}", index + 1));
        value
    }

    #[test]
    fn v5_accepts_atomic_evidence_and_rejects_legacy_or_untrusted_keys() {
        let packet = v5_packet();
        let raw = json!({"comments":[item(&packet)]}).to_string();
        let result = parse(&packet, &raw).unwrap().remove(0).unwrap();
        assert_eq!(result["schemaVersion"], "comment-extraction.schema.v5");
        assert_eq!(result["semantic"]["atoms"][0]["kind"], "problem");
        let legacy = json!({"comments":[{"commentRef":"C001","outcome":"interpretable","labels":[],"problems":[],"stances":[],"contextMissing":[],"uncertaintyReason":null,"limitations":[]}]}).to_string();
        assert_eq!(
            parse(&packet, &legacy).unwrap(),
            vec![Err("item_schema_invalid")]
        );
        let mut unsafe_item = item(&packet);
        unsafe_item["atoms"][0]["rawProviderError"] = json!("secret");
        let diagnostics =
            validation_diagnostics(&packet, &json!({"comments":[unsafe_item]}).to_string());
        assert!(!json!(diagnostics).to_string().contains("secret"));
    }

    #[test]
    fn missing_two_v5_members_preserves_the_other_five() {
        let packet = packet_with_comments(7);
        let rows = (0..5)
            .map(|index| item_for(&packet, index))
            .collect::<Vec<_>>();
        let parsed = parse(&packet, &json!({"comments":rows}).to_string()).unwrap();
        assert_eq!(parsed.len(), 7);
        assert!(parsed.iter().take(5).all(Result::is_ok));
        assert_eq!(parsed[5], Err("missing_comment"));
        assert_eq!(parsed[6], Err("missing_comment"));
        let diagnostics = validation_diagnostics(&packet, &json!({"comments":rows}).to_string());
        assert_eq!(diagnostics.len(), 2);
        assert!(
            diagnostics
                .iter()
                .all(|value| value["code"] == "missing_comment")
        );
    }

    #[test]
    fn v5_matches_by_comment_ref_and_isolates_one_bad_atom() {
        let packet = packet_with_comments(3);
        let mut rows = (0..3)
            .map(|index| item_for(&packet, index))
            .collect::<Vec<_>>();
        rows.reverse();
        let parsed = parse(&packet, &json!({"comments":rows}).to_string()).unwrap();
        assert!(parsed.iter().all(Result::is_ok));
        let mut rows = (0..3)
            .map(|index| item_for(&packet, index))
            .collect::<Vec<_>>();
        rows[1]["atoms"][0]["kind"] = json!("invented_kind");
        let raw = json!({"comments":rows}).to_string();
        let parsed = parse(&packet, &raw).unwrap();
        assert!(parsed[0].is_ok());
        assert_eq!(parsed[1], Err("all_fields_rejected"));
        assert!(parsed[2].is_ok());
        let diagnostics = validation_diagnostics(&packet, &raw);
        assert_eq!(diagnostics[0]["code"], "field_schema_invalid");
        assert_eq!(diagnostics[0]["path"], "$.comments[1].atoms[0]");
    }

    #[test]
    fn v5_keeps_a_valid_atom_and_reports_its_bad_sibling_precisely() {
        let packet = v5_packet();
        let mut row = item(&packet);
        let mut bad = row["atoms"][0].clone();
        bad["ordinal"] = json!(2);
        bad["evidence"] = json!([{"quote":"不在目标评论中的文字"}]);
        row["atoms"].as_array_mut().unwrap().push(bad);
        let raw = json!({"comments":[row]}).to_string();
        let result = parse(&packet, &raw).unwrap().remove(0).unwrap();
        assert_eq!(result["semantic"]["atoms"].as_array().unwrap().len(), 1);
        assert_eq!(result["semantic"]["acceptance"], "partial");
        assert_eq!(
            result["semantic"]["rejectedFields"][0]["path"],
            "atoms[1].evidence"
        );
        let diagnostics = validation_diagnostics(&packet, &raw);
        assert_eq!(diagnostics[0]["path"], "$.comments[0].atoms[1].evidence");
        assert_eq!(diagnostics[0]["code"], "quote_missing_or_ambiguous");
    }

    #[test]
    fn v5_stance_requires_its_real_target_and_position_and_excludes_them_elsewhere() {
        let mut packet = v5_packet();
        packet
            .rule_snapshot
            .field_definitions
            .push(EditableFieldDefinition {
                kind: AtomicKind::Stance,
                name: "立场".into(),
                definition: "评论者对明确对象或命题的立场。".into(),
            });
        let mut stance = item(&packet);
        stance["atoms"][0]["kind"] = json!("stance");
        stance["atoms"][0]["target"] = json!("每天都要陪写");
        stance["atoms"][0]["position"] = json!("concern");
        let accepted = parse(&packet, &json!({"comments":[stance]}).to_string())
            .unwrap()
            .remove(0)
            .unwrap();
        assert_eq!(accepted["semantic"]["atoms"][0]["target"], "每天都要陪写");
        assert_eq!(accepted["semantic"]["atoms"][0]["position"], "concern");

        let mut non_stance = item(&packet);
        non_stance["atoms"][0]["target"] = json!("不应出现在problem");
        non_stance["atoms"][0]["position"] = json!("neutral");
        assert_eq!(
            parse(&packet, &json!({"comments":[non_stance]}).to_string()).unwrap()[0],
            Err("all_fields_rejected")
        );
    }
}
