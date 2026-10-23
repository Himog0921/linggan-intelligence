//! Only frozen windows that can still be reconstructed are projected.
use super::*;
use crate::topic_map_research_analysis::{self as analysis, ResearchOutput};
use sqlx::{Row, postgres::PgRow};

const RESULTS: &str = r#"
    SELECT DISTINCT ON(r.domain_ref,r.work_public_ref,r.input_hash)
        r.work_public_ref,r.domain_ref,r.output_json,r.core_json,r.method_version,
        r.result_ref,r.created_at::text AS time,request.request_manifest AS input_refs,
        t.task_ref,run.config_ref
    FROM linggan_topic_map_research_result r
    JOIN linggan_topic_map_research_request request USING(invocation_ref)
    JOIN linggan_topic_map_research_task t ON t.task_ref=request.task_ref
    JOIN linggan_topic_map_research_run run ON run.run_ref=request.run_ref
    WHERE ($1::uuid IS NULL OR r.domain_ref=$1)
    ORDER BY r.domain_ref,r.work_public_ref,r.input_hash,r.created_at DESC,r.result_ref DESC
"#;

const PARTIALS: &str = r#"
    SELECT DISTINCT ON(t.domain_ref,t.work_public_ref,t.input_hash)
        t.work_public_ref,t.domain_ref,t.distilled_json AS output_json,
        jsonb_build_object('units',t.resolutions_json) AS core_json,run.method_version,
        NULL::uuid AS result_ref,t.updated_at::text AS time,t.input_refs,t.task_ref,run.config_ref
    FROM linggan_topic_map_research_task t
    JOIN linggan_topic_map_research_run run USING(run_ref)
    WHERE ($1::uuid IS NULL OR t.domain_ref=$1) AND t.phase='resolve'
        AND run.method_version='topic-map.research.v6'
        AND jsonb_typeof(t.resolutions_json)='array' AND jsonb_array_length(t.resolutions_json)>0
        AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison'
        AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_research_request request
            JOIN linggan_topic_map_research_result result USING(invocation_ref)
            WHERE request.task_ref=t.task_ref)
    ORDER BY t.domain_ref,t.work_public_ref,t.input_hash,t.updated_at DESC,t.task_ref DESC
"#;

pub(super) async fn load_windows(
    db: &Database,
    q: &TopicMapQuery,
    works: &[TopicMapWork],
    topics: &[TopicMapTopic],
) -> Result<(HashSet<Uuid>, HashMap<Uuid, Vec<Window>>), TopicMapError> {
    let mut rows = sqlx::query(RESULTS)
        .bind(q.domain_ref)
        .fetch_all(db.pool())
        .await?;
    rows.extend(
        sqlx::query(PARTIALS)
            .bind(q.domain_ref)
            .fetch_all(db.pool())
            .await?,
    );
    // A work may have qualified comments while the author's original is unavailable.
    let visible: HashSet<_> = works.iter().map(|w| w.work_ref).collect();
    rows.retain(|row| {
        !projection_targets(row.get("work_public_ref"), &row.get("input_refs"), &visible).is_empty()
    });
    let mut scopes = HashMap::<(Uuid, Uuid, String), BTreeSet<Uuid>>::new();
    for row in &rows {
        let manifest: Value = row.get("input_refs");
        let Some(as_of) = topic_map_research::manifest_as_of(&manifest) else {
            continue;
        };
        // Visibility limits projection, never the frozen participants used for qualification.
        scopes
            .entry((
                row.get("domain_ref"),
                row.get("config_ref"),
                as_of.to_owned(),
            ))
            .or_default()
            .extend(topic_map_research::manifest_source_work_refs(
                row.get("work_public_ref"),
                &manifest,
            ));
    }
    let (unavailable, inputs) =
        crate::topic_map_core::qualify_definitions_and_inputs(db, q.domain_ref, &scopes)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let mut windows = HashMap::<Uuid, Vec<Window>>::new();
    let mut window_counts = HashMap::<(Uuid, Uuid, Uuid, usize), usize>::new();
    let allowed: Vec<_> = topics.iter().map(|t| t.topic_ref).collect();
    for row in rows {
        let work: Uuid = row.get("work_public_ref");
        let targets = projection_targets(work, &row.get("input_refs"), &visible);
        if targets.is_empty() {
            continue;
        }
        let domain: Uuid = row.get("domain_ref");
        let config: Uuid = row.get("config_ref");
        let manifest: Value = row.get("input_refs");
        let Some(as_of) = topic_map_research::manifest_as_of(&manifest) else {
            continue;
        };
        let Some(frozen) = inputs.frozen.get(&(domain, config, as_of.to_owned())) else {
            continue;
        };
        let Some(current) = inputs.current.get(&(domain, config)) else {
            continue;
        };
        if let Some(window) = restore_row(
            &row,
            frozen,
            current,
            &allowed,
            &unavailable,
            &mut window_counts,
        ) {
            // Reconstruct once using the original primary and complete frozen scope.
            // Visibility selects readers, never the evidence used to qualify a result.
            for target in targets {
                windows.entry(target).or_default().push(window.clone());
            }
        }
    }
    Ok((unavailable, windows))
}

pub(super) fn projection_targets(
    primary: Uuid,
    wrapper: &Value,
    visible: &HashSet<Uuid>,
) -> Vec<Uuid> {
    let manifest = wrapper.get("source").unwrap_or(wrapper);
    let targets = if manifest["coverage"]["kind"] == "comparison" {
        ComparisonScope::from_manifest(manifest, primary)
            .map(|scope| scope.selected_work_refs)
            .unwrap_or_default()
    } else {
        vec![primary]
    };
    targets
        .into_iter()
        .filter(|work| visible.contains(work))
        .collect()
}

fn restore_row(
    row: &PgRow,
    all: &[ResearchInput],
    current_inventory: &[ResearchInput],
    allowed: &[Uuid],
    unavailable: &HashSet<Uuid>,
    window_counts: &mut HashMap<(Uuid, Uuid, Uuid, usize), usize>,
) -> Option<Window> {
    let work: Uuid = row.get("work_public_ref");
    let source = current_inventory.iter().find(|i| i.work.work_ref == work);
    let wrapper: Value = row.get("input_refs");
    let manifest = wrapper.get("source").unwrap_or(&wrapper).clone();
    if !comparison_definitions_current(&manifest, unavailable) {
        return None;
    }
    let output: Value = row.get("output_json");
    let mut current = topic_map_research::restore_scoped_window(all, work, &manifest)?;
    let current_coverage_eligible =
        source.is_some_and(|source| sources_match_current(&current, source));
    if manifest["coverage"]["kind"] != "comparison" {
        // Current qualified inventory is the denominator, including newly added sources.
        current.coverage["sourceChars"] = source
            .map(|source| source.coverage["sourceChars"].clone())
            .unwrap_or(json!(0));
        current.coverage["windowCount"] = json!(
            *window_counts
                .entry((row.get("domain_ref"), row.get("config_ref"), work, 0))
                .or_insert_with(|| source
                    .map(topic_map_research::research_window_count)
                    .unwrap_or(0))
        );
    }
    let typed: ResearchOutput = serde_json::from_value(output.clone()).ok()?;
    analysis::validate_output(&typed, &current.fragments, allowed).ok()?;
    let result: Option<Uuid> = row.get("result_ref");
    let core: Value = row.get("core_json");
    let (output, core) = if result.is_none() {
        partial::payload(&typed, &current, &core["units"])?
    } else {
        // Reject malformed stored unit identities without discarding valid source analysis.
        let accepted = partial::accepted_units(&typed, &current, &core["units"]);
        (
            output,
            json!({"units":accepted.into_iter().map(|(_,u)|u).collect::<Vec<_>>()}),
        )
    };
    Some(Window {
        result,
        task: row.get("task_ref"),
        method: row.get("method_version"),
        time: row.get("time"),
        input: current,
        manifest,
        output,
        core,
        current_coverage_eligible,
    })
}

fn sources_match_current(frozen: &ResearchInput, current: &ResearchInput) -> bool {
    frozen.fragments.iter().all(|source| {
        let origin = frozen.coverage["fragmentOrigins"][&source.fragment_id].as_str();
        current.fragments.iter().any(|now| {
            origin == Some(now.fragment_id.as_str())
                && source.source_ref == now.source_ref
                && source.source_version == now.source_version
                && source.reading_ref == now.reading_ref
                && source.unit_ref == now.unit_ref
                && source.start == now.start
                && source.end == now.end
                && source.text == now.text
                && source.speaker == now.speaker
                && source.structure == now.structure
                && source.issues == now.issues
        })
    })
}

pub(super) fn comparison_definitions_current(
    manifest: &Value,
    unavailable: &HashSet<Uuid>,
) -> bool {
    if manifest["coverage"]["kind"] != "comparison" {
        return true;
    }
    !manifest["coverage"]["selectedDiscussions"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(units::definition_dependencies)
        .any(|id| unavailable.contains(&id))
}
