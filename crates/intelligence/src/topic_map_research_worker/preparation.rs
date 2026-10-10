//! Source extraction and candidate-resolution request construction.
use super::*;

pub(super) async fn prepare(
    db: &Database,
    adapter: &PiAdapter,
    input: &ResearchInput,
    row: &sqlx::postgres::PgRow,
    input_limit: i64,
) -> Result<Prepared, ModelError> {
    let phase: String = row.get("phase");
    let mut prepared = Prepared {
        system: analysis::SYSTEM.into(),
        prompt: String::new(),
        draft: None,
        units: vec![],
        candidates: json!([]),
        recall: json!({}),
        cursor: row.get::<i32, _>("resolution_cursor") as usize,
        total_units: 0,
        definition_unavailable: false,
        eligible_definitions: vec![],
        previous: row
            .get::<Value, _>("resolutions_json")
            .as_array()
            .cloned()
            .unwrap_or_default(),
    };
    if phase == "extract" || phase == "compare" {
        prepared.prompt=json!({"contract":analysis::EXTRACT_CONTRACT,"inputTokenLimit":input_limit,
            "input":{"domain":input.domain,"workRef":input.work.work_ref,"fragments":input.fragments,
                "definitions":[],"commentStudy":input.comment_study,"roleMetadata":input.role_metadata,
                "coverage":input.coverage,"comparisonWorkRefs":input.context_work_refs,
                "comparisonBoundary":if phase=="compare" {"Only selected cited evidence from the listed works is supplied. Distinguish each work and author/commenter role. Compare only these observations; coverage is partial and is not audience prevalence."} else {"A source window is partial evidence. Other selected works are not supplied here; cross-work claims remain unknown."}},
            "outputSchema":analysis::output_schema()}).to_string();
        return Ok(prepared);
    }
    let draft: ResearchOutput = serde_json::from_value(
        row.get::<Option<Value>, _>("distilled_json")
            .ok_or(ModelError::InvalidOutput)?,
    )
    .map_err(|_| ModelError::InvalidOutput)?;
    let previous_recall: Value = row.get("recall_manifest");
    let backfill = previous_recall.get("backfill").cloned();
    let units: Vec<_> = core::units(input, &draft)
        .into_iter()
        .filter(|(id, _)| {
            backfill.as_ref().is_none_or(|b| {
                b["unitKeys"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|key| key.as_str() == Some(id)))
            })
        })
        .collect();
    prepared.total_units = units.len();
    if prepared.cursor >= units.len() {
        return Err(ModelError::InvalidOutput);
    }
    let domain = input.domain["domainRef"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or(ModelError::Source)?;
    // Capture the raw identity stamp before loading candidates or waiting for
    // embeddings. A later catalog cannot certify candidates prepared earlier.
    let catalog_version = core::catalog_version(db, domain).await?;
    let catalog = core::catalog(db, domain).await?;
    prepared.eligible_definitions = catalog
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["definitionRef"].as_str()?.parse().ok())
        .collect();
    // A fresh concept is visible to the very next discussion. This also prevents two synonymous
    // new candidates in the same batch from being created without comparing their boundaries.
    let count = 1;
    prepared.units = units
        .into_iter()
        .skip(prepared.cursor)
        .take(count)
        .collect();
    (prepared.candidates, prepared.recall) =
        core::recall_topics(db, adapter, &prepared.units, &catalog).await?;
    prepared.recall["catalogFingerprint"] = json!(catalog_version);
    apply_backfill(&mut prepared, &catalog, backfill);
    prepared.system = analysis::RESOLVE_SYSTEM.into();
    let cited: std::collections::HashSet<_> = prepared
        .units
        .iter()
        .flat_map(|(_, d)| d.evidence.iter().map(|c| c.fragment_id.as_str()))
        .collect();
    let fragments: Vec<_> = input
        .fragments
        .iter()
        .filter(|f| cited.contains(f.fragment_id.as_str()) || f.field == "parent_comment_context")
        .collect();
    prepared.prompt=json!({"contract":analysis::RESOLVE_CONTRACT,"inputTokenLimit":input_limit,
        "input":{"domain":input.domain,"units":prepared.units.iter().map(|(id,d)|json!({"unitId":id,"discussion":d})).collect::<Vec<_>>(),
            "fragments":fragments,"definitions":prepared.candidates,"recall":prepared.recall},
        "outputSchema":analysis::resolution_schema()}).to_string();
    prepared.draft = Some(draft);
    Ok(prepared)
}
fn apply_backfill(prepared: &mut Prepared, catalog: &Value, backfill: Option<Value>) {
    if let Some(backfill) = backfill {
        prepared.definition_unavailable = backfill["forcedDefinitionRefs"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|definition| {
                !catalog
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|t| t["definitionRef"] == *definition)
            });
        for definition in backfill["forcedDefinitionRefs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(topic) = catalog
                .as_array()
                .into_iter()
                .flatten()
                .find(|t| t["definitionRef"] == *definition)
                && !prepared
                    .candidates
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|c| c["definitionRef"] == *definition)
                && let Some(candidates) = prepared.candidates.as_array_mut()
            {
                candidates.push(topic.clone());
            }
        }
        prepared.recall["backfill"] = backfill;
    }
}
pub(super) fn catalog_fingerprint(catalog: &Value) -> String {
    linggan_evidence::creator_discovery::hash(
        &json!(
            catalog
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"]}))
                .collect::<Vec<_>>()
        )
        .to_string(),
    )
}
