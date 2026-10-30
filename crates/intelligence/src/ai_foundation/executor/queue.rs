//! Claims carry both instance and item generations. A claim is not dispatch admission.
use super::{ExecutorError, ExecutorFence};
use crate::ai_foundation::{TaskKey, batch_store::InputRef, rounds};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;
const LEASE_SECONDS: i32 = 30;
#[derive(Debug, Clone)]
pub struct ClaimedItem {
    pub item_ref: Uuid,
    pub batch_ref: Uuid,
    pub position: u32,
    pub task_key: TaskKey,
    pub task_config_ref: Uuid,
    pub kind: String,
    pub input: InputRef,
    pub lease_generation: i64,
    pub lease_token: Uuid,
    pub(crate) owner: ExecutorFence,
}
impl ClaimedItem {
    pub(crate) async fn skip_unattempted(
        &self,
        db: &Database,
        code: &'static str,
    ) -> Result<(), ExecutorError> {
        let mut tx = self.owner.begin(db, false).await?;
        let cancelled = self.lock_in(&mut tx).await?;
        let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET state=$2,usable=false,failure_code=$3,lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,finished_at=scope_001_now() WHERE item_ref=$1 AND attempt_count=0 AND invocation_ref IS NULL")
            .bind(self.item_ref).bind(if cancelled{"cancelled"}else{"skipped"}).bind(code).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        rounds::evaluate_in(&mut tx, self.batch_ref).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(crate) async fn lock_in(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<bool, ExecutorError> {
        // Cancellation and settlement own the batch before any item, avoiding lock upgrades.
        sqlx::query("SELECT batch_ref FROM linggan_ai_batch_control WHERE batch_ref=$1 FOR UPDATE")
            .bind(self.batch_ref)
            .fetch_one(&mut **tx)
            .await?;
        let cancelled=sqlx::query_scalar("SELECT b.cancel_requested FROM linggan_ai_batch_item_control c JOIN linggan_ai_batch_item item USING(item_ref) JOIN linggan_ai_batch_control b USING(batch_ref) WHERE c.item_ref=$1 AND c.state='running' AND c.lease_generation=$2 AND c.lease_token=$3 AND c.lease_instance_ref=$4 AND c.lease_instance_generation=$5 AND c.lease_expires_at>clock_timestamp() FOR UPDATE OF c")
            .bind(self.item_ref).bind(self.lease_generation).bind(self.lease_token).bind(self.owner.instance_ref()).bind(self.owner.generation()).fetch_optional(&mut **tx).await?;
        cancelled.ok_or(ExecutorError::FenceLost)
    }
    pub async fn renew(&self, db: &Database) -> Result<(), ExecutorError> {
        let mut tx = self.owner.begin(db, false).await?;
        let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET lease_expires_at=clock_timestamp()+make_interval(secs=>$4) WHERE item_ref=$1 AND state='running' AND lease_generation=$2 AND lease_token=$3 AND lease_instance_ref=$5 AND lease_instance_generation=$6 AND lease_expires_at>clock_timestamp()")
            .bind(self.item_ref).bind(self.lease_generation).bind(self.lease_token).bind(LEASE_SECONDS as f64).bind(self.owner.instance_ref()).bind(self.owner.generation()).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn defer(&self, db: &Database, reason: &str) -> Result<(), ExecutorError> {
        if reason.is_empty()
            || reason.len() > 80
            || !reason.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        {
            return Err(ExecutorError::Invalid);
        }
        let mut tx = self.owner.begin(db, false).await?;
        self.lock_in(&mut tx).await?;
        let changed=sqlx::query("UPDATE linggan_ai_batch_item_control c SET state=CASE WHEN b.cancel_requested THEN 'cancelled' ELSE 'pending' END,usable=CASE WHEN b.cancel_requested THEN false ELSE NULL END,finished_at=CASE WHEN b.cancel_requested THEN scope_001_now() ELSE NULL END,lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,wait_reason=$4,next_attempt_at=clock_timestamp()+interval '5 seconds' FROM linggan_ai_batch_item item JOIN linggan_ai_batch_control b USING(batch_ref) WHERE c.item_ref=$1 AND item.item_ref=c.item_ref AND c.state='running' AND c.lease_generation=$2 AND c.lease_token=$3 AND c.lease_instance_ref=$5 AND c.lease_instance_generation=$6 AND c.attempt_count=0 AND c.invocation_ref IS NULL AND c.lease_expires_at>clock_timestamp()")
            .bind(self.item_ref).bind(self.lease_generation).bind(self.lease_token).bind(reason).bind(self.owner.instance_ref()).bind(self.owner.generation()).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        rounds::evaluate_in(&mut tx, self.batch_ref).await?;
        tx.commit().await?;
        Ok(())
    }
}
pub async fn claim_next(
    db: &Database,
    owner: ExecutorFence,
) -> Result<Option<ClaimedItem>, ExecutorError> {
    let mut tx = owner.begin(db, true).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,4540139))")
        .bind(owner.instance_ref().to_string())
        .execute(&mut *tx)
        .await?;
    let capacity:bool=sqlx::query_scalar("SELECT (SELECT count(*) FROM linggan_ai_batch_item_control WHERE state='running' AND lease_instance_ref=$1)<concurrency FROM linggan_ai_executor_instance WHERE instance_ref=$1").bind(owner.instance_ref()).fetch_one(&mut *tx).await?;
    if !capacity {
        return Ok(None);
    }
    let row=sqlx::query("SELECT item.item_ref,item.batch_ref,item.position,item.input_ref,b.task_key,b.task_config_ref,b.kind,c.lease_generation FROM linggan_ai_batch_item_control c JOIN linggan_ai_batch_item item USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_control bc USING(batch_ref) JOIN linggan_ai_task t ON t.task_key=b.task_key JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=b.task_config_ref WHERE c.state='pending' AND c.next_attempt_at<=clock_timestamp() AND bc.state='open' AND NOT bc.cancel_requested AND t.lifecycle='active' AND t.task_key IN ('model_probe','writing_legacy','creation_title','ocr_recognition','work_comparison','creator_research','comment_study','comment_problem_resolution','comment_problem_pair_review','creator_account_research','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND cfg.execution_contract=t.handler_contract AND c.attempt_count<$1 AND item.position<(bc.last_round_no+1)*$2 AND NOT EXISTS(SELECT 1 FROM linggan_ai_control switch WHERE switch.scope_key IN ('global','executor','task:'||b.task_key) AND NOT switch.enabled) AND (b.task_key<>'ocr_recognition' OR NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item previous JOIN linggan_ai_batch_item_control previous_control USING(item_ref) WHERE previous.batch_ref=b.batch_ref AND previous.position<item.position AND (previous_control.state NOT IN('succeeded','failed','skipped','cancelled') OR previous_control.failure_code='ai_dispatch_unknown'))) AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_stage current_stage JOIN linggan_topic_research_stage prior ON prior.window_ref=current_stage.window_ref AND prior.stage_number<current_stage.stage_number JOIN linggan_ai_batch_item_control pc ON pc.item_ref=prior.item_ref WHERE current_stage.item_ref=item.item_ref AND pc.state NOT IN('succeeded','failed','skipped','cancelled')) AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_stage current_stage JOIN linggan_comment_research_stage prior ON prior.request_ref=current_stage.request_ref AND prior.stage_number<current_stage.stage_number JOIN linggan_ai_batch_item_control pc ON pc.item_ref=prior.item_ref WHERE current_stage.item_ref=item.item_ref AND pc.state NOT IN('succeeded','failed','skipped','cancelled')) ORDER BY b.created_at,b.batch_ref,item.position FOR UPDATE OF c SKIP LOCKED LIMIT 1")
        .bind(AI_LIMITS.max_attempts as i32).bind(AI_LIMITS.round_size as i32).fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let lease_token = Uuid::new_v4();
    let generation = row
        .get::<i64, _>("lease_generation")
        .checked_add(1)
        .ok_or(ExecutorError::Invalid)?;
    let item_ref: Uuid = row.get("item_ref");
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=$2,lease_token=$3,lease_expires_at=clock_timestamp()+make_interval(secs=>$4),lease_instance_ref=$5,lease_instance_generation=$6,wait_reason=NULL,started_at=COALESCE(started_at,scope_001_now()) WHERE item_ref=$1")
        .bind(item_ref).bind(generation).bind(lease_token).bind(LEASE_SECONDS as f64).bind(owner.instance_ref()).bind(owner.generation()).execute(&mut *tx).await?;
    let item = ClaimedItem {
        item_ref,
        batch_ref: row.get("batch_ref"),
        position: row.get::<i32, _>("position") as u32,
        task_key: serde_json::from_value(json!(row.get::<String, _>("task_key")))
            .map_err(|_| ExecutorError::Invalid)?,
        task_config_ref: row.get("task_config_ref"),
        kind: row.get("kind"),
        input: serde_json::from_value(row.get("input_ref")).map_err(|_| ExecutorError::Invalid)?,
        lease_generation: generation,
        lease_token,
        owner,
    };
    tx.commit().await?;
    Ok(Some(item))
}
/// Attempted leases settle as unknown; recovery never blindly sends another provider request.
pub async fn recover_expired(db: &Database, owner: ExecutorFence) -> Result<u64, ExecutorError> {
    let mut tx = owner.begin_mode(db, false, true).await?;
    let rows=sqlx::query("WITH locked_batches AS MATERIALIZED (SELECT b.batch_ref FROM linggan_ai_batch_control b WHERE b.state='open' AND EXISTS(SELECT 1 FROM linggan_ai_batch_item item JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE item.batch_ref=b.batch_ref AND c.state='running' AND (c.lease_expires_at<=clock_timestamp() OR NOT linggan_ai_executor_is_owner(c.lease_instance_ref,c.lease_instance_generation,false))) ORDER BY b.batch_ref FOR UPDATE OF b SKIP LOCKED LIMIT 100) SELECT c.item_ref,c.attempt_count,c.invocation_ref,item.batch_ref,b.cancel_requested FROM linggan_ai_batch_item_control c JOIN linggan_ai_batch_item item USING(item_ref) JOIN locked_batches locked USING(batch_ref) JOIN linggan_ai_batch_control b USING(batch_ref) WHERE c.state='running' AND (c.lease_expires_at<=clock_timestamp() OR NOT linggan_ai_executor_is_owner(c.lease_instance_ref,c.lease_instance_generation,false)) ORDER BY c.lease_expires_at,c.item_ref FOR UPDATE OF c SKIP LOCKED LIMIT 100").fetch_all(&mut *tx).await?;
    let mut batches = BTreeSet::new();
    for row in &rows {
        let attempted = row.get::<i32, _>("attempt_count") > 0;
        if attempted {
            crate::model_invocation::finish_invocation_in(
                &mut tx,
                row.get("invocation_ref"),
                None,
                false,
                Some("ai_dispatch_unknown"),
                &json!({"executorRecovery":true}),
            )
            .await
            .map_err(|_| ExecutorError::Invalid)?;
        }
        let cancelled: bool = row.get("cancel_requested");
        sqlx::query("UPDATE linggan_ai_batch_item_control SET state=$2,usable=$3,failure_code=$4,counts_as_failure=$5,finished_at=CASE WHEN $2='pending' THEN NULL ELSE scope_001_now() END,lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,wait_reason=CASE WHEN $2='pending' THEN 'unattempted_lease_recovered' ELSE NULL END WHERE item_ref=$1")
            .bind(row.get::<Uuid,_>("item_ref")).bind(if attempted{"failed"}else if cancelled{"cancelled"}else{"pending"})
            .bind(if attempted||cancelled{Some(false)}else{None}).bind(if attempted{Some("ai_dispatch_unknown")}else{None}).bind(attempted).execute(&mut *tx).await?;
        batches.insert(row.get::<Uuid, _>("batch_ref"));
    }
    for batch in batches {
        rounds::evaluate_in(&mut tx, batch).await?;
    }
    tx.commit().await?;
    Ok(rows.len() as u64)
}
