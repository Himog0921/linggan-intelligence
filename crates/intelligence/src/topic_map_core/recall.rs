//! Complete source-qualified concept catalog. Candidate scope is explicit.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn catalog(db: &Database, domain: Uuid) -> Result<Value, ModelError> {
    let unavailable = super::unavailable_definitions(db, Some(domain)).await?;
    let mut catalog = catalog_with(db.pool(), domain).await?;
    if let Some(rows) = catalog.as_array_mut() {
        rows.retain(|r| {
            r["definitionRef"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| !unavailable.contains(&id))
        });
    }
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
