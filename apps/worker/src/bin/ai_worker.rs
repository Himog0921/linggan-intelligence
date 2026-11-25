//! Dedicated AI execution only. It never performs collection, media work or database migration.
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_intelligence::{
    ai_foundation::executor::{ExecutorError, ExecutorSession, service}, model_secrets::model_secret_store,
    model_worker_drain::ModelWorkerDrain, pi_adapter::PiAdapter,
};
use linggan_storage_postgres::{Database, StorageError};
use std::{process::ExitCode, time::Duration};

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
    let drain = ModelWorkerDrain::new();
    let signal = drain.clone();
    let signal_task = tokio::spawn(async move {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM is available");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
        signal.request();
    });
    let result = match connect_database(&url, &drain).await {
        Ok(db) => run_sessions(db, concurrency, drain).await,
        Err(code) => Err(code),
    };
    signal_task.abort();
    result
}

async fn connect_database(url: &str, stop: &ModelWorkerDrain) -> Result<Database, &'static str> {
    let mut stopping = stop.subscribe();
    let mut backoff = Duration::from_secs(1);
    loop {
        if stop.is_requested() {
            return Err("ai_database_unavailable");
        }
        let result = tokio::select! {
            _ = stopping.changed() => return Err("ai_database_unavailable"),
            result = tokio::time::timeout(Duration::from_secs(10), Database::connect(url)) => result,
        };
        match result {
            Ok(Ok(db)) => return Ok(db),
            Ok(Err(StorageError::Connect(error))) => {
                if !ExecutorError::Database(error).should_reconnect() {
                    return Err("ai_database_unavailable");
                }
            }
            Err(_) => {},
            Ok(Err(_)) => return Err("ai_database_unavailable"),
        }
        eprintln!("linggan AI worker: ai_database_unavailable (waiting for startup)");
        tokio::select! {
            _ = stopping.changed() => return Err("ai_database_unavailable"),
            _ = tokio::time::sleep(backoff) => {},
        }
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn run_sessions(
    db: Database,
    concurrency: u32,
    stop: ModelWorkerDrain,
) -> Result<service::DrainReceipt, &'static str> {
    let secrets = model_secret_store();
    let adapter = PiAdapter::configured();
    let mut stopping = stop.subscribe();
    let mut backoff = Duration::from_secs(1);
    let mut reconnecting = false;
    loop {
        let drain = ModelWorkerDrain::new();
        let session_started = tokio::time::Instant::now();
        if stop.is_requested() {
            drain.request();
        }
        let session = service::run(
            db.clone(),
            linggan_evidence::runtime_revision(),
            concurrency,
            secrets.clone(),
            adapter.clone(),
            drain.clone(),
        );
        tokio::pin!(session);
        let result = tokio::select! {
            biased;
            _ = stopping.changed() => { drain.request(); session.await }
            result = &mut session => result,
        };
        let error = match result {
            Err(error) if !stop.is_requested() && error.should_reconnect() => error,
            Err(ExecutorError::Busy) if reconnecting && !stop.is_requested() => {
                // A timed-out backend can still hold its advisory lock briefly.
                // A different live process remains authoritative.
                let owner = tokio::select! {
                    _ = stopping.changed() => return Err(ExecutorError::Busy.code()),
                    result = ExecutorSession::current_process_id(&db) => result.map_err(|error| error.code())?,
                };
                if owner != Some(std::process::id() as i32) {
                    return Err(ExecutorError::Busy.code());
                }
                ExecutorError::Busy
            }
            result => return result.map_err(|error| error.code()),
        };
        reconnecting = true;
        eprintln!("linggan AI worker: {} (reconnecting)", error.code());
        if session_started.elapsed() >= Duration::from_secs(30) {
            backoff = Duration::from_secs(1);
        }
        tokio::select! {
            biased;
            _ = stopping.changed() => return Err(error.code()),
            _ = tokio::time::sleep(backoff) => {},
        }
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}
