-- Round facts are derived from settled, fenced items. System brakes can only switch off.
CREATE TABLE linggan_ai_round (
 round_ref uuid PRIMARY KEY DEFAULT gen_random_uuid(),
 batch_ref uuid NOT NULL REFERENCES linggan_ai_batch,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 round_no integer NOT NULL CHECK(round_no>0),
 evaluated_items integer NOT NULL DEFAULT 0 CHECK(evaluated_items BETWEEN 1 AND 50),
 attempted integer NOT NULL DEFAULT 0 CHECK(attempted BETWEEN 0 AND 50),
 failed integer NOT NULL DEFAULT 0 CHECK(failed BETWEEN 0 AND attempted),
 usable integer NOT NULL DEFAULT 0 CHECK(usable BETWEEN 0 AND attempted-failed),
 consecutive_without_usable integer NOT NULL DEFAULT 0 CHECK(consecutive_without_usable>=0),
 brake_reason text CHECK(brake_reason IN ('failure_ratio','no_usable_output')),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(batch_ref,round_no)
);
CREATE TABLE linggan_ai_task_round_state (
 task_key text PRIMARY KEY REFERENCES linggan_ai_task,
 consecutive_without_usable integer NOT NULL DEFAULT 0 CHECK(consecutive_without_usable>=0),
 evaluated_rounds bigint NOT NULL DEFAULT 0 CHECK(evaluated_rounds>=0),
 last_round_ref uuid REFERENCES linggan_ai_round
);
INSERT INTO linggan_ai_task_round_state(task_key) SELECT task_key FROM linggan_ai_task;
CREATE FUNCTION linggan_ai_item_was_attempted(attempts integer,invocation uuid) RETURNS boolean LANGUAGE sql STABLE AS $$ SELECT attempts>0 $$;
CREATE FUNCTION linggan_ai_prepare_round() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE batch linggan_ai_batch%ROWTYPE; progress linggan_ai_batch_control%ROWTYPE;
 previous integer; automatic boolean; total integer; settled boolean;
BEGIN
 SELECT * INTO STRICT progress FROM linggan_ai_batch_control WHERE batch_ref=NEW.batch_ref FOR UPDATE;
 SELECT * INTO STRICT batch FROM linggan_ai_batch WHERE batch_ref=NEW.batch_ref;
 IF progress.state<>'open' OR NEW.round_no<>progress.last_round_no+1 OR NEW.task_key<>batch.task_key
 THEN RAISE EXCEPTION 'ai_round_out_of_order'; END IF;
 SELECT consecutive_without_usable INTO STRICT previous FROM linggan_ai_task_round_state WHERE task_key=NEW.task_key FOR UPDATE;
 SELECT automatic_brake INTO STRICT automatic FROM linggan_ai_task WHERE task_key=NEW.task_key;
 SELECT count(*)::integer,bool_and(c.state NOT IN ('pending','running')),
  count(*) FILTER(WHERE linggan_ai_item_was_attempted(c.attempt_count,c.invocation_ref))::integer,
  count(*) FILTER(WHERE c.counts_as_failure AND linggan_ai_item_was_attempted(c.attempt_count,c.invocation_ref))::integer,
  count(*) FILTER(WHERE c.usable)::integer
 INTO total,settled,NEW.attempted,NEW.failed,NEW.usable
 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
 WHERE i.batch_ref=NEW.batch_ref AND i.position>=(NEW.round_no-1)*50 AND i.position<NEW.round_no*50;
 IF total=0 OR settled IS DISTINCT FROM true THEN RAISE EXCEPTION 'ai_round_not_settled'; END IF;
 NEW.evaluated_items:=total;
 NEW.consecutive_without_usable:=CASE WHEN NOT automatic OR NEW.attempted=0 THEN previous
  WHEN NEW.usable>0 THEN 0 ELSE previous+1 END;
 NEW.brake_reason:=CASE WHEN NOT automatic OR NEW.attempted=0 THEN NULL
  WHEN NEW.failed*100>NEW.attempted*30 THEN 'failure_ratio'
  WHEN NEW.consecutive_without_usable>=2 THEN 'no_usable_output' ELSE NULL END;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_round_prepare BEFORE INSERT ON linggan_ai_round FOR EACH ROW EXECUTE FUNCTION linggan_ai_prepare_round();
CREATE FUNCTION linggan_ai_commit_round() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE control_revision bigint; total integer; cancelled boolean;
BEGIN
 UPDATE linggan_ai_task_round_state SET consecutive_without_usable=NEW.consecutive_without_usable,
  evaluated_rounds=evaluated_rounds+1,last_round_ref=NEW.round_ref WHERE task_key=NEW.task_key;
 IF NEW.brake_reason IS NOT NULL THEN
  UPDATE linggan_ai_control SET enabled=false,revision=revision+1,actor_kind='system',
   reason='automatic_brake:'||NEW.brake_reason,command_ref=NEW.round_ref,updated_at=scope_001_now()
  WHERE scope_key='task:'||NEW.task_key RETURNING revision INTO STRICT control_revision;
  INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES(NEW.round_ref,'round_brake',
   jsonb_build_object('batchRef',NEW.batch_ref,'roundNo',NEW.round_no,'reason',NEW.brake_reason),
   jsonb_build_object('taskKey',NEW.task_key,'revision',control_revision,'enabled',false));
 END IF;
 SELECT item_count INTO STRICT total FROM linggan_ai_batch WHERE batch_ref=NEW.batch_ref;
 SELECT cancel_requested INTO STRICT cancelled FROM linggan_ai_batch_control WHERE batch_ref=NEW.batch_ref;
 UPDATE linggan_ai_batch_control SET last_round_no=NEW.round_no,revision=revision+1,
  state=CASE WHEN NEW.round_no*50>=total THEN CASE WHEN cancelled THEN 'cancelled' ELSE 'completed' END ELSE 'open' END,
  finished_at=CASE WHEN NEW.round_no*50>=total THEN scope_001_now() ELSE NULL END
 WHERE batch_ref=NEW.batch_ref;
 RETURN NULL;
END $$;
CREATE TRIGGER ai_round_commit AFTER INSERT ON linggan_ai_round FOR EACH ROW EXECUTE FUNCTION linggan_ai_commit_round();
CREATE TRIGGER ai_round_immutable BEFORE UPDATE OR DELETE ON linggan_ai_round FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE FUNCTION linggan_guard_ai_batch_control() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE total integer;
BEGIN
 IF TG_OP='DELETE' OR OLD.state<>'open' THEN RAISE EXCEPTION 'ai_batch_terminal_immutable'; END IF;
 IF NEW.batch_ref<>OLD.batch_ref OR NEW.revision<>OLD.revision+1
  OR NEW.last_round_no NOT BETWEEN OLD.last_round_no AND OLD.last_round_no+1
  OR (OLD.cancel_requested AND NOT NEW.cancel_requested)
 THEN RAISE EXCEPTION 'ai_batch_control_revision_invalid'; END IF;
 IF NEW.last_round_no<>OLD.last_round_no AND NOT EXISTS(
  SELECT 1 FROM linggan_ai_round WHERE batch_ref=NEW.batch_ref AND round_no=NEW.last_round_no)
 THEN RAISE EXCEPTION 'ai_batch_round_missing'; END IF;
 IF NEW.state<>'open' THEN
  SELECT item_count INTO STRICT total FROM linggan_ai_batch WHERE batch_ref=NEW.batch_ref;
  IF NEW.last_round_no<>(total+49)/50 OR EXISTS(
   SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
   WHERE i.batch_ref=NEW.batch_ref AND c.state IN ('pending','running'))
  THEN RAISE EXCEPTION 'ai_batch_not_settled'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_batch_control_guard BEFORE UPDATE OR DELETE ON linggan_ai_batch_control
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_batch_control();
CREATE FUNCTION linggan_ai_evaluate_next_round(batch uuid) RETURNS uuid LANGUAGE plpgsql AS $$
DECLARE progress linggan_ai_batch_control%ROWTYPE; task text; round uuid; total integer; settled boolean;
BEGIN
 SELECT * INTO STRICT progress FROM linggan_ai_batch_control WHERE batch_ref=$1 FOR UPDATE;
 IF progress.state<>'open' THEN RETURN NULL; END IF;
 SELECT count(*)::integer,bool_and(c.state NOT IN ('pending','running')) INTO total,settled
 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
 WHERE i.batch_ref=$1 AND i.position>=progress.last_round_no*50 AND i.position<(progress.last_round_no+1)*50;
 IF total=0 OR settled IS DISTINCT FROM true THEN RETURN NULL; END IF;
 SELECT task_key INTO STRICT task FROM linggan_ai_batch WHERE batch_ref=$1;
 INSERT INTO linggan_ai_round(batch_ref,task_key,round_no) VALUES($1,task,progress.last_round_no+1) RETURNING round_ref INTO round;
 RETURN round;
END $$;
