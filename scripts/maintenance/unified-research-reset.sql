-- UNIFIED-RESEARCH-AUTOMATION-001: clear legacy research results only.
-- Run in one explicit transaction after isolated rehearsal and current-scope verification.
-- Source Evidence, media, current readings, account/operations data and other tasks' model ledger stay.
BEGIN;
SET LOCAL search_path = public;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '120s';
SET CONSTRAINTS ALL DEFERRED;

DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM linggan_model_invocation WHERE state='running') THEN
  RAISE EXCEPTION 'active_model_invocation';
 END IF;
 IF EXISTS (SELECT 1 FROM linggan_creator_discovery_work_analysis WHERE manual_overrides IS NOT NULL AND manual_overrides <> '{}'::jsonb)
 OR EXISTS (SELECT 1 FROM linggan_creator_discovery_author_analysis WHERE manual_overrides IS NOT NULL AND manual_overrides <> '{}'::jsonb)
 OR EXISTS (SELECT 1 FROM linggan_topic_map_alternative)
 OR EXISTS (SELECT 1 FROM linggan_topic_map_collection_round)
 OR EXISTS (SELECT 1 FROM linggan_topic_map_structure_source)
 OR EXISTS (SELECT 1 FROM linggan_topic_map_structure_destination)
 OR EXISTS (SELECT 1 FROM linggan_topic_import_receipt)
 OR EXISTS (SELECT 1 FROM linggan_topic_classification_run WHERE run_kind <> 'machine_proposed')
 OR EXISTS (SELECT 1 FROM linggan_topic_definition d WHERE NOT EXISTS
   (SELECT 1 FROM linggan_topic_classification_run c WHERE c.definition_ref=d.definition_ref AND c.run_kind='machine_proposed'))
 OR EXISTS (SELECT 1 FROM linggan_comment_study_problem) THEN
  RAISE EXCEPTION 'research_scope_has_unreviewed_human_or_problem_data';
 END IF;
END $$;

CREATE TEMP TABLE protected_research_reset (table_name text PRIMARY KEY, row_count bigint, row_hash numeric) ON COMMIT DROP;
DO $$ DECLARE n text; c bigint; h numeric; BEGIN
 FOREACH n IN ARRAY ARRAY['linggan_material_content','linggan_material_content_detail',
  'linggan_material_comment','linggan_material_comment_restriction','linggan_media_blob',
  'linggan_media_materialization','linggan_material_media_origin','linggan_material_unit',
  'linggan_material_reading','linggan_material_reading_selection','linggan_runtime_capture_package',
  'linggan_model_connection','linggan_model_connection_version',
  'linggan_model_config','linggan_ai_task_config','linggan_ops_publication_material_projection'] LOOP
   EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric),0) FROM %I t',n) INTO c,h;
   INSERT INTO protected_research_reset VALUES(n,c,h);
 END LOOP;
END $$;
CREATE TEMP TABLE protected_other_invocations ON COMMIT DROP AS
 SELECT count(*) row_count,COALESCE(sum(hashtextextended(to_jsonb(i)::text,0)::numeric),0) row_hash
 FROM linggan_model_invocation i
 WHERE task_key IS NULL OR task_key NOT IN ('comment_study','legacy_creator_discovery','legacy_topic_map');

ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_frozen_guard;
ALTER TABLE linggan_comment_study_start_request DISABLE TRIGGER cs_start_immutable;
ALTER TABLE linggan_comment_study_model_request DISABLE TRIGGER cs_request_immutable;
ALTER TABLE linggan_comment_study_policy DISABLE TRIGGER cs_policy_immutable;
ALTER TABLE linggan_topic_workspace DISABLE TRIGGER linggan_topic_workspace_is_append_only;
ALTER TABLE linggan_topic_definition DISABLE TRIGGER linggan_topic_definition_is_append_only;
ALTER TABLE linggan_topic_classification_run DISABLE TRIGGER linggan_topic_classification_run_is_append_only;
ALTER TABLE linggan_topic_material_member DISABLE TRIGGER linggan_topic_material_member_is_append_only;
ALTER TABLE linggan_topic_material_pack DISABLE TRIGGER linggan_topic_material_pack_is_append_only;
ALTER TABLE linggan_topic_map_binding DISABLE TRIGGER linggan_topic_map_binding_append_only;
ALTER TABLE linggan_topic_map_membership DISABLE TRIGGER topic_map_membership_append_only;
ALTER TABLE linggan_topic_map_work_annotation DISABLE TRIGGER linggan_topic_map_work_annotation_append_only;
ALTER TABLE linggan_topic_map_research_result DISABLE TRIGGER topic_map_research_result_immutable;
ALTER TABLE linggan_topic_map_concept_rule DISABLE TRIGGER topic_map_concept_rule_immutable;
ALTER TABLE linggan_topic_map_discussion_unit DISABLE TRIGGER topic_map_discussion_unit_immutable;
ALTER TABLE linggan_topic_map_unit_resolution DISABLE TRIGGER topic_map_unit_resolution_immutable;
ALTER TABLE linggan_topic_map_unit_assignment DISABLE TRIGGER topic_map_unit_assignment_immutable;
ALTER TABLE linggan_topic_map_concept_relation DISABLE TRIGGER topic_map_concept_relation_immutable;
ALTER TABLE linggan_model_invocation DISABLE TRIGGER ai_invocation_guard;

DELETE FROM linggan_comment_study_pool_cursor;
DELETE FROM linggan_comment_study_model_request;
DELETE FROM linggan_comment_study_problem_membership;
DELETE FROM linggan_comment_study_problem_pair;
DELETE FROM linggan_comment_study_resolution;
DELETE FROM linggan_comment_study_signal;
DELETE FROM linggan_comment_study_semantic_attempt;
DELETE FROM linggan_comment_study_batch_target;
DELETE FROM linggan_comment_study_target;
DELETE FROM linggan_comment_study_batch;
DELETE FROM linggan_comment_study_work;
DELETE FROM linggan_comment_study_start_request;
DELETE FROM linggan_comment_study_run;
DELETE FROM linggan_comment_study_active_policy;
DELETE FROM linggan_comment_study_policy;
DELETE FROM linggan_comment_study_clean_cache;
DELETE FROM linggan_comment_study_embedding_cache;
DELETE FROM linggan_creator_discovery_work_analysis;
DELETE FROM linggan_creator_discovery_author_analysis;
DELETE FROM linggan_creator_discovery_policy;

DELETE FROM linggan_topic_map_concept_relation;
DELETE FROM linggan_topic_map_unit_assignment;
DELETE FROM linggan_topic_map_unit_resolution;
DELETE FROM linggan_topic_map_discussion_unit;
DELETE FROM linggan_topic_map_backfill_queue;
DELETE FROM linggan_topic_map_definition_scan;
DELETE FROM linggan_topic_map_concept_rule;
DELETE FROM linggan_topic_map_work_annotation;
DELETE FROM linggan_topic_map_membership;
DELETE FROM linggan_topic_map_binding;
DELETE FROM linggan_topic_map_research_result;
DELETE FROM linggan_topic_map_research_request;
DELETE FROM linggan_topic_map_research_task;
DELETE FROM linggan_topic_map_research_run;
DELETE FROM linggan_topic_map_research_command;
DELETE FROM linggan_topic_map_research_policy;
DELETE FROM linggan_topic_material_member;
DELETE FROM linggan_topic_material_pack;
DELETE FROM linggan_topic_classification_run;
DELETE FROM linggan_topic_definition;
DELETE FROM linggan_topic_workspace;
DELETE FROM linggan_model_invocation WHERE task_key IN ('comment_study','legacy_creator_discovery','legacy_topic_map');

ALTER TABLE linggan_comment_study_run ENABLE TRIGGER cs_run_frozen_guard;
ALTER TABLE linggan_comment_study_start_request ENABLE TRIGGER cs_start_immutable;
ALTER TABLE linggan_comment_study_model_request ENABLE TRIGGER cs_request_immutable;
ALTER TABLE linggan_comment_study_policy ENABLE TRIGGER cs_policy_immutable;
ALTER TABLE linggan_topic_workspace ENABLE TRIGGER linggan_topic_workspace_is_append_only;
ALTER TABLE linggan_topic_definition ENABLE TRIGGER linggan_topic_definition_is_append_only;
ALTER TABLE linggan_topic_classification_run ENABLE TRIGGER linggan_topic_classification_run_is_append_only;
ALTER TABLE linggan_topic_material_member ENABLE TRIGGER linggan_topic_material_member_is_append_only;
ALTER TABLE linggan_topic_material_pack ENABLE TRIGGER linggan_topic_material_pack_is_append_only;
ALTER TABLE linggan_topic_map_binding ENABLE TRIGGER linggan_topic_map_binding_append_only;
ALTER TABLE linggan_topic_map_membership ENABLE TRIGGER topic_map_membership_append_only;
ALTER TABLE linggan_topic_map_work_annotation ENABLE TRIGGER linggan_topic_map_work_annotation_append_only;
ALTER TABLE linggan_topic_map_research_result ENABLE TRIGGER topic_map_research_result_immutable;
ALTER TABLE linggan_topic_map_concept_rule ENABLE TRIGGER topic_map_concept_rule_immutable;
ALTER TABLE linggan_topic_map_discussion_unit ENABLE TRIGGER topic_map_discussion_unit_immutable;
ALTER TABLE linggan_topic_map_unit_resolution ENABLE TRIGGER topic_map_unit_resolution_immutable;
ALTER TABLE linggan_topic_map_unit_assignment ENABLE TRIGGER topic_map_unit_assignment_immutable;
ALTER TABLE linggan_topic_map_concept_relation ENABLE TRIGGER topic_map_concept_relation_immutable;
ALTER TABLE linggan_model_invocation ENABLE TRIGGER ai_invocation_guard;

DO $$ DECLARE r record; c bigint; h numeric; BEGIN
 FOR r IN SELECT * FROM protected_research_reset LOOP
  EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric),0) FROM %I t',r.table_name) INTO c,h;
  IF c<>r.row_count OR h<>r.row_hash THEN RAISE EXCEPTION 'protected_data_changed: %',r.table_name; END IF;
 END LOOP;
 SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(i)::text,0)::numeric),0) INTO c,h
 FROM linggan_model_invocation i
 WHERE task_key IS NULL OR task_key NOT IN ('comment_study','legacy_creator_discovery','legacy_topic_map');
 IF NOT EXISTS (SELECT 1 FROM protected_other_invocations WHERE row_count=c AND row_hash=h) THEN
  RAISE EXCEPTION 'other_task_invocations_changed';
 END IF;
END $$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
