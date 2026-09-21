//! 关键词建档第二段的 tick 步骤。
//!
//! 单独一个文件：入口文件有 200 行硬上限（治理脚本守着），把每一条 tick 的日志处理
//! 都堆在 `main` 里，那个文件会慢慢变成所有调度分支的集合。

use linggan_evidence::{STEP_KEYWORD_DETAILS, StepReport, TickLedger};
use linggan_storage_postgres::Database;

/// 关键词建档的第二段：把链接换成详情。
///
/// 博主那条路由 `run_progressive_archives` 推进，关键词此前只有手点一个入口：第一段跑完
/// 之后界面上多出一个按钮，人不点就永远停在一堆链接上——而这个词看上去已经建好档了。
pub(crate) async fn advance_keyword_details(
    database: &Database,
    ledger: &TickLedger,
) -> StepReport {
    let (report, result) = ledger
        .run_step(
            STEP_KEYWORD_DETAILS,
            linggan_evidence::run_keyword_archive_details(
                database,
                "关键词建档详情补采（调度推进）",
            ),
            linggan_evidence::KeywordDetailTickSummary::step_outcome,
        )
        .await;
    match result {
        Some(Ok(summary)) => {
            for (target_ref, works) in &summary.queued {
                println!("linggan worker: keyword details queued {target_ref}: {works} works");
            }
            // 没推进的理由逐条打印：一个只报「本轮排了 3 个」的调度器，在没排的时候
            // 没人说得清为什么。
            for (target_ref, reason) in &summary.skipped {
                println!("linggan worker: keyword details skipped {target_ref}: {reason}");
            }
        }
        // 「自己的表不在」是没轮到，不是失败：账本与事件里它落在 `skipped` 上，日志不喊。
        Some(Err(error)) if report.outcome.error_class().is_some() => {
            println!("linggan worker: keyword detail tick failed: {error}");
        }
        _ => {}
    }
    report
}
