//! Serialize run control with the existing dispatcher without blocking shared queue fairness.
use crate::selected_collection_run::{self, ExecutionScope, SelectedCollectionRun};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Return only this run's material refresh orders. A linkage to the run is
/// insufficient if the order contains another work or an operations target.
const SELECTED_ORDER: &str = "SELECT orders.work_order_ref FROM collection_run_order orders
    JOIN collection_work_order w USING(work_order_ref)
    JOIN collection_run run USING(run_ref)
    JOIN collection_admission_decision decision ON decision.decision_ref=w.decision_ref
    WHERE orders.run_ref=$1 AND orders.stage='detail' AND orders.content_public_ref=ANY($2)
      AND run.phase='materials'
      AND w.target_ref=run.execution_target_ref AND decision.authorization_ref=run.authorization_ref
      AND EXISTS (SELECT 1 FROM collection_work_order_material_target material
                  WHERE material.work_order_ref=w.work_order_ref
                    AND material.content_public_ref=orders.content_public_ref AND material.acquire_media)
      AND NOT EXISTS (SELECT 1 FROM collection_work_order_material_target material
                      WHERE material.work_order_ref=w.work_order_ref
                        AND (material.content_public_ref<>orders.content_public_ref
                             OR material.comment_limit<>0 OR material.reply_expand_limit<>0))
      AND NOT EXISTS (SELECT 1 FROM collection_work_order_ops_publication_target ops
                      WHERE ops.work_order_ref=w.work_order_ref)";

pub(crate) async fn selected_orders_in(
    tx: &mut Transaction<'_, Postgres>,
    scope: &SelectedCollectionRun,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(SELECTED_ORDER)
        .bind(scope.run_ref)
        .bind(&scope.work_refs)
        .fetch_all(&mut **tx)
        .await
}

/// A run command locks the same identity before changing authority. Take that
/// lock before the row-level authority lock to preserve the existing lock order.
pub(crate) async fn lock_selected_run_in(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
) -> Result<Option<SelectedCollectionRun>, sqlx::Error> {
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,400))")
            .bind(run.to_string())
            .fetch_one(&mut **tx)
            .await?;
    if !locked {
        return Ok(None);
    }
    match selected_collection_run::lock_in(tx).await? {
        ExecutionScope::Selected(scope) if scope.run_ref == run => Ok(Some(scope)),
        _ => Ok(None),
    }
}

/// Used when issuing a lease or revalidating a previously issued task. Without an
/// explicit selection this preserves the existing lease and delivery rules.
pub(crate) async fn selected_order_block_in(
    tx: &mut Transaction<'_, Postgres>,
    order: Uuid,
) -> Result<Option<&'static str>, sqlx::Error> {
    match selected_collection_run::load_in(tx).await? {
        ExecutionScope::Unrestricted => Ok(None),
        ExecutionScope::Blocked => Ok(Some("selected_collection_run_unavailable")),
        ExecutionScope::Selected(scope) => {
            let Some(scope) = lock_selected_run_in(tx, scope.run_ref).await? else {
                return Ok(Some("selected_collection_run_unavailable"));
            };
            Ok((!selected_orders_in(tx, &scope).await?.contains(&order))
                .then_some("outside_selected_collection_run"))
        }
    }
}

pub(crate) async fn guard_order(
    tx: &mut Transaction<'_, Postgres>,
    order: Uuid,
    installation: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('collection_run_order') IS NOT NULL")
        .fetch_one(&mut **tx)
        .await?;
    if !ready {
        return Ok(selected_collection_run::configured()
            .then(|| "selected_collection_run_unavailable".into()));
    }
    let run: Option<Uuid> =
        sqlx::query_scalar("SELECT run_ref FROM collection_run_order WHERE work_order_ref=$1")
            .bind(order)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(run) = run else {
        return Ok(
            selected_collection_run::configured().then(|| "outside_selected_collection_run".into())
        );
    };
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,400))")
            .bind(run.to_string())
            .fetch_one(&mut **tx)
            .await?;
    if !locked {
        return Ok(Some("collection_run_control_in_progress".into()));
    }
    let (intent,status,mode,phase):(String,String,String,String)=sqlx::query_as("SELECT r.control_intent,d.status,r.comment_mode,r.phase FROM collection_run r JOIN observation_domain d USING(domain_ref) WHERE r.run_ref=$1").bind(run).fetch_one(&mut **tx).await?;
    if let Some(reason) = selected_order_block_in(tx, order).await? {
        return Ok(Some(reason.into()));
    }
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
async fn probe_installation(
    tx: &mut Transaction<'_, Postgres>,
    install_key: &str,
    credential: &str,
) -> Result<Uuid, crate::CollectionControlError> {
    let install: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation WHERE install_key=$1 AND superseded_at IS NULL",
    ).bind(install_key).fetch_optional(&mut **tx).await?;
    let install = install.ok_or(crate::CollectionControlError::UnknownInstallation)?;
    if !crate::collection_control::validate_installation_credential_in(tx, install, credential)
        .await?
    {
        return Err(crate::CollectionControlError::InvalidCredential);
    }
    Ok(install)
}

async fn held_probe_lease(
    tx: &mut Transaction<'_, Postgres>,
    task: Uuid,
    install: Uuid,
) -> Result<Uuid, crate::CollectionControlError> {
    sqlx::query_scalar("SELECT t.lease_ref FROM collection_work_order_lease_task t JOIN collection_work_order_lease l USING(lease_ref) WHERE t.task_id=$1 AND t.claimed_by_installation_ref=$2 AND l.released_at IS NULL AND l.expires_at>scope_001_now()")
        .bind(task).bind(install).fetch_optional(&mut **tx).await?
        .ok_or(crate::CollectionControlError::ClaimedTaskNotHeld)
}

pub async fn probe(
    db: &linggan_storage_postgres::Database,
    install_key: &str,
    credential: &str,
    task: Uuid,
) -> Result<serde_json::Value, crate::CollectionControlError> {
    let mut tx = db.pool().begin().await?;
    // Authenticate before waiting, but acquire no lease row locks before the run lock.
    // Commands and dispatch use run -> order/lease order; reversing it could deadlock.
    let install = probe_installation(&mut tx, install_key, credential).await?;
    let lease = held_probe_lease(&mut tx, task, install).await?;
    let run: Option<Uuid> = sqlx::query_scalar("SELECT r.run_ref FROM collection_run_order r JOIN collection_work_order_lease l USING(work_order_ref) WHERE l.lease_ref=$1")
        .bind(lease).fetch_optional(&mut *tx).await?;
    if let Some(run) = run {
        // Dispatcher try-lock contention is a scheduling deferral. The in-page probe is
        // different: mayAct=false permanently stops this Attempt. Wait for serialization
        // rather than inventing a stop while ordinary advancement owns the same lock.
        // Bound the wait below the plugin's 15s request deadline; timeout still fails closed.
        sqlx::query("SET LOCAL lock_timeout='5s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,400))")
            .bind(run.to_string())
            .execute(&mut *tx)
            .await?;
        // Authority may have changed while waiting. Recheck credentials and the live
        // task/lease binding after acquiring the run lock, before allowing another action.
        let current_install = probe_installation(&mut tx, install_key, credential).await?;
        if current_install != install || held_probe_lease(&mut tx, task, install).await? != lease {
            return Err(crate::CollectionControlError::ClaimedTaskNotHeld);
        }
    }
    let reason = guard_lease(&mut tx, lease, install).await?;
    tx.commit().await?;
    Ok(serde_json::json!({"mayAct":reason.is_none(),"reasonCode":reason}))
}
