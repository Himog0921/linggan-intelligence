SELECT DISTINCT a.domain_ref,core.request_ref,core.work_ref,
 md5('topic-auto:'||a.domain_ref::text||':'||core.request_ref::text||':'||$3)::uuid AS command_ref
FROM linggan_topic_research_automation a
JOIN observation_domain d USING(domain_ref)
JOIN linggan_material_domain_usage usage USING(domain_ref)
JOIN LATERAL (
 SELECT r.request_ref,r.work_ref FROM linggan_content_understanding_result r
 WHERE r.work_ref=usage.content_public_ref AND r.result IS NOT NULL
 ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 1
) core ON true
WHERE a.enabled AND d.status='active' AND usage.use_kind='domain_research'
 AND ($1::uuid IS NULL OR (a.domain_ref,core.request_ref)>($1::uuid,$2::uuid))
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_control c WHERE c.scope_key IN('global','executor','task:topic_research','task:topic_research_extract_review','task:topic_research_resolve','task:topic_research_resolve_review') AND NOT c.enabled)
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=s.current_config_ref WHERE s.task_key IN('topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND cfg.budget_mode='unconfigured')
 AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) WHERE r.domain_ref=a.domain_ref AND w.input_refs#>>'{coverage,coreRef}'=core.request_ref::text AND w.input_refs->>'methodVersion'=$3)
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_command c WHERE c.command_ref=md5('topic-auto:'||a.domain_ref::text||':'||core.request_ref::text||':'||$3)::uuid)
 AND (SELECT count(DISTINCT w.window_ref) FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=a.domain_ref AND b.kind='automatic' AND c.state IN('pending','running'))<8
ORDER BY a.domain_ref,core.request_ref
LIMIT 8
