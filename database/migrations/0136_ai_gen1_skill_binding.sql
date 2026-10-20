CREATE TABLE linggan_skill_binding (
 task_key text PRIMARY KEY REFERENCES linggan_ai_task,
 current_version_ref uuid REFERENCES linggan_skill_version,
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 activation_config_ref uuid,
 approval_source text CHECK(approval_source IN ('trial','first_version','prior_approval')),
 trial_comparison_ref uuid REFERENCES linggan_ai_trial_comparison,
 reason text,
 command_ref uuid REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(activation_config_ref,task_key) REFERENCES linggan_ai_task_config(task_config_ref,task_key)
);
INSERT INTO linggan_skill_binding(task_key) SELECT task_key FROM linggan_ai_task;
CREATE TABLE linggan_skill_binding_event (
 event_ref uuid PRIMARY KEY REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 revision bigint NOT NULL CHECK(revision>0),
 version_ref uuid NOT NULL REFERENCES linggan_skill_version,
 activation_config_ref uuid NOT NULL,
 approval_source text NOT NULL CHECK(approval_source IN ('trial','first_version','prior_approval')),
 trial_comparison_ref uuid REFERENCES linggan_ai_trial_comparison,
 reason text NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 1 AND 300),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(task_key,revision),
 FOREIGN KEY(activation_config_ref,task_key) REFERENCES linggan_ai_task_config(task_config_ref,task_key),
 CHECK((approval_source='trial')=(trial_comparison_ref IS NOT NULL))
);
CREATE INDEX skill_binding_approval ON linggan_skill_binding_event(task_key,version_ref);
CREATE FUNCTION linggan_ai_trial_qualifies(comparison uuid,task text,version uuid,selected_config uuid) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM linggan_ai_trial_comparison comparison
  JOIN linggan_ai_batch candidate ON candidate.batch_ref=comparison.candidate_batch_ref
  JOIN linggan_ai_task_config config ON config.task_config_ref=candidate.task_config_ref
  JOIN linggan_ai_task_config selected ON selected.task_config_ref=$4 AND selected.task_key=config.task_key
  JOIN linggan_ai_batch_control candidate_state ON candidate_state.batch_ref=candidate.batch_ref
  LEFT JOIN linggan_ai_batch_control baseline_state ON baseline_state.batch_ref=comparison.baseline_batch_ref
  WHERE comparison.comparison_ref=$1 AND comparison.task_key=$2 AND config.skill_version_ref=$3
   AND candidate_state.state='completed' AND (comparison.baseline_batch_ref IS NULL OR baseline_state.state='completed')
   AND (selected.model_config_ref,selected.skill_version_ref,selected.execution_contract,selected.material_columns,selected.budget_mode,selected.daily_token_limit,selected.allowed_tools)
    IS NOT DISTINCT FROM (config.model_config_ref,config.skill_version_ref,config.execution_contract,config.material_columns,config.budget_mode,config.daily_token_limit,config.allowed_tools)
   AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
    WHERE i.batch_ref IN (comparison.candidate_batch_ref,comparison.baseline_batch_ref) AND c.state IN ('pending','running'))
   AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
    WHERE i.batch_ref=candidate.batch_ref AND c.attempt_count>0 AND
     (c.executed_config_ref IS DISTINCT FROM config.task_config_ref OR c.executed_skill_version_ref IS DISTINCT FROM config.skill_version_ref
      OR c.execution_contract IS DISTINCT FROM config.execution_contract))
   AND EXISTS(SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
    WHERE i.batch_ref=candidate.batch_ref AND c.attempt_count>0)
   AND (comparison.baseline_batch_ref IS NULL OR EXISTS(
    SELECT 1 FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref)
    WHERE i.batch_ref=comparison.baseline_batch_ref AND c.attempt_count>0)))
$$;
CREATE FUNCTION linggan_guard_skill_binding() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE candidate linggan_skill_version%ROWTYPE;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'skill_binding_delete_forbidden'; END IF;
 IF NEW.task_key<>OLD.task_key OR NEW.revision<>OLD.revision+1 OR NEW.command_ref IS NULL
  OR NEW.current_version_ref IS NULL OR NEW.reason IS NULL OR char_length(btrim(NEW.reason)) NOT BETWEEN 1 AND 300
  OR NEW.approval_source IS NULL OR NOT EXISTS(
   SELECT 1 FROM linggan_ai_task_config WHERE task_config_ref=NEW.activation_config_ref
    AND task_key=NEW.task_key AND skill_version_ref=NEW.current_version_ref)
 THEN RAISE EXCEPTION 'skill_binding_revision_invalid'; END IF;
 SELECT * INTO STRICT candidate FROM linggan_skill_version WHERE version_ref=NEW.current_version_ref;
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_task WHERE task_key=NEW.task_key AND skill_allowed)
 THEN RAISE EXCEPTION 'ai_skill_not_allowed_for_task'; END IF;
 IF NEW.approval_source='trial' THEN
  IF NOT linggan_ai_trial_qualifies(NEW.trial_comparison_ref,NEW.task_key,NEW.current_version_ref,NEW.activation_config_ref)
  THEN RAISE EXCEPTION 'skill_trial_required'; END IF;
 ELSIF NEW.approval_source='first_version' THEN
  IF candidate.version_no<>1 OR EXISTS(SELECT 1 FROM linggan_skill_binding_event e
   JOIN linggan_skill_version v ON v.version_ref=e.version_ref WHERE e.task_key=NEW.task_key AND v.skill_ref=candidate.skill_ref)
  THEN RAISE EXCEPTION 'skill_first_version_exception_invalid'; END IF;
 ELSIF NEW.approval_source='prior_approval' THEN
  IF NOT EXISTS(SELECT 1 FROM linggan_skill_binding_event WHERE task_key=NEW.task_key AND version_ref=NEW.current_version_ref)
  THEN RAISE EXCEPTION 'skill_prior_approval_missing'; END IF;
 END IF;
 INSERT INTO linggan_skill_binding_event(event_ref,task_key,revision,version_ref,activation_config_ref,
  approval_source,trial_comparison_ref,reason,created_at)
 VALUES(NEW.command_ref,NEW.task_key,NEW.revision,NEW.current_version_ref,NEW.activation_config_ref,
  NEW.approval_source,NEW.trial_comparison_ref,NEW.reason,NEW.updated_at);
 RETURN NEW;
END $$;
CREATE TRIGGER skill_binding_valid BEFORE UPDATE OR DELETE ON linggan_skill_binding FOR EACH ROW EXECUTE FUNCTION linggan_guard_skill_binding();
CREATE TRIGGER skill_binding_event_immutable BEFORE UPDATE OR DELETE ON linggan_skill_binding_event FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Safe configuration DTO; raw connection versions and secrets are not part of this projection.
CREATE VIEW linggan_ai_config_projection AS SELECT c.task_config_ref,c.task_key,c.config_sequence,
 jsonb_build_object('contract','ai-task-configuration.v1','taskConfigRef',c.task_config_ref,'taskKey',c.task_key,
 'executionContract',c.execution_contract,'skillVersionRef',c.skill_version_ref,'materialColumns',c.material_columns,
 'budget',CASE WHEN c.budget_mode='capped' THEN jsonb_build_object('mode',c.budget_mode,'tokens',c.daily_token_limit::text)
  ELSE jsonb_build_object('mode',c.budget_mode) END,'origin',c.origin,'createdAt',c.created_at,
 'model',CASE WHEN m.config_ref IS NULL THEN NULL ELSE jsonb_build_object(
  'modelConfigRef',m.config_ref,'modelRef',m.model_ref,'identityRef',identity.identity_ref,'modelId',entry.model_id,
  'connectionRef',version.connection_ref,'connectionVersionRef',version.version_ref,'capabilityRef',m.capability_ref,
  'limits',jsonb_build_object('inputTokens',m.input_token_limit,'outputTokens',m.output_token_limit,
   'timeoutSeconds',m.timeout_seconds,'attempts',m.max_attempts,'thinkingLevel',m.thinking_level)) END) AS configuration
 FROM linggan_ai_task_config c LEFT JOIN linggan_model_config m ON m.config_ref=c.model_config_ref
 LEFT JOIN linggan_model_entry entry ON entry.model_ref=m.model_ref
 LEFT JOIN linggan_model_connection_version version ON version.version_ref=entry.connection_version_ref
 LEFT JOIN linggan_ai_model_identity identity ON identity.connection_ref=version.connection_ref AND identity.model_id=entry.model_id;
