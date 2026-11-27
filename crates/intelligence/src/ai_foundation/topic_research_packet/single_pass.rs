//! One model response projects author Core and assigns concepts. Validation remains deterministic.
use super::*;
use crate::topic_map_research_analysis::{
    ConceptDefinition, ProposedParent, ProposedTopic, ResolutionOutput, TopicMatch, TopicRelation,
    UnitDecision,
};
use serde::Deserialize;
use std::collections::HashMap;

pub(crate) const CONTRACT: &str = "topic-map.single-pass.v1";
const MAX_NEW_TOPIC_ID: usize = 40;

pub(super) async fn prepare(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    input: &ResearchInput,
) -> Result<Packet, FoundationError> {
    let comment = comments::is_comment(input);
    let reuse = input.coverage["reusedExtractionWindow"].as_str().is_some();
    let projection = if reuse {
        Some(draft_in(tx, window).await?)
    } else {
        None
    };
    let mode = if comment {
        "comment"
    } else if reuse {
        "reassignment"
    } else {
        "author"
    };
    let domain =
        serde_json::from_value(input.domain["domainRef"].clone()).map_err(|_| invalid())?;
    let mut catalog = crate::topic_map_core::catalog_in(tx, domain)
        .await
        .map_err(|e| match e {
            crate::model_settings::ModelError::Database(e) => FoundationError::Database(e),
            _ => invalid(),
        })?;
    let candidates = catalog.as_array_mut().ok_or_else(invalid)?;
    for candidate in candidates.iter_mut() {
        compact_support_examples(candidate);
    }
    let mut schema = output_schema(mode);
    schema["properties"]["contract"]["enum"] = json!([CONTRACT]);
    if mode == "author" && input.comment_study.as_array().is_some_and(Vec::is_empty) {
        schema["properties"]["projection"]["properties"]["rm"]["maxItems"] = json!(0);
    }
    let mut unit_ids = None;
    let units = if mode == "author" {
        Vec::new()
    } else {
        // Indices travel over one program-built list; the model never deduplicates or renumbers it.
        let discussions = if comment {
            comments::discussions(input)
        } else {
            projection
                .as_ref()
                .map(|p| p.discussions.clone())
                .ok_or_else(invalid)?
        };
        crate::topic_map_core::units_for_discussions(input, &discussions)
    };
    if mode != "author" {
        unit_ids = Some(json!(units.iter().map(|(id, _)| id).collect::<Vec<_>>()));
    }
    let payload_input = input_payload(mode, input, candidates, &units);
    let mut context = json!({"contract":CONTRACT,"mode":mode,"projection":projection,"candidates":candidates,
        "recall":{"method":"complete_catalog","catalogTopics":candidates.len(),"similarityIsMembership":false}});
    if let Some(unit_ids) = unit_ids {
        context["unitIds"] = unit_ids;
    }
    Ok(Packet {
        payload: json!({"contract":CONTRACT,"input":payload_input,"outputFieldNames":analysis::compact_field_names(),"outputSchema":schema}),
        schema,
        context,
    })
}

/// Author sends exactly one Core; other modes send only the saved discussions and definitions.
fn input_payload(
    mode: &str,
    input: &ResearchInput,
    definitions: &[Value],
    units: &[(String, analysis::Discussion)],
) -> Value {
    let mut payload = json!({"mode":mode,"domain":input.domain,"workRef":input.work.work_ref,
        "definitionDefaults":{"conceptRole":"topic","qualityState":"current","parentTopicRef":null},
        "definitions":definitions.iter().map(model_definition).collect::<Vec<_>>()});
    if mode == "author" {
        payload["fragments"] = json!(
            input
                .fragments
                .iter()
                .enumerate()
                .map(|(index, f)| model_fragment(index, f))
                .collect::<Vec<_>>()
        );
        payload["citationSpeakerRule"] = json!(
            "判断讨论的 speakerRole 时读取所引片段的 citationSpeakerRole。只有全部引用均为 author 才标 author。只有全部引用为 quoted 才标 quoted。混合、未知或归属不明用 unknown。作者作品中的引述或图片模型观察不自动成为作者原话。字段 sp 为 unknown 时，陈述用“该材料讨论”等来源中立的表达。机制、疗效或因果判断要保留为来源主张及其不确定性。若 commentStudy 为空，projection.rm 必须是空数组。"
        );
        payload["commentStudy"] = input.comment_study.clone();
        payload["roleMetadata"] = input.role_metadata.clone();
        let mut coverage = input.coverage.clone();
        if let Some(object) = coverage.as_object_mut() {
            for key in [
                "core",
                "sourceHashes",
                "fragmentOrigins",
                "sourceFragmentIds",
                "sourceFragmentId",
                "windowKey",
            ] {
                object.remove(key);
            }
        }
        payload["coverage"] = coverage;
        payload["contentCore"] = input.coverage["core"].clone();
    } else {
        payload["units"] = json!(units.iter().enumerate().map(|(index,(_,discussion))|json!({"discussionIndex":index,"discussion":discussion})).collect::<Vec<_>>());
    }
    payload
}

/// Keep full topic coverage and exact source quotes, with each example limited
/// to the boundary statement and its own citations. Core units and raw anchors
/// are retained by the original research window, where they can be qualified.
fn compact_support_examples(candidate: &mut Value) {
    let Some(examples) = candidate
        .get_mut("supportExamples")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for example in examples {
        if let Some(example) = example.as_object_mut() {
            example.remove("sourceSnapshot");
            if let Some(evidence) = example.get_mut("evidence").and_then(Value::as_array_mut) {
                for citation in evidence.iter_mut().filter_map(Value::as_object_mut) {
                    citation.remove("anchor");
                    citation.remove("knowledgeUnits");
                }
            }
        }
    }
}

/// Stable fragment indices resolve exact quotes against the complete frozen manifest.
fn model_fragment(index: usize, fragment: &linggan_evidence::creator_discovery::Fragment) -> Value {
    let role = analysis::fragment_role(fragment);
    if matches!(
        fragment.field.as_str(),
        "core_text_evidence" | "core_image_evidence"
    ) {
        return json!({"fragmentIndex":index,"field":fragment.field,"citationSpeakerRole":role,
            "speaker":fragment.speaker,"content":{"text":fragment.text,"evidenceRef":fragment.structure["evidenceRef"],
            "kind":fragment.fragment_id.rsplit('.').next()},"issues":fragment.issues});
    }
    json!({"fragmentIndex":index,"fragmentId":fragment.fragment_id,"field":fragment.field,
        "sourceRef":fragment.source_ref,"sourceVersion":fragment.source_version,"speaker":fragment.speaker,
        "citationSpeakerRole":role,"content":fragment.model_content(),"issues":fragment.issues})
}

/// A boundary example needs its meaning and source role. The dispatch context retains
/// its stable source, offsets and version for qualification and acceptance.
fn model_definition(definition: &Value) -> Value {
    let mut wire = definition.clone();
    if let Some(object) = wire.as_object_mut() {
        // The immutable definitionRef already identifies its version. Induction history
        // stays in the dispatch context; qualityState carries its semantic consequence.
        object.remove("version");
        object.remove("inductionMethod");
        if object.get("conceptRole").and_then(Value::as_str) == Some("topic") {
            object.remove("conceptRole");
        }
        if object.get("qualityState").and_then(Value::as_str) == Some("current") {
            object.remove("qualityState");
        }
        if object.get("parentTopicRef").is_some_and(Value::is_null) {
            object.remove("parentTopicRef");
        }
    }
    if let Some(examples) = wire
        .get_mut("supportExamples")
        .and_then(Value::as_array_mut)
    {
        for example in examples {
            if let Some(citations) = example.get_mut("evidence").and_then(Value::as_array_mut) {
                for citation in citations.iter_mut().filter_map(Value::as_object_mut) {
                    citation
                        .retain(|key, _| matches!(key.as_str(), "text" | "field" | "speakerRole"));
                }
            }
            if let Some(object) = example.as_object_mut() {
                object.retain(|key, _| {
                    matches!(
                        key.as_str(),
                        "statement" | "speakerRole" | "evidenceRole" | "evidence"
                    )
                });
            }
        }
    }
    wire
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Output {
    contract: String,
    projection: Value,
    new_topics: Vec<NewTopic>,
    decisions: Vec<WireDecision>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewTopic {
    new_topic_id: String,
    label: String,
    definition: String,
    inclusion_criteria: Vec<String>,
    exclusion_criteria: Vec<String>,
    domain_fit: String,
    domain_reason: String,
    abstraction_reason: String,
    parent: WireParent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireParent {
    kind: String,
    topic_ref: Option<Uuid>,
    definition_ref: Option<Uuid>,
    new_topic_id: Option<String>,
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDecision {
    discussion_index: usize,
    status: String,
    matches: Vec<TopicMatch>,
    new_topic_id: Option<String>,
    relations: Vec<TopicRelation>,
    reason: String,
}

fn output_schema(mode: &str) -> Value {
    let resolution = analysis::resolution_schema();
    let mut decisions = resolution["properties"]["decisions"].clone();
    let proposed = decisions["items"]["properties"]["proposedTopic"].clone();
    let fields = decisions["items"]["properties"].as_object_mut().unwrap();
    fields.remove("unitId");
    fields.remove("proposedTopic");
    fields.insert("discussionIndex".into(), json!({"type":"integer","minimum":0,
        "description":"author: zero-based index into projection.discussions; comment and reassignment: discussionIndex supplied in input.units"}));
    fields.insert("newTopicId".into(), json!({"anyOf":[{"type":"string","minLength":1,"maxLength":MAX_NEW_TOPIC_ID},{"type":"null"}],
        "description":"Same local id for every unit sharing one new topic; null unless status is new."}));
    decisions["items"]["required"] = json!([
        "discussionIndex",
        "status",
        "matches",
        "newTopicId",
        "relations",
        "reason"
    ]);
    decisions["minItems"] = json!(0);
    if mode == "comment" {
        decisions["items"]["properties"]["status"]["enum"] =
            json!(["matched", "uncertain", "out_of_scope"]);
        decisions["items"]["properties"]["newTopicId"] = json!({"type":"null"});
        decisions["items"]["properties"]["relations"]["maxItems"] = json!(0);
    }
    let mut topic = proposed["anyOf"][0].clone();
    let properties = topic["properties"].as_object_mut().unwrap();
    properties.insert(
        "newTopicId".into(),
        json!({"type":"string","minLength":1,"maxLength":MAX_NEW_TOPIC_ID}),
    );
    properties.insert("parent".into(), json!({"type":"object","additionalProperties":false,"required":["kind","topicRef","definitionRef","newTopicId","reason"],"properties":{
        "kind":{"type":"string","enum":["root","existing","newTopic"]},
        "topicRef":{"anyOf":[{"type":"string","format":"uuid"},{"type":"null"}]},
        "definitionRef":{"anyOf":[{"type":"string","format":"uuid"},{"type":"null"}]},
        "newTopicId":{"anyOf":[{"type":"string","minLength":1,"maxLength":MAX_NEW_TOPIC_ID},{"type":"null"}]},
        "reason":{"type":"string","minLength":1,"maxLength":500}}}));
    topic["required"] = json!([
        "newTopicId",
        "label",
        "definition",
        "inclusionCriteria",
        "exclusionCriteria",
        "domainFit",
        "domainReason",
        "abstractionReason",
        "parent"
    ]);
    let new_topics = json!({"type":"array","minItems":0,
        "maxItems":if mode == "comment" {0} else {analysis::MAX_DISCUSSIONS},"items":topic});
    json!({"type":"object","additionalProperties":false,"required":["contract","projection","newTopics","decisions"],"properties":{
        "contract":{"type":"string","enum":[CONTRACT]},
        "projection":if mode == "author" {analysis::compact_output_schema()} else {json!({"type":"null"})},
        "newTopics":new_topics,
        "decisions":decisions
    }})
}

fn concept(topic: &NewTopic) -> ConceptDefinition {
    ConceptDefinition {
        label: topic.label.clone(),
        definition: topic.definition.clone(),
        inclusion_criteria: topic.inclusion_criteria.clone(),
        exclusion_criteria: topic.exclusion_criteria.clone(),
        domain_fit: topic.domain_fit.clone(),
        domain_reason: topic.domain_reason.clone(),
        abstraction_reason: topic.abstraction_reason.clone(),
    }
}

/// Local ids exist only inside this response; acceptance receives ordinary database-facing proposals.
/// A topic referenced only as another topic's parent still carries its own parent chain.
fn proposed_topics(
    new_topics: &[NewTopic],
) -> Result<HashMap<String, ProposedTopic>, &'static str> {
    let mut by_id = HashMap::new();
    for topic in new_topics {
        if topic.new_topic_id.trim().is_empty()
            || topic.new_topic_id.chars().count() > MAX_NEW_TOPIC_ID
        {
            return Err("new_topic_id_invalid");
        }
        if by_id.insert(topic.new_topic_id.as_str(), topic).is_some() {
            return Err("new_topic_id_duplicate");
        }
    }
    let mut built = HashMap::new();
    for id in by_id.keys() {
        build_topic(id, &by_id, &mut Vec::new(), &mut built)?;
    }
    Ok(built)
}

fn build_topic(
    id: &str,
    topics: &HashMap<&str, &NewTopic>,
    stack: &mut Vec<String>,
    built: &mut HashMap<String, ProposedTopic>,
) -> Result<ProposedTopic, &'static str> {
    if let Some(topic) = built.get(id) {
        return Ok(topic.clone());
    }
    if stack.iter().any(|ancestor| ancestor.as_str() == id) {
        return Err("new_topic_parent_cycle");
    }
    let topic = *topics.get(id).ok_or("new_topic_parent_unknown")?;
    stack.push(id.to_owned());
    let parent = build_parent(topic, topics, stack, built)?;
    stack.pop();
    let proposed = ProposedTopic {
        label: topic.label.clone(),
        definition: topic.definition.clone(),
        inclusion_criteria: topic.inclusion_criteria.clone(),
        exclusion_criteria: topic.exclusion_criteria.clone(),
        domain_fit: topic.domain_fit.clone(),
        domain_reason: topic.domain_reason.clone(),
        abstraction_reason: topic.abstraction_reason.clone(),
        parent,
    };
    built.insert(id.to_owned(), proposed.clone());
    Ok(proposed)
}

fn build_parent(
    topic: &NewTopic,
    topics: &HashMap<&str, &NewTopic>,
    stack: &mut Vec<String>,
    built: &mut HashMap<String, ProposedTopic>,
) -> Result<ProposedParent, &'static str> {
    let parent = &topic.parent;
    match parent.kind.as_str() {
        "root" => {
            if parent.topic_ref.is_some()
                || parent.definition_ref.is_some()
                || parent.new_topic_id.is_some()
            {
                return Err("new_topic_parent_reference_ambiguous");
            }
            Ok(ProposedParent {
                kind: "root".into(),
                topic_ref: None,
                definition_ref: None,
                proposal: None,
                ancestor: None,
                reason: parent.reason.clone(),
            })
        }
        "existing" => {
            let (Some(topic_ref), Some(definition_ref)) = (parent.topic_ref, parent.definition_ref)
            else {
                return Err("new_topic_parent_reference_missing");
            };
            if parent.new_topic_id.is_some() {
                return Err("new_topic_parent_reference_ambiguous");
            }
            Ok(ProposedParent {
                kind: "existing".into(),
                topic_ref: Some(topic_ref),
                definition_ref: Some(definition_ref),
                proposal: None,
                ancestor: None,
                reason: parent.reason.clone(),
            })
        }
        "newTopic" => {
            let id = parent
                .new_topic_id
                .as_deref()
                .ok_or("new_topic_parent_reference_missing")?;
            if parent.topic_ref.is_some() || parent.definition_ref.is_some() {
                return Err("new_topic_parent_reference_ambiguous");
            }
            if id == topic.new_topic_id {
                return Err("new_topic_parent_self");
            }
            let parent_topic = *topics.get(id).ok_or("new_topic_parent_unknown")?;
            let resolved = build_topic(id, topics, stack, built)?;
            let ancestor = (resolved.parent.kind != "root").then(|| Box::new(resolved.parent));
            Ok(ProposedParent {
                kind: "proposed".into(),
                topic_ref: None,
                definition_ref: None,
                proposal: Some(concept(parent_topic)),
                ancestor,
                reason: parent.reason.clone(),
            })
        }
        _ => Err("new_topic_parent_kind_invalid"),
    }
}

fn expand_decision(
    wire: WireDecision,
    units: &[(String, analysis::Discussion)],
    proposed: &HashMap<String, ProposedTopic>,
) -> Result<UnitDecision, &'static str> {
    let (unit_id, _) = units
        .get(wire.discussion_index)
        .ok_or("discussion_index_invalid")?;
    let proposed_topic = match wire.new_topic_id.as_deref() {
        Some(id) => Some(
            proposed
                .get(id)
                .ok_or("new_topic_reference_unknown")?
                .clone(),
        ),
        None => None,
    };
    let mut relations = wire.relations;
    // The single declared parent is reflected on every decision that reuses this new topic.
    if let Some(topic) = &proposed_topic
        && topic.parent.kind == "existing"
        && let (Some(topic_ref), Some(definition_ref)) =
            (topic.parent.topic_ref, topic.parent.definition_ref)
        && !relations
            .iter()
            .any(|relation| relation.topic_ref == topic_ref)
    {
        relations.push(TopicRelation {
            topic_ref,
            definition_ref,
            relation: "narrower".into(),
            reason: topic.parent.reason.clone(),
        });
    }
    Ok(UnitDecision {
        unit_id: unit_id.clone(),
        status: wire.status,
        matches: wire.matches,
        proposed_topic,
        relations,
        reason: wire.reason,
    })
}

/// Rejects contradictory replies for the same stable unit; identical repeats are one decision.
fn insert_decision(
    decisions: &mut Vec<UnitDecision>,
    decision: UnitDecision,
) -> Result<(), &'static str> {
    if let Some(previous) = decisions
        .iter()
        .find(|item| item.unit_id == decision.unit_id)
    {
        return (serde_json::to_value(previous).ok() == serde_json::to_value(&decision).ok())
            .then_some(())
            .ok_or("resolution_unit_decision_conflict");
    }
    decisions.push(decision);
    Ok(())
}

pub(super) fn validate(
    text: &str,
    input: &ResearchInput,
    context: &Value,
) -> Result<Value, &'static str> {
    let wire: Output = serde_json::from_str(text).map_err(|_| "output_schema_invalid")?;
    if wire.contract != CONTRACT {
        return Err("output_contract_invalid");
    }
    let comment = comments::is_comment(input);
    if comment && !wire.new_topics.is_empty() {
        return Err("comment_topic_creation_forbidden");
    }
    let projection: Option<ResearchOutput> = match context["mode"].as_str() {
        Some("author") => {
            let draft =
                analysis::decode_compact_output(&wire.projection.to_string(), &input.fragments)
                    .map_err(|e| e.code)?;
            analysis::validate_output(&draft, &input.fragments, &[])?;
            analysis::validate_context_links(&draft, &input.fragments, &input.comment_study)?;
            Some(draft)
        }
        Some("reassignment") if wire.projection.is_null() => Some(
            serde_json::from_value(context["projection"].clone())
                .map_err(|_| "topic_dependency_failed")?,
        ),
        Some("comment") if wire.projection.is_null() => None,
        _ => return Err("output_schema_invalid"),
    };
    // Author decisions index the model's own projection; other modes index the supplied unit list.
    let (indexed, units) = match context["mode"].as_str() {
        Some("author") => {
            let projection = projection.as_ref().ok_or("output_schema_invalid")?;
            let indexed = crate::topic_map_core::indexed_units(input, &projection.discussions);
            let units =
                crate::topic_map_core::units_for_discussions(input, &projection.discussions);
            (indexed, units)
        }
        Some("comment" | "reassignment") => {
            let discussions = if comment {
                comments::discussions(input)
            } else {
                projection
                    .as_ref()
                    .ok_or("output_schema_invalid")?
                    .discussions
                    .clone()
            };
            let units = crate::topic_map_core::units_for_discussions(input, &discussions);
            let ids = context["unitIds"]
                .as_array()
                .ok_or("topic_dependency_failed")?;
            if ids.len() != units.len() {
                return Err("topic_dependency_failed");
            }
            let indexed = ids
                .iter()
                .zip(units.iter())
                .map(|(id, (_, discussion))| {
                    (
                        id.as_str().unwrap_or_default().to_owned(),
                        discussion.clone(),
                    )
                })
                .collect::<Vec<_>>();
            (indexed, units)
        }
        _ => return Err("output_schema_invalid"),
    };
    let proposed = proposed_topics(&wire.new_topics)?;
    let mut decisions = Vec::new();
    for decision in wire.decisions {
        insert_decision(
            &mut decisions,
            expand_decision(decision, &indexed, &proposed)?,
        )?;
    }
    let resolution = ResolutionOutput {
        contract: analysis::RESOLVE_CONTRACT.into(),
        decisions,
    };
    if comment {
        comments::validate_matched_only(&resolution)?;
    }
    if units.is_empty() {
        if !resolution.decisions.is_empty() {
            return Err("resolution_units_invalid");
        }
    } else {
        analysis::validate_resolution(&resolution, &units, &context["candidates"])?;
    }
    Ok(json!({"contract":CONTRACT,"projection":projection,"resolution":resolution}))
}

pub(super) async fn publish_in(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    invocation: Uuid,
    input: &ResearchInput,
    context: &Value,
    parsed: &Value,
) -> Result<(), crate::model_settings::ModelError> {
    use crate::model_settings::ModelError;
    let resolution: ResolutionOutput = serde_json::from_value(parsed["resolution"].clone())
        .map_err(|_| ModelError::InvalidOutput)?;
    let projection: Option<ResearchOutput> = serde_json::from_value(parsed["projection"].clone())
        .map_err(|_| ModelError::InvalidOutput)?;
    let units = if comments::is_comment(input) {
        crate::topic_map_core::units_for_discussions(input, &comments::discussions(input))
    } else {
        crate::topic_map_core::units_for_discussions(
            input,
            &projection
                .as_ref()
                .ok_or(ModelError::InvalidOutput)?
                .discussions,
        )
    };
    let eligible = context["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["definitionRef"].as_str()?.parse().ok())
        .collect::<Vec<_>>();
    let accepted = crate::topic_map_core::accept_in(
        tx,
        crate::topic_map_core::Acceptance {
            input,
            task: window,
            invocation,
            units: &units,
            resolution: &resolution,
            candidates: &context["candidates"],
            recall: &context["recall"],
            semantic_verification: &Value::Null,
            eligible_definitions: &eligible,
        },
    )
    .await?;
    let output = if comments::is_comment(input) {
        json!({"contract":comments::CONTRACT,"semanticRequestRef":input.coverage["semanticRequestRef"]})
    } else {
        json!(projection)
    };
    sqlx::query("INSERT INTO linggan_topic_research_result(result_ref,window_ref,invocation_ref,output_json,core_json,method_version) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::new_v4()).bind(window).bind(invocation).bind(output).bind(json!({"units":accepted})).bind(analysis::METHOD_VERSION).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
