-- Material observations are not distinct comments. Metadata only, never old-body fallback.
-- $1 domain; $2 work; $3 comment id; $4 asOf; $5 created-before; $6 ref-before; $7 limit+1.
WITH scoped AS MATERIALIZED (
    SELECT source.material_ref, source.package_ref, source.observed_at, source.created_at,
           source.body_state, source.body_text IS NOT NULL AS has_body
    FROM linggan_material_comment source
    JOIN linggan_material_content work ON work.public_ref = source.content_public_ref
    JOIN linggan_runtime_capture_package package ON package.package_ref = source.package_ref
    WHERE work.domain_ref = $1 AND source.content_public_ref = $2 AND source.comment_external_id = $3
      AND package.accepted_at <= $4::timestamptz AND source.created_at <= $4::timestamptz
), page AS MATERIALIZED (
    SELECT * FROM scoped
    WHERE $5::timestamptz IS NULL OR (created_at, material_ref) < ($5::timestamptz, $6::uuid)
    ORDER BY created_at DESC, material_ref DESC LIMIT $7
), permission AS (
    SELECT EXISTS (
        SELECT 1 FROM linggan_material_comment_restriction restriction
        WHERE restriction.content_public_ref = $2 AND restriction.comment_external_id = $3
    ) AS restricted
)
SELECT jsonb_build_object(
    'totalCount', (SELECT count(*) FROM scoped),
    'rows', COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'position', jsonb_build_object(
            'createdAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'reference', page.material_ref
        ),
        'item', jsonb_build_object(
            'sourceRef', page.material_ref, 'packageRef', page.package_ref,
            'observedAt', to_char(page.observed_at::timestamptz AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'receivedAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'sourceState', CASE WHEN permission.restricted THEN 'restricted'
                WHEN page.body_state = 'KNOWN' AND page.has_body THEN 'known' ELSE 'unknown' END,
            'isCurrent', page.material_ref = (SELECT material_ref FROM scoped
                ORDER BY observed_at::timestamptz DESC, created_at DESC, material_ref DESC LIMIT 1)
        )
    ) ORDER BY page.created_at DESC, page.material_ref DESC)
    FROM page CROSS JOIN permission), '[]'::jsonb)
);
