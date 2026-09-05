-- COLLECTION-SCHEDULING-CLARITY-001
--
-- A monitoring rule says *when a target should be observed*.  Historical archive
-- completeness is a result-quality fact, not a precondition for creating that
-- rule.  A Work Order is the one durable browser-work queue; a station is chosen
-- only when an eligible installation claims that Work Order and receives a Lease.

-- Rule revisions stay immutable.  Mutable timing belongs to the target that owns
-- the active revision, so a scheduler can advance `next` atomically with creating
-- its Work Order without rewriting a historical rule revision.
ALTER TABLE collection_observation_target
    ADD COLUMN monitor_schedule_anchor_at timestamptz,
    ADD COLUMN monitor_next_run_at timestamptz,
    ADD COLUMN monitor_missed_run_count integer NOT NULL DEFAULT 0
        CHECK (monitor_missed_run_count >= 0);

-- Repair legacy states that claimed automatic monitoring without one active,
-- fixed automatic rule.  A historical dynamic rule is retained as history,
-- but it is not silently presented as running after the fixed-interval
-- scheduler replaces that second scheduling model.
INSERT INTO collection_observation_target_transition
    (transition_ref,target_ref,from_state,to_state,actor,reason_code,reason)
SELECT gen_random_uuid(),target.target_ref,'monitoring','paused','system',
       'monitor_rule_missing_repaired',
       '0036 closed a legacy monitoring state with no enabled active rule'
FROM collection_observation_target target
LEFT JOIN collection_monitor_rule_revision rule
  ON rule.rule_revision_ref=target.active_monitor_rule_revision_ref
WHERE target.lifecycle_state='monitoring'
  AND (target.active_monitor_rule_revision_ref IS NULL
       OR COALESCE(rule.automatic_enabled,false)=false
       OR COALESCE(rule.mode,'')<>'fixed');

UPDATE collection_observation_target target
SET monitoring_enabled=false,
    lifecycle_state=CASE WHEN lifecycle_state='monitoring' THEN 'paused' ELSE lifecycle_state END,
    lifecycle_changed_at=CASE WHEN lifecycle_state='monitoring' THEN scope_001_now()
                              ELSE lifecycle_changed_at END,
    monitor_schedule_anchor_at=NULL,
    monitor_next_run_at=NULL
FROM collection_monitor_rule_revision rule
WHERE target.active_monitor_rule_revision_ref=rule.rule_revision_ref
  AND target.monitoring_enabled
  AND (NOT rule.automatic_enabled OR rule.mode<>'fixed');

UPDATE collection_observation_target
SET monitoring_enabled=false,
    lifecycle_state=CASE WHEN lifecycle_state='monitoring' THEN 'paused' ELSE lifecycle_state END,
    lifecycle_changed_at=CASE WHEN lifecycle_state='monitoring' THEN scope_001_now()
                              ELSE lifecycle_changed_at END,
    monitor_schedule_anchor_at=NULL,
    monitor_next_run_at=NULL
WHERE monitoring_enabled AND active_monitor_rule_revision_ref IS NULL;

-- Valid legacy rules get one catch-up opportunity, never one Work Order per
-- missed interval.  A completed or failed run moves the schedule from its
-- anchor; no completion-time drift is introduced.
UPDATE collection_observation_target target
SET monitor_schedule_anchor_at=COALESCE(last_patrol_dispatched_at,scope_001_now()),
    monitor_next_run_at=COALESCE(
        last_patrol_dispatched_at + make_interval(secs=>patrol_interval_seconds),
        scope_001_now()
    )
FROM collection_monitor_rule_revision rule
WHERE target.monitoring_enabled
  AND target.active_monitor_rule_revision_ref=rule.rule_revision_ref
  AND rule.automatic_enabled
  AND rule.mode='fixed';

ALTER TABLE collection_observation_target
    ADD CONSTRAINT collection_observation_target_monitoring_requires_rule
    CHECK (NOT monitoring_enabled OR active_monitor_rule_revision_ref IS NOT NULL);

ALTER TABLE collection_observation_target
    ADD CONSTRAINT collection_observation_target_monitoring_lifecycle_requires_rule
    CHECK (lifecycle_state <> 'monitoring' OR
           (monitoring_enabled AND active_monitor_rule_revision_ref IS NOT NULL));

CREATE INDEX collection_observation_target_due_rule_idx
    ON collection_observation_target (monitor_next_run_at,target_ref)
    WHERE monitoring_enabled AND monitor_next_run_at IS NOT NULL;

-- Structured authorization scope replaces exact matching on a human-readable
-- purpose string.  `purpose` remains an immutable audit explanation.
ALTER TABLE collection_acquisition_authorization
    ADD COLUMN allowed_task_templates text[],
    ADD COLUMN allowed_dispatch_lanes text[],
    ADD COLUMN max_work_units integer CHECK (max_work_units IS NULL OR max_work_units > 0);

UPDATE collection_acquisition_authorization
SET allowed_task_templates=CASE
        WHEN lane='patrol' AND target_kind='creator' THEN ARRAY['creator_patrol']::text[]
        WHEN lane='patrol' AND target_kind='keyword' THEN ARRAY['keyword_patrol']::text[]
        WHEN lane='deep_archive' AND target_kind='creator'
            THEN ARRAY['creator_archive','material_deepening']::text[]
        ELSE ARRAY['keyword_archive','material_deepening']::text[]
    END,
    allowed_dispatch_lanes=CASE
        WHEN lane='patrol' THEN ARRAY['immediate','scheduled']::text[]
        ELSE ARRAY['immediate','batch']::text[]
    END,
    max_work_units=COALESCE(max_works_per_target,200);

ALTER TABLE collection_acquisition_authorization
    ALTER COLUMN allowed_task_templates SET NOT NULL,
    ALTER COLUMN allowed_dispatch_lanes SET NOT NULL,
    ALTER COLUMN max_work_units SET NOT NULL,
    ADD CONSTRAINT collection_acquisition_authorization_task_templates_nonempty
        CHECK (cardinality(allowed_task_templates) > 0),
    ADD CONSTRAINT collection_acquisition_authorization_dispatch_lanes_closed
        CHECK (allowed_dispatch_lanes <@ ARRAY['immediate','scheduled','batch']::text[]
               AND cardinality(allowed_dispatch_lanes) > 0);

CREATE INDEX collection_acquisition_authorization_structured_scope_idx
    ON collection_acquisition_authorization (platform,target_kind,lane,expires_at DESC)
    WHERE revoked_at IS NULL;

-- `collection_work_order` is the single durable browser-work pool.  Existing
-- orders retain `legacy`, rather than being guessed into the new queue.
ALTER TABLE collection_work_order
    ADD COLUMN dispatch_lane text NOT NULL DEFAULT 'legacy'
        CHECK (dispatch_lane IN ('legacy','immediate','scheduled','batch')),
    ADD COLUMN queue_state text NOT NULL DEFAULT 'legacy'
        CHECK (queue_state IN ('legacy','queued','leased','completed','cancelled')),
    ADD COLUMN scheduled_for timestamptz,
    ADD COLUMN dedupe_key text,
    ADD COLUMN estimated_work_units integer NOT NULL DEFAULT 1
        CHECK (estimated_work_units > 0);

ALTER TABLE collection_work_order
    ADD CONSTRAINT collection_work_order_new_queue_has_schedule
    CHECK (queue_state IN ('legacy','cancelled') OR scheduled_for IS NOT NULL);

CREATE UNIQUE INDEX collection_work_order_active_dedupe_idx
    ON collection_work_order (dedupe_key)
    WHERE dedupe_key IS NOT NULL AND queue_state IN ('queued','leased');

-- A scheduled rule run is an idempotent fact: even two scheduler invocations
-- must never create two Work Orders for the same immutable rule revision and
-- scheduled point.  Manual/immediate work deliberately does not use this key.
CREATE UNIQUE INDEX collection_work_order_scheduled_rule_once_idx
    ON collection_work_order (monitor_rule_revision_ref,scheduled_for)
    WHERE dispatch_lane='scheduled'
      AND monitor_rule_revision_ref IS NOT NULL
      AND scheduled_for IS NOT NULL;

CREATE INDEX collection_work_order_dispatch_queue_idx
    ON collection_work_order (dispatch_lane,scheduled_for,created_at)
    WHERE queue_state='queued';

-- A page-start failure happens before a producer Attempt. Keep the failed
-- Lease as history, then return its Work Order to the shared pool for a fresh
-- eligible station claim instead of pinning retries to one installation.
ALTER TABLE collection_work_order_lease
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_release_reason_check;
ALTER TABLE collection_work_order_lease
    ADD CONSTRAINT collection_work_order_lease_release_reason_check
    CHECK (release_reason IN (
        'completed','expired','revoked','station_unavailable','dispatch_start_failed'
    ));

-- Three technical dispatch lanes, not module queues.  These are policy rows,
-- not a second task authority: each claim locks one row, advances its virtual
-- finish by estimated work / weight, and then claims one Work Order.
CREATE TABLE collection_dispatch_lane_fairness (
    dispatch_lane text PRIMARY KEY CHECK (dispatch_lane IN ('immediate','scheduled','batch')),
    weight integer NOT NULL CHECK (weight > 0),
    concurrent_cap integer CHECK (concurrent_cap > 0),
    virtual_finish double precision NOT NULL DEFAULT 0 CHECK (virtual_finish >= 0),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

INSERT INTO collection_dispatch_lane_fairness
    (dispatch_lane,weight,concurrent_cap)
VALUES ('immediate',3,3),('scheduled',2,NULL),('batch',1,3);

-- A command can honestly create a queued Work Order before a station has
-- claimed it.  Only a Lease requires a Work Order; the old equality incorrectly
-- required both to appear together.
DO $$
DECLARE constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid='collection_monitor_rule_command_identity'::regclass
      AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%work_order_ref IS NULL%lease_ref IS NULL%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE collection_monitor_rule_command_identity DROP CONSTRAINT %I',constraint_name);
    END IF;
END $$;

ALTER TABLE collection_monitor_rule_command_identity
    ADD CONSTRAINT collection_monitor_rule_command_identity_lease_needs_work_order
    CHECK (lease_ref IS NULL OR work_order_ref IS NOT NULL);

-- Scheduler decisions distinguish “queued for a station to claim” from a
-- browser dispatch.  Historical reasons remain readable.
ALTER TABLE collection_scheduler_run
    DROP CONSTRAINT collection_scheduler_run_outcome_check;
ALTER TABLE collection_scheduler_run
    ADD CONSTRAINT collection_scheduler_run_outcome_check
    CHECK (outcome IN ('idle','queued','dispatched','partial','failed'));

ALTER TABLE collection_scheduler_heartbeat
    DROP CONSTRAINT collection_scheduler_heartbeat_last_outcome_check;
ALTER TABLE collection_scheduler_heartbeat
    ADD CONSTRAINT collection_scheduler_heartbeat_last_outcome_check
    CHECK (last_outcome IN ('unknown','idle','queued','dispatched','partial','failed'));

ALTER TABLE collection_scheduler_target_decision
    DROP CONSTRAINT collection_scheduler_target_decision_outcome_check,
    DROP CONSTRAINT collection_scheduler_target_decision_reason_code_check;
ALTER TABLE collection_scheduler_target_decision
    ADD CONSTRAINT collection_scheduler_target_decision_outcome_check
    CHECK (outcome IN ('queued','dispatched','skipped','deferred','rejected')),
    ADD CONSTRAINT collection_scheduler_target_decision_reason_code_check
    CHECK (reason_code IN (
        'queued','dispatched','rule_missing','rule_revision_changed','manual_only','monitoring_paused','not_due',
        'dynamic_unavailable','baseline_not_ready','target_not_requestable',
        'in_flight_work_covers_it',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling','account_needs_login','account_restricted',
        'account_unknown','account_busy','station_daily_budget_reached','capacity_unknown',
        'authorization_missing','authorization_purpose_mismatch','authorization_target_limit_reached',
        'authorization_scope_mismatch','authorization_work_unit_limit_reached',
        'authorization_expired_or_revoked','admission_refused','lease_issue_failed','database_error'
    ));

COMMENT ON COLUMN collection_observation_target.monitor_next_run_at IS
    'Mutable schedule state for the active monitoring rule. It advances with a queued Work Order, never with Capture completion.';
COMMENT ON TABLE collection_dispatch_lane_fairness IS
    'Three bounded claim lanes. Work Orders remain the single browser-work queue authority.';
