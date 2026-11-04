use super::*;
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn text(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 12000
}
fn refs(v: &[String], set: &BTreeSet<&str>, required: bool) -> bool {
    (!required || !v.is_empty()) && v.iter().all(|x| set.contains(x.as_str()))
}
/// Resolve only unambiguous exact quotes; the provider never supplies source identities.
pub fn accept(mut wire: Value, packet: &Packet) -> Result<Output, &'static str> {
    let raw = wire
        .get_mut("evidence")
        .and_then(Value::as_array_mut)
        .ok_or("evidence_missing")?;
    for e in raw {
        let obj = e.as_object().ok_or("evidence_invalid")?;
        if obj.len() != 3 {
            return Err("evidence_fields");
        }
        let id = e["id"].as_str().ok_or("evidence_id")?;
        let quote = e["quote"]
            .as_str()
            .filter(|s| text(s))
            .ok_or("quote_empty")?;
        let source = packet
            .sources
            .iter()
            .find(|s| Some(s.citation_id.as_str()) == e["inputCitationId"].as_str())
            .ok_or("citation_unknown")?;
        let mut matches = source.text.match_indices(quote);
        let (byte, _) = matches.next().ok_or("quote_not_found")?;
        if matches.next().is_some() {
            return Err("quote_ambiguous");
        }
        let start = source.start + source.text[..byte].chars().count();
        *e = json!(Evidence {
            id: id.into(),
            kind: source.kind.clone(),
            source_ref: source.source_ref,
            source_version: source.source_version.clone(),
            citation_id: source.citation_id.clone(),
            start,
            end: start + quote.chars().count(),
            quote: quote.into()
        });
    }
    let output: Output = serde_json::from_value(wire).map_err(|_| "output_shape")?;
    validate(&output, packet)?;
    Ok(output)
}
pub fn validate(o: &Output, p: &Packet) -> Result<(), &'static str> {
    if o.contract != CONTRACT || !text(&o.summary) {
        return Err("output_contract");
    }
    let mut evidence = BTreeSet::new();
    for e in &o.evidence {
        if !text(&e.id) || !evidence.insert(e.id.as_str()) {
            return Err("duplicate_evidence");
        }
        let s = p
            .sources
            .iter()
            .find(|s| s.citation_id == e.citation_id)
            .ok_or("citation_unknown")?;
        if e.source_ref != s.source_ref
            || e.source_version != s.source_version
            || e.kind != s.kind
            || e.start < s.start
            || e.end <= e.start
            || e.end > s.start + s.text.chars().count()
            || !text(&e.quote)
            || s.text
                .chars()
                .skip(e.start - s.start)
                .take(e.end - e.start)
                .collect::<String>()
                != e.quote
        {
            return Err("evidence_mismatch");
        }
    }
    let mut ids = BTreeSet::new();
    let mut insert = |id: &str, t: &str, r: &[String]| {
        if !text(id) || !text(t) || !refs(r, &evidence, true) {
            return false;
        }
        ids.insert(id.to_owned())
    };
    for s in o
        .understanding
        .statements()
        .into_iter()
        .chain(o.feedback.iter())
    {
        if !insert(&s.id, &s.text, &s.evidence_refs) {
            return Err("statement_invalid");
        }
    }
    for f in &o.features {
        if !insert(&f.id, &f.text, &f.evidence_refs) || !text(&f.label) {
            return Err("feature_invalid");
        }
    }
    for x in &o.explanations {
        if !insert(&x.id, &x.text, &x.evidence_refs) || !text(&x.support) {
            return Err("explanation_invalid");
        }
    }
    let ids: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let base: BTreeSet<&str> = o
        .understanding
        .statements()
        .into_iter()
        .map(|s| s.id.as_str())
        .chain(o.features.iter().map(|f| f.id.as_str()))
        .chain(o.feedback.iter().map(|s| s.id.as_str()))
        .collect();
    let features = o.features.iter().map(|s| s.id.as_str()).collect();
    let feedback = o.feedback.iter().map(|s| s.id.as_str()).collect();
    if !refs(&o.summary_basis_ids, &base, true) {
        return Err("summary_basis");
    }
    for x in &o.explanations {
        if !refs(&x.feature_ids, &features, true) || !refs(&x.feedback_ids, &feedback, false) {
            return Err("explanation_basis");
        }
    }
    for t in &o.tags {
        if !text(&t.label) || !text(&t.retrieval_use) || !refs(&t.judgment_ids, &base, true) {
            return Err("tag_basis");
        }
    }
    let mut methods = BTreeSet::new();
    for m in &o.method_candidates {
        if !text(&m.id)
            || ids.contains(m.id.as_str())
            || !methods.insert(&m.id)
            || !text(&m.text)
            || !text(&m.conditions)
            || !refs(&m.basis_ids, &ids, true)
        {
            return Err("method_basis");
        }
    }
    if p.scope == Scope::Note && !o.feedback.is_empty() {
        return Err("feedback_without_comments");
    }
    for f in &o.feedback {
        if !f.evidence_refs.iter().any(|id| {
            o.evidence
                .iter()
                .any(|e| e.id == *id && e.kind == "comment")
        }) {
            return Err("feedback_without_comment_evidence");
        }
    }
    Ok(())
}
/// Deterministic projection only; consumers remain separate and keep original evidence.
pub fn context(o: &Output, analysis: Uuid, material: &str, purpose: &str) -> Option<Value> {
    if !["topic", "comment"].contains(&purpose) {
        return None;
    }
    let is_material = |rs: &[String]| {
        !rs.is_empty()
            && rs.iter().all(|id| {
                o.evidence
                    .iter()
                    .any(|e| e.id == *id && e.kind == "material")
            })
    };
    if !is_material(&o.understanding.discussion.evidence_refs)
        || !is_material(&o.understanding.answer.evidence_refs)
    {
        return None;
    }
    let mut u = serde_json::to_value(&o.understanding).ok()?;
    for (key, statement) in [
        ("audience", &o.understanding.audience),
        ("situation", &o.understanding.situation),
        ("offeredValue", &o.understanding.offered_value),
    ] {
        if statement
            .as_ref()
            .is_some_and(|s| !is_material(&s.evidence_refs))
        {
            u[key] = Value::Null;
        }
    }
    if purpose == "comment" {
        u.as_object_mut()?.remove("audience");
        u.as_object_mut()?.remove("offeredValue");
    }
    let f: Vec<_> = o
        .features
        .iter()
        .filter(|f| {
            purpose == "comment"
                && f.category == FeatureCategory::Participation
                && is_material(&f.evidence_refs)
        })
        .collect();
    let mut used = BTreeSet::new();
    for s in u.as_object()?.values() {
        for id in s["evidenceRefs"].as_array().into_iter().flatten() {
            used.insert(id.as_str()?);
        }
    }
    for x in &f {
        used.extend(x.evidence_refs.iter().map(String::as_str));
    }
    Some(
        json!({"contract":"note-analysis.context.v1","analysisRef":analysis,"materialSnapshotIdentity":material,"purpose":purpose,"understanding":u,"interactionFeatures":f,"evidence":o.evidence.iter().filter(|e|used.contains(e.id.as_str())).collect::<Vec<_>>()}),
    )
}
