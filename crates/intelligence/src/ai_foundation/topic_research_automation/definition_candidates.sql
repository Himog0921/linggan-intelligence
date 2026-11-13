-- Only versions already used by a successful judgment can invalidate it.
-- New catalog topics never enter this dependency set, so discovery cannot trigger a rerun loop.
WITH latest AS (
 SELECT DISTINCT ON(r.domain_ref,w.work_ref,w.input_hash)
  r.domain_ref,w.window_ref,w.work_ref,w.input_hash,w.input_refs,result.core_json
 FROM linggan_topic_research_result result
 JOIN linggan_topic_research_window w USING(window_ref)
 JOIN linggan_topic_research_run r USING(run_ref)
 JOIN linggan_topic_research_automation a ON a.domain_ref=r.domain_ref AND a.enabled
 JOIN observation_domain d ON d.domain_ref=r.domain_ref AND d.status='active'
 WHERE w.input_refs->>'inputContract'='topic-map.core-source.v1'
  AND w.input_refs->>'methodVersion'=$1
  AND NOT EXISTS(SELECT 1 FROM linggan_ai_control c WHERE c.scope_key IN('global','executor','task:topic_research') AND NOT c.enabled)
  AND w.input_refs#>>'{coverage,coreRef}'=(SELECT request_ref::text FROM linggan_content_understanding_result WHERE work_ref=w.work_ref AND result IS NOT NULL ORDER BY created_at DESC,request_ref DESC LIMIT 1)
 ORDER BY r.domain_ref,w.work_ref,w.input_hash,result.created_at DESC,result.result_ref DESC
), changed AS (
 SELECT l.*,changes.versions,
  md5('topic-definition-repair:'||l.window_ref::text||':'||changes.versions::text)::uuid AS command_ref
 FROM latest l
 CROSS JOIN LATERAL (
  SELECT jsonb_agg(jsonb_build_object('previous',dep.ref,'current',cur.definition_ref,'structure',ss.request_ref)
    ORDER BY dep.ref) AS versions
  FROM (
   SELECT DISTINCT ref FROM jsonb_array_elements(l.core_json->'units') unit
   CROSS JOIN LATERAL (
    SELECT value::uuid AS ref FROM jsonb_array_elements_text(COALESCE(unit->'comparedDefinitionRefs','[]'))
    UNION SELECT (value->>'definitionRef')::uuid FROM jsonb_array_elements(COALESCE(unit->'assignments','[]'))
    UNION SELECT (value->>'definitionRef')::uuid FROM jsonb_array_elements(COALESCE(unit->'relations','[]'))
    UNION SELECT (unit#>>'{hierarchy,parentDefinitionRef}')::uuid
   ) refs WHERE ref IS NOT NULL
  ) dep
  JOIN linggan_topic_definition old ON old.definition_ref=dep.ref
  JOIN LATERAL(SELECT definition_ref FROM linggan_topic_definition WHERE topic_ref=old.topic_ref ORDER BY version DESC LIMIT 1)cur ON true
  LEFT JOIN linggan_topic_map_structure_source ss ON ss.topic_ref=old.topic_ref
  WHERE cur.definition_ref<>dep.ref OR ss.topic_ref IS NOT NULL
 ) changes
 WHERE changes.versions IS NOT NULL
)
SELECT * FROM changed c
WHERE ($2::uuid IS NULL OR (c.domain_ref,c.window_ref)>($2::uuid,$3::uuid))
 AND NOT EXISTS(SELECT 1 FROM linggan_ai_command cmd WHERE cmd.command_ref=c.command_ref)
 AND NOT EXISTS(SELECT 1 FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref)
  JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item_control ctl USING(item_ref)
  WHERE r.domain_ref=c.domain_ref AND w.input_hash=c.input_hash AND ctl.state IN('pending','running'))
ORDER BY c.domain_ref,c.window_ref LIMIT 8
