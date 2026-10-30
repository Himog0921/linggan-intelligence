//! Preview and human confirmation save local intent only. No secret lookup or provider calls.
use super::{
    AiError, ExecuteRequest, METHOD, PreviewRequest, content_workbench, execution, inputs_in,
    sources, status,
};
use crate::ai_foundation::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::FoundationError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

async fn current_in(tx: &mut Transaction<'_, Postgres>, task: TaskKey) -> Result<Uuid, AiError> {
    sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key=$1 FOR SHARE",
    )
    .bind(task.as_str())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AiError::Model)
}
pub(super) async fn readiness(db: &Database, task: TaskKey) -> Result<Value, AiError> {
    let method = if task == TaskKey::CreationTitle {
        super::TITLE_METHOD
    } else {
        METHOD
    };
    let mut tx = db.pool().begin().await?;
    let config = current_in(&mut tx, task).await?;
    let model = match execution::model_in(&mut tx, config, task).await {
        Ok(model) => model,
        Err(AiError::Database(error)) => return Err(AiError::Database(error)),
        Err(_) => {
            return Ok(
                json!({"configured":false,"transportCallable":null,"writingQualified":null,
            "configRef":null,"taskConfigRef":config,"methodVersion":method,"deniedReasons":["model_configuration_unqualified"]}),
            );
        }
    };
    let denied = match admission::check_queue_configuration(&mut tx, task, config, false).await {
        Ok(()) => vec![],
        Err(FoundationError::AdmissionDenied(reasons)) => reasons,
        Err(error) => return Err(error.into()),
    };
    let callable:Option<bool>=sqlx::query_scalar("SELECT state='succeeded' AND result->>'modelCallable'='true' FROM linggan_model_invocation WHERE model_ref=$1 AND connection_version_ref=$2 AND operation='probe' ORDER BY created_at DESC,invocation_ref DESC LIMIT 1")
        .bind(model.current_model_ref).bind(model.connection_version_ref).fetch_optional(&mut *tx).await?.flatten();
    let qualified:Option<bool>=sqlx::query_scalar("SELECT qualified FROM linggan_creation_writing_qualification WHERE model_ref=$1 AND connection_version_ref=$2 AND method_version=$3 ORDER BY created_at DESC,invocation_ref DESC LIMIT 1")
        .bind(model.current_model_ref).bind(model.connection_version_ref).bind(method).fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"configured":true,"configRef":model.config_ref,"taskConfigRef":config,"modelRef":model.current_model_ref,
        "connectionVersionRef":model.connection_version_ref,"transportCallable":callable,"writingQualified":qualified,
        "methodVersion":method,"deniedReasons":denied}),
    )
}
pub(super) async fn preview(db: &Database, r: &PreviewRequest) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(r.preview_ref.to_string())
        .execute(&mut *tx)
        .await?;
    let task = r.action.task();
    let method = r.action.method();
    let config = current_in(&mut tx, task).await?;
    let model = execution::model_in(&mut tx, config, task).await?;
    if r.config_ref != model.config_ref {
        return Err(AiError::Stale);
    }
    let (revision, payload, body_hash) = inputs_in(&mut tx, r).await?;
    let protected = super::protected_experience(&revision.document, r)?;
    let system = execution::system_in(&mut tx, config, task).await?;
    let prompt = payload.to_string();
    let estimated = crate::ai_foundation::transport::estimate_input_tokens(&system, &prompt);
    if estimated > model.limits.input_tokens {
        return Err(AiError::InputLimit);
    }
    model
        .execution
        .validate(
            &model.api,
            model.limits.output_tokens,
            model.limits.timeout_seconds * 1000,
            &system,
            &prompt,
        )
        .map_err(|_| AiError::Invalid)?;
    let reserved = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(AiError::Invalid)?;
    let previous_salt: Option<Uuid> = sqlx::query_scalar(
        "SELECT hash_salt FROM linggan_creation_ai_experience_material WHERE preview_ref=$1",
    )
    .bind(r.preview_ref)
    .fetch_optional(&mut *tx)
    .await?;
    let salt = protected.map(|_| previous_salt.unwrap_or_else(Uuid::new_v4));
    let hash = execution::protected_preview_hash(method, config, &body_hash, &system, salt);
    let actual_manifest = serde_json::to_value(r).map_err(|_| AiError::Invalid)?;
    let manifest = if protected.is_some() {
        json!({"experienceProtected":true})
    } else {
        actual_manifest.clone()
    };
    let inserted=sqlx::query("INSERT INTO linggan_creation_ai_preview(preview_ref,work_ref,input_revision,config_ref,request_hash,method_version,action,input_manifest,reserved_tokens,task_config_ref)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT DO NOTHING")
        .bind(r.preview_ref).bind(r.work_ref).bind(r.expected_revision).bind(r.config_ref).bind(&hash).bind(method)
        .bind(serde_json::to_value(&r.action).map_err(|_|AiError::Invalid)?.as_str()).bind(&manifest).bind(reserved).bind(config).execute(&mut *tx).await?;
    if inserted.rows_affected() == 0 {
        let identical:bool=sqlx::query_scalar("SELECT request_hash=$2 AND task_config_ref=$3 AND input_manifest=$4 AND reserved_tokens=$5 FROM linggan_creation_ai_preview WHERE preview_ref=$1")
            .bind(r.preview_ref).bind(&hash).bind(config).bind(&manifest).bind(reserved).fetch_one(&mut *tx).await?;
        if !identical {
            return Err(AiError::Conflict);
        }
    }
    if let (Some(experience), Some(salt)) = (protected, salt) {
        let result = sqlx::query("INSERT INTO linggan_creation_ai_experience_material(preview_ref,experience_ref,manifest,hash_salt) VALUES($1,$2,$3,$4) ON CONFLICT(preview_ref) DO NOTHING")
            .bind(r.preview_ref).bind(experience).bind(&actual_manifest).bind(salt).execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            let same: bool = sqlx::query_scalar("SELECT experience_ref=$2 AND manifest=$3 AND hash_salt=$4 FROM linggan_creation_ai_experience_material WHERE preview_ref=$1")
                .bind(r.preview_ref).bind(experience).bind(&actual_manifest).bind(salt).fetch_one(&mut *tx).await?;
            if !same {
                return Err(AiError::Conflict);
            }
        }
    }
    tx.commit().await?;
    Ok(
        json!({"previewRef":r.preview_ref,"inputRevision":r.expected_revision,"inputHash":hash,"action":r.action,"methodVersion":method,
        "configRef":r.config_ref,"taskConfigRef":config,"modelRef":model.current_model_ref,"connectionVersionRef":model.connection_version_ref,
        "sourceCount":r.citations.len(),"inputCharacterCount":prompt.chars().count(),"estimatedInputTokens":estimated,
        "reservedTokens":reserved,"maxCalls":1,"outboundInput":payload,"outboundSystem":system,"confirmationRequired":true}),
    )
}
pub(super) async fn enqueue(db: &Database, r: &ExecuteRequest) -> Result<Value, AiError> {
    if !r.confirmed || r.budget_tokens < 0 {
        return Err(AiError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    content_workbench::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        content_workbench::RequestOwner::AiExecution,
    )
    .await
    .map_err(|e| match e {
        content_workbench::WorkbenchError::Database(e) => AiError::Database(e),
        _ => AiError::Conflict,
    })?;
    let command_hash = sources::text_hash(&serde_json::to_string(r).map_err(|_| AiError::Invalid)?);
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(r.preview_ref.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(old)=sqlx::query("SELECT request_ref,preview_ref,command_hash FROM linggan_creation_ai_request WHERE preview_ref=$1 OR request_ref=$2")
        .bind(r.preview_ref).bind(r.request_id).fetch_optional(&mut *tx).await? {
        if old.get::<Uuid,_>("request_ref")!=r.request_id||old.get::<Uuid,_>("preview_ref")!=r.preview_ref||old.get::<String,_>("command_hash")!=command_hash {return Err(AiError::Conflict)}
        tx.commit().await?;return status(db,r.request_id).await;
    }
    let input = execution::load_in(&mut tx, r.preview_ref).await?;
    let reserved: i64 = sqlx::query_scalar(
        "SELECT reserved_tokens FROM linggan_creation_ai_preview WHERE preview_ref=$1",
    )
    .bind(r.preview_ref)
    .fetch_one(&mut *tx)
    .await?;
    if r.expected_revision != input.manifest.expected_revision
        || r.confirmed_input_hash != input.hash
        || r.budget_tokens < reserved
    {
        return Err(AiError::Invalid);
    }
    if current_in(&mut tx, input.manifest.action.task()).await? != input.preview_config {
        return Err(AiError::Stale);
    }
    let items = [BatchInput {
        item_key: r.request_id.to_string(),
        input: InputRef::WritingRequest {
            request_ref: r.request_id,
        },
    }];
    let hash = batch_store::input_hash(input.manifest.action.task(), &items, false)?;
    let batch = batch_store::insert_batch(
        &mut tx,
        r.request_id,
        input.manifest.action.task(),
        input.preview_config,
        BatchKind::Interactive,
        &items,
        &hash,
    )
    .await?;
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'writing_enqueue',$2,$3)")
        .bind(r.request_id).bind(json!({"requestHash":command_hash})).bind(json!({"requestRef":r.request_id,"batchRef":batch,"itemRef":item,"accepted":true})).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_creation_ai_request(request_ref,preview_ref,command_hash,state,task_config_ref,batch_ref,item_ref,approved_budget_tokens) VALUES($1,$2,$3,'queued',$4,$5,$6,$7)")
        .bind(r.request_id).bind(r.preview_ref).bind(command_hash).bind(input.preview_config).bind(batch).bind(item).bind(r.budget_tokens).execute(&mut *tx).await?;
    tx.commit().await?;
    status(db, r.request_id).await
}
pub(super) async fn cancel(db: &Database, id: Uuid) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;
    let row =
        sqlx::query("SELECT state,batch_ref FROM linggan_creation_ai_request WHERE request_ref=$1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AiError::NotFound)?;
    if matches!(
        row.get::<String, _>("state").as_str(),
        "queued" | "running" | "cancel_requested"
    ) {
        let batch: Option<Uuid> = row.get("batch_ref");
        batch_store::cancel_in(&mut tx, batch.ok_or(AiError::Model)?).await?;
        sqlx::query("UPDATE linggan_creation_ai_request SET state='cancel_requested' WHERE request_ref=$1 AND state IN ('queued','running')")
            .bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    status(db, id).await
}
