WITH latest AS (
 SELECT DISTINCT ON(r.work_ref,m.comment_external_id)
 r.request_ref,r.work_ref,r.source_ref,m.comment_external_id,r.created_at
 FROM linggan_comment_research_request r
 JOIN linggan_comment_research_result result USING(request_ref)
 JOIN linggan_material_comment m ON m.material_ref=r.source_ref
 WHERE r.domain_ref=$1 AND ($2::uuid IS NULL OR r.work_ref=$2) AND ($3::uuid IS NULL OR r.request_ref=$3)
 ORDER BY r.work_ref,m.comment_external_id,r.created_at DESC,r.request_ref DESC
)
SELECT semantic.*,w.window_ref,result.core_json,dispatch.context,
 (result.result_ref IS NULL
 AND EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.window_ref=w.window_ref AND c.state='failed')
 AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.window_ref=w.window_ref AND (c.state IN('pending','running') OR c.failure_code='ai_dispatch_unknown'))) AS retryable,

 CASE WHEN result.result_ref IS NOT NULL THEN 'complete'
 WHEN w.window_ref IS NULL THEN 'not_analyzed'
 WHEN EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.window_ref=w.window_ref AND c.state IN('pending','running')) THEN 'pending'
 ELSE 'failed' END AS assignment_state
FROM latest semantic
LEFT JOIN LATERAL (
 SELECT w.window_ref FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref)
 WHERE r.domain_ref=$1 AND w.input_refs->>'inputContract'='topic-map.comment-semantic-source.v1'
 AND w.input_refs->>'semanticRequestRef'=semantic.request_ref::text
 ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 1
) w ON true
LEFT JOIN linggan_topic_research_result result USING(window_ref)
LEFT JOIN linggan_topic_research_stage stage ON stage.window_ref=w.window_ref AND stage.stage_number=2
LEFT JOIN linggan_topic_research_dispatch dispatch USING(item_ref)
ORDER BY semantic.created_at DESC,semantic.request_ref DESC
