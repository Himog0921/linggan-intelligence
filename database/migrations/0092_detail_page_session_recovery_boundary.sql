-- DETAIL-PAGE-SESSION-RECOVERY-BOUNDARY-001
--
-- A browser may prove that it must not navigate again while being unable to
-- deliver the same-page cache. This is neither a transient grant transport
-- failure nor a page-read retry: record it and terminalize the frozen detail
-- lanes as unavailable so a claim cannot remain in_progress indefinitely.

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
        'detail_page_session_grant_unavailable','detail_page_session_recovery_required'
    ));

COMMENT ON COLUMN collection_work_order_lease_task_dispatch_failure.failure_code IS
    'Bounded pre-Attempt browser dispatch outcomes. detail_page_session_recovery_required means no automatic navigation can safely recover the frozen detail lanes.';
