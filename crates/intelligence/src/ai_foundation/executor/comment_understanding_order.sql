-- Research units and consolidation share one domain catalog.
AND (
 b.task_key NOT IN ('comment_understanding','comment_problem_consolidation')
 OR NOT EXISTS (
  SELECT 1 FROM linggan_ai_batch_item_control other_control
  JOIN linggan_ai_batch_item other_item USING(item_ref)
  LEFT JOIN linggan_comment_research_unit other_unit ON other_unit.item_ref=other_item.item_ref
  LEFT JOIN linggan_comment_research_unit current_unit ON current_unit.item_ref=item.item_ref
  WHERE other_control.state='running' AND other_item.item_ref<>item.item_ref
    AND other_item.input_ref->>'kind' IN ('comment_research_unit','comment_problem_consolidation')
    AND COALESCE(other_unit.domain_ref::text,other_item.input_ref->>'domainRef') =
        COALESCE(current_unit.domain_ref::text,item.input_ref->>'domainRef')
 )
)
