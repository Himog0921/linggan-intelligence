-- DOSSIER-OCR-CLEANUP-20261026, explicitly authorized development-data deletion.
-- KEEP latest local Paddle/local-v4, local-vl-v1, and current DeepSeek OCR capability.
-- DELETE every historical machine OCR result and its proven derived research closure.
-- No archive or backup; raw platform material, originals, ASR and human content stay.
-- The normal migrator owns the single transaction and requires the runtime stopped.
SET LOCAL lock_timeout='5s';
SET LOCAL statement_timeout='60s';
-- Flush any earlier initialization events before changing named trigger guards.
SET CONSTRAINTS ALL IMMEDIATE;
SET CONSTRAINTS ALL DEFERRED;

CREATE TEMP TABLE _ocr_purge_jobs ON COMMIT DROP AS
 SELECT job_ref FROM linggan_media_processing_job
 WHERE processor_kind IN ('image_ocr','video_frame_ocr');
CREATE TEMP TABLE _ocr_purge_derivatives ON COMMIT DROP AS
 SELECT derivative_ref FROM linggan_media_derivative
 WHERE job_ref IN (SELECT job_ref FROM _ocr_purge_jobs);
CREATE TEMP TABLE _ocr_purge_layouts ON COMMIT DROP AS
 SELECT layout_ref FROM linggan_media_ocr_layout
 WHERE ocr_derivative_ref IN (SELECT derivative_ref FROM _ocr_purge_derivatives);
CREATE TEMP TABLE _ocr_purge_readings ON COMMIT DROP AS
 SELECT reading_ref FROM linggan_material_reading
 WHERE producer_kind IN ('ordinary_ocr','local_vision','cloud_vision','frame_ocr')
    OR (producer_kind<>'human_edit' AND derivative_ref IN (SELECT derivative_ref FROM _ocr_purge_derivatives));
CREATE TEMP TABLE _ocr_purge_units ON COMMIT DROP AS
 SELECT u.unit_ref FROM linggan_material_unit u
 WHERE u.kind IN ('image','video_frame')
 AND NOT EXISTS(SELECT 1 FROM linggan_material_reading r
                WHERE r.unit_ref=u.unit_ref AND r.producer_kind='human_edit');
CREATE TEMP TABLE _ocr_purge_reading_commands ON COMMIT DROP AS
 SELECT command_ref FROM linggan_material_reading_command
 WHERE unit_ref IN (SELECT unit_ref FROM _ocr_purge_units)
 OR candidate_reading_ref IN (SELECT reading_ref FROM _ocr_purge_readings)
 OR expected_reading_ref IN (SELECT reading_ref FROM _ocr_purge_readings)
 OR resulting_reading_ref IN (SELECT reading_ref FROM _ocr_purge_readings);
CREATE TEMP TABLE _ocr_purge_refs ON COMMIT DROP AS
 SELECT derivative_ref::text ref FROM _ocr_purge_derivatives
 UNION SELECT reading_ref::text FROM _ocr_purge_readings
 UNION SELECT layout_ref::text FROM _ocr_purge_layouts;

-- Typed UUID in frozen JSON is positive evidence; do not use loose word 'ocr' as a purge key.
-- Comment Study is separately initialized; an installation without it has no such derived rows.
DO $$ BEGIN
 IF to_regclass('linggan_comment_study_run') IS NOT NULL AND to_regclass('linggan_comment_study_work') IS NULL THEN RAISE EXCEPTION 'ocr_purge_comment_schema_incomplete'; END IF;
 IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
CREATE TEMP TABLE _ocr_purge_comment_runs ON COMMIT DROP AS
 SELECT DISTINCT w.run_ref FROM linggan_comment_study_work w
 WHERE EXISTS(SELECT 1 FROM jsonb_path_query(w.context_manifest,'$.** ? (@.type() == "string")') v
              WHERE v#>>'{}' IN(SELECT ref FROM _ocr_purge_refs))
 UNION SELECT DISTINCT b.run_ref FROM linggan_comment_study_batch b
 WHERE EXISTS(SELECT 1 FROM jsonb_path_query(b.input_manifest,'$.** ? (@.type() == "string")') v
              WHERE v#>>'{}' IN(SELECT ref FROM _ocr_purge_refs))
 UNION SELECT DISTINCT t.run_ref FROM linggan_comment_study_target t
 WHERE EXISTS(SELECT 1 FROM jsonb_path_query(t.input_manifest,'$.** ? (@.type() == "string")') v
              WHERE v#>>'{}' IN(SELECT ref FROM _ocr_purge_refs));
CREATE TEMP TABLE _ocr_purge_comment_targets ON COMMIT DROP AS
 SELECT target_ref FROM linggan_comment_study_target
 WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs);
CREATE TEMP TABLE _ocr_purge_comment_batches ON COMMIT DROP AS
 SELECT batch_ref FROM linggan_comment_study_batch
 WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs);
CREATE TEMP TABLE _ocr_purge_comment_attempts ON COMMIT DROP AS
 SELECT attempt_ref FROM linggan_comment_study_semantic_attempt
 WHERE target_ref IN(SELECT target_ref FROM _ocr_purge_comment_targets)
 OR batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches);
CREATE TEMP TABLE _ocr_purge_comment_signals ON COMMIT DROP AS
 SELECT signal_ref FROM linggan_comment_study_signal
 WHERE target_ref IN(SELECT target_ref FROM _ocr_purge_comment_targets)
 OR semantic_attempt_ref IN(SELECT attempt_ref FROM _ocr_purge_comment_attempts);
CREATE TEMP TABLE _ocr_purge_comment_pairs ON COMMIT DROP AS
 SELECT pair_ref FROM linggan_comment_study_problem_pair
 WHERE first_signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals)
 OR second_signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals);
CREATE TEMP TABLE _ocr_purge_comment_resolutions ON COMMIT DROP AS
 SELECT resolution_ref FROM linggan_comment_study_resolution
 WHERE signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals);

-- Freeze all comment invocations BEFORE deleting any owner. These are NOT OCR
-- task invocations: current 398 (266 succeeded,132 failed), task=comment_study.
CREATE TEMP TABLE _ocr_purge_comment_requests ON COMMIT DROP AS
 SELECT invocation_ref FROM linggan_comment_study_model_request
 WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs)
 OR batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches)
 OR pair_ref IN(SELECT pair_ref FROM _ocr_purge_comment_pairs)
 OR resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions);
CREATE TEMP TABLE _ocr_purge_comment_invocations ON COMMIT DROP AS
 SELECT model_invocation_ref invocation_ref FROM linggan_comment_study_batch
 WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches) AND model_invocation_ref IS NOT NULL
 UNION SELECT model_invocation_ref FROM linggan_comment_study_semantic_attempt
 WHERE attempt_ref IN(SELECT attempt_ref FROM _ocr_purge_comment_attempts) AND model_invocation_ref IS NOT NULL
 UNION SELECT model_invocation_ref FROM linggan_comment_study_resolution
 WHERE resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions) AND model_invocation_ref IS NOT NULL
 UNION SELECT model_invocation_ref FROM linggan_comment_study_problem_pair
 WHERE pair_ref IN(SELECT pair_ref FROM _ocr_purge_comment_pairs) AND model_invocation_ref IS NOT NULL
 UNION SELECT invocation_ref FROM _ocr_purge_comment_requests;

ELSE
CREATE TEMP TABLE _ocr_purge_comment_runs(run_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_targets(target_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_batches(batch_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_attempts(attempt_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_signals(signal_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_pairs(pair_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_resolutions(resolution_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_requests(invocation_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE _ocr_purge_comment_invocations(invocation_ref uuid) ON COMMIT DROP;
END IF; END $$;

CREATE TEMP TABLE _ocr_purge_batches ON COMMIT DROP AS
 SELECT batch_ref FROM linggan_ai_batch WHERE task_key IN('ocr_recognition','ocr_quality_audit');
CREATE TEMP TABLE _ocr_purge_items ON COMMIT DROP AS
 SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches);
CREATE TEMP TABLE _ocr_purge_invocations ON COMMIT DROP AS
 SELECT invocation_ref FROM linggan_model_invocation WHERE task_key IN('ocr_recognition','ocr_quality_audit')
 OR batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches)
 UNION SELECT invocation_ref FROM linggan_ocr_recognition_item WHERE invocation_ref IS NOT NULL
 UNION SELECT invocation_ref FROM linggan_ocr_audit_dispatch
 UNION SELECT invocation_ref FROM linggan_ocr_audit_observation;
-- Common ledger deletion uses this UNION; OCR task/config/round selection remains
-- restricted to _ocr_purge_invocations. Current union=536, no overlap observed.
CREATE TEMP TABLE _ocr_purge_all_invocations ON COMMIT DROP AS
 SELECT invocation_ref FROM _ocr_purge_invocations WHERE invocation_ref IS NOT NULL
 UNION SELECT invocation_ref FROM _ocr_purge_comment_invocations;
CREATE TEMP TABLE _ocr_purge_rounds ON COMMIT DROP AS
 SELECT round_ref FROM linggan_ai_round WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches);
CREATE TEMP TABLE _ocr_purge_comparisons ON COMMIT DROP AS
 SELECT comparison_ref FROM linggan_ai_trial_comparison
 WHERE baseline_batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches)
 OR candidate_batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches);
CREATE TEMP TABLE _ocr_purge_ai_commands ON COMMIT DROP AS
 SELECT command_ref FROM linggan_ocr_recognition_run
 UNION SELECT command_ref FROM linggan_ai_batch WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches)
 UNION SELECT comparison_ref FROM _ocr_purge_comparisons
 UNION SELECT command_ref FROM linggan_ai_command
 WHERE operation IN('ocr_recognition_start','ocr_audit_settings')
 OR (operation='batch_cancel' AND payload->>'batchRef' IN(SELECT batch_ref::text FROM _ocr_purge_batches));
CREATE TEMP TABLE _ocr_purge_snapshots ON COMMIT DROP AS
 SELECT DISTINCT snapshot_ref FROM linggan_dossier_fragment
 WHERE source_ref::text IN(SELECT ref FROM _ocr_purge_refs)
 OR split_part(logical_position,'.',2) IN(SELECT reading_ref::text FROM _ocr_purge_readings);

-- Current production observations: 12 comment runs/66 works/259 batches/1000 targets;
-- 108 OCR AI batches/166 batch items/138 invocation rows. Empty fresh DB is valid.
-- Do not hard-code those totals into an all-environments migration; compare explicit
-- production preview totals immediately before approved execution instead.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM linggan_model_invocation WHERE state='running') THEN
  RAISE EXCEPTION 'ocr_purge_requires_no_running_invocation'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_ai_executor_instance WHERE state IN('running','draining')
           AND heartbeat_at>clock_timestamp()-interval '30 seconds') THEN
  RAISE EXCEPTION 'ocr_purge_requires_executor_drained'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_material_reading h WHERE h.producer_kind='human_edit'
           AND (h.based_on_reading_ref IN(SELECT reading_ref FROM _ocr_purge_readings)
                OR h.derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives))) THEN
  RAISE EXCEPTION 'human_reading_depends_on_machine_requires_explicit_detach'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_material_reading_command c
           JOIN linggan_material_reading h ON h.reading_ref=c.resulting_reading_ref
           WHERE c.command_ref IN(SELECT command_ref FROM _ocr_purge_reading_commands)
           AND h.producer_kind='human_edit') THEN
  RAISE EXCEPTION 'human_edit_command_would_be_deleted'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_material_reading r
           WHERE r.unit_ref IN(SELECT unit_ref FROM _ocr_purge_units)
           AND r.reading_ref NOT IN(SELECT reading_ref FROM _ocr_purge_readings)) THEN
  RAISE EXCEPTION 'non_machine_reading_in_purge_unit'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_material_unit u
           WHERE u.current_reading_ref IN(SELECT reading_ref FROM _ocr_purge_readings)
           AND u.unit_ref NOT IN(SELECT unit_ref FROM _ocr_purge_units)) THEN
  RAISE EXCEPTION 'retained_unit_points_at_purged_machine'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_media_processing_work w
           WHERE w.reading_base_ref IN(SELECT reading_ref FROM _ocr_purge_readings)
           AND w.job_ref NOT IN(SELECT job_ref FROM _ocr_purge_jobs)) THEN
  RAISE EXCEPTION 'non_ocr_work_references_purge_reading'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_media_derivative d
           WHERE d.derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives)
           AND d.derivative_kind NOT IN('ocr_text','frame_ocr_text')) THEN
  RAISE EXCEPTION 'non_ocr_derivative_in_purge_jobs'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_model_invocation
           WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_invocations)
           AND task_key NOT IN('ocr_recognition','ocr_quality_audit')) THEN
  RAISE EXCEPTION 'non_ocr_invocation_in_purge_scope'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_skill_binding WHERE trial_comparison_ref IN(SELECT comparison_ref FROM _ocr_purge_comparisons))
 OR EXISTS(SELECT 1 FROM linggan_skill_binding_event WHERE trial_comparison_ref IN(SELECT comparison_ref FROM _ocr_purge_comparisons)) THEN
  RAISE EXCEPTION 'active_skill_depends_on_purged_trial'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_ai_task_round_state WHERE task_key NOT IN('ocr_recognition','ocr_quality_audit')
           AND last_round_ref IN(SELECT round_ref FROM _ocr_purge_rounds)) THEN
  RAISE EXCEPTION 'non_ocr_task_points_at_purged_round'; END IF;
END $$;

-- Exact incoming FK guards: retained owners cannot point to any purged ledger.
-- The five comment owners are allowed ONLY inside their already-frozen closure.
DO $$ BEGIN
 IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
 IF EXISTS(SELECT 1 FROM linggan_comment_study_batch WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations) AND batch_ref NOT IN(SELECT batch_ref FROM _ocr_purge_comment_batches))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_semantic_attempt WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations) AND attempt_ref NOT IN(SELECT attempt_ref FROM _ocr_purge_comment_attempts))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_problem_revision WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_resolution WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations) AND resolution_ref NOT IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_comparison WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_problem_pair WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations) AND pair_ref NOT IN(SELECT pair_ref FROM _ocr_purge_comment_pairs))
 OR EXISTS(SELECT 1 FROM linggan_comment_study_model_request WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations) AND invocation_ref NOT IN(SELECT invocation_ref FROM _ocr_purge_comment_requests))
 THEN RAISE EXCEPTION 'retained_comment_owner_references_purged_invocation'; END IF;
 END IF;
 IF EXISTS(SELECT 1 FROM linggan_creator_discovery_work_analysis WHERE result_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_creator_discovery_author_analysis WHERE result_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_topic_map_research_request WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_topic_map_research_result WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_topic_map_concept_rule WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_topic_map_unit_resolution WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_creation_adoption WHERE model_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_creation_ai_request WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_creation_writing_qualification WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_model_capability WHERE probe_invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations))
 OR EXISTS(SELECT 1 FROM linggan_work_comparison_result WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations)) THEN
  RAISE EXCEPTION 'retained_owner_references_purged_invocation'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_model_invocation
           WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_comment_invocations)
           AND task_key<>'comment_study') THEN
  RAISE EXCEPTION 'unexpected_task_in_comment_invocation_closure'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_ai_batch_item_control
           WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations)
           AND item_ref NOT IN(SELECT item_ref FROM _ocr_purge_items)) THEN
  RAISE EXCEPTION 'retained_item_control_references_purged_invocation'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_model_invocation
           WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_comment_invocations)
           AND batch_ref IS NOT NULL AND batch_ref NOT IN(SELECT batch_ref FROM _ocr_purge_batches)) THEN
  RAISE EXCEPTION 'comment_invocation_owns_unscoped_common_batch'; END IF;
 IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
 -- UUID-array semantic seed is not an FK, so protect it separately.
 IF EXISTS(SELECT 1 FROM linggan_comment_study_problem_revision
           WHERE seed_signal_refs && ARRAY(SELECT signal_ref FROM _ocr_purge_comment_signals)) THEN
  RAISE EXCEPTION 'retained_problem_revision_uses_purged_signal'; END IF;
 -- Existing pool/revision data has zero such refs; do not delete shared human/problem
 -- assets on speculation if a future/fixture row exposes a wider closure.
 IF EXISTS(SELECT 1 FROM linggan_comment_study_problem_pair
           WHERE pair_ref IN(SELECT pair_ref FROM _ocr_purge_comment_pairs) AND created_problem_ref IS NOT NULL)
 OR EXISTS(SELECT 1 FROM linggan_comment_study_resolution
           WHERE resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions) AND resolved_problem_ref IS NOT NULL) THEN
  RAISE EXCEPTION 'comment_closure_has_problem_output_requires_explicit_review'; END IF;
 END IF;
END $$;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM linggan_creation_ai_request WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches)
           OR item_ref IN(SELECT item_ref FROM _ocr_purge_items))
 OR EXISTS(SELECT 1 FROM linggan_work_comparison_request WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches)) THEN
  RAISE EXCEPTION 'non_ocr_owner_references_ocr_batch'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_ai_task_config WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_ai_control WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_ai_control_event WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_skill_binding WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_skill_binding_event WHERE event_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_skill_import WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands))
 OR EXISTS(SELECT 1 FROM linggan_work_comparison_request WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands)) THEN
  RAISE EXCEPTION 'retained_configuration_owns_purge_command'; END IF;
END $$;

-- Before trigger disabling, capture scalar count+digest (not source contents) for
-- raw material/media/origins/captures, platform/human readings, ASR/audio/thumbnail
-- jobs+derivatives, non-OCR retirement rows, non-OCR invocations/batches, current
-- DeepSeek OCR task config/model config/connection, and raw Comment tables.
-- Parent should reuse its existing protected-table digest helper and assert exact
-- equality at the end. Do not digest/copy source rows into a backup table.

CREATE TEMP TABLE _ocr_keep_counts(table_name text PRIMARY KEY,row_count bigint,row_digest text) ON COMMIT DROP;
DO $$ DECLARE name text; n bigint; digest text; BEGIN
 FOREACH name IN ARRAY ARRAY['linggan_material_content','linggan_material_content_detail','linggan_material_comment','linggan_media_blob','linggan_media_materialization','linggan_material_media_origin','linggan_runtime_capture_package','linggan_model_connection','linggan_model_connection_version','linggan_model_entry','linggan_model_config'] LOOP
  EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric)::text,''0'') FROM %I t',name) INTO n,digest;
  INSERT INTO _ocr_keep_counts VALUES(name,n,digest);
 END LOOP;
END $$;
CREATE TEMP TABLE _ocr_keep_readings ON COMMIT DROP AS
 SELECT count(*) row_count,COALESCE(sum(hashtextextended(to_jsonb(r)::text,0)::numeric)::text,'0') row_digest
 FROM linggan_material_reading r WHERE producer_kind IN('platform','human_edit');

-- Only the exact known DELETE guards of affected tables; no DISABLE TRIGGER ALL,
-- session_replication_role, or global forbid-function replacement.
ALTER TABLE linggan_material_derived_text DISABLE TRIGGER linggan_material_derived_text_is_append_only;
ALTER TABLE linggan_media_processing_job DISABLE TRIGGER linggan_media_processing_job_is_append_only;
ALTER TABLE linggan_media_processing_job_event DISABLE TRIGGER linggan_media_processing_job_event_is_append_only;
ALTER TABLE linggan_media_derivative DISABLE TRIGGER linggan_media_derivative_is_append_only;
DO $$ BEGIN IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
ALTER TABLE linggan_comment_study_run DISABLE TRIGGER cs_run_frozen_guard;
ALTER TABLE linggan_comment_study_start_request DISABLE TRIGGER cs_start_immutable;
ALTER TABLE linggan_comment_study_model_request DISABLE TRIGGER cs_request_immutable;
END IF; END $$;
ALTER TABLE linggan_ai_command DISABLE TRIGGER ai_command_immutable;
ALTER TABLE linggan_ai_batch DISABLE TRIGGER ai_batch_immutable;
ALTER TABLE linggan_ai_batch_control DISABLE TRIGGER ai_batch_control_guard;
ALTER TABLE linggan_ai_batch_item DISABLE TRIGGER ai_batch_item_immutable;
ALTER TABLE linggan_ai_batch_item_control DISABLE TRIGGER ai_item_control_guard;
ALTER TABLE linggan_ai_trial_comparison DISABLE TRIGGER ai_trial_comparison_immutable;
ALTER TABLE linggan_ai_round DISABLE TRIGGER ai_round_immutable;
ALTER TABLE linggan_model_invocation DISABLE TRIGGER ai_invocation_guard;
ALTER TABLE linggan_ai_execution_attempt DISABLE TRIGGER ai_execution_attempt_immutable;
ALTER TABLE linggan_ai_late_usage_receipt DISABLE TRIGGER ai_late_usage_immutable;
ALTER TABLE linggan_media_ocr_adoption DISABLE TRIGGER linggan_media_ocr_adoption_is_append_only;
ALTER TABLE linggan_ocr_audit_run DISABLE TRIGGER linggan_ocr_audit_run_immutable;
ALTER TABLE linggan_ocr_audit_dispatch DISABLE TRIGGER linggan_ocr_audit_dispatch_immutable;
ALTER TABLE linggan_material_reading DISABLE TRIGGER linggan_material_reading_immutable;
ALTER TABLE linggan_material_reading_selection DISABLE TRIGGER linggan_material_reading_selection_immutable;
ALTER TABLE linggan_material_reading_command DISABLE TRIGGER linggan_material_reading_command_immutable;
ALTER TABLE linggan_dossier_snapshot DISABLE TRIGGER dossier_snapshot_immutable;
ALTER TABLE linggan_dossier_fragment DISABLE TRIGGER dossier_fragment_immutable;
ALTER TABLE linggan_ocr_audit_issue DISABLE TRIGGER linggan_ocr_audit_issue_immutable;
ALTER TABLE linggan_ocr_audit_observation DISABLE TRIGGER linggan_ocr_audit_observation_immutable;
ALTER TABLE linggan_ocr_audit_sample DISABLE TRIGGER linggan_ocr_audit_sample_immutable;
ALTER TABLE linggan_ocr_audit_review DISABLE TRIGGER ocr_audit_review_immutable;

ALTER TABLE linggan_media_resource_relation DISABLE TRIGGER linggan_media_resource_relation_is_append_only;

ALTER TABLE linggan_material_media_disposition_event DISABLE TRIGGER linggan_material_media_disposition_is_append_only;

-- Comment-study leaf-first closure. These are DERIVED run children; raw comments stay.
DO $$ BEGIN IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
DELETE FROM linggan_comment_study_pool_cursor WHERE last_signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals);
DELETE FROM linggan_comment_study_model_request
 WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs)
 OR batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches)
 OR pair_ref IN(SELECT pair_ref FROM _ocr_purge_comment_pairs)
 OR resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions);
DELETE FROM linggan_comment_study_problem_membership
 WHERE signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals)
 OR resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions);
DELETE FROM linggan_comment_study_problem_pair WHERE pair_ref IN(SELECT pair_ref FROM _ocr_purge_comment_pairs);
DELETE FROM linggan_comment_study_resolution WHERE resolution_ref IN(SELECT resolution_ref FROM _ocr_purge_comment_resolutions);
DELETE FROM linggan_comment_study_signal WHERE signal_ref IN(SELECT signal_ref FROM _ocr_purge_comment_signals);
DELETE FROM linggan_comment_study_semantic_attempt WHERE attempt_ref IN(SELECT attempt_ref FROM _ocr_purge_comment_attempts);
DELETE FROM linggan_comment_study_batch_target
 WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches)
 OR target_ref IN(SELECT target_ref FROM _ocr_purge_comment_targets);
DELETE FROM linggan_comment_study_target WHERE target_ref IN(SELECT target_ref FROM _ocr_purge_comment_targets);
DELETE FROM linggan_comment_study_batch WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_comment_batches);
DELETE FROM linggan_comment_study_work WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs);
DELETE FROM linggan_comment_study_start_request WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs);
DELETE FROM linggan_comment_study_run WHERE run_ref IN(SELECT run_ref FROM _ocr_purge_comment_runs);

END IF; END $$;

-- OCR-only audit/recognition histories. Do not delete generic material reading edit API.
DELETE FROM linggan_ocr_audit_review;
DELETE FROM linggan_ocr_audit_issue;
DELETE FROM linggan_ocr_audit_observation;
DELETE FROM linggan_ocr_audit_dispatch;
DELETE FROM linggan_ocr_audit_sample;
DELETE FROM linggan_ocr_audit_run;
DELETE FROM linggan_ocr_recognition_slot_alias;
DELETE FROM linggan_ocr_recognition_item;
DELETE FROM linggan_ocr_recognition_run;
DELETE FROM linggan_dossier_fragment WHERE snapshot_ref IN(SELECT snapshot_ref FROM _ocr_purge_snapshots);
DELETE FROM linggan_dossier_snapshot WHERE snapshot_ref IN(SELECT snapshot_ref FROM _ocr_purge_snapshots);

-- Clear FK children. No UPDATE to a unit that is about to be deleted: such UPDATE
-- would queue material_unit_current_event and fail at commit after the unit is gone.
DELETE FROM linggan_media_processing_work WHERE job_ref IN(SELECT job_ref FROM _ocr_purge_jobs);
DELETE FROM linggan_material_reading_selection
 WHERE unit_ref IN(SELECT unit_ref FROM _ocr_purge_units)
 OR reading_ref IN(SELECT reading_ref FROM _ocr_purge_readings)
 OR command_ref IN(SELECT command_ref FROM _ocr_purge_reading_commands);
DELETE FROM linggan_material_reading_command WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_reading_commands);
DELETE FROM linggan_material_reading WHERE reading_ref IN(SELECT reading_ref FROM _ocr_purge_readings);
DELETE FROM linggan_material_unit WHERE unit_ref IN(SELECT unit_ref FROM _ocr_purge_units);

DELETE FROM linggan_media_ocr_line WHERE layout_ref IN(SELECT layout_ref FROM _ocr_purge_layouts);
DELETE FROM linggan_media_ocr_layering_result WHERE layout_ref IN(SELECT layout_ref FROM _ocr_purge_layouts);
DELETE FROM linggan_media_ocr_adoption
 WHERE layout_ref IN(SELECT layout_ref FROM _ocr_purge_layouts)
 OR derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives);
-- Current counts of these two derivative children are zero; still scope explicitly.
DELETE FROM linggan_media_resource_relation WHERE derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives);
DELETE FROM linggan_material_media_disposition_event WHERE derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives);
DELETE FROM linggan_media_ocr_layout WHERE layout_ref IN(SELECT layout_ref FROM _ocr_purge_layouts);
DELETE FROM linggan_material_derived_text WHERE derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives);
DELETE FROM linggan_media_derivative WHERE derivative_ref IN(SELECT derivative_ref FROM _ocr_purge_derivatives);
-- IMPORTANT: preserve ASR or any non-OCR retirement rows; never TRUNCATE this table.
DELETE FROM linggan_media_ocr_retirement WHERE retired_job_ref IN(SELECT job_ref FROM _ocr_purge_jobs);
DELETE FROM linggan_media_processing_job_event WHERE job_ref IN(SELECT job_ref FROM _ocr_purge_jobs);
DELETE FROM linggan_media_processing_job WHERE job_ref IN(SELECT job_ref FROM _ocr_purge_jobs);

-- Generic AI subtree: common invocation children use OCR + polluted-comment UNION;
-- only OCR-owned batches/items/rounds/config counters are removed/reset.
DELETE FROM linggan_ai_trial_comparison WHERE comparison_ref IN(SELECT comparison_ref FROM _ocr_purge_comparisons);
UPDATE linggan_ai_task_round_state SET consecutive_without_usable=0,evaluated_rounds=0,last_round_ref=NULL
 WHERE task_key IN('ocr_recognition','ocr_quality_audit');
DELETE FROM linggan_ai_late_usage_receipt
 WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations)
 OR item_ref IN(SELECT item_ref FROM _ocr_purge_items);
DELETE FROM linggan_ai_execution_attempt
 WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations)
 OR item_ref IN(SELECT item_ref FROM _ocr_purge_items);
DELETE FROM linggan_ai_batch_item_control WHERE item_ref IN(SELECT item_ref FROM _ocr_purge_items);
DELETE FROM linggan_model_invocation WHERE invocation_ref IN(SELECT invocation_ref FROM _ocr_purge_all_invocations);
DELETE FROM linggan_ai_round WHERE round_ref IN(SELECT round_ref FROM _ocr_purge_rounds);
DELETE FROM linggan_ai_batch_item WHERE item_ref IN(SELECT item_ref FROM _ocr_purge_items);
DELETE FROM linggan_ai_batch_control WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches);
DELETE FROM linggan_ai_batch WHERE batch_ref IN(SELECT batch_ref FROM _ocr_purge_batches);
-- Remove the obsolete second-opinion task. Shared model/connection/secret rows remain.
ALTER TABLE linggan_ai_task_config DISABLE TRIGGER ai_task_config_immutable;
ALTER TABLE linggan_ai_task_setting DISABLE TRIGGER ai_task_setting_revision;
ALTER TABLE linggan_ai_control DISABLE TRIGGER ai_control_revision;
ALTER TABLE linggan_ai_control_event DISABLE TRIGGER ai_control_event_immutable;
ALTER TABLE linggan_model_egress_event DISABLE TRIGGER ai_egress_immutable;
ALTER TABLE linggan_skill_binding DISABLE TRIGGER skill_binding_valid;
ALTER TABLE linggan_skill_binding_event DISABLE TRIGGER skill_binding_event_immutable;
CREATE TEMP TABLE _ocr_purge_configs ON COMMIT DROP AS
 SELECT task_config_ref FROM linggan_ai_task_config c
 WHERE task_key='ocr_quality_audit' OR (task_key='ocr_recognition' AND task_config_ref<>(SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'));
INSERT INTO _ocr_purge_ai_commands
 SELECT command_ref FROM linggan_ai_task_config WHERE task_config_ref IN(SELECT task_config_ref FROM _ocr_purge_configs) AND command_ref IS NOT NULL
 UNION SELECT command_ref FROM linggan_ai_control_event WHERE scope_key='task:ocr_quality_audit'
 UNION SELECT event_ref FROM linggan_skill_binding_event WHERE task_key='ocr_quality_audit'
 UNION SELECT command_ref FROM linggan_ai_command WHERE payload->>'taskKey'='ocr_quality_audit';
DELETE FROM linggan_ai_control_event WHERE scope_key='task:ocr_quality_audit';
DELETE FROM linggan_ai_control WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_model_egress_event WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_skill_binding_event WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_skill_binding WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_ai_task_setting WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_ai_task_config WHERE task_config_ref IN(SELECT task_config_ref FROM _ocr_purge_configs);
DELETE FROM linggan_ai_task_round_state WHERE task_key='ocr_quality_audit';
DELETE FROM linggan_ai_task WHERE task_key='ocr_quality_audit';
-- Resolve deferred source/selection FK events before restoring table guards.
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE linggan_ai_task_config ENABLE TRIGGER ai_task_config_immutable;
ALTER TABLE linggan_ai_task_setting ENABLE TRIGGER ai_task_setting_revision;
ALTER TABLE linggan_ai_control ENABLE TRIGGER ai_control_revision;
ALTER TABLE linggan_ai_control_event ENABLE TRIGGER ai_control_event_immutable;
ALTER TABLE linggan_model_egress_event ENABLE TRIGGER ai_egress_immutable;
ALTER TABLE linggan_skill_binding ENABLE TRIGGER skill_binding_valid;
ALTER TABLE linggan_skill_binding_event ENABLE TRIGGER skill_binding_event_immutable;
DELETE FROM linggan_ai_command WHERE command_ref IN(SELECT command_ref FROM _ocr_purge_ai_commands);

ALTER TABLE linggan_material_derived_text ENABLE TRIGGER linggan_material_derived_text_is_append_only;
ALTER TABLE linggan_media_processing_job ENABLE TRIGGER linggan_media_processing_job_is_append_only;
ALTER TABLE linggan_media_processing_job_event ENABLE TRIGGER linggan_media_processing_job_event_is_append_only;
ALTER TABLE linggan_media_derivative ENABLE TRIGGER linggan_media_derivative_is_append_only;
DO $$ BEGIN IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
ALTER TABLE linggan_comment_study_run ENABLE TRIGGER cs_run_frozen_guard;
ALTER TABLE linggan_comment_study_start_request ENABLE TRIGGER cs_start_immutable;
ALTER TABLE linggan_comment_study_model_request ENABLE TRIGGER cs_request_immutable;
END IF; END $$;
ALTER TABLE linggan_ai_command ENABLE TRIGGER ai_command_immutable;
ALTER TABLE linggan_ai_batch ENABLE TRIGGER ai_batch_immutable;
ALTER TABLE linggan_ai_batch_control ENABLE TRIGGER ai_batch_control_guard;
ALTER TABLE linggan_ai_batch_item ENABLE TRIGGER ai_batch_item_immutable;
ALTER TABLE linggan_ai_batch_item_control ENABLE TRIGGER ai_item_control_guard;
ALTER TABLE linggan_ai_trial_comparison ENABLE TRIGGER ai_trial_comparison_immutable;
ALTER TABLE linggan_ai_round ENABLE TRIGGER ai_round_immutable;
ALTER TABLE linggan_model_invocation ENABLE TRIGGER ai_invocation_guard;
ALTER TABLE linggan_ai_execution_attempt ENABLE TRIGGER ai_execution_attempt_immutable;
ALTER TABLE linggan_ai_late_usage_receipt ENABLE TRIGGER ai_late_usage_immutable;
ALTER TABLE linggan_media_ocr_adoption ENABLE TRIGGER linggan_media_ocr_adoption_is_append_only;
ALTER TABLE linggan_ocr_audit_run ENABLE TRIGGER linggan_ocr_audit_run_immutable;
ALTER TABLE linggan_ocr_audit_dispatch ENABLE TRIGGER linggan_ocr_audit_dispatch_immutable;
ALTER TABLE linggan_material_reading ENABLE TRIGGER linggan_material_reading_immutable;
ALTER TABLE linggan_material_reading_selection ENABLE TRIGGER linggan_material_reading_selection_immutable;
ALTER TABLE linggan_material_reading_command ENABLE TRIGGER linggan_material_reading_command_immutable;
ALTER TABLE linggan_dossier_snapshot ENABLE TRIGGER dossier_snapshot_immutable;
ALTER TABLE linggan_dossier_fragment ENABLE TRIGGER dossier_fragment_immutable;
ALTER TABLE linggan_ocr_audit_issue ENABLE TRIGGER linggan_ocr_audit_issue_immutable;
ALTER TABLE linggan_ocr_audit_observation ENABLE TRIGGER linggan_ocr_audit_observation_immutable;
ALTER TABLE linggan_ocr_audit_sample ENABLE TRIGGER linggan_ocr_audit_sample_immutable;
ALTER TABLE linggan_ocr_audit_review ENABLE TRIGGER ocr_audit_review_immutable;

ALTER TABLE linggan_media_resource_relation ENABLE TRIGGER linggan_media_resource_relation_is_append_only;

ALTER TABLE linggan_material_media_disposition_event ENABLE TRIGGER linggan_material_media_disposition_is_append_only;

-- Force deferred current/source FKs before exiting the maintenance transaction.
SET CONSTRAINTS ALL IMMEDIATE;
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM linggan_media_processing_job WHERE processor_kind IN('image_ocr','video_frame_ocr'))
 OR EXISTS(SELECT 1 FROM linggan_material_reading WHERE producer_kind IN('ordinary_ocr','local_vision','cloud_vision','frame_ocr'))
 OR EXISTS(SELECT 1 FROM linggan_ocr_recognition_item)
 OR EXISTS(SELECT 1 FROM linggan_ocr_recognition_run)
 OR EXISTS(SELECT 1 FROM linggan_model_invocation WHERE task_key IN('ocr_recognition','ocr_quality_audit')) THEN
  RAISE EXCEPTION 'old_machine_ocr_rows_remain'; END IF;
 IF to_regclass('linggan_comment_study_work') IS NOT NULL THEN
 IF EXISTS(SELECT 1 FROM linggan_comment_study_work w
           WHERE EXISTS(SELECT 1 FROM jsonb_path_query(w.context_manifest,'$.** ? (@.type() == "string")') v
                        WHERE v#>>'{}' IN(SELECT ref FROM _ocr_purge_refs))) THEN
  RAISE EXCEPTION 'frozen_machine_ocr_reference_remains'; END IF;
 END IF;
END $$;
-- Mandatory end checks: original/platform/human/ASR/thumbnail/non-OCR-retirement
-- digests equal their captured scalar baselines, current DeepSeek configs unchanged,
-- generic control/egress remain off, all scoped named triggers restored.
-- File deletion is separate: enforce canonical root + exact hashed file manifest;
-- include failed cloud files and frame job directories, never original blobs.

DROP VIEW linggan_ocr_audit_eligible_source;
DROP TABLE linggan_ocr_audit_review;
DROP TABLE linggan_ocr_audit_issue;
DROP TABLE linggan_ocr_audit_observation;
DROP TABLE linggan_ocr_audit_dispatch;
DROP TABLE linggan_ocr_audit_sample;
DROP TABLE linggan_ocr_audit_run;
DROP TABLE linggan_ocr_audit_settings;

-- One current reading and optional human edits, including transcription before any machine result.
ALTER TABLE linggan_ocr_recognition_item RENAME COLUMN candidate_reading_ref TO reading_ref;
ALTER TABLE linggan_material_reading_command RENAME COLUMN candidate_reading_ref TO base_reading_ref;
ALTER TABLE linggan_material_reading_command ALTER COLUMN base_reading_ref DROP NOT NULL;
ALTER INDEX material_reading_command_candidate_time RENAME TO material_reading_command_base_time;
DO $$ DECLARE name text; BEGIN
 FOR name IN SELECT conname FROM pg_constraint WHERE conrelid='linggan_material_reading'::regclass AND contype='c' AND pg_get_constraintdef(oid) LIKE '%based_on_reading_ref%' LOOP
  EXECUTE format('ALTER TABLE linggan_material_reading DROP CONSTRAINT %I',name);
 END LOOP;
 FOR name IN SELECT conname FROM pg_constraint WHERE conrelid='linggan_material_reading_command'::regclass AND contype='c' AND pg_get_constraintdef(oid) LIKE '%action%' LOOP
  EXECUTE format('ALTER TABLE linggan_material_reading_command DROP CONSTRAINT %I',name);
 END LOOP;
END $$;
ALTER TABLE linggan_material_reading ADD CONSTRAINT reading_optional_human_base CHECK(based_on_reading_ref IS NULL OR producer_kind='human_edit');
ALTER TABLE linggan_material_reading ADD CONSTRAINT reading_current_publication CHECK(publication IN('automatic','human_edit'));
ALTER TABLE linggan_material_reading_command ADD CONSTRAINT reading_edit_only CHECK(action='edit' AND resulting_reading_ref IS NOT NULL);

DO $$ DECLARE saved record; n bigint; digest text; BEGIN
 FOR saved IN SELECT * FROM _ocr_keep_counts LOOP
  EXECUTE format('SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(t)::text,0)::numeric)::text,''0'') FROM %I t',saved.table_name) INTO n,digest;
  IF n<>saved.row_count OR digest<>saved.row_digest THEN RAISE EXCEPTION 'ocr_purge_changed_protected_table: %',saved.table_name; END IF;
 END LOOP;
 SELECT count(*),COALESCE(sum(hashtextextended(to_jsonb(r)::text,0)::numeric)::text,'0') INTO n,digest FROM linggan_material_reading r WHERE producer_kind IN('platform','human_edit');
 IF NOT EXISTS(SELECT 1 FROM _ocr_keep_readings WHERE row_count=n AND row_digest=digest) THEN RAISE EXCEPTION 'ocr_purge_changed_native_or_human_readings'; END IF;
END $$;
