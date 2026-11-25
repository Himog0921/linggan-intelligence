-- One ordered result per reading row. A unit may have several historical versions.
WITH inputs AS (
 SELECT * FROM jsonb_to_recordset($1) AS i(
   position integer, work uuid, kind text, slot text, source uuid, bytes_source uuid, derivative uuid
 )
), dispositions AS MATERIALIZED (
 SELECT slot_key, blob_sha256, materialization_ref, derivative_ref, state
 FROM linggan_current_material_media_disposition
)
SELECT i.position,
 CASE WHEN i.kind='body' THEN EXISTS (
   SELECT 1 FROM linggan_material_content_detail d
   JOIN linggan_runtime_record_disposition r USING(package_ref,record_ordinal)
   WHERE d.content_public_ref=i.work AND d.material_ref=i.source
   AND r.disposition='accepted_for_library_content'
 ) ELSE NOT EXISTS (
   SELECT 1 FROM dispositions d WHERE d.state='WITHDRAWN_OR_RESTRICTED'
   AND (d.slot_key=i.slot OR d.derivative_ref=i.derivative
        OR d.materialization_ref=i.source OR d.blob_sha256=m.blob_sha256)
 ) AND NOT EXISTS (
   SELECT 1 FROM linggan_media_derivative d
   JOIN linggan_media_ocr_retirement r ON r.retired_job_ref=d.job_ref
   WHERE d.derivative_ref=i.derivative
 ) END AS unrestricted,
 i.kind='body' OR NOT EXISTS (
   SELECT 1 FROM dispositions d WHERE d.state='BYTES_CLEANED'
   AND d.materialization_ref=i.bytes_source
 ) AS bytes_available
FROM inputs i LEFT JOIN linggan_media_materialization m ON m.materialization_ref=i.source
ORDER BY i.position
