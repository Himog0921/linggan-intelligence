-- Retire the standalone note-analysis task. Historical requests, results and immutable
-- task configuration remain available for audit; no new work may be dispatched.
UPDATE linggan_ai_task
SET lifecycle = 'retired'
WHERE task_key = 'note_interpretation'
  AND lifecycle <> 'retired';

-- Disable the task switch through the existing immutable command/event contract.
WITH retirement_command AS (
    INSERT INTO linggan_ai_command (command_ref, operation, payload, receipt)
    SELECT gen_random_uuid(),
           'task_retire',
           jsonb_build_object('taskKey', 'note_interpretation'),
           jsonb_build_object('taskKey', 'note_interpretation', 'enabled', false)
    WHERE EXISTS (
        SELECT 1
        FROM linggan_ai_control
        WHERE scope_key = 'task:note_interpretation'
          AND enabled
    )
    RETURNING command_ref
)
UPDATE linggan_ai_control AS control
SET enabled = false,
    revision = control.revision + 1,
    actor_kind = 'system',
    reason = 'task_retired',
    command_ref = retirement_command.command_ref,
    updated_at = scope_001_now()
FROM retirement_command
WHERE control.scope_key = 'task:note_interpretation'
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
  AND batch.task_key = 'note_interpretation'
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
  AND batch.task_key = 'note_interpretation'
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
        WHERE batch.task_key = 'note_interpretation' AND control.state = 'open'
    LOOP
        WHILE linggan_ai_evaluate_next_round(candidate) IS NOT NULL LOOP
            NULL;
        END LOOP;
    END LOOP;
END $$;
