-- psql -v domain_ref=<uuid> -v since=<timestamp with offset> -f scripts/sql/corpus-automation-progress.sql
-- Times and counts in one snapshot. Scheduler and executor are shared across domains.
\set ON_ERROR_STOP on
BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;

SELECT now() AS observed_at, (SELECT max(completed_at) FROM collection_scheduler_run
       WHERE scheduler_key='patrol') AS last_patrol_cycle, i.state, i.heartbeat_at, i.source_revision,
       (SELECT count(*) FROM linggan_ai_executor_instance WHERE started_at >= :'since'::timestamptz) AS executor_starts
FROM linggan_ai_executor_current c LEFT JOIN linggan_ai_executor_instance i USING(instance_ref,generation);

SELECT count(DISTINCT d.package_ref) AS accepted_detail_packages,
       count(DISTINCT d.content_public_ref) AS observed_works, max(p.accepted_at) AS latest_detail
FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref)
WHERE p.accepted_at >= :'since'::timestamptz AND EXISTS (
    SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=d.content_public_ref
    AND u.domain_ref=:'domain_ref'::uuid AND u.use_kind='domain_research' AND u.role='primary');

WITH items AS (
    SELECT q.item_ref FROM linggan_content_understanding_request q WHERE q.domain_ref=:'domain_ref'::uuid
    UNION ALL
    SELECT s.item_ref FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref)
    JOIN linggan_topic_research_stage s USING(window_ref) WHERE r.domain_ref=:'domain_ref'::uuid
)
SELECT b.task_key, c.state, c.failure_code, count(*) AS items, min(b.created_at) AS oldest_created,
       max(c.finished_at) AS latest_finished
FROM items JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref)
JOIN linggan_ai_batch_item_control c USING(item_ref)
WHERE b.kind='automatic' AND (b.created_at >= :'since'::timestamptz OR c.state IN ('pending','running'))
GROUP BY 1,2,3 ORDER BY 1,2,3;

SELECT enabled, auto_submit, time_zone, next_run_at,
       (SELECT max(generated_at) FROM linggan_comment_research_selection s WHERE s.domain_ref=a.domain_ref) AS latest_selection
FROM linggan_comment_research_automation a WHERE domain_ref=:'domain_ref'::uuid;
SELECT t.submission_state,t.submission_reason,count(*) AS targets,min(s.generated_at) AS oldest_selection
FROM linggan_comment_research_selection_target t JOIN linggan_comment_research_selection s USING(domain_ref,selection_ref)
WHERE t.domain_ref=:'domain_ref'::uuid AND (s.generated_at >= :'since'::timestamptz OR t.next_check_at IS NOT NULL)
GROUP BY 1,2 ORDER BY 1,2;

SELECT count(*) AS pending_cards,min(q.queued_at) AS oldest_pending,max(q.changed_at) AS latest_change,
       (SELECT max(c.built_at) FROM linggan_opportunity_opportunity_card c JOIN linggan_opportunity_problem p USING(problem_ref)
        WHERE p.domain_ref=:'domain_ref'::uuid) AS latest_card_build
FROM linggan_opportunity_card_change q JOIN linggan_opportunity_problem p USING(problem_ref)
WHERE p.domain_ref=:'domain_ref'::uuid;
COMMIT;
