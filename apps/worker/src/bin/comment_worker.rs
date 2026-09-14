//! Explicit local runner for COMMENT-RESEARCH-RESET-001.
//!
//! It advances only V1 work already created through the saved-policy/start-run path. It never
//! recreates the retired comment-analysis queue and never starts an external call in queue-only
//! mode.
use linggan_intelligence::{
    comment_research_kernel::{prewarm_current_sources, reset_development_derived},
    model_runner::{model_schema_ready, model_worker_heartbeat, run_model_work_once},
    model_secrets::model_secret_store,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| {
        !matches!(
            arg.as_str(),
            "--once" | "--execute" | "--derive-current" | "--reset-development-derived"
        )
    }) {
        return Err(
            "usage: linggan-comment-worker --execute [--once] | --derive-current | --reset-development-derived".into(),
        );
    }
    let reset = args.iter().any(|arg| arg == "--reset-development-derived");
    let execute = args.iter().any(|arg| arg == "--execute");
    let derive_current = args.iter().any(|arg| arg == "--derive-current");
    if [reset, execute, derive_current]
        .into_iter()
        .filter(|selected| *selected)
        .count()
        != 1
        || (args.iter().any(|arg| arg == "--once") && !execute)
    {
        return Err(
            "choose exactly one of --execute, --derive-current, or --reset-development-derived; --once requires --execute".into(),
        );
    }
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")
        .map_err(|_| "local database is not configured")?;
    let database = Database::connect(&url)
        .await
        .map_err(|_| "local database is unavailable")?;
    if !model_schema_ready(&database).await {
        return Err(
            "comment research V1 terminal migrations 0069–0081 are required; no work was started"
                .into(),
        );
    }
    if reset {
        let receipt = reset_development_derived(&database).await?;
        println!(
            "comment research V1 development reset: derivations={}, runs={}, runItems={}, atoms={}, problems={}, results={}",
            receipt.deleted_derivations,
            receipt.deleted_runs,
            receipt.deleted_run_items,
            receipt.deleted_atoms,
            receipt.deleted_problems,
            receipt.deleted_results,
        );
        return Ok(());
    }
    if derive_current {
        let derived = prewarm_current_sources(&database).await?;
        println!("comment research current derivations prewarmed: {derived}");
        return Ok(());
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
