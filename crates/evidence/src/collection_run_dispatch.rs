//! Serialize run control with the existing dispatcher without blocking shared queue fairness.
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn guard_order(
    tx: &mut Transaction<'_, Postgres>,
    order: Uuid,
    installation: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('collection_run_order') IS NOT NULL")
        .fetch_one(&mut **tx)
        .await?;
    if !ready {
        return Ok(None);
    }
    let run: Option<Uuid> =
        sqlx::query_scalar("SELECT run_ref FROM collection_run_order WHERE work_order_ref=$1")
            .bind(order)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(run) = run else { return Ok(None) };
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,400))")
            .bind(run.to_string())
            .fetch_one(&mut **tx)
            .await?;
    if !locked {
        return Ok(Some("collection_run_control_in_progress".into()));
    }
    let (intent,status,mode,phase):(String,String,String,String)=sqlx::query_as("SELECT r.control_intent,d.status,r.comment_mode,r.phase FROM collection_run r JOIN observation_domain d USING(domain_ref) WHERE r.run_ref=$1").bind(run).fetch_one(&mut **tx).await?;
    let reason = if intent != "running" {
        Some(format!("collection_run_{intent}"))
    } else if matches!(phase.as_str(), "ended" | "blocked") {
        Some(format!("collection_run_{phase}"))
    } else if status != "active" {
        Some("collection_run_domain_paused".into())
    } else {
        let protocol:bool=sqlx::query_scalar("SELECT capabilities @> '[\"collection.single_run.v1\"]'::jsonb FROM plugin_installation WHERE installation_ref=$1").bind(installation).fetch_one(&mut **tx).await?;
        if !protocol {
            Some("collection_run_protocol_unavailable".into())
        }
        // Until the complete bounded deep collector is negotiated, never silently sample a
        // person's all-public request. Its durable scope stays waiting in the same queue.
        // A targeted count runs on the same traversal, so it needs the same negotiated collector.
        else if matches!(mode.as_str(), "all_public" | "up_to") {
            let deep:bool=sqlx::query_scalar("SELECT capabilities @> '[\"collection.all_public.v1\"]'::jsonb FROM plugin_installation WHERE installation_ref=$1").bind(installation).fetch_one(&mut **tx).await?;
            (!deep).then(|| "collection_all_public_protocol_unavailable".into())
        } else {
            None
        }
    };
    Ok(reason)
}
pub(crate) async fn guard_lease(
    tx: &mut Transaction<'_, Postgres>,
    lease: Uuid,
    installation: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let order: Uuid = sqlx::query_scalar(
        "SELECT work_order_ref FROM collection_work_order_lease WHERE lease_ref=$1",
    )
    .bind(lease)
    .fetch_one(&mut **tx)
    .await?;
    guard_order(tx, order, installation).await
}
pub async fn probe(
    db: &linggan_storage_postgres::Database,
    install_key: &str,
    credential: &str,
    task: Uuid,
) -> Result<serde_json::Value, crate::CollectionControlError> {
    let mut tx = db.pool().begin().await?;
    let install:Option<Uuid>=sqlx::query_scalar("SELECT installation_ref FROM plugin_installation WHERE install_key=$1 AND superseded_at IS NULL").bind(install_key).fetch_optional(&mut *tx).await?;
    let install = install.ok_or(crate::CollectionControlError::UnknownInstallation)?;
    if !crate::collection_control::validate_installation_credential_in(&mut tx, install, credential)
        .await?
    {
        return Err(crate::CollectionControlError::InvalidCredential);
    }
    let lease:Option<Uuid>=sqlx::query_scalar("SELECT t.lease_ref FROM collection_work_order_lease_task t JOIN collection_work_order_lease l USING(lease_ref) WHERE t.task_id=$1 AND t.claimed_by_installation_ref=$2 AND l.released_at IS NULL AND l.expires_at>scope_001_now()").bind(task).bind(install).fetch_optional(&mut *tx).await?;
    let lease = lease.ok_or(crate::CollectionControlError::ClaimedTaskNotHeld)?;
    let reason = guard_lease(&mut tx, lease, install).await?;
    tx.commit().await?;
    Ok(serde_json::json!({"mayAct":reason.is_none(),"reasonCode":reason}))
}
