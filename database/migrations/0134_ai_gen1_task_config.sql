-- Resource ceilings are shared with ai-resource-limits.v1. Capability remains mandatory
-- for newly configured execution; small, frozen legacy configurations stay readable.
ALTER TABLE linggan_model_config
 DROP CONSTRAINT linggan_model_config_input_token_limit_check,
 DROP CONSTRAINT linggan_model_config_output_token_limit_check,
 DROP CONSTRAINT linggan_model_config_timeout_seconds_check,
 ADD CHECK(input_token_limit BETWEEN 1024 AND 2097152),
 ADD CHECK(output_token_limit BETWEEN 16 AND 2097152),
 ADD CHECK(timeout_seconds BETWEEN 1 AND 3600),
 ADD COLUMN capability_ref uuid REFERENCES linggan_model_capability,
 ADD COLUMN thinking_level text NOT NULL DEFAULT 'off'
  CHECK(thinking_level IN ('off','minimal','low','medium','high','xhigh','max'));
CREATE FUNCTION linggan_validate_ai_model_config() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE cap linggan_model_capability%ROWTYPE;
BEGIN
 IF NEW.capability_ref IS NULL THEN
  IF NEW.input_token_limit>32768 OR NEW.output_token_limit NOT BETWEEN 128 AND 8192
   OR NEW.timeout_seconds>60 OR NEW.thinking_level<>'off'
  THEN RAISE EXCEPTION 'ai_capability_required'; END IF;
  RETURN NEW;
 END IF;
 SELECT * INTO STRICT cap FROM linggan_model_capability WHERE capability_ref=NEW.capability_ref;
 IF NOT EXISTS(SELECT 1 FROM linggan_model_entry m
  JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref
  JOIN linggan_ai_model_identity i ON i.connection_ref=v.connection_ref AND i.model_id=m.model_id
  JOIN linggan_ai_connection_endpoint p ON p.version_ref=v.version_ref
  WHERE m.model_ref=NEW.model_ref AND i.identity_ref=cap.identity_ref AND p.endpoint_ref=cap.endpoint_ref)
 THEN RAISE EXCEPTION 'ai_config_model_capability_mismatch'; END IF;
 IF NEW.input_token_limit::bigint+NEW.output_token_limit>cap.context_tokens
  OR NEW.output_token_limit>cap.max_output_tokens OR NEW.timeout_seconds>cap.max_timeout_seconds
  OR NOT cap.thinking_levels ? NEW.thinking_level
 THEN RAISE EXCEPTION 'ai_config_exceeds_capability'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_model_config_capability BEFORE INSERT ON linggan_model_config
 FOR EACH ROW EXECUTE FUNCTION linggan_validate_ai_model_config();

ALTER TABLE linggan_ai_task ADD COLUMN requires_skill boolean NOT NULL DEFAULT false;
ALTER TABLE linggan_ai_task ADD COLUMN skill_allowed boolean NOT NULL DEFAULT false;
ALTER TABLE linggan_ai_task ADD COLUMN handler_contract text;
UPDATE linggan_ai_task SET skill_allowed=true WHERE task_key IN
 ('note_interpretation','direction_list_draft','direction_proposal','author_org_identity','writing_assistant','writing_legacy');
UPDATE linggan_ai_task SET handler_contract='ai-probe.synthetic.v1' WHERE task_key='model_probe';
UPDATE linggan_ai_task SET handler_contract='content-workbench-writing.v1' WHERE task_key='writing_legacy';
UPDATE linggan_ai_task SET requires_skill=true WHERE task_key IN
 ('note_interpretation','direction_list_draft','direction_proposal','author_org_identity','writing_assistant');
CREATE FUNCTION linggan_ai_columns_valid(value jsonb) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE WHEN jsonb_typeof(value)<>'array' THEN false ELSE
  NOT EXISTS(SELECT 1 FROM jsonb_array_elements(value)x WHERE jsonb_typeof(x)<>'string'
    OR (x#>>'{}') NOT IN ('title','cover_headline','body','tags','ocr','transcript'))
  AND jsonb_array_length(value)=(SELECT count(DISTINCT x) FROM jsonb_array_elements(value)x) END
$$;
CREATE TABLE linggan_ai_task_config (
 task_config_ref uuid PRIMARY KEY,
 config_sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
 task_key text NOT NULL REFERENCES linggan_ai_task,
 model_config_ref uuid REFERENCES linggan_model_config,
 execution_contract text,
 skill_version_ref uuid REFERENCES linggan_skill_version,
 material_columns jsonb NOT NULL CHECK(linggan_ai_columns_valid(material_columns)),
 budget_mode text NOT NULL CHECK(budget_mode IN ('unconfigured','unlimited','capped')),
 daily_token_limit bigint CHECK(daily_token_limit>=0),
 allowed_tools jsonb NOT NULL DEFAULT '[]' CHECK(allowed_tools='[]'::jsonb),
 origin text NOT NULL CHECK(origin IN ('seed','human','skill_activation','legacy_import')),
 command_ref uuid REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((budget_mode='capped')=(daily_token_limit IS NOT NULL)),
 CHECK(origin IN ('seed','legacy_import') OR command_ref IS NOT NULL),
 CHECK(origin<>'legacy_import' OR task_key='writing_legacy'),
 UNIQUE(task_config_ref,task_key)
);
CREATE FUNCTION linggan_validate_ai_task_config() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.skill_version_ref IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_ai_task WHERE task_key=NEW.task_key AND skill_allowed)
 THEN RAISE EXCEPTION 'ai_skill_not_allowed_for_task'; END IF;
 IF NEW.origin NOT IN ('seed','legacy_import') AND NEW.model_config_ref IS NOT NULL
  AND NOT EXISTS(SELECT 1 FROM linggan_model_config WHERE config_ref=NEW.model_config_ref AND capability_ref IS NOT NULL)
 THEN RAISE EXCEPTION 'ai_task_config_unqualified_model'; END IF;
 IF NEW.task_key IN ('note_interpretation','direction_list_draft','direction_proposal') AND jsonb_array_length(NEW.material_columns)=0
 THEN RAISE EXCEPTION 'ai_task_columns_empty'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_task_config_valid BEFORE INSERT ON linggan_ai_task_config
 FOR EACH ROW EXECUTE FUNCTION linggan_validate_ai_task_config();
CREATE TRIGGER ai_task_config_immutable BEFORE UPDATE OR DELETE ON linggan_ai_task_config
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TABLE linggan_ai_task_setting (
 task_key text PRIMARY KEY REFERENCES linggan_ai_task,
 current_config_ref uuid NOT NULL,
 revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 FOREIGN KEY(current_config_ref,task_key) REFERENCES linggan_ai_task_config(task_config_ref,task_key)
);
WITH configs AS (
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,execution_contract,material_columns,budget_mode,origin)
 SELECT gen_random_uuid(),t.task_key,
  CASE WHEN t.task_key='writing_legacy' THEN w.default_config_ref ELSE NULL END,
  t.handler_contract,
  CASE WHEN t.task_key IN ('note_interpretation','direction_list_draft','direction_proposal')
   THEN '["title","cover_headline","body","tags","ocr","transcript"]'::jsonb ELSE '[]'::jsonb END,
  CASE WHEN t.task_key='note_interpretation' THEN 'unlimited' ELSE 'unconfigured' END,
  CASE WHEN t.task_key='writing_legacy' AND w.default_config_ref IS NOT NULL THEN 'legacy_import' ELSE 'seed' END
 FROM linggan_ai_task t CROSS JOIN linggan_model_workspace w
 RETURNING task_config_ref,task_key
) INSERT INTO linggan_ai_task_setting(task_key,current_config_ref) SELECT task_key,task_config_ref FROM configs;
CREATE FUNCTION linggan_guard_ai_task_setting() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'ai_setting_delete_forbidden'; END IF;
 IF NEW.task_key<>OLD.task_key OR NEW.revision<>OLD.revision+1 OR NEW.current_config_ref=OLD.current_config_ref
 THEN RAISE EXCEPTION 'ai_setting_revision_conflict'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_task_setting_revision BEFORE UPDATE OR DELETE ON linggan_ai_task_setting
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_ai_task_setting();
