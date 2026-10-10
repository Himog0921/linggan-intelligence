//! Qualified reconstruction of frozen source ranges, including explicit legacy reads.
use super::identity::{domain_identity, input_identity, role_identity};
use super::windows::{is_research_evidence, selected_comment_study, with_comparison_context};
use super::{INPUT_CONTRACT, ResearchError, ResearchInput, load_inputs};
use linggan_evidence::creator_discovery::{self, Fragment};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

/// Restore only the frozen window, resolving every range through today's qualified
/// canonical source. No raw text is stored in the durable request manifest.
pub(crate) fn restore_window(input: &ResearchInput, manifest: &Value) -> Option<ResearchInput> {
    current_identity(input, manifest)?;
    let (fragments, origins, current_sources) = restore_fragments(input, manifest)?;
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
    restored.coverage["currentSources"] = current_sources;
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
    restored.hash = input_identity(&restored);
    if manifest["inputHash"].as_str()? != restored.hash {
        return None;
    }
    restored.comment_study = manifest["commentStudy"].clone();
    Some(restored)
}

fn current_identity(input: &ResearchInput, manifest: &Value) -> Option<()> {
    if manifest["inputContract"].as_str()? != INPUT_CONTRACT
        || manifest["workRef"] != json!(input.work.work_ref)
        || manifest["configRef"] != input.coverage["configRef"]
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

fn restore_fragments(
    input: &ResearchInput,
    manifest: &Value,
) -> Option<(Vec<Fragment>, Value, Value)> {
    let refs = manifest["fragments"].as_array()?;
    if refs.is_empty() {
        return None;
    }
    let mut fragments = Vec::new();
    let mut origins = serde_json::Map::new();
    let mut current_sources = serde_json::Map::new();
    let mut ids = BTreeSet::new();
    for reference in refs {
        let id = reference["fragmentId"].as_str()?;
        let origin = reference["sourceFragmentId"].as_str()?;
        if !ids.insert(id.to_owned()) {
            return None;
        }
        let source = input
            .fragments
            .iter()
            .find(|f| f.fragment_id == origin && reference["field"] == json!(f.field))?;
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
        if start < source.start || end > source.end || start >= end {
            return None;
        }
        let expected = format!("{origin}.chars.{start}.{end}");
        if id != expected && !(id == origin && start == source.start && end == source.end) {
            return None;
        }
        let text: String = source
            .text
            .chars()
            .skip(start - source.start)
            .take(end - start)
            .collect();
        if text.chars().count() != end - start
            || reference["textHash"].as_str()? != creator_discovery::hash(&text)
            || reference["contextOnly"].as_bool()? != !is_research_evidence(source)
        {
            return None;
        }
        origins.insert(id.to_owned(), json!(origin));
        current_sources.insert(id.to_owned(),json!({"sourceFragmentId":origin,"sourceRef":source.source_ref,
            "sourceVersion":source.source_version,"sourceTextHash":creator_discovery::hash(&source.text)}));
        // The current qualified source proves the text; frozen physical references
        // remain the audit identity of the already dispatched request and its units.
        fragments.push(Fragment {
            fragment_id: id.to_owned(),
            source_ref: frozen_source,
            field: source.field.clone(),
            source_version: frozen_version,
            start,
            end,
            text,
        });
    }
    if !fragments.iter().any(is_research_evidence) {
        return None;
    }
    Some((
        fragments,
        Value::Object(origins),
        Value::Object(current_sources),
    ))
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

/// Only a caller that has read the persisted v1 output contract may choose this
/// path. Legacy prefixes are resolved against today's qualified full sources.
pub(crate) fn restore_legacy_scoped(
    all: &[ResearchInput],
    source: &ResearchInput,
    manifest: &Value,
) -> Option<ResearchInput> {
    let manifest = manifest.get("source").unwrap_or(manifest);
    let context: Vec<Uuid> = manifest["contextWorkRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|value| value.as_str()?.parse().ok())
        .collect::<Option<_>>()?;
    if context.len() > 10
        || context
            .iter()
            .any(|work| !all.iter().any(|input| input.work.work_ref == *work))
    {
        return None;
    }
    let input = with_comparison_context(source.clone(), all, &context);
    let mut restored = input.clone();
    restored.fragments.clear();
    let refs = manifest["fragments"].as_array()?;
    if refs.is_empty() {
        return None;
    }
    for reference in refs {
        let start = usize::try_from(reference["start"].as_u64()?).ok()?;
        let end = usize::try_from(reference["end"].as_u64()?).ok()?;
        let original = input.fragments.iter().find(|fragment| {
            fragment.field == reference["field"].as_str().unwrap_or("")
                && (reference["sourceRef"] == json!(fragment.source_ref)
                    || reference["fragmentId"].as_str() == Some(&fragment.fragment_id))
        })?;
        if start < original.start || end > original.end || start >= end {
            return None;
        }
        let text: String = original
            .text
            .chars()
            .skip(start - original.start)
            .take(end - start)
            .collect();
        if text.chars().count() != end - start
            || reference["textHash"] != creator_discovery::hash(&text)
        {
            return None;
        }
        let mut fragment = original.clone();
        fragment.fragment_id = reference["fragmentId"].as_str()?.into();
        fragment.start = start;
        fragment.end = end;
        fragment.text = text;
        restored.fragments.push(fragment);
    }
    Some(restored)
}

pub(crate) async fn research_result_sources_current(
    db: &Database,
    result: Uuid,
) -> Result<bool, ResearchError> {
    let row=sqlx::query("SELECT r.domain_ref,r.work_public_ref,r.output_json->>'contract' AS contract,run.config_ref,q.request_manifest AS input_refs FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request q USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) JOIN linggan_topic_map_research_run run ON run.run_ref=t.run_ref WHERE r.result_ref=$1").bind(result).fetch_optional(db.pool()).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let domain: Uuid = row.get("domain_ref");
    let config: Uuid = row.get("config_ref");
    let work: Uuid = row.get("work_public_ref");
    let manifest: Value = row.get("input_refs");
    let all = load_inputs(db, domain, config).await?;
    Ok(match row.get::<Option<String>, _>("contract").as_deref() {
        Some("topic-map.research.v1") => all
            .iter()
            .find(|input| input.work.work_ref == work)
            .is_some_and(|source| restore_legacy_scoped(&all, source, &manifest).is_some()),
        Some(crate::topic_map_research_analysis::EXTRACT_CONTRACT) => {
            restore_scoped_window(&all, work, &manifest).is_some()
        }
        _ => false,
    })
}
