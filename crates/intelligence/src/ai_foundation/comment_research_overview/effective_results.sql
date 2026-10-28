SELECT current.request_ref,current.result
FROM qualified q
JOIN LATERAL(SELECT r.request_ref,r.created_at,result.result,original.body_text
FROM linggan_comment_research_request r JOIN linggan_material_comment original ON original.material_ref=r.source_ref JOIN linggan_comment_research_result result USING(request_ref)
WHERE r.domain_ref=$1 AND r.work_ref=q.content_public_ref AND original.comment_external_id=q.comment_external_id AND result.result IS NOT NULL AND result.created_at<=$2::timestamptz AND NOT EXISTS(SELECT 1
FROM linggan_comment_research_cancellation cancelled
WHERE cancelled.request_ref=r.request_ref)
ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 1)current ON current.body_text IS NOT DISTINCT
FROM (SELECT body_text
FROM linggan_material_comment
WHERE material_ref=q.material_ref)
WHERE q.exclusion_reason IS NULL
ORDER BY current.created_at DESC,current.request_ref DESC
