-- CAPTURE-DELIVERY-REJECTION-001
--
-- A scheduled browser package may be conclusively rejected after the page was
-- read. This ends only that immutable Task; it never licenses another page
-- navigation and does not discard separately deliverable lanes from the same
-- cached page session.

DO $$
DECLARE
    failure_code_constraint record;
BEGIN
    FOR failure_code_constraint IN
        SELECT constraint_row.conname
        FROM pg_constraint constraint_row
        WHERE constraint_row.conrelid = 'collection_work_order_lease_task_dispatch_failure'::regclass
          AND constraint_row.contype = 'c'
          AND pg_get_constraintdef(constraint_row.oid) LIKE '%failure_code%'
    LOOP
        EXECUTE format(
            'ALTER TABLE collection_work_order_lease_task_dispatch_failure DROP CONSTRAINT %I',
            failure_code_constraint.conname
        );
    END LOOP;
END $$;

ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD CONSTRAINT collection_work_order_lease_task_dispatch_failure_failure_code_check
    CHECK (failure_code IN (
        'capability_not_executable_here','target_incomplete','tab_unavailable','page_timeout',
        'page_unavailable','page_receipt_missing','page_receipt_identity_mismatch','page_read_failed',
        'account_observation_blocked','execution_locator_unavailable',
        'detail_page_session_grant_unavailable','detail_page_session_recovery_required',
        'capture_delivery_rejected'
    ));

COMMENT ON COLUMN collection_work_order_lease_task_dispatch_failure.failure_code IS
    'Bounded browser dispatch outcomes. capture_delivery_rejected ends only the rejected immutable Package lane and never permits another page navigation.';
