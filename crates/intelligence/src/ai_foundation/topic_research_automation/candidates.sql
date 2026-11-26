WITH cutoff AS (
    SELECT statement_timestamp() - interval '1 day' AS recent_since
), saved_cores AS MATERIALIZED (
    SELECT work_ref,request_ref,max(created_at) AS created_at
    FROM linggan_content_understanding_saved_core WHERE result IS NOT NULL
    GROUP BY work_ref,request_ref
), latest_core AS (
    SELECT DISTINCT ON(work_ref) work_ref,request_ref,created_at
    FROM saved_cores ORDER BY work_ref,created_at DESC,request_ref DESC
), researched AS MATERIALIZED (
    SELECT DISTINCT r.domain_ref,w.input_refs#>>'{coverage,coreRef}' AS core_ref
    FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref)
    WHERE w.input_refs->>'methodVersion'=$3
), pending_windows AS (
    SELECT DISTINCT r.domain_ref, w.window_ref, w.work_ref, w.input_refs#>>'{coverage,coreRef}' AS core_ref
    FROM linggan_topic_research_run r
    JOIN linggan_topic_research_window w USING(run_ref)
    JOIN linggan_topic_research_stage s USING(window_ref)
    JOIN linggan_ai_batch_item i USING(item_ref)
    JOIN linggan_ai_batch b USING(batch_ref)
    JOIN linggan_ai_batch_item_control c USING(item_ref)
    WHERE b.kind='automatic' AND c.state IN('pending','running')
      AND i.input_ref->>'collectionRunRef' IS NOT DISTINCT FROM $6::uuid::text
), pending AS (
    SELECT w.domain_ref, count(*) AS total,
           count(*) FILTER (WHERE core.created_at >= cutoff.recent_since) AS recent
    FROM pending_windows w CROSS JOIN cutoff
    LEFT JOIN saved_cores core ON core.work_ref=w.work_ref AND core.request_ref::text=w.core_ref
    GROUP BY w.domain_ref
), eligible AS (
    SELECT DISTINCT a.domain_ref,core.request_ref,core.work_ref,
           core.created_at >= cutoff.recent_since AS recent,
           COALESCE(p.total,0) AS pending_total, COALESCE(p.recent,0) AS pending_recent,
           md5('topic-auto:'||a.domain_ref::text||':'||core.request_ref::text||':'||$3)::uuid AS command_ref
    FROM linggan_topic_research_automation a
    JOIN observation_domain d USING(domain_ref)
    JOIN linggan_material_domain_usage usage USING(domain_ref)
    LEFT JOIN pending p USING(domain_ref)
    CROSS JOIN cutoff
    JOIN latest_core core ON core.work_ref=usage.content_public_ref
    LEFT JOIN researched prior ON prior.domain_ref=a.domain_ref AND prior.core_ref=core.request_ref::text
    WHERE a.enabled AND d.status='active' AND usage.use_kind='domain_research'
      AND ($4::uuid IS NULL OR (a.domain_ref=$4 AND core.work_ref=ANY($5::uuid[])))
      AND COALESCE(p.total,0)<8
      AND NOT EXISTS(SELECT 1 FROM linggan_ai_control c WHERE c.scope_key IN('global','executor','task:topic_research') AND NOT c.enabled)
      AND NOT EXISTS(SELECT 1 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=s.current_config_ref WHERE s.task_key='topic_research' AND cfg.budget_mode='unconfigured')
      AND prior.core_ref IS NULL
      AND NOT EXISTS(SELECT 1 FROM linggan_ai_command c WHERE c.command_ref=md5('topic-auto:'||a.domain_ref::text||':'||core.request_ref::text||':'||$3)::uuid)
), ranked AS (
    SELECT *, row_number() OVER (PARTITION BY domain_ref,recent ORDER BY request_ref)
        + CASE WHEN recent THEN pending_recent ELSE pending_total-pending_recent END AS lane_position
    FROM eligible
    WHERE CASE WHEN recent
        THEN $1::uuid IS NULL OR (domain_ref,request_ref)>($1::uuid,$2::uuid)
        ELSE $7::uuid IS NULL OR (domain_ref,request_ref)>($7::uuid,$8::uuid)
    END
)
SELECT domain_ref,request_ref,work_ref,command_ref,recent
FROM ranked
ORDER BY lane_position,recent DESC,domain_ref,request_ref
LIMIT 8
