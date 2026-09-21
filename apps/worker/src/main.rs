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
//!
//! **但「干不了活」与「没有活」必须分得开**（COLLECTION-UPGRADE-001 · T29）。数据库连不上、
//! 迁移台账读不到、迁移没跑、schema 不兼容时，这个进程以前会照常启动、照常每 60 秒走一遍、
//! 一行「本轮 0 个」都不打——和一台空闲机器长得一模一样。现在每轮先判就绪（`runtime_readiness`）：
//! 未就绪就不碰任何采集步骤，按退避重试，状态变化时才说话。连不上数据库绝不以 SUCCESS 退出
//! （那是把故障说成正常），也绝不空转热重启：进程留在原地等它回来。这道闸只管**采集面**——
//! 模型评论循环是另一条通道，有自己的表，只要有连接就照常跑（见 tick 循环里的注释）。

use std::{path::PathBuf, process::ExitCode, time::Duration};

use linggan_evidence::{
    COLLECTION_RUNTIME_REQUIREMENTS, READINESS_RETRY_START, RuntimeReadiness,
    connect_runtime_readiness, next_readiness_retry, probe_runtime_readiness,
};
use linggan_intelligence::{
    model_runner::{model_worker_heartbeat, run_model_worker_with_drain},
    model_worker_drain::{MODEL_WORKER_DRAIN_GRACE, ModelWorkerDrain},
};
use linggan_storage_postgres::Database;

/// 两次扫描的间隔。
///
/// 这是**扫描**频率，不是**巡检**频率：多久看一次由每条监控规则自己的周期决定
/// （`collection_monitor_rule_revision.fixed_interval_seconds`，默认 24 小时；一个关键词
/// 可以有几条口径各设各的周期）。扫描频繁一点只是让到期的规则不必等太久才被发现，
/// 代价是一次几乎为空的查询。
const TICK_INTERVAL: Duration = Duration::from_secs(60);

mod keyword_details;

type ModelWorkerTask =
    tokio::task::JoinHandle<Result<(), linggan_intelligence::model_settings::ModelError>>;

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
    let drain = ModelWorkerDrain::new();
    let drain_on_signal = drain.clone();
    // Listen independently: a patrol database await must not delay closing model reservations.
    let mut shutdown = tokio::spawn(async move {
        wait_for_worker_shutdown().await;
        drain_on_signal.request();
    });

    // 先说着，再说别的：判定本身可能有十几秒，而这十几秒里不能让日志空着——
    // 空日志与「一台没事可做的机器」是同一个样子，正是这一版要消灭的东西。
    println!("linggan worker: started; checking whether this machine can take collection work");

    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    // 卡住的那一轮不该在恢复之后补跑一串：延后到下一个周期就够了。
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut database: Option<Database> = None;
    let mut model_worker: Option<ModelWorkerTask> = None;
    // 只在状态**变化**时说话：持续故障每分钟重复一行会把日志变成噪音，而噪音里看不见变化。
    let mut announced: Option<&'static str> = None;
    let mut tick_announced = false;
    let mut retry_after = READINESS_RETRY_START;

    loop {
        let readiness = observe_readiness(&url, &mut database).await;
        // 模型评论循环**不**由采集面的就绪判据决定：它有自己的表，采集面的缺口不该把另一条
        // 通道连坐停摆（要求清单是每消费者各主张的下限，不是一个总闸）。它的判据是「有没有
        // 连接」——连不上时把它跑起来，只会制造一屋子错误日志。
        if model_worker.is_none() {
            if let Some(connection) = database.as_ref() {
                let worker_instance_ref = uuid::Uuid::new_v4();
                if let Err(error) =
                    linggan_evidence::record_scheduler_started(connection, worker_instance_ref)
                        .await
                {
                    println!("linggan worker: cannot record scheduler identity: {error}");
                }
                model_worker = Some(tokio::spawn(run_model_worker_with_drain(
                    connection.clone(),
                    drain.clone(),
                )));
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
                announced = Some(readiness.state.code());
            }
            tokio::select! {
                _ = &mut shutdown => break,
                _ = tokio::time::sleep(retry_after) => {}
            }
            retry_after = next_readiness_retry(retry_after);
            continue;
        }
        if let Some(previous) = announced.take() {
            println!("linggan worker: ready again (was {previous}); resuming work");
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
        run_tick_steps(database).await;
        tokio::select! {
            _ = &mut shutdown => break,
            _ = ticker.tick() => {}
        }
    }

    drain.request();
    println!("linggan worker: shutdown requested; stopping new model reservations");
    match (database.as_ref(), model_worker.as_mut()) {
        (Some(database), Some(model_worker)) => {
            finish_worker_shutdown(database, model_worker).await
        }
        // 一次都没连上数据库，模型循环也就没起过：没有在途预留要等；但「它没起过」要如实说出来。
        _ => {
            write_drain_ack("drained");
            println!("linggan worker: the model loop was never started; nothing to drain");
            ExitCode::SUCCESS
        }
    }
}

/// 一次判定最多等多久。
///
/// sqlx 的连接池会在内部按自己的 `acquire_timeout`（默认 30 秒）反复重试。那是连接池的耐心，
/// 不是操作员该等的：没有这道上限，一台连不上数据库的机器要半分钟才第一次说出「不可达」，
/// 而在这半分钟里它和一台健康空闲的机器长得一模一样。本机数据库接受连接是毫秒级的，
/// 十秒还没结果就是「这一轮没问到」，如实记下来、退避后再问。
const READINESS_PROBE_BUDGET: Duration = Duration::from_secs(10);

/// 判定一次就绪，带自己的时间上限。没连上就连一次，连上了就地问一次——每轮都新建连接池
/// 没有必要，而且会在数据库刚好抖动时把一个旧池换成新池。
async fn observe_readiness(url: &str, database: &mut Option<Database>) -> RuntimeReadiness {
    let budget = READINESS_PROBE_BUDGET;
    let probe = async {
        match database.as_ref() {
            Some(database) => {
                probe_runtime_readiness(database, &COLLECTION_RUNTIME_REQUIREMENTS).await
            }
            None => {
                let (connected, readiness) =
                    connect_runtime_readiness(url, &COLLECTION_RUNTIME_REQUIREMENTS).await;
                *database = connected;
                readiness
            }
        }
    };
    match tokio::time::timeout(budget, probe).await {
        Ok(readiness) => readiness,
        Err(_) => RuntimeReadiness::unreachable("probe_timeout"),
    }
}

/// 一轮 tick 的四步。每一步各报各的结果——一步失败不该静默掉另外三步。
async fn run_tick_steps(database: &Database) {
    if let Err(error) = linggan_evidence::ensure_discovery_cover_media_work(database).await {
        println!("linggan worker: media acquisition projection failed: {error}");
    }
    match linggan_evidence::run_progressive_archives(database).await {
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
    keyword_details::advance_keyword_details(database).await;
    match linggan_evidence::run_due_patrols(database).await {
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

async fn finish_worker_shutdown(
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
