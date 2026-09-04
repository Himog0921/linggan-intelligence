-- COLLECTION-CONTROL-CLOSURE-001 / Issue #149
--
-- Additive control facts for station admission, platform-account eligibility,
-- versioned monitoring rules and durable scheduler decisions.  Nothing in
-- this migration stores a raw platform account id, browser credential, Cookie,
-- page HTML or free-form platform error.

ALTER TABLE execution_station
    ADD COLUMN accepting_tasks boolean NOT NULL DEFAULT false;

CREATE TABLE execution_station_acceptance_transition (
    transition_ref uuid PRIMARY KEY,
    station_ref uuid NOT NULL REFERENCES execution_station(station_ref),
    from_accepting boolean,
    to_accepting boolean NOT NULL,
    actor text NOT NULL CHECK (actor IN ('person','system')),
    reason_code text NOT NULL CHECK (reason_code IN (
        'registered_closed','person_enabled','person_disabled','migration_closed','station_retired'
    )),
    occurred_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX execution_station_acceptance_transition_station_idx
    ON execution_station_acceptance_transition (station_ref, occurred_at DESC);

-- Existing stations deliberately enter the new gate closed.  The deterministic
-- transition id makes an isolated replay fail rather than duplicate history.
INSERT INTO execution_station_acceptance_transition (
    transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code
)
SELECT md5(station_ref::text || ':0034:migration_closed')::uuid,
       station_ref,NULL,false,'system','migration_closed'
FROM execution_station;

CREATE TABLE installation_credential (
    credential_ref uuid PRIMARY KEY,
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    credential_hash text NOT NULL CHECK (credential_hash ~ '^[0-9a-f]{64}$'),
    hash_version text NOT NULL CHECK (hash_version = 'sha256-v1'),
    issued_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    activated_at timestamptz,
    revoked_at timestamptz,
    revoke_reason_code text CHECK (revoke_reason_code IN (
        'rotated','pending_replaced','installation_superseded','person_revoked','station_retired'
    )),
    CHECK (expires_at > issued_at),
    CHECK (activated_at IS NULL OR activated_at >= issued_at),
    CHECK ((revoked_at IS NULL) = (revoke_reason_code IS NULL))
);

CREATE UNIQUE INDEX installation_credential_active_installation_idx
    ON installation_credential (installation_ref)
    WHERE revoked_at IS NULL AND activated_at IS NOT NULL;

CREATE UNIQUE INDEX installation_credential_pending_installation_idx
    ON installation_credential (installation_ref)
    WHERE revoked_at IS NULL AND activated_at IS NULL;

CREATE TABLE platform_observation_account (
    account_ref uuid PRIMARY KEY,
    platform text NOT NULL CHECK (platform = 'xhs'),
    identity_digest text NOT NULL CHECK (identity_digest ~ '^[0-9a-f]{64}$'),
    digest_version text NOT NULL CHECK (digest_version = 'hmac-sha256-v1'),
    first_observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (platform,digest_version,identity_digest)
);

COMMENT ON COLUMN platform_observation_account.identity_digest IS
    'Keyed digest produced by the server from a transient loopback account identity; raw identity is never persisted.';

CREATE TABLE platform_observation_account_binding (
    binding_ref uuid PRIMARY KEY,
    account_ref uuid NOT NULL REFERENCES platform_observation_account(account_ref),
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    bound_by text NOT NULL CHECK (bound_by = 'person'),
    bound_at timestamptz NOT NULL DEFAULT scope_001_now(),
    confirmed_until timestamptz NOT NULL,
    ended_at timestamptz,
    end_reason_code text CHECK (end_reason_code IN (
        'person_unbound','installation_superseded','identity_changed','credential_revoked',
        'account_binding_expired','station_retired'
    )),
    CHECK (confirmed_until > bound_at),
    CHECK ((ended_at IS NULL) = (end_reason_code IS NULL))
);

CREATE UNIQUE INDEX platform_observation_account_binding_active_installation_idx
    ON platform_observation_account_binding (installation_ref)
    WHERE ended_at IS NULL;

-- v1 deliberately models a one-account-to-one-installation active binding.
-- Moving either side creates a new history row after ending the old binding.
CREATE UNIQUE INDEX platform_observation_account_binding_active_account_idx
    ON platform_observation_account_binding (account_ref)
    WHERE ended_at IS NULL;

CREATE INDEX platform_observation_account_binding_account_idx
    ON platform_observation_account_binding (account_ref,bound_at DESC);

CREATE TABLE platform_observation_account_eligibility_observation (
    eligibility_ref uuid PRIMARY KEY,
    account_ref uuid NOT NULL REFERENCES platform_observation_account(account_ref),
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    eligibility_state text NOT NULL CHECK (eligibility_state IN (
        'usable','cooling','needs_login','restricted','unknown'
    )),
    signal_version text NOT NULL CHECK (signal_version = 'xhs-account-eligibility-v1'),
    reason_code text NOT NULL CHECK (reason_code IN (
        'authenticated','cooldown_observed','login_required','access_restricted','signal_incomplete'
    )),
    observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    CHECK (expires_at > observed_at),
    CHECK ((eligibility_state,reason_code) IN (
        ('usable','authenticated'),
        ('cooling','cooldown_observed'),
        ('needs_login','login_required'),
        ('restricted','access_restricted'),
        ('unknown','signal_incomplete')
    ))
);

CREATE INDEX platform_observation_account_eligibility_current_idx
    ON platform_observation_account_eligibility_observation
       (account_ref,installation_ref,observed_at DESC);

-- Admission freezes the resources and rule it actually relied on.  Later
-- replacement or revocation is rechecked; it never rewrites the decision.
ALTER TABLE collection_admission_decision
    ADD COLUMN target_ref uuid REFERENCES collection_observation_target(target_ref),
    ADD COLUMN station_ref uuid REFERENCES execution_station(station_ref),
    ADD COLUMN installation_ref uuid REFERENCES plugin_installation(installation_ref),
    ADD COLUMN account_ref uuid REFERENCES platform_observation_account(account_ref),
    ADD COLUMN eligibility_ref uuid REFERENCES platform_observation_account_eligibility_observation(eligibility_ref),
    ADD COLUMN monitor_rule_revision_ref uuid;

ALTER TABLE collection_work_order
    ADD COLUMN installation_ref uuid REFERENCES plugin_installation(installation_ref),
    ADD COLUMN account_ref uuid REFERENCES platform_observation_account(account_ref),
    ADD COLUMN eligibility_ref uuid REFERENCES platform_observation_account_eligibility_observation(eligibility_ref),
    ADD COLUMN monitor_rule_revision_ref uuid;

CREATE TABLE collection_monitor_rule_revision (
    rule_revision_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    revision integer NOT NULL CHECK (revision > 0),
    mode text NOT NULL CHECK (mode IN ('manual_only','fixed','dynamic')),
    automatic_enabled boolean NOT NULL,
    timezone text NOT NULL CHECK (timezone = 'Asia/Shanghai'),
    run_on_weekdays boolean NOT NULL DEFAULT true,
    run_on_weekends boolean NOT NULL DEFAULT true,
    all_day boolean NOT NULL DEFAULT true,
    window_start_minute smallint,
    window_end_minute smallint,
    fixed_interval_seconds integer CHECK (
        fixed_interval_seconds IS NULL OR fixed_interval_seconds BETWEEN 21600 AND 604800
    ),
    fallback_interval_seconds integer NOT NULL DEFAULT 86400
        CHECK (fallback_interval_seconds BETWEEN 21600 AND 604800),
    surface_key text NOT NULL CHECK (length(btrim(surface_key)) > 0),
    ranking_key text,
    task_contract_version text NOT NULL CHECK (length(btrim(task_contract_version)) > 0),
    rule_payload_digest text NOT NULL CHECK (rule_payload_digest ~ '^[0-9a-f]{64}$'),
    created_by text NOT NULL CHECK (created_by IN ('person','system')),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (target_ref,revision),
    UNIQUE (target_ref,rule_revision_ref),
    CHECK (
        (all_day AND window_start_minute IS NULL AND window_end_minute IS NULL)
        OR
        (NOT all_day AND window_start_minute BETWEEN 0 AND 1439
         AND window_end_minute BETWEEN 1 AND 1440
         AND window_start_minute < window_end_minute)
    ),
    CHECK (
        (mode='manual_only' AND fixed_interval_seconds IS NULL AND NOT automatic_enabled)
        OR (mode='fixed' AND fixed_interval_seconds IS NOT NULL)
        OR (mode='dynamic' AND fixed_interval_seconds IS NULL)
    ),
    CHECK (mode='manual_only' OR run_on_weekdays OR run_on_weekends)
);

ALTER TABLE collection_observation_target
    ADD COLUMN active_monitor_rule_revision_ref uuid,
    ADD COLUMN last_scheduler_considered_at timestamptz;

ALTER TABLE collection_observation_target
    ADD CONSTRAINT collection_observation_target_active_rule_fk
    FOREIGN KEY (target_ref,active_monitor_rule_revision_ref)
    REFERENCES collection_monitor_rule_revision(target_ref,rule_revision_ref);

ALTER TABLE collection_admission_decision
    ADD CONSTRAINT collection_admission_decision_monitor_rule_fk
    FOREIGN KEY (target_ref,monitor_rule_revision_ref)
    REFERENCES collection_monitor_rule_revision(target_ref,rule_revision_ref);

ALTER TABLE collection_work_order
    ADD CONSTRAINT collection_work_order_monitor_rule_fk
    FOREIGN KEY (target_ref,monitor_rule_revision_ref)
    REFERENCES collection_monitor_rule_revision(target_ref,rule_revision_ref);

CREATE TABLE collection_monitor_rule_command_identity (
    command_identity_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    idempotency_key uuid NOT NULL,
    command_kind text NOT NULL CHECK (command_kind IN (
        'save_rule','pause','resume','stop','manual_observe'
    )),
    payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
    first_outcome text NOT NULL CHECK (first_outcome IN (
        'applied','stale_revision','rejected'
    )),
    first_reason_code text NOT NULL CHECK (first_reason_code IN (
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
        'account_unknown','account_busy','station_daily_budget_reached','capacity_unknown'
    )),
    applied_rule_revision_ref uuid REFERENCES collection_monitor_rule_revision(rule_revision_ref),
    work_order_ref uuid REFERENCES collection_work_order(work_order_ref),
    lease_ref uuid REFERENCES collection_work_order_lease(lease_ref),
    established_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (target_ref,idempotency_key),
    CHECK ((work_order_ref IS NULL) = (lease_ref IS NULL))
);

CREATE TABLE collection_monitor_rule_command_receipt (
    command_receipt_ref uuid PRIMARY KEY,
    command_identity_ref uuid REFERENCES collection_monitor_rule_command_identity(command_identity_ref),
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    idempotency_key uuid NOT NULL,
    command_kind text NOT NULL CHECK (command_kind IN (
        'save_rule','pause','resume','stop','manual_observe'
    )),
    expected_revision integer NOT NULL CHECK (expected_revision >= 0),
    payload_digest text NOT NULL CHECK (payload_digest ~ '^[0-9a-f]{64}$'),
    actor text NOT NULL CHECK (actor IN ('person','system')),
    source text NOT NULL CHECK (source IN ('targets_ui','scheduler','local_api')),
    outcome text NOT NULL CHECK (outcome IN (
        'applied','replay','stale_revision','identity_conflict','rejected'
    )),
    reason_code text NOT NULL CHECK (reason_code IN (
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
        'account_unknown','account_busy','station_daily_budget_reached','capacity_unknown'
    )),
    applied_rule_revision_ref uuid REFERENCES collection_monitor_rule_revision(rule_revision_ref),
    work_order_ref uuid REFERENCES collection_work_order(work_order_ref),
    lease_ref uuid REFERENCES collection_work_order_lease(lease_ref),
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX collection_monitor_rule_receipt_target_idx
    ON collection_monitor_rule_command_receipt (target_ref,recorded_at DESC);

CREATE TABLE collection_scheduler_run (
    scheduler_run_ref uuid PRIMARY KEY,
    scheduler_key text NOT NULL CHECK (scheduler_key='patrol'),
    started_at timestamptz NOT NULL DEFAULT scope_001_now(),
    completed_at timestamptz,
    outcome text CHECK (outcome IN ('idle','dispatched','partial','failed')),
    considered_count integer NOT NULL DEFAULT 0 CHECK (considered_count >= 0),
    dispatched_count integer NOT NULL DEFAULT 0 CHECK (dispatched_count >= 0),
    CHECK ((completed_at IS NULL) = (outcome IS NULL))
);

CREATE TABLE collection_scheduler_target_decision (
    target_decision_ref uuid PRIMARY KEY,
    scheduler_run_ref uuid NOT NULL REFERENCES collection_scheduler_run(scheduler_run_ref),
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    rule_revision_ref uuid REFERENCES collection_monitor_rule_revision(rule_revision_ref),
    outcome text NOT NULL CHECK (outcome IN ('dispatched','skipped','deferred','rejected')),
    reason_code text NOT NULL CHECK (reason_code IN (
        'dispatched','rule_missing','rule_revision_changed','manual_only','monitoring_paused','not_due',
        'dynamic_unavailable','baseline_not_ready','target_not_requestable',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling',
        'account_needs_login','account_restricted','account_unknown','account_busy',
        'station_daily_budget_reached','capacity_unknown',
        'authorization_missing','authorization_purpose_mismatch',
        'authorization_target_limit_reached','authorization_expired_or_revoked',
        'admission_refused','lease_issue_failed','database_error'
    )),
    cadence_source text CHECK (cadence_source IN ('fixed','dynamic','fixed_fallback')),
    effective_interval_seconds integer CHECK (
        effective_interval_seconds IS NULL OR effective_interval_seconds BETWEEN 21600 AND 604800
    ),
    dynamic_reason_code text CHECK (dynamic_reason_code IS NULL OR dynamic_reason_code='dynamic_unavailable'),
    next_eligible_at timestamptz,
    work_order_ref uuid REFERENCES collection_work_order(work_order_ref),
    lease_ref uuid REFERENCES collection_work_order_lease(lease_ref),
    decided_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (scheduler_run_ref,target_ref)
);

CREATE INDEX collection_scheduler_target_decision_target_idx
    ON collection_scheduler_target_decision (target_ref,decided_at DESC);

CREATE INDEX collection_observation_target_scheduler_fair_idx
    ON collection_observation_target
       (last_scheduler_considered_at NULLS FIRST,target_ref)
    WHERE monitoring_enabled;

COMMENT ON TABLE installation_credential IS
    'Hash-only local installation credentials. Raw credential material is returned once and never stored.';
COMMENT ON COLUMN execution_station.accepting_tasks IS
    'Explicit person-controlled admission gate. 0034 closes every historical station; shared migration requires a person to enable each station deliberately.';
COMMENT ON TABLE collection_monitor_rule_command_receipt IS
    'Durable command outcome; saving a rule is not proof that collection ran.';
COMMENT ON TABLE collection_scheduler_target_decision IS
    'Per-target scheduler decision. It contains control facts only, never Evidence content.';
