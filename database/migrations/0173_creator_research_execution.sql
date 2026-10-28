-- Creator work research uses the shared AI queue; all controls and permissions remain off.
UPDATE linggan_ai_task SET label='创作者作品研究',lifecycle='active',handler_contract='creator-work-research.v1' WHERE task_key='creator_research';
WITH replacement AS (
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,prompt_version_ref)
 SELECT gen_random_uuid(),c.task_key,c.model_config_ref,c.skill_version_ref,'creator-work-research.v1',c.material_columns,c.budget_mode,c.daily_token_limit,c.allowed_tools,'seed',c.prompt_version_ref
 FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref
 WHERE s.task_key='creator_research' RETURNING task_config_ref
)
UPDATE linggan_ai_task_setting SET current_config_ref=replacement.task_config_ref,revision=revision+1,updated_at=scope_001_now()
 FROM replacement WHERE task_key='creator_research';
ALTER TABLE linggan_ai_batch DROP CONSTRAINT ai_batch_task_kind_valid;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe')
 OR (task_key IN('writing_legacy','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe')
 OR (task_key IN('work_comparison','creator_research') AND kind='interactive'));
ALTER TABLE linggan_model_egress_event DROP CONSTRAINT ai_egress_material_task_valid;
ALTER TABLE linggan_model_egress_event ADD CONSTRAINT ai_egress_material_task_valid CHECK(
 (task_key='model_probe' AND material_class IN('synthetic_probe','model_catalog'))
 OR (task_key IN('note_interpretation','direction_list_draft','direction_proposal','work_comparison','creator_research') AND material_class='note_material')
 OR (task_key='author_org_identity' AND material_class='public_author_bio')
 OR (task_key IN('writing_assistant','writing_legacy') AND material_class='selected_writing_material')
 OR (task_key='ocr_quality_audit' AND material_class IN('synthetic_image_probe','ocr_quality_image'))
 OR (task_key='ocr_recognition' AND material_class='manual_ocr_image'));
CREATE TABLE linggan_creator_research_request (
 request_ref uuid PRIMARY KEY,
 command_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 work_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 knowledge_cutoff timestamptz NOT NULL,
 input_fingerprint text NOT NULL CHECK(input_fingerprint ~ '^[0-9a-f]{64}$'),
 task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config,
 batch_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_batch,
 method_version text NOT NULL CHECK(method_version='creator-work-research.v1'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TABLE linggan_creator_research_result (
 request_ref uuid PRIMARY KEY REFERENCES linggan_creator_research_request,
 invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
 result jsonb,
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK(result IS NULL OR jsonb_typeof(result)='object'),
 CHECK((result IS NOT NULL)=(failure_code IS NULL))
);
CREATE TRIGGER creator_research_request_immutable BEFORE UPDATE OR DELETE ON linggan_creator_research_request
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER creator_research_result_immutable BEFORE UPDATE OR DELETE ON linggan_creator_research_result
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
INSERT INTO linggan_ai_failure_code VALUES
 ('creator_publication_withheld',NULL,false,'来源、授权或任务状态已变化，结果不发布','executor_v1');
