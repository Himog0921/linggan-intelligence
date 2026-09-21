//! 关停：等模型循环把在途预留收干净，再写收条、定退出码。
//!
//! 单独一个文件是为了守住入口文件的 200 行上限（治理脚本守着）。这里的语义由 T29 定下：
//! **连不上数据库不以 SUCCESS 退出**——那是把故障说成正常；`drain` 收条是部署脚本
//! （`scripts/runtime/sync.sh`）等待的信号，它有三种取值，缺一种都会让脚本猜。

use std::path::PathBuf;
use std::process::ExitCode;

use linggan_intelligence::model_runner::model_worker_heartbeat;
use linggan_intelligence::model_worker_drain::MODEL_WORKER_DRAIN_GRACE;
use linggan_storage_postgres::Database;

pub type ModelWorkerTask =
    tokio::task::JoinHandle<Result<(), linggan_intelligence::model_settings::ModelError>>;

pub async fn finish_worker_shutdown(
    database: &Database,
    model_worker: &mut ModelWorkerTask,
) -> ExitCode {
    match tokio::time::timeout(MODEL_WORKER_DRAIN_GRACE, &mut *model_worker).await {
        Ok(Ok(Ok(()))) => {
            write_drain_ack("drained");
            println!("linggan worker: model drain confirmed");
            ExitCode::SUCCESS
        }
        Ok(Ok(Err(error))) => {
            eprintln!(
                "linggan worker: model receipt finalization failed: {}",
                error.code()
            );
            let _ =
                model_worker_heartbeat(database, "error", Some("drain_finalization_failed")).await;
            write_drain_ack("failed");
            ExitCode::FAILURE
        }
        Ok(Err(error)) => {
            eprintln!("linggan worker: model drain task failed: {error}");
            let _ = model_worker_heartbeat(database, "error", Some("drain_task_failed")).await;
            write_drain_ack("failed");
            ExitCode::FAILURE
        }
        Err(_) => {
            model_worker.abort();
            let _ = model_worker.await;
            let _ = model_worker_heartbeat(database, "error", Some("drain_timeout")).await;
            write_drain_ack("timed_out");
            eprintln!("linggan worker: model drain exceeded its bounded grace");
            ExitCode::FAILURE
        }
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

/// 收条是给部署脚本（`scripts/runtime/sync.sh`）等的信号，三条退出路径各写各的取值。
pub(crate) fn write_drain_ack(state: &str) {
    let path = std::env::var_os("LINGGAN_WORKER_DRAIN_ACK_PATH")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LINGGAN_SUPPORT_DIR").map(|root| {
                PathBuf::from(root)
                    .join("runtime-drain")
                    .join("worker-drain-ack")
            })
        });
    let Some(path) = path else {
        eprintln!("linggan worker: no drain acknowledgement path is configured");
        return;
    };
    let Some(parent) = path.parent() else {
        eprintln!("linggan worker: drain acknowledgement path has no parent");
        return;
    };
    if let Err(error) = std::fs::create_dir_all(parent).and_then(|_| {
        std::fs::write(
            &path,
            format!("pid={}\nstate={state}\n", std::process::id()),
        )
    }) {
        eprintln!("linggan worker: cannot persist drain acknowledgement: {error}");
    }
}
