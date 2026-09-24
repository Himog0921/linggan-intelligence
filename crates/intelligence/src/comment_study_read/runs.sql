-- COMMENT-STUDY-PRODUCTIZATION-001 / P1 / T44 counterexample.
-- Bound the Run page first; Work and Target are separate one-to-many relations.
WITH selected_runs AS MATERIALIZED (
    SELECT run.run_ref, run.as_of, run.state, run.created_at, run.finished_at
    FROM linggan_comment_study_run run
    JOIN linggan_comment_study_policy policy USING (policy_ref)
    WHERE ($1::uuid IS NULL OR policy.domain_ref = $1)
    ORDER BY run.created_at DESC, run.run_ref DESC
    LIMIT $2
)
SELECT run.run_ref,
       run.as_of::text AS as_of,
       run.state,
       run.created_at::text AS created_at,
       run.finished_at::text AS finished_at,
       works.work_count,
       targets.target_count,
       targets.succeeded_count,
       targets.no_signal_count,
       targets.needs_context_count,
       targets.failed_count,
       targets.excluded_count
FROM selected_runs run
CROSS JOIN LATERAL (
    SELECT count(*) AS work_count
    FROM linggan_comment_study_work work
    WHERE work.run_ref = run.run_ref
) works
CROSS JOIN LATERAL (
    SELECT count(*) AS target_count,
           count(*) FILTER (WHERE target.state = 'succeeded') AS succeeded_count,
           count(*) FILTER (WHERE target.state = 'no_signal') AS no_signal_count,
           count(*) FILTER (WHERE target.state = 'needs_context') AS needs_context_count,
           count(*) FILTER (WHERE target.state = 'failed') AS failed_count,
           count(*) FILTER (WHERE target.state = 'excluded') AS excluded_count
    FROM linggan_comment_study_target target
    WHERE target.run_ref = run.run_ref
) targets
ORDER BY run.created_at DESC, run.run_ref DESC;
