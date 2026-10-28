//! A saved decision keeps no permission to disclose withdrawn machine definitions.
use super::TopicMapError;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

pub(crate) async fn available(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    definition: Option<Uuid>,
    manifest: &Value,
    unavailable: &HashSet<Uuid>,
) -> Result<bool, TopicMapError> {
    let mut used: HashSet<Uuid> = definition.into_iter().collect();
    used.extend(crate::topic_map_core::definition_dependencies(manifest));
    if used.iter().any(|id| unavailable.contains(id)) {
        return Ok(false);
    }
    if let Some(result) = manifest["resultRef"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
    {
        // Public manifests intentionally omit propositions and selected discussion
        // summaries. Their private immutable task/result still owns dependencies.
        let row = sqlx::query("SELECT r.core_json,w.input_refs,COALESCE(dispatch.context,'{}'::jsonb) AS request_manifest FROM linggan_topic_research_result r JOIN linggan_topic_research_window w USING(window_ref) JOIN linggan_topic_research_run run USING(run_ref) LEFT JOIN linggan_topic_research_dispatch dispatch USING(invocation_ref) WHERE r.result_ref=$1 AND run.domain_ref=$2")
            .bind(result).bind(domain).fetch_optional(&mut **tx).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        for field in ["core_json", "input_refs", "request_manifest"] {
            used.extend(crate::topic_map_core::definition_dependencies(
                &row.get::<Value, _>(field),
            ));
        }
    }
    Ok(!used.iter().any(|id| unavailable.contains(id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn comparison_and_unassigned_neighbors_are_actual_definition_dependencies() {
        let (assigned, compared) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let used = crate::topic_map_core::definition_dependencies(
            &json!({"source":{"coverage":{"selectedDiscussions":[{
            "assignments":[{"definitionRef":assigned}],"comparedDefinitionRefs":[compared]}]}}}),
        );
        assert_eq!(used, HashSet::from([assigned, compared]));
    }
}
