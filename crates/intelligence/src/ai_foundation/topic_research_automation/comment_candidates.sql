WITH latest AS (
 SELECT DISTINCT ON(r.domain_ref,r.work_ref,m.comment_external_id)
 r.domain_ref,r.request_ref,result.result
 FROM linggan_comment_research_request r
 JOIN linggan_comment_research_result result USING(request_ref)
 JOIN linggan_material_comment m ON m.material_ref=r.source_ref
 JOIN linggan_ai_batch_item i USING(batch_ref)
 JOIN linggan_ai_batch_item_control c USING(item_ref)
 WHERE c.state='succeeded'
 ORDER BY r.domain_ref,r.work_ref,m.comment_external_id,r.created_at DESC,r.request_ref DESC
)
SELECT a.domain_ref,semantic.request_ref
FROM linggan_topic_research_automation a
JOIN observation_domain d USING(domain_ref)
JOIN latest semantic USING(domain_ref)
WHERE a.enabled AND d.status='active' AND jsonb_array_length(semantic.result->'signals')>0
 AND ($1::uuid IS NULL OR (a.domain_ref,semantic.request_ref)>($1::uuid,$2::uuid))
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_control c WHERE c.scope_key IN('global','executor','task:topic_research') AND NOT c.enabled)
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=s.current_config_ref WHERE s.task_key IN('topic_research') AND cfg.budget_mode='unconfigured')
 AND (SELECT count(DISTINCT w.window_ref) FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=a.domain_ref AND b.kind='automatic' AND c.state IN('pending','running'))<8
ORDER BY a.domain_ref,semantic.request_ref LIMIT 8
