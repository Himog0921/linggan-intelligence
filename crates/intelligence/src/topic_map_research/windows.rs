//! Complete, stable Unicode windows with explicit roles and frozen source manifests.
pub(crate) use super::identity::input_identity;
pub(super) use super::identity::origin_id;
use super::identity::{domain_identity, fragment_reference, role_identity};
use super::{INPUT_CONTRACT, PARENT_CONTEXT_FIELD, ResearchInput};
use crate::topic_map_research_analysis::METHOD_VERSION;
use linggan_evidence::creator_discovery::{self, Fragment};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use uuid::Uuid;

pub(crate) fn is_research_evidence(fragment: &Fragment) -> bool {
    fragment.field != PARENT_CONTEXT_FIELD && !fragment.field.starts_with("work_context:")
}

pub(super) fn update_source_coverage(input: &mut ResearchInput) {
    let evidence: Vec<_> = input
        .fragments
        .iter()
        .filter(|f| is_research_evidence(f))
        .collect();
    input.coverage["sourceChars"] = json!(
        evidence
            .iter()
            .map(|f| f.text.chars().count())
            .sum::<usize>()
    );
    input.coverage["sourceCount"] = json!(evidence.len());
    input.coverage["commentCount"] = json!(input.comment_study.as_array().map_or(0, Vec::len));
    input.coverage["sourceContextChars"] = json!(
        input
            .fragments
            .iter()
            .filter(|f| !is_research_evidence(f))
            .map(|f| f.text.chars().count())
            .sum::<usize>()
    );
    input.coverage["platform"] = json!(input.work.platform);
    input.coverage["sourceHashes"] = Value::Object(
        input
            .fragments
            .iter()
            .map(|fragment| {
                (
                    origin_id(input, fragment),
                    json!(creator_discovery::hash(&fragment.text)),
                )
            })
            .collect(),
    );
}

pub(crate) fn selected_comment_study(
    input: &ResearchInput,
    fragments: &[Fragment],
    origins: &Value,
) -> Value {
    let mut selected = Vec::new();
    for fragment in fragments.iter().filter(|f| is_research_evidence(f)) {
        let origin = origins
            .get(&fragment.fragment_id)
            .and_then(Value::as_str)
            .unwrap_or(&fragment.fragment_id);
        if let Some(comment) = input
            .comment_study
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| {
                c["sourceFragmentId"]
                    .as_str()
                    .or_else(|| c["fragmentId"].as_str())
                    == Some(origin)
            })
        {
            let mut comment = comment.clone();
            comment["fragmentId"] = json!(fragment.fragment_id);
            comment["sourceFragmentId"] = json!(origin);
            let parent = comment["parentFragmentId"].as_str();
            comment["parentFragmentIds"] = json!(
                fragments
                    .iter()
                    .filter(|f| !is_research_evidence(f))
                    .filter(|f| origins.get(&f.fragment_id).and_then(Value::as_str) == parent)
                    .map(|f| f.fragment_id.clone())
                    .collect::<Vec<_>>()
            );
            selected.push(comment);
        }
    }
    json!(selected)
}

/// A whole note is the minimum author context. Comments are grouped by explicit
/// voice count, retaining every selected comment and its complete parents.
pub(crate) fn research_windows(input: &ResearchInput) -> Vec<ResearchInput> {
    let author: Vec<_> = input.fragments.iter()
        .filter(|f| is_research_evidence(f) && !is_comment(f)).cloned().collect();
    let mut windows = Vec::new();
    if !author.is_empty() { windows.push(source_window(input, author, Vec::new())); }
    let comments: Vec<_> = input.fragments.iter().filter(|f| is_comment(f)).cloned().collect();
    for group in comments.chunks(3) {
        windows.push(source_window(input, group.to_vec(), comment_context(input, group)));
    }
    let count = windows.len();
    for (index, window) in windows.iter_mut().enumerate() {
        window.coverage["windowIndex"] = json!(index);
        window.coverage["windowCount"] = json!(count);
    }
    windows
}

pub(crate) fn research_window_count(input: &ResearchInput) -> usize {
    usize::from(input.fragments.iter().any(|f| is_research_evidence(f) && !is_comment(f)))
        + input.fragments.iter().filter(|f| is_comment(f)).count().div_ceil(3)
}

fn chars(fragments: &[Fragment]) -> usize {
    fragments.iter().map(|f| f.text.chars().count()).sum()
}

fn is_comment(fragment: &Fragment) -> bool {
    matches!(fragment.field.as_str(), "studied_comment" | "unresearched_comment")
}

fn range_origin(input: &ResearchInput, fragment: &Fragment) -> String {
    input.coverage["fragmentOrigins"]
        .get(&fragment.fragment_id)
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            fragment
                .fragment_id
                .split(".chars.")
                .next()
                .unwrap_or(&fragment.fragment_id)
        })
        .to_owned()
}

fn comment_context(
    input: &ResearchInput,
    evidence: &[Fragment],
) -> Vec<Fragment> {
    let mut context = Vec::new();
    let mut parent_ids = BTreeSet::new();
    let origins: BTreeSet<_> = evidence.iter().map(|f| range_origin(input, f)).collect();
    for comment in input.comment_study.as_array().into_iter().flatten() {
        if origins.contains(
            comment["sourceFragmentId"]
                .as_str()
                .or_else(|| comment["fragmentId"].as_str())
                .unwrap_or(""),
        ) && let Some(parent) = comment["parentFragmentId"].as_str()
        {
            parent_ids.insert(parent);
        }
    }
    for source in &input.work.fragments {
        let mut piece = source.clone();
        piece.fragment_id = format!("{}.context", source.fragment_id);
        piece.field = format!("work_context:{}", source.field);
        context.push(piece);
    }
    context.extend(input.fragments.iter().filter(|source| parent_ids.contains(source.fragment_id.as_str())).cloned());
    context
}

fn source_window(
    input: &ResearchInput,
    evidence: Vec<Fragment>,
    context: Vec<Fragment>,
) -> ResearchInput {
    let evidence_chars = chars(&evidence);
    let context_chars = chars(&context);
    let mut origins = serde_json::Map::new();
    for f in evidence.iter().chain(&context) {
        let origin = f
            .fragment_id
            .split(if f.field.starts_with("work_context:") {
                ".context"
            } else {
                ".chars."
            })
            .next()
            .unwrap_or(&f.fragment_id);
        origins.insert(
            f.fragment_id.clone(),
            json!(
                input.coverage["fragmentOrigins"]
                    .get(&f.fragment_id)
                    .and_then(Value::as_str)
                    .unwrap_or(origin)
            ),
        );
    }
    let origins = Value::Object(origins);
    let mut window = input.clone();
    window.work.fragments = evidence
        .iter()
        .filter(|f| !is_comment(f))
        .cloned()
        .collect();
    window.fragments = evidence;
    window.fragments.extend(context);
    window.comment_study = selected_comment_study(input, &window.fragments, &origins);
    window.coverage["fragmentOrigins"] = origins;
    window.coverage["windowChars"] = json!(evidence_chars);
    window.coverage["evidenceChars"] = json!(evidence_chars);
    window.coverage["contextChars"] = json!(context_chars);
    window.coverage["inputChars"] = json!(evidence_chars + context_chars);
    window.coverage["workContextPartial"] = json!(false);
    window.coverage["repeatedEvidence"] = json!(false);
    window.coverage["materialSelection"] = json!("whole_note");
    window.coverage["sourceFragmentIds"] = json!(
        window
            .fragments
            .iter()
            .filter(|f| is_research_evidence(f))
            .map(|f| origin_id(&window, f))
            .collect::<BTreeSet<_>>()
    );
    window.coverage["sourceFragmentId"] = if window.coverage["sourceFragmentIds"]
        .as_array()
        .is_some_and(|s| s.len() == 1)
    {
        window.coverage["sourceFragmentIds"][0].clone()
    } else {
        Value::Null
    };
    identify_window(&mut window);
    window
}

fn identify_window(window: &mut ResearchInput) {
    window.role_metadata["authorTextAvailable"] = json!(!window.work.fragments.is_empty());
    window.role_metadata["commentOnly"] = json!(window.work.fragments.is_empty());
    window.role_metadata["sourceRole"] = json!(if window.work.fragments.is_empty() {
        "user_comment"
    } else {
        "author_work"
    });
    window.hash = input_identity(window);
    window.coverage["windowKey"] = json!(window.hash);
}

pub(crate) fn with_comparison_context(
    mut input: ResearchInput,
    all: &[ResearchInput],
    context: &[Uuid],
) -> ResearchInput {
    let mut seen: BTreeSet<_> = input
        .fragments
        .iter()
        .map(|f| f.fragment_id.clone())
        .collect();
    for work in context {
        if let Some(other) = all.iter().find(|i| i.work.work_ref == *work)
            && other.work.work_ref != input.work.work_ref
        {
            input.fragments.extend(
                other
                    .fragments
                    .iter()
                    .filter(|f| seen.insert(f.fragment_id.clone()))
                    .cloned(),
            );
            if let (Some(target), Some(more)) = (
                input.comment_study.as_array_mut(),
                other.comment_study.as_array(),
            ) {
                target.extend(more.clone());
            }
        }
    }
    if !context.is_empty() {
        input.role_metadata = json!({"works":all.iter().filter(|i|context.contains(&i.work.work_ref)||i.work.work_ref==input.work.work_ref).map(|i|i.role_metadata.clone()).collect::<Vec<_>>()});
        input.context_work_refs = context
            .iter()
            .copied()
            .filter(|work| *work != input.work.work_ref)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        input.coverage["kind"] = json!("comparison");
        update_source_coverage(&mut input);
        input.hash = input_identity(&input);
    }
    input
}

pub(crate) fn reference_manifest(input: &ResearchInput) -> Value {
    let scope: Vec<_> = std::iter::once(input.work.work_ref)
        .chain(input.context_work_refs.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    json!({"inputContract":INPUT_CONTRACT,"inputHash":input.hash,"workRef":input.work.work_ref,
        "contextWorkRefs":input.context_work_refs,"scopeWorkRefs":scope,"methodVersion":METHOD_VERSION,
        "configRef":input.coverage["configRef"],"domain":input.domain["domainRef"],
        "domainDefinitionHash":creator_discovery::hash(&domain_identity(&input.domain).to_string()),
        "roleIdentity":role_identity(input),"topics":[],"commentStudy":input.comment_study,
        "roleMetadata":input.role_metadata,"coverage":input.coverage,
        "fragments":input.fragments.iter().map(|f|fragment_reference(input,f)).collect::<Vec<_>>()})
}
