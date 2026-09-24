-- $1 work refs, $2 cutoff. Research context is not the display-title fallback.
WITH work_scope AS (SELECT public_ref FROM linggan_material_content WHERE public_ref=ANY($1::uuid[])),
native_head AS (
    SELECT DISTINCT ON (d.content_public_ref) d.*
    FROM linggan_material_content_detail d JOIN work_scope w ON w.public_ref=d.content_public_ref
    JOIN linggan_runtime_capture_package p USING(package_ref)
    WHERE p.accepted_at<=$2::timestamptz AND d.created_at<=$2::timestamptz
    ORDER BY d.content_public_ref,d.observed_at::timestamptz DESC,d.created_at DESC,d.material_ref DESC
), media AS (
    SELECT text.content_public_ref,job.slot_key,
           CASE WHEN text.kind='ocr_text' THEN 'image_substantive_text' ELSE text.kind END AS kind,
           derived.derivative_ref AS source_ref,derived.created_at,derived.content_hash,
           slot.ordinal AS slot_ordinal,text.source_location,
           CASE WHEN text.kind='ocr_text' THEN layer.image_substantive_text ELSE text.text_content END AS text
    FROM linggan_material_derived_text text
    JOIN work_scope w ON w.public_ref=text.content_public_ref
    JOIN linggan_media_derivative derived USING(derivative_ref)
    JOIN linggan_media_processing_job job USING(job_ref)
    LEFT JOIN linggan_media_slot slot ON slot.slot_key=job.slot_key
    LEFT JOIN linggan_media_ocr_layout layout ON layout.ocr_derivative_ref=derived.derivative_ref
    LEFT JOIN LATERAL (
        SELECT result.state,result.image_substantive_text FROM linggan_media_ocr_layering_result result
        WHERE result.layout_ref=layout.layout_ref AND result.created_at<=$2::timestamptz
        ORDER BY result.created_at DESC,result.layering_ref DESC LIMIT 1
    ) layer ON true
    WHERE text.created_at<=$2::timestamptz AND derived.created_at<=$2::timestamptz
      AND job.created_at<=$2::timestamptz
      AND (text.kind<>'ocr_text' OR (layout.created_at<=$2::timestamptz AND layer.state='ACCEPTED'
           AND NULLIF(btrim(layer.image_substantive_text),'') IS NOT NULL))
      AND EXISTS (
          SELECT 1 FROM linggan_material_media_origin origin JOIN linggan_runtime_capture_package package USING(package_ref)
          WHERE origin.slot_key=job.slot_key AND origin.content_public_ref=text.content_public_ref
            AND origin.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz
      )
      AND (SELECT event.state FROM linggan_media_processing_job_event event
           WHERE event.job_ref=job.job_ref AND event.occurred_at<=$2::timestamptz
           ORDER BY event.occurred_at DESC,event.event_ref DESC LIMIT 1)='succeeded'
      AND NOT EXISTS (SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref=job.job_ref)
      AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition disposition
          WHERE disposition.state='WITHDRAWN_OR_RESTRICTED' AND
            (disposition.derivative_ref=derived.derivative_ref OR disposition.slot_key=job.slot_key
             OR disposition.blob_sha256=job.blob_sha256))
), media_head AS (
    SELECT DISTINCT ON (content_public_ref,slot_key,kind) * FROM media
    ORDER BY content_public_ref,slot_key,kind,created_at DESC,source_ref DESC
), fragments AS (
    SELECT content_public_ref,0 AS priority,NULL::integer AS slot_ordinal,material_ref AS source_ref,
           'native_title'::text AS kind,title AS text,NULL::jsonb AS source_location,NULL::text AS content_hash
    FROM native_head WHERE title_state='KNOWN' AND title IS NOT NULL
    UNION ALL
    SELECT content_public_ref,1,NULL::integer,material_ref,'body',body_text,NULL::jsonb,NULL::text
    FROM native_head WHERE body_state='KNOWN' AND body_text IS NOT NULL
    UNION ALL
    SELECT content_public_ref,2,slot_ordinal,source_ref,kind,text,source_location,content_hash FROM media_head
), measured AS (
    SELECT *,char_length(text) AS character_count,
           sum(char_length(text)) OVER (PARTITION BY content_public_ref ORDER BY priority,slot_ordinal NULLS LAST,source_ref,kind) AS running_size
    FROM fragments
)
SELECT w.public_ref AS work_ref, COALESCE(jsonb_agg(jsonb_build_object(
    'kind',f.kind,'sourceRef',f.source_ref,'slotOrdinal',f.slot_ordinal,
    'text',CASE WHEN f.running_size<=20000 THEN f.text ELSE NULL END,
    'characterCount',f.character_count,'sourceLocation',f.source_location,'contentHash',f.content_hash
) ORDER BY f.priority,f.slot_ordinal NULLS LAST,f.source_ref,f.kind)
    FILTER(WHERE f.source_ref IS NOT NULL),'[]'::jsonb) AS fragments
FROM work_scope w LEFT JOIN measured f ON f.content_public_ref=w.public_ref GROUP BY w.public_ref;
