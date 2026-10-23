-- Dossier v2 is the only active material/input contract. Immutable historical
-- configurations and writing receipts retain the versions they actually used.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM linggan_creation_ai_request
   WHERE state IN ('queued','running','cancel_requested')) THEN
  RAISE EXCEPTION 'research_dossier_writing_drain_required';
 END IF;
END $$;

CREATE OR REPLACE FUNCTION linggan_ai_columns_valid(value jsonb)
RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
 SELECT CASE WHEN jsonb_typeof(value)<>'array' THEN false ELSE
  NOT EXISTS(SELECT 1 FROM jsonb_array_elements(value)x WHERE jsonb_typeof(x)<>'string'
    OR (x#>>'{}') NOT IN ('title','body','tags','ocr','transcript','frame_text'))
  AND jsonb_array_length(value)=(SELECT count(DISTINCT x) FROM jsonb_array_elements(value)x) END
$$;

-- A fresh install may run the complete migration chain in one transaction.
-- Validate deferred seed command references before altering their table, then
-- restore this constraint's declared deferred mode for later commands.
SET CONSTRAINTS linggan_ai_task_config_command_ref_fkey IMMEDIATE;

-- Historical rows have already passed their own insert contract. Store their
-- shape, while the insert and activation guards below enforce the current set.
ALTER TABLE linggan_ai_task_config
 DROP CONSTRAINT linggan_ai_task_config_material_columns_check,
 ADD CONSTRAINT ai_task_config_columns_history_shape CHECK(
  jsonb_typeof(material_columns)='array'
  AND NOT jsonb_path_exists(material_columns,'$[*] ? (@.type() != "string")'));
SET CONSTRAINTS linggan_ai_task_config_command_ref_fkey DEFERRED;

CREATE FUNCTION linggan_guard_dossier_task_config() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT linggan_ai_columns_valid(NEW.material_columns) THEN
  RAISE EXCEPTION 'ai_dossier_columns_required' USING ERRCODE='23514';
 END IF;
 IF NEW.task_key='writing_legacy'
  AND NEW.execution_contract IS DISTINCT FROM 'content-workbench-writing.v2' THEN
  RAISE EXCEPTION 'ai_dossier_writing_contract_required' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_dossier_task_config BEFORE INSERT ON linggan_ai_task_config
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_dossier_task_config();

CREATE FUNCTION linggan_guard_dossier_task_setting() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM linggan_ai_task_config c
  WHERE c.task_config_ref=NEW.current_config_ref AND c.task_key=NEW.task_key
   AND linggan_ai_columns_valid(c.material_columns)
   AND (c.task_key<>'writing_legacy' OR c.execution_contract='content-workbench-writing.v2')) THEN
  RAISE EXCEPTION 'ai_dossier_configuration_required' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_dossier_task_setting BEFORE INSERT OR UPDATE ON linggan_ai_task_setting
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_dossier_task_setting();

UPDATE linggan_ai_task SET handler_contract='content-workbench-writing.v2'
 WHERE task_key='writing_legacy';
WITH current_configs AS (
 SELECT c.*,
  CASE WHEN c.material_columns @> '["title","cover_headline","body","tags","ocr","transcript"]'::jsonb
   THEN '["title","body","tags","ocr","transcript","frame_text"]'::jsonb
   ELSE (SELECT COALESCE(jsonb_agg(name ORDER BY ordinal),'[]'::jsonb)
    FROM (VALUES ('title',1),('body',2),('tags',3),('ocr',4),('transcript',5),('frame_text',6)) fields(name,ordinal)
    WHERE c.material_columns ? name OR (name='ocr' AND c.material_columns ? 'cover_headline'))
  END AS canonical_columns,
  CASE WHEN c.task_key='writing_legacy' THEN 'content-workbench-writing.v2'
   ELSE c.execution_contract END AS canonical_contract
 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref
), replacements AS (
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,execution_contract,
  skill_version_ref,material_columns,budget_mode,daily_token_limit,allowed_tools,origin)
 SELECT gen_random_uuid(),task_key,model_config_ref,canonical_contract,skill_version_ref,
  canonical_columns,budget_mode,daily_token_limit,allowed_tools,'seed'
 FROM current_configs WHERE material_columns IS DISTINCT FROM canonical_columns
  OR execution_contract IS DISTINCT FROM canonical_contract
 RETURNING task_config_ref,task_key
)
UPDATE linggan_ai_task_setting s SET current_config_ref=r.task_config_ref,
 revision=revision+1,updated_at=scope_001_now()
 FROM replacements r WHERE r.task_key=s.task_key;

ALTER TABLE linggan_creation_ai_preview
 DROP CONSTRAINT linggan_creation_ai_preview_method_version_check,
 ADD CONSTRAINT creation_ai_preview_method_history CHECK(
  method_version IN ('content-workbench-writing.v1','content-workbench-writing.v2'));
ALTER TABLE linggan_creation_writing_qualification
 DROP CONSTRAINT linggan_creation_writing_qualification_method_version_check,
 ADD CONSTRAINT creation_writing_qualification_method_history CHECK(
  method_version IN ('content-workbench-writing.v1','content-workbench-writing.v2'));

CREATE FUNCTION linggan_guard_dossier_writing_method() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.method_version<>'content-workbench-writing.v2' THEN
  RAISE EXCEPTION 'ai_dossier_writing_contract_required' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER ai_dossier_writing_preview BEFORE INSERT ON linggan_creation_ai_preview
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_dossier_writing_method();
CREATE TRIGGER ai_dossier_writing_qualification BEFORE INSERT ON linggan_creation_writing_qualification
 FOR EACH ROW EXECUTE FUNCTION linggan_guard_dossier_writing_method();
