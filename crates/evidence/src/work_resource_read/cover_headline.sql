-- The title supplement is a heading in an explicitly selected canonical reading.
-- $1 is a work UUID; $2 freezes source/selection visibility, never current restrictions.
WITH current_origin AS MATERIALIZED (
    SELECT DISTINCT ON (origin.slot_key) origin.*
    FROM linggan_material_media_origin origin
    JOIN linggan_runtime_capture_package package USING(package_ref)
    JOIN linggan_runtime_record_disposition disposition
      ON disposition.package_ref=origin.package_ref AND disposition.record_ordinal=origin.record_ordinal
    WHERE origin.content_public_ref = $1 AND origin.created_at <= $2::timestamptz
      AND package.accepted_at <= $2::timestamptz AND disposition.disposition<>'quarantined'
    ORDER BY origin.slot_key,origin.source_generation DESC
)
SELECT heading.block->>'text' AS cover_headline,origin.display_ordinal,origin.slot_key,reading.reading_ref,unit.unit_ref,reading.source_location->>'layoutRef' AS layout_ref
FROM current_origin origin
JOIN linggan_material_unit unit ON unit.content_public_ref=origin.content_public_ref
  AND unit.slot_key=origin.slot_key AND unit.kind='image'
JOIN LATERAL (
    SELECT selection.* FROM linggan_material_reading_selection selection
    WHERE selection.unit_ref=unit.unit_ref AND selection.created_at <= $2::timestamptz
    ORDER BY selection.revision DESC LIMIT 1
) selected ON selected.source_state='present'
JOIN linggan_material_reading reading ON reading.reading_ref=selected.reading_ref
  AND reading.unit_ref=unit.unit_ref AND reading.source_version=selected.source_version
  AND reading.created_at <= $2::timestamptz
JOIN LATERAL (
    SELECT materialization.* FROM linggan_media_materialization materialization
    JOIN linggan_media_download_attempt attempt USING(download_attempt_ref)
    JOIN linggan_media_blob blob ON blob.sha256=materialization.blob_sha256
    WHERE attempt.media_observation_ref=origin.observation_ref
      AND materialization.verified_at <= $2::timestamptz
      AND blob.mime_type LIKE 'image/%'
    ORDER BY materialization.verified_at DESC,materialization.materialization_ref DESC LIMIT 1
) materialization ON materialization.materialization_ref::text=reading.source_version
JOIN LATERAL (
    WITH blocks AS (
        SELECT block,ordinal FROM jsonb_array_elements(reading.blocks) WITH ORDINALITY AS blocks(block,ordinal)
    ), first_heading AS (
        SELECT min(ordinal) AS start FROM blocks WHERE block->>'kind'='heading' AND nullif(btrim(block->>'text'),'') IS NOT NULL
    ), title_lines AS (
        SELECT btrim(line, U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000') AS text,ordinal,line_ordinal
        FROM blocks,first_heading, LATERAL regexp_split_to_table(block->>'text', E'\\r?\\n') WITH ORDINALITY AS lines(line,line_ordinal)
        WHERE ordinal>=start AND block->>'kind'='heading'
        AND NOT EXISTS(SELECT 1 FROM blocks gap WHERE gap.ordinal>=start AND gap.ordinal<blocks.ordinal AND gap.block->>'kind'<>'heading')
    ), connected AS (
        SELECT text,ordinal,line_ordinal,lag(text) OVER(ORDER BY ordinal,line_ordinal) AS previous FROM title_lines WHERE text<>''
    )
    SELECT jsonb_build_object('text',string_agg(CASE WHEN previous~'[A-Za-z0-9]$' AND text~'^[A-Za-z0-9]' THEN ' ' ELSE '' END || text,'' ORDER BY ordinal,line_ordinal)) AS block,min(ordinal) AS ordinal FROM connected HAVING count(*)>0
) heading ON true
WHERE (origin.purpose='cover' OR (origin.display_order_state='KNOWN' AND origin.display_ordinal=1 AND NOT EXISTS(SELECT 1 FROM current_origin cover WHERE cover.purpose='cover')))
  AND linggan_dossier_slot_active(origin.content_public_ref,origin.slot_key,$2::timestamptz)
  AND NOT EXISTS (
    SELECT 1 FROM linggan_current_material_media_disposition disposition
    WHERE disposition.state IN ('WITHDRAWN_OR_RESTRICTED','BYTES_CLEANED')
      AND (disposition.slot_key=origin.slot_key OR disposition.blob_sha256=materialization.blob_sha256
        OR disposition.materialization_ref=materialization.materialization_ref
        OR disposition.derivative_ref=reading.derivative_ref)
  )
  AND NOT EXISTS (
    SELECT 1 FROM linggan_media_derivative derivative JOIN linggan_media_ocr_retirement retired
      ON retired.retired_job_ref=derivative.job_ref WHERE derivative.derivative_ref=reading.derivative_ref
  )
ORDER BY (origin.purpose='cover') DESC,origin.display_ordinal NULLS LAST,heading.ordinal,origin.slot_key
LIMIT 1
