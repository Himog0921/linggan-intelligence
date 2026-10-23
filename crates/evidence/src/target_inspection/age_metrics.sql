WITH history AS (
    SELECT content_public_ref,material_ref,package_ref,record_ordinal,observed_at::timestamptz AS observed_at,
           'content_detail'::text AS source_kind,like_count,like_count_state,comment_count,comment_count_state,
           collect_count,collect_count_state,share_count,share_count_state
    FROM linggan_material_content_detail WHERE content_public_ref=$1
    UNION ALL
    SELECT content_public_ref,material_ref,package_ref,record_ordinal,observed_at::timestamptz,discovery_kind,
           like_count,like_count_state,comment_count,comment_count_state,
           collect_count,collect_count_state,share_count,share_count_state
    FROM linggan_material_discovery_finding WHERE content_public_ref=$1
)
SELECT DISTINCT ON(metric.name) metric.name,metric.value,history.material_ref,
       history.source_kind,
       to_char(history.observed_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS observed_at,
       (extract(epoch FROM history.observed_at)*1000)::bigint AS observed_epoch_ms,
       package.payload #> ARRAY['records',history.record_ordinal::text,'payload'] AS raw
FROM history
JOIN linggan_runtime_capture_package package USING(package_ref)
JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
JOIN linggan_runtime_record_disposition disposition USING(package_ref,record_ordinal)
JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id
JOIN collection_work_order_lease lease USING(lease_ref)
JOIN collection_work_order work_order USING(work_order_ref)
CROSS JOIN LATERAL (VALUES
    ('likes',history.like_count,history.like_count_state),
    ('comments',history.comment_count,history.comment_count_state),
    ('collects',history.collect_count,history.collect_count_state),
    ('shares',history.share_count,history.share_count_state)
) metric(name,value,state)
WHERE receipt.material_admission='ACCEPTED'
  AND disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content')
  AND metric.state='KNOWN' AND metric.value>=0
  AND package.accepted_at <= $2::timestamptz
  AND history.observed_at <= $2::timestamptz
  AND abs(extract(epoch FROM history.observed_at)-$3::double precision/1000.0-86400)<=3600
  AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage usage
      WHERE usage.work_order_ref=work_order.work_order_ref AND ($4::uuid IS NULL OR usage.domain_ref=$4))
ORDER BY metric.name,
         abs(extract(epoch FROM history.observed_at)-$3::double precision/1000.0-86400),
         history.observed_at DESC,(history.source_kind='content_detail') DESC,history.material_ref
