//! 一次性的存量重排工具：把旧版本处理器做过的作业，按当前版本重新排一遍。
//!
//! **它不是常驻服务**，也不属于 launchd 托管的那三个进程；跑一次就退出。常驻的
//! `linggan-media-worker` 不会调用它，只为它提供了「排出来的作业真的会被领走」这条路：
//! 重排只往库里插作业，可认领的 work 行由 worker 每 tick 的 `ensure_media_processing_work`
//! 投影出来，认领顺序也仍由 `claim_media_processing_work` 决定。
//!
//! 为什么需要它、抬版本号的规矩，见 `crates/evidence/src/material_processing.rs`。
//!
//! ```text
//! linggan-media-requeue --kinds image_ocr,video_frame_ocr --dry-run
//! linggan-media-requeue --kinds asr
//! ```

use linggan_evidence::{REGISTERED_PROCESSOR_KINDS, processor_version_for_kind};
use std::process::ExitCode;

const USAGE: &str = "用法: linggan-media-requeue --kinds <处理器名,逗号分隔> [--dry-run]";

#[tokio::main]
async fn main() -> ExitCode {
    let mut kinds: Vec<String> = Vec::new();
    let mut dry_run = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" => dry_run = true,
            "--kinds" => {
                let Some(value) = arguments.next() else {
                    eprintln!("--kinds 后面没给处理器名。{USAGE}");
                    return ExitCode::from(2);
                };
                kinds.extend(value.split(',').filter(|kind| !kind.is_empty()).map(str::to_owned));
            }
            other => {
                eprintln!("未知参数：{other}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if kinds.is_empty() {
        eprintln!("{USAGE}\n登记的处理器：{}", REGISTERED_PROCESSOR_KINDS.join(","));
        return ExitCode::from(2);
    }
    // 名字打错必须当场报错。安静地排 0 条会被读成「这一类没有旧作业」——那是本次重排要
    // 消灭的失败形态（196 条 asr 里 195 条静默失败就是这么藏了三天）。
    for kind in &kinds {
        if processor_version_for_kind(kind).is_none() {
            eprintln!(
                "没有登记的处理器：{kind}。登记的处理器：{}",
                REGISTERED_PROCESSOR_KINDS.join(",")
            );
            return ExitCode::from(2);
        }
    }
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        eprintln!("linggan media requeue: LINGGAN_LOCAL_DATABASE_URL is not set");
        return ExitCode::FAILURE;
    };
    let database = match linggan_storage_postgres::Database::connect(&url).await {
        Ok(database) => database,
        Err(error) => {
            eprintln!("linggan media requeue: cannot reach the local database: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "linggan media requeue: {} [{}]",
        if dry_run { "预演（做完即撤回）" } else { "写入" },
        kinds.join(",")
    );
    // **逐类调用、逐类打印。** 函数内部本来就是「一类一个事务」，一次传多个名字并不会让它们变成一个
    // 事务；但如果第二类在数据库层失败，第一类**已经提交**却一个字都还没打印，操作员就看不到「刚刚
    // 排进去多少」。重跑是幂等的，所以这不是数据风险，而是「刚发生了什么」不可见——对着生产跑的工具
    // 上不值得省这一行。名字仍然在上面一次性校验完，误输不会走到这里。
    for kind in &kinds {
        match linggan_evidence::requeue_outdated_processor_jobs(
            &database,
            std::slice::from_ref(kind),
            dry_run,
        )
        .await
        {
            Ok(summaries) => {
                for summary in &summaries {
                    println!(
                        "  {} → {}：{} {} 条",
                        summary.processor_kind,
                        summary.target_version,
                        if dry_run { "会排" } else { "已排" },
                        summary.enqueued_jobs
                    );
                }
            }
            Err(error) => {
                eprintln!("linggan media requeue: 失败：{error}");
                // 上面那句是错误链的**最外层**（`Internal(#[source] sqlx::Error)` 的 Display 只印
                // 「事务没有提交」）。真实原因——唯一约束冲突、被触发器拒绝、连接断开——在 `Debug`
                // 里。这是对着生产跑一次就退的工具，失败必须说得出为什么，否则操作员下一步只能猜。
                eprintln!("  原因：{error:?}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
