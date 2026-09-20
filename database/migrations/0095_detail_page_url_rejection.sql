-- 0095_DETAIL-PAGE-URL-REJECTION-001
--
-- A concrete signed XHS execution URL can be disproven by the final browser
-- document without proving the underlying work is deleted. Persist only a
-- SHA-256 fingerprint on the existing server-owned detail session. That lets
-- dispatch reject the same URL across later Work Orders while a newly observed
-- signed URL remains eligible naturally.

ALTER TABLE collection_detail_page_session
    ADD COLUMN execution_source_url_sha256 text
    CHECK (execution_source_url_sha256 IS NULL OR execution_source_url_sha256 ~ '^[0-9a-f]{64}$');

DO $$
DECLARE
    stop_reason_constraint record;
BEGIN
    FOR stop_reason_constraint IN
        SELECT constraint_row.conname
        FROM pg_constraint constraint_row
        WHERE constraint_row.conrelid = 'collection_detail_page_session'::regclass
          AND constraint_row.contype = 'c'
          AND pg_get_constraintdef(constraint_row.oid) LIKE '%stop_reason%'
    LOOP
        EXECUTE format(
            'ALTER TABLE collection_detail_page_session DROP CONSTRAINT %I',
            stop_reason_constraint.conname
        );
    END LOOP;
END $$;

ALTER TABLE collection_detail_page_session
    ADD CONSTRAINT collection_detail_page_session_stop_reason_check
    CHECK (stop_reason IS NULL OR stop_reason IN (
        'navigation_state_unknown','owner_unavailable','page_unavailable',
        'risk_stop','delivery_terminal','detail_page_url_invalid'
    ));

CREATE INDEX collection_detail_page_session_rejected_source_idx
    ON collection_detail_page_session (execution_source_url_sha256)
    WHERE stop_reason='detail_page_url_invalid';

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
        'detail_page_url_invalid','account_observation_blocked','execution_locator_unavailable',
        'detail_page_session_grant_unavailable','detail_page_session_recovery_required',
        'capture_delivery_rejected'
    ));

COMMENT ON COLUMN collection_detail_page_session.execution_source_url_sha256 IS
    'SHA-256 of the server-verified ephemeral execution URL. Raw signed URL and token are never persisted.';
