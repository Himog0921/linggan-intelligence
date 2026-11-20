//! A run is one batch of units. Its receipt and its cancellation, without any extra state: every
//! figure is derived from the unit items, the stored results and the invocation ledger.
use crate::ai_foundation::{
    batch_store,
    store::{Command, FoundationError},
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub batch_ref: Uuid,
}

/// Stop further dispatch and publication. Provider calls already sent still settle usage.
pub async fn cancel(db: &Database, request: &Cancel) -> Result<Value, FoundationError> {
    let mut command = Command::begin(
        db,
        request.command_ref,
        "comment_understanding_cancel",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let owned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_unit u JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref WHERE i.batch_ref=$1 AND u.domain_ref=$2)")
        .bind(request.batch_ref)
        .bind(request.domain_ref)
        .fetch_one(&mut *command.tx)
        .await?;
    if !owned {
        return Err(FoundationError::NotFound);
    }
    batch_store::cancel_in(&mut command.tx, request.batch_ref).await?;
    command
        .finish(json!({"batchRef": request.batch_ref, "cancelRequested": true}))
        .await
}

/// What a run is doing and what it cost.
pub async fn read_run(db: &Database, domain: Uuid, batch: Uuid) -> Result<Value, FoundationError> {
    let mut tx = super::start::read_only_tx(db).await?;
    let units = sqlx::query("SELECT u.unit_ref,u.work_ref,c.state,c.failure_code,(SELECT count(*) FROM linggan_comment_research_unit_target t WHERE t.unit_ref=u.unit_ref) AS targets FROM linggan_comment_research_unit u JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref WHERE i.batch_ref=$1 AND u.domain_ref=$2 ORDER BY i.position")
        .bind(batch)
        .bind(domain)
        .fetch_all(&mut *tx)
        .await?;
    if units.is_empty() {
        return Err(FoundationError::NotFound);
    }
    // The same derivation the comment directory uses for a comment's latest study.
    let targets = sqlx::query("SELECT state,failure_code,count(*) AS n FROM (SELECT CASE WHEN x.result IS NOT NULL THEN 'succeeded' WHEN c.state='cancelled' THEN 'cancelled' WHEN x.failure_code IS NOT NULL THEN 'failed' WHEN c.state IN('pending','running') THEN c.state ELSE 'failed' END AS state,COALESCE(x.failure_code,CASE WHEN x.request_ref IS NULL THEN c.failure_code END) AS failure_code FROM linggan_comment_research_unit_target ut JOIN linggan_comment_research_unit u ON u.unit_ref=ut.unit_ref JOIN linggan_ai_batch_item i ON i.item_ref=u.item_ref JOIN linggan_ai_batch_item_control c ON c.item_ref=u.item_ref LEFT JOIN linggan_comment_research_result x ON x.request_ref=ut.request_ref WHERE i.batch_ref=$1) t GROUP BY state,failure_code ORDER BY state,failure_code")
        .bind(batch)
        .fetch_all(&mut *tx)
        .await?;
    let usage = sqlx::query("SELECT count(*) AS calls,COALESCE(sum(input_tokens),0)::bigint AS input,COALESCE(sum(output_tokens),0)::bigint AS output FROM linggan_model_invocation WHERE batch_ref=$1 AND task_key='comment_understanding' AND dispatch_state IS DISTINCT FROM 'not_dispatched'")
        .bind(batch)
        .fetch_one(&mut *tx)
        .await?;
    let cancel_requested: bool = sqlx::query_scalar(
        "SELECT cancel_requested FROM linggan_ai_batch_control WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(&mut *tx)
    .await?;
    let retry_selections: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('workRef',r.work_ref,'sourceRef',r.source_ref,
           'observationRole',r.observation_role,'commentExternalId',source.comment_external_id)
         FROM linggan_comment_research_request r
         JOIN linggan_material_comment source ON source.material_ref=r.source_ref
         JOIN linggan_comment_research_unit_target target USING(request_ref)
         JOIN linggan_comment_research_unit unit USING(unit_ref)
         JOIN linggan_ai_batch_item_control control USING(item_ref)
         LEFT JOIN linggan_comment_research_result result USING(request_ref)
         WHERE r.batch_ref=$1 AND r.domain_ref=$2 AND control.state<>'cancelled'
           AND (result.failure_code IS NOT NULL OR (result.request_ref IS NULL AND control.state='failed'))
           AND coalesce(result.failure_code,control.failure_code,'')<>'ai_dispatch_unknown'
         ORDER BY r.request_ref",
    ).bind(batch).bind(domain).fetch_all(&mut *tx).await?;
    let result = json!({
        "batchRef": batch,
        "cancelRequested": cancel_requested,
        "retrySelections": retry_selections,
        "units": units.iter().map(|row| Ok(json!({
            "unitRef": row.try_get::<Uuid, _>("unit_ref")?,
            "workRef": row.try_get::<Uuid, _>("work_ref")?,
            "state": row.try_get::<String, _>("state")?,
            "failureCode": row.try_get::<Option<String>, _>("failure_code")?,
            "targetCount": row.try_get::<i64, _>("targets")?,
        }))).collect::<Result<Vec<_>, sqlx::Error>>()?,
        "targets": targets.iter().map(|row| Ok(json!({
            "state": row.try_get::<String, _>("state")?,
            "failureCode": row.try_get::<Option<String>, _>("failure_code")?,
            "count": row.try_get::<i64, _>("n")?,
        }))).collect::<Result<Vec<_>, sqlx::Error>>()?,
        "usage": {
            "modelCalls": usage.try_get::<i64, _>("calls")?,
            "inputTokens": usage.try_get::<i64, _>("input")?,
            "outputTokens": usage.try_get::<i64, _>("output")?,
        },
    });
    tx.commit().await?;
    Ok(result)
}

/// The newest research batches for the domain. The full unit and target receipt loads on demand.
pub async fn read_runs(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let runs: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('batchRef',batch.batch_ref,'createdAt',batch.created_at,
           'state',control.state,'cancelRequested',control.cancel_requested,'unitCount',batch.item_count)
         FROM linggan_ai_batch batch JOIN linggan_ai_batch_control control USING(batch_ref)
         WHERE batch.task_key='comment_understanding' AND EXISTS(
           SELECT 1 FROM linggan_ai_batch_item item JOIN linggan_comment_research_unit unit USING(item_ref)
           WHERE item.batch_ref=batch.batch_ref AND unit.domain_ref=$1
         ) ORDER BY batch.created_at DESC,batch.batch_ref DESC LIMIT 100"
    ).bind(domain).fetch_all(db.pool()).await?;
    let consolidation: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='comment_problem_consolidation'",
    ).fetch_one(db.pool()).await?;
    let consolidations: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('batchRef',batch.batch_ref,'researchBatchRef',scope.research_batch_ref,
           'createdAt',batch.created_at,'state',control.state,'failureCode',control.failure_code,'output',control.output)
         FROM linggan_comment_problem_consolidation_batch scope
         JOIN linggan_ai_batch batch USING(batch_ref)
         JOIN linggan_ai_batch_item item USING(batch_ref)
         JOIN linggan_ai_batch_item_control control USING(item_ref)
         WHERE scope.domain_ref=$1 ORDER BY batch.created_at DESC,batch.batch_ref DESC LIMIT 20"
    ).bind(domain).fetch_all(db.pool()).await?;
    Ok(
        json!({"domainRef":domain,"runs":runs,"consolidationTaskRevision":consolidation,"consolidations":consolidations}),
    )
}
