-- OPS-LAUNCH-001 H03: stopping access is a durable, independently retryable action.
-- It deliberately precedes any later reassignment workflow: a failed handover never restores a
-- departing member's browser access or rewrites their historical work.

CREATE TABLE linggan_ops_member_deactivation_request (
    request_ref uuid PRIMARY KEY,
    actor_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    target_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    expected_version bigint NOT NULL CHECK (expected_version > 0),
    payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
    reason text NOT NULL CHECK (length(btrim(reason)) BETWEEN 1 AND 1000),
    outcome text NOT NULL CHECK (outcome IN ('accepted', 'unchanged', 'rejected', 'stale')),
    result_code text NOT NULL CHECK (length(btrim(result_code)) BETWEEN 1 AND 120),
    unassigned_account_count integer NOT NULL DEFAULT 0 CHECK (unassigned_account_count >= 0),
    resulting_version bigint,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK ((outcome IN ('accepted', 'unchanged')) = (resulting_version IS NOT NULL))
);

CREATE INDEX linggan_ops_member_deactivation_target_idx
    ON linggan_ops_member_deactivation_request (target_member_ref, created_at DESC);

CREATE TRIGGER linggan_ops_member_deactivation_request_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_member_deactivation_request
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_member_deactivation_request IS
    'Append-only member stop-access receipt. It revokes sessions and unassigns current accounts, never historical attribution.';
