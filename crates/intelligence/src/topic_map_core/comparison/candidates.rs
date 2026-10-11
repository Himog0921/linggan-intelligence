use super::*;

pub(super) fn current_candidates(
    inputs: &[ResearchInput],
    tasks: &[TaskEvidence],
    requested: &[Uuid],
    catalog: &Value,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for task in tasks.iter().filter(|t| requested.contains(&t.work)) {
        let Some(source) = inputs.iter().find(|i| i.work.work_ref == task.work) else {
            continue;
        };
        let manifest = task.manifest.get("source").unwrap_or(&task.manifest);
        if manifest["coverage"]["kind"] == "comparison" {
            continue;
        }
        let Some(window) = research::restore_window(source, manifest) else {
            continue;
        };
        for unit in task.resolutions.as_array().into_iter().flatten() {
            let Some(id) = unit["unitId"].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            if !["matched", "new", "uncertain"].contains(&unit["status"].as_str().unwrap_or(""))
                || !unit["statement"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty())
                || seen.contains(&(task.work, id.to_owned()))
            {
                continue;
            }
            let Some(evidence) = unit_evidence(unit, task.work, &window).filter(|e| {
                e.iter().any(|s| {
                    s.context_field.is_none()
                        && source
                            .fragments
                            .iter()
                            .any(|f| f.fragment_id == s.origin && research::is_research_evidence(f))
                })
            }) else {
                continue;
            };
            if speaker_lacks_evidence(unit, source, &evidence) {
                continue;
            }
            let Some(spans) = with_parent_context(source, &window, &evidence, task.work) else {
                continue;
            };
            let Some(unit) = current_definition_unit(unit, catalog) else {
                continue;
            };
            seen.insert((task.work, id.to_owned()));
            candidates.push(Candidate {
                task: task.task,
                work: task.work,
                unit,
                evidence,
                spans: merge_spans(spans),
            });
        }
    }
    candidates
}

fn unit_evidence(unit: &Value, work: Uuid, window: &ResearchInput) -> Option<Vec<Span>> {
    unit["evidence"]
        .as_array()?
        .iter()
        .map(|reference| {
            let fragment = window
                .fragments
                .iter()
                .find(|f| reference["fragmentId"].as_str() == Some(&f.fragment_id))?;
            let start = usize::try_from(reference["start"].as_u64()?).ok()?;
            let end = usize::try_from(reference["end"].as_u64()?).ok()?;
            if start < fragment.start || end > fragment.end || start >= end {
                return None;
            }
            let origin = window.coverage["fragmentOrigins"][&fragment.fragment_id]
                .as_str()
                .unwrap_or(&fragment.fragment_id);
            Some(Span {
                work,
                origin: origin.into(),
                context_field: (!research::is_research_evidence(fragment))
                    .then(|| fragment.field.clone()),
                start,
                end,
            })
        })
        .collect()
}

fn speaker_lacks_evidence(unit: &Value, source: &ResearchInput, evidence: &[Span]) -> bool {
    let author_evidence = evidence.iter().any(|span| {
        span.context_field.is_none()
            && source
                .work
                .fragments
                .iter()
                .any(|fragment| fragment.fragment_id == span.origin)
    });
    let comment_evidence = evidence.iter().any(|span| {
        span.context_field.is_none()
            && source
                .comment_study
                .as_array()
                .into_iter()
                .flatten()
                .any(|comment| {
                    comment["sourceFragmentId"]
                        .as_str()
                        .or_else(|| comment["fragmentId"].as_str())
                        == Some(&span.origin)
                })
    });
    (unit["speakerRole"] == "author" && !author_evidence)
        || (unit["speakerRole"] == "commenter" && !comment_evidence)
}

fn with_parent_context(
    source: &ResearchInput,
    window: &ResearchInput,
    evidence: &[Span],
    work: Uuid,
) -> Option<Vec<Span>> {
    // A parent is context for its child, never an additional person's experience.
    let mut spans = evidence.to_vec();
    for reference in evidence.iter().filter(|span| span.context_field.is_none()) {
        let parent = source
            .comment_study
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| {
                c["sourceFragmentId"]
                    .as_str()
                    .or_else(|| c["fragmentId"].as_str())
                    == Some(&reference.origin)
            })
            .and_then(|c| c["parentFragmentId"].as_str());
        if let Some(parent) = parent
            && !spans.iter().any(|s| {
                s.origin == parent && s.context_field.as_deref() == Some("parent_comment_context")
            })
        {
            // Keep the parent segment actually supplied to this accepted window.
            let fragment = window.fragments.iter().find(|f| {
                f.field == "parent_comment_context"
                    && window.coverage["fragmentOrigins"][&f.fragment_id]
                        .as_str()
                        .unwrap_or(&f.fragment_id)
                        == parent
            })?;
            spans.push(Span {
                work,
                origin: parent.into(),
                context_field: Some(fragment.field.clone()),
                start: fragment.start,
                end: fragment
                    .end
                    .min(fragment.start.saturating_add(PARENT_CHARS)),
            });
        }
    }
    (!spans.is_empty()).then_some(spans)
}

fn current_definition_unit(unit: &Value, catalog: &Value) -> Option<Value> {
    let mut unit = unit.clone();
    let compared: BTreeSet<String> = unit["comparedDefinitionRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if let Some(parent) = unit["hierarchy"].as_object() {
        if !catalog.as_array().into_iter().flatten().any(|topic| {
            topic["topicRef"] == parent["parentTopicRef"]
                && topic["definitionRef"] == parent["parentDefinitionRef"]
        }) {
            return None;
        }
    }
    // A rejected neighbour can affect a decision just as an assignment
    // can. Keep that dependency; do not silently discard a stale boundary.
    if compared.iter().any(|definition| {
        !catalog
            .as_array()
            .into_iter()
            .flatten()
            .any(|topic| topic["definitionRef"].as_str() == Some(definition.as_str()))
    }) {
        return None;
    }
    unit["comparedDefinitionRefs"] = json!(compared);
    let mut assignments: Vec<_> =
        unit["assignments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| {
                catalog.as_array().into_iter().flatten().any(|t| {
                    t["topicRef"] == a["topicRef"] && t["definitionRef"] == a["definitionRef"]
                })
            })
            .cloned()
            .collect();
    if assignments.len() != unit["assignments"].as_array().map_or(0, Vec::len) {
        unit["status"] = json!("uncertain");
        unit["reason"] = json!("主题定义已经变化，原讨论保留，旧归属不作为当前主题一致性的依据。");
        assignments.clear();
    }
    unit["assignments"] = json!(assignments);
    Some(unit)
}
