-- Paired arrays are a bounded set of (work,id), never a cross-product.
WITH requested AS (SELECT DISTINCT * FROM unnest($1::uuid[],$2::text[]) AS k(work_ref,external_id))
SELECT k.work_ref,k.external_id,latest.material_ref AS source_ref,
       CASE WHEN r.content_public_ref IS NOT NULL THEN 'restricted'
            WHEN latest.body_state='KNOWN' AND latest.body_text IS NOT NULL THEN 'known'
            ELSE 'unknown' END AS source_state,
       CASE WHEN r.content_public_ref IS NULL AND latest.body_state='KNOWN'
            THEN left(latest.body_text,16001) ELSE NULL END AS raw_prefix
FROM requested k
LEFT JOIN LATERAL (
    SELECT c.material_ref,c.body_state,c.body_text FROM linggan_material_comment c
    JOIN linggan_runtime_capture_package p USING(package_ref)
    WHERE c.content_public_ref=k.work_ref AND c.comment_external_id=k.external_id
      AND p.accepted_at<=$3::timestamptz AND c.created_at<=$3::timestamptz
    ORDER BY c.observed_at::timestamptz DESC,c.created_at DESC,c.material_ref DESC LIMIT 1
) latest ON true
LEFT JOIN linggan_material_comment_restriction r
  ON r.content_public_ref=k.work_ref AND r.comment_external_id=k.external_id;
