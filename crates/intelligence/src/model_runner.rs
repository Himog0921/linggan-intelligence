//! Bounded execution loop for the one current comment-research kernel.
//!
//! The model worker owns no scheduler and has no legacy fallback. A saved V1 policy plus a
//! user-created Run is the sole source of external work; a tick with no pending V1 step is idle.

use crate::{
    comment_research_kernel::prewarm_current_sources,
    comment_research_worker,
    model_secrets::{ModelSecretStore, model_secret_store},
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;

pub async fn run_model_work_once(
    database: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    comment_research_worker::run_once(database, store, adapter, &ModelWorkerDrain::new()).await
}

pub async fn model_schema_ready(database: &Database) -> bool {
    comment_research_worker::schema_ready(database)
        .await
        .unwrap_or(false)
}

pub async fn run_model_worker(database: Database) {
    let _ = run_model_worker_with_drain(database, ModelWorkerDrain::new()).await;
}

pub async fn run_model_worker_with_drain(
    database: Database,
    drain: ModelWorkerDrain,
) -> Result<(), ModelError> {
    let store = model_secret_store();
    let adapter = PiAdapter::configured();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    let mut shutdown = drain.subscribe();
    loop {
        if drain.is_requested() {
            break;
        }
        tokio::select! {
            _ = interval.tick() => {}
            changed = shutdown.changed() => {
                if changed.is_ok() && *shutdown.borrow() {
                    break;
                }
            }
        }
        if drain.is_requested() || !model_schema_ready(&database).await {
            continue;
        }
        // Keep the read projection on the current input contract before serving any queued
        // work. This is deterministic local derivation only: it never saves a policy, creates a
        // Run, or contacts a provider. In particular, a derivation-version cutover must not make
        // the read-only Voices view empty until a user happens to preview a Run.
        if let Err(error) = prewarm_current_sources(&database).await {
            eprintln!("comment research input derivation failed: {error}");
            model_worker_heartbeat(&database, "error", Some("derivation_refresh_failed")).await?;
            continue;
        }
        model_worker_heartbeat(&database, "running", None).await?;
        match comment_research_worker::run_once(&database, store.as_ref(), &adapter, &drain).await {
            Ok(_) => model_worker_heartbeat(&database, "idle", None).await?,
            Err(error) => {
                eprintln!("comment research V1 worker: {}", error.code());
                model_worker_heartbeat(&database, "error", Some(error.code())).await?;
                if drain.is_requested() {
                    return Err(error);
                }
            }
        }
    }
    model_worker_heartbeat(&database, "idle", None).await?;
    Ok(())
}

pub async fn model_worker_heartbeat(
    database: &Database,
    state: &str,
    error: Option<&str>,
) -> Result<(), ModelError> {
    sqlx::query(
        "UPDATE linggan_model_workspace \
         SET worker_last_seen_at=scope_001_now(),worker_state=$1,worker_last_error=$2 \
         WHERE singleton",
    )
    .bind(state)
    .bind(error)
    .execute(database.pool())
    .await?;
    Ok(())
}
