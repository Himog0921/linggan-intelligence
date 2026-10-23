//! Machine-induced concept text remains dependent on its qualified source window.
use crate::{
    model_settings::ModelError,
    topic_map_research::{self, ResearchInput},
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// Only machine rules are included. Human definitions have their own explicit authority.
/// A concept may be available before its source window finishes all later units.
pub(crate) async fn unavailable_definitions(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<HashSet<Uuid>, ModelError> {
    qualify_definitions_and_inputs(db, domain, &HashMap::new())
        .await
        .map(|(unavailable, _)| unavailable)
}

pub(crate) type FrozenScope = (Uuid, Uuid, String);
pub(crate) type FrozenScopes = HashMap<FrozenScope, BTreeSet<Uuid>>;

pub(crate) struct QualifiedInputs {
    pub current: HashMap<(Uuid, Uuid), Vec<ResearchInput>>,
    pub frozen: HashMap<FrozenScope, Vec<ResearchInput>>,
}

/// Merge definition creation sources and projected result sources before qualification.
/// Only request-local inputs are reused; every request rechecks current authority.
pub(crate) async fn qualify_definitions_and_inputs(
    db: &Database,
    domain: Option<Uuid>,
    additional_scopes: &FrozenScopes,
) -> Result<(HashSet<Uuid>, QualifiedInputs), ModelError> {
    let rows = sqlx::query("SELECT rule.definition_ref,rule.domain_ref,task.work_public_ref,run.config_ref,request.request_manifest FROM linggan_topic_map_concept_rule rule LEFT JOIN linggan_topic_map_research_request request ON request.invocation_ref=rule.invocation_ref LEFT JOIN linggan_topic_map_research_task task USING(task_ref) LEFT JOIN linggan_topic_map_research_run run ON run.run_ref=task.run_ref WHERE ($1::uuid IS NULL OR rule.domain_ref=$1)")
        .bind(domain).fetch_all(db.pool()).await?;
    let mut scopes = additional_scopes.clone();
    for row in &rows {
        let (Some(config), Some(work), Some(manifest)) = (
            row.get::<Option<Uuid>, _>("config_ref"),
            row.get::<Option<Uuid>, _>("work_public_ref"),
            row.get::<Option<Value>, _>("request_manifest"),
        ) else {
            continue;
        };
        let Some(as_of) = topic_map_research::manifest_as_of(&manifest) else {
            continue;
        };
        scopes
            .entry((row.get("domain_ref"), config, as_of.to_owned()))
            .or_default()
            .extend(topic_map_research::manifest_source_work_refs(
                work, &manifest,
            ));
    }
    let mut frozen = HashMap::<FrozenScope, Vec<ResearchInput>>::new();
    for ((domain, config, as_of), scope) in scopes {
        let scope: Vec<_> = scope.into_iter().collect();
        let loaded =
            topic_map_research::load_inputs_for_works_at(db, domain, config, &scope, &as_of)
                .await
                .map_err(source_error)?;
        frozen.insert((domain, config, as_of), loaded);
    }
    // Current inventory supplies coverage denominators only. Frozen request
    // restoration always selects the exact cutoff above, without a latest-source retry.
    let mut current_scopes = HashMap::<(Uuid, Uuid), BTreeSet<Uuid>>::new();
    for ((domain, config, _), scope) in additional_scopes {
        current_scopes
            .entry((*domain, *config))
            .or_default()
            .extend(scope);
    }
    let mut current = HashMap::new();
    for ((domain, config), scope) in current_scopes {
        let scope: Vec<_> = scope.into_iter().collect();
        let loaded = topic_map_research::load_inputs_for_works(db, domain, config, &scope)
            .await
            .map_err(source_error)?;
        current.insert((domain, config), loaded);
    }
    let mut unavailable = legacy_unverifiable_definitions(db, domain).await?;
    let mut dependencies = HashMap::new();
    for row in rows {
        let domain: Uuid = row.get("domain_ref");
        let (Some(config), Some(work), Some(manifest)) = (
            row.get::<Option<Uuid>, _>("config_ref"),
            row.get::<Option<Uuid>, _>("work_public_ref"),
            row.get::<Option<Value>, _>("request_manifest"),
        ) else {
            unavailable.insert(row.get("definition_ref"));
            continue;
        };
        dependencies.insert(
            row.get::<Uuid, _>("definition_ref"),
            definition_dependencies(&manifest),
        );
        let qualified = topic_map_research::manifest_as_of(&manifest)
            .and_then(|as_of| frozen.get(&(domain, config, as_of.to_owned())))
            .and_then(|inputs| topic_map_research::restore_scoped_window(inputs, work, &manifest));
        if qualified.is_none() {
            unavailable.insert(row.get("definition_ref"));
        }
    }
    propagate_unavailable(&mut unavailable, &dependencies);
    Ok((unavailable, QualifiedInputs { current, frozen }))
}

fn source_error(error: topic_map_research::ResearchError) -> ModelError {
    match error {
        topic_map_research::ResearchError::Database(error) => ModelError::Database(error),
        _ => ModelError::Source,
    }
}

async fn legacy_unverifiable_definitions(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<HashSet<Uuid>, ModelError> {
    // Before concept rules, machine proposals had no creation-invocation link.
    // A material member cannot establish which input created a definition.
    let rows = sqlx::query_scalar::<_, Uuid>(
        "SELECT definition.definition_ref FROM linggan_topic_definition definition JOIN linggan_topic_classification_run classification USING(definition_ref) LEFT JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=definition.topic_ref ORDER BY version DESC LIMIT 1)binding ON true WHERE classification.run_kind='machine_proposed' AND ($1::uuid IS NULL OR binding.domain_ref=$1) AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_concept_rule rule WHERE rule.definition_ref=definition.definition_ref)",
    )
    .bind(domain)
    .fetch_all(db.pool())
    .await?;
    Ok(rows.into_iter().collect())
}

/// Only explicit definition-reference fields carry dependency identity. Text labels
/// and arbitrary UUID-looking prose never become graph edges.
pub(crate) fn definition_dependencies(value: &Value) -> HashSet<Uuid> {
    let mut result = HashSet::new();
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    match key.as_str() {
                        "definitionRef" | "definition_ref" | "parentDefinitionRef" => {
                            if let Some(id) = value.as_str().and_then(|s| s.parse::<Uuid>().ok()) {
                                result.insert(id);
                            }
                        }
                        "comparedDefinitionRefs" | "forcedDefinitionRefs" => {
                            result.extend(
                                value.as_array().into_iter().flatten().filter_map(|v| {
                                    v.as_str().and_then(|s| s.parse::<Uuid>().ok())
                                }),
                            );
                        }
                        _ => pending.push(value),
                    }
                }
            }
            Value::Array(values) => pending.extend(values),
            _ => {}
        }
    }
    result
}
fn propagate_unavailable(
    unavailable: &mut HashSet<Uuid>,
    dependencies: &HashMap<Uuid, HashSet<Uuid>>,
) {
    let mut reverse = HashMap::<Uuid, Vec<Uuid>>::new();
    for (definition, used) in dependencies {
        for dependency in used {
            reverse.entry(*dependency).or_default().push(*definition);
        }
    }
    let mut pending: VecDeque<_> = unavailable.iter().copied().collect();
    while let Some(restricted) = pending.pop_front() {
        for dependent in reverse.get(&restricted).into_iter().flatten() {
            if unavailable.insert(*dependent) {
                pending.push_back(*dependent);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn hierarchy_parent_is_an_exact_dependency_without_candidate_comparison() {
        let parent = Uuid::from_u128(120);
        let value = json!({"comparedDefinitionRefs":[],"hierarchy":{"parentTopicRef":Uuid::from_u128(119),"parentDefinitionRef":parent},"label":Uuid::from_u128(118).to_string()});
        assert_eq!(definition_dependencies(&value), HashSet::from([parent]));
    }
    #[test]
    fn withdrawal_propagates_through_definition_creation_context_without_hiding_independent_rules()
    {
        let (a, b, c, independent) = (
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
        );
        let edges = HashMap::from([
            (
                b,
                definition_dependencies(
                    &json!({"topics":[{"definitionRef":a}],"label":independent.to_string()}),
                ),
            ),
            (
                c,
                definition_dependencies(
                    &json!({"source":{"coverage":{"selectedDiscussions":[{"comparedDefinitionRefs":[b]}]}}}),
                ),
            ),
            (independent, HashSet::new()),
        ]);
        let mut unavailable = HashSet::from([a]);
        propagate_unavailable(&mut unavailable, &edges);
        assert_eq!(unavailable, HashSet::from([a, b, c]));
    }
}
