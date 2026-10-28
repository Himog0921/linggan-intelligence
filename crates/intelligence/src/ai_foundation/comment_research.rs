//! One explicitly selected comment, its parent and its canonical work dossier.
use super::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::{
    comment_study_selection::{CommentKey, PreparedStudyInput, prepare_study_input},
    comment_study_source::{self, SelectedStudySource, StudySourceRole},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const CONTRACT: &str = "comment-research-semantic.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub source_ref: Uuid,
    pub observation_role: StudySourceRole,
    pub knowledge_cutoff: String,
    pub expected_input_hash: String,
    pub task_config_ref: Uuid,
    pub expected_task_revision: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
}
fn source_error(e: comment_study_source::StudySourceError) -> FoundationError {
    match e {
        comment_study_source::StudySourceError::Database(e) => FoundationError::Database(e),
        _ => FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]),
    }
}
async fn source_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    source: Uuid,
    role: StudySourceRole,
    cutoff: &str,
) -> Result<(SelectedStudySource, PreparedStudyInput), FoundationError> {
    let selected = comment_study_source::selected_source_in(tx, domain, work, source, role, cutoff)
        .await
        .map_err(source_error)?
        .ok_or_else(|| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))?;
    let key = CommentKey {
        work_ref: work,
        comment_external_id: selected.comment_external_id.clone(),
    };
    let input = prepare_study_input(
        &key,
        source,
        &selected.raw_text,
        &selected.source.context_manifest,
        selected.parent.as_ref(),
    )
    .map_err(|_| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))?;
    Ok((selected, input))
}
pub async fn preview(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    source: Uuid,
    role: StudySourceRole,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let (selected, input) = source_in(&mut tx, domain, work, source, role, &cutoff).await?;
    let setting:(Uuid,i64)=sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='comment_study'").fetch_one(&mut *tx).await?;
    let result = json!({"domainRef":domain,"workRef":work,"sourceRef":source,"observationRole":role,"knowledgeCutoff":cutoff,
        "inputHash":input.input_hash,"coverage":selected.source.context_manifest["coverage"],"parentState":input.dependency_state,
        "taskConfigRef":setting.0,"taskRevision":setting.1,"methodVersion":CONTRACT});
    tx.commit().await?;
    Ok(result)
}
pub async fn start(db: &Database, request: &Start) -> Result<Receipt, FoundationError> {
    if request.domain_ref.is_nil()
        || request.work_ref.is_nil()
        || request.source_ref.is_nil()
        || request.expected_task_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(db, request.command_ref, "comment_research", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config = lock_setting(
        &mut command.tx,
        TaskKey::CommentStudy,
        request.expected_task_revision,
    )
    .await?;
    if config != request.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let (_, input) = source_in(
        &mut command.tx,
        request.domain_ref,
        request.work_ref,
        request.source_ref,
        request.observation_role,
        &request.knowledge_cutoff,
    )
    .await?;
    if input.input_hash != request.expected_input_hash {
        return Err(FoundationError::Conflict);
    }
    let unresolved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_request r JOIN linggan_comment_research_stage s USING(request_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND r.work_ref=$2 AND r.input_fingerprint=$3 AND c.failure_code='ai_dispatch_unknown')").bind(request.domain_ref).bind(request.work_ref).bind(&input.input_fingerprint).fetch_one(&mut *command.tx).await?;
    if unresolved {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_prior_dispatch_unresolved".into(),
        ]));
    }
    let request_ref = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request_ref.to_string(),
        input: InputRef::CommentResearch { request_ref },
    }];
    let hash = batch_store::input_hash(TaskKey::CommentStudy, &inputs, false)?;
    let batch_ref = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        TaskKey::CommentStudy,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5,$6,$7::timestamptz,$8,$9,$10,$11,$12)")
        .bind(request_ref).bind(request.command_ref).bind(request.domain_ref).bind(request.work_ref).bind(request.source_ref).bind(request.observation_role.as_str()).bind(&request.knowledge_cutoff)
        .bind(input.input_hash).bind(input.input_fingerprint).bind(config).bind(batch_ref).bind(CONTRACT).execute(&mut *command.tx).await?;
    sqlx::query("INSERT INTO linggan_comment_research_stage(item_ref,request_ref,stage_number) SELECT item_ref,$2,0 FROM linggan_ai_batch_item WHERE batch_ref=$1").bind(batch_ref).bind(request_ref).execute(&mut *command.tx).await?;
    for (stage, task) in [
        (1_u8, TaskKey::CommentProblemResolution),
        (2_u8, TaskKey::CommentProblemPairReview),
    ] {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_task_setting WHERE task_key=$1")
                .bind(task.as_str())
                .fetch_one(&mut *command.tx)
                .await?;
        let config = lock_setting(&mut command.tx, task, revision).await?;
        let inputs = vec![BatchInput {
            item_key: format!("{request_ref}:{stage}"),
            input: InputRef::CommentProblemStage {
                request_ref,
                stage_number: stage,
            },
        }];
        let hash = batch_store::input_hash(task, &inputs, false)?;
        let batch = batch_store::insert_batch(
            &mut command.tx,
            request.command_ref,
            task,
            config,
            BatchKind::Interactive,
            &inputs,
            &hash,
        )
        .await?;
        sqlx::query("INSERT INTO linggan_comment_research_stage(item_ref,request_ref,stage_number) SELECT item_ref,$2,$3 FROM linggan_ai_batch_item WHERE batch_ref=$1").bind(batch).bind(request_ref).bind(stage as i32).execute(&mut *command.tx).await?;
    }
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
) -> Result<(SelectedStudySource, PreparedStudyInput), FoundationError> {
    let row=sqlx::query("SELECT domain_ref,work_ref,source_ref,observation_role,to_char(knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,input_hash,input_fingerprint FROM linggan_comment_research_request WHERE request_ref=$1").bind(request).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let role = match row.get::<String, _>("observation_role").as_str() {
        "primary" => StudySourceRole::Primary,
        "reference" => StudySourceRole::Reference,
        _ => return Err(FoundationError::Invalid),
    };
    let cutoff: String = row.get("cutoff");
    let (source, input) = source_in(
        tx,
        row.get("domain_ref"),
        row.get("work_ref"),
        row.get("source_ref"),
        role,
        &cutoff,
    )
    .await?;
    if input.input_hash != row.get::<String, _>("input_hash")
        || input.input_fingerprint != row.get::<String, _>("input_fingerprint")
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok((source, input))
}
pub async fn latest(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    source: Uuid,
) -> Result<Value, FoundationError> {
    let request: Option<Uuid> = sqlx::query_scalar("SELECT request_ref FROM linggan_comment_research_request WHERE domain_ref=$1 AND work_ref=$2 AND source_ref=$3 ORDER BY created_at DESC,request_ref DESC LIMIT 1")
        .bind(domain).bind(work).bind(source).fetch_optional(db.pool()).await?;
    match request {
        Some(request) => read(db, request).await,
        None => Ok(Value::Null),
    }
}
pub async fn read(db: &Database, request: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    let frozen = match frozen_in(&mut tx, request).await {
        Ok(value) => Some(value),
        Err(FoundationError::AdmissionDenied(_)) => None,
        Err(e) => return Err(e),
    };
    let available = frozen.is_some();
    let row=sqlx::query("SELECT r.batch_ref,c.state,c.failure_code,result.result FROM linggan_comment_research_request r JOIN linggan_ai_batch_item i USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_comment_research_result result USING(request_ref) WHERE r.request_ref=$1")
        .bind(request).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let output: Option<Value> = row.get("result");
    let result = json!({"requestRef":request,"batchRef":row.get::<Uuid,_>("batch_ref"),"state":row.get::<String,_>("state"),"failureCode":row.get::<Option<String>,_>("failure_code"),"sourceAvailable":available,"result":if available{output}else{None},"methodVersion":CONTRACT});
    let stages:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('stageNumber',s.stage_number,'state',c.state,'failureCode',c.failure_code,'batchRef',i.batch_ref,'cancelRequested',batch.cancel_requested) FROM linggan_comment_research_stage s JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch_control batch USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.request_ref=$1 ORDER BY s.stage_number").bind(request).fetch_all(&mut *tx).await?;
    if stages.len() != 3 {
        return Err(FoundationError::Invalid);
    }
    let pending = stages
        .iter()
        .any(|s| s["state"] == "pending" || s["state"] == "running");
    let failed = stages.iter().find(|s| {
        s["state"] == "failed"
            || (s["state"] == "skipped"
                && s["failureCode"] != "comment_no_problem_signal"
                && s["failureCode"] != "comment_problem_support_pending")
    });
    let cancellation_requested: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_research_cancellation WHERE request_ref=$1)",
    )
    .bind(request)
    .fetch_one(&mut *tx)
    .await?;
    let cancelled = stages.iter().any(|s| s["state"] == "cancelled");
    let waiting_support = stages
        .iter()
        .any(|s| s["failureCode"] == "comment_problem_support_pending");
    let supported =
        waiting_support && super::comment_problem::pending_supported_in(&mut tx, request).await?;
    let mut result = result;
    result["supportState"] = json!(if supported {
        "admitted_by_later_review"
    } else if waiting_support {
        "awaiting_independent_support"
    } else {
        "not_pending"
    });
    result["cancelRequested"] = json!(cancellation_requested);
    result["semanticState"] = result["state"].clone();
    result["state"] = json!(if pending {
        "running"
    } else if failed.is_some() {
        "failed"
    } else if cancelled
        || (waiting_support
            && !supported
            && (cancellation_requested || stages.iter().any(|s| s["cancelRequested"] == true)))
    {
        "cancelled"
    } else if waiting_support && !supported {
        "needs_support"
    } else {
        "succeeded"
    });
    if let Some(failed) = failed {
        result["failureCode"] = failed["failureCode"].clone();
    }
    result["stages"] = json!(stages);
    result["target"]=json!(frozen.as_ref().map(|(source,_)|json!({"sourceRef":source.source.source_ref,"commentExternalId":source.comment_external_id,"rawText":source.raw_text})));
    tx.commit().await?;
    Ok(result)
}

/// Recent workflow receipts; source bodies are only available through qualified detail reads.
pub async fn runs(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let rows=sqlx::query("SELECT r.request_ref,r.work_ref,r.source_ref,m.comment_external_id,r.created_at::text AS created_at FROM linggan_comment_research_request r JOIN linggan_material_comment m ON m.material_ref=r.source_ref WHERE r.domain_ref=$1 ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 100").bind(domain).fetch_all(db.pool()).await?;
    let mut runs = Vec::new();
    for row in rows {
        let mut value = read(db, row.get("request_ref")).await?;
        value["workRef"] = json!(row.get::<Uuid, _>("work_ref"));
        value["sourceRef"] = json!(row.get::<Uuid, _>("source_ref"));
        value["commentExternalId"] = json!(row.get::<String, _>("comment_external_id"));
        value["createdAt"] = json!(row.get::<String, _>("created_at"));
        value.as_object_mut().unwrap().remove("result");
        value.as_object_mut().unwrap().remove("target");
        runs.push(value);
    }
    Ok(json!({"domainRef":domain,"runs":runs,"limit":100}))
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub request_ref: Uuid,
}
pub async fn cancel(db: &Database, request: &Cancel) -> Result<Value, FoundationError> {
    let mut command =
        Command::begin(db, request.command_ref, "comment_research_cancel", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_request WHERE request_ref=$1 AND domain_ref=$2)").bind(request.request_ref).bind(request.domain_ref).fetch_one(&mut *command.tx).await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    let batches:Vec<Uuid>=sqlx::query_scalar("SELECT i.batch_ref FROM linggan_comment_research_stage s JOIN linggan_ai_batch_item i USING(item_ref) WHERE s.request_ref=$1 ORDER BY i.batch_ref").bind(request.request_ref).fetch_all(&mut *command.tx).await?;
    for batch in batches {
        batch_store::cancel_in(&mut command.tx, batch).await?;
    }
    // Match settlement's batch → domain lock order. Returned cancellation cannot
    // race with a later publication admitting this pending candidate.
    super::comment_problem::lock_publication_in(&mut command.tx, request.domain_ref).await?;
    sqlx::query("INSERT INTO linggan_comment_research_cancellation(request_ref,command_ref) VALUES($1,$2) ON CONFLICT(request_ref) DO NOTHING").bind(request.request_ref).bind(request.command_ref).execute(&mut *command.tx).await?;
    command
        .finish(json!({"requestRef":request.request_ref,"cancelRequested":true}))
        .await
}
