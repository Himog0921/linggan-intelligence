WITH observations AS (
    SELECT material_ref,content_public_ref,package_ref,record_ordinal,
           discovery_kind AS source_kind,observed_at::timestamptz AS observed_at,
           result_position::bigint AS position,
           like_count,like_count_state,comment_count,comment_count_state,
           collect_count,collect_count_state,share_count,share_count_state
    FROM linggan_material_discovery_finding WHERE content_public_ref=$3
    UNION ALL
    SELECT material_ref,content_public_ref,package_ref,record_ordinal,
           'content_detail',observed_at::timestamptz,NULL::bigint,
           like_count,like_count_state,comment_count,comment_count_state,
           collect_count,collect_count_state,share_count,share_count_state
    FROM linggan_material_content_detail WHERE content_public_ref=$3
), qualified AS (
    SELECT observation.*,package.attempt_id,package.accepted_at,
           package.payload #> ARRAY['records',observation.record_ordinal::text,'payload'] AS raw,
           work_order.target_ref,target.display_name AS target_name,rule.slot_key
    FROM observations observation
    JOIN linggan_runtime_capture_package package USING(package_ref)
    JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
    JOIN linggan_runtime_record_disposition disposition
      ON disposition.package_ref=observation.package_ref AND disposition.record_ordinal=observation.record_ordinal
    LEFT JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id
    LEFT JOIN collection_work_order_lease lease USING(lease_ref)
    LEFT JOIN collection_work_order work_order USING(work_order_ref)
    LEFT JOIN collection_observation_target target USING(target_ref)
    LEFT JOIN collection_monitor_rule_revision revision
      ON revision.rule_revision_ref=work_order.monitor_rule_revision_ref
    LEFT JOIN collection_monitor_rule rule ON rule.rule_ref=revision.rule_ref
    WHERE receipt.material_admission='ACCEPTED'
      AND disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content')
      AND package.accepted_at <= $4::timestamptz AND observation.observed_at <= $4::timestamptz
      AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage usage
          WHERE usage.work_order_ref=work_order.work_order_ref
            AND ($2::uuid IS NULL OR usage.domain_ref=$2))
      AND $1::uuid IS NOT NULL
)
SELECT qualified.*,
       to_char(observed_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS observed_text,
       (extract(epoch FROM observed_at)*1000)::bigint AS observed_epoch_ms,
       to_char(accepted_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS accepted_text,
       EXISTS(SELECT 1 FROM qualified newer WHERE newer.observed_at>qualified.observed_at
               AND newer.accepted_at<qualified.accepted_at) AS late_arrival
FROM qualified
WHERE $7::uuid[] IS NULL OR material_ref=ANY($7)
ORDER BY observed_at DESC,material_ref DESC
LIMIT $5 OFFSET $6
