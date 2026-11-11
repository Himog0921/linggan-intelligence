SELECT EXISTS (
 SELECT 1 FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref)
 WHERE w.window_ref=$1 AND r.domain_ref=$2 AND w.input_refs->>'semanticRequestRef'=$3
 AND w.window_ref=(SELECT latest.window_ref FROM linggan_topic_research_window latest JOIN linggan_topic_research_run run USING(run_ref) WHERE run.domain_ref=$2 AND latest.input_refs->>'semanticRequestRef'=$3 ORDER BY run.created_at DESC,run.run_ref DESC LIMIT 1)
 AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_result result WHERE result.window_ref=w.window_ref)
 AND EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.window_ref=w.window_ref AND c.state='failed')
 AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE s.window_ref=w.window_ref AND (c.state IN('pending','running') OR c.failure_code='ai_dispatch_unknown'))
)
