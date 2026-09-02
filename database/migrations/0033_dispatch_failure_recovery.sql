-- DISPATCH-RECOVERY-001
--
-- A dispatch claim is only an exclusive permission to begin execution; it is
-- not proof that a browser page became ready.  Keep each page-start failure as
-- an append-only fact, then return the *current eligibility* of the scheduled
-- task to pending.  Without this boundary a transient tab/content-script
-- failure leaves an otherwise valid task in_progress until its whole lease
-- expires, which is both invisible and needlessly blocks the ordered queue.

CREATE TABLE collection_work_order_lease_task_dispatch_failure (
    failure_ref uuid PRIMARY KEY,
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),

    -- This is a deliberately small, implementation-independent vocabulary.
    -- Raw browser/website error text is not durable Evidence and must not be
    -- copied into the local intelligence database.
    failure_code text NOT NULL CHECK (failure_code IN (
        'capability_not_executable_here',
        'target_incomplete',
        'tab_unavailable',
        'page_timeout',
        'page_unavailable',
        'page_receipt_missing',
        'page_receipt_identity_mismatch',
        'page_read_failed'
    )),
    occurred_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX collection_work_order_lease_task_dispatch_failure_task_idx
    ON collection_work_order_lease_task_dispatch_failure (task_id, occurred_at DESC);

COMMENT ON TABLE collection_work_order_lease_task_dispatch_failure IS
    'Append-only execution-start failures reported by the installation that held a scheduled task claim; not source Evidence and not a producer Attempt.';

COMMENT ON COLUMN collection_work_order_lease_task.execution_state IS
    'Current queue eligibility only: pending can follow an append-only dispatch failure; prior claims remain in collection_work_order_lease_task_dispatch_failure.';
