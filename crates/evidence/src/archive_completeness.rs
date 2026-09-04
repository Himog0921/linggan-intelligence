//! COLLECTION-001 · 档案完整度。
//!
//! 取自内容工作台 `AuthorArchiveJob` 的分层计数（`totalDiscovered` /
//! `detailSucceeded` / `detailFailed`），而不是一个百分比。**百分比会把「没采到」和
//! 「采了但失败」压成同一个数字**，而这两件事的处置完全不同：前者要派任务，后者要查原因。
//!
//! 归属靠 `Package → Lease Task → Lease → Work Order → Observation Target`
//! 这条确切链路。详情任务只携带 `contentExternalId`，若靠任务规格里的
//! `authorExternalId` 反查，渐进补齐的详情会全部丢失归属。

use linggan_storage_postgres::Database;
use std::collections::HashMap;

/// 一个创作者目标的档案分层。
///
/// 每一层都可能是「没做过」，那与「做了但一条都没拿到」不同，因此用计数而不是布尔。
#[derive(Debug, Default, Clone)]
pub struct ArchiveCompleteness {
    /// 是否存在仍有效且尚未释放的建档/完善租约。
    ///
    /// 这是「查看进度」的 durable 依据；不能用目标生命周期或一次按钮点击猜测在途状态。
    pub work_in_progress: bool,
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
    let rows: Vec<(String, bool, i64, i64, i64, i64)> = sqlx::query_as(
        "WITH target_records AS ( \
             SELECT target.identity_key AS author_external_id, \
                    p.package_ref, p.package_kind, d.record_ordinal, d.disposition \
             FROM linggan_runtime_record_disposition d \
             JOIN linggan_runtime_capture_package p ON p.package_ref=d.package_ref \
             JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=p.task_id \
             JOIN collection_work_order_lease lease USING (lease_ref) \
             JOIN collection_work_order work_order USING (work_order_ref) \
             JOIN collection_observation_target target ON target.target_ref=work_order.target_ref \
             WHERE p.platform=$1 \
               AND target.target_kind='creator' \
               AND d.disposition <> 'retained_uninterpreted' \
         ), record_totals AS ( \
             SELECT records.author_external_id, \
                    count(DISTINCT records.package_ref) FILTER ( \
                        WHERE records.package_kind='author_profile' \
                          AND records.disposition <> 'quarantined' \
                    ) AS author_profile_captures, \
                    count(DISTINCT finding.content_public_ref) FILTER ( \
                        WHERE records.package_kind='profile_discovery' \
                          AND records.disposition <> 'quarantined' \
                    ) AS works_listed, \
                    count(DISTINCT detail.content_public_ref) FILTER ( \
                        WHERE records.package_kind='content_detail' \
                          AND records.disposition <> 'quarantined' \
                    ) AS details_captured, \
                    count(*) FILTER (WHERE records.disposition='quarantined') AS quarantined \
             FROM target_records records \
             LEFT JOIN linggan_material_discovery_finding finding \
               ON finding.package_ref=records.package_ref \
              AND finding.record_ordinal=records.record_ordinal \
             LEFT JOIN linggan_material_content_detail detail \
               ON detail.package_ref=records.package_ref \
              AND detail.record_ordinal=records.record_ordinal \
             GROUP BY records.author_external_id \
         ), live_archive AS ( \
             SELECT DISTINCT target.identity_key AS author_external_id \
             FROM collection_work_order_lease lease \
             JOIN collection_work_order work_order USING (work_order_ref) \
             JOIN collection_observation_target target USING (target_ref) \
             WHERE work_order.lane='deep_archive' \
               AND target.platform=$1 \
               AND target.target_kind='creator' \
               AND lease.released_at IS NULL \
               AND lease.expires_at > scope_001_now() \
         ), target_keys AS ( \
             SELECT author_external_id FROM record_totals \
             UNION \
             SELECT author_external_id FROM live_archive \
         ) \
         SELECT keys.author_external_id, \
                live.author_external_id IS NOT NULL AS work_in_progress, \
                coalesce(totals.author_profile_captures, 0), \
                coalesce(totals.works_listed, 0), \
                coalesce(totals.details_captured, 0), \
                coalesce(totals.quarantined, 0) \
         FROM target_keys keys \
         LEFT JOIN record_totals totals USING (author_external_id) \
         LEFT JOIN live_archive live USING (author_external_id)",
    )
    .bind(platform)
    .fetch_all(database.pool())
    .await?;

    let mut totals: HashMap<String, ArchiveCompleteness> = HashMap::new();
    for (author_external_id, work_in_progress, profiles, works, details, quarantined) in rows {
        totals.insert(
            author_external_id,
            ArchiveCompleteness {
                work_in_progress,
                author_profile_captures: profiles,
                works_listed: works,
                details_captured: details,
                quarantined,
            },
        );
    }
    Ok(totals)
}
