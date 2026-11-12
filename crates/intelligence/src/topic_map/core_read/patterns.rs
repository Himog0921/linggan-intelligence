//! Author-owned expression metadata from the same qualified Core as the visible windows.
use super::*;

pub(super) fn project(parts: &[Window]) -> Value {
    let Some(latest) = parts.iter().find(|part| {
        part.current_coverage_eligible
            && !part.is_comparison()
            && part.input.coverage["coreRef"].is_string()
    }) else {
        return Value::Null;
    };
    let reference = &latest.input.coverage["coreRef"];
    let core = &latest.input.coverage["core"];
    let Some(units) = core["knowledgeUnits"].as_array() else {
        return Value::Null;
    };
    let fragments: Vec<_> = parts
        .iter()
        .filter(|part| {
            part.current_coverage_eligible
                && !part.is_comparison()
                && part.input.coverage["coreRef"] == *reference
        })
        .flat_map(|part| &part.input.fragments)
        .collect();
    let item = |kind: &str, label: &str, ids: &[Value]| -> Option<Value> {
        if label.trim().is_empty() || ids.is_empty() {
            return None;
        }
        let mut evidence = Vec::new();
        for id in ids {
            let unit = units
                .iter()
                .find(|unit| unit["id"] == *id && unit["speakerRole"] == "author")?;
            let refs = unit["evidenceRefs"].as_array()?;
            if refs.is_empty() {
                return None;
            }
            for evidence_ref in refs {
                let matching: Vec<_> = fragments
                    .iter()
                    .filter(|fragment| fragment.structure["anchor"]["id"] == *evidence_ref)
                    .collect();
                if matching.is_empty() {
                    return None;
                }
                for fragment in matching {
                    let citation = json!({"fragmentId":fragment.fragment_id,"start":fragment.start,"end":fragment.end});
                    if !evidence.contains(&citation) {
                        evidence.push(citation);
                    }
                }
            }
        }
        Some(json!({"kind":kind,"label":label,"unitIds":ids,"evidence":evidence}))
    };
    let mut items = Vec::new();
    if let Some(kind) = core["contentType"].as_str().filter(|kind| *kind != "other")
        && let Some(value) = item("content_type", kind, &[core["mainTopicUnitId"].clone()])
    {
        items.push(value);
    }
    let mut labelled = HashSet::new();
    for label in core["candidateLabels"].as_array().into_iter().flatten() {
        let Some(kind @ ("expression" | "answer_form")) = label["dimension"].as_str() else {
            continue;
        };
        let Some(ids) = label["unitIds"].as_array() else {
            continue;
        };
        if let Some(value) = label["label"]
            .as_str()
            .and_then(|text| item(kind, text, ids))
        {
            for id in ids {
                labelled.insert((kind.to_owned(), id.to_string()));
            }
            if !items.contains(&value) {
                items.push(value);
            }
        }
    }
    for unit in units {
        let Some(kind @ ("expression" | "answer_form")) = unit["unitType"].as_str() else {
            continue;
        };
        if labelled.contains(&(kind.to_owned(), unit["id"].to_string())) {
            continue;
        }
        if let Some(value) = unit["statement"]
            .as_str()
            .and_then(|text| item(kind, text, &[unit["id"].clone()]))
        {
            items.push(value);
        }
    }
    json!({"coreRef":reference,"items":items})
}

#[cfg(test)]
mod tests {
    use super::super::tests::{input, window};
    use super::*;
    fn part(reference: u128, speaker: &str) -> Window {
        let mut source = input("core_text_evidence");
        source.coverage["coreRef"] = json!(Uuid::from_u128(reference));
        source.coverage["core"] = json!({"contentType":"personal_story","mainTopicUnitId":"u1",
            "knowledgeUnits":[{"id":"u1","unitType":"expression","statement":"亲历叙述","speakerRole":speaker,"evidenceRefs":["e1"]}],
            "candidateLabels":[{"dimension":"expression","label":"第一人称回顾","unitIds":["u1"]}]});
        source.fragments[0].structure = json!({"anchor":{"id":"e1"}});
        window(source, json!({}), json!({}), Some(reference), "2026-11-11")
    }
    #[test]
    fn keeps_core_unit_and_exact_source_identity_without_duplicate_labelled_units() {
        let part = part(12, "author");
        let actual = project(&[part.clone(), part.clone()]);
        assert_eq!(actual["coreRef"], json!(Uuid::from_u128(12)));
        assert_eq!(actual["items"].as_array().unwrap().len(), 2);
        assert_eq!(actual["items"][1]["label"], "第一人称回顾");
        assert_eq!(actual["items"][1]["unitIds"], json!(["u1"]));
        assert_eq!(actual["items"][1]["evidence"].as_array().unwrap().len(), 1);
        assert_eq!(
            actual["items"][1]["evidence"][0]["start"],
            part.input.fragments[0].start
        );
    }
    #[test]
    fn quoted_unknown_other_and_unlocated_units_never_become_author_patterns() {
        for speaker in ["quoted", "unknown"] {
            assert_eq!(project(&[part(12, speaker)])["items"], json!([]));
        }
        let mut other = part(12, "author");
        other.input.coverage["core"]["contentType"] = json!("other");
        assert_eq!(
            project(&[other.clone()])["items"].as_array().unwrap().len(),
            1
        );
        other.input.fragments.clear();
        assert_eq!(project(&[other])["items"], json!([]));
    }
    #[test]
    fn does_not_blend_old_core_or_use_stale_and_comparison_windows() {
        let mut newest = part(13, "author");
        newest.input.coverage["core"]["contentType"] = json!("checklist");
        let old = part(12, "author");
        let actual = project(&[newest.clone(), old]);
        assert_eq!(actual["items"][0]["label"], "checklist");
        assert_eq!(actual["coreRef"], json!(Uuid::from_u128(13)));
        newest.current_coverage_eligible = false;
        assert!(project(&[newest.clone()]).is_null());
        newest.current_coverage_eligible = true;
        newest.manifest["coverage"]["kind"] = json!("comparison");
        assert!(project(&[newest]).is_null());
    }
}
