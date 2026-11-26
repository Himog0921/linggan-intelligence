SELECT r.domain_ref, w.work_ref, w.window_ref,
       (w.input_refs#>>'{coverage,coreRef}')::uuid AS core_ref,
       md5('topic-auto-retry:'||w.window_ref::text)::uuid AS command_ref
FROM linggan_topic_research_window w
JOIN linggan_topic_research_run r USING(run_ref)
JOIN linggan_topic_research_automation a ON a.domain_ref=r.domain_ref AND a.enabled
JOIN observation_domain d ON d.domain_ref=r.domain_ref AND d.status='active'
JOIN linggan_topic_research_stage s ON s.window_ref=w.window_ref AND s.stage_number=0
JOIN linggan_ai_batch_item i USING(item_ref)
JOIN linggan_ai_batch b USING(batch_ref)
JOIN linggan_ai_batch_item_control c USING(item_ref)
JOIN linggan_topic_research_stage_result receipt USING(item_ref)
JOIN linggan_model_invocation invocation ON invocation.invocation_ref=receipt.invocation_ref
JOIN LATERAL (
    SELECT request_ref FROM linggan_content_understanding_saved_core
    WHERE work_ref=w.work_ref AND result IS NOT NULL
    ORDER BY created_at DESC,request_ref DESC LIMIT 1
) current_core ON current_core.request_ref::text=w.input_refs#>>'{coverage,coreRef}'
WHERE ($1::uuid IS NULL OR w.window_ref=$1)
  AND ($3::uuid IS NULL OR w.window_ref>$3)
  AND i.input_ref->>'collectionRunRef' IS NOT DISTINCT FROM $4::uuid::text
  AND ($5::uuid IS NULL OR (r.domain_ref=$5 AND w.work_ref=ANY($6::uuid[])))
  AND w.input_refs->>'inputContract'='topic-map.core-source.v1'
  AND w.input_refs->>'methodVersion'=$2
  AND w.input_refs->>'executionMode'='single_pass'
  AND b.kind='automatic'
  AND c.state='failed' AND c.failure_code='ai_output_invalid'
  AND c.invocation_ref=receipt.invocation_ref AND receipt.failure_code='ai_output_invalid'
  AND invocation.state='failed' AND invocation.failure_code='ai_output_invalid'
  AND invocation.dispatch_state='started'
  AND c.finished_at<=scope_001_now()-interval '1 minute'
  AND NOT EXISTS (
      SELECT 1 FROM linggan_ai_control WHERE scope_key IN
          ('global','executor','task:topic_research') AND NOT enabled
  )
  AND NOT EXISTS (
      SELECT 1 FROM linggan_topic_research_window other
      JOIN linggan_topic_research_run owner USING(run_ref)
      WHERE owner.domain_ref=r.domain_ref AND other.window_ref<>w.window_ref
        AND other.work_ref=w.work_ref
        AND other.input_refs#>>'{coverage,coreRef}'=w.input_refs#>>'{coverage,coreRef}'
        AND other.input_refs->>'methodVersion'=$2
  )
ORDER BY w.window_ref
LIMIT 4
