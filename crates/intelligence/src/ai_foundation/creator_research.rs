//! Creator work analysis on a frozen canonical dossier and the shared AI queue.
use super::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use linggan_evidence::creator_discovery::{self, DiscoveryData};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const CONTRACT: &str = "creator-work-research.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub knowledge_cutoff: String,
    pub expected_fingerprint: String,
    pub task_config_ref: Uuid,
    pub expected_task_revision: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
}
fn source_error(error: creator_discovery::DiscoveryError) -> FoundationError {
    match error {
        creator_discovery::DiscoveryError::Database(e) => FoundationError::Database(e),
        _ => FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]),
    }
}
async fn material_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    cutoff: Option<&str>,
) -> Result<DiscoveryData, FoundationError> {
    let data = creator_discovery::load_topic_materials_for_works_at_in(tx, domain, &[work], cutoff)
        .await
        .map_err(source_error)?;
    if data.works.len() != 1 || data.works[0].work_ref != work || data.works[0].fragments.is_empty()
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(data)
}
pub async fn preview(db: &Database, domain: Uuid, work: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let data = material_in(&mut tx, domain, work, None).await?;
    let setting: (Uuid,i64) = sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='creator_research'").fetch_one(&mut *tx).await?;
    let result = json!({"domainRef":domain,"workRef":work,"knowledgeCutoff":data.as_of,
        "inputFingerprint":data.works[0].fingerprint,"coverage":data.works[0].dossier_coverage,
        "taskConfigRef":setting.0,"taskRevision":setting.1,"methodVersion":CONTRACT});
    tx.commit().await?;
    Ok(result)
}
pub async fn start(db: &Database, request: &Start) -> Result<Receipt, FoundationError> {
    if request.domain_ref.is_nil()
        || request.work_ref.is_nil()
        || request.expected_task_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "creator_research", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config = lock_setting(
        &mut command.tx,
        TaskKey::CreatorResearch,
        request.expected_task_revision,
    )
    .await?;
    if config != request.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let data = material_in(
        &mut command.tx,
        request.domain_ref,
        request.work_ref,
        Some(&request.knowledge_cutoff),
    )
    .await?;
    if data.works[0].fingerprint != request.expected_fingerprint {
        return Err(FoundationError::Conflict);
    }
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creator_research_request r JOIN linggan_ai_batch_item item USING(batch_ref) JOIN linggan_ai_batch_item_control control USING(item_ref) WHERE r.domain_ref=$1 AND r.work_ref=$2 AND r.input_fingerprint=$3 AND control.failure_code='ai_dispatch_unknown')").bind(request.domain_ref).bind(request.work_ref).bind(&request.expected_fingerprint).fetch_one(&mut *command.tx).await?;
    if unresolved {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_prior_dispatch_unresolved".into(),
        ]));
    }
    let request_ref = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request_ref.to_string(),
        input: InputRef::CreatorWorkResearch { request_ref },
    }];
    let hash = batch_store::input_hash(TaskKey::CreatorResearch, &inputs, false)?;
    let batch_ref = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        TaskKey::CreatorResearch,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_creator_research_request(request_ref,command_ref,domain_ref,work_ref,knowledge_cutoff,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5::timestamptz,$6,$7,$8,$9)")
        .bind(request_ref).bind(request.command_ref).bind(request.domain_ref).bind(request.work_ref).bind(&request.knowledge_cutoff)
        .bind(&request.expected_fingerprint).bind(config).bind(batch_ref).bind(CONTRACT).execute(&mut *command.tx).await?;
    command
        .finish(Receipt {
            request_ref,
            batch_ref,
        })
        .await
}
pub(crate) async fn frozen_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<DiscoveryData, FoundationError> {
    let row = sqlx::query("SELECT domain_ref,work_ref,to_char(knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,input_fingerprint FROM linggan_creator_research_request WHERE request_ref=$1")
        .bind(request).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let cutoff: String = row.get("cutoff");
    let data = material_in(
        tx,
        row.get("domain_ref"),
        row.get("work_ref"),
        Some(&cutoff),
    )
    .await?;
    if data.works[0].fingerprint != row.get::<String, _>("input_fingerprint") {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(data)
}
pub async fn read(db: &Database, request: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    let available = match frozen_in(&mut tx, request).await {
        Ok(_) => true,
        Err(FoundationError::AdmissionDenied(_)) => false,
        Err(error) => return Err(error),
    };
    let row=sqlx::query("SELECT r.batch_ref,c.state,c.failure_code,result.result FROM linggan_creator_research_request r JOIN linggan_ai_batch_item i USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_creator_research_result result USING(request_ref) WHERE r.request_ref=$1")
        .bind(request).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let output: Option<Value> = row.get("result");
    let result = json!({"requestRef":request,"batchRef":row.get::<Uuid,_>("batch_ref"),"state":row.get::<String,_>("state"),
        "failureCode":row.get::<Option<String>,_>("failure_code"),"sourceAvailable":available,
        "result":if available{output}else{None},"methodVersion":CONTRACT});
    tx.commit().await?;
    Ok(result)
}
