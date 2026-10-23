-- Target-scoped content comparison uses the existing AI executor and budget ledger.
-- No task control, egress permission, model or automatic trigger is enabled here.
INSERT INTO linggan_ai_task(task_key,label,lifecycle,automatic_brake,requires_skill,skill_allowed,handler_contract)
 VALUES('work_comparison','作品内容对照','active',false,false,false,'work-content-comparison.v1');
INSERT INTO linggan_ai_control(scope_key,task_key) VALUES('task:work_comparison','work_comparison');
WITH config AS (
 INSERT INTO linggan_ai_task_config(task_config_ref,task_key,execution_contract,material_columns,budget_mode,origin)
 VALUES(gen_random_uuid(),'work_comparison','work-content-comparison.v1',
        '["title","cover_headline","body","tags","ocr","transcript"]','unconfigured','seed')
 RETURNING task_config_ref
) INSERT INTO linggan_ai_task_setting(task_key,current_config_ref)
 SELECT 'work_comparison',task_config_ref FROM config;
INSERT INTO linggan_skill_binding(task_key) VALUES('work_comparison');
INSERT INTO linggan_ai_task_round_state(task_key) VALUES('work_comparison');
ALTER TABLE linggan_ai_batch DROP CONSTRAINT ai_batch_task_kind_valid;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe')
 OR (task_key IN('writing_legacy','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe')
 OR (task_key='work_comparison' AND kind='interactive'));
ALTER TABLE linggan_model_egress_event DROP CONSTRAINT ai_egress_material_task_valid;
ALTER TABLE linggan_model_egress_event ADD CONSTRAINT ai_egress_material_task_valid CHECK(
 (task_key='model_probe' AND material_class IN('synthetic_probe','model_catalog'))
 OR (task_key IN('note_interpretation','direction_list_draft','direction_proposal','work_comparison') AND material_class='note_material')
 OR (task_key='author_org_identity' AND material_class='public_author_bio')
 OR (task_key IN('writing_assistant','writing_legacy') AND material_class='selected_writing_material')
 OR (task_key='ocr_quality_audit' AND material_class IN('synthetic_image_probe','ocr_quality_image'))
 OR (task_key='ocr_recognition' AND material_class='manual_ocr_image'));

-- Only scope, immutable versions and derived results live here. Source bodies remain canonical.
CREATE TABLE linggan_work_comparison_request (
 request_ref uuid PRIMARY KEY,
 command_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 target_ref uuid NOT NULL,
 domain_ref uuid,
 work_refs uuid[] NOT NULL CHECK(cardinality(work_refs) BETWEEN 2 AND 3),
 knowledge_cutoff timestamptz NOT NULL,
 material_revisions jsonb NOT NULL CHECK(jsonb_typeof(material_revisions)='array'),
 input_fingerprint text NOT NULL CHECK(input_fingerprint ~ '^[0-9a-f]{64}$'),
 task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config,
 batch_ref uuid NOT NULL UNIQUE REFERENCES linggan_ai_batch,
 method_version text NOT NULL CHECK(method_version='work-content-comparison.v1'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE INDEX work_comparison_object ON linggan_work_comparison_request(target_ref,domain_ref,created_at DESC);
CREATE TABLE linggan_work_comparison_result (
 request_ref uuid PRIMARY KEY REFERENCES linggan_work_comparison_request,
 invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
 result jsonb,
 failure_code text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK(result IS NULL OR jsonb_typeof(result)='object'),
 CHECK((result IS NOT NULL)=(failure_code IS NULL))
);
CREATE TRIGGER work_comparison_request_immutable BEFORE UPDATE OR DELETE ON linggan_work_comparison_request
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER work_comparison_result_immutable BEFORE UPDATE OR DELETE ON linggan_work_comparison_result
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
INSERT INTO linggan_ai_failure_code VALUES
 ('comparison_source_unavailable',false,false,'作品对照的来源已不可读取，未发起模型调用','executor_v1'),
 ('comparison_source_changed',false,false,'作品对照的冻结来源不再匹配，未发起模型调用','executor_v1'),
 ('comparison_schema_invalid',true,true,'内容对照返回缺少可核验的双侧来源','executor_v1'),
 ('comparison_provider_failed',NULL,true,'内容对照调用失败，是否发出以调用回执为准','executor_v1'),
 ('comparison_publication_withheld',NULL,false,'来源、授权或任务状态已变化，结果不发布','executor_v1');
