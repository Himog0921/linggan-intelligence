//! Reading metadata shares known Citation restrictions, without becoming a writing placement.
use super::*;
use std::collections::HashMap;

fn visit_mut(doc: &mut WorkDocument, mut visit: impl FnMut(String, &mut String)) {
    if let Some(name) = &mut doc.idea_name {
        visit("idea.name".into(), name);
    }
    for (index, text) in doc.cover_title_candidates.iter_mut().enumerate() {
        visit(format!("cover-candidate.{index}"), text);
    }
    for (index, text) in doc.note_title_candidates.iter_mut().enumerate() {
        visit(format!("note-candidate.{index}"), text);
    }
    if let Some(preparation) = &mut doc.material_preparation {
        visit("preparation.query".into(), &mut preparation.query);
        for (index, term) in preparation.terms.iter_mut().enumerate() {
            visit(format!("preparation.term.{index}"), term);
        }
        for (index, gap) in preparation.gaps.iter_mut().enumerate() {
            visit(format!("preparation.gap.{index}"), gap);
        }
    }
    for material in &mut doc.materials {
        for (index, reason) in material.match_reasons.iter_mut().enumerate() {
            visit(
                format!("material.{}.reason.{index}", material.document_id),
                reason,
            );
        }
    }
}
fn values(doc: &WorkDocument) -> Vec<(String, String)> {
    let mut values = Vec::new();
    if let Some(name) = &doc.idea_name {
        values.push(("idea.name".into(), name.clone()));
    }
    values.extend(
        doc.cover_title_candidates
            .iter()
            .enumerate()
            .map(|(index, text)| (format!("cover-candidate.{index}"), text.clone())),
    );
    values.extend(
        doc.note_title_candidates
            .iter()
            .enumerate()
            .map(|(index, text)| (format!("note-candidate.{index}"), text.clone())),
    );
    if let Some(preparation) = &doc.material_preparation {
        values.push(("preparation.query".into(), preparation.query.clone()));
        values.extend(
            preparation
                .terms
                .iter()
                .enumerate()
                .map(|(index, text)| (format!("preparation.term.{index}"), text.clone())),
        );
        values.extend(
            preparation
                .gaps
                .iter()
                .enumerate()
                .map(|(index, text)| (format!("preparation.gap.{index}"), text.clone())),
        );
    }
    for material in &doc.materials {
        values.extend(
            material
                .match_reasons
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    (
                        format!("material.{}.reason.{index}", material.document_id),
                        text.clone(),
                    )
                }),
        );
    }
    values
}
pub(super) fn contains_quote(doc: &WorkDocument, raw: &str) -> bool {
    values(doc).iter().any(|(_, text)| raw_present(text, raw))
}
pub(super) fn adds_quote(doc: &WorkDocument, old: Option<&WorkDocument>, raw: &str) -> bool {
    let old: HashMap<_, _> = old.map(values).unwrap_or_default().into_iter().collect();
    values(doc).iter().any(|(key, text)| {
        let before = old.get(key).map_or(0, |text| exact_spans(text, raw).len());
        exact_spans(text, raw).len() > before
    })
}
pub(super) fn redact(doc: &mut WorkDocument, quotes: &[String]) {
    visit_mut(doc, |_, text| {
        let mut spans: Vec<_> = quotes
            .iter()
            .flat_map(|raw| exact_spans(text, raw))
            .map(|span| (span.start, span.end))
            .collect();
        spans.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in spans {
            if let Some(previous) = merged.last_mut().filter(|previous| start <= previous.1) {
                previous.1 = previous.1.max(end)
            } else {
                merged.push((start, end));
            }
        }
        for (start, end) in merged.into_iter().rev() {
            let prefix: String = text.chars().take(start).collect();
            let suffix: String = text.chars().skip(end).collect();
            // One scalar cannot grow a nonempty Citation range beyond the field limit.
            *text = format!("{prefix}…{suffix}");
        }
    });
    if let Some(preparation) = &mut doc.material_preparation {
        // A wholly hidden short term is no longer a usable query. Preserve every other term;
        // projection does not grant clients an exemption from ordinary field validation.
        preparation.terms.retain(|term| term.chars().count() >= 2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_reason_and_preparation_hide_only_known_quote_preserving_independent_words() {
        let quote = "原声😀第一步".to_owned();
        let mut doc = WorkDocument::default();
        doc.idea_name = Some(format!("选题{quote}后记"));
        doc.note_title_candidates.push(format!("标题{quote}后记"));
        doc.material_preparation = Some(MaterialPreparation {
            method_version: "workbench.lexical-context.v1".into(),
            query: format!("作者前言{quote}作者后记"),
            terms: vec!["作业".into(), quote.clone()],
            state: "ready".into(),
            gaps: vec![format!("说明{quote}，仍须核对")],
        });
        doc.materials.push(MaterialSelection {
            document_id: "legacy".into(),
            source_kind: SourceKind::Comment,
            source_ref: Uuid::new_v4(),
            work_ref: None,
            source_version: "a".repeat(64),
            added_by: "prepared".into(),
            match_reasons: vec![format!("旧理由{quote}；正常说明")],
        });
        assert!(contains_quote(&doc, &quote));
        assert!(adds_quote(&doc, None, &quote));
        assert!(!adds_quote(&doc, Some(&doc), &quote));
        redact(&mut doc, &[quote.clone()]);
        assert!(!contains_quote(&doc, &quote));
        assert_eq!(doc.idea_name.as_deref(), Some("选题…后记"));
        assert_eq!(doc.note_title_candidates[0], "标题…后记");
        let preparation = doc.material_preparation.unwrap();
        assert_eq!(preparation.terms[0], "作业");
        assert_eq!(preparation.query, "作者前言…作者后记");
        assert_eq!(doc.materials[0].match_reasons[0], "旧理由…；正常说明");
    }
}
