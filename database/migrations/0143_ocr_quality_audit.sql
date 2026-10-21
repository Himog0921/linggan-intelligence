-- Optional OCR second opinion. No grants, calls, automatic sampling or correction enabled.
INSERT INTO linggan_ai_task(task_key,label,lifecycle,automatic_brake,requires_skill,skill_allowed,handler_contract)
 VALUES('ocr_quality_audit','图片文字质量抽检','active',false,false,false,'ocr-visual-audit.v1');
INSERT INTO linggan_ai_control(scope_key,task_key) VALUES('task:ocr_quality_audit','ocr_quality_audit');
WITH config AS (INSERT INTO linggan_ai_task_config(task_config_ref,task_key,execution_contract,material_columns,budget_mode,origin)
 VALUES(gen_random_uuid(),'ocr_quality_audit','ocr-visual-audit.v1','[]','unconfigured','seed') RETURNING task_config_ref)
 INSERT INTO linggan_ai_task_setting(task_key,current_config_ref) SELECT 'ocr_quality_audit',task_config_ref FROM config;
INSERT INTO linggan_skill_binding(task_key) VALUES('ocr_quality_audit');
INSERT INTO linggan_ai_task_round_state(task_key) VALUES('ocr_quality_audit');
DO $$ DECLARE name text; BEGIN
 FOR name IN SELECT conname FROM pg_constraint WHERE conrelid='linggan_model_egress_event'::regclass AND contype='c'
  AND pg_get_constraintdef(oid) LIKE '%material_class%' LOOP
  EXECUTE format('ALTER TABLE linggan_model_egress_event DROP CONSTRAINT %I',name);
 END LOOP;
END $$;
ALTER TABLE linggan_model_egress_event ADD CONSTRAINT ai_egress_material_task_valid CHECK(
 (task_key='model_probe' AND material_class IN ('synthetic_probe','model_catalog'))
 OR (task_key IN ('note_interpretation','direction_list_draft','direction_proposal') AND material_class='note_material')
 OR (task_key='author_org_identity' AND material_class='public_author_bio')
 OR (task_key IN ('writing_assistant','writing_legacy') AND material_class='selected_writing_material')
 OR (task_key='ocr_quality_audit' AND material_class IN ('synthetic_image_probe','ocr_quality_image')));
DO $$ DECLARE name text; BEGIN
 FOR name IN SELECT conname FROM pg_constraint WHERE conrelid='linggan_ai_batch'::regclass AND contype='c' AND pg_get_constraintdef(oid) LIKE '%task_key%' LOOP
 EXECUTE format('ALTER TABLE linggan_ai_batch DROP CONSTRAINT %I',name); END LOOP;
END $$;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe') OR (task_key IN('writing_legacy','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit') AND kind<>'probe'));
CREATE VIEW linggan_ocr_audit_eligible_source AS
 SELECT a.*,l.content_public_ref,j.slot_key,j.blob_sha256,blob.storage_key,blob.mime_type,blob.byte_size
 FROM linggan_media_ocr_adoption a JOIN linggan_media_ocr_layout l USING(layout_ref)
 JOIN linggan_media_derivative d ON d.derivative_ref=a.derivative_ref
 JOIN linggan_media_processing_job j USING(job_ref) JOIN linggan_media_blob blob ON blob.sha256=j.blob_sha256
 WHERE a.created_at<=scope_001_now() AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition dis
 WHERE dis.state IN ('WITHDRAWN_OR_RESTRICTED','BYTES_CLEANED') AND (dis.blob_sha256=j.blob_sha256 OR dis.slot_key=j.slot_key
 OR dis.derivative_ref=d.derivative_ref OR dis.materialization_ref IN(SELECT materialization_ref FROM linggan_media_materialization WHERE blob_sha256=j.blob_sha256)))
 AND NOT EXISTS(SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref=j.job_ref);
CREATE TABLE linggan_ocr_audit_settings(
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
 settings jsonb NOT NULL CHECK(jsonb_typeof(settings)='object'),updated_at timestamptz NOT NULL DEFAULT scope_001_now());
INSERT INTO linggan_ocr_audit_settings(settings) VALUES('{"automatic":false,"domainRef":null,"randomCount":30,"riskCount":20,"regressionRefs":[],"privacyClearedRefs":[],"privacyClearanceActor":"","privacyClearanceReason":"","regressionCount":10,"maxRequests":60,"maxTokens":600000,"maxCostMicroUsd":2000000,"concurrency":1,"timeoutSeconds":60}');
CREATE TABLE linggan_ocr_audit_run(
 run_ref uuid PRIMARY KEY,run_key text NOT NULL UNIQUE CHECK(char_length(run_key) BETWEEN 1 AND 180),
 seed text NOT NULL,kind text NOT NULL CHECK(kind IN('manual','weekly','visual_probe')),
 settings jsonb NOT NULL,task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config,
 endpoint_ref uuid NOT NULL REFERENCES linggan_ai_endpoint,model_id text NOT NULL,connection_version_ref uuid NOT NULL REFERENCES linggan_model_connection_version,
 price_ref uuid REFERENCES linggan_model_price,prompt_version text NOT NULL,
 inventory jsonb NOT NULL,created_at timestamptz NOT NULL DEFAULT scope_001_now());
CREATE TABLE linggan_ocr_audit_sample(
 sample_ref uuid PRIMARY KEY,run_ref uuid NOT NULL REFERENCES linggan_ocr_audit_run,
 adoption_ref uuid REFERENCES linggan_media_ocr_adoption,pool text NOT NULL CHECK(pool IN('random','risk','regression','probe')),
 local_snapshot jsonb NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(local_snapshot)='object'),
 blob_sha256 text NOT NULL CHECK(blob_sha256~'^[0-9a-f]{64}$'),local_rule_version text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),UNIQUE(run_ref,adoption_ref),CHECK((pool='probe')=(adoption_ref IS NULL)));
CREATE TABLE linggan_ocr_audit_dispatch(
 sample_ref uuid PRIMARY KEY REFERENCES linggan_ocr_audit_sample,invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
 reserved_tokens bigint NOT NULL CHECK(reserved_tokens>0),reserved_cost_micro_usd bigint NOT NULL CHECK(reserved_cost_micro_usd>=0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now());
CREATE TABLE linggan_ocr_audit_observation(
 sample_ref uuid PRIMARY KEY REFERENCES linggan_ocr_audit_sample,invocation_ref uuid NOT NULL UNIQUE REFERENCES linggan_model_invocation,
 result jsonb,vision_qualified boolean NOT NULL DEFAULT false,failure_code text,created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 CHECK((result IS NOT NULL)=(failure_code IS NULL)));
CREATE TABLE linggan_ocr_audit_issue(
 issue_ref uuid PRIMARY KEY,sample_ref uuid NOT NULL REFERENCES linggan_ocr_audit_sample,position integer NOT NULL CHECK(position>=0),
 suggestion jsonb NOT NULL CHECK(jsonb_typeof(suggestion)='object'),created_at timestamptz NOT NULL DEFAULT scope_001_now(),UNIQUE(sample_ref,position));
CREATE TABLE linggan_media_ocr_review(
 review_ref uuid PRIMARY KEY REFERENCES linggan_ai_command DEFERRABLE INITIALLY DEFERRED,
 sample_ref uuid NOT NULL REFERENCES linggan_ocr_audit_sample,issue_ref uuid REFERENCES linggan_ocr_audit_issue,
 adoption_ref uuid NOT NULL REFERENCES linggan_media_ocr_adoption,revision bigint NOT NULL CHECK(revision>0),
 decision text NOT NULL CHECK(decision IN('adopt','reject','pending')),field text NOT NULL CHECK(field IN('adopted_body','cover_headline')),
 before_text text NOT NULL,after_text text,source_kind text CHECK(source_kind IN('ocr_block','manual_transcription')),
 block_ref text,source_start integer CHECK(source_start>=0),source_end integer CHECK(source_end>=source_start),
 actor text NOT NULL CHECK(char_length(btrim(actor)) BETWEEN 1 AND 120),reason text NOT NULL CHECK(char_length(btrim(reason)) BETWEEN 1 AND 300),
 metrics jsonb NOT NULL CHECK(jsonb_typeof(metrics)='object'),created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 UNIQUE(adoption_ref,revision),CHECK((decision='adopt')=(after_text IS NOT NULL)),
 CHECK(decision<>'adopt' OR (source_kind='manual_transcription' AND block_ref IS NULL AND source_start IS NULL AND source_end IS NULL)
 OR (source_kind='ocr_block' AND block_ref IS NOT NULL AND source_start IS NOT NULL AND source_end IS NOT NULL)));
CREATE INDEX ocr_audit_sample_run ON linggan_ocr_audit_sample(run_ref,pool);
CREATE INDEX ocr_audit_issue_sample ON linggan_ocr_audit_issue(sample_ref);
CREATE INDEX ocr_review_adoption ON linggan_media_ocr_review(adoption_ref,field,revision DESC);
CREATE INDEX ocr_audit_run_recent ON linggan_ocr_audit_run(created_at DESC);
DO $$ DECLARE name text; BEGIN
 FOREACH name IN ARRAY ARRAY['linggan_ocr_audit_run','linggan_ocr_audit_sample','linggan_ocr_audit_dispatch','linggan_ocr_audit_observation','linggan_ocr_audit_issue','linggan_media_ocr_review'] LOOP
 EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation()',name||'_immutable',name);
 END LOOP;
END $$;
