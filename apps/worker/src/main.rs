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
//! **但「干不了活」与「没有活」必须分得开**（COLLECTION-UPGRADE-001 · T29/T30）。数据库连不上、
//! 迁移台账读不到、迁移没跑、schema 不兼容时，这个进程以前会照常启动、照常每 60 秒走一遍、
//! 一行「本轮 0 个」都不打——和一台空闲机器长得一模一样。现在每轮先判就绪（`runtime_readiness`）：
//! 未就绪就不碰任何采集步骤，按退避重试，状态变化时才说话。连不上数据库绝不以 SUCCESS 退出
//! （那是把故障说成正常），也绝不空转热重启：进程留在原地等它回来。这道闸只管**采集面**——
//! 模型评论循环是另一条通道，有自己的表，只要有连接就照常跑（见 tick 循环里的注释）。
//!
//! 一轮 tick 的四步与每一步的结果住在 `tick.rs`；关停住在 `shutdown.rs`。这个文件只负责
//! 把它们按顺序装起来。

use std::process::ExitCode;

use linggan_evidence::{
    EVENT_READINESS, EVENT_STARTUP, READINESS_RETRY_START, RuntimeEvent, SERVICE_WORKER,
    next_readiness_retry, record_readiness, record_scheduler_started,
};
use linggan_intelligence::model_worker_drain::ModelWorkerDrain;
use linggan_storage_postgres::Database;

mod keyword_details;
mod shutdown;
mod tick;

use shutdown::{
    ModelWorkerTask, finish_worker_shutdown, wait_for_worker_shutdown, write_drain_ack,
};
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
    // 事件说的是**哪个部署的哪个进程**在跑：revision 由部署脚本写、进程只读；读不到就是
    // `unknown`，不拿别的东西冒充（见 `runtime_event`）。
    RuntimeEvent::new(SERVICE_WORKER, EVENT_STARTUP).emit();

    let mut ticker =
        tokio::time::interval_at(tokio::time::Instant::now() + TICK_INTERVAL, TICK_INTERVAL);
    // 卡住的那一轮不该在恢复之后补跑一串：延后到下一个周期就够了。
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut database: Option<Database> = None;
    let mut model_worker: Option<ModelWorkerTask> = None;
    // 只在状态**变化**时说话：持续故障每分钟重复一行会把日志变成噪音，而噪音里看不见变化。
    let mut announced: Option<&'static str> = None;
    let mut tick_announced = false;
    let mut retry_after = READINESS_RETRY_START;

    // Cancel only the collection loop. In-flight model receipts drain independently below.
    // An interrupted tick retains its unfinished ledger row; cancellation is not completion.
    tokio::select! {
        biased;
        _ = &mut shutdown => {},
        _ = async {
            loop {
                let readiness = observe_readiness(&url, &mut database).await;
                // 模型评论循环**不**由采集面的就绪判据决定：它有自己的表，采集面的缺口不该把另一条
                // 通道连坐停摆（要求清单是每消费者各主张的下限，不是一个总闸）。它的判据是「有没有
                // 连接」——连不上时把它跑起来，只会制造一屋子错误日志。
                if model_worker.is_none() {
                    if let Some(connection) = database.as_ref() {
                        let worker_instance_ref = uuid::Uuid::new_v4();
                        if let Err(error) =
                            record_scheduler_started(connection, worker_instance_ref).await
                        {
                            println!("linggan worker: cannot record scheduler identity: {error}");
                        }
                        model_worker = Some(tokio::spawn(
                            linggan_intelligence::model_runner::run_model_worker_with_drain(
                                connection.clone(),
                                drain.clone(),
                            ),
                        ));
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

/// 一次就绪判定写一行事件。人读的散文行与机器读的事件都在状态**变化**时出现。
fn emit_readiness(readiness: &linggan_evidence::RuntimeReadiness) {
    let mut event =
        RuntimeEvent::new(SERVICE_WORKER, EVENT_READINESS).with_outcome(readiness.state.code());
    if let Some(detail) = readiness.detail.as_deref() {
        event = event.with_reason(detail);
    }
    event.emit();
}
