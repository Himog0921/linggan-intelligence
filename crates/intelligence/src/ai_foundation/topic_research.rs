//! Topic workflow ownership. Every stage is queued with its own frozen AI method.
use super::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::topic_map_research::{self as source, ResearchInput};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const CONTRACT: &str = "topic-research-stage.v1";
pub const TASKS: [TaskKey; 4] = [
    TaskKey::TopicResearch,
    TaskKey::TopicResearchExtractReview,
    TaskKey::TopicResearchResolve,
    TaskKey::TopicResearchResolveReview,
];
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_refs: Vec<Uuid>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub run_ref: Uuid,
    pub window_count: usize,
    pub stage_count: usize,
}
fn source_error(error: source::ResearchError) -> FoundationError {
    match error {
        source::ResearchError::Database(e) => FoundationError::Database(e),
        _ => FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]),
    }
}
pub async fn start(db: &Database, request: &Start) -> Result<Receipt, FoundationError> {
    if request.work_refs.is_empty()
        || request.work_refs.len() > 10
        || request.work_refs.iter().any(Uuid::is_nil)
        || request
            .work_refs
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != request.work_refs.len()
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "topic_research", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let mut configurations = Vec::new();
    for task in TASKS {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_task_setting WHERE task_key=$1")
                .bind(task.as_str())
                .fetch_one(&mut *command.tx)
                .await?;
        configurations.push(lock_setting(&mut command.tx, task, revision).await?);
    }
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut command.tx, None).await?;
    let input = source::load_inputs_for_works_at(
        db,
        request.domain_ref,
        configurations[0],
        &request.work_refs,
        &cutoff,
    )
    .await
    .map_err(source_error)?;
    if input.len() != request.work_refs.len() {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    let windows: Vec<_> = input.iter().flat_map(source::research_windows).collect();
    if windows.is_empty() || windows.len() > 50 {
        return Err(FoundationError::AdmissionDenied(vec![
            "research_scope_too_large".into(),
        ]));
    }
    let uncertain:Vec<Value>=sqlx::query_scalar("SELECT DISTINCT w.input_refs FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND w.work_ref=ANY($2) AND c.failure_code='ai_dispatch_unknown'").bind(request.domain_ref).bind(&request.work_refs).fetch_all(&mut *command.tx).await?;
    let refs: Vec<Value> = uncertain
        .iter()
        .flat_map(|v| v["fragments"].as_array().into_iter().flatten().cloned())
        .collect();
    for window in &windows {
        if let Some(full) = input
            .iter()
            .find(|i| i.work.work_ref == window.work.work_ref)
        {
            if source::overlaps_unknown_dispatch(window, full, &refs) {
                return Err(FoundationError::AdmissionDenied(vec![
                    "ai_prior_dispatch_unresolved".into(),
                ]));
            }
        }
    }
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_research_run(run_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,configurations) VALUES($1,$2,$3,$4,$5::timestamptz,$6)")
        .bind(run).bind(request.command_ref).bind(request.domain_ref).bind(&request.work_refs).bind(cutoff).bind(json!(configurations)).execute(&mut *command.tx).await?;
    for window in &windows {
        let reference = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_topic_research_window(window_ref,run_ref,work_ref,input_hash,input_refs) VALUES($1,$2,$3,$4,$5)")
            .bind(reference).bind(run).bind(window.work.work_ref).bind(&window.hash).bind(source::reference_manifest(window)).execute(&mut *command.tx).await?;
        for (index, task) in TASKS.into_iter().enumerate() {
            let inputs = vec![BatchInput {
                item_key: format!("{reference}:{index}"),
                input: InputRef::TopicResearch {
                    window_ref: reference,
                    stage_number: index as u8,
                },
            }];
            let hash = batch_store::input_hash(task, &inputs, false)?;
            let batch = batch_store::insert_batch(
                &mut command.tx,
                request.command_ref,
                task,
                configurations[index],
                BatchKind::Interactive,
                &inputs,
                &hash,
            )
            .await?;
            sqlx::query("INSERT INTO linggan_topic_research_stage(item_ref,window_ref,stage_number) SELECT item_ref,$2,$3 FROM linggan_ai_batch_item WHERE batch_ref=$1")
                .bind(batch).bind(reference).bind(index as i32).execute(&mut *command.tx).await?;
        }
    }
    command
        .finish(Receipt {
            run_ref: run,
            window_count: windows.len(),
            stage_count: windows.len() * 4,
        })
        .await
}
pub(crate) async fn frozen(db: &Database, window: Uuid) -> Result<ResearchInput, FoundationError> {
    let row=sqlx::query("SELECT w.work_ref,w.input_refs,r.domain_ref,r.configurations,to_char(r.knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) WHERE window_ref=$1").bind(window).fetch_optional(db.pool()).await?.ok_or(FoundationError::NotFound)?;
    let configs: Vec<Uuid> =
        serde_json::from_value(row.get("configurations")).map_err(|_| FoundationError::Invalid)?;
    let manifest: Value = row.get("input_refs");
    let work: Uuid = row.get("work_ref");
    let refs = source::manifest_source_work_refs(work, &manifest);
    let loaded = source::load_inputs_for_works_at(
        db,
        row.get("domain_ref"),
        configs[0],
        &refs,
        &row.get::<String, _>("cutoff"),
    )
    .await
    .map_err(source_error)?;
    source::restore_scoped_window(&loaded, work, &manifest)
        .ok_or_else(|| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))
}
pub(crate) async fn previous_in(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    stage: u8,
) -> Result<Value, FoundationError> {
    let row = sqlx::query("SELECT result.output,COALESCE(result.failure_code,control.failure_code) AS failure_code FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control control USING(item_ref) LEFT JOIN linggan_topic_research_stage_result result USING(item_ref) WHERE s.window_ref=$1 AND s.stage_number=$2").bind(window).bind(stage as i32).fetch_optional(&mut **tx).await?;
    match row {
        Some(row) => {
            if let Some(output) = row.get::<Option<Value>, _>("output") {
                return Ok(output);
            }
            let code = row.get::<Option<String>, _>("failure_code");
            Err(FoundationError::AdmissionDenied(vec![
                if code.as_deref() == Some("topic_assignment_not_required") {
                    "topic_assignment_not_required"
                } else {
                    "topic_dependency_failed"
                }
                .into(),
            ]))
        }
        None => Err(FoundationError::AdmissionDenied(vec![
            "topic_dependency_failed".into(),
        ])),
    }
}
pub async fn read(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let rows=sqlx::query("SELECT r.run_ref,r.work_refs,r.created_at::text AS created_at,count(DISTINCT w.window_ref) AS windows,count(*) FILTER(WHERE c.state='succeeded') AS succeeded,count(*) FILTER(WHERE c.state='failed' OR (c.state='skipped' AND c.failure_code IS DISTINCT FROM 'topic_assignment_not_required')) AS failed,count(*) FILTER(WHERE c.state IN('pending','running')) AS pending,jsonb_agg(jsonb_build_object('windowRef',w.window_ref,'workRef',w.work_ref,'stageNumber',s.stage_number,'taskKey',b.task_key,'state',c.state,'failureCode',c.failure_code,'waitReason',c.wait_reason,'batchRef',b.batch_ref) ORDER BY w.window_ref,s.stage_number) AS stages FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item item USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 GROUP BY r.run_ref ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 30")
        .bind(domain).fetch_all(db.pool()).await?;
    let summary:Value=sqlx::query_scalar("SELECT jsonb_build_object('pendingStageCount',count(*) FILTER(WHERE c.state IN('pending','running')),'succeededStageCount',count(*) FILTER(WHERE c.state='succeeded'),'failedStageCount',count(*) FILTER(WHERE c.state='failed' OR (c.state='skipped' AND c.failure_code IS DISTINCT FROM 'topic_assignment_not_required'))) FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1").bind(domain).fetch_one(db.pool()).await?;
    Ok(
        json!({"summary":summary,"domainRef":domain,"methodVersion":CONTRACT,"runs":rows.iter().map(|r|json!({"runRef":r.get::<Uuid,_>("run_ref"),"workRefs":r.get::<Vec<Uuid>,_>("work_refs"),"createdAt":r.get::<String,_>("created_at"),"windowCount":r.get::<i64,_>("windows"),"succeededStageCount":r.get::<i64,_>("succeeded"),"failedStageCount":r.get::<i64,_>("failed"),"pendingStageCount":r.get::<i64,_>("pending"),"stages":r.get::<Value,_>("stages")})).collect::<Vec<_>>()}),
    )
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub run_ref: Uuid,
}
pub async fn cancel(db: &Database, request: &Cancel) -> Result<Value, FoundationError> {
    let mut command =
        Command::begin(db, request.command_ref, "topic_research_cancel", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_research_run WHERE run_ref=$1 AND domain_ref=$2)").bind(request.run_ref).bind(request.domain_ref).fetch_one(&mut *command.tx).await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    let batches:Vec<Uuid>=sqlx::query_scalar("SELECT i.batch_ref FROM linggan_topic_research_window w JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) WHERE w.run_ref=$1 ORDER BY i.batch_ref").bind(request.run_ref).fetch_all(&mut *command.tx).await?;
    for batch in batches {
        batch_store::cancel_in(&mut command.tx, batch).await?;
    }
    command
        .finish(json!({"runRef":request.run_ref,"cancelRequested":true}))
        .await
}
