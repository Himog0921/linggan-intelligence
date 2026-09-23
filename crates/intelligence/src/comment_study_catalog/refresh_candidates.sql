-- Latest first, eligibility second: an UNKNOWN newer version must not revive an older body.
-- Cache even creator/unknown-author text; cleaning is not study eligibility.
WITH latest AS MATERIALIZED (
    SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id)
        comment.material_ref, comment.content_public_ref, comment.comment_external_id,
        comment.body_state, (comment.body_text IS NOT NULL) AS has_body, comment.created_at
    FROM linggan_material_comment comment
    JOIN linggan_material_content content ON content.public_ref = comment.content_public_ref
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE content.domain_ref = $1 AND package.accepted_at <= scope_001_now()
      AND comment.created_at <= scope_001_now()
    ORDER BY comment.content_public_ref, comment.comment_external_id,
             comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC
), pending AS MATERIALIZED (
    SELECT source.*
    FROM latest source
    WHERE source.body_state = 'KNOWN' AND source.has_body
      AND NOT EXISTS (
          SELECT 1 FROM linggan_material_comment_restriction restriction
          WHERE restriction.content_public_ref = source.content_public_ref
            AND restriction.comment_external_id = source.comment_external_id
      )
      AND NOT EXISTS (
          SELECT 1 FROM linggan_comment_study_clean_cache cache
          WHERE cache.source_ref = source.material_ref AND cache.cleaner_version = $2
      )
    ORDER BY source.created_at, source.material_ref
    LIMIT $3
)
SELECT pending.material_ref, left(raw.body_text, 16001) AS raw_prefix,
       encode(sha256(convert_to(raw.body_text, 'UTF8')), 'hex') AS raw_sha256
FROM pending
JOIN linggan_material_comment raw ON raw.material_ref = pending.material_ref
ORDER BY pending.created_at, pending.material_ref;
