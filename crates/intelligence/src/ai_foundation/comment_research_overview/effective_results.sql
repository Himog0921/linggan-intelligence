-- Choose the latest successful request before checking whether its source body is still current.
, current_results AS MATERIALIZED (
    SELECT DISTINCT ON (r.work_ref, original.comment_external_id)
           r.work_ref, original.comment_external_id, r.request_ref, r.created_at,
           result.result, original.body_text
    FROM linggan_comment_research_request r
    JOIN linggan_comment_research_result result USING (request_ref)
    JOIN linggan_material_comment original ON original.material_ref = r.source_ref
    WHERE r.method_version = 'comment-understanding.v1' AND r.domain_ref = $1
      AND result.result IS NOT NULL AND result.created_at <= $2::timestamptz
    ORDER BY r.work_ref, original.comment_external_id, r.created_at DESC, r.request_ref DESC
)
SELECT current.request_ref, current.result
FROM current_results current
JOIN qualified q ON q.content_public_ref = current.work_ref
                AND q.comment_external_id = current.comment_external_id
JOIN linggan_material_comment head ON head.material_ref = q.material_ref
WHERE q.exclusion_reason IS NULL AND current.body_text IS NOT DISTINCT FROM head.body_text
ORDER BY current.created_at DESC, current.request_ref DESC
