//! L1 and topic responses are saved independently of their slower material qualification.
//! Only the original attempt writes a response. Recovery never constructs a Pi request.
use super::{content_understanding, topic_research};
use crate::{
    ai_foundation::{
        TaskKey,
        executor::{
            ExecutorError, ExecutorFence,
            queue::{self, ClaimedItem},
        },
        selected_collection_scope,
    },
    pi_adapter::PiResponse,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn capture(
    db: &Database,
    item: &ClaimedItem,
    invocation: Uuid,
    publication: &impl Serialize,
    response: &PiResponse,
) -> Result<(), ExecutorError> {
    let publication = serde_json::to_value(publication).map_err(|_| ExecutorError::Invalid)?;
    let bytes = serde_json::to_vec(response).map_err(|_| ExecutorError::Invalid)?;
    // Retain the in-memory response while PostgreSQL reconnects. A slow pool checkout must
    // not apply the old publication timeout to the only copy of the provider response.
    loop {
        let saved: Result<(), ExecutorError> = async {
            let mut tx = db.pool().begin().await?;
            // Serialize capture with stale-lease recovery. The immutable attempt also identifies
            // late responses from a previously valid owner, even if its lease is already closed.
            let verified: Option<Uuid> = sqlx::query_scalar(
                "SELECT a.item_ref FROM linggan_ai_execution_attempt a
                 JOIN linggan_ai_batch_item_control c USING(item_ref)
                 WHERE a.invocation_ref=$1 AND a.item_ref=$2 AND a.instance_ref=$3
                   AND a.lease_generation=$4 FOR SHARE OF c",
            ).bind(invocation).bind(item.item_ref).bind(item.owner.instance_ref())
                .bind(item.lease_generation).fetch_optional(&mut *tx).await?;
            if verified.is_none() { return Err(ExecutorError::FenceLost); }
            sqlx::query("INSERT INTO linggan_ai_returned_response(invocation_ref,item_ref,publication,response,lease_token)
                VALUES($1,$2,$3,$4,$5) ON CONFLICT(invocation_ref) DO NOTHING")
                .bind(invocation).bind(item.item_ref).bind(&publication).bind(&bytes).bind(item.lease_token).execute(&mut *tx).await?;
            tx.commit().await?;
            Ok(())
        }.await;
        match saved {
            Err(error) if error.is_database() && error.should_reconnect() => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            result => return result,
        }
    }
}

pub(super) struct Saved {
    publication: Value,
    response: Vec<u8>,
    terminal_unknown: bool,
}

pub(super) async fn claim(
    db: &Database,
    owner: ExecutorFence,
) -> Result<Option<(ClaimedItem, Saved)>, ExecutorError> {
    let mut tx = owner.begin_mode(db, false, true).await?;
    let Some(scope) = selected_collection_scope::filter_in(&mut tx).await? else {
        return Ok(None);
    };
    let row = sqlx::query(concat!(
        "WITH selected_collection AS (SELECT $1::uuid AS run_ref,$2::uuid AS domain_ref,$3::uuid[] AS work_refs)
         SELECT item.item_ref,item.batch_ref,item.position,item.input_ref,b.task_key,b.task_config_ref,b.kind,
                saved.invocation_ref,c.lease_generation,saved.lease_token,saved.publication,saved.response,c.state='failed' AS terminal_unknown
         FROM linggan_ai_returned_response saved JOIN linggan_ai_batch_item_control c USING(invocation_ref,item_ref)
         JOIN linggan_ai_batch_item item USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref)
         JOIN linggan_ai_batch_control bc USING(batch_ref)
         WHERE ((c.state='running' AND bc.state='open'
           AND (c.lease_expires_at<=clock_timestamp() OR NOT linggan_ai_executor_is_owner(c.lease_instance_ref,c.lease_instance_generation,false)))
             OR (c.state='failed' AND c.failure_code='ai_dispatch_unknown' AND saved.local_state IS NULL
                 AND (saved.reconcile_after IS NULL OR saved.reconcile_after<=clock_timestamp())))
           AND (($1::uuid IS NULL AND NOT (item.input_ref ? 'collectionRunRef')) OR (true ", include_str!("../selected_collection_item.sql"),
         ")) ORDER BY saved.received_at,c.item_ref LIMIT 1 FOR UPDATE OF saved,bc,c SKIP LOCKED"
    )).bind(scope.collection_run_ref).bind(scope.domain).bind(scope.works.as_deref())
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else { return Ok(None) };
    let saved = Saved {
        publication: row.get("publication"),
        response: row.get("response"),
        terminal_unknown: row.get("terminal_unknown"),
    };
    let token = row.get("lease_token");
    let claimed = if saved.terminal_unknown {
        // An interrupted local reconciliation can be claimed again. No dispatch lease is created.
        let item = queue::claimed_row(&row, owner, row.get("lease_generation"), token)?;
        sqlx::query("UPDATE linggan_ai_returned_response SET reconcile_after=clock_timestamp()+interval '30 seconds' WHERE invocation_ref=$1")
            .bind(row.get::<Uuid,_>("invocation_ref")).execute(&mut *tx).await?;
        tx.commit().await?;
        item
    } else {
        queue::lease_row(tx, owner, row, Some(token)).await?
    };
    Ok(Some((claimed, saved)))
}

pub(super) fn publication_result(result: Result<(), ExecutorError>) -> Result<(), ExecutorError> {
    match result {
        Err(e) if e.is_database() || matches!(e, ExecutorError::FenceLost) => {
            eprintln!("saved AI response awaits local publication: {}", e.code());
            Ok(())
        }
        other => other,
    }
}

pub(super) async fn publish(
    db: &Database,
    item: &ClaimedItem,
    saved: Saved,
) -> Result<(), ExecutorError> {
    let response: PiResponse =
        serde_json::from_slice(&saved.response).map_err(|_| ExecutorError::Invalid)?;
    let result = match item.task_key {
        TaskKey::ContentUnderstanding => {
            let publication =
                serde_json::from_value(saved.publication).map_err(|_| ExecutorError::Invalid)?;
            content_understanding::settle(
                db,
                item,
                &publication,
                Some(&response),
                saved.terminal_unknown,
            )
            .await
        }
        task if task.topic_stage().is_some() => {
            let publication =
                serde_json::from_value(saved.publication).map_err(|_| ExecutorError::Invalid)?;
            topic_research::settle(
                db,
                item,
                &publication,
                Some(&response),
                saved.terminal_unknown,
            )
            .await
        }
        _ => Err(ExecutorError::Invalid),
    };
    publication_result(result)
}

pub(super) async fn lock_publication<'a>(
    db: &'a Database,
    item: &ClaimedItem,
    invocation: Uuid,
    response: Option<&PiResponse>,
    terminal_unknown: bool,
) -> Result<(sqlx::Transaction<'a, sqlx::Postgres>, bool), ExecutorError> {
    if !terminal_unknown {
        return super::super::receipt::lock_in(db, item, invocation, response).await;
    }
    let mut tx = item.owner.begin(db, false).await?;
    let cancelled = sqlx::query_scalar("SELECT bc.cancel_requested FROM linggan_ai_returned_response saved
         JOIN linggan_ai_batch_item_control c USING(item_ref,invocation_ref)
         JOIN linggan_ai_batch_item item USING(item_ref) JOIN linggan_ai_batch_control bc USING(batch_ref)
         WHERE saved.invocation_ref=$1 AND saved.item_ref=$2 AND c.state='failed' AND c.failure_code='ai_dispatch_unknown'
           AND saved.local_state IS NULL FOR UPDATE OF saved,bc")
        .bind(invocation).bind(item.item_ref).fetch_optional(&mut *tx).await?.ok_or(ExecutorError::FenceLost)?;
    Ok((tx, cancelled))
}

pub(super) async fn reconcile_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invocation: Uuid,
    response: &PiResponse,
    success: bool,
    cancelled: bool,
    failure: Option<&str>,
) -> Result<(), ExecutorError> {
    let (item_ref,instance_ref,generation): (Uuid,Uuid,i64) = sqlx::query_as(
        "SELECT item_ref,instance_ref,lease_generation FROM linggan_ai_execution_attempt WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(&mut **tx).await?;
    super::super::receipt::record_late_usage_in(
        tx,
        invocation,
        item_ref,
        instance_ref,
        generation,
        response,
    )
    .await?;
    sqlx::query("UPDATE linggan_ai_returned_response SET reconciled_at=scope_001_now(),local_state=$2,local_failure_code=$3,reconcile_after=NULL WHERE invocation_ref=$1")
        .bind(invocation).bind(if cancelled {"cancelled"} else if success {"succeeded"} else {"failed"}).bind(failure).execute(&mut **tx).await?;
    Ok(())
}
