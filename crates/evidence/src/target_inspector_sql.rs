//! SQL owned by the target inspector read projection.

use crate::archive_ledger::directory_works_sql;
use crate::directory_boundary::{directory_proven_sql, historical_directory_scan_qualified_sql, patrol_scan_qualified_sql};

pub(super) const EXECUTION_SQL: &str = r#"
SELECT work_order.lane,
 count(DISTINCT work_order.work_order_ref) FILTER (WHERE work_order.queue_state='queued') AS queued_work_orders,
 count(DISTINCT lease_task.task_id) FILTER (WHERE lease.released_at IS NULL AND lease.expires_at>$2::timestamptz AND (lease_task.execution_state='pending' OR (lease_task.execution_state='in_progress' AND NOT EXISTS (SELECT 1 FROM linggan_runtime_attempt attempt WHERE attempt.task_id=lease_task.task_id)))) AS awaiting_producer_tasks,
 count(DISTINCT lease_task.task_id) FILTER (WHERE lease.released_at IS NULL AND lease.expires_at>$2::timestamptz AND lease_task.execution_state='in_progress' AND EXISTS (SELECT 1 FROM linggan_runtime_attempt attempt WHERE attempt.task_id=lease_task.task_id) AND NOT EXISTS (SELECT 1 FROM linggan_runtime_capture_package package WHERE package.task_id=lease_task.task_id AND package.accepted_at<=$2::timestamptz)) AS running_attempts,
 -- Historical failures remain in archive coverage/history; they are not current execution.
 count(DISTINCT lease_task.task_id) FILTER (WHERE lease.released_at IS NULL AND lease.expires_at>$2::timestamptz AND lease_task.execution_state IN ('blocked','unavailable')) AS blocked_tasks
FROM collection_work_order work_order
LEFT JOIN collection_work_order_lease lease USING(work_order_ref)
LEFT JOIN collection_work_order_lease_task lease_task USING(lease_ref)
WHERE work_order.target_ref=$1 AND work_order.lane IN ('deep_archive','patrol') AND work_order.created_at<=$2::timestamptz
GROUP BY work_order.lane
"#;

pub(super) const ARCHIVE_SQL: &str = concat!(
    r#"
WITH orders AS (
 SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1 AND lane='deep_archive' AND created_at<=$2::timestamptz
), root_orders AS (
 SELECT orders.work_order_ref FROM orders WHERE NOT EXISTS (
  SELECT 1 FROM collection_work_order_material_target material_scope
  WHERE material_scope.work_order_ref=orders.work_order_ref
 )
), latest_progressive_root AS (
 SELECT root_orders.work_order_ref FROM root_orders
 JOIN collection_work_order work_order USING(work_order_ref)
 WHERE work_order.stop_conditions #>> '{progressiveArchive,version}'='1'
   AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text
 ORDER BY work_order.created_at DESC,work_order.work_order_ref DESC LIMIT 1
), records AS (
 SELECT orders.work_order_ref,task.task_spec,package.package_ref,package.package_kind,package.coverage,package.checkpoint,disposition.record_ordinal,
        CASE WHEN disposition.requalified_at>$2::timestamptz THEN disposition.initial_disposition ELSE disposition.disposition END AS disposition
 FROM orders JOIN collection_work_order_lease lease USING(work_order_ref)
 JOIN collection_work_order_lease_task lease_task USING(lease_ref)
 JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
 JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id AND package.accepted_at<=$2::timestamptz
 JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
 JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=package.package_ref AND disposition.created_at<=$2::timestamptz
 WHERE receipt.material_admission='ACCEPTED'
), "#,
    directory_works_sql!(
        "target.target_ref=$1",
        "package.accepted_at<=$2::timestamptz"
    ),
    r#", blocked AS (
 SELECT count(DISTINCT task.task_spec #>> '{target,contentExternalId}') AS total
 FROM orders JOIN collection_work_order_lease lease USING(work_order_ref)
 JOIN collection_work_order_lease_task lease_task USING(lease_ref)
 JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
 WHERE lease_task.execution_state='blocked' AND NULLIF(task.task_spec #>> '{target,contentExternalId}','') IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM linggan_material_content content JOIN directory_work detail ON detail.content_public_ref=content.public_ref AND (detail.has_detail OR detail.is_retired) WHERE content.content_external_id=task.task_spec #>> '{target,contentExternalId}')
), ready AS (
 SELECT EXISTS(SELECT 1 FROM records CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(records.coverage->'layers')='array' THEN records.coverage->'layers' ELSE '[]'::jsonb END) layer
 WHERE records.package_kind='profile_discovery'
    AND records.work_order_ref IN (SELECT work_order_ref FROM latest_progressive_root)
    AND EXISTS (SELECT 1 FROM linggan_runtime_submission_receipt proof WHERE proof.package_ref=records.package_ref AND proof.execution_effect='COMPLETED_LIVE_STEP')
    AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition rejected
                    WHERE rejected.package_ref=records.package_ref AND rejected.disposition='quarantined')
    AND (SELECT count(*) FROM linggan_runtime_record_disposition accepted
         WHERE accepted.package_ref=records.package_ref
           AND accepted.disposition='accepted_for_library_discovery')
        =COALESCE((layer->>'acquired')::integer,-1)
    AND "#,
    directory_proven_sql!(),
    r#") AS value
), historical_root AS (
 SELECT EXISTS (
  SELECT 1 FROM root_orders
  JOIN collection_work_order work_order USING(work_order_ref)
  JOIN collection_work_order_lease lease USING(work_order_ref)
  JOIN collection_work_order_lease_task lease_task USING(lease_ref)
  JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
  JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
  CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array'
    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer
  WHERE package.accepted_at<=$2::timestamptz AND "#,
    historical_directory_scan_qualified_sql!(),
    r#"
    AND (NOT EXISTS (SELECT 1 FROM latest_progressive_root)
      OR (work_order.work_order_ref=(SELECT work_order_ref FROM latest_progressive_root)
          AND (task.task_spec->>'maximumQuota')::integer BETWEEN 1 AND 199))
 ) AS value
), patrol_quarantine AS (
 SELECT count(*) AS total FROM collection_work_order work_order
 JOIN collection_work_order_lease lease USING(work_order_ref)
 JOIN collection_work_order_lease_task lease_task USING(lease_ref)
 JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id
 JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
 JOIN linggan_runtime_record_disposition disposition USING(package_ref)
 WHERE work_order.target_ref=$1 AND work_order.lane='patrol'
   AND package.accepted_at<=$2::timestamptz AND receipt.material_admission='ACCEPTED'
   AND disposition.created_at<=$2::timestamptz AND disposition.disposition='quarantined'
)
SELECT EXISTS(SELECT 1 FROM root_orders) AS started,
 EXISTS(SELECT 1 FROM root_orders JOIN collection_work_order_lease lease USING(work_order_ref) JOIN collection_work_order_lease_task lease_task USING(lease_ref) JOIN linggan_runtime_attempt attempt ON attempt.task_id=lease_task.task_id WHERE attempt.started_at<=$2::timestamptz) AS attempted,
 (SELECT count(DISTINCT package_ref) FROM records WHERE package_kind='author_profile' AND disposition<>'quarantined') AS profiles,
 (SELECT count(*) FROM directory_work) AS works, 
 (SELECT count(*) FROM directory_work WHERE has_detail) AS details,
 (SELECT count(*) FROM directory_work WHERE is_retired AND NOT has_detail) AS retired_works,
 (SELECT count(*) FROM records WHERE disposition='quarantined')
   + (SELECT total FROM patrol_quarantine) AS quarantined,
 (SELECT total FROM blocked) AS blocked_details,(SELECT value FROM ready) AS standard_directory_ready,
 (SELECT value FROM historical_root) AS historical_root_eligible
"#,
);

pub(super) const LATEST_PATROL_SQL: &str = concat!(r#"
WITH latest AS (
 SELECT package.package_ref,package.accepted_at FROM collection_work_order work_order
 JOIN collection_work_order_lease lease USING(work_order_ref) JOIN collection_work_order_lease_task lease_task USING(lease_ref)
 JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
 JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
 CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer
 WHERE work_order.target_ref=$1 AND work_order.lane='patrol' AND package.package_kind=CASE WHEN $2='creator' THEN 'profile_discovery' ELSE 'discovery_search' END
   AND package.accepted_at<=$3::timestamptz AND receipt.execution_effect='COMPLETED_LIVE_STEP' AND receipt.material_admission='ACCEPTED'
   AND layer->>'capability'=package.package_kind AND "#, patrol_scan_qualified_sql!(), r#"
 ORDER BY package.accepted_at DESC,package.package_ref DESC LIMIT 1
)
SELECT count(finding.content_public_ref) FILTER (WHERE disposition.disposition<>'quarantined') AS hits,
 count(finding.content_public_ref) FILTER (WHERE disposition.disposition<>'quarantined' AND NOT EXISTS (
  SELECT 1 FROM linggan_material_discovery_finding earlier JOIN linggan_runtime_capture_package earlier_package USING(package_ref)
  JOIN linggan_runtime_submission_receipt earlier_receipt USING(package_ref) JOIN linggan_runtime_task earlier_task ON earlier_task.task_id=earlier_package.task_id
  JOIN collection_work_order_lease_task earlier_lease_task ON earlier_lease_task.task_id=earlier_task.task_id JOIN collection_work_order_lease earlier_lease USING(lease_ref)
  JOIN collection_work_order earlier_order USING(work_order_ref)
  WHERE earlier_order.target_ref=$1 AND earlier.discovery_kind=finding.discovery_kind AND earlier.content_public_ref=finding.content_public_ref
    AND earlier_receipt.material_admission='ACCEPTED' AND earlier_package.accepted_at<latest.accepted_at)) AS newly_discovered
FROM latest LEFT JOIN linggan_material_discovery_finding finding USING(package_ref)
LEFT JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal
GROUP BY latest.package_ref
"#);

pub(super) const LATEST_PATROL_INTERRUPTED_SQL: &str = concat!(r#"
SELECT NOT (receipt.execution_effect='COMPLETED_LIVE_STEP' AND "#, patrol_scan_qualified_sql!(), r#") FROM collection_work_order work_order
JOIN collection_work_order_lease lease USING(work_order_ref)
JOIN collection_work_order_lease_task lease_task USING(lease_ref)
JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
CROSS JOIN LATERAL jsonb_array_elements(CASE WHEN jsonb_typeof(package.coverage->'layers')='array' THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer
WHERE work_order.target_ref=$1 AND work_order.lane='patrol'
  AND package.package_kind=CASE WHEN $2='creator' THEN 'profile_discovery' ELSE 'discovery_search' END
  AND package.accepted_at<=$3::timestamptz
  AND receipt.material_admission='ACCEPTED' AND layer->>'capability'=package.package_kind
ORDER BY package.accepted_at DESC,package.package_ref DESC LIMIT 1
"#);
