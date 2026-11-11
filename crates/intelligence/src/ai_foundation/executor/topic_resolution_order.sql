-- Serialize only the assignment portion of a domain, without holding a network-time lock.
-- Reviews remain claimable so existing assignments always have a path to settlement.
AND NOT EXISTS (
 SELECT 1
 FROM linggan_topic_research_stage current_stage
 JOIN linggan_topic_research_window current_window USING(window_ref)
 JOIN linggan_topic_research_run current_run USING(run_ref)
 WHERE current_stage.item_ref=item.item_ref AND current_stage.stage_number=2
 AND EXISTS (
  SELECT 1
  FROM linggan_topic_research_window other_window
  JOIN linggan_topic_research_run other_run USING(run_ref)
  JOIN linggan_topic_research_stage resolve_stage
    ON resolve_stage.window_ref=other_window.window_ref AND resolve_stage.stage_number=2
  JOIN linggan_ai_batch_item_control resolve_control ON resolve_control.item_ref=resolve_stage.item_ref
  JOIN linggan_topic_research_stage review_stage
    ON review_stage.window_ref=other_window.window_ref AND review_stage.stage_number=3
  JOIN linggan_ai_batch_item_control review_control ON review_control.item_ref=review_stage.item_ref
  WHERE other_run.domain_ref=current_run.domain_ref AND other_window.window_ref<>current_window.window_ref
  AND (resolve_control.state='running' OR review_control.state='running'
    OR (resolve_control.state='succeeded' AND review_control.state='pending'))
 )
)
