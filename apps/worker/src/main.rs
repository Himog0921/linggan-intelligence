//! Linggan Intelligence 的巡检 tick。
//!
//! 这是全系统第一个会自己动的东西。在它之前每一步都要人去戳一下。
//!
//! **tick 只做轻量的状态推进与入队**（产品规则 §6.3，取自内容工作台的教训：那边的调度
//! tick 把媒体下载和转录塞进同一个 120 秒事务里，任何超时的 tick 都会丢锁并以异常收场）。
//! 这里不下载、不转录、不访问任何平台——它只判断「谁到期了」，然后走一遍与人点按钮
//! 完全相同的授权链。
//!
//! **自动化不是豁免权**：准入若说资源不够、风险暂停生效或额度触顶，tick 也只能记下理由
//! 然后等下一轮。

use std::time::Duration;

/// 两次扫描的间隔。
///
/// 这是**扫描**频率，不是**巡检**频率：每个目标多久看一次由它自己的
/// `patrol_interval_seconds` 决定（默认 24 小时）。扫描频繁一点只是让到期的目标不必等
/// 太久才被发现，代价是一次几乎为空的查询。
const TICK_INTERVAL: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        // 没有数据库就什么都不做，而不是拿一个默认连接串去猜。
        println!("linggan worker: LINGGAN_LOCAL_DATABASE_URL is not set; no jobs are enabled");
        return;
    };
    let database = match linggan_storage_postgres::Database::connect(&url).await {
        Ok(database) => database,
        Err(error) => {
            println!("linggan worker: cannot reach the local database: {error}");
            return;
        }
    };
    println!(
        "linggan worker: patrol tick every {}s",
        TICK_INTERVAL.as_secs()
    );

    let mut ticker = tokio::time::interval(TICK_INTERVAL);
    loop {
        ticker.tick().await;
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
}
