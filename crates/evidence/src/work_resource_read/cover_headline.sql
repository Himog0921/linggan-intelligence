-- Shared cover-headline selector. $1 is the work UUID; $2 is the material cutoff.
-- Used by the existing Evidence renderer and the set-based study work catalog.
SELECT result.cover_headline, origin.display_ordinal, job.slot_key, layout.layout_ref
FROM linggan_media_ocr_layering_result result
JOIN linggan_media_ocr_layout layout USING (layout_ref)
JOIN linggan_media_derivative derivative ON derivative.derivative_ref = layout.ocr_derivative_ref
JOIN linggan_media_processing_job job ON job.job_ref = derivative.job_ref
JOIN LATERAL (
    SELECT origin.purpose, origin.display_ordinal
    FROM linggan_material_media_origin origin
    JOIN linggan_runtime_capture_package package ON package.package_ref = origin.package_ref
    WHERE origin.slot_key = job.slot_key AND origin.content_public_ref = $1
      AND origin.created_at <= $2::timestamptz AND package.accepted_at <= $2::timestamptz
    ORDER BY origin.created_at DESC, origin.package_ref DESC
    LIMIT 1
) origin ON true
WHERE result.state IN ('ACCEPTED', 'PARTIAL')
  AND NULLIF(btrim(result.cover_headline), '') IS NOT NULL
  AND job.processor_kind = 'image_ocr'
  AND (origin.purpose = 'cover' OR origin.display_ordinal BETWEEN 1 AND 3)
  AND result.created_at <= $2::timestamptz AND job.created_at <= $2::timestamptz
  AND layout.created_at <= $2::timestamptz AND derivative.created_at <= $2::timestamptz
  AND (
      SELECT event.state FROM linggan_media_processing_job_event event
      WHERE event.job_ref = job.job_ref AND event.occurred_at <= $2::timestamptz
      ORDER BY event.occurred_at DESC, event.event_ref DESC LIMIT 1
  ) = 'succeeded'
  AND NOT EXISTS (
      SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref = job.job_ref
  )
  AND NOT EXISTS (
      SELECT 1 FROM linggan_current_material_media_disposition disposition
      WHERE disposition.state = 'WITHDRAWN_OR_RESTRICTED'
        AND (disposition.derivative_ref = derivative.derivative_ref
          OR disposition.blob_sha256 = job.blob_sha256 OR disposition.slot_key = job.slot_key)
  )
ORDER BY (origin.purpose = 'cover') DESC, origin.display_ordinal NULLS LAST,
         result.created_at DESC, result.layering_ref DESC
LIMIT 1
