-- Manual, work-scoped cloud OCR. Creation freezes local image identities only; it never starts a worker.
INSERT INTO linggan_ai_task(task_key,label,lifecycle,automatic_brake,requires_skill,skill_allowed,handler_contract)
 VALUES('ocr_recognition','图片文字识别','active',false,false,false,'ocr-recognition.v1');
INSERT INTO linggan_ai_control(scope_key,task_key) VALUES('task:ocr_recognition','ocr_recognition');
WITH config AS (INSERT INTO linggan_ai_task_config(task_config_ref,task_key,execution_contract,material_columns,budget_mode,origin)
 VALUES(gen_random_uuid(),'ocr_recognition','ocr-recognition.v1','[]','unconfigured','seed') RETURNING task_config_ref)
 INSERT INTO linggan_ai_task_setting(task_key,current_config_ref) SELECT 'ocr_recognition',task_config_ref FROM config;
INSERT INTO linggan_skill_binding(task_key) VALUES('ocr_recognition');
INSERT INTO linggan_ai_task_round_state(task_key) VALUES('ocr_recognition');
INSERT INTO linggan_ai_failure_code VALUES
 ('ocr_visual_schema_invalid',true,true,'图片文字识别返回内容未通过版本化视觉合同','executor_v1'),
 ('ocr_provider_failed',NULL,true,'图片文字识别供应商调用失败，是否已发出须由回执核对','executor_v1'),
 ('ocr_admission_denied',false,false,'图片文字识别在派发前被当前控制、范围或额度拒绝','executor_v1'),
 ('ocr_prior_item_failed',false,false,'同一作品前一张图片失败，后续图片没有派发','executor_v1');

-- Manual image egress has its own explicit scope.  A task configuration or a
-- local media record never grants cloud egress by itself.
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
 OR (task_key='ocr_quality_audit' AND material_class IN ('synthetic_image_probe','ocr_quality_image'))
 OR (task_key='ocr_recognition' AND material_class='manual_ocr_image'));
DO $$ DECLARE name text; BEGIN
 FOR name IN SELECT conname FROM pg_constraint WHERE conrelid='linggan_ai_batch'::regclass AND contype='c'
  AND pg_get_constraintdef(oid) LIKE '%task_key%' LOOP
  EXECUTE format('ALTER TABLE linggan_ai_batch DROP CONSTRAINT %I',name);
 END LOOP;
END $$;
ALTER TABLE linggan_ai_batch ADD CONSTRAINT ai_batch_task_kind_valid CHECK(
 (task_key='model_probe' AND kind='probe') OR (task_key IN('writing_legacy','writing_assistant','direction_list_draft','direction_proposal') AND kind IN('interactive','trial'))
 OR (task_key IN('note_interpretation','author_org_identity','ocr_quality_audit','ocr_recognition') AND kind<>'probe'));

CREATE TABLE linggan_ocr_recognition_run(
 run_ref uuid PRIMARY KEY,command_ref uuid NOT NULL UNIQUE,content_public_ref uuid NOT NULL,
 task_config_ref uuid NOT NULL REFERENCES linggan_ai_task_config, batch_ref uuid REFERENCES linggan_ai_batch, state text NOT NULL CHECK(state IN('queued','running','finalizing','succeeded','stopped','unknown')),
 model_label text, endpoint_url text, scope_sha256 text NOT NULL CHECK(scope_sha256~'^[0-9a-f]{64}$'), total_images integer NOT NULL CHECK(total_images>0), completed_images integer NOT NULL DEFAULT 0 CHECK(completed_images>=0), failure_code text, created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now());
CREATE UNIQUE INDEX ocr_recognition_open_work ON linggan_ocr_recognition_run(content_public_ref) WHERE state IN('queued','running','finalizing','unknown');
CREATE TABLE linggan_ocr_recognition_item(
 item_ref uuid PRIMARY KEY,run_ref uuid NOT NULL REFERENCES linggan_ocr_recognition_run,position integer NOT NULL CHECK(position>=0),
 slot_key text NOT NULL,blob_sha256 text NOT NULL CHECK(blob_sha256~'^[0-9a-f]{64}$'),materialization_ref uuid NOT NULL REFERENCES linggan_media_materialization,ordinal integer,role text NOT NULL CHECK(role IN('cover','page','unknown')),
 state text NOT NULL CHECK(state IN('pending','running','succeeded','failed','unattempted','unknown')),
 original_url text NOT NULL CHECK(original_url ~ '^/api/local/media/' AND original_url !~ '[[:space:]]'),storage_key text NOT NULL,mime_type text NOT NULL,byte_size bigint NOT NULL CHECK(byte_size>0),
 invocation_ref uuid REFERENCES linggan_model_invocation,adoption_ref uuid REFERENCES linggan_media_ocr_adoption,
 raw_result_ref text,raw_result_sha256 text CHECK(raw_result_sha256 IS NULL OR raw_result_sha256~'^[0-9a-f]{64}$'),raw_json text,finish_reason text,
 UNIQUE(run_ref,position),UNIQUE(run_ref,blob_sha256));
CREATE TABLE linggan_ocr_recognition_slot_alias(
 item_ref uuid NOT NULL REFERENCES linggan_ocr_recognition_item,item_slot_key text NOT NULL,
 PRIMARY KEY(item_ref,item_slot_key));
-- The producer chain remains shared with local OCR.  The cloud-manual engine is
-- intentionally distinct from paddleocr; it cannot overwrite a local layout.
ALTER TABLE linggan_media_ocr_layout DROP CONSTRAINT linggan_media_ocr_layout_engine_check;
ALTER TABLE linggan_media_ocr_layout ADD CONSTRAINT linggan_media_ocr_layout_engine_check CHECK(engine IN ('paddleocr','cloud_manual'));
