//! Explicit local runner for COMMENT-RESEARCH-RESET-001.
//!
//! It advances only V1 work already created through the saved-policy/start-run path. It never
//! recreates the retired comment-analysis queue and never starts an external call in queue-only
//! mode.
use linggan_intelligence::{
    model_runner::{model_schema_ready, model_worker_heartbeat, run_model_work_once},
    model_secrets::model_secret_store,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|arg| !matches!(arg.as_str(), "--once" | "--execute"))
    {
        return Err("usage: linggan-comment-worker --execute [--once]".into());
    }
    if !args.iter().any(|arg| arg == "--execute") {
        return Err(
            "V1 has no queue-only compatibility mode; save a policy and start a Run in the UI"
                .into(),
        );
    }
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")
        .map_err(|_| "local database is not configured")?;
    let database = Database::connect(&url)
        .await
        .map_err(|_| "local database is unavailable")?;
    if !model_schema_ready(&database).await {
        return Err(
            "comment research V1 terminal migrations 0069–0070 are required; no work was started"
                .into(),
        );
    }
    let once = args.iter().any(|arg| arg == "--once");
    let store = model_secret_store();
    let adapter = PiAdapter::configured();
    loop {
        model_worker_heartbeat(&database, "running", None).await?;
        match run_model_work_once(&database, store.as_ref(), &adapter).await {
            Ok(worked) => {
                model_worker_heartbeat(&database, "idle", None).await?;
                if once || !worked {
                    println!("comment research V1 worker: completed step; work selected: {worked}");
                    return Ok(());
                }
            }
            Err(error) => {
                model_worker_heartbeat(&database, "error", Some(error.code())).await?;
                return Err(error.into());
            }
        }
    }
}
