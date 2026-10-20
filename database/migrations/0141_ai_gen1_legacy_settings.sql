-- AI-GEN1 / PR-1B. writing_legacy already owns its immutable task configuration.
-- No ledger, legacy result, policy or business table is removed by this cutover.
DO $$
BEGIN
 IF to_regclass('linggan_creation_ai_request') IS NULL OR NOT EXISTS (
  SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema()
   AND table_name='linggan_creation_ai_request' AND column_name='task_config_ref'
 ) THEN
  RAISE EXCEPTION 'ai_settings_cutover_requires_writing_bridge';
 END IF;
END $$;

ALTER TABLE linggan_model_workspace
 DROP COLUMN default_config_ref,
 DROP COLUMN worker_last_seen_at,
 DROP COLUMN worker_state,
 DROP COLUMN worker_last_error;
