-- COLLECTION-DOSSIER-RELIABILITY-002
--
-- A repeated page-read failure is neither accepted Evidence nor proof that the
-- source page disappeared. It must nevertheless stop consuming browser capacity
-- forever. Preserve every failure in the append-only ledger, then represent the
-- current task as `blocked`: its approved page could not be read after bounded
-- retries and requires a later, explicitly new execution attempt.

ALTER TABLE collection_work_order_lease_task
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_check;
ALTER TABLE collection_work_order_lease_task
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_execution_state_check;
ALTER TABLE collection_work_order_lease_task
    ADD CONSTRAINT collection_work_order_lease_task_execution_state_check
    CHECK (
        (execution_state = 'pending' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'in_progress' AND claimed_at IS NOT NULL AND completed_at IS NULL)
        OR (execution_state = 'completed' AND claimed_at IS NOT NULL AND completed_at IS NOT NULL)
        OR (execution_state = 'unavailable' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'blocked' AND claimed_at IS NULL AND completed_at IS NULL)
    );

ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD COLUMN failure_disposition text NOT NULL DEFAULT 'requeued'
        CHECK (failure_disposition IN ('requeued', 'unavailable', 'blocked'));

-- Only historical rows whose task is already terminally unavailable receive
-- that disposition. Earlier recoverable `page_unavailable` rows stay replayable
-- as recoverable history; the migration must not reinterpret them.
UPDATE collection_work_order_lease_task_dispatch_failure failure
SET failure_disposition = 'unavailable'
FROM collection_work_order_lease_task task
WHERE task.task_id = failure.task_id
  AND task.execution_state = 'unavailable'
  AND failure.failure_code = 'page_unavailable';

COMMENT ON COLUMN collection_work_order_lease_task.execution_state IS
    'Current execution eligibility only: pending/in_progress are runnable; completed has an accepted Package and Receipt; unavailable is a producer-confirmed absent page; blocked is a bounded page-read failure without accepted Evidence.';
COMMENT ON COLUMN collection_work_order_lease_task_dispatch_failure.failure_disposition IS
    'Durable replay outcome for a pre-Attempt dispatch failure. It is not source Evidence and never stores raw browser or platform error text.';
