-- $1 domain, $2 cutoff, $3 selected work refs or NULL, $4 matching selected roles,
-- $5 preview role when $3 is NULL, $6 rank-after, $7 work-after, $8 page size.
WITH requested_roles AS MATERIALIZED (
    SELECT selected.work_ref, selected.observation_role
    FROM unnest($3::uuid[], $4::text[]) AS selected(work_ref, observation_role)
    UNION ALL
    SELECT usage.content_public_ref, $5::text
    FROM linggan_material_domain_usage usage
    WHERE $3::uuid[] IS NULL AND $5::text IS NOT NULL
      AND usage.domain_ref = $1 AND usage.role = $5
), eligible_work_roles AS MATERIALIZED (
    SELECT requested.work_ref, requested.observation_role
    FROM requested_roles requested
    JOIN linggan_material_domain_usage usage
      ON usage.content_public_ref = requested.work_ref
     AND usage.domain_ref = $1
     AND usage.role = requested.observation_role
    GROUP BY requested.work_ref, requested.observation_role
), latest AS MATERIALIZED (
    SELECT DISTINCT ON (c.content_public_ref, c.comment_external_id)
           c.material_ref, c.content_public_ref, c.comment_external_id,
           c.parent_comment_external_id, c.body_state, c.author_external_id,
           c.created_at, eligible_work_roles.observation_role
    FROM linggan_material_comment c
    JOIN eligible_work_roles ON eligible_work_roles.work_ref = c.content_public_ref
    JOIN linggan_runtime_capture_package p ON p.package_ref = c.package_ref
    WHERE p.accepted_at <= $2::timestamptz AND c.created_at <= $2::timestamptz
    ORDER BY c.content_public_ref, c.comment_external_id,
             c.observed_at::timestamptz DESC, c.created_at DESC, c.material_ref DESC
), ranked AS (
    SELECT latest.*,
           row_number() OVER (PARTITION BY content_public_ref ORDER BY created_at DESC, material_ref DESC) AS rank
    FROM latest
), page AS MATERIALIZED (
    SELECT * FROM ranked
    WHERE rank > $6 OR (rank = $6 AND content_public_ref > $7)
    ORDER BY rank, content_public_ref
    LIMIT $8
)
SELECT page.*, left(raw.body_text, 16001) AS body_text,
       author.author_external_id AS work_author_external_id,
       EXISTS (
           SELECT 1 FROM linggan_material_comment_restriction restriction
           WHERE restriction.content_public_ref = page.content_public_ref
             AND restriction.comment_external_id = page.comment_external_id
       ) AS source_restricted
FROM page
JOIN linggan_material_comment raw ON raw.material_ref = page.material_ref
LEFT JOIN linggan_material_content_author author ON author.content_public_ref = page.content_public_ref
ORDER BY page.rank, page.content_public_ref;
