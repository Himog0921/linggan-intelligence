-- Drop the retired workflow only when its stage history is empty. A populated
-- environment requires a separate retention decision before applying this migration.
LOCK TABLE linggan_comment_research_stage, linggan_comment_problem_dispatch,
           linggan_comment_problem_stage_result IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM linggan_comment_research_stage)
       OR EXISTS (SELECT 1 FROM linggan_comment_problem_dispatch)
       OR EXISTS (SELECT 1 FROM linggan_comment_problem_stage_result) THEN
        RAISE EXCEPTION 'Retired comment research stage tables must be empty';
    END IF;
END $$;

UPDATE linggan_ai_task
SET lifecycle = 'retired'
WHERE task_key IN ('comment_study', 'comment_problem_resolution', 'comment_problem_pair_review')
  AND lifecycle <> 'retired';

-- Disable the task switch through the existing immutable command/event contract.
WITH retirement_command AS (
    INSERT INTO linggan_ai_command (command_ref, operation, payload, receipt)
    SELECT gen_random_uuid(), 'task_retire',
           jsonb_build_object('taskKey', task.task_key),
           jsonb_build_object('taskKey', task.task_key, 'enabled', false)
    FROM linggan_ai_task AS task
    JOIN linggan_ai_control AS control ON control.scope_key = 'task:' || task.task_key
    WHERE task.task_key IN ('comment_study', 'comment_problem_resolution', 'comment_problem_pair_review')
      AND control.enabled
    RETURNING command_ref, payload
)
UPDATE linggan_ai_control AS control
SET enabled = false,
    revision = control.revision + 1,
    actor_kind = 'system',
    reason = 'task_retired',
    command_ref = retirement_command.command_ref,
    updated_at = scope_001_now()
FROM retirement_command
WHERE control.scope_key = 'task:' || (retirement_command.payload->>'taskKey')
  AND control.enabled;

-- Cancel queued items that have no in-flight provider invocation. Do not rewrite
-- running items or an unresolved invocation; the batch cancellation fence stops
-- further claims and lets the existing executor/recovery path settle them.
UPDATE linggan_ai_batch_item_control AS item_control
SET state = 'cancelled',
    usable = false,
    failure_code = 'task_retired',
    finished_at = scope_001_now()
FROM linggan_ai_batch_item AS item
JOIN linggan_ai_batch AS batch USING (batch_ref)
WHERE item_control.item_ref = item.item_ref
  AND batch.task_key IN ('comment_study', 'comment_problem_resolution', 'comment_problem_pair_review')
  AND item_control.state = 'pending'
  AND (
      item_control.invocation_ref IS NULL
      OR NOT EXISTS (
          SELECT 1
          FROM linggan_model_invocation AS invocation
          WHERE invocation.invocation_ref = item_control.invocation_ref
            AND (invocation.state = 'running' OR invocation.finished_at IS NULL)
      )
  );

UPDATE linggan_ai_batch_control AS batch_control
SET cancel_requested = true,
    revision = batch_control.revision + 1,
    stop_reason = COALESCE(batch_control.stop_reason, 'task_retired')
FROM linggan_ai_batch AS batch
WHERE batch_control.batch_ref = batch.batch_ref
  AND batch.task_key IN ('comment_study', 'comment_problem_resolution', 'comment_problem_pair_review')
  AND batch_control.state = 'open'
  AND NOT batch_control.cancel_requested;

-- Settle each completed round through the existing database contract. Unresolved
-- invocations and running items remain open for the recovery path.
DO $$
DECLARE candidate uuid;
BEGIN
    FOR candidate IN
        SELECT control.batch_ref FROM linggan_ai_batch_control AS control
        JOIN linggan_ai_batch AS batch USING (batch_ref)
        WHERE batch.task_key IN ('comment_study', 'comment_problem_resolution', 'comment_problem_pair_review') AND control.state = 'open'
    LOOP
        WHILE linggan_ai_evaluate_next_round(candidate) IS NOT NULL LOOP
            NULL;
        END LOOP;
    END LOOP;
END $$;

DROP TABLE linggan_comment_problem_stage_result, linggan_comment_problem_dispatch,
           linggan_comment_research_stage;

-- Resolve current mentions directly against the effective V2 support.
CREATE OR REPLACE VIEW linggan_opportunity_current_comment_mention AS
 SELECT mention.*
 FROM linggan_opportunity_comment_mention mention
 JOIN linggan_opportunity_problem problem USING (problem_ref)
 JOIN linggan_comment_problem_support support
   ON support.problem_ref=problem.source_ref AND support.request_ref=mention.source_request_ref
   AND support.signal_ordinal=mention.source_signal_ordinal;

DROP VIEW linggan_opportunity_comment_problem_support;

CREATE OR REPLACE VIEW linggan_opportunity_comment_problem_source AS
 SELECT current.problem_ref, current.definition_ref, current.domain_ref, current.label
 FROM linggan_comment_problem_current current
 JOIN linggan_comment_problem_definition definition USING (definition_ref)
 WHERE definition.origin_invocation_ref IS NOT NULL;
