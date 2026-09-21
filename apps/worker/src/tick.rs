//! 一轮 tick：开账本、跑四步、每步记一行、收轮。
//!
//! COLLECTION-UPGRADE-001 · T30。此前只有巡查那一步写账本，另外三步只打日志：一步崩了，
//! 账本上只留下「这一轮没派活」，分不清「没有到期的活」与「这一步根本没跑成」。现在一轮
//! tick 有一个 run 号，四步各有一行结果，事件里带着同一个 tick 号——一次失败可以从日志一路
//! 查到库里的决定、工单与目标。
//!
//! **一步失败，其余照跑**：四步互不连坐（媒体投影崩了不该让巡查停摆），各自的失败落在
//! 各自的步骤行上；整轮的结局由 [`tick_outcome`] 统一算——有一步失败，这一轮就是失败。
//!
//! 就绪判定也在这里（`observe_readiness`）：它必须在任何一步之前，且未就绪时不碰账本、
//! 不碰任何步骤（见 `runtime_readiness`）。

use std::time::Duration;

use linggan_evidence::{
    COLLECTION_RUNTIME_REQUIREMENTS, EVENT_TICK, PatrolTickSummary, RuntimeEvent, RuntimeReadiness,
    SERVICE_WORKER, STEP_MEDIA_ACQUISITION, STEP_PATROL, STEP_PROGRESSIVE_DOSSIERS, StepOutcome,
    StepReport, TickLedger, connect_runtime_readiness, ensure_discovery_cover_media_work,
    probe_runtime_readiness, run_due_patrol_step, run_progressive_archives, tick_outcome,
};
use linggan_storage_postgres::Database;

use crate::keyword_details;

/// 两次扫描的间隔。
///
/// 这是**扫描**频率，不是**巡检**频率：多久看一次由每条监控规则自己的周期决定
/// （`collection_monitor_rule_revision.fixed_interval_seconds`，默认 24 小时；一个关键词
/// 可以有几条口径各设各的周期）。扫描频繁一点只是让到期的规则不必等太久才被发现，
/// 代价是一次几乎为空的查询。
pub const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// 一次就绪判定最多等多久。
///
/// sqlx 的连接池会在内部按自己的 `acquire_timeout`（默认 30 秒）反复重试。那是连接池的耐心，
/// 不是操作员该等的：没有这道上限，一台连不上数据库的机器要半分钟才第一次说出「不可达」，
/// 而在这半分钟里它和一台健康空闲的机器长得一模一样。本机数据库接受连接是毫秒级的，
/// 十秒还没结果就是「这一轮没问到」，如实记下来、退避后再问。
const READINESS_PROBE_BUDGET: Duration = Duration::from_secs(10);

/// 判定一次就绪，带自己的时间上限。没连上就连一次，连上了就地问一次——每轮都新建连接池
/// 没有必要，而且会在数据库刚好抖动时把一个旧池换成新池。
pub async fn observe_readiness(url: &str, database: &mut Option<Database>) -> RuntimeReadiness {
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

/// 一轮 tick 的四步。每一轮自己开一个 run，四步各自留一行结果。
pub async fn run_collection_tick(database: &Database) {
    let ledger = match TickLedger::begin(database).await {
        Ok(Some(ledger)) => ledger,
        // 就绪判定刚说过这台机器能接活，账本却不在（表被删了、或迁移还没跑到）：**不跑任何
        // 步骤**。跑了却记不上，账本上会留下一个「什么都没发生」的轮次，比不跑更坏。
        Ok(None) => {
            println!(
                "linggan worker: the scheduler ledger is absent from this database; \
                 no collection step ran this tick"
            );
            RuntimeEvent::new(SERVICE_WORKER, EVENT_TICK)
                .with_outcome("skipped")
                .with_reason("ledger_absent")
                .emit();
            return;
        }
        Err(error) => {
            println!("linggan worker: cannot open the tick ledger: {error}");
            RuntimeEvent::new(SERVICE_WORKER, EVENT_TICK)
                .with_outcome("failed")
                .with_error_class("ledger_unwritable")
                .emit();
            return;
        }
    };
    // 顺序是有意的：先做投影，再推进档案与关键词，最后按到期派活——巡查看到的是前三步
    // 已经落库的世界。四步各自 await 完毕再开下一步，不并发。
    let reports = vec![
        media_step(database, &ledger).await,
        dossier_step(database, &ledger).await,
        keyword_details::advance_keyword_details(database, &ledger).await,
        patrol_step(database, &ledger).await,
    ];
    for report in &reports {
        emit_step(&ledger, report);
    }
    RuntimeEvent::new(SERVICE_WORKER, EVENT_TICK)
        .with_tick(ledger.run_ref())
        .with_outcome(tick_outcome(&reports))
        .emit();
    if let Err(error) = ledger.finish(&reports).await {
        // run 行与心跳停在「没有结局」上：这一轮会在页面上永远显示为还在跑，如实说出来。
        println!("linggan worker: the tick ledger could not be closed: {error}");
    }
}

/// 媒体投影：把历史发现里还缺封面媒资的行补上工位活。
async fn media_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, result) = ledger
        .run_step(
            STEP_MEDIA_ACQUISITION,
            ensure_discovery_cover_media_work(database),
            |projected| StepOutcome::produced(i64::try_from(*projected).unwrap_or(i64::MAX)),
        )
        .await;
    report_failure(&report, result, "media acquisition projection");
    report
}

/// 渐进档案：博主那条深度采集的推进。
async fn dossier_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let (report, result) = ledger
        .run_step(
            STEP_PROGRESSIVE_DOSSIERS,
            run_progressive_archives(database),
            |summary| summary.step_outcome(),
        )
        .await;
    if let Some(Ok(summary)) = &result {
        if !summary.queued.is_empty() || !summary.skipped.is_empty() {
            println!(
                "linggan worker: progressive dossiers queued {}, skipped {}",
                summary.queued.len(),
                summary.skipped.len()
            );
        }
    }
    report_failure(&report, result, "progressive dossier tick");
    report
}

/// 巡查：到期就派。目标级的决定住在 `collection_scheduler_target_decision` 上。
async fn patrol_step(database: &Database, ledger: &TickLedger) -> StepReport {
    let run_ref = ledger.run_ref();
    let (report, result) = ledger
        .run_step(
            STEP_PATROL,
            run_due_patrol_step(database, run_ref),
            PatrolTickSummary::step_outcome,
        )
        .await;
    if let Some(Ok(summary)) = &result {
        if !summary.queued.is_empty() || !summary.skipped.is_empty() {
            // 只在真的发生了什么时说话。一个每分钟打印「本轮 0 个」的 tick 会让日志变成
            // 噪音，等真有事发生时反而看不见。
            println!(
                "linggan worker: queued {}, skipped {}",
                summary.queued.len(),
                summary.skipped.len()
            );
            // 跳过的理由逐条打印：一个只报「本轮派了 3 个」的调度器，在没派的时候没人
            // 说得清为什么。
            for (target_ref, reason) in &summary.skipped {
                println!("linggan worker: skipped {target_ref}: {reason}");
            }
        }
    }
    report_failure(&report, result, "patrol tick");
    report
}

/// 步骤真失败时打印原始报文；「没轮到」（自己的表不在）不是失败，不在日志里喊。
///
/// 报文里可能带着数据库自己给的细节，它有诊断价值、也只写进日志；账本与事件里落的是受限
/// 分类码（见 `step_report::StepFailure`）。
fn report_failure<T, E: std::fmt::Display>(
    report: &StepReport,
    result: Option<Result<T, E>>,
    what: &str,
) {
    if report.outcome.error_class().is_none() {
        return;
    }
    if let Some(Err(error)) = result {
        println!("linggan worker: {what} failed: {error}");
    }
}

/// 一步一行事件。字段是闭集（`runtime_event::EVENT_FIELD_WHITELIST`）：没有能塞任意键的入口。
///
/// 值不在这里拼：同一份报告还要写进账本，两处的映射只应有一份（[`StepReport::event`]）。
/// 这里只负责把那一行发出去。
///
/// **号也不经手**：这一轮的号当场问账本要。上一个版本把它取出来放在一个局部变量里，于是
/// 这个变量就有了两个去处（事件与收轮），而两处各写各的号时它们会各自都对、合起来对不上——
/// 那正是这条链最要命的错法。手里没有第二个号，就没有传错的机会。
fn emit_step(ledger: &TickLedger, report: &StepReport) {
    report.event(ledger.run_ref()).emit();
}
