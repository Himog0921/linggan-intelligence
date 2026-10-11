//! Bounded, source-qualified author responses and cross-work comparisons.
use crate::topic_map_research::{self as research, ResearchInput};
use linggan_evidence::creator_discovery::Fragment;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::Uuid;

#[path = "comparison/candidates.rs"]
mod candidates;
#[path = "comparison/queue.rs"]
mod queue;
#[path = "comparison/scheduling.rs"]
mod scheduling;
#[path = "comparison/selection.rs"]
mod selection;
#[path = "comparison/unknown.rs"]
mod unknown;
use candidates::current_candidates;
pub(crate) use queue::{has_current_comparison, pending_in, queue_once};
use selection::selected_spans;
#[cfg(test)]
use selection::shared_topics;
use selection::{choose, merge_spans};
pub(crate) use unknown::blocks_comparison as blocks_unknown_dispatch;
#[cfg(test)]
#[path = "comparison/tests.rs"]
mod tests;

const MAX_CHARS: usize = 3000;
const MAX_UNITS_PER_WORK: usize = 2;
const PARENT_CHARS: usize = 160;
const TERMINAL: [&str; 7] = [
    "succeeded",
    "no_signal",
    "insufficient",
    "stale",
    "failed",
    "stopped",
    "unknown_dispatch",
];
#[derive(Clone)]
struct TaskEvidence {
    task: Uuid,
    work: Uuid,
    manifest: Value,
    resolutions: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Span {
    work: Uuid,
    origin: String,
    context_field: Option<String>,
    start: usize,
    end: usize,
}

#[derive(Clone)]
struct Candidate {
    task: Uuid,
    work: Uuid,
    unit: Value,
    evidence: Vec<Span>,
    spans: Vec<Span>,
}

struct ComparisonFragments {
    fragments: Vec<Fragment>,
    origins: serde_json::Map<String, Value>,
    owners: serde_json::Map<String, Value>,
}

fn scope(value: &Value) -> Option<Vec<Uuid>> {
    let rows = value.get("workRefs")?.as_array()?;
    if !(1..=10).contains(&rows.len()) {
        return None;
    }
    let mut refs: Vec<Uuid> = rows
        .iter()
        .map(|v| v.as_str()?.parse().ok())
        .collect::<Option<_>>()?;
    refs.sort_unstable();
    (refs.iter().copied().collect::<BTreeSet<_>>().len() == refs.len()).then_some(refs)
}

fn brief(value: &Value, max: usize) -> String {
    value.as_str().unwrap_or("").chars().take(max).collect()
}

fn build_comparison(
    inputs: &[ResearchInput],
    tasks: &[TaskEvidence],
    requested: &[Uuid],
    catalog: &Value,
) -> Result<ResearchInput, &'static str> {
    if !(1..=10).contains(&requested.len())
        || requested.iter().copied().collect::<BTreeSet<_>>().len() != requested.len()
    {
        return Err("comparison_scope_invalid");
    }
    let mut canonical_scope = requested.to_vec();
    canonical_scope.sort_unstable();
    let requested = canonical_scope.as_slice();
    let candidates = current_candidates(inputs, tasks, requested, catalog);
    let selected = choose(&candidates, requested);
    let selected_works: Vec<_> = requested
        .iter()
        .filter(|w| selected.iter().any(|c| c.work == **w))
        .copied()
        .collect();
    if requested.len() == 1 && (selected.len() != 2 || selected_works.len() != 1) {
        return Err("comparison_requires_author_and_commenter_discussions");
    }
    if requested.len() > 1 && selected_works.len() < 2 {
        return Err("comparison_requires_two_current_discussion_sources");
    }
    let primary = inputs
        .iter()
        .find(|i| i.work.work_ref == selected_works[0])
        .ok_or("comparison_source_unavailable")?;
    let available: Vec<_> = requested
        .iter()
        .filter(|w| inputs.iter().any(|i| i.work.work_ref == **w))
        .copied()
        .collect();
    let full = research::with_comparison_context(primary.clone(), inputs, &available);
    let spans = selected_spans(&selected);
    let sources = comparison_fragments(inputs, &spans)?;
    let discussions = selected_discussions(&selected, &sources)?;
    let ComparisonFragments {
        fragments,
        origins,
        owners,
    } = sources;
    let evidence_chars: usize = fragments
        .iter()
        .filter(|f| research::is_research_evidence(f))
        .map(|f| f.text.chars().count())
        .sum();
    let context_chars: usize = fragments
        .iter()
        .filter(|f| !research::is_research_evidence(f))
        .map(|f| f.text.chars().count())
        .sum();
    let source_chars: usize = inputs
        .iter()
        .filter(|i| available.contains(&i.work.work_ref))
        .flat_map(|i| &i.fragments)
        .filter(|f| research::is_research_evidence(f))
        .map(|f| f.text.chars().count())
        .sum();
    let mut input = full.clone();
    input.coverage = json!({"inputContract":full.coverage["inputContract"],"configRef":full.coverage["configRef"],
        "kind":"comparison","scopeWorkRefs":requested,"availableWorkRefs":available,"selectedWorkRefs":selected_works,
        "sourceChars":source_chars,"coveredChars":evidence_chars,"evidenceChars":evidence_chars,"contextChars":context_chars,
        "inputChars":evidence_chars+context_chars,"maxChars":MAX_CHARS,"state":"partial",
        "availableDiscussionCount":candidates.len(),"selectedDiscussionCount":selected.len(),
        "selectionPolicy":"current_units_shared_topic_counterexample_first_v1","selectedDiscussions":discussions,
        "fragmentOrigins":origins,"fragmentWorkRefs":owners,
        "sourceHashes":spans.iter().map(|span|(span.origin.clone(),full.coverage["sourceHashes"][&span.origin].clone())).collect::<serde_json::Map<String,Value>>(),
        "perWork":requested.iter().map(|work|json!({"workRef":work,
            "availableDiscussionCount":candidates.iter().filter(|c|c.work==*work).count(),
            "selectedDiscussionCount":selected.iter().filter(|c|c.work==*work).count(),
            "selectedChars":fragments.iter().filter(|f|owners[&f.fragment_id]==json!(work) && research::is_research_evidence(f)).map(|f|f.text.chars().count()).sum::<usize>(),
            "parentContextChars":fragments.iter().filter(|f|owners[&f.fragment_id]==json!(work) && f.field=="parent_comment_context").map(|f|f.text.chars().count()).sum::<usize>()
        })).collect::<Vec<_>>(),"countsArePeople":false,
        "boundary":"只比较所列作品的代表讨论与引用；未选入材料、缺少的作者或评论立场及因果关系仍未知。"});
    input.work.fragments = fragments
        .iter()
        .filter(|f| research::is_research_evidence(f))
        .filter(|f| {
            primary
                .work
                .fragments
                .iter()
                .any(|s| input.coverage["fragmentOrigins"][&f.fragment_id] == s.fragment_id)
        })
        .cloned()
        .collect();
    input.comment_study =
        research::selected_comment_study(&full, &fragments, &input.coverage["fragmentOrigins"]);
    input.fragments = fragments;
    input.hash = research::input_identity(&input);
    // The same reader/worker reconstruction must be able to prove every participant now.
    research::restore_window(&full, &research::reference_manifest(&input))
        .ok_or("comparison_manifest_not_restorable")
}

fn comparison_fragments(
    inputs: &[ResearchInput],
    spans: &[Span],
) -> Result<ComparisonFragments, &'static str> {
    let mut fragments = Vec::new();
    let mut origins = serde_json::Map::new();
    let mut owners = serde_json::Map::new();
    for span in spans {
        let source = inputs
            .iter()
            .find(|i| i.work.work_ref == span.work)
            .and_then(|i| i.fragments.iter().find(|f| f.fragment_id == span.origin))
            .ok_or("comparison_source_unavailable")?;
        let text: String = source
            .text
            .chars()
            .skip(span.start - source.start)
            .take(span.end - span.start)
            .collect();
        if text.chars().count() != span.end - span.start {
            return Err("comparison_source_range_invalid");
        }
        let field = span.context_field.as_deref().unwrap_or(&source.field);
        let marker = if field.starts_with("work_context:") {
            "context.chars"
        } else {
            "chars"
        };
        let id = format!("{}.{marker}.{}.{}", span.origin, span.start, span.end);
        origins.insert(id.clone(), json!(span.origin));
        owners.insert(id.clone(), json!(span.work));
        fragments.push(Fragment {
            fragment_id: id,
            source_ref: source.source_ref,
            source_version: source.source_version.clone(),
            field: field.into(),
            start: span.start,
            end: span.end,
            text,
        });
    }
    Ok(ComparisonFragments {
        fragments,
        origins,
        owners,
    })
}

fn selected_discussions(
    selected: &[Candidate],
    sources: &ComparisonFragments,
) -> Result<Vec<Value>, &'static str> {
    let mut discussions = Vec::new();
    for candidate in selected {
        let evidence: Vec<_> = candidate
            .evidence
            .iter()
            .map(|span| {
                let fragment = sources
                    .fragments
                    .iter()
                    .find(|f| {
                        sources.origins[&f.fragment_id] == span.origin
                            && sources.owners[&f.fragment_id] == json!(span.work)
                            && match &span.context_field {
                                Some(field) => f.field == *field,
                                None => research::is_research_evidence(f),
                            }
                            && f.start <= span.start
                            && f.end >= span.end
                    })
                    .ok_or("comparison_source_range_invalid")?;
                Ok(json!({"fragmentId":fragment.fragment_id,"start":span.start,"end":span.end}))
            })
            .collect::<Result<_, &'static str>>()?;
        discussions.push(json!({"unitId":candidate.unit["unitId"],"workRef":candidate.work,"taskRef":candidate.task,
            "label":brief(&candidate.unit["label"],120),"statement":brief(&candidate.unit["statement"],300),
            "statementTruncated":candidate.unit["statement"].as_str().is_some_and(|s|s.chars().count()>300),
            "speakerRole":candidate.unit["speakerRole"],"evidenceRole":candidate.unit["evidenceRole"],
            "comparedDefinitionRefs":candidate.unit["comparedDefinitionRefs"],
            "hierarchy":candidate.unit["hierarchy"].as_object().map(|h|json!({"parentTopicRef":h["parentTopicRef"],"parentDefinitionRef":h["parentDefinitionRef"]})),
            "reason":brief(&candidate.unit["reason"],160),
            "status":candidate.unit["status"],"assignments":candidate.unit["assignments"].as_array().into_iter().flatten().map(|a|
                json!({"topicRef":a["topicRef"],"definitionRef":a["definitionRef"],"label":brief(&a["label"],120),"reason":brief(&a["reason"],160)})).collect::<Vec<_>>(),
            "evidence":evidence}));
    }
    Ok(discussions)
}
