use super::*;

// A summary is a checkpoint, not a permanent prohibition. Accepted discussion
// changes or a new boundary can make an original run eligible again. This scan
// only rebuilds a bounded input; the semantic input hash still gates dispatch.
pub(super) const CANDIDATE_SQL: &str = r#"
    WITH eligible AS (
        SELECT r.run_ref,r.domain_ref,r.config_ref,r.input_scope,r.trigger,r.state,r.created_at,p.automatic_enabled
        FROM linggan_topic_map_research_run r
        JOIN linggan_topic_map_research_policy p ON p.domain_ref=r.domain_ref
        JOIN observation_domain d ON d.domain_ref=r.domain_ref
        WHERE r.method_version=$1 AND r.state IN ('queued','running','completed')
            AND ($2::uuid IS NULL OR r.run_ref=$2)
            AND p.status='active' AND d.status='active' AND jsonb_typeof(r.input_scope)='object'
    ), candidates AS (
        SELECT e.*,NULL::uuid AS comparison_work,
            ARRAY(SELECT jsonb_array_elements_text(e.input_scope->'workRefs')) AS requested_refs,
            COALESCE(e.input_scope->'comparison','{}'::jsonb) AS previous
        FROM eligible e WHERE e.trigger='on_demand'
            AND jsonb_array_length(CASE WHEN jsonb_typeof(e.input_scope->'workRefs')='array'
                THEN e.input_scope->'workRefs' ELSE '[]'::jsonb END) BETWEEN 1 AND 10
        UNION ALL
        SELECT e.*,w.work_public_ref AS comparison_work,ARRAY[w.work_public_ref::text] AS requested_refs,
            COALESCE(e.input_scope#>ARRAY['workComparisons',w.work_public_ref::text],'{}'::jsonb) AS previous
        FROM eligible e CROSS JOIN LATERAL (
            SELECT DISTINCT t.work_public_ref FROM linggan_topic_map_research_task t
            WHERE t.run_ref=e.run_ref AND t.phase IN ('extract','resolve')
                AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison'
        ) w WHERE e.trigger IN ('historical','incremental') AND (e.automatic_enabled
            OR EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(e.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                WHERE permission->'workRefs' ? w.work_public_ref::text))
    ), snapshots AS (
        SELECT c.*,sources.source_dependencies,definitions.refs AS definition_refs,
            md5(sources.discussions::text||definitions.refs::text) AS dependency_fingerprint
        FROM candidates c CROSS JOIN LATERAL (
            SELECT COALESCE(jsonb_agg(jsonb_build_object('taskRef',t.task_ref,'workRef',t.work_public_ref,
                    'backfill',t.recall_manifest->'backfill') ORDER BY t.task_ref),'[]'::jsonb) AS source_dependencies,
                COALESCE(jsonb_agg(jsonb_build_array(t.task_ref,t.input_hash,t.resolutions_json)
                    ORDER BY t.task_ref),'[]'::jsonb) AS discussions
            FROM (
                SELECT DISTINCT ON(t.work_public_ref,t.input_hash) t.*
                FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref)
                WHERE t.domain_ref=c.domain_ref AND t.work_public_ref::text=ANY(c.requested_refs)
                    AND r.config_ref=c.config_ref AND r.method_version=$1
                    AND t.phase IN ('extract','resolve') AND t.state IN ('succeeded','no_signal','insufficient','stale','failed','stopped','unknown_dispatch')
                    AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison'
                    AND jsonb_array_length(t.resolutions_json)>0
                ORDER BY t.work_public_ref,t.input_hash,t.updated_at DESC,t.task_ref DESC
            ) t
        ) sources CROSS JOIN LATERAL (
            SELECT COALESCE(jsonb_agg(d.definition_ref ORDER BY d.definition_ref),'[]'::jsonb) AS refs
            FROM linggan_topic_definition d
            JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true
            WHERE b.domain_ref=c.domain_ref AND d.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=d.topic_ref)
                AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=d.topic_ref)
        ) definitions
    )
    SELECT c.* FROM snapshots c
    WHERE c.previous->>'dependencyFingerprint' IS DISTINCT FROM c.dependency_fingerprint
        AND (c.automatic_enabled OR (c.trigger='on_demand' AND NOT c.previous ? 'state'
                AND COALESCE(c.input_scope->>'backfillReopened','false')<>'true')
            OR (c.trigger='on_demand' AND c.state IN ('queued','running')
                AND COALESCE(c.input_scope->>'backfillReopened','false')<>'true')
            OR (
                EXISTS(SELECT 1 FROM jsonb_array_elements(c.source_dependencies) dep
                    WHERE NOT COALESCE(c.previous->'sourceTaskRefs','[]'::jsonb) ? (dep->>'taskRef'))
                AND EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(c.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                    WHERE permission->'workRefs' @> to_jsonb(c.requested_refs)
                        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(c.source_dependencies) dep
                            WHERE NOT COALESCE(c.previous->'sourceTaskRefs','[]'::jsonb) ? (dep->>'taskRef')
                                AND NOT permission->'definitionRefs' @> (dep#>'{backfill,forcedDefinitionRefs}')))
                AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(c.source_dependencies) dep
                    WHERE NOT COALESCE(c.previous->'sourceTaskRefs','[]'::jsonb) ? (dep->>'taskRef')
                        AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(c.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                            WHERE permission->>'requestRef'=dep#>>'{backfill,authorizationRequestRef}'
                                AND permission->'workRefs' @> to_jsonb(c.requested_refs)
                                AND jsonb_array_length(COALESCE(dep#>'{backfill,forcedDefinitionRefs}','[]'::jsonb))>0
                                AND permission->'definitionRefs' @> (dep#>'{backfill,forcedDefinitionRefs}')))
            ))
        AND NOT EXISTS (
            SELECT 1 FROM linggan_topic_map_research_task t WHERE t.run_ref=c.run_ref
                AND (c.comparison_work IS NULL OR t.work_public_ref=c.comparison_work)
                AND t.state IN ('queued','running')
                AND (t.phase='compare' OR COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}')='comparison')
        ) AND NOT EXISTS (
            SELECT 1 FROM linggan_topic_map_research_task t WHERE t.domain_ref=c.domain_ref
                AND t.work_public_ref::text=ANY(c.requested_refs)
                AND t.phase IN ('extract','resolve') AND t.state IN ('queued','running')
                AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison'
        ) ORDER BY (c.trigger<>'on_demand'),c.created_at,c.run_ref,c.comparison_work LIMIT 1
"#;

pub(super) fn may_refresh(
    input_scope: &Value,
    previous: &Value,
    dependencies: &Value,
    requested: &[Uuid],
    automatic: bool,
    current_on_demand: bool,
) -> bool {
    if automatic || current_on_demand {
        return true;
    }
    renewal_authorization(input_scope, previous, dependencies, requested).is_some()
}

pub(super) fn current_on_demand(
    trigger: &str,
    state: &str,
    scope: &Value,
    previous: &Value,
) -> bool {
    trigger == "on_demand"
        && scope["backfillReopened"] != true
        && (previous.get("state").is_none() || ["queued", "running"].contains(&state))
}

pub(super) fn renewal_authorization(
    input_scope: &Value,
    previous: &Value,
    dependencies: &Value,
    requested: &[Uuid],
) -> Option<Value> {
    let old: BTreeSet<_> = previous["sourceTaskRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let changed: Vec<_> = dependencies
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["taskRef"].as_str().is_none_or(|id| !old.contains(id)))
        .collect();
    if changed.is_empty() {
        return None;
    }
    // A new explicit selection must cover every side of a renewed comparison,
    // not merely the work whose source window happened to be reassigned.
    let grants: Vec<_> = input_scope["backfillAuthorizations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| {
            requested.iter().all(|work| {
                g["workRefs"]
                    .as_array()
                    .is_some_and(|refs| refs.contains(&json!(work)))
            })
        })
        .cloned()
        .collect();
    let allowed = json!({"backfillAuthorizations":grants});
    if !changed.iter().all(|dependency| {
        let Some(work) = dependency["workRef"]
            .as_str()
            .and_then(|id| id.parse().ok())
        else {
            return false;
        };
        requested.contains(&work)
            && crate::topic_map_core::backfill::task_is_explicitly_authorized(
                &allowed,
                &json!({"backfill":dependency["backfill"]}),
                work,
            )
    }) {
        return None;
    }
    let definitions: BTreeSet<_> = changed
        .iter()
        .flat_map(|d| {
            d["backfill"]["forcedDefinitionRefs"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(Value::as_str)
        .collect();
    let grant = grants.iter().rev().find(|grant| {
        grant["requestRef"]
            .as_str()
            .is_some_and(|id| id.parse::<Uuid>().is_ok())
            && definitions.iter().all(|definition| {
                grant["definitionRefs"]
                    .as_array()
                    .is_some_and(|refs| refs.contains(&json!(definition)))
            })
    })?;
    Some(
        json!({"backfill":{"authorizationRequestRef":grant["requestRef"],
        "forcedDefinitionRefs":definitions,"comparisonScopeWorkRefs":requested,
        "reason":"comparison_after_authorized_reassignment"}}),
    )
}
