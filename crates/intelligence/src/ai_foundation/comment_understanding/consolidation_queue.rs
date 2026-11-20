//! Automatic consolidation belongs to a terminal research run, even when its last unit was
//! skipped or cancelled. The origin link and AI command commit with the new batch.
use crate::ai_foundation::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const TASK: TaskKey = TaskKey::CommentProblemConsolidation;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartConsolidation {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_task_revision: i64,
}

pub async fn start_consolidation(
    db: &Database,
    request: &StartConsolidation,
) -> Result<Value, FoundationError> {
    let mut command =
        Command::begin(db, request.command_ref, "comment_consolidation", request).await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let config =
        task_config::lock_setting(&mut command.tx, TASK, request.expected_task_revision).await?;
    let batch = enqueue_in(
        &mut command.tx,
        request.command_ref,
        config,
        request.domain_ref,
        None,
    )
    .await?;
    command.finish(json!({"batchRef":batch})).await
}

pub(crate) async fn tick(db: &Database) -> Result<(), FoundationError> {
    let pending: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT batch.batch_ref, unit.domain_ref
         FROM linggan_ai_batch batch
         JOIN linggan_ai_batch_control control USING(batch_ref)
         JOIN linggan_ai_control switch ON switch.scope_key='task:comment_problem_consolidation' AND switch.enabled
         JOIN LATERAL (
          SELECT unit.domain_ref FROM linggan_ai_batch_item item
          JOIN linggan_comment_research_unit unit USING(item_ref)
          JOIN linggan_ai_batch_item_control item_control USING(item_ref)
          WHERE item.batch_ref=batch.batch_ref AND item_control.output->'newCandidates' > '0'::jsonb
          LIMIT 1
         ) unit ON true
         WHERE batch.task_key='comment_understanding' AND control.state<>'open'
           AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item item
             JOIN linggan_ai_batch_item_control item_control USING(item_ref)
             WHERE item.batch_ref=batch.batch_ref AND item_control.state IN ('pending','running'))
           AND NOT EXISTS(SELECT 1 FROM linggan_comment_problem_consolidation_batch consolidation
             WHERE consolidation.research_batch_ref=batch.batch_ref)
         ORDER BY control.finished_at,batch.batch_ref LIMIT 1",
    )
    .fetch_optional(db.pool())
    .await?;
    let Some((origin, domain)) = pending else {
        return Ok(());
    };
    let command_ref = Uuid::new_v4();
    let payload = json!({"researchBatchRef":origin,"domainRef":domain});
    let mut command =
        Command::begin(db, command_ref, "comment_consolidation_automatic", &payload).await?;
    // Serialize producers by origin without sharing a lock between different runs.
    sqlx::query("SELECT 1 FROM linggan_ai_batch_control WHERE batch_ref=$1 FOR UPDATE")
        .bind(origin)
        .execute(&mut *command.tx)
        .await?;
    let already_queued: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_problem_consolidation_batch WHERE research_batch_ref=$1)",
    )
    .bind(origin)
    .fetch_one(&mut *command.tx)
    .await?;
    if already_queued {
        return Ok(());
    }
    let config = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key=$1 FOR SHARE",
    )
    .bind(TASK.as_str())
    .fetch_one(&mut *command.tx)
    .await?;
    match enqueue_in(&mut command.tx, command_ref, config, domain, Some(origin)).await {
        Ok(batch) => {
            command.finish(json!({"batchRef":batch})).await?;
            Ok(())
        }
        // A fresh configuration or permission can unblock the next tick.
        Err(FoundationError::AdmissionDenied(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn enqueue_in(
    tx: &mut Transaction<'_, Postgres>,
    command: Uuid,
    config: Uuid,
    domain: Uuid,
    origin: Option<Uuid>,
) -> Result<Uuid, FoundationError> {
    let kind = if origin.is_some() {
        BatchKind::Automatic
    } else {
        BatchKind::Interactive
    };
    let inputs = [BatchInput {
        item_key: domain.to_string(),
        input: InputRef::CommentProblemConsolidation { domain_ref: domain },
    }];
    let hash = batch_store::input_hash(TASK, &inputs, false)?;
    let batch = batch_store::insert_batch(tx, command, TASK, config, kind, &inputs, &hash).await?;
    let changed = sqlx::query(
        "INSERT INTO linggan_comment_problem_consolidation_batch(batch_ref,domain_ref,research_batch_ref)
         SELECT $1,domain_ref,$3 FROM observation_domain WHERE domain_ref=$2",
    )
    .bind(batch)
    .bind(domain)
    .bind(origin)
    .execute(&mut **tx)
    .await?;
    if changed.rows_affected() == 0 {
        return Err(FoundationError::NotFound);
    }
    Ok(batch)
}
