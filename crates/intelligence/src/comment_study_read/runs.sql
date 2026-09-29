-- COMMENT-STUDY-PRODUCTIZATION-001 / P1 / T44 counterexample.
-- Bound the Run page first; Work and Target are separate one-to-many relations.
WITH selected_runs AS MATERIALIZED (
    SELECT run.run_ref, run.policy_ref,
           (to_jsonb(run)->>'comment_budget')::integer AS comment_budget,
           (to_jsonb(run)->>'context_character_budget')::integer AS context_character_budget,
           (to_jsonb(run)->>'token_limit')::bigint AS token_limit,
           run.as_of, run.state, run.created_at, run.finished_at,
           to_char(run.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"') AS cursor_created_at,
           to_jsonb(run)->'selection_manifest'->>'contract' AS selection_contract,
           to_jsonb(run)->'selection_manifest'->>'recoverySourceRunRef' AS recovery_source_run_ref,
           COALESCE(to_jsonb(run)->>'dispatch_state','stopped') AS dispatch_state,
           to_jsonb(run)->>'dispatch_reason' AS dispatch_reason,
           COALESCE((to_jsonb(run)->>'control_version')::bigint,0) AS control_version
    FROM linggan_comment_study_run run
    JOIN linggan_comment_study_policy policy USING (policy_ref)
    WHERE ($1::uuid IS NULL OR policy.domain_ref = $1)
      AND run.created_at <= $2::text::timestamptz
      AND ($3::text IS NULL OR run.created_at < $3::text::timestamptz
        OR (run.created_at = $3::text::timestamptz AND run.run_ref < $4::uuid))
    ORDER BY run.created_at DESC, run.run_ref DESC
    LIMIT $5
)
SELECT run.run_ref,
       run.policy_ref,
       run.comment_budget,
       run.context_character_budget,
       run.token_limit,
       run.as_of::text AS as_of,
       run.state,
       run.created_at::text AS created_at,
       run.cursor_created_at,
       run.finished_at::text AS finished_at,
       run.selection_contract,
       run.recovery_source_run_ref,
       run.dispatch_state,
       run.dispatch_reason,
       run.control_version,
       works.work_count,
       works.primary_work_count,
       works.reference_work_count,
       targets.target_count,
       targets.pending_count,
       targets.succeeded_count,
       targets.no_signal_count,
       targets.needs_context_count,
       targets.failed_count,
       targets.excluded_count,
       targets.cancelled_count
FROM selected_runs run
CROSS JOIN LATERAL (
    SELECT count(*) AS work_count,
           count(*) FILTER (WHERE work.observation_role = 'primary') AS primary_work_count,
           count(*) FILTER (WHERE work.observation_role = 'reference') AS reference_work_count
    FROM linggan_comment_study_work work
    WHERE work.run_ref = run.run_ref
) works
CROSS JOIN LATERAL (
    SELECT count(*) AS target_count,
           count(*) FILTER (WHERE target.state IN ('ready','queued','running')) AS pending_count,
           count(*) FILTER (WHERE target.state = 'succeeded') AS succeeded_count,
           count(*) FILTER (WHERE target.state = 'no_signal') AS no_signal_count,
           count(*) FILTER (WHERE target.state = 'needs_context') AS needs_context_count,
           count(*) FILTER (WHERE target.state = 'failed') AS failed_count,
           count(*) FILTER (WHERE target.state = 'excluded') AS excluded_count,
           count(*) FILTER (WHERE target.state = 'cancelled') AS cancelled_count
    FROM linggan_comment_study_target target
    WHERE target.run_ref = run.run_ref
) targets
ORDER BY run.created_at DESC, run.run_ref DESC;
