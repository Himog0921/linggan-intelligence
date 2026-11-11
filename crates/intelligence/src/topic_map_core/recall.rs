//! Complete source-qualified concept catalog. Candidate scope is explicit.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn catalog(db: &Database, domain: Uuid) -> Result<Value, ModelError> {
    let (unavailable, sources) =
        super::qualify_definitions_and_inputs(db, Some(domain), &std::collections::HashMap::new())
            .await?;
    let mut catalog = catalog_with(db.pool(), domain).await?;
    if let Some(rows) = catalog.as_array_mut() {
        rows.retain(|r| {
            r["definitionRef"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| !unavailable.contains(&id))
        });
    }
    attach_support(db, domain, &sources, &mut catalog).await?;
    Ok(catalog)
}
async fn catalog_with<'e, E: sqlx::Executor<'e, Database = sqlx::Postgres>>(
    executor: E,
    domain: Uuid,
) -> Result<Value, ModelError> {
    let rows=sqlx::query("SELECT t.topic_ref,d.definition_ref,d.version,d.display_name,d.definition_text,c.inclusion_criteria,c.exclusion_criteria,c.method_version AS concept_method,b.parent_topic_ref,d.lifecycle_state,r.method_version AS induction_method FROM linggan_topic_workspace t JOIN LATERAL(SELECT *FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true JOIN LATERAL(SELECT *FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true LEFT JOIN linggan_topic_map_concept_rule c ON c.definition_ref=d.definition_ref LEFT JOIN linggan_topic_research_result r ON r.invocation_ref=c.invocation_ref WHERE b.domain_ref=$1 AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=t.topic_ref) ORDER BY t.topic_ref")
        .bind(domain).fetch_all(executor).await?;
    Ok(json!(rows.iter().map(|r|json!({"topicRef":r.get::<Uuid,_>("topic_ref"),"definitionRef":r.get::<Uuid,_>("definition_ref"),"version":r.get::<i32,_>("version"),"label":r.get::<String,_>("display_name"),"definition":r.get::<String,_>("definition_text"),"parentTopicRef":r.get::<Option<Uuid>,_>("parent_topic_ref"),"inductionMethod":r.get::<Option<String>,_>("induction_method"),"qualityState":if r.get::<String,_>("lifecycle_state")=="candidate" && !matches!(r.get::<Option<String>,_>("induction_method").as_deref(),Some(crate::topic_map_research_analysis::METHOD_VERSION)){"legacy_candidate"}else{"current"},"conceptRole":if r.get::<Option<String>,_>("concept_method").as_deref()==Some("topic-map.core.parent.v1"){"parent"}else{"topic"},"inclusionCriteria":r.get::<Option<Vec<String>>,_>("inclusion_criteria").unwrap_or_default(),"exclusionCriteria":r.get::<Option<Vec<String>>,_>("exclusion_criteria").unwrap_or_default()})).collect::<Vec<_>>()))
}

/// Reuse the exact qualified definition-creation windows already loaded above.
/// One example per definition keeps prompt growth linear; it is illustrative, not exhaustive.
async fn attach_support(
    db: &Database,
    domain: Uuid,
    sources: &super::source_qualification::QualifiedInputs,
    catalog: &mut Value,
) -> Result<(), ModelError> {
    let rows = sqlx::query("SELECT rule.definition_ref, task.work_ref, task.input_refs, (run.configurations->>0)::uuid AS config_ref, result.core_json FROM linggan_topic_map_concept_rule rule JOIN linggan_topic_research_result result ON result.invocation_ref=rule.invocation_ref JOIN linggan_topic_research_window task USING(window_ref) JOIN linggan_topic_research_run run USING(run_ref) WHERE rule.domain_ref=$1")
        .bind(domain).fetch_all(db.pool()).await?;
    for row in rows {
        let definition: Uuid = row.get("definition_ref");
        let Some(candidate) = catalog.as_array_mut().and_then(|items| {
            items
                .iter_mut()
                .find(|item| item["definitionRef"] == json!(definition))
        }) else {
            continue;
        };
        let manifest: Value = row.get("input_refs");
        let Some(as_of) = crate::topic_map_research::manifest_as_of(&manifest) else {
            continue;
        };
        let scope = (domain, row.get::<Uuid, _>("config_ref"), as_of.to_owned());
        let Some(input) = sources.frozen.get(&scope).and_then(|inputs| {
            crate::topic_map_research::restore_scoped_window(inputs, row.get("work_ref"), &manifest)
        }) else {
            continue;
        };
        let accepted: Value = row.get("core_json");
        let example = accepted["units"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|unit| {
                unit["assignments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|assignment| assignment["definitionRef"] == json!(definition))
                    || unit["hierarchy"]["parentDefinitionRef"] == json!(definition)
            })
            .and_then(|unit| support_example(&input, unit));
        if let Some(example) = example {
            candidate["supportExamples"] = json!([example]);
        }
    }
    Ok(())
}

fn support_example(
    input: &crate::topic_map_research::ResearchInput,
    unit: &Value,
) -> Option<Value> {
    let mut evidence = Vec::new();
    for citation in unit["evidence"].as_array()? {
        let fragment = input
            .fragments
            .iter()
            .find(|fragment| citation["fragmentId"] == fragment.fragment_id)?;
        let start = usize::try_from(citation["start"].as_u64()?).ok()?;
        let end = usize::try_from(citation["end"].as_u64()?).ok()?;
        if start < fragment.start || end > fragment.end || start >= end {
            return None;
        }
        let quote: String = fragment
            .text
            .chars()
            .skip(start - fragment.start)
            .take(end - start)
            .collect();
        evidence.push(
            json!({"fragmentId":fragment.fragment_id,"sourceRef":fragment.source_ref,
            "sourceVersion":fragment.source_version,"field":fragment.field,"start":start,"end":end,
            "text":quote,"anchor":fragment.structure.get("anchor"),
            "knowledgeUnits":fragment.structure.get("knowledgeUnits"),
            "evidenceRef":fragment.structure.get("evidenceRef")}),
        );
        if evidence.len() == 2 {
            break;
        }
    }
    if evidence.is_empty() {
        return None;
    }
    Some(
        json!({"workRef":input.work.work_ref,"coreRef":input.coverage.get("coreRef"),
        "sourceSnapshot":input.coverage.get("coreSnapshot"),"unitId":unit["unitId"],
        "statement":unit["statement"],"speakerRole":unit["speakerRole"],"evidenceRole":unit["evidenceRole"],
        "evidence":evidence,"scope":"definition_creation_example"}),
    )
}
