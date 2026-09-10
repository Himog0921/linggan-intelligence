-- ACCOUNT-OBSERVATION-BOOTSTRAP-002 · a definitive block can occur before a
-- page exposes the authenticated identity.  Keep that fact at the installation
-- boundary rather than inventing an account, a binding, or an UNKNOWN record.
--
-- A NULL account_ref is therefore permitted only for a closed negative signal;
-- the application validates the closed signal/state cross-product before this
-- table is written. Existing account-scoped observations and all binding
-- history remain unchanged.

ALTER TABLE platform_observation_account_eligibility_observation
    ALTER COLUMN account_ref DROP NOT NULL;

-- `scope_001_now()` is intentionally stable during a transaction. Two reports may therefore
-- share the same server timestamp; UUIDv4 is not a causal tie-breaker. The identity sequence is
-- append order at this table boundary and makes the most recently accepted observation decisive.
-- Existing rows were historically ordered by `(observed_at, eligibility_ref)`. Backfill the new
-- monotonic field in precisely that order before future writes receive sequence values, so an
-- upgrade cannot reverse an already-established usable/negative projection.
ALTER TABLE platform_observation_account_eligibility_observation
    ADD COLUMN observation_sequence bigint;

WITH ordered_observation AS (
    SELECT eligibility_ref,
           row_number() OVER (ORDER BY observed_at,eligibility_ref) AS observation_sequence
    FROM platform_observation_account_eligibility_observation
)
UPDATE platform_observation_account_eligibility_observation observation
SET observation_sequence=ordered_observation.observation_sequence
FROM ordered_observation
WHERE ordered_observation.eligibility_ref=observation.eligibility_ref;

ALTER TABLE platform_observation_account_eligibility_observation
    ALTER COLUMN observation_sequence SET NOT NULL;

CREATE SEQUENCE platform_observation_account_eligibility_observation_sequence_seq;
SELECT setval(
    'platform_observation_account_eligibility_observation_sequence_seq',
    COALESCE((SELECT max(observation_sequence) FROM platform_observation_account_eligibility_observation), 1),
    EXISTS (SELECT 1 FROM platform_observation_account_eligibility_observation)
);
ALTER SEQUENCE platform_observation_account_eligibility_observation_sequence_seq
    OWNED BY platform_observation_account_eligibility_observation.observation_sequence;
ALTER TABLE platform_observation_account_eligibility_observation
    ALTER COLUMN observation_sequence SET DEFAULT nextval(
        'platform_observation_account_eligibility_observation_sequence_seq'
    );

DROP INDEX platform_observation_account_eligibility_installation_current_idx;
CREATE INDEX platform_observation_account_eligibility_installation_current_idx
    ON platform_observation_account_eligibility_observation
       (installation_ref,observation_sequence DESC);

COMMENT ON COLUMN platform_observation_account_eligibility_observation.account_ref IS
    'Authenticated observations reference a keyed account digest. A NULL value is an explicit installation-level negative observation recorded when no authenticated account identity is available; it never invents an account or binding.';

COMMENT ON COLUMN platform_observation_account_eligibility_observation.observation_sequence IS
    'Monotonic database append order for eligibility observations. It is the authoritative latest-fact order when server observation timestamps tie.';

-- A first-task installation lock is distinct from the account lock. Scheduler and command
-- receipts must persist that concrete wait reason instead of collapsing it into account_unknown.
DO $$
DECLARE constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid='collection_monitor_rule_command_identity'::regclass
      AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%first_reason_code%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE collection_monitor_rule_command_identity DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;
ALTER TABLE collection_monitor_rule_command_identity
    ADD CONSTRAINT collection_command_identity_reason_codes_ck
    CHECK (first_reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision',
        'invalid_mode','invalid_interval','invalid_schedule','baseline_not_ready',
        'target_not_requestable','database_unavailable',
        'authorization_missing','authorization_purpose_mismatch',
        'authorization_target_limit_reached','authorization_expired_or_revoked',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling','account_needs_login','account_restricted',
        'account_unknown','account_busy','station_busy','station_daily_budget_reached','capacity_unknown'
    ));

DO $$
DECLARE constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid='collection_monitor_rule_command_receipt'::regclass
      AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%reason_code%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE collection_monitor_rule_command_receipt DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;
ALTER TABLE collection_monitor_rule_command_receipt
    ADD CONSTRAINT collection_command_receipt_reason_codes_ck
    CHECK (reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision',
        'identity_conflict','invalid_mode','invalid_interval','invalid_schedule',
        'baseline_not_ready','target_not_requestable','database_unavailable',
        'authorization_missing','authorization_purpose_mismatch',
        'authorization_target_limit_reached','authorization_expired_or_revoked',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling','account_needs_login','account_restricted',
        'account_unknown','account_busy','station_busy','station_daily_budget_reached','capacity_unknown'
    ));

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
        'account_unknown','account_busy','station_busy','station_daily_budget_reached','platform_concurrency_reached',
        'capacity_unknown',
        'authorization_missing','authorization_purpose_mismatch','authorization_target_limit_reached',
        'authorization_scope_mismatch','authorization_work_unit_limit_reached',
        'authorization_expired_or_revoked','admission_refused','lease_issue_failed','database_error'
    ));
