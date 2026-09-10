-- ACCOUNT-OBSERVATION-NORMALIZATION-001 · account binding is a durable human fact.
--
-- A calendar deadline cannot prove that a fixed browser account has changed. Earlier control
-- code used `confirmed_until` as a hard eligibility gate; this migration removes that obsolete
-- deadline while retaining the append-only binding history and its explicit end reasons.

ALTER TABLE platform_observation_account_binding
    DROP COLUMN confirmed_until;

-- An observation is an append-only fact, not a lease.  The current projection is selected by
-- `observed_at`; a later explicit positive or negative fact replaces its operational meaning.
-- Retaining an `expires_at` check here would invite the same time-passing-as-failure mistake we
-- just removed from bindings, so make the legacy field nullable for historic rows and stop
-- producing expiry values for new observations.
ALTER TABLE platform_observation_account_eligibility_observation
    ALTER COLUMN expires_at DROP NOT NULL;

DO $$
DECLARE
    expiry_constraint text;
BEGIN
    SELECT check_constraint.constraint_name
      INTO expiry_constraint
      FROM information_schema.check_constraints check_constraint
      JOIN information_schema.constraint_column_usage usage
        ON usage.constraint_catalog=check_constraint.constraint_catalog
       AND usage.constraint_schema=check_constraint.constraint_schema
       AND usage.constraint_name=check_constraint.constraint_name
     WHERE usage.table_schema=current_schema()
       AND usage.table_name='platform_observation_account_eligibility_observation'
       AND check_constraint.check_clause LIKE '%expires_at > observed_at%'
     LIMIT 1;
    IF expiry_constraint IS NOT NULL THEN
        EXECUTE format(
            'ALTER TABLE platform_observation_account_eligibility_observation DROP CONSTRAINT %I',
            expiry_constraint
        );
    END IF;
END $$;

-- Current capacity reads first ask for the latest observation of an installation, then compare
-- that identity with the durable human binding. The old index only served account-first reads.
CREATE INDEX platform_observation_account_eligibility_installation_current_idx
    ON platform_observation_account_eligibility_observation
       (installation_ref,observed_at DESC,eligibility_ref DESC);

-- A task page can now stop before collection because it observed an explicit account block or
-- a different account. Keep that reason distinct from parser/DOM failures so Runtime and Task
-- history explain why the work was safely returned to the queue.
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    DROP CONSTRAINT collection_work_order_lease_task_dispatch_failure_failure_code_check;

ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD CONSTRAINT collection_work_order_lease_task_dispatch_failure_failure_code_check
    CHECK (failure_code IN (
        'capability_not_executable_here',
        'target_incomplete',
        'tab_unavailable',
        'page_timeout',
        'page_unavailable',
        'page_receipt_missing',
        'page_receipt_identity_mismatch',
        'page_read_failed',
        'account_observation_blocked'
    ));

COMMENT ON COLUMN platform_observation_account_binding.bound_at IS
    'Human binding creation time for audit only; a binding remains active until an explicit end event, never a calendar expiry.';
