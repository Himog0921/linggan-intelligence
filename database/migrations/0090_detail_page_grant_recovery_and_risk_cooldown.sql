-- DETAIL-PAGE-GRANT-RECOVERY-AND-RISK-COOLDOWN-001
--
-- Grant transport is not browser navigation.  Keep the server-side grant
-- attempts auditable without turning them into Attempts or Evidence.  A
-- separate installation-scoped circuit breaker prevents one browser from
-- repeatedly opening detail pages after XHS displays an explicit risk prompt.

CREATE TABLE collection_detail_page_session_grant_attempt (
    grant_attempt_ref uuid PRIMARY KEY,
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    lease_ref uuid NOT NULL REFERENCES collection_work_order_lease(lease_ref),
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    grant_request_id uuid NOT NULL,
    attempt_no integer NOT NULL CHECK (attempt_no > 0),
    outcome text NOT NULL CHECK (outcome IN ('authorized','replay','suppressed')),
    session_ref uuid REFERENCES collection_detail_page_session(session_ref),
    requested_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (installation_ref, grant_request_id, attempt_no)
);

CREATE INDEX collection_detail_page_session_grant_attempt_request_idx
    ON collection_detail_page_session_grant_attempt (installation_ref, grant_request_id, requested_at DESC);

ALTER TABLE collection_work_order_lease_task_dispatch_failure
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_dispatch_failure_failure_code_check;
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD CONSTRAINT collection_work_order_lease_task_dispatch_failure_failure_code_check
    CHECK (failure_code IN (
        'capability_not_executable_here','target_incomplete','tab_unavailable','page_timeout',
        'page_unavailable','page_receipt_missing','page_receipt_identity_mismatch','page_read_failed',
        'account_observation_blocked','execution_locator_unavailable',
        'detail_page_session_grant_unavailable'
    ));

CREATE TABLE collection_installation_risk_signal (
    risk_signal_ref uuid PRIMARY KEY,
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    lease_ref uuid NOT NULL REFERENCES collection_work_order_lease(lease_ref),
    detail_page_session_ref uuid REFERENCES collection_detail_page_session(session_ref),
    platform text NOT NULL CHECK (platform IN ('xhs')),
    signal_code text NOT NULL CHECK (signal_code IN ('risk_control_interstitial')),
    detector_version text NOT NULL CHECK (length(btrim(detector_version)) > 0 AND length(detector_version) <= 80),
    observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (risk_signal_ref)
);

CREATE INDEX collection_installation_risk_signal_streak_idx
    ON collection_installation_risk_signal (installation_ref, platform, observed_at DESC);

CREATE TABLE collection_installation_risk_cooldown (
    installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    platform text NOT NULL CHECK (platform IN ('xhs')),
    trigger_signal_ref uuid NOT NULL REFERENCES collection_installation_risk_signal(risk_signal_ref),
    trigger_count integer NOT NULL CHECK (trigger_count >= 2),
    opened_at timestamptz NOT NULL DEFAULT scope_001_now(),
    until_at timestamptz NOT NULL,
    PRIMARY KEY (installation_ref, platform),
    CHECK (until_at > opened_at)
);

COMMENT ON TABLE collection_detail_page_session_grant_attempt IS
    'Append-only server grant outcomes. It is neither browser navigation proof nor a producer Attempt.';
COMMENT ON TABLE collection_installation_risk_signal IS
    'Conclusive risk interstitial observations from an already-open claimed page; no raw page text or screenshot is retained.';
COMMENT ON TABLE collection_installation_risk_cooldown IS
    'Automatic installation-scoped dispatch circuit breaker. A live row blocks only this plugin installation until until_at; it does not change the human accepting_tasks setting.';
