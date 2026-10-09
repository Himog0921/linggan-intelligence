//! Confirmed merge/split: preview exact versions, explicitly reclassify, never inherit judgments.
use crate::{
    TopicMaterialMemberImport, TopicWorkspaceError, TopicWorkspaceImport, topic_map::TopicMapError,
    topic_workspace::import_reclassified_workspace_in,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructureSource {
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructureDestination {
    pub display_name: String,
    pub definition_text: String,
    pub parent_topic_ref: Option<Uuid>,
    pub members: Vec<TopicMaterialMemberImport>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructurePlan {
    pub request_ref: Uuid,
    pub domain_ref: Uuid,
    pub kind: String,
    pub sources: Vec<StructureSource>,
    pub destinations: Vec<StructureDestination>,
    pub rationale: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyStructure {
    pub plan: StructurePlan,
    pub preview_hash: String,
}
fn hash(value: &impl Serialize) -> Result<String, TopicMapError> {
    Ok(
        Sha256::digest(
            serde_json::to_vec(value).map_err(|e| TopicMapError::Source(e.to_string()))?,
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect(),
    )
}
fn valid_text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max
}
fn validate(plan: &StructurePlan) -> Result<(), TopicMapError> {
    if !valid_text(&plan.rationale, 2000)
        || !matches!(plan.kind.as_str(), "merge" | "split")
        || plan.sources.len() > 20
        || plan.destinations.len() > 10
        || plan.destinations.is_empty()
        || plan.sources.is_empty()
    {
        return Err(TopicMapError::Invalid("invalid_structure_plan"));
    }
    if (plan.kind == "merge" && (plan.sources.len() < 2 || plan.destinations.len() != 1))
        || (plan.kind == "split" && (plan.sources.len() != 1 || plan.destinations.len() < 2))
    {
        return Err(TopicMapError::Invalid("invalid_structure_cardinality"));
    }
    let mut sources = HashSet::new();
    for s in &plan.sources {
        if !sources.insert(s.topic_ref) {
            return Err(TopicMapError::Invalid("duplicate_structure_source"));
        }
    }
    for d in &plan.destinations {
        if !valid_text(&d.display_name, 120)
            || !valid_text(&d.definition_text, 2000)
            || d.members.is_empty()
            || d.members.len() > 100
            || d.parent_topic_ref.is_some_and(|p| sources.contains(&p))
        {
            return Err(TopicMapError::Invalid("invalid_structure_destination"));
        }
        let mut seen = HashSet::new();
        for m in &d.members {
            if !seen.insert(m.work_public_ref) || !valid_text(&m.rationale, 1000) {
                return Err(TopicMapError::Invalid("invalid_structure_member"));
            }
        }
    }
    Ok(())
}
async fn impact(
    tx: &mut Transaction<'_, Postgres>,
    plan: &StructurePlan,
    readable_refs: &HashSet<Uuid>,
) -> Result<Value, TopicMapError> {
    validate(plan)?;
    let mut sources = Vec::new();
    let mut allowed = HashSet::new();
    for source in &plan.sources {
        let row=sqlx::query("SELECT d.definition_ref,d.version,d.display_name,b.domain_ref,b.version AS binding_version FROM linggan_topic_workspace t JOIN LATERAL(SELECT * FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)d ON true JOIN LATERAL(SELECT * FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE t.topic_ref=$1").bind(source.topic_ref).fetch_optional(&mut **tx).await?.ok_or(TopicMapError::NotFound)?;
        if row.get::<Uuid, _>("domain_ref") != plan.domain_ref
            || row.get::<Uuid, _>("definition_ref") != source.definition_ref
        {
            return Err(TopicMapError::Conflict);
        }
        let replaced:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE definition_ref=$1)").bind(source.definition_ref).fetch_one(&mut **tx).await?;
        if replaced {
            return Err(TopicMapError::Conflict);
        }
        let mut refs:Vec<Uuid>=sqlx::query_scalar("SELECT m.work_public_ref FROM linggan_topic_material_member m JOIN linggan_topic_classification_run r USING(classification_run_ref) WHERE r.definition_ref=$1 UNION SELECT work_public_ref FROM linggan_topic_map_work_annotation WHERE topic_ref=$2 AND definition_ref=$1 AND domain_ref=$3").bind(source.definition_ref).bind(source.topic_ref).bind(plan.domain_ref).fetch_all(&mut **tx).await?;
        refs.sort_unstable();
        allowed.extend(refs.iter().copied());
        let alternatives: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_topic_map_alternative WHERE definition_ref=$1",
        )
        .bind(source.definition_ref)
        .fetch_one(&mut **tx)
        .await?;
        let children:Vec<Uuid>=sqlx::query_scalar("SELECT topic_ref FROM (SELECT DISTINCT ON(topic_ref) topic_ref,parent_topic_ref,domain_ref FROM linggan_topic_map_binding ORDER BY topic_ref,version DESC)b WHERE parent_topic_ref=$1 AND domain_ref=$2").bind(source.topic_ref).bind(plan.domain_ref).fetch_all(&mut **tx).await?;
        // A child move is a separate navigation decision; do not strand children behind a hidden node.
        if !children.is_empty() {
            return Err(TopicMapError::Invalid(
                "move_children_before_structural_replacement",
            ));
        }
        sources.push(json!({"topicRef":source.topic_ref,"definitionRef":source.definition_ref,"name":row.get::<String,_>("display_name"),"version":row.get::<i32,_>("version"),"bindingVersion":row.get::<i32,_>("binding_version"),"workRefs":refs,"retainedAlternativeCount":alternatives}));
    }
    let mut selected = HashSet::new();
    for destination in &plan.destinations {
        if let Some(parent) = destination.parent_topic_ref {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM (SELECT DISTINCT ON(topic_ref) topic_ref,domain_ref FROM linggan_topic_map_binding ORDER BY topic_ref,version DESC)b WHERE topic_ref=$1 AND domain_ref=$2 AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source s JOIN LATERAL(SELECT definition_ref FROM linggan_topic_definition WHERE topic_ref=$1 ORDER BY version DESC LIMIT 1)d ON true WHERE s.definition_ref=d.definition_ref))").bind(parent).bind(plan.domain_ref).fetch_one(&mut **tx).await?;
            if !valid {
                return Err(TopicMapError::Invalid("structure_parent_outside_domain"));
            }
        }
        for m in &destination.members {
            if !allowed.contains(&m.work_public_ref) {
                return Err(TopicMapError::Invalid(
                    "structure_member_outside_original_set",
                ));
            }
            let in_domain:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2)").bind(plan.domain_ref).bind(m.work_public_ref).fetch_one(&mut **tx).await?;
            if !in_domain || !readable_refs.contains(&m.work_public_ref) {
                return Err(TopicMapError::Invalid("structure_source_restricted"));
            }
            selected.insert(m.work_public_ref);
        }
    }
    let mut unassigned: Vec<_> = allowed.difference(&selected).copied().collect();
    unassigned.sort_unstable();
    let impact = json!({"sources":sources,"destinations":plan.destinations,"unassignedWorkRefs":unassigned,"oldAlternativePolicy":"retain_original_definition_and_evidence","classificationPolicy":"explicit_new_members_only","researchState":"requires_new_definition_analysis","claimsInherited":false});
    Ok(json!({"previewHash":hash(&(plan,&impact))?,"impact":impact}))
}
async fn current_sources(
    db: &Database,
    plan: &StructurePlan,
) -> Result<HashSet<Uuid>, TopicMapError> {
    let refs: HashSet<_> = plan
        .destinations
        .iter()
        .flat_map(|d| d.members.iter().map(|m| m.work_public_ref))
        .collect();
    let mut qualified = HashSet::new();
    for reference in refs {
        let work = linggan_evidence::read_work_resource(db, reference)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?
            .ok_or(TopicMapError::NotFound)?;
        if work.summary.restriction_state != "WITHDRAWN_OR_RESTRICTED" {
            qualified.insert(reference);
        }
    }
    Ok(qualified)
}
pub async fn preview_structure(
    db: &Database,
    plan: &StructurePlan,
) -> Result<Value, TopicMapError> {
    validate(plan)?;
    let refs = current_sources(db, plan).await?;
    let mut tx = db.pool().begin().await?;
    let result = impact(&mut tx, plan, &refs).await?;
    tx.rollback().await?;
    Ok(result)
}
fn workspace_error(error: TopicWorkspaceError) -> TopicMapError {
    match error {
        TopicWorkspaceError::InvalidRequest(c) => TopicMapError::Invalid(c),
        TopicWorkspaceError::UnknownWorkResource(_) => {
            TopicMapError::Invalid("unknown_work_resource")
        }
        TopicWorkspaceError::IdempotencyConflict | TopicWorkspaceError::VersionConflict { .. } => {
            TopicMapError::Conflict
        }
        _ => TopicMapError::Source(error.to_string()),
    }
}
pub async fn apply_structure(
    db: &Database,
    request: &ApplyStructure,
) -> Result<Value, TopicMapError> {
    validate(&request.plan)?;
    let plan = &request.plan;
    let digest = hash(plan)?;
    let refs = current_sources(db, plan).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,41))")
        .bind(plan.request_ref.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(row)=sqlx::query("SELECT request_hash,preview_hash,receipt FROM linggan_topic_map_structure_receipt WHERE request_ref=$1").bind(plan.request_ref).fetch_optional(&mut *tx).await?{return if row.get::<String,_>("request_hash")==digest&&row.get::<String,_>("preview_hash")==request.preview_hash{Ok(row.get("receipt"))}else{Err(TopicMapError::Conflict)};}
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('topic-map:commands',0))")
        .execute(&mut *tx)
        .await?;
    let mut locks: Vec<_> = plan
        .sources
        .iter()
        .map(|s| format!("topic-binding:{}", s.topic_ref))
        .collect();
    locks.sort();
    for lock in locks {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(lock)
            .execute(&mut *tx)
            .await?;
    }
    let preview = impact(&mut tx, plan, &refs).await?;
    if preview["previewHash"].as_str() != Some(request.preview_hash.as_str()) {
        return Err(TopicMapError::Conflict);
    }
    let mut destinations = Vec::new();
    for (index, d) in plan.destinations.iter().enumerate() {
        let import=TopicWorkspaceImport{idempotency_key:format!("structure:{}:{index}",plan.request_ref),domain_key:format!("domain-{}",plan.domain_ref),canonical_key:format!("topic-{}-{index}",plan.request_ref),display_name:d.display_name.clone(),definition_text:d.definition_text.clone(),expected_version:None,adjudication_note:plan.rationale.clone(),source_boundary:"Confirmed structural reclassification of an explicit canonical set; prior judgments and statistics are not inherited.".into(),members:d.members.clone()};
        let receipt = import_reclassified_workspace_in(&mut tx, &import)
            .await
            .map_err(workspace_error)?;
        let binding = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_topic_map_receipt(receipt_ref,idempotency_key,request_sha256,action,subject_ref)VALUES($1,$2,$3,'structureBinding',$4)").bind(binding).bind(format!("structure-binding:{}:{index}",plan.request_ref)).bind(&digest).bind(receipt.topic_ref).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO linggan_topic_map_binding(binding_ref,topic_ref,domain_ref,parent_topic_ref,version,receipt_ref)VALUES($1,$2,$3,$4,1,$5)").bind(Uuid::new_v4()).bind(receipt.topic_ref).bind(plan.domain_ref).bind(d.parent_topic_ref).bind(binding).execute(&mut *tx).await?;
        destinations.push(receipt);
    }
    let receipt = json!({"requestRef":plan.request_ref,"domainRef":plan.domain_ref,"kind":plan.kind,"state":"persisted","previewHash":request.preview_hash,"destinations":destinations,"oldReferencesRetained":true,"claimsInherited":false});
    sqlx::query("INSERT INTO linggan_topic_map_structure_receipt(request_ref,domain_ref,request_hash,preview_hash,kind,receipt)VALUES($1,$2,$3,$4,$5,$6)").bind(plan.request_ref).bind(plan.domain_ref).bind(&digest).bind(&request.preview_hash).bind(&plan.kind).bind(&receipt).execute(&mut *tx).await?;
    for s in &plan.sources {
        sqlx::query("INSERT INTO linggan_topic_map_structure_source(request_ref,topic_ref,definition_ref)VALUES($1,$2,$3)").bind(plan.request_ref).bind(s.topic_ref).bind(s.definition_ref).execute(&mut *tx).await?;
    }
    for d in &destinations {
        sqlx::query("INSERT INTO linggan_topic_map_structure_destination(request_ref,topic_ref,definition_ref)VALUES($1,$2,$3)").bind(plan.request_ref).bind(d.topic_ref).bind(d.definition_ref).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(receipt)
}
