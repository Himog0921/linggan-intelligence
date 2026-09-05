-- COLLECTION-SCHEDULER-SCALE-001
--
-- This migration scales the existing Rule -> WorkOrder -> Lease path.  It does
-- not introduce a second queue, a baseline state, or a separate scheduler
-- authority.  Every mutable control here is persisted so concurrent Runtime
-- processes and later operators see the same fact.

-- A fixed rule gets a stable interval phase when it is saved or resumed.  The
-- phase belongs to mutable target scheduling state, not immutable rule history.
-- Existing schedules retain phase zero until their next user-authored revision;
-- rewriting an already published next_run_at would be an unannounced reschedule.
ALTER TABLE collection_observation_target
    ADD COLUMN monitor_schedule_slot_seconds integer NOT NULL DEFAULT 0
        CHECK (monitor_schedule_slot_seconds >= 0);

-- Retry timing is a WorkOrder fact.  A dispatch-start failure is retained on
-- its old Lease/Task, while a fresh eligible station may claim only after this
-- time.  Batch group is a fairness key, never a business queue name.
ALTER TABLE collection_work_order
    ADD COLUMN dispatch_group_key text,
    ADD COLUMN retry_not_before_at timestamptz NOT NULL DEFAULT scope_001_now(),
    ADD COLUMN dispatch_failure_count integer NOT NULL DEFAULT 0
        CHECK (dispatch_failure_count >= 0),
    ADD CONSTRAINT collection_work_order_dispatch_group_only_for_batch
        CHECK (dispatch_lane='batch' OR dispatch_group_key IS NULL);

UPDATE collection_work_order
SET dispatch_group_key=concat('target:',target_ref::text)
WHERE dispatch_lane='batch' AND dispatch_group_key IS NULL;

DROP INDEX IF EXISTS collection_work_order_dispatch_queue_idx;
CREATE INDEX collection_work_order_dispatch_queue_ready_idx
    ON collection_work_order
       (dispatch_lane,retry_not_before_at,scheduled_for,created_at,work_order_ref)
    WHERE queue_state='queued';
CREATE INDEX collection_work_order_batch_group_ready_idx
    ON collection_work_order
       (dispatch_lane,dispatch_group_key,retry_not_before_at,scheduled_for,created_at,work_order_ref)
    WHERE queue_state='queued' AND dispatch_lane='batch';

-- Keep the reported delay alongside the immutable dispatch-failure record, so
-- replaying a producer's failure acknowledgement returns the original answer.
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD COLUMN retry_after_seconds integer NOT NULL DEFAULT 60
        CHECK (retry_after_seconds > 0 AND retry_after_seconds <= 900);
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_dispatch_failure_failure_code_check;
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
        'execution_locator_unavailable'
    ));

ALTER TABLE collection_work_order_lease
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_release_reason_check;
ALTER TABLE collection_work_order_lease
    ADD CONSTRAINT collection_work_order_lease_release_reason_check
    CHECK (release_reason IN (
        'completed','expired','revoked','station_unavailable','dispatch_start_failed',
        'execution_locator_unavailable'
    ));

-- These remain the three technical lanes.  The multiplier says how much ready
-- batch work may be kept per currently eligible claimant; it is deliberately
-- policy data rather than a hidden worker-loop constant.
ALTER TABLE collection_dispatch_lane_fairness
    ADD COLUMN ready_work_multiplier integer NOT NULL DEFAULT 1
        CHECK (ready_work_multiplier > 0);
UPDATE collection_dispatch_lane_fairness
SET ready_work_multiplier=2
WHERE dispatch_lane='batch';

-- Platform concurrency is a third hard capacity layer after a station and its
-- bound account.  Claim issuance locks this row before counting live leases,
-- making the cap correct across processes rather than a best-effort dashboard.
CREATE TABLE collection_platform_dispatch_policy (
    platform text PRIMARY KEY,
    concurrent_cap integer NOT NULL CHECK (concurrent_cap > 0),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);
INSERT INTO collection_platform_dispatch_policy (platform,concurrent_cap)
VALUES ('xhs',10);

ALTER TABLE collection_scheduler_target_decision
    DROP CONSTRAINT collection_scheduler_target_decision_reason_code_check;
ALTER TABLE collection_scheduler_target_decision
    ADD CONSTRAINT collection_scheduler_target_decision_reason_code_check
    CHECK (reason_code IN (
        'queued','dispatched','rule_missing','rule_revision_changed','manual_only','monitoring_paused','not_due',
        'dynamic_unavailable','baseline_not_ready','target_not_requestable',
        'in_flight_work_covers_it',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling','account_needs_login','account_restricted',
        'account_unknown','account_busy','station_daily_budget_reached','platform_concurrency_reached',
        'capacity_unknown',
        'authorization_missing','authorization_purpose_mismatch','authorization_target_limit_reached',
        'authorization_scope_mismatch','authorization_work_unit_limit_reached',
        'authorization_expired_or_revoked','admission_refused','lease_issue_failed','database_error'
    ));

COMMENT ON COLUMN collection_observation_target.monitor_schedule_slot_seconds IS
    'Stable phase within the selected fixed interval. It is assigned on Rule save/resume and is visible through persisted monitor_next_run_at.';
COMMENT ON COLUMN collection_work_order.retry_not_before_at IS
    'Earliest time a queued WorkOrder may be claimed after a recoverable dispatch or lease failure.';
COMMENT ON COLUMN collection_work_order.dispatch_group_key IS
    'Batch-lane fairness group. It does not create another queue or business lifecycle.';
COMMENT ON TABLE collection_platform_dispatch_policy IS
    'Lockable platform-wide concurrent Lease limit. The WorkOrder queue remains the sole browser-work authority.';
