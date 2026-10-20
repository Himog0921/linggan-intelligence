//! Dedicated AI execution only. It never performs collection, media work or database migration.
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_intelligence::{
    ai_foundation::executor::service, model_secrets::model_secret_store,
    model_worker_drain::ModelWorkerDrain, pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use std::process::ExitCode;

#[path = "ai_worker_support/ack.rs"]
mod ack;

#[tokio::main]
async fn main() -> ExitCode {
    let outcome = run().await;
    let value = match &outcome {
        Ok(receipt) => {
            serde_json::json!({"state":"drained","processId":std::process::id(),"receipt":receipt})
        }
        Err(code) => {
            serde_json::json!({"state":"failed","processId":std::process::id(),"reason":code})
        }
    };
    if ack::write(&value).is_err() {
        eprintln!("linggan AI worker: ai_drain_ack_unavailable");
        return ExitCode::FAILURE;
    }
    match outcome {
        Ok(_) => ExitCode::SUCCESS,
        Err(code) => {
            eprintln!("linggan AI worker: {code}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<service::DrainReceipt, &'static str> {
    let url =
        std::env::var("LINGGAN_LOCAL_DATABASE_URL").map_err(|_| "ai_database_not_configured")?;
    let concurrency = std::env::var("LINGGAN_AI_CONCURRENCY")
        .map_or(Ok(AI_LIMITS.default_concurrency), |value| {
            value.parse::<u32>()
        })
        .map_err(|_| "ai_concurrency_invalid")?;
    if !(AI_LIMITS.default_concurrency..=AI_LIMITS.max_concurrency).contains(&concurrency) {
        return Err("ai_concurrency_invalid");
    }
    linggan_evidence::RuntimeEvent::new(
        linggan_evidence::SERVICE_AI_WORKER,
        linggan_evidence::EVENT_STARTUP,
    )
    .emit();
    let db = tokio::time::timeout(std::time::Duration::from_secs(10), Database::connect(&url))
        .await
        .map_err(|_| "ai_database_unavailable")?
        .map_err(|_| "ai_database_unavailable")?;
    let drain = ModelWorkerDrain::new();
    let signal = drain.clone();
    let signal_task = tokio::spawn(async move {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM is available");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
        signal.request();
    });
    let result = service::run(
        db,
        linggan_evidence::runtime_revision(),
        concurrency,
        model_secret_store(),
        PiAdapter::configured(),
        drain,
    )
    .await;
    signal_task.abort();
    result.map_err(|error| error.code())
}
