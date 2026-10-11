//! Semantic reuse keys are independent of immutable physical audit references.
use super::windows::is_research_evidence;
use super::{INPUT_CONTRACT, ResearchInput};
use crate::topic_map_research_analysis::METHOD_VERSION;
use linggan_evidence::creator_discovery::{self, Fragment};
use serde_json::{Value, json};

pub(super) fn domain_identity(domain: &Value) -> Value {
    json!({"domainRef":domain["domainRef"],"name":domain["name"],"description":domain["description"],"researchGoal":domain["researchGoal"]})
}

pub(super) fn role_identity(input: &ResearchInput) -> Value {
    if input.coverage["kind"] == "comparison" {
        let mut roles: Vec<_> = input.role_metadata["works"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|role| {
                json!({"workRef":role["workRef"],"platform":role["platform"],
                "authorExternalId":role["authorExternalId"],"usageRoles":role["usageRoles"],
                "own":role["own"],"ownScopeConfigured":role["ownScopeConfigured"],
                "manualOwnBreakout":role["manualOwnBreakout"],"likes":role["likes"],
                "followerCount":role["followerCount"],"publishedAt":role["publishedAt"]})
            })
            .collect();
        roles.sort_by(|a, b| a["workRef"].as_str().cmp(&b["workRef"].as_str()));
        return json!({"works":roles});
    }
    let role = input
        .role_metadata
        .get("works")
        .and_then(Value::as_array)
        .and_then(|works| {
            works
                .iter()
                .find(|r| r["workRef"] == json!(input.work.work_ref))
        })
        .unwrap_or(&input.role_metadata);
    json!({"workRef":input.work.work_ref,"platform":input.work.platform,
        "authorExternalId":input.work.author_external_id,"usageRoles":input.work.usage_roles,
        "own":role["own"],"ownScopeConfigured":role["ownScopeConfigured"]})
}

pub(super) fn origin_id(input: &ResearchInput, fragment: &Fragment) -> String {
    input
        .coverage
        .get("fragmentOrigins")
        .and_then(|origins| origins.get(&fragment.fragment_id))
        .and_then(Value::as_str)
        .unwrap_or(&fragment.fragment_id)
        .to_owned()
}

/// Logical owner of a quoted range, shared by unit deduplication and coverage.
/// Physical request references remain available separately for audit.
pub(crate) fn semantic_source_identity(input: &ResearchInput, fragment: &Fragment) -> String {
    let origin = origin_id(input, fragment);
    match fragment.field.as_str() {
        "title" | "body" => format!(
            "{}.{}",
            origin.split('.').next().unwrap_or(""),
            fragment.field
        ),
        "studied_comment" | "unresearched_comment" => origin
            .replacen(".studied_comment.", ".comment.", 1)
            .replacen(".unresearched_comment.", ".comment.", 1),
        _ => origin,
    }
}

pub(super) fn fragment_reference(input: &ResearchInput, fragment: &Fragment) -> Value {
    let origin = origin_id(input, fragment);
    json!({"fragmentId":fragment.fragment_id,"sourceFragmentId":origin,
        "sourceRef":fragment.source_ref,"sourceVersion":fragment.source_version,"field":fragment.field,
        "start":fragment.start,"end":fragment.end,"textHash":creator_discovery::hash(&fragment.text),
        "sourceTextHash":input.coverage["sourceHashes"][&origin],
        "workRef":fragment.fragment_id.split('.').next(),"contextOnly":!is_research_evidence(fragment)})
}

fn semantic_fragment(input: &ResearchInput, fragment: &Fragment) -> Value {
    json!({"sourceFragmentId":semantic_source_identity(input,fragment),"field":fragment.field,
        "start":fragment.start,"end":fragment.end,"textHash":creator_discovery::hash(&fragment.text),
        "contextOnly":!is_research_evidence(fragment)})
}

fn semantic_comment_study(input: &ResearchInput) -> Value {
    let mut comments = input.comment_study.clone();
    for comment in comments.as_array_mut().into_iter().flatten() {
        if let Some(comment) = comment.as_object_mut() {
            comment.remove("sourceRef");
            comment.remove("parentSourceRef");
        }
    }
    comments
}

fn semantic_discussions(input: &ResearchInput) -> Value {
    if input.coverage["kind"] != "comparison" {
        return Value::Null;
    }
    let mut discussions = input.coverage["selectedDiscussions"].clone();
    for discussion in discussions.as_array_mut().into_iter().flatten() {
        if let Some(discussion) = discussion.as_object_mut() {
            discussion.remove("taskRef");
        }
    }
    discussions
}

pub(crate) fn input_identity(input: &ResearchInput) -> String {
    input_identity_for(input, INPUT_CONTRACT, METHOD_VERSION)
}

/// A historical frozen request is verified under its original contract/method;
/// this does not admit historical tasks for new dispatch.
pub(super) fn input_identity_for(input: &ResearchInput, contract: &str, method: &str) -> String {
    // Definitions affect assignment. Re-observation and unselected source tails
    // retain audit provenance, but cannot invalidate a semantically equal window.
    let mut comparison_scope: Vec<_> = input.coverage["scopeWorkRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    comparison_scope.sort();
    comparison_scope.dedup();
    creator_discovery::hash(&json!({
        "contract":contract,"kind":input.coverage["kind"].as_str().unwrap_or("source"),
        "domain":domain_identity(&input.domain),"workRef":input.work.work_ref,
        "fragments":input.fragments.iter().map(|f|semantic_fragment(input,f)).collect::<Vec<_>>(),
        "commentStudy":semantic_comment_study(input),"roles":role_identity(input),
        "selectedDiscussions":semantic_discussions(input),
        "comparisonScope":if input.coverage["kind"] == "comparison" { json!(comparison_scope) } else { Value::Null },
        "method":method,"config":input.coverage["configRef"],
    }).to_string())
}
