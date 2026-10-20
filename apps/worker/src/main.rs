//! Collection patrol and deterministic catalog maintenance only.
//! AI calls and embeddings are not composed into this process. The AI executor has its own
//! session, switches and receipts; an interrupted collection tick is not a completed tick.

use std::process::ExitCode;

use linggan_evidence::{
    EVENT_READINESS, EVENT_STARTUP, READINESS_RETRY_START, RuntimeEvent, SERVICE_WORKER,
    next_readiness_retry, record_readiness, record_scheduler_started,
};
use linggan_storage_postgres::Database;

mod keyword_details;
mod maintenance;
mod shutdown;
mod tick;

use maintenance::MaintenanceTask;
use shutdown::{finish_worker_shutdown, wait_for_worker_shutdown, write_drain_ack};
use tick::{TICK_INTERVAL, observe_readiness, run_collection_tick};

#[tokio::main]
async fn main() -> ExitCode {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        // 没有地址不是「能跑但没有活干」，是一个不会自愈的配置错误：以失败退出，让
        // supervisor 按节流重启并把这件事暴露出来。用 SUCCESS 退出等于告诉系统一切正常。
        eprintln!(
            "linggan worker: LINGGAN_LOCAL_DATABASE_URL is not set; refusing to start \
             (no work can be claimed without a database)"
        );
        return ExitCode::FAILURE;
    };
    let (stop, _) = tokio::sync::watch::channel(false);
    let stop_on_signal = stop.clone();
    // A blocked patrol SQL await must not delay the shutdown signal.
    let mut shutdown = tokio::spawn(async move {
        wait_for_worker_shutdown().await;
        stop_on_signal.send_replace(true);
    });

    // 先说着，再说别的：判定本身可能有十几秒，而这十几秒里不能让日志空着——
    // 空日志与「一台没事可做的机器」是同一个样子，正是这一版要消灭的东西。
    println!("linggan worker: started; checking whether this machine can take collection work");
    // 事件说的是**哪个部署的哪个进程**在跑：revision 由部署脚本写、进程只读；读不到就是
    // `unknown`，不拿别的东西冒充（见 `runtime_event`）。
    RuntimeEvent::new(SERVICE_WORKER, EVENT_STARTUP).emit();

    let mut ticker =
        tokio::time::interval_at(tokio::time::Instant::now() + TICK_INTERVAL, TICK_INTERVAL);
    // 卡住的那一轮不该在恢复之后补跑一串：延后到下一个周期就够了。
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut database: Option<Database> = None;
    let mut maintenance: Option<MaintenanceTask> = None;
    // 只在状态**变化**时说话：持续故障每分钟重复一行会把日志变成噪音，而噪音里看不见变化。
    let mut announced: Option<&'static str> = None;
    let mut tick_announced = false;
    let mut retry_after = READINESS_RETRY_START;

    // Cancel only the collection loop. Local maintenance finishes independently below.
    // An interrupted tick retains its unfinished ledger row; cancellation is not completion.
    tokio::select! {
        biased;
        _ = &mut shutdown => {},
        _ = async {
            loop {
                let readiness = observe_readiness(&url, &mut database).await;
                if maintenance.is_none() {
                    if let Some(connection)=database.as_ref() {
                        if record_scheduler_started(connection,uuid::Uuid::new_v4()).await.is_err() {
                            eprintln!("linggan worker: scheduler_identity_unavailable");
                        }
                        maintenance=Some(tokio::spawn(maintenance::run(connection.clone(),stop.subscribe())));
                    }
                }
                if !readiness.is_ready() {
                    if announced != Some(readiness.state.code()) {
                        println!(
                            "linggan worker: not ready ({}{}); no work is claimed until this clears",
                            readiness.state.code(),
                            readiness
                                .detail
                                .as_deref()
                                .map(|detail| format!(": {detail}"))
                                .unwrap_or_default(),
                        );
                        emit_readiness(&readiness);
                        announced = Some(readiness.state.code());
                    }
                    // 未就绪同样要落心跳，而且是**每轮都落**：日志只在状态变化时说话（上面那句），
                    // 但心跳那一行是「最后一次判定」——只写第一次，`readiness_checked_at` 会永远停在
                    // 故障开始的那一刻，读它的人分不清「现在还接不了活」和「这台机器说完那句话就死了」。
                    // 连不上数据库时 `database` 还是 None，没有可写之处：那种情况由 `/health` 说。
                    if let Some(database) = database.as_ref() {
                        if let Err(error) = record_readiness(database, &readiness).await {
                            println!("linggan worker: cannot record readiness: {error}");
                        }
                    }
                    tokio::time::sleep(retry_after).await;
                    retry_after = next_readiness_retry(retry_after);
                    continue;
                }
                if let Some(previous) = announced.take() {
                    println!("linggan worker: ready again (was {previous}); resuming work");
                    emit_readiness(&readiness);
                }
                retry_after = READINESS_RETRY_START;
                if !tick_announced {
                    println!(
                        "linggan worker: patrol tick every {}s",
                        TICK_INTERVAL.as_secs()
                    );
                    tick_announced = true;
                }
                let database = database
                    .as_ref()
                    .expect("a ready probe always has a connection behind it");
                // 就绪这件事写进心跳：进程被杀掉时，日志没了，心跳里那行还在。
                if let Err(error) = record_readiness(database, &readiness).await {
                    println!("linggan worker: cannot record readiness: {error}");
                }
                run_collection_tick(database).await;
                ticker.tick().await;
            }
        } => {},
    }

    stop.send_replace(true);
    println!("linggan worker: shutdown requested; finishing local maintenance");
    match (database.as_ref(), maintenance.as_mut()) {
        (Some(_), Some(task)) => finish_worker_shutdown(task).await,
        _ => {
            let _ = write_drain_ack("failed");
            eprintln!("linggan worker: database_unverified_at_shutdown");
            ExitCode::FAILURE
        }
    }
}

/// 一次就绪判定写一行事件。人读的散文行与机器读的事件都在状态**变化**时出现。
fn emit_readiness(readiness: &linggan_evidence::RuntimeReadiness) {
    let mut event =
        RuntimeEvent::new(SERVICE_WORKER, EVENT_READINESS).with_outcome(readiness.state.code());
    if let Some(detail) = readiness.detail.as_deref() {
        event = event.with_reason(detail);
    }
    event.emit();
}
