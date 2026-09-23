-- One contextual parent: choose the latest accepted version, THEN test its readability.
-- Creator replies may explain a reader's words; they never become that reader's evidence.
WITH latest AS (
    SELECT parent.material_ref, parent.body_state, parent.body_text
    FROM linggan_material_comment parent
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE parent.content_public_ref = $1 AND parent.comment_external_id = $2
      AND package.accepted_at <= $3::timestamptz AND parent.created_at <= $3::timestamptz
    ORDER BY parent.observed_at::timestamptz DESC, parent.created_at DESC, parent.material_ref DESC
    LIMIT 1
), permission AS (
    SELECT EXISTS (
        SELECT 1 FROM linggan_material_comment_restriction restriction
        WHERE restriction.content_public_ref = $1 AND restriction.comment_external_id = $2
    ) AS restricted
)
SELECT latest.material_ref AS source_ref,
       CASE WHEN permission.restricted THEN 'restricted'
            WHEN latest.body_state = 'KNOWN' AND latest.body_text IS NOT NULL THEN 'known'
            ELSE 'unknown' END AS source_state,
       CASE WHEN NOT permission.restricted AND latest.body_state = 'KNOWN'
            THEN left(latest.body_text, 16001) ELSE NULL END AS raw_prefix
FROM permission LEFT JOIN latest ON true;
