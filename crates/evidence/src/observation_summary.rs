//! Target-scoped, user-facing observation facts.
//!
//! The Collection target ledger needs one small read model for answers such as
//! “did the latest patrol find anything new?”.  It is deliberately not a new
//! observation store: every value below is calculated from accepted discovery
//! packages already tied to this target's patrol Work Orders.

use crate::ObservationTarget;
use linggan_storage_postgres::Database;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PatrolReadState {
    Disabled,
    Waiting,
    Running,
    Normal,
    Blocked,
    Unavailable,
}

#[derive(Debug, Clone, Default)]
pub struct TargetObservationSummary {
    /// How many accepted discovery records the latest successful patrol returned.
    pub latest_hits: Option<i64>,
    /// Of those hits, how many were first seen by this target in that patrol.
    pub latest_new: Option<i64>,
    pub patrol_state: PatrolReadState,
}

impl Default for PatrolReadState {
    fn default() -> Self {
        Self::Unavailable
    }
}

/// Read target-local patrol results in one query.  A target with no qualified
/// successful patrol remains `None` for the two counts: that is different from
/// a successful patrol that found zero works.
pub async fn read_target_observation_summaries(
    database: &Database,
    targets: &[ObservationTarget],
) -> Result<HashMap<Uuid, TargetObservationSummary>, sqlx::Error> {
    if targets.is_empty() {
        return Ok(HashMap::new());
    }
    let refs = targets
        .iter()
        .map(|target| target.target_ref)
        .collect::<Vec<_>>();
    let rows: Vec<(Uuid, Option<i64>, Option<i64>, bool, bool, bool)> = sqlx::query_as(
        "WITH selected AS ( \
             SELECT target_ref,target_kind,monitoring_enabled,lifecycle_state \
             FROM collection_observation_target WHERE target_ref=ANY($1) \
         ), successful AS ( \
             SELECT selected.target_ref,package.package_ref,package.accepted_at, \
                    row_number() OVER (PARTITION BY selected.target_ref \
                                       ORDER BY package.accepted_at DESC,package.package_ref DESC) AS package_rank \
             FROM selected \
             JOIN collection_work_order work_order USING(target_ref) \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             WHERE work_order.lane='patrol' \
               AND package.package_kind=CASE WHEN selected.target_kind='creator' \
                                              THEN 'profile_discovery' ELSE 'discovery_search' END \
               AND package.platform=task.platform \
               AND task.task_spec->'capabilitiesRequested' ? package.package_kind \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND receipt.material_admission='ACCEPTED' \
         ), latest AS ( \
             SELECT target_ref,package_ref,accepted_at FROM successful WHERE package_rank=1 \
         ), latest_counts AS ( \
             SELECT latest.target_ref, \
                    count(*) FILTER (WHERE disposition.disposition <> 'quarantined') AS hits, \
                    count(*) FILTER (WHERE disposition.disposition <> 'quarantined' AND NOT EXISTS ( \
                        SELECT 1 FROM linggan_material_discovery_finding earlier \
                        JOIN linggan_runtime_capture_package earlier_package USING(package_ref) \
                        JOIN linggan_runtime_submission_receipt earlier_receipt USING(package_ref) \
                        JOIN linggan_runtime_task earlier_task ON earlier_task.task_id=earlier_package.task_id \
                        JOIN collection_work_order_lease_task earlier_lease_task \
                          ON earlier_lease_task.task_id=earlier_task.task_id \
                        JOIN collection_work_order_lease earlier_lease USING(lease_ref) \
                        JOIN collection_work_order earlier_order USING(work_order_ref) \
                        WHERE earlier_order.target_ref=latest.target_ref \
                          AND earlier.discovery_kind=finding.discovery_kind \
                          AND earlier.content_public_ref=finding.content_public_ref \
                          AND earlier_receipt.material_admission='ACCEPTED' \
                          AND earlier_package.accepted_at < latest.accepted_at \
                    )) AS newly_discovered \
             FROM latest \
             JOIN linggan_material_discovery_finding finding USING(package_ref) \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal \
             GROUP BY latest.target_ref \
         ), active_patrol AS ( \
             SELECT DISTINCT selected.target_ref \
             FROM selected JOIN collection_work_order work_order USING(target_ref) \
             LEFT JOIN collection_work_order_lease lease USING(work_order_ref) \
             WHERE work_order.lane='patrol' \
               AND (work_order.queue_state='queued' OR (lease.released_at IS NULL AND lease.expires_at>scope_001_now())) \
         ), blocked_patrol AS ( \
             -- 「受阻」说的是**现在有事卡着、需要人看一眼**，不是「历史上重试过」。
             -- 派发失败会自动重排队（failure_disposition='requeued'），失败计数是那次
             -- 重试的留痕，会永久留在工单上；只看它 >0 就把重试后成功的工单也判成受阻，
             -- 而且**永不恢复**——这个标记因此从来没有正确工作过：库里有失败计数的
             -- 工单全部已经 completed 或 cancelled，一张卡住的都没有，界面却一直红着。
             -- 一个永远亮的警告等于没有警告，真正卡住时反而看不出来。
             -- 只有仍在队列里或已租出未完成的工单，才谈得上受阻。其中 'queued' 这一支
             -- 实际总被 Running 抢先（active_patrol 也认这个状态、判定顺序又在前），
             -- 留着它是为了把「未结束」这个判据说完整，不是它在起作用。
             SELECT DISTINCT selected.target_ref \
             FROM selected JOIN collection_work_order work_order USING(target_ref) \
             WHERE work_order.lane='patrol' AND work_order.dispatch_failure_count>0 \
               AND work_order.queue_state IN ('queued','leased') \
         ) \
         SELECT selected.target_ref,latest_counts.hits,latest_counts.newly_discovered, \
                selected.monitoring_enabled,active_patrol.target_ref IS NOT NULL, \
                blocked_patrol.target_ref IS NOT NULL \
         FROM selected \
         LEFT JOIN latest_counts USING(target_ref) \
         LEFT JOIN active_patrol USING(target_ref) \
         LEFT JOIN blocked_patrol USING(target_ref)",
    )
    .bind(&refs)
    .fetch_all(database.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(target_ref, hits, newly_discovered, monitoring_enabled, running, blocked)| {
                let patrol_state = if !monitoring_enabled {
                    PatrolReadState::Disabled
                } else if running {
                    PatrolReadState::Running
                } else if blocked {
                    PatrolReadState::Blocked
                } else if hits.is_some() {
                    PatrolReadState::Normal
                } else {
                    PatrolReadState::Waiting
                };
                (
                    target_ref,
                    TargetObservationSummary {
                        latest_hits: hits,
                        latest_new: newly_discovered,
                        patrol_state,
                    },
                )
            },
        )
        .collect())
}
