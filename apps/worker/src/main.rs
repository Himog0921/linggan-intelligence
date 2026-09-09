//! Linggan Intelligence 的巡检 tick。
//!
//! 这是全系统第一个会自己动的东西。在它之前每一步都要人去戳一下。
//!
//! **tick 只做轻量的状态推进与入队**（产品规则 §6.3，取自内容工作台的教训：那边的调度
//! tick 把媒体下载和转录塞进同一个 120 秒事务里，任何超时的 tick 都会丢锁并以异常收场）。
//! 这里不下载、不转录、不访问任何平台——它只判断「谁到期了」，然后走一遍与人点按钮
//! 完全相同的授权链。
//!
//! MODEL-PI-001 additionally composes an independent asynchronous comment model loop.
//! Its bounded external calls never run inside or block the patrol tick below.
//!
//! **自动化不是豁免权**：准入若说资源不够、风险暂停生效或额度触顶，tick 也只能记下理由
//! 然后等下一轮。

use std::{path::PathBuf, process::ExitCode, time::Duration};

use linggan_intelligence::{
    model_runner::{model_worker_heartbeat, run_model_worker_with_drain},
    model_worker_drain::{MODEL_WORKER_DRAIN_GRACE, ModelWorkerDrain},
};

/// 两次扫描的间隔。
///
/// 这是**扫描**频率，不是**巡检**频率：每个目标多久看一次由它自己的
/// `patrol_interval_seconds` 决定（默认 24 小时）。扫描频繁一点只是让到期的目标不必等
/// 太久才被发现，代价是一次几乎为空的查询。
const TICK_INTERVAL: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> ExitCode {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        // 没有数据库就什么都不做，而不是拿一个默认连接串去猜。
        println!("linggan worker: LINGGAN_LOCAL_DATABASE_URL is not set; no jobs are enabled");
        return ExitCode::SUCCESS;
    };
    let database = match linggan_storage_postgres::Database::connect(&url).await {
        Ok(database) => database,
        Err(error) => {
            println!("linggan worker: cannot reach the local database: {error}");
            return ExitCode::SUCCESS;
        }
    };
    let drain = ModelWorkerDrain::new();
    let drain_on_signal = drain.clone();
    // Listen independently: a patrol database await must not delay closing model reservations.
    let mut shutdown = tokio::spawn(async move {
        wait_for_worker_shutdown().await;
        drain_on_signal.request();
    });
    let mut model_worker =
        tokio::spawn(run_model_worker_with_drain(database.clone(), drain.clone()));
    println!(
        "linggan worker: patrol tick every {}s",
        TICK_INTERVAL.as_secs()
    );
    let worker_instance_ref = uuid::Uuid::new_v4();
    if let Err(error) =
        linggan_evidence::record_scheduler_started(&database, worker_instance_ref).await
    {
        println!("linggan worker: cannot record scheduler identity: {error}");
    }
    match linggan_evidence::ensure_discovery_cover_media_work(&database).await {
        Ok(count) if count > 0 => {
            println!("linggan worker: projected {count} discovery covers into media acquisition")
        }
        Ok(_) => {}
        Err(error) => println!("linggan worker: media acquisition projection unavailable: {error}"),
    }

    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            _ = ticker.tick() => {}
        }
        if let Err(error) = linggan_evidence::ensure_discovery_cover_media_work(&database).await {
            println!("linggan worker: media acquisition projection failed: {error}");
        }
        match linggan_evidence::run_progressive_archives(&database).await {
            Ok(summary) if !summary.queued.is_empty() || !summary.skipped.is_empty() => {
                println!(
                    "linggan worker: progressive dossiers queued {}, skipped {}",
                    summary.queued.len(),
                    summary.skipped.len()
                );
            }
            Ok(_) => {}
            Err(error) => println!("linggan worker: progressive dossier tick failed: {error}"),
        }
        match linggan_evidence::run_due_patrols(&database).await {
            Ok(summary) => {
                // 只在真的发生了什么时说话。一个每分钟打印「本轮 0 个」的 tick 会让日志
                // 变成噪音，等真有事发生时反而看不见。
                if !summary.dispatched.is_empty() || !summary.skipped.is_empty() {
                    println!(
                        "linggan worker: dispatched {}, skipped {}",
                        summary.dispatched.len(),
                        summary.skipped.len()
                    );
                    // 跳过的理由逐条打印：一个只报「本轮派了 3 个」的调度器，在没派的
                    // 时候没人说得清为什么。
                    for (target_ref, reason) in &summary.skipped {
                        println!("linggan worker: skipped {target_ref}: {reason}");
                    }
                }
            }
            Err(error) => println!("linggan worker: patrol tick failed: {error}"),
        }
    }
    drain.request();
    println!("linggan worker: shutdown requested; stopping new model reservations");
    finish_worker_shutdown(&database, &mut model_worker).await
}

async fn finish_worker_shutdown(
    database: &linggan_storage_postgres::Database,
    model_worker: &mut tokio::task::JoinHandle<
        Result<(), linggan_intelligence::model_settings::ModelError>,
    >,
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
                model_worker_heartbeat(&database, "error", Some("drain_finalization_failed")).await;
            write_drain_ack("failed");
            ExitCode::FAILURE
        }
        Ok(Err(error)) => {
            eprintln!("linggan worker: model drain task failed: {error}");
            let _ = model_worker_heartbeat(&database, "error", Some("drain_task_failed")).await;
            write_drain_ack("failed");
            ExitCode::FAILURE
        }
        Err(_) => {
            model_worker.abort();
            let _ = model_worker.await;
            let _ = model_worker_heartbeat(&database, "error", Some("drain_timeout")).await;
            write_drain_ack("timed_out");
            eprintln!("linggan worker: model drain exceeded its bounded grace");
            ExitCode::FAILURE
        }
    }
}

async fn wait_for_worker_shutdown() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("SIGTERM handler is available on macOS");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

fn write_drain_ack(state: &str) {
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
