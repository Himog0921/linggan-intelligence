//! Immutable acceptance of one verified discussion batch and exact definition versions.
use super::*;
use crate::model_settings::ModelError;
use crate::topic_map_research_analysis::{ProposedTopic, ResolutionOutput, UnitDecision};
use sqlx::{Postgres, Transaction};

type Assignment = (Uuid, Uuid, String, String, Option<(Uuid, Uuid)>);
pub(crate) struct Acceptance<'a> {
    pub input: &'a ResearchInput,
    pub task: Uuid,
    pub invocation: Uuid,
    pub units: &'a [(String, Discussion)],
    pub resolution: &'a ResolutionOutput,
    pub candidates: &'a Value,
    pub recall: &'a Value,
    pub eligible_definitions: &'a [Uuid],
}

pub(crate) async fn accept_in(
    tx: &mut Transaction<'_, Postgres>,
    batch: Acceptance<'_>,
) -> Result<Vec<Value>, ModelError> {
    let domain = batch.input.domain["domainRef"]
        .as_str()
        .and_then(|s| s.parse::<Uuid>().ok())
        .ok_or(ModelError::Source)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,120))")
        .bind(format!("topic-core:{domain}"))
        .execute(&mut **tx)
        .await?;
    // Verify only compared definitions. Unrelated changes never invalidate extraction.
    for candidate in batch.candidates.as_array().into_iter().flatten() {
        let topic = candidate["topicRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .ok_or(ModelError::InvalidOutput)?;
        let definition = candidate["definitionRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .ok_or(ModelError::InvalidOutput)?;
        let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition d JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.topic_ref=$1 AND d.definition_ref=$2 AND b.domain_ref=$3 AND d.version=(SELECT max(version)FROM linggan_topic_definition WHERE topic_ref=$1) AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=$1))")
            .bind(topic).bind(definition).bind(domain).fetch_one(&mut **tx).await?;
        if !current {
            return Err(ModelError::InvalidOutput);
        }
    }
    let mut projected = Vec::new();
    for decision in &batch.resolution.decisions {
        projected.push(accept_unit(tx, &batch, domain, decision).await?);
    }
    Ok(projected)
}

async fn accept_unit(
    tx: &mut Transaction<'_, Postgres>,
    batch: &Acceptance<'_>,
    domain: Uuid,
    decision: &UnitDecision,
) -> Result<Value, ModelError> {
    let input = batch.input;
    let candidates = batch.candidates;
    let (_, discussion) = batch
        .units
        .iter()
        .find(|(id, _)| id == &decision.unit_id)
        .ok_or(ModelError::InvalidOutput)?;
    let unit_ref =
        Uuid::parse_str(&decision.unit_id[..32]).map_err(|_| ModelError::InvalidOutput)?;
    let refs:Vec<_>=discussion.evidence.iter().filter_map(|c|input.fragments.iter().find(|f|f.fragment_id==c.fragment_id)
            .map(|f|json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,"field":f.field,"start":c.start,"end":c.end}))).collect();
    sqlx::query("INSERT INTO linggan_topic_map_discussion_unit(unit_ref,domain_ref,work_public_ref,unit_key,statement,speaker_role,evidence_role,evidence_refs,method_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)ON CONFLICT(domain_ref,work_public_ref,unit_key)DO NOTHING")
            .bind(unit_ref).bind(domain).bind(input.work.work_ref).bind(&decision.unit_id).bind(&discussion.statement)
            .bind(&discussion.speaker_role).bind(&discussion.evidence_role).bind(json!(refs)).bind(METHOD_VERSION).execute(&mut **tx).await?;
    let resolution_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_map_unit_resolution(resolution_ref,unit_ref,invocation_ref,task_ref,status,reason,candidate_manifest)VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(resolution_ref).bind(unit_ref).bind(batch.invocation).bind(batch.task).bind(&decision.status).bind(&decision.reason)
            .bind(json!({"topics":candidates.as_array().into_iter().flatten().map(|c|json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"],"recallScore":c["recallScore"]})).collect::<Vec<_>>(),"recall":batch.recall})).execute(&mut **tx).await?;
    let assignments = assignments_for(tx, batch, domain, decision, discussion).await?;
    for (topic, definition, _, reason, _) in &assignments {
        sqlx::query("INSERT INTO linggan_topic_map_unit_assignment(resolution_ref,topic_ref,definition_ref,reason)VALUES($1,$2,$3,$4)")
            .bind(resolution_ref).bind(topic).bind(definition).bind(reason).execute(&mut **tx).await?;
    }
    append_relations(tx, decision, resolution_ref, &assignments).await?;
    let hierarchy = accepted_hierarchy(tx, decision, resolution_ref, &assignments).await?;
    Ok(
        json!({"unitId":decision.unit_id,"unitRef":unit_ref,"resolutionRef":resolution_ref,
            "label":discussion.label,"statement":discussion.statement,"speakerRole":discussion.speaker_role,
            "evidenceRole":discussion.evidence_role,"rationale":discussion.rationale,"evidence":discussion.evidence,
            "status":decision.status,"reason":decision.reason,"hierarchy":hierarchy,"proposedTopic":decision.proposed_topic,"relations":decision.relations,"comparedDefinitionRefs":candidates.as_array().into_iter().flatten().map(|c|c["definitionRef"].clone()).collect::<Vec<_>>(),
            "assignments":assignments.iter().map(|(topic,definition,label,reason,_)|json!({"topicRef":topic,"definitionRef":definition,"label":label,"reason":reason})).collect::<Vec<_>>(),
            "recall":batch.recall}),
    )
}

async fn assignments_for(
    tx: &mut Transaction<'_, Postgres>,
    batch: &Acceptance<'_>,
    domain: Uuid,
    decision: &UnitDecision,
    discussion: &Discussion,
) -> Result<Vec<Assignment>, ModelError> {
    let input = batch.input;
    let candidates = batch.candidates;
    let citations: Vec<_> = discussion
        .evidence
        .iter()
        .filter_map(|c| {
            input
                .fragments
                .iter()
                .find(|f| f.fragment_id == c.fragment_id)
                .map(|f| crate::topic_map::TopicMapCitation {
                    fragment_id: f.fragment_id.clone(),
                    source_ref: f.source_ref,
                    field: f.field.clone(),
                })
        })
        .collect();
    let mut assignments = Vec::new();
    if let Some(proposal) = &decision.proposed_topic {
        let accepted = crate::topic_map::accept_topic_map_candidates_in(
            tx,
            domain,
            input.work.work_ref,
            &[crate::topic_map::TopicMapCandidateRequest {
                is_parent: false,
                label: proposal.label.clone(),
                topic_ref: None,
                evidence_citations: citations.clone(),
                evidence_role: discussion.evidence_role.clone(),
                definition_text: proposal.definition.clone(),
                inclusion_criteria: proposal.inclusion_criteria.clone(),
                exclusion_criteria: proposal.exclusion_criteria.clone(),
                invocation_ref: batch.invocation,
                reuse_definition_refs: batch.eligible_definitions.to_vec(),
            }],
        )
        .await
        .map_err(|e| match e {
            crate::topic_map::TopicMapError::Database(e) => ModelError::Database(e),
            _ => ModelError::InvalidOutput,
        })?;
        let topic = accepted[0];
        let created_here: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition d JOIN linggan_topic_map_concept_rule rule USING(definition_ref) JOIN LATERAL(SELECT version,parent_topic_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.topic_ref=$1 AND d.version=1 AND rule.invocation_ref=$2 AND b.version=1 AND b.parent_topic_ref IS NULL)")
            .bind(topic).bind(batch.invocation).fetch_one(&mut **tx).await?;
        let parent = if created_here {
            accept_parent(tx, batch, domain, proposal, &citations, discussion).await?
        } else {
            None
        };
        if let Some((parent_topic, parent_definition)) = parent {
            crate::topic_map::bind_new_candidate_parent_in(
                tx,
                domain,
                topic,
                parent_topic,
                parent_definition,
                batch.invocation,
            )
            .await
            .map_err(topic_error)?;
        }
        let definition:Uuid=sqlx::query_scalar("SELECT definition_ref FROM linggan_topic_definition WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1")
                .bind(topic).fetch_one(&mut **tx).await?;
        assignments.push((
            topic,
            definition,
            proposal.label.clone(),
            decision.reason.clone(),
            parent,
        ));
    }
    for matched in &decision.matches {
        let label = candidates
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["topicRef"].as_str() == Some(&matched.topic_ref.to_string()))
            .and_then(|c| c["label"].as_str())
            .unwrap_or(&discussion.label)
            .to_owned();
        assignments.push((
            matched.topic_ref,
            matched.definition_ref,
            label,
            matched.reason.clone(),
            None,
        ));
    }
    Ok(assignments)
}

async fn append_relations(
    tx: &mut Transaction<'_, Postgres>,
    decision: &UnitDecision,
    resolution_ref: Uuid,
    assignments: &[Assignment],
) -> Result<(), ModelError> {
    if decision.status == "new"
        && decision.proposed_topic.is_some()
        && let Some((source, source_def, _, _, _)) = assignments.first()
    {
        for relation in &decision.relations {
            if source == &relation.topic_ref || relation.relation == "equivalent" {
                continue;
            }
            sqlx::query("INSERT INTO linggan_topic_map_concept_relation(relation_ref,resolution_ref,source_topic_ref,source_definition_ref,target_topic_ref,target_definition_ref,relation,reason)VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
                    .bind(Uuid::new_v4()).bind(resolution_ref).bind(source).bind(source_def).bind(relation.topic_ref).bind(relation.definition_ref)
                    .bind(&relation.relation).bind(&relation.reason).execute(&mut **tx).await?;
        }
    }
    Ok(())
}

fn topic_error(error: crate::topic_map::TopicMapError) -> ModelError {
    match error {
        crate::topic_map::TopicMapError::Database(error) => ModelError::Database(error),
        _ => ModelError::InvalidOutput,
    }
}

async fn accept_parent(
    tx: &mut Transaction<'_, Postgres>,
    batch: &Acceptance<'_>,
    domain: Uuid,
    proposal: &ProposedTopic,
    citations: &[crate::topic_map::TopicMapCitation],
    discussion: &Discussion,
) -> Result<Option<(Uuid, Uuid)>, ModelError> {
    let parent = &proposal.parent;
    if parent.kind == "root" {
        return Ok(None);
    }
    if parent.kind == "existing" {
        return parent
            .topic_ref
            .zip(parent.definition_ref)
            .map(Some)
            .ok_or(ModelError::InvalidOutput);
    }
    let concept = parent.proposal.as_ref().ok_or(ModelError::InvalidOutput)?;
    let topics = crate::topic_map::accept_topic_map_candidates_in(
        tx,
        domain,
        batch.input.work.work_ref,
        &[crate::topic_map::TopicMapCandidateRequest {
            is_parent: true,
            label: concept.label.clone(),
            topic_ref: None,
            evidence_citations: citations.to_vec(),
            evidence_role: discussion.evidence_role.clone(),
            definition_text: concept.definition.clone(),
            inclusion_criteria: concept.inclusion_criteria.clone(),
            exclusion_criteria: concept.exclusion_criteria.clone(),
            invocation_ref: batch.invocation,
            reuse_definition_refs: batch.eligible_definitions.to_vec(),
        }],
    )
    .await
    .map_err(topic_error)?;
    let topic = topics[0];
    let definition = sqlx::query_scalar("SELECT definition_ref FROM linggan_topic_definition WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1")
        .bind(topic).fetch_one(&mut **tx).await?;
    Ok(Some((topic, definition)))
}

async fn accepted_hierarchy(
    tx: &mut Transaction<'_, Postgres>,
    decision: &UnitDecision,
    resolution: Uuid,
    assignments: &[Assignment],
) -> Result<Value, ModelError> {
    let Some((child, child_definition, _, _, Some((parent, parent_definition)))) =
        assignments.first()
    else {
        return Ok(Value::Null);
    };
    let reason = decision
        .proposed_topic
        .as_ref()
        .map(|p| p.parent.reason.as_str())
        .ok_or(ModelError::InvalidOutput)?;
    if !decision.relations.iter().any(|r| &r.topic_ref == parent) {
        sqlx::query("INSERT INTO linggan_topic_map_concept_relation(relation_ref,resolution_ref,source_topic_ref,source_definition_ref,target_topic_ref,target_definition_ref,relation,reason)VALUES($1,$2,$3,$4,$5,$6,'narrower',$7)")
            .bind(Uuid::new_v4()).bind(resolution).bind(child).bind(child_definition).bind(parent).bind(parent_definition).bind(reason).execute(&mut **tx).await?;
    }
    Ok(
        json!({"parentTopicRef":parent,"parentDefinitionRef":parent_definition,"state":"attached","reason":reason}),
    )
}
