-- DEEP-ARCHIVE-RECOVERY-001
--
-- A page explicitly unavailable to the approved Browser Producer is neither
-- an accepted detail nor a transient queue failure.  Keep that bounded fact
-- on the scheduled task so one inaccessible work cannot indefinitely block
-- the later, independent works in the same deep-archive batch.

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
        -- The claim that established unavailability remains in the append-only
        -- dispatch-failure table.  These rows deliberately retain no live
        -- claimant, Attempt, Package, or Receipt.
        OR (execution_state = 'unavailable' AND claimed_at IS NULL AND completed_at IS NULL)
    );

ALTER TABLE collection_work_order_lease
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_release_reason_check;
ALTER TABLE collection_work_order_lease
    ADD CONSTRAINT collection_work_order_lease_release_reason_check
    CHECK (release_reason IN (
        'completed','partial','expired','revoked','station_unavailable','dispatch_start_failed',
        'execution_locator_unavailable'
    ));

COMMENT ON COLUMN collection_work_order_lease_task.execution_state IS
    'Current execution eligibility only: pending/in_progress are runnable; completed has an accepted Package and Receipt; unavailable is a producer-confirmed unreadable approved page, not completed Evidence.';
