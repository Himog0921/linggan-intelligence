SELECT DISTINCT a.domain_ref,u.content_public_ref AS work_ref
FROM linggan_content_understanding_automation a
JOIN observation_domain d USING(domain_ref)
JOIN linggan_material_domain_usage u ON u.domain_ref=a.domain_ref AND u.use_kind='domain_research'
WHERE a.enabled AND d.status='active'
 AND ($4::uuid IS NULL OR (a.domain_ref=$4 AND u.content_public_ref=ANY($5::uuid[])))
 AND ($1::uuid IS NULL OR (a.domain_ref,u.content_public_ref)>($1::uuid,$2::uuid))
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_control c WHERE c.scope_key IN('global','executor','task:content_understanding') AND NOT c.enabled)
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=s.current_config_ref WHERE s.task_key='content_understanding' AND cfg.budget_mode='unconfigured')
ORDER BY 1,2
LIMIT $3
