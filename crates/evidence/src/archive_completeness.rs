//! COLLECTION-001 · 档案完整度。
//!
//! 取自内容工作台 `AuthorArchiveJob` 的分层计数（`totalDiscovered` /
//! `detailSucceeded` / `detailFailed`），而不是一个百分比。**百分比会把「没采到」和
//! 「采了但失败」压成同一个数字**，而这两件事的处置完全不同：前者要派任务，后者要查原因。
//!
//! 归属靠 `Package → Lease Task → Lease → Work Order → Observation Target`
//! 这条确切链路。详情任务只携带 `contentExternalId`，若靠任务规格里的
//! `authorExternalId` 反查，渐进补齐的详情会全部丢失归属。

use crate::archive_ledger::directory_works_sql;
use crate::directory_boundary::{directory_proven_sql, historical_directory_scan_qualified_sql, surface_scan_complete_sql};
use crate::qualified_detail::qualified_detail_missing_sql;
use linggan_storage_postgres::Database;
use std::collections::HashMap;
use uuid::Uuid;

/// 当前主页扫描是否证明了基线边界。已接纳的单篇作品可以先登记和补采，
/// 但它们的数量不能代替 `surface_ended` 或完整配额来证明主页目录已经建成。
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum ArchiveDirectoryBaseline {
    #[default]
    NotStarted,
    Building,
    Ready,
    /// 早于 200 篇目录合同的已接纳目录仍是当前在库事实。它可以展示和接收巡查增量，
    /// 但不能被叙述成已经证明了创作者主页的完整边界。
    HistoricalDirectory,
    RebuildRequired,
}

/// 一个创作者目标的档案分层。
///
/// 每一层都可能是「没做过」，那与「做了但一条都没拿到」不同，因此用计数而不是布尔。
#[derive(Debug, Default, Clone)]
pub struct ArchiveCompleteness {
    /// 是否已经形成过创作者主页根建档 Work Order；逐篇材料补采不算。
    pub started: bool,
    /// 是否有执行 Attempt 已经从这张目标自己的根建档 Lease Task 开始。
    pub attempted: bool,
    /// 是否存在执行中的根建档租约，或尚未领取、等待重试的根建档任务。
    ///
    /// 这是「查看建档状态」的 durable 依据；不能用目标生命周期或一次按钮点击猜测在途状态。
    pub work_in_progress: bool,
    /// 作者档案：拿到过几次公开资料。
    pub author_profile_captures: i64,
    /// 作品清单：已入库的作品条数。
    pub works_listed: i64,
    /// 逐篇详情：已入库的详情条数。
    pub details_captured: i64,
    /// 被隔离的记录条数——它们采到了但没进语料库，必须看得见。
    pub quarantined: i64,
    /// 当前根建档中因连续页面读取失败而停止自动重试的作品详情数。它不是页面
    /// 不存在，也没有形成 Attempt、Package、Receipt 或 Evidence。
    pub blocked_details: i64,
    /// 人已确认在平台上不存在的作品数。它们仍留在作品目录里，只是不再计入待补齐——
    /// 抹掉分母会让「档案完成」建立在一个被修饰过的数字上。
    pub retired_works: i64,
    /// 既没有详情、也没有被确认失效的作品数：**真正还要去采的那些**。
    ///
    /// 它是一个独立的投影列，不是 `works_listed - details_captured`。此前七处展示层各自
    /// 做那个减法，于是「已确认失效」加进来之后，只有其中一处知道要再减一次——木可可
    /// 确认了 3 篇已删除，界面照旧催他去补那 3 篇。三个互斥的数相加等于作品总数，
    /// 展示层就没有可减的东西了。
    pub pending_details: i64,
    /// 当前目录根的边界是否已经由受接纳 Package 证明。
    pub directory_baseline: ArchiveDirectoryBaseline,
}

impl ArchiveCompleteness {
    /// 一层都没有过。用它来区分「还没开始」与「开始了但结果为空」。
    pub fn is_untouched(&self) -> bool {
        !self.started
            && !self.attempted
            && !self.work_in_progress
            && self.author_profile_captures == 0
            && self.works_listed == 0
            && self.details_captured == 0
            && self.quarantined == 0
            && self.blocked_details == 0
            && self.directory_baseline == ArchiveDirectoryBaseline::NotStarted
    }

    pub fn has_displayable_directory(&self) -> bool {
        matches!(
            self.directory_baseline,
            ArchiveDirectoryBaseline::Ready | ArchiveDirectoryBaseline::HistoricalDirectory
        )
    }

    pub fn requires_directory_rebuild(&self) -> bool {
        self.directory_baseline == ArchiveDirectoryBaseline::RebuildRequired
    }

    pub fn has_actionable_problems(&self) -> bool {
        self.quarantined > 0 || self.blocked_details > 0
    }
}

/// 读一批创作者的档案完整度。
///
/// 一次查完而不是逐个查：列表最多两百行，逐行发查询会让页面打开一次跑两百次数据库。
pub async fn read_archive_completeness(
    database: &Database,
    platform: &str,
) -> Result<HashMap<String, ArchiveCompleteness>, sqlx::Error> {
    let rows: Vec<(String, bool, bool, bool, i64, i64, i64, i64, i64, i64, i64, bool)> = sqlx::query_as(
        concat!(
            "WITH ranked_roots AS ( \
             SELECT target.target_ref,target.identity_key AS author_external_id, \
                    work_order.work_order_ref AS root_work_order_ref, \
                    row_number() OVER (PARTITION BY target.target_ref \
                                       ORDER BY work_order.created_at DESC,work_order.work_order_ref DESC) AS root_rank \
             FROM collection_observation_target target \
             JOIN collection_work_order work_order USING(target_ref) \
             WHERE target.platform=$1 AND target.target_kind='creator' \
               AND work_order.lane='deep_archive' \
               AND work_order.stop_conditions #>> '{progressiveArchive,version}'='1' \
               AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
         ), active_roots AS ( \
             SELECT target_ref,author_external_id,root_work_order_ref \
             FROM ranked_roots WHERE root_rank=1 \
         ), scoped_orders AS ( \
             SELECT roots.author_external_id,roots.root_work_order_ref,work_order.work_order_ref \
             FROM active_roots roots \
             JOIN collection_work_order work_order ON work_order.target_ref=roots.target_ref \
             WHERE work_order.lane='deep_archive' \
               AND (work_order.work_order_ref=roots.root_work_order_ref \
                    OR work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=roots.root_work_order_ref::text) \
         ), ranked_directory_packages AS ( \
             SELECT roots.author_external_id,package.package_ref, \
                    row_number() OVER (PARTITION BY roots.root_work_order_ref \
                                       ORDER BY package.accepted_at DESC,package.package_ref DESC) AS package_rank \
             FROM active_roots roots \
             JOIN collection_work_order root_order ON root_order.work_order_ref=roots.root_work_order_ref \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE package.platform=$1 AND package.platform=task.platform \
               AND package.package_kind='profile_discovery' \
               AND task.task_spec->'capabilitiesRequested' ? package.package_kind \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' AND receipt.material_admission='ACCEPTED' \
               AND layer->>'capability'='profile_discovery' \
               AND ",
            directory_proven_sql!(),
            " \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref AND disposition.disposition='quarantined') \
               AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                    WHERE disposition.package_ref=package.package_ref \
                      AND disposition.disposition='accepted_for_library_discovery')=COALESCE((layer->>'acquired')::integer,-1) \
         ), directory_packages AS ( \
             SELECT author_external_id,package_ref FROM ranked_directory_packages WHERE package_rank=1 \
         ), root_profile_records AS ( \
             SELECT scoped.author_external_id,p.package_ref,p.package_kind,d.record_ordinal,d.disposition \
             FROM scoped_orders scoped \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_capture_package p ON p.task_id=lease_task.task_id \
             JOIN linggan_runtime_record_disposition d ON d.package_ref=p.package_ref \
             WHERE p.platform=$1 AND p.package_kind='author_profile' \
               AND d.disposition <> 'retained_uninterpreted' AND d.disposition <> 'quarantined' \
         ), root_quarantined_records AS ( \
             SELECT scoped.author_external_id,p.package_ref,p.package_kind,d.record_ordinal,d.disposition \
             FROM scoped_orders scoped \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_capture_package p ON p.task_id=lease_task.task_id \
             JOIN linggan_runtime_record_disposition d ON d.package_ref=p.package_ref \
             WHERE p.platform=$1 AND d.disposition='quarantined' \
         ), directory_records AS ( \
             SELECT packages.author_external_id,p.package_ref,p.package_kind,d.record_ordinal,d.disposition \
             FROM directory_packages packages \
             JOIN linggan_runtime_capture_package p ON p.package_ref=packages.package_ref \
             JOIN linggan_runtime_record_disposition d ON d.package_ref=p.package_ref \
             WHERE d.disposition <> 'retained_uninterpreted' \
         ), qualified_patrol_packages AS ( \
             SELECT DISTINCT target.identity_key AS author_external_id,package.package_ref \
             FROM collection_observation_target target \
             JOIN collection_work_order work_order USING(target_ref) \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE target.platform=$1 AND target.target_kind='creator' AND work_order.lane='patrol' \
               AND package.package_kind='profile_discovery' AND package.platform=task.platform \
               AND task.task_spec->'capabilitiesRequested' ? package.package_kind \
               -- 同 work_order_ledger：归属由 task_id 保证，不靠 target 逐键包含。
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' AND receipt.material_admission='ACCEPTED' \
               AND layer->>'capability'='profile_discovery' \
               AND ",
            surface_scan_complete_sql!(),
            " \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref AND disposition.disposition='quarantined') \
               AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition \
                    WHERE disposition.package_ref=package.package_ref \
                      AND disposition.disposition='accepted_for_library_discovery')=COALESCE((layer->>'acquired')::integer,-1) \
         ), patrol_records AS ( \
             SELECT patrol.author_external_id,p.package_ref,p.package_kind,d.record_ordinal,d.disposition \
             FROM qualified_patrol_packages patrol \
             JOIN linggan_runtime_capture_package p ON p.package_ref=patrol.package_ref \
             JOIN linggan_runtime_record_disposition d ON d.package_ref=p.package_ref \
             WHERE d.disposition <> 'retained_uninterpreted' \
         ), directory_work_refs AS ( \
             SELECT DISTINCT records.author_external_id,finding.content_public_ref \
             FROM ( \
                 SELECT author_external_id,package_ref,record_ordinal,disposition FROM directory_records \
                 UNION ALL \
                 SELECT author_external_id,package_ref,record_ordinal,disposition FROM patrol_records \
             ) records \
             JOIN linggan_material_discovery_finding finding \
               ON finding.package_ref=records.package_ref AND finding.record_ordinal=records.record_ordinal \
             WHERE records.disposition <> 'quarantined' \
         ), detail_records AS ( \
             SELECT scoped.author_external_id,p.package_ref,p.package_kind,d.record_ordinal,d.disposition \
             FROM scoped_orders scoped \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_capture_package p ON p.task_id=lease_task.task_id \
             JOIN linggan_runtime_record_disposition d ON d.package_ref=p.package_ref \
             JOIN linggan_material_content_detail detail \
               ON detail.package_ref=d.package_ref AND detail.record_ordinal=d.record_ordinal \
             JOIN directory_work_refs works \
               ON works.author_external_id=scoped.author_external_id \
              AND works.content_public_ref=detail.content_public_ref \
             WHERE p.platform=$1 AND p.package_kind='content_detail' \
               AND d.disposition <> 'retained_uninterpreted' \
         ), target_records AS ( \
             SELECT author_external_id,package_ref,package_kind,record_ordinal,disposition FROM root_profile_records \
             UNION ALL \
             SELECT author_external_id,package_ref,package_kind,record_ordinal,disposition FROM root_quarantined_records \
             UNION ALL \
             SELECT author_external_id,package_ref,package_kind,record_ordinal,disposition FROM directory_records \
             UNION ALL \
             SELECT author_external_id,package_ref,package_kind,record_ordinal,disposition FROM patrol_records \
             UNION ALL \
             SELECT author_external_id,package_ref,package_kind,record_ordinal,disposition FROM detail_records \
         ), record_totals AS ( \
             SELECT records.author_external_id, \
                    count(DISTINCT records.package_ref) FILTER ( \
                        WHERE records.package_kind='author_profile' AND records.disposition <> 'quarantined' \
                    ) AS author_profile_captures, \
                    count(DISTINCT finding.content_public_ref) FILTER ( \
                        WHERE records.package_kind='profile_discovery' AND records.disposition <> 'quarantined' \
                    ) AS works_listed, \
                    count(DISTINCT detail.content_public_ref) FILTER ( \
                        WHERE records.package_kind='content_detail' AND records.disposition <> 'quarantined' \
                    ) AS details_captured, \
                    count(*) FILTER (WHERE records.disposition='quarantined') AS quarantined \
             FROM target_records records \
             LEFT JOIN linggan_material_discovery_finding finding \
               ON finding.package_ref=records.package_ref AND finding.record_ordinal=records.record_ordinal \
             LEFT JOIN linggan_material_content_detail detail \
               ON detail.package_ref=records.package_ref AND detail.record_ordinal=records.record_ordinal \
             GROUP BY records.author_external_id \
         ), archive_progress AS ( \
             SELECT roots.author_external_id, true AS started, \
                    bool_or(attempt.attempt_id IS NOT NULL) AS attempted, \
                    bool_or(work_order.queue_state='queued' \
                           OR (lease.released_at IS NULL AND lease.expires_at>scope_001_now())) AS work_in_progress \
             FROM active_roots roots \
             JOIN scoped_orders scoped ON scoped.root_work_order_ref=roots.root_work_order_ref \
             JOIN collection_work_order work_order USING(work_order_ref) \
             LEFT JOIN collection_work_order_lease lease USING(work_order_ref) \
             LEFT JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             LEFT JOIN linggan_runtime_attempt attempt ON attempt.task_id=lease_task.task_id \
             GROUP BY roots.author_external_id \
         ), blocked_details AS ( \
             SELECT scoped.author_external_id, \
                    count(DISTINCT runtime.task_spec #>> '{target,contentExternalId}') AS blocked_details \
             FROM scoped_orders scoped \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task runtime ON runtime.task_id=lease_task.task_id \
             WHERE lease_task.execution_state='blocked' \
               AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
               AND NULLIF(runtime.task_spec #>> '{target,contentExternalId}','') IS NOT NULL \
               -- 人已经确认这篇在平台上没了，它就不再是待处理项。受阻记录本身保留——
               -- 那次读取失败确实发生过——但它不该继续把整个档案标成「有问题」。
               AND NOT EXISTS ( \
                 SELECT 1 FROM collection_material_retirement retired \
                 JOIN linggan_material_content content \
                   ON content.public_ref=retired.content_public_ref \
                 JOIN collection_observation_target target \
                   ON target.target_ref=retired.target_ref \
                 WHERE target.identity_key=scoped.author_external_id \
                   AND content.content_external_id \
                       =runtime.task_spec #>> '{target,contentExternalId}') \
             GROUP BY scoped.author_external_id \
         ), ",
            directory_works_sql!("target.platform=$1 AND target.target_kind='creator'", "true"),
            ", ledger_totals AS ( \
             SELECT target.identity_key AS author_external_id, \
                    count(*) AS works_listed, \
                    count(*) FILTER (WHERE ledger.has_detail) AS details_captured, \
                    count(*) FILTER (WHERE ledger.is_retired AND NOT ledger.has_detail) \
                        AS retired_works, \
                    count(*) FILTER (WHERE NOT ledger.has_detail AND NOT ledger.is_retired) \
                        AS pending_details \
             FROM directory_work ledger \
             JOIN collection_observation_target target \
               ON target.target_ref=ledger.target_ref \
             GROUP BY target.identity_key \
         ), patrol_quarantined AS ( \
             SELECT target.identity_key AS author_external_id,count(*) AS quarantined \
             FROM ledger_registered_packages packages \
             JOIN collection_observation_target target ON target.target_ref=packages.target_ref \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=packages.package_ref \
             WHERE packages.lane='patrol' AND disposition.disposition='quarantined' \
             GROUP BY target.identity_key \
         ) \
         SELECT target.identity_key AS author_external_id,coalesce(progress.started,false),coalesce(progress.attempted,false), \
                coalesce(progress.work_in_progress,false),coalesce(totals.author_profile_captures,0), \
                coalesce(ledger.works_listed,0),coalesce(ledger.details_captured,0), \
                coalesce(ledger.retired_works,0),coalesce(ledger.pending_details,0), \
                coalesce(totals.quarantined,0)+coalesce(patrol_quarantined.quarantined,0), \
                coalesce(blocked_details.blocked_details,0), \
                directories.package_ref IS NOT NULL \
         FROM collection_observation_target target \
         LEFT JOIN active_roots roots ON roots.target_ref=target.target_ref \
         LEFT JOIN record_totals totals ON totals.author_external_id=target.identity_key \
         LEFT JOIN archive_progress progress ON progress.author_external_id=target.identity_key \
         LEFT JOIN blocked_details ON blocked_details.author_external_id=target.identity_key \
         LEFT JOIN ledger_totals ledger ON ledger.author_external_id=target.identity_key \
         LEFT JOIN patrol_quarantined ON patrol_quarantined.author_external_id=target.identity_key \
         LEFT JOIN directory_packages directories ON directories.author_external_id=target.identity_key \
         WHERE target.platform=$1 AND target.target_kind='creator'",
        ),
    )
    .bind(platform)
    .fetch_all(database.pool())
    .await?;

    let mut totals: HashMap<String, ArchiveCompleteness> = HashMap::new();
    for (
        author_external_id,
        started,
        attempted,
        work_in_progress,
        profiles,
        works,
        details,
        retired_works,
        pending_details,
        quarantined,
        blocked_details,
        directory_ready,
    ) in rows
    {
        totals.insert(
            author_external_id,
            ArchiveCompleteness {
                started,
                attempted,
                work_in_progress,
                author_profile_captures: profiles,
                works_listed: works,
                details_captured: details,
                retired_works,
                pending_details,
                quarantined,
                blocked_details,
                directory_baseline: if directory_ready {
                    ArchiveDirectoryBaseline::Ready
                } else if work_in_progress {
                    ArchiveDirectoryBaseline::Building
                } else if !started {
                    ArchiveDirectoryBaseline::NotStarted
                } else {
                    ArchiveDirectoryBaseline::RebuildRequired
                },
            },
        );
    }

    // The 200-link contract applies to a newly-established standard directory.  It does not
    // erase an older target-scoped directory that has already been accepted.  Keep those
    // historical works visible when their details are complete; the UI can then say exactly
    // what is known instead of converting a real 41/41 archive into an empty “rebuild” row.
    let historical_rows: Vec<(String, i64, i64, bool, bool)> = sqlx::query_as(concat!(
        "SELECT target.identity_key, \
                count(DISTINCT finding.content_public_ref) FILTER ( \
                    WHERE package.package_kind='profile_discovery' \
                      AND disposition.disposition <> 'quarantined' \
                ) AS works_listed, \
                count(DISTINCT detail.content_public_ref) FILTER ( \
                    WHERE package.package_kind='content_detail' \
                      AND disposition.disposition <> 'quarantined' \
                ) AS details_captured, \
                coalesce(bool_or(", historical_directory_scan_qualified_sql!(), " \
                  AND (task.task_spec->>'maximumQuota')::integer BETWEEN 1 AND 199 \
                  AND work_order.work_order_ref=( \
                    SELECT current_root.work_order_ref FROM collection_work_order current_root \
                    WHERE current_root.target_ref=target.target_ref \
                      AND current_root.lane='deep_archive' \
                      AND current_root.stop_conditions #>> '{progressiveArchive,version}'='1' \
                      AND current_root.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}' \
                          =current_root.work_order_ref::text \
                    ORDER BY current_root.created_at DESC,current_root.work_order_ref DESC LIMIT 1 \
                  )),false) \
                  AS legacy_smaller_quota, \
                coalesce(bool_or(", historical_directory_scan_qualified_sql!(), "),false) \
                  AS historical_scan_eligible \
         FROM collection_observation_target target \
         JOIN collection_work_order work_order USING(target_ref) \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
         JOIN linggan_runtime_task task ON task.task_id=package.task_id \
         CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
           THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
         JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=package.package_ref \
         LEFT JOIN linggan_material_discovery_finding finding \
           ON finding.package_ref=package.package_ref AND finding.record_ordinal=disposition.record_ordinal \
         LEFT JOIN linggan_material_content_detail detail \
           ON detail.package_ref=package.package_ref AND detail.record_ordinal=disposition.record_ordinal \
         WHERE target.platform=$1 AND target.target_kind='creator' \
           AND work_order.lane='deep_archive' AND package.platform=$1 \
           AND disposition.disposition <> 'retained_uninterpreted' \
         GROUP BY target.identity_key"),
    )
    .bind(platform)
    .fetch_all(database.pool())
    .await?;
    for (author_external_id, works_listed, details_captured, legacy_smaller_quota, historical_scan_eligible) in historical_rows {
        if !historical_scan_eligible {
            continue;
        }
        let archive = totals.entry(author_external_id).or_default();
        archive.started = true;
        // A current progressive root takes precedence over any earlier accepted rows.  Until
        // that root has reached its bounded result, its old partial history must not leak back
        // into the visible denominator.  The historical fallback is only for targets that do
        // not have an active current root (the legacy South-Pumpkin case).
        if !archive.work_in_progress
            && (archive.directory_baseline == ArchiveDirectoryBaseline::NotStarted
                || legacy_smaller_quota)
            && works_listed > 0
            && details_captured >= works_listed
        {
            archive.works_listed = archive.works_listed.max(works_listed);
            archive.details_captured = archive.details_captured.max(details_captured);
            archive.directory_baseline = ArchiveDirectoryBaseline::HistoricalDirectory;
        }
    }

    // A pending material-scoped follow-up is not a homepage baseline in progress. Progressive
    // root children are already covered by `archive_progress`; this fallback recognizes legacy
    // root WorkOrders without a progressive marker.
    let pending_rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT target.identity_key \
         FROM collection_observation_target target \
         JOIN collection_work_order work_order USING(target_ref) \
         LEFT JOIN collection_work_order_lease lease USING(work_order_ref) \
         LEFT JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         WHERE target.platform=$1 AND target.target_kind='creator' \
           AND work_order.lane='deep_archive' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_material_target material_scope \
                           WHERE material_scope.work_order_ref=work_order.work_order_ref) \
           AND (work_order.queue_state='queued' \
                OR (lease_task.execution_state IN ('pending','in_progress') \
                    AND lease.released_at IS NULL AND lease.expires_at>scope_001_now()))",
    )
    .bind(platform)
    .fetch_all(database.pool())
    .await?;
    for author_external_id in pending_rows {
        let archive = totals.entry(author_external_id).or_default();
        archive.started = true;
        archive.work_in_progress = true;
        if archive.directory_baseline != ArchiveDirectoryBaseline::Ready
            && archive.directory_baseline != ArchiveDirectoryBaseline::HistoricalDirectory
        {
            archive.directory_baseline = ArchiveDirectoryBaseline::Building;
        }
    }
    Ok(totals)
}

/// 当前因连续读取失败而停在待补齐里的一篇作品，以及人做判断需要看到的东西。
#[derive(Debug, Clone)]
pub struct BlockedMaterial {
    pub content_public_ref: Uuid,
    pub content_external_id: String,
    /// 采集时记下的标题。可能为空——那也是事实，不编一个出来。
    pub title: Option<String>,
}

/// 读出这个目标当前**还需要人来判断**的作品。
///
/// 已经确认失效的不在其中：那个判断已经做过了，再列一遍等于问同一个问题两次。
pub async fn read_blocked_materials(
    database: &Database,
    target_ref: Uuid,
) -> Result<Vec<BlockedMaterial>, sqlx::Error> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(concat!(
        "SELECT DISTINCT content.public_ref,content.content_external_id, \
                (SELECT finding.title FROM linggan_material_discovery_finding finding \
                 WHERE finding.content_public_ref=content.public_ref \
                   AND NULLIF(btrim(finding.title),'') IS NOT NULL \
                 ORDER BY finding.created_at DESC LIMIT 1) \
         FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task runtime ON runtime.task_id=lease_task.task_id \
         JOIN linggan_material_content content \
           ON content.content_external_id=runtime.task_spec #>> '{target,contentExternalId}' \
         WHERE work_order.target_ref=$1 \
           AND lease_task.execution_state='blocked' \
           AND runtime.task_spec #>> '{capabilitiesRequested,0}'='content_detail' \
           -- 已经取到合格详情的不再问人；只有真的还欠着才进这张待补齐清单。
           AND ",
        qualified_detail_missing_sql!("content.public_ref"),
        " \
           AND NOT EXISTS (SELECT 1 FROM collection_material_retirement retired \
                           WHERE retired.target_ref=$1 \
                             AND retired.content_public_ref=content.public_ref) \
         ORDER BY content.content_external_id",
    ))
    .bind(target_ref)
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(content_public_ref, content_external_id, title)| BlockedMaterial {
                content_public_ref,
                content_external_id,
                title,
            },
        )
        .collect())
}

/// 人确认这几篇在平台上已经不存在了。
///
/// **只记结论，不动作品，也不动那次失败。**作品继续留在目录里（博主当时确实发过），受阻
/// 记录继续留着（那次读取失败确实发生过）——这里新增的是第三件事：有人看过，它没了。
///
/// 重复确认是同一个结论，靠唯一索引收敛成一条，不报错也不写第二条：人多点一次不该变成
/// 两个事实。
///
/// 只接受**已经在这个目标目录里、且当前确实没有详情**的作品。凭一个任意 id 就能写下
/// 「已失效」，等于给这张表开一个不受目录约束的后门。
pub async fn retire_materials(
    database: &Database,
    target_ref: Uuid,
    content_public_refs: &[Uuid],
    reason_code: &str,
) -> Result<u64, sqlx::Error> {
    if content_public_refs.is_empty() || !matches!(reason_code, "page_gone" | "page_unreadable") {
        return Ok(0);
    }
    let inserted = sqlx::query(
        "INSERT INTO collection_material_retirement \
             (retirement_ref,target_ref,content_public_ref,reason_code,decided_by) \
         SELECT gen_random_uuid(),$1,candidate.content_public_ref,$3,'person' \
         FROM unnest($2::uuid[]) AS candidate(content_public_ref) \
         WHERE EXISTS (SELECT 1 FROM linggan_material_content content \
                       WHERE content.public_ref=candidate.content_public_ref) \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_content_detail detail \
                           WHERE detail.content_public_ref=candidate.content_public_ref) \
         ON CONFLICT (target_ref,content_public_ref) DO NOTHING",
    )
    .bind(target_ref)
    .bind(content_public_refs)
    .bind(reason_code)
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok(inserted)
}
