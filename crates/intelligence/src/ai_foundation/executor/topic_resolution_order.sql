-- A new fused call and an old resolve/review pair share the same domain assignment slot.
-- Historical reviews remain claimable, allowing the old pair to settle.
AND NOT EXISTS (
 SELECT 1 FROM linggan_topic_research_stage current_stage
 JOIN linggan_topic_research_window current_window USING(window_ref)
 JOIN linggan_topic_research_run current_run USING(run_ref)
 WHERE current_stage.item_ref=item.item_ref
 AND (current_stage.stage_number=2 OR
      (current_stage.stage_number=0 AND current_window.input_refs->>'executionMode'='single_pass'))
 AND EXISTS (
  SELECT 1 FROM linggan_topic_research_window other_window
  JOIN linggan_topic_research_run other_run USING(run_ref)
  JOIN linggan_topic_research_stage resolve_stage ON resolve_stage.window_ref=other_window.window_ref
   AND resolve_stage.stage_number=CASE WHEN other_window.input_refs->>'executionMode'='single_pass' THEN 0 ELSE 2 END
  JOIN linggan_ai_batch_item_control resolve_control ON resolve_control.item_ref=resolve_stage.item_ref
  LEFT JOIN linggan_topic_research_stage review_stage ON review_stage.window_ref=other_window.window_ref AND review_stage.stage_number=3
  LEFT JOIN linggan_ai_batch_item_control review_control ON review_control.item_ref=review_stage.item_ref
  WHERE other_run.domain_ref=current_run.domain_ref AND other_window.window_ref<>current_window.window_ref
  AND (resolve_control.state='running' OR review_control.state='running'
    OR (resolve_control.state='succeeded' AND review_control.state='pending'))
 )
)
