-- Run only against an owned full-schema fixture: psql -v proof_schema=... -f this_file.
-- Every write, including migration application, rolls back. No provider requests.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL search_path TO :"proof_schema", public;
DO $fixture$
DECLARE t record; cfg linggan_ai_task_config%ROWTYPE; p uuid; c uuid; cmd uuid;
BEGIN
 FOR t IN SELECT * FROM (VALUES ('topic_research',2),('topic_research_resolve',1),
  ('topic_research_extract_review',1),('topic_research_resolve_review',1))x(task,version) LOOP
  SELECT old.* INTO STRICT cfg FROM linggan_ai_task_setting s JOIN linggan_ai_task_config old ON old.task_config_ref=s.current_config_ref WHERE s.task_key=t.task;
  SELECT version_ref INTO STRICT p FROM linggan_ai_prompt_version WHERE task_key=t.task AND version_no=t.version;
  c:=gen_random_uuid();
  INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,prompt_version_ref)
   VALUES(c,cfg.task_key,cfg.model_config_ref,cfg.skill_version_ref,cfg.execution_contract,cfg.material_columns,cfg.budget_mode,cfg.daily_token_limit,cfg.allowed_tools,'seed',p);
  UPDATE linggan_ai_task_setting SET current_config_ref=c,revision=revision+1 WHERE task_key=t.task;
 END LOOP;
 -- A real custom immutable prompt identity must stay active.
 SELECT c0.* INTO STRICT cfg FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c0 ON c0.task_config_ref=s.current_config_ref WHERE s.task_key='topic_research_extract_review';
 p:=gen_random_uuid(); c:=gen_random_uuid(); cmd:=gen_random_uuid();
 INSERT INTO linggan_ai_prompt_version(version_ref,task_key,version_no,system_text,instruction_text)
 SELECT p,cfg.task_key,max(version_no)+1,'USER CUSTOM: keep this method','USER CUSTOM instruction' FROM linggan_ai_prompt_version WHERE task_key=cfg.task_key;
 INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES(cmd,'prompt_activate','{}','{}');
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,command_ref,prompt_version_ref)
 VALUES(c,cfg.task_key,cfg.model_config_ref,cfg.skill_version_ref,cfg.execution_contract,cfg.material_columns,cfg.budget_mode,cfg.daily_token_limit,cfg.allowed_tools,'prompt_activation',cmd,p);
 UPDATE linggan_ai_task_setting SET current_config_ref=c,revision=revision+1 WHERE task_key=cfg.task_key;
 -- A human-configured task still using the seed method must upgrade, preserving capped budget/model.
 SELECT c0.* INTO STRICT cfg FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c0 ON c0.task_config_ref=s.current_config_ref WHERE s.task_key='topic_research_resolve';
 c:=gen_random_uuid(); cmd:=gen_random_uuid();
 INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES(cmd,'task_config','{}','{}');
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,command_ref,prompt_version_ref)
 VALUES(c,cfg.task_key,cfg.model_config_ref,cfg.skill_version_ref,cfg.execution_contract,cfg.material_columns,'capped',123456,cfg.allowed_tools,'human',cmd,cfg.prompt_version_ref);
 UPDATE linggan_ai_task_setting SET current_config_ref=c,revision=revision+1 WHERE task_key=cfg.task_key;
END $fixture$;
CREATE TEMP TABLE prompt_before ON COMMIT DROP AS SELECT s.task_key,s.revision,s.current_config_ref,to_jsonb(c) AS cfg FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key LIKE 'topic_research%';
CREATE TEMP TABLE batch_before ON COMMIT DROP AS SELECT batch_ref,task_config_ref FROM linggan_ai_batch;
CREATE TEMP TABLE controls_before ON COMMIT DROP AS SELECT to_jsonb(c) AS row FROM linggan_ai_control c;
CREATE TEMP TABLE prompts_before ON COMMIT DROP AS SELECT version_ref,to_jsonb(p) AS row FROM linggan_ai_prompt_version p;
\ir ../../database/migrations/0202_topic_core_merge_prompts.sql
DO $assert$
DECLARE t record; s record; p record;
BEGIN
 FOR t IN SELECT * FROM prompt_before LOOP
  SELECT setting.*,to_jsonb(c) AS cfg INTO STRICT s FROM linggan_ai_task_setting setting JOIN linggan_ai_task_config c ON c.task_config_ref=setting.current_config_ref WHERE setting.task_key=t.task_key;
  IF t.task_key='topic_research_extract_review' THEN
   IF s.current_config_ref<>t.current_config_ref OR s.revision<>t.revision THEN RAISE EXCEPTION 'custom prompt changed'; END IF;
  ELSE
   IF s.current_config_ref=t.current_config_ref OR s.revision<>t.revision+1 THEN RAISE EXCEPTION 'seed not advanced'; END IF;
   IF (s.cfg - ARRAY['task_config_ref','config_sequence','origin','command_ref','created_at','prompt_version_ref']) IS DISTINCT FROM
      (t.cfg - ARRAY['task_config_ref','config_sequence','origin','command_ref','created_at','prompt_version_ref']) THEN RAISE EXCEPTION 'other settings changed'; END IF;
  END IF;
 END LOOP;
 FOR p IN SELECT DISTINCT ON(task_key) * FROM linggan_ai_prompt_version WHERE task_key LIKE 'topic_research%' ORDER BY task_key,version_no DESC LOOP
  IF p.system_text NOT LIKE '%verification=model_read%' THEN RAISE EXCEPTION 'missing image provenance'; END IF;
  IF p.system_text LIKE '%new 对每个候选提供%' OR p.system_text LIKE '%逐一核查new对全部候选的比较%' THEN RAISE EXCEPTION 'obsolete full relation instruction'; END IF;
  IF p.task_key='topic_research_resolve' AND p.system_text NOT LIKE '%逐字复用同一规范proposedTopic%' THEN RAISE EXCEPTION 'missing shared proposal instruction'; END IF;
 END LOOP;
 IF EXISTS(SELECT 1 FROM prompts_before b JOIN linggan_ai_prompt_version stored_prompt USING(version_ref) WHERE b.row<>to_jsonb(stored_prompt)) THEN RAISE EXCEPTION 'historical prompt mutated'; END IF;
 IF EXISTS(SELECT 1 FROM batch_before b JOIN linggan_ai_batch a USING(batch_ref) WHERE a.task_config_ref<>b.task_config_ref) THEN RAISE EXCEPTION 'batch rebound'; END IF;
 IF EXISTS((SELECT row FROM controls_before) EXCEPT (SELECT to_jsonb(c) FROM linggan_ai_control c)) THEN RAISE EXCEPTION 'controls changed'; END IF;
END $assert$;
ROLLBACK;
\echo 'PASS: 0202 seed upgrade, human settings preserved, custom active preserved, historical methods/batches unchanged, image and merge instructions synchronized'
