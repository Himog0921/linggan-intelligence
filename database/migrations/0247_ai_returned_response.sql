-- A returned model response is durable before material qualification and publication.
-- The request and provider credentials are never stored here.
CREATE TABLE linggan_ai_returned_response (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation(invocation_ref),
 item_ref uuid NOT NULL REFERENCES linggan_ai_batch_item(item_ref),
 publication jsonb NOT NULL,
 response bytea NOT NULL,
 lease_token uuid NOT NULL,
 reconcile_after timestamptz,
 reconciled_at timestamptz,
 local_state text CHECK(local_state IN ('succeeded','failed','cancelled')),
 local_failure_code text,
 CHECK ((reconciled_at IS NULL)=(local_state IS NULL)),
 received_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

-- Research consumers see a locally reconciled outcome while the original terminal item
-- and invocation remain immutable evidence of what the stale executor knew.
CREATE VIEW linggan_ai_research_item_outcome AS
SELECT c.item_ref,c.invocation_ref,c.wait_reason,c.finished_at,
       COALESCE(r.local_state,c.state) AS state,
       CASE WHEN r.local_state IS NULL THEN c.failure_code ELSE r.local_failure_code END AS failure_code
FROM linggan_ai_batch_item_control c
LEFT JOIN linggan_ai_returned_response r ON r.invocation_ref=c.invocation_ref AND r.local_state IS NOT NULL;

CREATE OR REPLACE VIEW linggan_content_understanding_saved_core AS
SELECT o.request_ref,o.work_ref,o.material_snapshot_identity,o.method_version,o.result,
       COALESCE(saved.reconciled_at,o.created_at) AS created_at,(saved.local_state='succeeded') IS TRUE AS offline_recovery
FROM linggan_content_understanding_result o
LEFT JOIN linggan_ai_returned_response saved ON saved.invocation_ref=o.invocation_ref AND saved.reconciled_at IS NOT NULL
WHERE o.result IS NOT NULL
UNION ALL
SELECT request_ref,work_ref,material_snapshot_identity,method_version,result,created_at,true AS offline_recovery
FROM linggan_content_understanding_offline_recovery;

-- A complete measured receipt replaces its original reservation in operational quota.
-- Partial usage and unrelated historical unknowns retain their original held quota.
CREATE VIEW linggan_ai_budget_usage AS
SELECT i.invocation_ref,i.task_key,i.budget_day,i.state,i.cost_pico_usd,
       CASE WHEN u.invocation_ref IS NULL THEN i.usage_state ELSE 'measured' END AS usage_state,
       COALESCE(u.input_tokens,i.input_tokens) AS input_tokens,
       COALESCE(u.output_tokens,i.output_tokens) AS output_tokens,
       CASE WHEN u.invocation_ref IS NULL THEN i.cached_input_tokens ELSE u.cached_input_tokens END AS cached_input_tokens,
       CASE WHEN u.invocation_ref IS NULL THEN i.cached_write_tokens ELSE u.cached_write_tokens END AS cached_write_tokens,
       CASE WHEN u.invocation_ref IS NULL THEN i.reasoning_tokens ELSE u.reasoning_tokens END AS reasoning_tokens,
       COALESCE(u.input_tokens+u.output_tokens,i.measured_tokens) AS measured_tokens,
       COALESCE(u.input_tokens+u.output_tokens,i.charged_tokens) AS charged_tokens,
       u.invocation_ref IS NOT NULL AS quota_reconciled
FROM linggan_model_invocation i
LEFT JOIN (linggan_ai_late_usage_receipt u JOIN linggan_ai_returned_response r USING(invocation_ref,item_ref))
 ON u.invocation_ref=i.invocation_ref AND i.usage_state='unknown_reserved' AND i.failure_code='ai_dispatch_unknown'
 AND r.reconciled_at IS NOT NULL AND u.input_tokens IS NOT NULL AND u.output_tokens IS NOT NULL;

CREATE OR REPLACE FUNCTION linggan_guard_ai_executor_item() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE actor uuid; actor_generation bigint; recovery boolean;
BEGIN
 IF OLD.state<>'running' AND NEW.state<>'running' THEN RETURN NEW; END IF;
 actor=NULLIF(current_setting('linggan.ai_executor_instance',true),'')::uuid;
 actor_generation=NULLIF(current_setting('linggan.ai_executor_generation',true),'')::bigint;
 recovery=COALESCE(current_setting('linggan.ai_executor_recovery',true),'')='true';
 IF actor IS NULL OR actor_generation IS NULL
  OR NOT linggan_ai_executor_is_owner(actor,actor_generation,OLD.state='pending')
 THEN RAISE EXCEPTION 'ai_executor_fence_lost'; END IF;
 IF NEW.state='running' AND (NEW.lease_instance_ref,NEW.lease_instance_generation) IS DISTINCT FROM (actor,actor_generation)
 THEN RAISE EXCEPTION 'ai_item_executor_mismatch'; END IF;
 IF OLD.state='running' AND ((OLD.lease_instance_ref,OLD.lease_instance_generation) IS DISTINCT FROM (actor,actor_generation)
   OR OLD.lease_expires_at<=clock_timestamp()) THEN
  -- An already-returned response may be claimed for local publication only.
  -- No transition to a dispatchable pending state is introduced.
  IF NEW.state='running' AND recovery AND OLD.attempt_count>0
     AND NEW.invocation_ref=OLD.invocation_ref AND NEW.attempt_count=OLD.attempt_count
     AND NEW.lease_generation=OLD.lease_generation AND NEW.lease_token=OLD.lease_token
     AND EXISTS(SELECT 1 FROM linggan_ai_returned_response r
                WHERE r.item_ref=OLD.item_ref AND r.invocation_ref=OLD.invocation_ref)
     AND (OLD.lease_expires_at<=clock_timestamp() OR NOT linggan_ai_executor_is_owner(OLD.lease_instance_ref,OLD.lease_instance_generation,false))
  THEN RETURN NEW; END IF;
  IF NOT (NEW.state IN ('succeeded','failed','cancelled')
      AND OLD.lease_instance_ref=actor AND OLD.lease_instance_generation=actor_generation
      AND current_setting('linggan.ai_publication_token',true)=OLD.lease_token::text
      AND EXISTS(SELECT 1 FROM linggan_ai_returned_response r
                 WHERE r.item_ref=OLD.item_ref AND r.invocation_ref=OLD.invocation_ref)) IS TRUE THEN
  IF NOT recovery OR NEW.state NOT IN ('pending','failed','cancelled') OR NEW.output IS NOT NULL
   OR (OLD.lease_expires_at>clock_timestamp() AND linggan_ai_executor_is_owner(OLD.lease_instance_ref,OLD.lease_instance_generation,false))
  THEN RAISE EXCEPTION 'ai_item_stale_owner'; END IF;
  END IF;
  IF NEW.state='pending' AND OLD.attempt_count>0 THEN RAISE EXCEPTION 'ai_item_ambiguous_retry'; END IF;
 END IF;
 IF NEW.state='succeeded' AND EXISTS(SELECT 1 FROM linggan_ai_batch_item item
  JOIN linggan_ai_batch_control b USING(batch_ref) WHERE item.item_ref=NEW.item_ref AND b.cancel_requested)
 THEN RAISE EXCEPTION 'ai_cancelled_output_publication'; END IF;
 RETURN NEW;
END $$;
