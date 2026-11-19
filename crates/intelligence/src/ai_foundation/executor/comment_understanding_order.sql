-- One research unit per domain at a time: every unit reads the catalog the previous unit wrote, so
-- a Problem a unit creates can be matched by the next one instead of being created again.
AND NOT EXISTS (
 SELECT 1 FROM linggan_comment_research_unit current_unit
 JOIN linggan_comment_research_unit other_unit
  ON other_unit.domain_ref=current_unit.domain_ref AND other_unit.item_ref<>current_unit.item_ref
 JOIN linggan_ai_batch_item_control other_control ON other_control.item_ref=other_unit.item_ref
 WHERE current_unit.item_ref=item.item_ref AND other_control.state='running'
)
