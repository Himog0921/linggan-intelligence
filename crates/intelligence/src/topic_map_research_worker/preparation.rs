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
        rule_feedback_unavailable: false,
        eligible_definitions: vec![],
        review: None,
        review_packet: Value::Null,
        review_unavailable: false,
        review_catalog_changed: false,
        previous: row
            .get::<Value, _>("resolutions_json")
            .as_array()
            .cloned()
            .unwrap_or_default(),
    };
    if super::semantic::prepare_review(db, input, row, &mut prepared, input_limit, output_limit)
        .await?
    {
        return Ok(prepared);
    }
    if phase == "extract" || phase == "compare" {
        prepared.system.push_str("\n运输约定：上述字段名描述展开后的业务语义。实际只返回 topic-map.extract.quotes.v1 的 outputSchema 短键JSON，不返回业务长键；outputFieldNames 给出映射。证据只包含 f（冻结 fragmentIndex）与 q（逐字短引句），绝不返回s/e或长键；短引句须在指定片段中完整且唯一出现，多个不连续句可分别引用，不能拼接成不存在的引句。评论 fragmentIndex 与 parentFragmentIndexes 对应同一冻结编号；父上下文必须和对应主评论共同引用。所有业务字段及低频讨论均保留，短键只减少运输重复。");
        prepared.prompt=json!({"contract":analysis::COMPACT_EXTRACT_CONTRACT,"inputTokenLimit":input_limit,"outputTokenLimit":output_limit,
            "input":{"domain":input.domain,"workRef":input.work.work_ref,"asOf":input.coverage["asOf"],"fragments":compact_provider_fragments(&input.fragments, input.work.work_ref, &input.coverage),
                "definitions":[],"commentStudy":compact_provider_comments(&input.comment_study, &input.fragments),"roleMetadata":input.role_metadata,
                "coverage":compact_provider_coverage(&input.coverage, &input.fragments),"comparisonWorkRefs":input.context_work_refs,
                "comparisonBoundary":if phase=="compare" {"Selected discussions include full cited blocks and complete note context from the listed works. Distinguish each work and author/commenter role. Compare only these observations; coverage is partial and is not audience prevalence."} else {"This packet contains the complete note or selected comments with their complete note and parent context. Other works are not supplied here; cross-work claims remain unknown."}},
            "outputFieldNames":analysis::compact_field_names(),
            "outputSchema":analysis::compact_output_schema()}).to_string();
        add_rule_guidance(db, row, input, &mut prepared).await?;
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
        "input":{"domain":input.domain,"asOf":input.coverage["asOf"],"units":prepared.units.iter().map(|(id,d)|json!({"unitId":id,"discussion":d})).collect::<Vec<_>>(),
            "batchContext":batch_units.iter().map(|(id,d)|json!({"unitId":id,"statement":d.statement,"speakerRole":d.speaker_role,"evidenceRole":d.evidence_role,"evidence":d.evidence})).collect::<Vec<_>>(),
            "batchBoundary":"Only input.units may be assigned. batchContext supplies related and contrasting discussions for concept abstraction; its other units are not extra independent people or additional assignments.",
            "fragments":provider_fragments(fragments.into_iter()),"definitions":prepared.candidates,"recall":prepared.recall},
        "outputSchema":analysis::resolution_schema()}).to_string();
    add_rule_guidance(db, row, input, &mut prepared).await?;
    prepared.draft = Some(draft);
    Ok(prepared)
}

async fn add_rule_guidance(
    db: &Database,
    row: &sqlx::postgres::PgRow,
    input: &ResearchInput,
    prepared: &mut Prepared,
) -> Result<(), ModelError> {
    let mut prompt: Value =
        serde_json::from_str(&prepared.prompt).map_err(|_| ModelError::InvalidOutput)?;
    let phase: String = row.get("phase");
    if phase == "resolve" {
        prompt["candidateComparisonChecklist"] = json!(prepared.candidates.as_array().into_iter().flatten()
            .map(|candidate| json!({"topicRef":candidate["topicRef"],"definitionRef":candidate["definitionRef"]})).collect::<Vec<_>>());
        prompt["strictFieldGuidance"] = json!(
            "new 的 relations 必须逐一覆盖 candidateComparisonChecklist 每一个精确候选。matched 使用明确已有主题匹配；equivalent 不能创建新主题。所有纳入与排除条件数组各至少一条非空标准。"
        );
    } else {
        prompt["strictFieldGuidance"] = json!(
            "所有字段必须满足 outputSchema 类型：讨论纳入 i 和排除 x 各至少一条非空条件；angles 的 t 标题为非空字符串，不能是 null。journey 的 m 不是 unclear 时必须在 e 给实际有效引用；没有旅程依据保持 unclear。空数组只用于 schema 允许 minItems=0 的字段。不补造引用，不因反馈改变原声身份。"
        );
    }
    match super::feedback::previous_feedback(db, row, input).await? {
        super::feedback::RuleFeedback::Available(feedback) => {
            prompt["validationFeedback"] = feedback
        }
        super::feedback::RuleFeedback::Unavailable => prepared.rule_feedback_unavailable = true,
        super::feedback::RuleFeedback::NotRequired => {}
    }
    prepared.prompt = prompt.to_string();
    Ok(())
}

fn provider_fragments<'a>(
    fragments: impl Iterator<Item = &'a linggan_evidence::creator_discovery::Fragment>,
) -> Vec<Value> {
    fragments
        .map(|f| {
            json!({"fragmentId":f.fragment_id,"blockRef":f.fragment_id.strip_suffix(".context").unwrap_or(&f.fragment_id),"sourceRef":f.source_ref,"sourceVersion":f.source_version,"readingRef":f.reading_ref,"unitRef":f.unit_ref,"field":f.field,
        "start":f.start,"end":f.end,"content":f.model_content(),"speaker":f.speaker,"issues":f.issues})
        })
        .collect()
}

fn compact_provider_fragments(
    fragments: &[linggan_evidence::creator_discovery::Fragment],
    primary_work: Uuid,
    coverage: &Value,
) -> Vec<Value> {
    fragments.iter().enumerate().map(|(index, f)| {
        let owner = coverage["fragmentWorkRefs"].get(&f.fragment_id).cloned()
            .unwrap_or_else(|| if coverage["kind"] == "comparison" { Value::Null } else { json!(primary_work) });
        json!({"fragmentIndex":index,"blockRef":f.fragment_id.strip_suffix(".context").unwrap_or(&f.fragment_id),"sourceRef":f.source_ref,"sourceVersion":f.source_version,"readingRef":f.reading_ref,"unitRef":f.unit_ref,"workRef":owner,"field":f.field,"start":f.start,"end":f.end,"content":f.model_content(),"speaker":f.speaker,"issues":f.issues})
    }).collect()
}

fn compact_provider_coverage(
    coverage: &Value,
    fragments: &[linggan_evidence::creator_discovery::Fragment],
) -> Value {
    fn reindex(
        value: &Value,
        fragments: &[linggan_evidence::creator_discovery::Fragment],
    ) -> Value {
        match value {
            Value::Object(fields) => {
                Value::Object(
                    fields
                        .iter()
                        .map(|(key, child)| {
                            if key == "fragmentId" {
                                (
                                    "fragmentIndex".into(),
                                    json!(fragments.iter().position(
                                        |f| child.as_str() == Some(f.fragment_id.as_str())
                                    )),
                                )
                            } else {
                                (key.clone(), reindex(child, fragments))
                            }
                        })
                        .collect(),
                )
            }
            Value::Array(values) => json!(
                values
                    .iter()
                    .map(|value| reindex(value, fragments))
                    .collect::<Vec<_>>()
            ),
            _ => value.clone(),
        }
    }
    let mut projected = provider_coverage(coverage);
    if let Some(fields) = projected.as_object_mut() {
        fields.remove("fragmentWorkRefs");
    }
    reindex(&projected, fragments)
}

fn compact_provider_comments(
    comments: &Value,
    fragments: &[linggan_evidence::creator_discovery::Fragment],
) -> Value {
    let mut comments = provider_comments(comments);
    for comment in comments.as_array_mut().into_iter().flatten() {
        if let Some(fields) = comment.as_object_mut() {
            if let Some(id) = fields.remove("fragmentId") {
                fields.insert(
                    "fragmentIndex".into(),
                    json!(
                        fragments
                            .iter()
                            .position(|f| Some(f.fragment_id.as_str()) == id.as_str())
                    ),
                );
            }
            if let Some(ids) = fields.remove("parentFragmentIds") {
                fields.insert(
                    "parentFragmentIndexes".into(),
                    json!(
                        ids.as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|id| {
                                fragments
                                    .iter()
                                    .position(|f| Some(f.fragment_id.as_str()) == id.as_str())
                            })
                            .collect::<Vec<_>>()
                    ),
                );
            }
        }
    }
    comments
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

#[cfg(test)]
mod tests {
    use super::*;
    use linggan_evidence::creator_discovery::Fragment;

    fn fragment(id: &str, field: &str, start: usize, text: &str) -> Fragment {
        Fragment {
            fragment_id: id.into(),
            structure: json!({"ref":id,"kind":"paragraph","text":text,"speaker":if field.contains("comment") {"other"} else {"author"}}),
            source_ref: Uuid::from_u128(1),
            field: field.into(),
            source_version: "synthetic.v1".into(),
            start,
            end: start + text.chars().count(),
            text: text.into(),
            speaker: if field.contains("comment") {
                "other"
            } else {
                "author"
            }
            .into(),
            ..Fragment::default()
        }
    }

    #[test]
    fn compact_input_indexes_keep_comment_parent_links_and_absolute_positions() {
        let fragments = vec![
            fragment(
                "synthetic.reply.chars.1200.1220",
                "unresearched_comment",
                1200,
                "😀尾部的新障碍仍需保留",
            ),
            fragment(
                "synthetic.parent.chars.8.20",
                "parent_comment_context",
                8,
                "请说明什么地方卡住",
            ),
            fragment(
                "synthetic.work.context",
                "work_context:body",
                5000,
                "作者实践建议",
            ),
        ];
        let projected = compact_provider_fragments(&fragments, Uuid::from_u128(1), &json!({}));
        assert_eq!(projected.len(), fragments.len());
        for (index, source) in fragments.iter().enumerate() {
            assert_eq!(projected[index]["fragmentIndex"], index);
            assert_eq!(projected[index]["start"], source.start);
            assert_eq!(projected[index]["end"], source.end);
            assert_eq!(projected[index]["content"]["text"], source.text);
            assert!(projected[index].get("text").is_none());
            assert_eq!(projected[index]["field"], source.field);
            assert!(projected[index].get("fragmentId").is_none());
        }
        let comments = json!([{"fragmentId":fragments[0].fragment_id,
            "sourceRef":"private-source","sourceFragmentId":"physical-original",
            "parentFragmentId":"physical-parent", "parentFragmentIds":[fragments[1].fragment_id],
            "parentSourceRef":"private-parent-source", "role":"eligible_user_comment", "contextOnly":false}]);
        let projected = compact_provider_comments(&comments, &fragments);
        assert_eq!(projected[0]["fragmentIndex"], 0);
        assert_eq!(projected[0]["parentFragmentIndexes"], json!([1]));
        assert_eq!(projected[0]["role"], "eligible_user_comment");
        assert_eq!(projected[0]["contextOnly"], false);
        assert!(!projected.to_string().contains("private-source"));
        assert!(!projected.to_string().contains("physical-original"));
        assert!(!projected.to_string().contains("synthetic.reply"));
        assert_eq!(comments[0]["fragmentId"], fragments[0].fragment_id);
    }

    #[test]
    fn compact_comparison_preserves_each_work_owner_and_selected_citation() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let fragments = vec![
            fragment("synthetic.work.a", "body", 900, "甲的建议"),
            fragment("synthetic.work.b", "body", 1400, "乙的不同建议"),
        ];
        let coverage = json!({"kind":"comparison","fragmentWorkRefs":{
            "synthetic.work.a":first,"synthetic.work.b":second},
            "selectedDiscussions":[{"workRef":second,"evidence":[{"fragmentId":"synthetic.work.b","start":1400,"end":1406}]}]});
        let projected = compact_provider_fragments(&fragments, first, &coverage);
        assert_eq!(projected[0]["workRef"], json!(first));
        assert_eq!(projected[1]["workRef"], json!(second));
        let projected = compact_provider_coverage(&coverage, &fragments);
        assert!(projected.get("fragmentWorkRefs").is_none());
        assert_eq!(
            projected["selectedDiscussions"][0]["evidence"][0]["fragmentIndex"],
            1
        );
        assert_eq!(
            projected["selectedDiscussions"][0]["evidence"][0]["start"],
            1400
        );
        assert_eq!(
            coverage["selectedDiscussions"][0]["evidence"][0]["fragmentId"],
            "synthetic.work.b"
        );
    }
}
