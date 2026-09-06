//! Standalone comment runner for isolated proof or explicit operation; the normal worker
//! composes the same loop. Queue-only compatibility never invokes a provider.
use linggan_evidence::comment_research_read::comment_research_schema_is_ready;
use linggan_intelligence::comment_analysis::{UNCONFIGURED_MODEL, sync_comment_analysis_work};
use linggan_storage_postgres::Database;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| !matches!(a.as_str(), "--once" | "--queue-only" | "--execute"))
        || (args.iter().any(|a| a == "--queue-only") == args.iter().any(|a| a == "--execute"))
    {
        return Err("usage: linggan-comment-worker (--queue-only | --execute) [--once]".into());
    }
    let url = std::env::var("LINGGAN_LOCAL_DATABASE_URL")
        .map_err(|_| "local database is not configured")?;
    let database = Database::connect(&url)
        .await
        .map_err(|_| "local database is unavailable")?;
    if !comment_research_schema_is_ready(&database)
        .await
        .map_err(|_| "schema read failed")?
    {
        return Err("comment research migration 0039 is required; no work was started".into());
    }
    let once = args.iter().any(|a| a == "--once");
    if args.iter().any(|a| a == "--execute") {
        use linggan_intelligence::{model_runner::*, model_secrets::*, pi_adapter::*};
        if !model_schema_ready(&database).await {
            return Err("model migration 0040 is required".into());
        }
        if once {
            model_worker_heartbeat(&database, "running", None).await?;
            let result = run_model_work_once(
                &database,
                model_secret_store().as_ref(),
                &PiAdapter::configured(),
            )
            .await;
            match result {
                Ok(worked) => {
                    model_worker_heartbeat(&database, "idle", None).await?;
                    println!("comment model worker: completed step; work selected: {worked}");
                }
                Err(e) => {
                    model_worker_heartbeat(&database, "error", Some(e.code())).await?;
                    return Err(e.into());
                }
            }
        } else {
            run_model_worker(database).await;
        }
        return Ok(());
    }
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    loop {
        interval.tick().await;
        let count = sync_comment_analysis_work(&database, UNCONFIGURED_MODEL)
            .await
            .map_err(|_| "comment queue synchronization failed")?;
        if count > 0 || once {
            println!(
                "comment worker: {count} source versions queued; model NOT_CONFIGURED; no source text was sent"
            );
        }
        if once {
            return Ok(());
        }
    }
}
