//! Patrol shutdown waits for deterministic local writes; it never drains a model owner.
use crate::maintenance::MaintenanceTask;
use std::{
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    process::ExitCode,
};

pub async fn finish_worker_shutdown(task: &mut MaintenanceTask) -> ExitCode {
    let state = match tokio::time::timeout(
        crate::maintenance::ATTEMPT_LIMIT + std::time::Duration::from_secs(1),
        &mut *task,
    )
    .await
    {
        Ok(Ok(Ok(()))) => "drained",
        Ok(_) => "failed",
        Err(_) => {
            task.abort();
            let _ = task.await;
            "timed_out"
        }
    };
    if write_drain_ack(state).is_err() {
        eprintln!("linggan worker: drain_ack_unavailable");
        return ExitCode::FAILURE;
    }
    if state == "drained" {
        ExitCode::SUCCESS
    } else {
        eprintln!("linggan worker: local_maintenance_drain_failed");
        ExitCode::FAILURE
    }
}

pub async fn wait_for_worker_shutdown() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("SIGTERM handler is available on macOS");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

pub(crate) fn write_drain_ack(state: &str) -> io::Result<()> {
    let path = std::env::var_os("LINGGAN_WORKER_DRAIN_ACK_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LINGGAN_SUPPORT_DIR")
                .map(|root| PathBuf::from(root).join("runtime-drain/worker-drain-ack"))
        })
        .ok_or_else(|| io::Error::other("drain_ack_not_configured"))?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("drain_ack_invalid"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".patrol-drain-{}-{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        write!(&mut file, "pid={}\nstate={state}\n", std::process::id())?;
        file.sync_all()?;
        std::fs::rename(&temp, &path)?;
        std::fs::File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
