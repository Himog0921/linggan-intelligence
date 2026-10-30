-- Enable the dedicated title task with the same frozen-input, source and budget gates as writing.
ALTER TABLE linggan_creation_adoption DROP CONSTRAINT linggan_creation_adoption_action_check;
ALTER TABLE linggan_creation_adoption ADD CONSTRAINT creation_adoption_action_check CHECK (action IN ('draft','rewrite_selection','page_copy','title_ideas'));
ALTER TABLE linggan_creation_ai_preview DROP CONSTRAINT linggan_creation_ai_preview_action_check;
ALTER TABLE linggan_creation_ai_preview ADD CONSTRAINT creation_ai_preview_action_check CHECK (action IN ('draft','rewrite_selection','page_copy','title_ideas'));
ALTER TABLE linggan_creation_ai_preview DROP CONSTRAINT creation_ai_preview_method_history;
ALTER TABLE linggan_creation_ai_preview ADD CONSTRAINT creation_ai_preview_method_history CHECK (method_version IN ('content-workbench-writing.v1','content-workbench-writing.v2','creation-title.v1'));
ALTER TABLE linggan_creation_writing_qualification DROP CONSTRAINT creation_writing_qualification_method_history;
ALTER TABLE linggan_creation_writing_qualification ADD CONSTRAINT creation_writing_qualification_method_history CHECK (method_version IN ('content-workbench-writing.v1','content-workbench-writing.v2','creation-title.v1'));
CREATE OR REPLACE FUNCTION linggan_guard_dossier_writing_method() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.method_version NOT IN ('content-workbench-writing.v2','creation-title.v1') THEN RAISE EXCEPTION 'ai_dossier_writing_contract_required' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE OR REPLACE FUNCTION linggan_guard_ai_writing_preview() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.task_config_ref IS NULL OR NOT EXISTS(SELECT 1 FROM linggan_ai_task_config c WHERE c.task_config_ref=NEW.task_config_ref AND c.model_config_ref=NEW.config_ref AND c.execution_contract=NEW.method_version AND ((NEW.action='title_ideas' AND c.task_key='creation_title' AND NEW.method_version='creation-title.v1') OR (NEW.action<>'title_ideas' AND c.task_key='writing_legacy' AND NEW.method_version='content-workbench-writing.v2')))
 THEN RAISE EXCEPTION 'ai_writing_preview_configuration_missing'; END IF;
 RETURN NEW;
END $$;
CREATE OR REPLACE FUNCTION linggan_guard_ai_writing_request() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner uuid; generation bigint;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ai_writing_request_delete_forbidden'; END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.state<>'queued' OR NEW.invocation_ref IS NOT NULL OR NEW.task_config_ref IS NULL
   OR NOT EXISTS(SELECT 1 FROM linggan_creation_ai_preview p
    JOIN linggan_ai_batch b ON b.task_config_ref=p.task_config_ref
    JOIN linggan_ai_batch_item item USING(batch_ref)
    WHERE p.preview_ref=NEW.preview_ref AND p.task_config_ref=NEW.task_config_ref
     AND b.batch_ref=NEW.batch_ref AND item.item_ref=NEW.item_ref AND b.task_key IN ('writing_legacy','creation_title') AND b.task_key=(SELECT task_key FROM linggan_ai_task_config WHERE task_config_ref=p.task_config_ref)
     AND b.kind='interactive' AND b.item_count=1 AND item.position=0
     AND item.input_ref->>'kind'='writing_request' AND item.input_ref->>'requestRef'=NEW.request_ref::text
     AND NEW.approved_budget_tokens>=p.reserved_tokens)
  THEN RAISE EXCEPTION 'ai_writing_queue_binding_invalid'; END IF;
 ELSE
  IF OLD.state IN ('succeeded','failed','cancelled') THEN RAISE EXCEPTION 'ai_writing_terminal_immutable'; END IF;
  IF ROW(NEW.request_ref,NEW.command_hash,NEW.preview_ref,NEW.task_config_ref,NEW.batch_ref,NEW.item_ref,NEW.approved_budget_tokens,NEW.created_at)
    IS DISTINCT FROM ROW(OLD.request_ref,OLD.command_hash,OLD.preview_ref,OLD.task_config_ref,OLD.batch_ref,OLD.item_ref,OLD.approved_budget_tokens,OLD.created_at)
   OR (OLD.invocation_ref IS NOT NULL AND NEW.invocation_ref IS DISTINCT FROM OLD.invocation_ref)
   OR (OLD.dispatch_started AND NOT NEW.dispatch_started)
   OR (OLD.state='cancel_requested' AND NEW.state NOT IN ('cancel_requested','cancelled','failed'))
  THEN RAISE EXCEPTION 'ai_writing_request_identity_immutable'; END IF;
  IF NEW.invocation_ref IS DISTINCT FROM OLD.invocation_ref OR NEW.state='succeeded'
   OR (OLD.invocation_ref IS NOT NULL AND NEW.state<>'cancel_requested') THEN
   owner=NULLIF(current_setting('linggan.ai_executor_instance',true),'')::uuid;
   generation=NULLIF(current_setting('linggan.ai_executor_generation',true),'')::bigint;
   IF owner IS NULL OR generation IS NULL OR NOT linggan_ai_executor_is_owner(owner,generation,false)
   THEN RAISE EXCEPTION 'ai_writing_executor_fence_missing'; END IF;
  END IF;
 END IF;
 IF NEW.state<>'succeeded' AND NEW.suggestion IS NOT NULL THEN RAISE EXCEPTION 'ai_writing_unpublished_suggestion'; END IF;
 IF NEW.invocation_ref IS NOT NULL AND NEW.task_config_ref IS NOT NULL AND NOT EXISTS(
  SELECT 1 FROM linggan_model_invocation i WHERE i.invocation_ref=NEW.invocation_ref
   AND i.task_key=(SELECT task_key FROM linggan_ai_task_config WHERE task_config_ref=NEW.task_config_ref) AND i.task_config_ref=NEW.task_config_ref AND i.batch_ref=NEW.batch_ref)
 THEN RAISE EXCEPTION 'ai_writing_invocation_binding_invalid'; END IF;
 IF NEW.state='succeeded' AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item_control c
   JOIN linggan_model_invocation i USING(invocation_ref) WHERE c.item_ref=NEW.item_ref
    AND c.state='running' AND c.lease_instance_ref=owner AND c.lease_instance_generation=generation
    AND i.state='succeeded' AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_control b WHERE b.batch_ref=NEW.batch_ref AND b.cancel_requested))
 THEN RAISE EXCEPTION 'ai_writing_success_receipt_missing'; END IF;
 IF NEW.state IN ('queued','running','cancel_requested') AND NEW.finished_at IS NOT NULL
  OR NEW.state IN ('succeeded','failed','cancelled') AND NEW.finished_at IS NULL
 THEN RAISE EXCEPTION 'ai_writing_completion_time_invalid'; END IF;
 RETURN NEW;
END $$;

ALTER TABLE linggan_ai_batch DROP CONSTRAINT ai_batch_task_kind_valid;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe') OR (task_key IN('writing_legacy','creation_title','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe')
 OR (task_key IN('work_comparison','creator_research','creator_account_research','comment_study','comment_problem_resolution','comment_problem_pair_review','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND kind='interactive'));
ALTER TABLE linggan_model_egress_event DROP CONSTRAINT ai_egress_material_task_valid;
ALTER TABLE linggan_model_egress_event ADD CONSTRAINT ai_egress_material_task_valid CHECK(
 (task_key='model_probe' AND material_class IN('synthetic_probe','model_catalog'))
 OR (task_key IN('note_interpretation','direction_list_draft','direction_proposal','work_comparison','creator_research') AND material_class='note_material')
 OR (task_key IN('comment_study','comment_problem_resolution','comment_problem_pair_review','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review') AND material_class IN('note_material','comment_material'))
 OR (task_key='creator_account_research' AND material_class IN('note_material','public_author_bio'))
 OR (task_key='author_org_identity' AND material_class='public_author_bio')
 OR (task_key IN('writing_assistant','writing_legacy','creation_title') AND material_class='selected_writing_material')
 OR (task_key='ocr_quality_audit' AND material_class IN('synthetic_image_probe','ocr_quality_image'))
 OR (task_key='ocr_recognition' AND material_class='manual_ocr_image'));
UPDATE linggan_ai_task SET lifecycle='active' WHERE task_key='creation_title';
