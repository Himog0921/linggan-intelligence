-- Development reset of image recognition only. Video OCR shares Paddle
-- and the media tables, so select by processor and material kind.
SET LOCAL lock_timeout='5s';
SET LOCAL statement_timeout='60s';
SET CONSTRAINTS ALL IMMEDIATE;
SET CONSTRAINTS ALL DEFERRED;

DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM linggan_ocr_recognition_run
            WHERE state IN ('queued','running','finalizing','unknown'))
 OR EXISTS (SELECT 1 FROM linggan_ai_batch b JOIN linggan_ai_batch_control c USING(batch_ref)
            WHERE b.task_key='ocr_recognition' AND c.state='open') THEN
  RAISE EXCEPTION 'image_recognition_reset_requires_settled_runs';
 END IF;
END $$;

CREATE TEMP TABLE _image_reset_jobs ON COMMIT DROP AS
 SELECT job_ref FROM linggan_media_processing_job WHERE processor_kind='image_ocr';
CREATE TEMP TABLE _image_reset_derivatives ON COMMIT DROP AS
 SELECT derivative_ref FROM linggan_media_derivative
 WHERE job_ref IN (SELECT job_ref FROM _image_reset_jobs);
CREATE TEMP TABLE _image_reset_layouts ON COMMIT DROP AS
 SELECT layout_ref FROM linggan_media_ocr_layout
 WHERE ocr_derivative_ref IN (SELECT derivative_ref FROM _image_reset_derivatives);
CREATE TEMP TABLE _image_reset_units ON COMMIT DROP AS
 SELECT unit_ref FROM linggan_material_unit WHERE kind='image';
CREATE TEMP TABLE _image_reset_readings ON COMMIT DROP AS
 SELECT reading_ref FROM linggan_material_reading
 WHERE unit_ref IN (SELECT unit_ref FROM _image_reset_units);
CREATE TEMP TABLE _image_reset_reading_commands ON COMMIT DROP AS
 SELECT command_ref FROM linggan_material_reading_command
 WHERE unit_ref IN (SELECT unit_ref FROM _image_reset_units);
CREATE TEMP TABLE _image_reset_refs ON COMMIT DROP AS
 SELECT derivative_ref::text AS ref FROM _image_reset_derivatives
 UNION SELECT reading_ref::text FROM _image_reset_readings
 UNION SELECT layout_ref::text FROM _image_reset_layouts;
CREATE TEMP TABLE _image_reset_snapshots ON COMMIT DROP AS
 SELECT DISTINCT snapshot_ref FROM linggan_dossier_fragment
 WHERE source_ref::text IN (SELECT ref FROM _image_reset_refs)
 OR split_part(logical_position,'.',2) IN (SELECT reading_ref::text FROM _image_reset_readings);

-- Every image reading and edit in the selected units is in scope. A reference
-- from a retained unit or task remains guarded by its foreign key.
ALTER TABLE linggan_material_derived_text DISABLE TRIGGER linggan_material_derived_text_is_append_only;
ALTER TABLE linggan_media_processing_job DISABLE TRIGGER linggan_media_processing_job_is_append_only;
ALTER TABLE linggan_media_processing_job_event DISABLE TRIGGER linggan_media_processing_job_event_is_append_only;
ALTER TABLE linggan_media_derivative DISABLE TRIGGER linggan_media_derivative_is_append_only;
ALTER TABLE linggan_media_ocr_adoption DISABLE TRIGGER linggan_media_ocr_adoption_is_append_only;
ALTER TABLE linggan_material_reading DISABLE TRIGGER linggan_material_reading_immutable;
ALTER TABLE linggan_material_reading_selection DISABLE TRIGGER linggan_material_reading_selection_immutable;
ALTER TABLE linggan_material_reading_command DISABLE TRIGGER linggan_material_reading_command_immutable;
ALTER TABLE linggan_dossier_snapshot DISABLE TRIGGER dossier_snapshot_immutable;
ALTER TABLE linggan_dossier_fragment DISABLE TRIGGER dossier_fragment_immutable;
ALTER TABLE linggan_media_resource_relation DISABLE TRIGGER linggan_media_resource_relation_is_append_only;
ALTER TABLE linggan_material_media_disposition_event DISABLE TRIGGER linggan_material_media_disposition_is_append_only;

DELETE FROM linggan_ocr_recognition_slot_alias;
DELETE FROM linggan_ocr_recognition_item;
DELETE FROM linggan_ocr_recognition_run;
DELETE FROM linggan_dossier_fragment WHERE snapshot_ref IN(SELECT snapshot_ref FROM _image_reset_snapshots);
DELETE FROM linggan_dossier_snapshot WHERE snapshot_ref IN(SELECT snapshot_ref FROM _image_reset_snapshots);

-- Clear FK children. No UPDATE to a unit that is about to be deleted: such UPDATE
-- would queue material_unit_current_event and fail at commit after the unit is gone.
DELETE FROM linggan_media_processing_work WHERE job_ref IN(SELECT job_ref FROM _image_reset_jobs);
DELETE FROM linggan_material_reading_selection
 WHERE unit_ref IN(SELECT unit_ref FROM _image_reset_units)
 OR reading_ref IN(SELECT reading_ref FROM _image_reset_readings)
 OR command_ref IN(SELECT command_ref FROM _image_reset_reading_commands);
DELETE FROM linggan_material_reading_command WHERE command_ref IN(SELECT command_ref FROM _image_reset_reading_commands);
DELETE FROM linggan_material_reading WHERE reading_ref IN(SELECT reading_ref FROM _image_reset_readings);
DELETE FROM linggan_material_unit WHERE unit_ref IN(SELECT unit_ref FROM _image_reset_units);

DELETE FROM linggan_media_ocr_line WHERE layout_ref IN(SELECT layout_ref FROM _image_reset_layouts);
DELETE FROM linggan_media_ocr_layering_result WHERE layout_ref IN(SELECT layout_ref FROM _image_reset_layouts);
DELETE FROM linggan_media_ocr_adoption
 WHERE layout_ref IN(SELECT layout_ref FROM _image_reset_layouts)
 OR derivative_ref IN(SELECT derivative_ref FROM _image_reset_derivatives);
-- Current counts of these two derivative children are zero; still scope explicitly.
DELETE FROM linggan_media_resource_relation WHERE derivative_ref IN(SELECT derivative_ref FROM _image_reset_derivatives);
DELETE FROM linggan_material_media_disposition_event WHERE derivative_ref IN(SELECT derivative_ref FROM _image_reset_derivatives);
DELETE FROM linggan_media_ocr_layout WHERE layout_ref IN(SELECT layout_ref FROM _image_reset_layouts);
DELETE FROM linggan_material_derived_text WHERE derivative_ref IN(SELECT derivative_ref FROM _image_reset_derivatives);
DELETE FROM linggan_media_derivative WHERE derivative_ref IN(SELECT derivative_ref FROM _image_reset_derivatives);
-- IMPORTANT: preserve ASR or any non-OCR retirement rows; never TRUNCATE this table.
DELETE FROM linggan_media_ocr_retirement WHERE retired_job_ref IN(SELECT job_ref FROM _image_reset_jobs);
DELETE FROM linggan_media_processing_job_event WHERE job_ref IN(SELECT job_ref FROM _image_reset_jobs);
DELETE FROM linggan_media_processing_job WHERE job_ref IN(SELECT job_ref FROM _image_reset_jobs);

SET CONSTRAINTS ALL IMMEDIATE;

ALTER TABLE linggan_material_derived_text ENABLE TRIGGER linggan_material_derived_text_is_append_only;
ALTER TABLE linggan_media_processing_job ENABLE TRIGGER linggan_media_processing_job_is_append_only;
ALTER TABLE linggan_media_processing_job_event ENABLE TRIGGER linggan_media_processing_job_event_is_append_only;
ALTER TABLE linggan_media_derivative ENABLE TRIGGER linggan_media_derivative_is_append_only;
ALTER TABLE linggan_media_ocr_adoption ENABLE TRIGGER linggan_media_ocr_adoption_is_append_only;
ALTER TABLE linggan_material_reading ENABLE TRIGGER linggan_material_reading_immutable;
ALTER TABLE linggan_material_reading_selection ENABLE TRIGGER linggan_material_reading_selection_immutable;
ALTER TABLE linggan_material_reading_command ENABLE TRIGGER linggan_material_reading_command_immutable;
ALTER TABLE linggan_dossier_snapshot ENABLE TRIGGER dossier_snapshot_immutable;
ALTER TABLE linggan_dossier_fragment ENABLE TRIGGER dossier_fragment_immutable;
ALTER TABLE linggan_media_resource_relation ENABLE TRIGGER linggan_media_resource_relation_is_append_only;
ALTER TABLE linggan_material_media_disposition_event ENABLE TRIGGER linggan_material_media_disposition_is_append_only;
