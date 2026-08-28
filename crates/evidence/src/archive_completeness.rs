//! COLLECTION-001 · 档案完整度。
//!
//! 取自内容工作台 `AuthorArchiveJob` 的分层计数（`totalDiscovered` /
//! `detailSucceeded` / `detailFailed`），而不是一个百分比。**百分比会把「没采到」和
//! 「采了但失败」压成同一个数字**，而这两件事的处置完全不同：前者要派任务，后者要查原因。
//!
//! 归属靠任务规格里的 `authorExternalId`——采集包挂在任务上，任务写明它当初是为谁派的。
//! 用工位或时间去猜归属，会在一台工位同时服务多个目标时立刻错乱。

use linggan_storage_postgres::Database;
use std::collections::HashMap;

/// 一个创作者目标的档案分层。
///
/// 每一层都可能是「没做过」，那与「做了但一条都没拿到」不同，因此用计数而不是布尔。
#[derive(Debug, Default, Clone)]
pub struct ArchiveCompleteness {
    /// 作者档案：拿到过几次公开资料。
    pub author_profile_captures: i64,
    /// 作品清单：已入库的作品条数。
    pub works_listed: i64,
    /// 逐篇详情：已入库的详情条数。
    pub details_captured: i64,
    /// 被隔离的记录条数——它们采到了但没进语料库，必须看得见。
    pub quarantined: i64,
}

impl ArchiveCompleteness {
    /// 一层都没有过。用它来区分「还没开始」与「开始了但结果为空」。
    pub fn is_untouched(&self) -> bool {
        self.author_profile_captures == 0 && self.works_listed == 0 && self.details_captured == 0
    }
}

/// 读一批创作者的档案完整度。
///
/// 一次查完而不是逐个查：列表最多两百行，逐行发查询会让页面打开一次跑两百次数据库。
pub async fn read_archive_completeness(
    database: &Database,
    platform: &str,
) -> Result<HashMap<String, ArchiveCompleteness>, sqlx::Error> {
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT t.task_spec->'target'->>'authorExternalId' AS author_external_id, \
                p.package_kind, \
                CASE WHEN d.disposition = 'quarantined' THEN 'quarantined' ELSE 'accepted' END \
                    AS admitted, \
                count(*) \
         FROM linggan_runtime_record_disposition d \
         JOIN linggan_runtime_capture_package p ON p.package_ref = d.package_ref \
         JOIN linggan_runtime_task t ON t.task_id = p.task_id \
         WHERE p.platform = $1 \
           AND t.task_spec->'target'->>'authorExternalId' IS NOT NULL \
           AND d.disposition <> 'retained_uninterpreted' \
         GROUP BY 1, 2, 3",
    )
    .bind(platform)
    .fetch_all(database.pool())
    .await?;

    let mut totals: HashMap<String, ArchiveCompleteness> = HashMap::new();
    for (author_external_id, package_kind, admitted, count) in rows {
        let entry = totals.entry(author_external_id).or_default();
        if admitted == "quarantined" {
            // 隔离的记录不计入任何一层的完整度——它们没成为可用材料。但要单列出来，
            // 否则「采到了却没进库」会彻底消失在视野外。
            entry.quarantined += count;
            continue;
        }
        match package_kind.as_str() {
            "author_profile" => entry.author_profile_captures += count,
            "profile_discovery" => entry.works_listed += count,
            "content_detail" => entry.details_captured += count,
            _ => {}
        }
    }
    Ok(totals)
}
