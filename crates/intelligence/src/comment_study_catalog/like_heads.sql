like_heads AS MATERIALIZED (
    -- A known count is eligible only after its own projection exists. A newer unknown
    -- observation does not hide the latest known one; the work's likes are never used.
    SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id)
           comment.content_public_ref, comment.comment_external_id,
           engagement.like_count, comment.observed_at AS like_observed_at
    FROM linggan_material_comment comment
    JOIN linggan_material_comment_engagement engagement
      ON engagement.material_ref = comment.material_ref AND engagement.like_count_state = 'KNOWN'
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE EXISTS (
        SELECT 1 FROM linggan_material_domain_usage usage
        WHERE usage.content_public_ref = comment.content_public_ref AND usage.domain_ref = $1
    )
      AND ($4::uuid IS NULL OR comment.content_public_ref = $4)
      AND ($5::text IS NULL OR comment.comment_external_id = $5)
      AND package.accepted_at <= $2::timestamptz AND comment.created_at <= $2::timestamptz
      AND engagement.created_at <= $2::timestamptz
    ORDER BY comment.content_public_ref, comment.comment_external_id,
             comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC
)
