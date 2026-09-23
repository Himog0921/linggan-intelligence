-- A recheck at write time limits propagation; every later reader must still check restrictions.
INSERT INTO linggan_comment_study_clean_cache
    (source_ref, cleaner_version, raw_sha256, research_text, clean_state, clean_reasons)
SELECT source.material_ref, $2, $3, $4, $5, $6
FROM linggan_material_comment source
JOIN linggan_material_content content ON content.public_ref = source.content_public_ref
WHERE source.material_ref = $1 AND content.domain_ref = $7
  AND source.body_state = 'KNOWN' AND source.body_text IS NOT NULL
  AND encode(sha256(convert_to(source.body_text, 'UTF8')), 'hex') = $3
  AND NOT EXISTS (
      SELECT 1 FROM linggan_material_comment_restriction restriction
      WHERE restriction.content_public_ref = source.content_public_ref
        AND restriction.comment_external_id = source.comment_external_id
  )
ON CONFLICT (source_ref, cleaner_version) DO NOTHING;
