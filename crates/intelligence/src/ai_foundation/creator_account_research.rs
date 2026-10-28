//! Account research over an explicitly previewed, bounded set of canonical dossiers.
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

pub const CONTRACT: &str = "creator-account-research.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_refs: Vec<Uuid>,
    pub platform: String,
    pub author_external_id: String,
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
    works: &[Uuid],
    platform: &str,
    author: &str,
    cutoff: Option<&str>,
) -> Result<DiscoveryData, FoundationError> {
    if works.is_empty()
        || works.len() > 30
        || works.iter().collect::<std::collections::HashSet<_>>().len() != works.len()
    {
        return Err(FoundationError::Invalid);
    }
    let data = creator_discovery::load_topic_materials_for_works_at_in(tx, domain, works, cutoff)
        .await
        .map_err(source_error)?;
    if data.works.len() != works.len()
        || data.works.iter().any(|w| {
            w.platform != platform
                || w.author_external_id.as_deref() != Some(author)
                || w.fragments.is_empty()
        })
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(data)
}
pub async fn preview(
    db: &Database,
    domain: Uuid,
    platform: &str,
    author: &str,
) -> Result<Value, FoundationError> {
    if author.trim().is_empty() || author.len() > 512 {
        return Err(FoundationError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    // Only formal domain-research works make the sample. Homepage-preview context cards are dropped by the
    // loader below anyway; selecting them here first would crowd real works out of the 30-work window.
    let works:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT content.public_ref FROM linggan_material_content content JOIN linggan_material_content_author author ON author.content_public_ref=content.public_ref JOIN linggan_material_domain_usage usage ON usage.content_public_ref=content.public_ref WHERE usage.domain_ref=$1 AND usage.use_kind='domain_research' AND content.platform=$2 AND author.author_external_id=$3 ORDER BY content.public_ref LIMIT 30")
        .bind(domain).bind(platform).bind(author).fetch_all(&mut *tx).await?;
    let mut data =
        creator_discovery::load_topic_materials_for_works_at_in(&mut tx, domain, &works, None)
            .await
            .map_err(source_error)?;
    data.works.retain(|w| {
        !w.fragments.is_empty()
            && w.platform == platform
            && w.author_external_id.as_deref() == Some(author)
    });
    if data.works.is_empty() {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    let works: Vec<Uuid> = data.works.iter().map(|w| w.work_ref).collect();
    let setting:(Uuid,i64)=sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='creator_account_research'").fetch_one(&mut *tx).await?;
    let fingerprint =
        creator_discovery::account_research_fingerprint(&data.works.iter().collect::<Vec<_>>());
    let result = json!({"domainRef":domain,"platform":platform,"authorExternalId":author,"workRefs":works,"knowledgeCutoff":data.as_of,"inputFingerprint":fingerprint,
        "taskConfigRef":setting.0,"taskRevision":setting.1,"methodVersion":CONTRACT,"coverage":data.works.iter().map(|w|json!({"workRef":w.work_ref,"coverage":w.dossier_coverage})).collect::<Vec<_>>(),"sampleLimitations":["本次从本领域按作品编号排序的前30篇已收录作品中选取可读底稿；不代表账号全量"]});
    tx.commit().await?;
    Ok(result)
}
pub async fn start(db: &Database, request: &Start) -> Result<Receipt, FoundationError> {
    if request.domain_ref.is_nil()
        || request.work_refs.is_empty()
        || request.expected_task_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut command =
        Command::begin(db, request.command_ref, "creator_account_research", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config = lock_setting(
        &mut command.tx,
        TaskKey::CreatorAccountResearch,
        request.expected_task_revision,
    )
    .await?;
    if config != request.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let data = material_in(
        &mut command.tx,
        request.domain_ref,
        &request.work_refs,
        &request.platform,
        &request.author_external_id,
        Some(&request.knowledge_cutoff),
    )
    .await?;
    if creator_discovery::account_research_fingerprint(&data.works.iter().collect::<Vec<_>>())
        != request.expected_fingerprint
    {
        return Err(FoundationError::Conflict);
    }
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creator_account_research_request r JOIN linggan_ai_batch_item item USING(batch_ref) JOIN linggan_ai_batch_item_control control USING(item_ref) WHERE r.domain_ref=$1 AND r.platform=$2 AND r.author_external_id=$3 AND r.input_fingerprint=$4 AND control.failure_code='ai_dispatch_unknown')").bind(request.domain_ref).bind(&request.platform).bind(&request.author_external_id).bind(&request.expected_fingerprint).fetch_one(&mut *command.tx).await?;
    if unresolved {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_prior_dispatch_unresolved".into(),
        ]));
    }
    let request_ref = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request_ref.to_string(),
        input: InputRef::CreatorAccountResearch { request_ref },
    }];
    let hash = batch_store::input_hash(TaskKey::CreatorAccountResearch, &inputs, false)?;
    let batch_ref = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        TaskKey::CreatorAccountResearch,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_creator_account_research_request(request_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,input_fingerprint,task_config_ref,batch_ref,method_version,platform,author_external_id) VALUES($1,$2,$3,$4,$5::timestamptz,$6,$7,$8,$9,$10,$11)")
        .bind(request_ref).bind(request.command_ref).bind(request.domain_ref).bind(&request.work_refs).bind(&request.knowledge_cutoff)
        .bind(&request.expected_fingerprint).bind(config).bind(batch_ref).bind(CONTRACT).bind(&request.platform).bind(&request.author_external_id).execute(&mut *command.tx).await?;
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
    let row = sqlx::query("SELECT domain_ref,work_refs,platform,author_external_id,to_char(knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,input_fingerprint FROM linggan_creator_account_research_request WHERE request_ref=$1")
        .bind(request).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let cutoff: String = row.get("cutoff");
    let data = material_in(
        tx,
        row.get("domain_ref"),
        &row.get::<Vec<Uuid>, _>("work_refs"),
        &row.get::<String, _>("platform"),
        &row.get::<String, _>("author_external_id"),
        Some(&cutoff),
    )
    .await?;
    if creator_discovery::account_research_fingerprint(&data.works.iter().collect::<Vec<_>>())
        != row.get::<String, _>("input_fingerprint")
    {
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
    let row=sqlx::query("SELECT r.batch_ref,c.state,c.failure_code,result.result FROM linggan_creator_account_research_request r JOIN linggan_ai_batch_item i USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_creator_account_research_result result USING(request_ref) WHERE r.request_ref=$1")
        .bind(request).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let output: Option<Value> = row.get("result");
    let result = json!({"requestRef":request,"batchRef":row.get::<Uuid,_>("batch_ref"),"state":row.get::<String,_>("state"),
        "failureCode":row.get::<Option<String>,_>("failure_code"),"sourceAvailable":available,
        "result":if available{output}else{None},"methodVersion":CONTRACT});
    tx.commit().await?;
    Ok(result)
}
