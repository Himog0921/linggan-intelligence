-- $1 domain, $2 cutoff, $3 optional selected works, $4 rank-after, $5 work-after, $6 page size.
WITH latest AS MATERIALIZED (
    SELECT DISTINCT ON (c.content_public_ref,c.comment_external_id)
           c.material_ref,c.content_public_ref,c.comment_external_id,c.parent_comment_external_id,
           c.body_state,c.author_external_id,c.created_at
    FROM linggan_material_comment c
    JOIN linggan_material_content w ON w.public_ref=c.content_public_ref
    JOIN linggan_runtime_capture_package p ON p.package_ref=c.package_ref
    WHERE w.domain_ref=$1 AND p.accepted_at<=$2::timestamptz AND c.created_at<=$2::timestamptz
      AND ($3::uuid[] IS NULL OR c.content_public_ref=ANY($3))
    ORDER BY c.content_public_ref,c.comment_external_id,c.observed_at::timestamptz DESC,
             c.created_at DESC,c.material_ref DESC
), ranked AS (
    SELECT latest.*, row_number() OVER (PARTITION BY content_public_ref ORDER BY created_at DESC,material_ref DESC) AS rank
    FROM latest
), page AS MATERIALIZED (
    SELECT * FROM ranked
    WHERE rank>$4 OR (rank=$4 AND content_public_ref>$5)
    ORDER BY rank,content_public_ref LIMIT $6
)
SELECT page.*, left(raw.body_text,16001) AS body_text,
       author.author_external_id AS work_author_external_id,
       EXISTS(SELECT 1 FROM linggan_material_comment_restriction r
              WHERE r.content_public_ref=page.content_public_ref AND r.comment_external_id=page.comment_external_id) AS source_restricted
FROM page JOIN linggan_material_comment raw ON raw.material_ref=page.material_ref
LEFT JOIN linggan_material_content_author author ON author.content_public_ref=page.content_public_ref
ORDER BY page.rank,page.content_public_ref;
