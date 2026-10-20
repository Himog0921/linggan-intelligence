-- Explicit writing becomes one queued item. Historical results stay readable; an old
-- in-flight request requires the deployment drain/preflight, never a speculative resend.
INSERT INTO linggan_ai_failure_code VALUES
 ('ai_cancelled_after_dispatch',false,false,'调用接纳后取消；已知或未知用量仍须结算','executor_v1'),
 ('ai_input_limit_before_dispatch',false,false,'固定输入超过所选能力上限，未派发','executor_v1');
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM linggan_creation_ai_request WHERE state IN ('running','cancel_requested'))
 THEN RAISE EXCEPTION 'ai_writing_bridge_preflight_required: inspect request_ref,invocation_ref,state,dispatch_started and drain old writing before migration'; END IF;
END $$;
ALTER TABLE linggan_creation_ai_preview ADD COLUMN task_config_ref uuid REFERENCES linggan_ai_task_config;
ALTER TABLE linggan_creation_ai_request
 ALTER COLUMN invocation_ref DROP NOT NULL,
 DROP CONSTRAINT linggan_creation_ai_request_state_check,
 ADD CONSTRAINT creation_ai_queue_state CHECK(state IN ('queued','running','succeeded','failed','cancel_requested','cancelled')),
 ADD COLUMN task_config_ref uuid REFERENCES linggan_ai_task_config,
 ADD COLUMN batch_ref uuid UNIQUE REFERENCES linggan_ai_batch,
 ADD COLUMN item_ref uuid UNIQUE REFERENCES linggan_ai_batch_item,
 ADD COLUMN approved_budget_tokens bigint CHECK(approved_budget_tokens>=0),
 ADD CONSTRAINT creation_ai_queue_binding CHECK(
  (task_config_ref IS NULL AND batch_ref IS NULL AND item_ref IS NULL AND approved_budget_tokens IS NULL AND invocation_ref IS NOT NULL)
  OR (task_config_ref IS NOT NULL AND batch_ref IS NOT NULL AND item_ref IS NOT NULL AND approved_budget_tokens IS NOT NULL)),
 ADD CONSTRAINT creation_ai_queue_invocation CHECK(invocation_ref IS NOT NULL OR state IN ('queued','cancel_requested','cancelled','failed')),
 ADD CONSTRAINT creation_ai_queued_not_dispatched CHECK(state<>'queued' OR (invocation_ref IS NULL AND NOT dispatch_started)),
 ADD CONSTRAINT creation_ai_queue_running CHECK(state<>'running' OR invocation_ref IS NOT NULL);

CREATE FUNCTION linggan_guard_ai_writing_preview() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.task_config_ref IS NULL OR NOT EXISTS(SELECT 1 FROM linggan_ai_task_config c
  WHERE c.task_config_ref=NEW.task_config_ref AND c.task_key='writing_legacy' AND c.model_config_ref=NEW.config_ref
   AND c.execution_contract=NEW.method_version)
 THEN RAISE EXCEPTION 'ai_writing_preview_configuration_missing'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_writing_preview_configuration BEFORE INSERT ON linggan_creation_ai_preview
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_writing_preview();

CREATE FUNCTION linggan_guard_ai_writing_request() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner uuid; generation bigint;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ai_writing_request_delete_forbidden'; END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.state<>'queued' OR NEW.invocation_ref IS NOT NULL OR NEW.task_config_ref IS NULL
   OR NOT EXISTS(SELECT 1 FROM linggan_creation_ai_preview p
    JOIN linggan_ai_batch b ON b.task_config_ref=p.task_config_ref
    JOIN linggan_ai_batch_item item USING(batch_ref)
    WHERE p.preview_ref=NEW.preview_ref AND p.task_config_ref=NEW.task_config_ref
     AND b.batch_ref=NEW.batch_ref AND item.item_ref=NEW.item_ref AND b.task_key='writing_legacy'
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
   AND i.task_key='writing_legacy' AND i.task_config_ref=NEW.task_config_ref AND i.batch_ref=NEW.batch_ref)
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
CREATE TRIGGER ai_writing_request_guard BEFORE INSERT OR UPDATE OR DELETE ON linggan_creation_ai_request
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_writing_request();

-- Recovery and unattempted cancellation must settle the public request in the same transaction.
-- Success is published by the writing handler before the item terminal write, never by a trigger.
CREATE FUNCTION linggan_settle_ai_writing_item() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.state='succeeded' AND EXISTS(SELECT 1 FROM linggan_creation_ai_request r
  WHERE r.item_ref=NEW.item_ref AND r.state<>'succeeded') THEN RAISE EXCEPTION 'ai_writing_publication_missing'; END IF;
 IF NEW.state IN ('failed','cancelled','skipped') THEN
  UPDATE linggan_creation_ai_request SET
   state=CASE WHEN state='cancel_requested' OR NEW.state='cancelled' THEN 'cancelled' ELSE 'failed' END,
   failure_code=NEW.failure_code,finished_at=NEW.finished_at
  WHERE item_ref=NEW.item_ref AND state IN ('queued','running','cancel_requested');
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER ai_writing_item_terminal AFTER UPDATE ON linggan_ai_batch_item_control
 FOR EACH ROW WHEN (NEW.state IN ('succeeded','failed','skipped','cancelled'))
 EXECUTE FUNCTION linggan_settle_ai_writing_item();
