-- Complete stable-comment attempt history. Bodies/frames/prompts are deliberately not projected.
-- $1 domain; $2 work; $3 comment id; $4 asOf; $5 created-before; $6 ref-before; $7 limit+1.
WITH scoped AS MATERIALIZED (
    SELECT target.target_ref, target.run_ref, target.source_ref, target.state,
           target.created_at, target.finished_at, target.dependency_state,
           run.state AS run_state, run.dispatch_state, run.dispatch_reason,
           policy.policy_ref, policy.method_name, policy.method_hash,
           policy.method_manifest IS NOT NULL AS method_recorded,
           source.body_state, source.body_text IS NOT NULL AS has_body
    FROM linggan_comment_study_target target
    JOIN linggan_material_comment source ON source.material_ref = target.source_ref
    JOIN linggan_comment_study_run run ON run.run_ref = target.run_ref
    JOIN linggan_comment_study_policy policy ON policy.policy_ref = run.policy_ref
    JOIN linggan_material_content work ON work.public_ref = source.content_public_ref
    WHERE policy.domain_ref = $1 AND work.domain_ref = $1
      AND target.content_public_ref = $2 AND source.content_public_ref = $2
      AND source.comment_external_id = $3 AND target.created_at <= $4::timestamptz
), page AS MATERIALIZED (
    SELECT * FROM scoped
    WHERE $5::timestamptz IS NULL OR (created_at, target_ref) < ($5::timestamptz, $6::uuid)
    ORDER BY created_at DESC, target_ref DESC LIMIT $7
), permission AS (
    SELECT EXISTS (
        SELECT 1 FROM linggan_material_comment_restriction restriction
        WHERE restriction.content_public_ref = $2 AND restriction.comment_external_id = $3
    ) AS restricted
), signal_counts AS (
    SELECT signal.target_ref, count(*) AS signal_count
    FROM linggan_comment_study_signal signal JOIN page ON page.target_ref = signal.target_ref
    GROUP BY signal.target_ref
)
SELECT jsonb_build_object(
    'totalCount', (SELECT count(*) FROM scoped),
    'rows', COALESCE((SELECT jsonb_agg(jsonb_build_object(
        'position', jsonb_build_object(
            'createdAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'reference', page.target_ref
        ),
        'item', jsonb_build_object(
            'targetRef', page.target_ref, 'runRef', page.run_ref, 'sourceRef', page.source_ref,
            'state', page.state, 'runState', page.run_state,
            'dispatchState', page.dispatch_state, 'dispatchReason', page.dispatch_reason,
            'dependencyState', page.dependency_state,
            'createdAt', to_char(page.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'finishedAt', to_char(page.finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
            'sourceState', CASE WHEN permission.restricted THEN 'restricted'
                WHEN page.body_state = 'KNOWN' AND page.has_body THEN 'known' ELSE 'unknown' END,
            'inputComparison', 'unknown',
            'isLatestAttempt', page.target_ref = (SELECT target_ref FROM scoped ORDER BY created_at DESC, target_ref DESC LIMIT 1),
            'isLatestSuccessfulAttempt', COALESCE(page.target_ref = (SELECT target_ref FROM scoped
                WHERE state IN ('succeeded', 'no_signal') ORDER BY created_at DESC, target_ref DESC LIMIT 1), false),
            'signalCount', COALESCE(signal_counts.signal_count, 0),
            'method', jsonb_build_object('policyRef', page.policy_ref, 'methodName', page.method_name,
                'methodHash', page.method_hash,
                'recordingState', CASE WHEN page.method_recorded THEN 'recorded' ELSE 'legacy_unrecorded' END)
        )
    ) ORDER BY page.created_at DESC, page.target_ref DESC)
    FROM page CROSS JOIN permission
    LEFT JOIN signal_counts ON signal_counts.target_ref = page.target_ref), '[]'::jsonb)
);
