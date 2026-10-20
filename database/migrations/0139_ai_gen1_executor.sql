-- The advisory lock belongs to a dedicated session, never a returned pool connection.
INSERT INTO linggan_ai_failure_code VALUES
 ('ai_adapter_rejected_before_dispatch',false,false,'固定适配器明确拒绝，未派发且不占预留','executor_v1');
CREATE FUNCTION linggan_ai_executor_lock_key() RETURNS bigint LANGUAGE sql STABLE AS $$
 SELECT hashtextextended(current_database()||':'||current_schema()||':ai-gen1-executor.v1',4540139)
$$;
CREATE TABLE linggan_ai_executor_instance (
 instance_ref uuid PRIMARY KEY,
 generation bigint NOT NULL UNIQUE CHECK(generation>0),
 owner_backend_pid integer NOT NULL CHECK(owner_backend_pid>0),
 process_id integer NOT NULL CHECK(process_id>0),
 source_revision text NOT NULL CHECK(char_length(source_revision) BETWEEN 1 AND 128),
 concurrency integer NOT NULL CHECK(concurrency BETWEEN 4 AND 8),
 state text NOT NULL CHECK(state IN ('running','draining','drained','lost')),
 started_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 heartbeat_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 drain_requested_at timestamptz,
 drained_at timestamptz,
 CHECK(state<>'running' OR (drain_requested_at IS NULL AND drained_at IS NULL)),
 CHECK(state NOT IN ('draining','drained') OR drain_requested_at IS NOT NULL),
 CHECK((state='drained')=(drained_at IS NOT NULL)),
 UNIQUE(instance_ref,generation)
);
CREATE TABLE linggan_ai_executor_current (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 generation bigint NOT NULL DEFAULT 0 CHECK(generation>=0),
 instance_ref uuid UNIQUE,
 FOREIGN KEY(instance_ref,generation) REFERENCES linggan_ai_executor_instance(instance_ref,generation)
  DEFERRABLE INITIALLY DEFERRED,
 CHECK((generation=0)=(instance_ref IS NULL))
);
INSERT INTO linggan_ai_executor_current(singleton) VALUES(true);
CREATE FUNCTION linggan_ai_executor_is_owner(owner_ref uuid,owner_generation bigint,require_running boolean DEFAULT true)
 RETURNS boolean LANGUAGE sql VOLATILE AS $$
 SELECT EXISTS(
  SELECT 1 FROM linggan_ai_executor_current c JOIN linggan_ai_executor_instance i USING(instance_ref,generation)
  WHERE c.instance_ref=owner_ref AND c.generation=owner_generation
   AND (i.state='running' OR (NOT require_running AND i.state='draining'))
   AND i.heartbeat_at>clock_timestamp()-interval '30 seconds'
   AND EXISTS(SELECT 1 FROM pg_locks l WHERE l.locktype='advisory' AND l.granted
    AND l.pid=i.owner_backend_pid AND l.objsubid=1
    AND l.database=(SELECT oid FROM pg_database WHERE datname=current_database())
    AND l.classid=((linggan_ai_executor_lock_key()>>32)&4294967295)::oid
    AND l.objid=(linggan_ai_executor_lock_key()&4294967295)::oid))
$$;
ALTER TABLE linggan_ai_batch_item_control
 ADD COLUMN lease_instance_ref uuid,
 ADD COLUMN lease_instance_generation bigint,
 ADD CONSTRAINT ai_item_executor_owner FOREIGN KEY(lease_instance_ref,lease_instance_generation)
  REFERENCES linggan_ai_executor_instance(instance_ref,generation),
 ADD CONSTRAINT ai_item_executor_presence CHECK(
  (state='running' AND lease_instance_ref IS NOT NULL AND lease_instance_generation IS NOT NULL)
  OR (state<>'running' AND lease_instance_ref IS NULL AND lease_instance_generation IS NULL));
CREATE INDEX ai_item_executor_inflight ON linggan_ai_batch_item_control(lease_instance_ref,item_ref) WHERE state='running';

-- The transaction context is set only after locking and validating the live session fence.
-- Recovery is explicit: an expired attempt can be settled, never silently sent twice.
CREATE FUNCTION linggan_guard_ai_executor_item() RETURNS trigger LANGUAGE plpgsql AS $$
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
  IF NOT recovery OR NEW.state NOT IN ('pending','failed','cancelled') OR NEW.output IS NOT NULL
   OR (OLD.lease_expires_at>clock_timestamp() AND linggan_ai_executor_is_owner(OLD.lease_instance_ref,OLD.lease_instance_generation,false))
  THEN RAISE EXCEPTION 'ai_item_stale_owner'; END IF;
  IF NEW.state='pending' AND OLD.attempt_count>0 THEN RAISE EXCEPTION 'ai_item_ambiguous_retry'; END IF;
 END IF;
 IF NEW.state='succeeded' AND EXISTS(SELECT 1 FROM linggan_ai_batch_item item
  JOIN linggan_ai_batch_control b USING(batch_ref) WHERE item.item_ref=NEW.item_ref AND b.cancel_requested)
 THEN RAISE EXCEPTION 'ai_cancelled_output_publication'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_item_executor_guard BEFORE UPDATE ON linggan_ai_batch_item_control
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_executor_item();

CREATE TABLE linggan_ai_execution_attempt (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation,
 item_ref uuid NOT NULL REFERENCES linggan_ai_batch_item,
 instance_ref uuid NOT NULL REFERENCES linggan_ai_executor_instance,
 lease_generation bigint NOT NULL CHECK(lease_generation>0),
 attempt_no integer NOT NULL CHECK(attempt_no BETWEEN 1 AND 3),
 task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config,
 skill_version_ref uuid REFERENCES linggan_skill_version,
 execution_contract text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(item_ref,attempt_no),
 UNIQUE(invocation_ref,item_ref,instance_ref,lease_generation)
);
CREATE FUNCTION linggan_record_ai_execution_attempt() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.attempt_count>OLD.attempt_count THEN
  INSERT INTO linggan_ai_execution_attempt(invocation_ref,item_ref,instance_ref,lease_generation,
    attempt_no,task_config_ref,skill_version_ref,execution_contract)
   VALUES(NEW.invocation_ref,NEW.item_ref,NEW.lease_instance_ref,NEW.lease_generation,
    NEW.attempt_count,NEW.executed_config_ref,NEW.executed_skill_version_ref,NEW.execution_contract);
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER ai_item_attempt_receipt AFTER UPDATE ON linggan_ai_batch_item_control
 FOR EACH ROW EXECUTE FUNCTION linggan_record_ai_execution_attempt();
CREATE TRIGGER ai_execution_attempt_immutable BEFORE UPDATE OR DELETE ON linggan_ai_execution_attempt
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- A late receipt preserves measured facts without rewriting a terminal invocation or quota.
CREATE TABLE linggan_ai_late_usage_receipt (
 invocation_ref uuid PRIMARY KEY REFERENCES linggan_model_invocation,
 item_ref uuid NOT NULL REFERENCES linggan_ai_batch_item,
 instance_ref uuid NOT NULL REFERENCES linggan_ai_executor_instance,
 lease_generation bigint NOT NULL CHECK(lease_generation>0),
 input_tokens bigint CHECK(input_tokens>=0),
 output_tokens bigint CHECK(output_tokens>=0),
 cached_input_tokens bigint CHECK(cached_input_tokens>=0 AND cached_input_tokens<=input_tokens),
 cached_write_tokens bigint CHECK(cached_write_tokens>=0 AND cached_write_tokens<=input_tokens),
 reasoning_tokens bigint CHECK(reasoning_tokens>=0 AND reasoning_tokens<=output_tokens),
 received_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(invocation_ref,item_ref,instance_ref,lease_generation)
  REFERENCES linggan_ai_execution_attempt(invocation_ref,item_ref,instance_ref,lease_generation),
 CHECK(input_tokens IS NOT NULL OR output_tokens IS NOT NULL)
);
CREATE TRIGGER ai_late_usage_immutable BEFORE UPDATE OR DELETE ON linggan_ai_late_usage_receipt
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Only the explicit fixed-adapter no-send receipt excludes an allocated attempt from rounds.
-- A missing dispatch observation remains attempted/unknown; it cannot clear the no-output brake.
CREATE OR REPLACE FUNCTION linggan_ai_item_was_attempted(attempts integer,invocation uuid) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT attempts>0 AND COALESCE((SELECT dispatch_state<>'not_dispatched' FROM linggan_model_invocation WHERE invocation_ref=invocation),true)
$$;

-- A tiny probe cannot invent larger capacities, extra grades, or an untested native format.
-- It can cite only the exact previously recorded declaration used in that probe's frozen profile.
CREATE FUNCTION linggan_ai_probe_supports_capability(invocation uuid,source_version text,identity uuid,endpoint uuid,
 limits jsonb,compat jsonb,level_map jsonb,output_mode text,strict_schema boolean) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM linggan_model_invocation i
  JOIN linggan_model_entry m ON m.model_ref=i.model_ref
  JOIN linggan_model_connection_version v ON v.version_ref=i.connection_version_ref
  JOIN linggan_ai_model_identity stable ON stable.connection_ref=v.connection_ref AND stable.model_id=m.model_id
  JOIN linggan_ai_connection_endpoint route ON route.version_ref=v.version_ref
  WHERE i.invocation_ref=invocation AND i.operation='probe' AND i.state='succeeded'
   AND stable.identity_ref=identity AND route.endpoint_ref=endpoint
   AND v.revision=(SELECT max(current.revision) FROM linggan_model_connection_version current WHERE current.connection_ref=v.connection_ref)
   AND i.result->>'producer'='ai-gen1-executor.v1' AND i.result->>'adapterVersion'=source_version
   AND i.result->'modelCallable'='true'::jsonb
   AND i.result#>'{testedProfile,execution,limits}'=limits
   AND i.result#>'{testedProfile,execution,compat}'=compat
   AND i.result#>'{testedProfile,execution,thinkingLevelMap}'=level_map
   AND i.result#>>'{testedProfile,execution,outputMode}'=output_mode
   AND i.result#>'{testedProfile,execution,strictSchema}'=to_jsonb(strict_schema)
   AND (output_mode='prompt_only' OR i.result->'syntheticFormatMatched'='true'::jsonb))
$$;
CREATE FUNCTION linggan_guard_ai_probe_capability() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.source_kind='probe' AND NOT linggan_ai_probe_supports_capability(NEW.probe_invocation_ref,NEW.source_version,
  NEW.identity_ref,NEW.endpoint_ref,jsonb_build_object('contextTokens',NEW.context_tokens,'maxOutputTokens',NEW.max_output_tokens,
   'maxTimeoutSeconds',NEW.max_timeout_seconds,'thinkingLevels',NEW.thinking_levels),NEW.compat,NEW.thinking_level_map,NEW.output_mode,NEW.strict_schema)
 THEN RAISE EXCEPTION 'ai_probe_profile_not_proven'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_probe_profile_qualification BEFORE INSERT ON linggan_model_capability
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_probe_capability();
