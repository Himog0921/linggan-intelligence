-- AI-GEN1 controls start off; migration never grants egress or dispatches work.
CREATE TABLE linggan_ai_command (
 command_ref uuid PRIMARY KEY,
 operation text NOT NULL,
 payload jsonb NOT NULL CHECK(jsonb_typeof(payload)='object' AND octet_length(payload::text)<=65536),
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER ai_command_immutable BEFORE UPDATE OR DELETE ON linggan_ai_command
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

CREATE TABLE linggan_ai_control (
 scope_key text PRIMARY KEY,
 task_key text UNIQUE REFERENCES linggan_ai_task,
 enabled boolean NOT NULL DEFAULT false,
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 actor_kind text NOT NULL DEFAULT 'system' CHECK(actor_kind IN ('human','system')),
 reason text NOT NULL DEFAULT 'not_enabled' CHECK(char_length(btrim(reason)) BETWEEN 1 AND 300),
 command_ref uuid REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((scope_key IN ('global','executor') AND task_key IS NULL)
  OR (task_key IS NOT NULL AND scope_key='task:'||task_key)),
 CHECK(actor_kind='human' OR NOT enabled)
);
INSERT INTO linggan_ai_control(scope_key) VALUES('global'),('executor');
INSERT INTO linggan_ai_control(scope_key,task_key) SELECT 'task:'||task_key,task_key FROM linggan_ai_task;
CREATE TABLE linggan_ai_control_event (
 event_sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 scope_key text NOT NULL REFERENCES linggan_ai_control,
 revision bigint NOT NULL CHECK(revision>0),
 enabled boolean NOT NULL,
 actor_kind text NOT NULL CHECK(actor_kind IN ('human','system')),
 reason text NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 1 AND 300),
 command_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(scope_key,revision),
 CHECK(actor_kind='human' OR NOT enabled)
);
CREATE TRIGGER ai_control_event_immutable BEFORE UPDATE OR DELETE ON linggan_ai_control_event
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE FUNCTION linggan_guard_ai_control() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ai_control_delete_forbidden'; END IF;
 IF NEW.scope_key<>OLD.scope_key OR NEW.task_key IS DISTINCT FROM OLD.task_key
 OR NEW.revision<>OLD.revision+1 OR NEW.command_ref IS NULL
 THEN RAISE EXCEPTION 'ai_control_revision_conflict'; END IF;
 IF NEW.enabled AND NEW.task_key IS NOT NULL AND NOT EXISTS(
  SELECT 1 FROM linggan_ai_task WHERE task_key=NEW.task_key AND lifecycle='active'
 ) THEN RAISE EXCEPTION 'ai_task_not_connected'; END IF;
 INSERT INTO linggan_ai_control_event(scope_key,revision,enabled,actor_kind,reason,command_ref,created_at)
 VALUES(NEW.scope_key,NEW.revision,NEW.enabled,NEW.actor_kind,NEW.reason,NEW.command_ref,NEW.updated_at);
 RETURN NEW;
END $$;
CREATE TRIGGER ai_control_revision BEFORE UPDATE OR DELETE ON linggan_ai_control
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_control();
