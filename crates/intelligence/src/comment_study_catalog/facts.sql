-- Shared catalog source/history relation; no title, pagination or page-specific eligibility.
-- $1 domain; $2 asOf; $3 cleaner; $4 optional work; $5 optional exact stable comment id.
WITH latest AS MATERIALIZED (
    SELECT DISTINCT ON (comment.content_public_ref, comment.comment_external_id)
           comment.material_ref, comment.content_public_ref, comment.comment_external_id,
           comment.parent_comment_external_id, comment.body_state,
           comment.author_external_id, comment.author_display_name,
           comment.observed_at, comment.created_at
    FROM linggan_material_comment comment
    JOIN linggan_material_content content ON content.public_ref = comment.content_public_ref
    JOIN linggan_runtime_capture_package package ON package.package_ref = comment.package_ref
    WHERE EXISTS (
        SELECT 1 FROM linggan_material_domain_usage usage
        WHERE usage.content_public_ref = comment.content_public_ref
          AND usage.domain_ref = $1
          AND ($4::uuid IS NOT NULL OR usage.use_kind = 'domain_research')
    )
      AND ($4::uuid IS NULL OR comment.content_public_ref = $4)
      AND ($5::text IS NULL OR comment.comment_external_id = $5)
      AND package.accepted_at <= $2::timestamptz AND comment.created_at <= $2::timestamptz
    ORDER BY comment.content_public_ref, comment.comment_external_id,
             comment.observed_at::timestamptz DESC, comment.created_at DESC, comment.material_ref DESC
), history AS MATERIALIZED (
    SELECT request.request_ref, request.source_ref,
           CASE WHEN bool_or(control.state IN ('pending','running')) THEN 'running'
                WHEN bool_or(control.state='failed' OR (control.state='skipped' AND control.failure_code IS DISTINCT FROM 'comment_no_problem_signal')) THEN 'failed'
                WHEN bool_or(control.state='cancelled') THEN 'cancelled'
                ELSE 'succeeded' END AS state,
           request.created_at,max(control.finished_at) AS finished_at,
           original.content_public_ref,original.comment_external_id
    FROM linggan_comment_research_request request
    JOIN linggan_material_comment original ON original.material_ref=request.source_ref
    JOIN linggan_comment_research_stage stage USING(request_ref)
    JOIN linggan_ai_batch_item_control control USING(item_ref)
    WHERE request.domain_ref=$1 AND request.created_at<=$2::timestamptz
      AND ($4::uuid IS NULL OR original.content_public_ref=$4)
      AND ($5::text IS NULL OR original.comment_external_id=$5)
    GROUP BY request.request_ref,original.content_public_ref,original.comment_external_id
), last_attempt AS (
    SELECT DISTINCT ON (content_public_ref, comment_external_id) * FROM history
    ORDER BY content_public_ref, comment_external_id, created_at DESC, request_ref DESC
), success AS (
    -- Select the success head BEFORE checking equality. Do not resurrect an older result.
    SELECT DISTINCT ON (content_public_ref, comment_external_id) * FROM history
    WHERE state IN ('succeeded', 'no_signal')
    ORDER BY content_public_ref, comment_external_id, created_at DESC, request_ref DESC
), active AS (
    SELECT DISTINCT content_public_ref, comment_external_id FROM history
    WHERE state IN ('pending','running')
), facts AS MATERIALIZED (
    SELECT latest.*, cache.source_ref AS cached_source_ref, cache.clean_state,
           cache.research_text, cache.clean_reasons,
           raw.body_text IS NOT NULL AS has_body,
           CASE WHEN EXISTS (
               SELECT 1 FROM linggan_material_domain_usage role_usage
               WHERE role_usage.content_public_ref = latest.content_public_ref
                 AND role_usage.domain_ref = $1 AND role_usage.role = 'primary'
                 AND ($4::uuid IS NOT NULL OR role_usage.use_kind = 'domain_research')
           ) THEN 'primary' ELSE 'reference' END AS observation_role,
           CASE
             WHEN NULLIF(btrim(latest.author_external_id), '') IS NULL
               OR NULLIF(btrim(author.author_external_id), '') IS NULL THEN 'unknown'
             WHEN btrim(latest.author_external_id) = btrim(author.author_external_id) THEN 'creator'
             ELSE 'reader'
           END AS voice_role,
           NULLIF(btrim(author.author_external_id), '') IS NULL AS work_author_unknown,
           EXISTS (
               SELECT 1 FROM linggan_material_comment_restriction restriction
               WHERE restriction.content_public_ref = latest.content_public_ref
                 AND restriction.comment_external_id = latest.comment_external_id
           ) AS source_restricted,
           last_attempt.request_ref AS latest_request_ref,
           last_attempt.state AS latest_state,
           last_attempt.created_at AS latest_created_at,
           last_attempt.finished_at AS latest_finished_at,
           CASE WHEN last_raw.body_state = 'KNOWN' AND last_raw.body_text IS NOT NULL
                THEN last_raw.body_text IS NOT DISTINCT FROM raw.body_text ELSE NULL END AS latest_raw_same,
           CASE WHEN last_raw.body_state = 'KNOWN' AND last_raw.body_text IS NOT NULL
                THEN 'known' ELSE 'unknown' END AS latest_source_state,
           success.request_ref AS effective_request_ref,
           success.source_ref AS effective_source_ref, success.state AS effective_state,
           success.created_at AS effective_created_at, success.finished_at AS effective_finished_at,
           CASE WHEN success_raw.body_state = 'KNOWN' AND success_raw.body_text IS NOT NULL
                THEN success_raw.body_text IS NOT DISTINCT FROM raw.body_text ELSE NULL END AS effective_raw_same,
           CASE WHEN success_raw.body_state = 'KNOWN' AND success_raw.body_text IS NOT NULL
                THEN 'known' ELSE 'unknown' END AS effective_source_state,
           active.content_public_ref IS NOT NULL AS in_progress
    FROM latest
    JOIN linggan_material_comment raw ON raw.material_ref = latest.material_ref
    LEFT JOIN linggan_comment_clean_cache cache
      ON cache.source_ref = latest.material_ref AND cache.cleaner_version = $3
    LEFT JOIN linggan_material_content_author author ON author.content_public_ref = latest.content_public_ref
    LEFT JOIN last_attempt ON last_attempt.content_public_ref = latest.content_public_ref
      AND last_attempt.comment_external_id = latest.comment_external_id
    LEFT JOIN success ON success.content_public_ref = latest.content_public_ref
      AND success.comment_external_id = latest.comment_external_id
    LEFT JOIN active ON active.content_public_ref = latest.content_public_ref
      AND active.comment_external_id = latest.comment_external_id
    LEFT JOIN linggan_material_comment last_raw ON last_raw.material_ref = last_attempt.source_ref
    LEFT JOIN linggan_material_comment success_raw ON success_raw.material_ref = success.source_ref
), qualified AS MATERIALIZED (
    SELECT facts.*,
           NOT source_restricted AND body_state = 'KNOWN' AND has_body AS readable,
           /*SOURCE_ELIGIBILITY_CASE*/ AS exclusion_reason
    FROM facts
)
