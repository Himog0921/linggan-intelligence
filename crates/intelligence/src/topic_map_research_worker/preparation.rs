//! Source extraction and candidate-resolution request construction.
use super::*;

pub(super) async fn prepare(
    db: &Database,
    adapter: &PiAdapter,
    input: &ResearchInput,
    row: &sqlx::postgres::PgRow,
    input_limit: i64,
    output_limit: i32,
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
        prepared.prompt=json!({"contract":analysis::EXTRACT_CONTRACT,"inputTokenLimit":input_limit,"outputTokenLimit":output_limit,
            "input":{"domain":input.domain,"workRef":input.work.work_ref,"fragments":provider_fragments(input.fragments.iter()),
                "definitions":[],"commentStudy":provider_comments(&input.comment_study),"roleMetadata":input.role_metadata,
                "coverage":provider_coverage(&input.coverage),"comparisonWorkRefs":input.context_work_refs,
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
    include_parent_candidates(&mut prepared, &catalog);
    prepared.system = analysis::RESOLVE_SYSTEM.into();
    let batch_units = core::units(input, &draft);
    let cited: std::collections::HashSet<_> = batch_units
        .iter()
        .flat_map(|(_, d)| d.evidence.iter().map(|c| c.fragment_id.as_str()))
        .collect();
    let fragments: Vec<_> = input
        .fragments
        .iter()
        .filter(|f| {
            cited.contains(f.fragment_id.as_str()) || !topic_map_research::is_research_evidence(f)
        })
        .collect();
    prepared.prompt=json!({"contract":analysis::RESOLVE_CONTRACT,"inputTokenLimit":input_limit,"outputTokenLimit":output_limit,
        "input":{"domain":input.domain,"units":prepared.units.iter().map(|(id,d)|json!({"unitId":id,"discussion":d})).collect::<Vec<_>>(),
            "batchContext":batch_units.iter().map(|(id,d)|json!({"unitId":id,"statement":d.statement,"speakerRole":d.speaker_role,"evidenceRole":d.evidence_role,"evidence":d.evidence})).collect::<Vec<_>>(),
            "batchBoundary":"Only input.units may be assigned. batchContext supplies related and contrasting discussions for concept abstraction; its other units are not extra independent people or additional assignments.",
            "fragments":provider_fragments(fragments.into_iter()),"definitions":prepared.candidates,"recall":prepared.recall},
        "outputSchema":analysis::resolution_schema()}).to_string();
    prepared.draft = Some(draft);
    Ok(prepared)
}

fn provider_fragments<'a>(
    fragments: impl Iterator<Item = &'a linggan_evidence::creator_discovery::Fragment>,
) -> Vec<Value> {
    fragments
        .map(|f| {
            json!({"fragmentId":f.fragment_id,"field":f.field,
        "start":f.start,"end":f.end,"text":f.text})
        })
        .collect()
}

fn provider_comments(comments: &Value) -> Value {
    let mut comments = comments.clone();
    for comment in comments.as_array_mut().into_iter().flatten() {
        if let Some(fields) = comment.as_object_mut() {
            for key in [
                "sourceRef",
                "parentSourceRef",
                "sourceFragmentId",
                "parentFragmentId",
            ] {
                fields.remove(key);
            }
        }
    }
    comments
}

fn provider_coverage(coverage: &Value) -> Value {
    // Physical restoration maps stay in the frozen manifest. Repeating them in
    // the semantic request can exhaust its budget even for very short evidence.
    let mut projected = coverage.clone();
    if let Some(fields) = projected.as_object_mut() {
        for field in [
            "currentSources",
            "sourceHashes",
            "fragmentOrigins",
            "sourceFragmentIds",
            "sourceFragmentId",
            "windowKey",
            "configRef",
            "inputContract",
        ] {
            fields.remove(field);
        }
    }
    projected
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

fn include_parent_candidates(prepared: &mut Prepared, catalog: &Value) {
    let ordinary = prepared.candidates.as_array().cloned().unwrap_or_default();
    let mut selected = Vec::new();
    // Explicit maintenance definitions retain their frozen comparison obligation.
    for forced in prepared.recall["backfill"]["forcedDefinitionRefs"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(candidate) = ordinary.iter().find(|c| c["definitionRef"] == *forced) {
            selected.push(candidate.clone());
        }
    }
    // Reserve a small part of the comparison budget for reusable parent concepts.
    let parents: Vec<_> = catalog
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["conceptRole"] == "parent")
        .cloned()
        .collect();
    let query = prepared
        .units
        .iter()
        .map(|(_, d)| format!("{} {}", d.label, d.definition))
        .collect::<Vec<_>>()
        .join(" ");
    let scores = core::parent_recall_scores(&query, &parents);
    let mut ranked: Vec<_> = parents.into_iter().zip(scores).collect();
    ranked.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| a.0["topicRef"].as_str().cmp(&b.0["topicRef"].as_str()))
    });
    for (parent, _) in ranked.into_iter().take(2) {
        if selected.len() < 8 && !selected.iter().any(|c| c["topicRef"] == parent["topicRef"]) {
            selected.push(parent);
        }
    }
    for candidate in ordinary {
        if selected.len() < 8
            && !selected
                .iter()
                .any(|c| c["topicRef"] == candidate["topicRef"])
        {
            selected.push(candidate);
        }
    }
    prepared.candidates = json!(selected);
    prepared.recall["parentSelection"] = json!({"method":"bounded_lexical_parent_candidates","maximumCandidates":8,"similarityIsNotMembership":true});
}
