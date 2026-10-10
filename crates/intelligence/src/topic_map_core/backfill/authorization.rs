//! An explicit Start grants only its frozen work/definition set, on original budgets.
use crate::model_settings::ModelError;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(super) fn matching_authorization(
    scope: &Value,
    work: Uuid,
    definitions: &[Uuid],
) -> Option<Uuid> {
    if definitions.is_empty() {
        return None;
    }
    scope["backfillAuthorizations"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .find_map(|grant| {
            let works = grant["workRefs"].as_array()?;
            let refs = grant["definitionRefs"].as_array()?;
            if !works.contains(&json!(work))
                || !definitions.iter().all(|id| refs.contains(&json!(id)))
            {
                return None;
            }
            grant["requestRef"].as_str()?.parse().ok()
        })
}

pub(crate) fn task_is_explicitly_authorized(scope: &Value, recall: &Value, work: Uuid) -> bool {
    let backfill = &recall["backfill"];
    let Some(request) = backfill["authorizationRequestRef"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
    else {
        return false;
    };
    let Some(definitions) = backfill["forcedDefinitionRefs"]
        .as_array()
        .and_then(|refs| {
            refs.iter()
                .map(|r| r.as_str()?.parse().ok())
                .collect::<Option<Vec<Uuid>>>()
        })
    else {
        return false;
    };
    let works = if let Some(scope) = backfill.get("comparisonScopeWorkRefs") {
        let Some(works) = scope.as_array().and_then(|refs| {
            refs.iter()
                .map(|r| r.as_str()?.parse().ok())
                .collect::<Option<Vec<Uuid>>>()
        }) else {
            return false;
        };
        if works.is_empty()
            || works.len() > 10
            || !works.contains(&work)
            || works
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != works.len()
        {
            return false;
        }
        works
    } else {
        vec![work]
    };
    scope["backfillAuthorizations"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|grant| {
            grant["requestRef"] == json!(request)
                && works.iter().all(|work| {
                    matching_authorization(
                        &json!({"backfillAuthorizations":[grant]}),
                        *work,
                        &definitions,
                    ) == Some(request)
                })
        })
}

/// Freeze all current definition IDs in a finite <=10-work Start scope before
/// the background scanner runs. At most twelve original runs receive new grants;
/// each event still recalls <=240 discussions and queues <=12 source windows.
pub(crate) async fn authorize_scope(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    request: Uuid,
    works: &[Uuid],
) -> Result<Option<Uuid>, ModelError> {
    if works.is_empty() || works.len() > 10 {
        return Ok(None);
    }
    let definitions: Vec<Uuid> = sqlx::query_scalar("SELECT d.definition_ref FROM linggan_topic_definition d JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE b.domain_ref=$1 AND d.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=d.topic_ref) AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=d.topic_ref) ORDER BY d.definition_ref")
        .bind(domain).fetch_all(&mut **tx).await?;
    if definitions.is_empty() {
        return Ok(None);
    }
    let rows = sqlx::query(r#"
        SELECT DISTINCT run.run_ref,run.created_at
        FROM linggan_topic_map_research_task source
        JOIN linggan_topic_map_research_run run USING(run_ref)
        WHERE source.domain_ref=$1 AND source.work_public_ref=ANY($2)
          AND source.state='succeeded' AND source.distilled_json IS NOT NULL
          AND run.state IN ('queued','running','completed')
          AND EXISTS(
            SELECT 1 FROM linggan_topic_definition definition
            WHERE definition.definition_ref=ANY($3)
              AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(run.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                WHERE permission->'workRefs' ? source.work_public_ref::text AND permission->'definitionRefs' ? definition.definition_ref::text)
              AND (
                EXISTS(SELECT 1 FROM linggan_topic_map_backfill_queue q WHERE q.source_task_ref=source.task_ref
                  AND q.definition_ref=definition.definition_ref AND q.state='pending')
                OR (NOT EXISTS(SELECT 1 FROM linggan_topic_map_definition_scan scan WHERE scan.definition_ref=definition.definition_ref)
                  AND EXISTS(SELECT 1 FROM linggan_topic_map_unit_resolution prior
                    WHERE prior.task_ref=source.task_ref AND NOT EXISTS(
                      SELECT 1 FROM linggan_topic_map_unit_resolution compared WHERE compared.unit_ref=prior.unit_ref AND (
                        compared.candidate_manifest @> jsonb_build_object('topics',jsonb_build_array(jsonb_build_object('definitionRef',definition.definition_ref)))
                        OR EXISTS(SELECT 1 FROM linggan_topic_map_unit_assignment a WHERE a.resolution_ref=compared.resolution_ref AND a.definition_ref=definition.definition_ref)))))
              ))
        ORDER BY run.created_at DESC,run.run_ref LIMIT 13
    "#).bind(domain).bind(works).bind(&definitions).fetch_all(&mut **tx).await?;
    let partial = rows.len() > 12;
    let mut first = None;
    for row in rows.into_iter().take(12) {
        let run: Uuid = row.get("run_ref");
        let grant = json!({"requestRef":request,"workRefs":works,"definitionRefs":definitions,
            "workLimit":10,"originalRunLimit":12,"taskLimitPerDefinition":12,"discussionLimitPerDefinition":240,
            "definitionCount":definitions.len(),"scopeState":if partial{"bounded_partial"}else{"frozen"},
            "newRun":false,"budgetOwner":"original_run"});
        sqlx::query("UPDATE linggan_topic_map_research_run SET input_scope=jsonb_set(input_scope,'{backfillAuthorizations}',COALESCE(input_scope->'backfillAuthorizations','[]'::jsonb)||jsonb_build_array($2::jsonb)),updated_at=scope_001_now() WHERE run_ref=$1")
            .bind(run).bind(grant).execute(&mut **tx).await?;
        first.get_or_insert(run);
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_grant_cannot_expand_to_later_definitions_or_other_works() {
        let (request, work, definition) =
            (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let scope = json!({"backfillAuthorizations":[{"requestRef":request,"workRefs":[work],"definitionRefs":[definition]}]});
        let recall = json!({"backfill":{"authorizationRequestRef":request,"forcedDefinitionRefs":[definition]}});
        assert!(task_is_explicitly_authorized(&scope, &recall, work));
        assert!(!task_is_explicitly_authorized(
            &scope,
            &recall,
            Uuid::from_u128(4)
        ));
        assert!(!task_is_explicitly_authorized(
            &scope,
            &json!({"backfill":{"authorizationRequestRef":request,"forcedDefinitionRefs":[Uuid::from_u128(5)]}}),
            work
        ));
        assert!(!task_is_explicitly_authorized(
            &scope,
            &json!({"backfill":{"forcedDefinitionRefs":[definition]}}),
            work
        ));
        let compare = json!({"backfill":{"authorizationRequestRef":request,"forcedDefinitionRefs":[definition],"comparisonScopeWorkRefs":[work,Uuid::from_u128(4)]}});
        assert!(!task_is_explicitly_authorized(&scope, &compare, work));
    }
}
