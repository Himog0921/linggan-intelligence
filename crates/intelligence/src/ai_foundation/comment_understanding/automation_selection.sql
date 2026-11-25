-- Resolve the current source only for the stable identities in this saved selection.
-- Take the newest observation before checking readability, as in the V2 source gate.
WITH cutoff AS MATERIALIZED (SELECT scope_001_now() AS as_of)
SELECT jsonb_build_object(
    'selectionRef', s.selection_ref,
    'domainRef', s.domain_ref,
    'selections', COALESCE((
        SELECT jsonb_agg(jsonb_build_object(
            'workRef', t.work_ref, 'sourceRef', head.material_ref,
            'commentExternalId', t.comment_external_id, 'observationRole', 'primary',
            'submissionState', t.submission_state, 'submissionReason', t.submission_reason,
            'submittedBatchRef', t.submitted_batch_ref
        ) ORDER BY t.position)
        FROM linggan_comment_research_selection_target t
        CROSS JOIN cutoff
        JOIN LATERAL (
            SELECT c.material_ref, c.body_state, c.body_text IS NOT NULL AS has_body
            FROM linggan_material_comment c
            JOIN linggan_runtime_capture_package p USING (package_ref)
            WHERE c.content_public_ref = t.work_ref AND c.comment_external_id = t.comment_external_id
              AND p.accepted_at <= cutoff.as_of AND c.created_at <= cutoff.as_of
              AND linggan_opportunity_observed_at(c.observed_at) IS NOT NULL
            ORDER BY linggan_opportunity_observed_at(c.observed_at) DESC, c.created_at DESC, c.material_ref DESC
            LIMIT 1
        ) head ON head.body_state = 'KNOWN' AND head.has_body
        WHERE t.selection_ref = s.selection_ref AND ($3::integer[] IS NULL OR t.position=ANY($3))
          AND EXISTS (
              SELECT 1 FROM linggan_material_domain_usage u
              WHERE u.content_public_ref = t.work_ref AND u.domain_ref = s.domain_ref
                AND u.role = 'primary' AND u.use_kind = 'domain_research'
          )
          AND NOT EXISTS (
              SELECT 1 FROM linggan_material_comment_restriction r
              WHERE r.content_public_ref = t.work_ref AND r.comment_external_id = t.comment_external_id
          )
    ), '[]'::jsonb)
)
FROM linggan_comment_research_selection s
WHERE s.selection_ref = $1 AND s.domain_ref = $2
