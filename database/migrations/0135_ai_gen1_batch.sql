-- Frozen input references/configurations and independently fenced execution facts.
CREATE TABLE linggan_ai_batch (
 batch_ref uuid PRIMARY KEY,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 task_config_ref uuid NOT NULL,
 kind text NOT NULL CHECK(kind IN ('trial','automatic','backfill','interactive','probe')),
 CHECK(kind<>'trial' OR item_count<=50),
 CHECK((task_key='model_probe' AND kind='probe')
  OR (task_key IN ('writing_legacy','writing_assistant','direction_list_draft','direction_proposal') AND kind IN ('interactive','trial'))
  OR (task_key IN ('note_interpretation','author_org_identity') AND kind<>'probe')),
 inputs_sha256 text NOT NULL CHECK(inputs_sha256 ~ '^[0-9a-f]{64}$'),
 item_count integer NOT NULL CHECK(item_count BETWEEN 1 AND 10000),
 command_ref uuid NOT NULL REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 ingest_transaction xid8 NOT NULL DEFAULT pg_current_xact_id(),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(task_config_ref,task_key) REFERENCES linggan_ai_task_config(task_config_ref,task_key),
 UNIQUE(batch_ref,task_key)
);
CREATE TABLE linggan_ai_batch_control (
 batch_ref uuid PRIMARY KEY REFERENCES linggan_ai_batch,
 state text NOT NULL DEFAULT 'open' CHECK(state IN ('open','completed','cancelled','failed')),
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 cancel_requested boolean NOT NULL DEFAULT false,
 last_round_no integer NOT NULL DEFAULT 0 CHECK(last_round_no>=0),
 stop_reason text,
 finished_at timestamptz,
 CHECK((state='open')=(finished_at IS NULL))
);
CREATE TABLE linggan_ai_batch_item (
 item_ref uuid PRIMARY KEY,
 batch_ref uuid NOT NULL REFERENCES linggan_ai_batch,
 position integer NOT NULL CHECK(position>=0),
 item_key text NOT NULL CHECK(char_length(item_key) BETWEEN 1 AND 200),
 input_ref jsonb NOT NULL CHECK(jsonb_typeof(input_ref)='object' AND octet_length(input_ref::text)<=4096),
 UNIQUE(batch_ref,position), UNIQUE(batch_ref,item_key)
);
CREATE TABLE linggan_ai_batch_item_control (
 item_ref uuid PRIMARY KEY REFERENCES linggan_ai_batch_item,
 state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','running','succeeded','failed','skipped','cancelled')),
 lease_generation bigint NOT NULL DEFAULT 0 CHECK(lease_generation>=0),
 lease_token uuid,
 lease_expires_at timestamptz,
 attempt_count integer NOT NULL DEFAULT 0 CHECK(attempt_count BETWEEN 0 AND 3),
 invocation_ref uuid UNIQUE REFERENCES linggan_model_invocation,
 next_attempt_at timestamptz NOT NULL DEFAULT scope_001_now(),
 wait_reason text,
 executed_config_ref uuid REFERENCES linggan_ai_task_config,
 executed_skill_version_ref uuid REFERENCES linggan_skill_version,
 execution_contract text,
 failure_code text,
 counts_as_failure boolean NOT NULL DEFAULT false,
 usable boolean,
 output jsonb CHECK(output IS NULL OR octet_length(output::text)<=8388608),
 started_at timestamptz,
 finished_at timestamptz,
 CHECK((state='running' AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL)
  OR (state<>'running' AND lease_token IS NULL AND lease_expires_at IS NULL)),
 CHECK((state IN ('pending','running'))=(finished_at IS NULL)),
 CHECK(state NOT IN ('pending','running') OR usable IS NULL),
 CHECK(state IN ('pending','running') OR usable IS NOT NULL),
 CHECK(state<>'succeeded' OR (output IS NOT NULL AND attempt_count>0)),
 CHECK(usable IS DISTINCT FROM true OR state='succeeded'),
 CHECK(NOT counts_as_failure OR (state='failed' AND attempt_count>0)),
 CHECK(attempt_count=0 OR invocation_ref IS NOT NULL),
 CHECK(attempt_count=0 OR (executed_config_ref IS NOT NULL AND execution_contract IS NOT NULL)),
 CHECK(state<>'skipped' OR attempt_count=0)
);
CREATE INDEX ai_batch_claim ON linggan_ai_batch_item_control(state,lease_expires_at,item_ref)
 WHERE state IN ('pending','running');
CREATE INDEX ai_batch_items ON linggan_ai_batch_item(batch_ref,position);
CREATE INDEX ai_batch_task_created ON linggan_ai_batch(task_key,created_at DESC);
CREATE FUNCTION linggan_initialize_ai_batch() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN INSERT INTO linggan_ai_batch_control(batch_ref) VALUES(NEW.batch_ref); RETURN NULL; END $$;
CREATE TRIGGER ai_batch_initialize AFTER INSERT ON linggan_ai_batch FOR EACH ROW EXECUTE FUNCTION linggan_initialize_ai_batch();
CREATE FUNCTION linggan_initialize_ai_batch_item() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_batch WHERE batch_ref=NEW.batch_ref AND ingest_transaction=pg_current_xact_id())
 THEN RAISE EXCEPTION 'ai_batch_inputs_sealed'; END IF;
 INSERT INTO linggan_ai_batch_item_control(item_ref) VALUES(NEW.item_ref);
 RETURN NULL;
END $$;
CREATE TRIGGER ai_batch_item_initialize AFTER INSERT ON linggan_ai_batch_item FOR EACH ROW EXECUTE FUNCTION linggan_initialize_ai_batch_item();
CREATE FUNCTION linggan_complete_ai_batch_inputs() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE total bigint; first_position integer; last_position integer;
BEGIN
 SELECT count(*),min(position),max(position) INTO total,first_position,last_position
 FROM linggan_ai_batch_item WHERE batch_ref=NEW.batch_ref;
 IF total<>NEW.item_count OR first_position<>0 OR last_position<>NEW.item_count-1
 THEN RAISE EXCEPTION 'ai_batch_inputs_incomplete'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER ai_batch_inputs_complete AFTER INSERT ON linggan_ai_batch
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION linggan_complete_ai_batch_inputs();
CREATE FUNCTION linggan_guard_ai_item_control() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR OLD.state IN ('succeeded','failed','skipped','cancelled')
 THEN RAISE EXCEPTION 'ai_item_terminal_immutable'; END IF;
 IF NEW.item_ref<>OLD.item_ref OR NEW.lease_generation<OLD.lease_generation
  OR NEW.attempt_count<OLD.attempt_count OR NEW.attempt_count>OLD.attempt_count+1
 THEN RAISE EXCEPTION 'ai_item_fence_invalid'; END IF;
 IF OLD.state='pending' AND NEW.state='running' AND NEW.lease_generation<>OLD.lease_generation+1
 THEN RAISE EXCEPTION 'ai_item_claim_generation_missing'; END IF;
 IF NEW.attempt_count>OLD.attempt_count AND NOT (OLD.state='running' AND NEW.state='running')
 THEN RAISE EXCEPTION 'ai_item_attempt_outside_lease'; END IF;
 IF NEW.attempt_count>OLD.attempt_count AND NOT EXISTS(
  SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch b USING(batch_ref)
  JOIN linggan_ai_task_config c ON c.task_config_ref=b.task_config_ref
  WHERE i.item_ref=NEW.item_ref AND c.task_config_ref=NEW.executed_config_ref
   AND c.skill_version_ref IS NOT DISTINCT FROM NEW.executed_skill_version_ref AND c.execution_contract=NEW.execution_contract)
 THEN RAISE EXCEPTION 'ai_item_execution_version_mismatch'; END IF;
 IF OLD.attempt_count>0 AND (NEW.executed_config_ref,NEW.executed_skill_version_ref,NEW.execution_contract)
  IS DISTINCT FROM (OLD.executed_config_ref,OLD.executed_skill_version_ref,OLD.execution_contract)
 THEN RAISE EXCEPTION 'ai_item_execution_stamp_immutable'; END IF;
 IF NEW.attempt_count>OLD.attempt_count AND EXISTS(
  SELECT 1 FROM linggan_ai_batch_item item JOIN linggan_model_invocation invocation ON invocation.invocation_ref=NEW.invocation_ref
  WHERE item.item_ref=NEW.item_ref AND item.input_ref->>'kind'='probe'
   AND invocation.operation IS DISTINCT FROM item.input_ref->>'operation')
 THEN RAISE EXCEPTION 'ai_item_probe_operation_mismatch'; END IF;
 IF NEW.state<>'running' AND NEW.invocation_ref IS NOT NULL AND EXISTS(
  SELECT 1 FROM linggan_model_invocation WHERE invocation_ref=NEW.invocation_ref AND (state='running' OR finished_at IS NULL))
 THEN RAISE EXCEPTION 'ai_item_invocation_unfinished'; END IF;
 IF NEW.state='succeeded' AND NOT EXISTS(SELECT 1 FROM linggan_model_invocation WHERE invocation_ref=NEW.invocation_ref AND state='succeeded')
 THEN RAISE EXCEPTION 'ai_item_success_receipt_missing'; END IF;
 IF NEW.lease_generation<>OLD.lease_generation AND NOT (
  NEW.lease_generation=OLD.lease_generation+1 AND NEW.state='running' AND OLD.state='pending'
  AND NEW.lease_token IS NOT NULL AND NEW.lease_token IS DISTINCT FROM OLD.lease_token)
 THEN RAISE EXCEPTION 'ai_item_lease_invalid'; END IF;
 IF OLD.state='running' AND NEW.state='running' AND NEW.lease_token IS DISTINCT FROM OLD.lease_token
 THEN RAISE EXCEPTION 'ai_item_lease_replaced'; END IF;
 IF OLD.state='running' AND NEW.state='running' AND OLD.invocation_ref IS NOT NULL
  AND NEW.invocation_ref IS DISTINCT FROM OLD.invocation_ref
  AND EXISTS(SELECT 1 FROM linggan_model_invocation WHERE invocation_ref=OLD.invocation_ref AND state='running')
 THEN RAISE EXCEPTION 'ai_item_invocation_in_flight'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_item_control_guard BEFORE UPDATE OR DELETE ON linggan_ai_batch_item_control FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_item_control();
CREATE TABLE linggan_ai_trial_comparison (
 comparison_ref uuid PRIMARY KEY REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 baseline_batch_ref uuid,
 candidate_batch_ref uuid NOT NULL UNIQUE,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(baseline_batch_ref,task_key) REFERENCES linggan_ai_batch(batch_ref,task_key),
 FOREIGN KEY(candidate_batch_ref,task_key) REFERENCES linggan_ai_batch(batch_ref,task_key),
 CHECK(baseline_batch_ref IS NULL OR baseline_batch_ref<>candidate_batch_ref)
);
CREATE FUNCTION linggan_guard_ai_trial_comparison() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE candidate linggan_ai_batch%ROWTYPE; baseline linggan_ai_batch%ROWTYPE;
BEGIN
 SELECT * INTO STRICT candidate FROM linggan_ai_batch WHERE batch_ref=NEW.candidate_batch_ref;
 IF candidate.kind<>'trial' THEN RAISE EXCEPTION 'ai_trial_batch_required'; END IF;
 IF NEW.baseline_batch_ref IS NOT NULL THEN
  SELECT * INTO STRICT baseline FROM linggan_ai_batch WHERE batch_ref=NEW.baseline_batch_ref;
  IF baseline.kind<>'trial' OR baseline.inputs_sha256<>candidate.inputs_sha256 OR baseline.item_count<>candidate.item_count
   OR baseline.task_config_ref=candidate.task_config_ref OR EXISTS(
    SELECT 1 FROM (SELECT position,input_ref FROM linggan_ai_batch_item WHERE batch_ref=baseline.batch_ref)a
    FULL JOIN (SELECT position,input_ref FROM linggan_ai_batch_item WHERE batch_ref=candidate.batch_ref)b USING(position)
    WHERE a.input_ref IS DISTINCT FROM b.input_ref)
  THEN RAISE EXCEPTION 'ai_trial_inputs_differ'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_trial_comparison_valid BEFORE INSERT ON linggan_ai_trial_comparison
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_trial_comparison();
CREATE TRIGGER ai_batch_immutable BEFORE UPDATE OR DELETE ON linggan_ai_batch FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_batch_item_immutable BEFORE UPDATE OR DELETE ON linggan_ai_batch_item FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER ai_trial_comparison_immutable BEFORE UPDATE OR DELETE ON linggan_ai_trial_comparison FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
