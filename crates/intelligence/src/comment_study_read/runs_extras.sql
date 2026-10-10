-- Resolve effective Signals once for the bounded Run page. Keep head selection inside the
-- authoritative view; filtering base Targets first would resurrect an older success.
WITH current_signals AS MATERIALIZED (
    SELECT signal.signal_ref,target.run_ref
    FROM linggan_comment_study_effective_signal signal
    JOIN linggan_comment_study_target target USING(target_ref)
    WHERE target.run_ref=ANY($1::uuid[]) AND signal.eligibility_state='eligible'
), pending_resolutions AS (
    SELECT signal.run_ref,count(*) AS count
    FROM linggan_comment_study_resolution resolution
    JOIN current_signals signal USING(signal_ref)
    WHERE resolution.state='pending'
    GROUP BY signal.run_ref
), pending_pairs AS (
    SELECT first_signal.run_ref,count(*) AS count
    FROM linggan_comment_study_problem_pair pair
    JOIN current_signals first_signal ON first_signal.signal_ref=pair.first_signal_ref
    JOIN current_signals second_signal ON second_signal.signal_ref=pair.second_signal_ref
      AND second_signal.run_ref=first_signal.run_ref
    WHERE pair.state='pending'
    GROUP BY first_signal.run_ref
)
SELECT run.run_ref,to_jsonb(policy)->>'method_name' AS method_name,
       COALESCE(resolutions.count,0)::bigint AS pending_resolution_count,
       COALESCE(pairs.count,0)::bigint AS pending_pair_count
FROM linggan_comment_study_run run
JOIN linggan_comment_study_policy policy USING(policy_ref)
LEFT JOIN pending_resolutions resolutions USING(run_ref)
LEFT JOIN pending_pairs pairs USING(run_ref)
WHERE run.run_ref=ANY($1::uuid[])
