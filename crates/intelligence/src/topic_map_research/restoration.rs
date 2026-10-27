//! Exact reconstruction of frozen canonical blocks under current source qualification.
use super::identity::{domain_identity, input_identity_for, role_identity};
use super::windows::{is_research_evidence, selected_comment_study, with_comparison_context};
use super::{
    INPUT_CONTRACT, ResearchError, ResearchInput, load_inputs_for_works_at, manifest_as_of,
    manifest_source_work_refs,
};
use linggan_evidence::creator_discovery::{self, Fragment};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

/// Restore the frozen window from its exact canonical source versions, qualified
/// under current restrictions. Durable request manifests contain references only.
pub(crate) fn restore_window(input: &ResearchInput, manifest: &Value) -> Option<ResearchInput> {
    manifest_as_of(manifest)?;
    current_identity(input, manifest)?;
    let (fragments, origins) = restore_fragments(input, manifest)?;
    let mut restored = input.clone();
    restored.work.fragments = fragments
        .iter()
        .filter(|f| is_research_evidence(f))
        .filter(|f| {
            input.work.fragments.iter().any(|source| {
                Some(source.fragment_id.as_str())
                    == origins.get(&f.fragment_id).and_then(Value::as_str)
            })
        })
        .cloned()
        .collect();
    restored.comment_study = selected_comment_study(input, &fragments, &origins);
    restored.fragments = fragments;
    restored.coverage = manifest["coverage"]
        .as_object()
        .cloned()
        .map(Value::Object)?;
    restored.coverage["fragmentOrigins"] = origins;
    restored.context_work_refs = manifest["contextWorkRefs"]
        .as_array()?
        .iter()
        .map(|v| v.as_str()?.parse().ok())
        .collect::<Option<Vec<Uuid>>>()?;
    if restored.context_work_refs.len() > 10 {
        return None;
    }
    if restored.coverage["kind"] != "comparison" {
        restored.role_metadata["authorTextAvailable"] = json!(!restored.work.fragments.is_empty());
        restored.role_metadata["commentOnly"] = json!(restored.work.fragments.is_empty());
        restored.role_metadata["sourceRole"] = json!(if restored.work.fragments.is_empty() {
            "user_comment"
        } else {
            "author_work"
        });
    }
    restored.hash = input_identity_for(
        &restored,
        manifest["inputContract"].as_str()?,
        manifest["methodVersion"].as_str()?,
    );
    if manifest["inputHash"].as_str()? != restored.hash {
        return None;
    }
    restored.comment_study = manifest["commentStudy"].clone();
    Some(restored)
}

fn current_identity(input: &ResearchInput, manifest: &Value) -> Option<()> {
    let contract = manifest["inputContract"].as_str()?;
    let method = manifest["methodVersion"].as_str()?;
    if contract != INPUT_CONTRACT
        || method != crate::topic_map_research_analysis::METHOD_VERSION
        || manifest["workRef"] != json!(input.work.work_ref)
        || manifest["configRef"] != input.coverage["configRef"]
        || manifest["coverage"]["asOf"] != input.coverage["asOf"]
        || manifest["domainDefinitionHash"].as_str()?
            != creator_discovery::hash(&domain_identity(&input.domain).to_string())
        || manifest["roleIdentity"] != role_identity(input)
    {
        return None;
    }
    if manifest["coverage"]["kind"] == "comparison"
        && manifest["coverage"]["selectedDiscussions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d["hierarchy"].as_object())
            .any(|parent| {
                !input.topics.as_array().into_iter().flatten().any(|topic| {
                    topic["topicRef"] == parent["parentTopicRef"]
                        && topic["definitionRef"] == parent["parentDefinitionRef"]
                })
            })
    {
        return None;
    }
    if manifest["coverage"]["kind"] == "comparison"
        && manifest["coverage"]["selectedDiscussions"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|discussion| {
                discussion["assignments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|assignment| assignment["definitionRef"].as_str())
                    .chain(
                        discussion["comparedDefinitionRefs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str),
                    )
            })
            .any(|definition| {
                !input
                    .topics
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|topic| topic["definitionRef"].as_str() == Some(definition))
            })
    {
        return None;
    }
    Some(())
}

// Comparison preparation only combines qualified canonical sources from the
// declared work scope. A copied work context must retain that exact author owner.
fn qualified_author_context(input: &ResearchInput, source: &Fragment) -> bool {
    matches!(
        source.field.as_str(),
        "title" | "body" | "tags" | "ocr" | "transcript" | "frame_text"
    ) && (input
        .work
        .fragments
        .iter()
        .any(|author| author.fragment_id == source.fragment_id)
        || input.coverage["kind"] == "comparison"
            && source
                .fragment_id
                .split('.')
                .next()
                .and_then(|owner| owner.parse::<Uuid>().ok())
                .is_some_and(|owner| input.context_work_refs.contains(&owner)))
}

fn restore_fragments(input: &ResearchInput, manifest: &Value) -> Option<(Vec<Fragment>, Value)> {
    let refs = manifest["fragments"].as_array()?;
    if refs.is_empty() {
        return None;
    }
    let mut fragments = Vec::new();
    let mut origins = serde_json::Map::new();
    let mut ids = BTreeSet::new();
    for reference in refs {
        let id = reference["fragmentId"].as_str()?;
        let origin = reference["sourceFragmentId"].as_str()?;
        if !ids.insert(id.to_owned()) {
            return None;
        }
        let source = input.fragments.iter().find(|f| {
            f.fragment_id == origin
                && (reference["field"] == json!(f.field)
                    || reference["field"]
                        .as_str()
                        .and_then(|field| field.strip_prefix("work_context:"))
                        == Some(f.field.as_str())
                        && qualified_author_context(input, f))
        })?;
        let frozen_source = reference["sourceRef"].as_str()?.parse().ok()?;
        let frozen_version = reference["sourceVersion"].as_str()?.to_owned();
        if reference["workRef"].as_str()? != origin.split('.').next()?
            || manifest["coverage"]["fragmentWorkRefs"]
                .get(id)
                .is_some_and(|work| work != &reference["workRef"])
        {
            return None;
        }
        let start = usize::try_from(reference["start"].as_u64()?).ok()?;
        let end = usize::try_from(reference["end"].as_u64()?).ok()?;
        if start != source.start || end != source.end || start >= end {
            return None;
        }
        let work_context = reference["field"].as_str()?.starts_with("work_context:");
        let expected = if work_context {
            format!("{origin}.context")
        } else {
            origin.to_owned()
        };
        if id != expected
            || reference["blockRef"] != origin
            || reference["asOf"] != manifest["coverage"]["asOf"]
            || source.source_ref != frozen_source
            || source.source_version != frozen_version
            || reference["readingRef"] != json!(source.reading_ref)
            || reference["unitRef"] != json!(source.unit_ref)
            || reference["speaker"] != source.speaker
            || reference["structureHash"] != creator_discovery::hash(&source.structure.to_string())
            || reference["issues"] != json!(source.issues)
        {
            return None;
        }
        let text = source.text.clone();
        if text.chars().count() != end - start
            || reference["textHash"].as_str()? != creator_discovery::hash(&text)
            || reference["contextOnly"].as_bool()?
                != (work_context || !is_research_evidence(source))
        {
            return None;
        }
        origins.insert(id.to_owned(), json!(origin));
        // The exact frozen physical source remains the request's audit identity;
        // the canonical reader has already applied current withdrawal restrictions.
        fragments.push(Fragment {
            fragment_id: id.to_owned(),
            source_ref: frozen_source,
            field: reference["field"].as_str()?.into(),
            source_version: frozen_version,
            start,
            end,
            text,
            speaker: source.speaker.clone(),
            structure: source.structure.clone(),
            issues: source.issues.clone(),
            reading_ref: source.reading_ref,
            unit_ref: source.unit_ref,
        });
    }
    if !fragments.iter().any(is_research_evidence) {
        return None;
    }
    Some((fragments, Value::Object(origins)))
}

/// Comparison manifests freeze several qualified works; ordinary extraction
/// windows retain one source even when they carry a future comparison scope.
pub(crate) fn restore_scoped_window(
    all: &[ResearchInput],
    work: Uuid,
    manifest: &Value,
) -> Option<ResearchInput> {
    let manifest = manifest.get("source").unwrap_or(manifest);
    let input = all.iter().find(|input| input.work.work_ref == work)?;
    if manifest["coverage"]["kind"] != "comparison" {
        return restore_window(input, manifest);
    }
    let context: Vec<Uuid> = manifest["contextWorkRefs"]
        .as_array()?
        .iter()
        .map(|value| value.as_str()?.parse().ok())
        .collect::<Option<_>>()?;
    let scope: BTreeSet<_> = std::iter::once(work)
        .chain(context.iter().copied())
        .collect();
    if scope.len() > 10
        || scope.len() != context.len() + 1
        || context
            .iter()
            .any(|work| !all.iter().any(|input| input.work.work_ref == *work))
    {
        return None;
    }
    // Passing the full scope also wraps the primary work's roles for a single
    // work's author/comment response comparison.
    restore_window(
        &with_comparison_context(input.clone(), all, &scope.into_iter().collect::<Vec<_>>()),
        manifest,
    )
}

#[cfg(test)]
pub(crate) fn sources_current(input: &ResearchInput, manifest: &Value) -> bool {
    restore_window(input, manifest).is_some()
}

pub(crate) async fn research_result_sources_current(
    db: &Database,
    result: Uuid,
) -> Result<bool, ResearchError> {
    let row=sqlx::query("SELECT run.domain_ref,t.work_ref AS work_public_ref,r.output_json->>'contract' AS contract,(run.configurations->>0)::uuid AS config_ref,t.input_refs FROM linggan_topic_research_result r JOIN linggan_topic_research_window t USING(window_ref) JOIN linggan_topic_research_run run USING(run_ref) WHERE r.result_ref=$1").bind(result).fetch_optional(db.pool()).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let domain: Uuid = row.get("domain_ref");
    let config: Uuid = row.get("config_ref");
    let work: Uuid = row.get("work_public_ref");
    let manifest: Value = row.get("input_refs");
    let contract: Option<String> = row.get("contract");
    if contract.as_deref() != Some(crate::topic_map_research_analysis::EXTRACT_CONTRACT) {
        return Ok(false);
    }
    let Some(as_of) = manifest_as_of(&manifest) else {
        return Ok(false);
    };
    let scope = manifest_source_work_refs(work, &manifest);
    let all = load_inputs_for_works_at(db, domain, config, &scope, as_of).await?;
    Ok(restore_scoped_window(&all, work, &manifest).is_some())
}
