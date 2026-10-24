WITH selected AS (
    SELECT finding.*
    FROM linggan_material_discovery_finding finding
    JOIN linggan_runtime_record_disposition disposition USING(package_ref,record_ordinal)
    WHERE finding.package_ref=$4
      AND disposition.disposition='accepted_for_library_discovery'
      AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage
        WHERE usage.content_public_ref=finding.content_public_ref
          AND usage.package_ref=finding.package_ref AND usage.record_ordinal=finding.record_ordinal
          AND ($2::uuid IS NULL OR usage.domain_ref=$2))
), source_history AS (
    SELECT finding.content_public_ref,finding.package_ref,finding.record_ordinal
    FROM linggan_material_discovery_finding finding
    WHERE finding.content_public_ref IN (SELECT content_public_ref FROM selected)
    UNION ALL
    SELECT detail.content_public_ref,detail.package_ref,detail.record_ordinal
    FROM linggan_material_content_detail detail
    WHERE detail.content_public_ref IN (SELECT content_public_ref FROM selected)
), history AS (
    SELECT source.content_public_ref,package.package_ref,package.observed_at::timestamptz AS observed_at,
           package.accepted_at,work_order.target_ref,revision.rule_ref,package.package_kind
    FROM source_history source
    JOIN linggan_runtime_capture_package package USING(package_ref)
    JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
    JOIN linggan_runtime_record_disposition disposition USING(package_ref,record_ordinal)
    JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id
    JOIN collection_work_order_lease lease USING(lease_ref)
    JOIN collection_work_order work_order USING(work_order_ref)
    LEFT JOIN collection_monitor_rule_revision revision
      ON revision.rule_revision_ref=work_order.monitor_rule_revision_ref
    WHERE receipt.material_admission='ACCEPTED'
      AND disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content')
      AND package.accepted_at <= $3::timestamptz
      AND package.observed_at::timestamptz <= $3::timestamptz
      AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage usage
        WHERE usage.work_order_ref=work_order.work_order_ref
          AND ($2::uuid IS NULL OR usage.domain_ref=$2))
)
SELECT DISTINCT ON(finding.content_public_ref)
       finding.content_public_ref,finding.material_ref,finding.title,finding.creator_display_name,
       finding.result_position::bigint AS position,
       finding.like_count,finding.like_count_state,finding.comment_count,finding.comment_count_state,
       finding.collect_count,finding.collect_count_state,finding.share_count,finding.share_count_state,
       package.payload #> ARRAY['records',finding.record_ordinal::text,'payload'] AS raw,
       NOT EXISTS(SELECT 1 FROM history earlier
         WHERE earlier.content_public_ref=finding.content_public_ref AND earlier.target_ref=$1
           AND ($5::uuid IS NULL OR earlier.rule_ref=$5)
           AND earlier.package_kind IN ('discovery_search','profile_discovery')
           AND earlier.observed_at < package.observed_at::timestamptz) AS first_seen_in_scope,
       CASE WHEN EXISTS(SELECT 1 FROM history earlier
             WHERE earlier.content_public_ref=finding.content_public_ref
               AND earlier.accepted_at<package.accepted_at) THEN 'existing'
            WHEN EXISTS(SELECT 1 FROM history concurrent
             WHERE concurrent.content_public_ref=finding.content_public_ref
               AND concurrent.accepted_at=package.accepted_at AND concurrent.package_ref<>package.package_ref)
              THEN 'unknown'
            ELSE 'new' END AS library_presence,
       EXISTS(SELECT 1 FROM history earlier
         WHERE earlier.content_public_ref=finding.content_public_ref AND earlier.target_ref=$1
           AND ($5::uuid IS NULL OR earlier.rule_ref=$5)
           AND earlier.package_kind IN ('discovery_search','profile_discovery')
           AND earlier.observed_at<$6::timestamptz) AS seen_before_left,
       (SELECT count(DISTINCT other.like_count)>1 FROM selected other
           WHERE other.content_public_ref=finding.content_public_ref) AS conflicting_likes,
       (SELECT count(DISTINCT other.comment_count)>1 FROM selected other
           WHERE other.content_public_ref=finding.content_public_ref) AS conflicting_comments,
       (SELECT count(DISTINCT other.collect_count)>1 FROM selected other
           WHERE other.content_public_ref=finding.content_public_ref) AS conflicting_collects,
       (SELECT count(DISTINCT other.share_count)>1 FROM selected other
           WHERE other.content_public_ref=finding.content_public_ref) AS conflicting_shares
FROM selected finding
JOIN linggan_runtime_capture_package package USING(package_ref)
ORDER BY finding.content_public_ref,finding.record_ordinal
LIMIT $7
