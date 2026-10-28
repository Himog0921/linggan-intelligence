-- Remove only the identified pre-2026-09-22 unclassified invocation history.
-- The evidence, creation, model configuration and all other invocation records are protected.
BEGIN;
SET LOCAL search_path=public;
SET LOCAL lock_timeout='5s';
SET LOCAL statement_timeout='120s';
LOCK TABLE linggan_model_invocation IN ACCESS EXCLUSIVE MODE;
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM linggan_model_invocation WHERE state='running') THEN
  RAISE EXCEPTION 'active_model_invocation';
 END IF;
 IF EXISTS (SELECT 1 FROM linggan_model_invocation WHERE task_key='legacy_unclassified'
            AND created_at >= '2026-09-22T00:00:00Z'::timestamptz) THEN
  RAISE EXCEPTION 'unreviewed_later_unclassified_invocation';
 END IF;
END $$;
CREATE TEMP TABLE protected_invocation_cleanup (table_name text PRIMARY KEY, row_count bigint, row_hash numeric) ON COMMIT DROP;
DO $$ DECLARE n text; c bigint; h numeric; BEGIN
 FOREACH n IN ARRAY ARRAY['linggan_material_content','linggan_material_content_detail','linggan_material_comment',
  'linggan_material_comment_restriction','linggan_media_blob','linggan_media_materialization',
  'linggan_material_media_origin','linggan_material_unit','linggan_material_reading',
  'linggan_material_reading_selection','linggan_runtime_capture_package','linggan_model_connection',
  'linggan_model_connection_version','linggan_model_config','linggan_ai_task_config',
  'linggan_ops_publication_material_projection','linggan_creation_ai_request','linggan_creation_adoption'] LOOP
  EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric),0) FROM %I t',n) INTO c,h;
  INSERT INTO protected_invocation_cleanup VALUES(n,c,h);
 END LOOP;
END $$;
CREATE TEMP TABLE protected_other_invocations ON COMMIT DROP AS
 SELECT count(*) row_count,COALESCE(sum(hashtextextended(to_jsonb(i)::text,0)::numeric),0) row_hash
 FROM linggan_model_invocation i WHERE task_key IS DISTINCT FROM 'legacy_unclassified';
ALTER TABLE linggan_model_invocation DISABLE TRIGGER ai_invocation_guard;
DELETE FROM linggan_model_invocation WHERE task_key='legacy_unclassified' AND created_at<'2026-09-22T00:00:00Z'::timestamptz;
ALTER TABLE linggan_model_invocation ENABLE TRIGGER ai_invocation_guard;
DO $$ DECLARE r record; c bigint; h numeric; BEGIN
 FOR r IN SELECT * FROM protected_invocation_cleanup LOOP
  EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric),0) FROM %I t',r.table_name) INTO c,h;
  IF c<>r.row_count OR h<>r.row_hash THEN RAISE EXCEPTION 'protected_data_changed: %',r.table_name; END IF;
 END LOOP;
 SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(i)::text,0)::numeric),0) INTO c,h
  FROM linggan_model_invocation i WHERE task_key IS DISTINCT FROM 'legacy_unclassified';
 IF NOT EXISTS (SELECT 1 FROM protected_other_invocations WHERE row_count=c AND row_hash=h) THEN
  RAISE EXCEPTION 'other_task_invocations_changed';
 END IF;
END $$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
