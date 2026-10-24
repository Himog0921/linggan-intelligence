-- OPS-LAUNCH-001: internal-team identity is distinct from the historical workbench names.
-- Browser secrets are always digests.  Raw session/invite/reset tokens must never be stored.

CREATE TABLE linggan_ops_member (
    member_ref uuid PRIMARY KEY,
    email_normalized text NOT NULL UNIQUE
        CHECK (email_normalized = lower(btrim(email_normalized)) AND length(email_normalized) BETWEEN 3 AND 320),
    display_name text NOT NULL CHECK (length(btrim(display_name)) BETWEEN 1 AND 160),
    role text NOT NULL CHECK (role IN ('admin', 'lead', 'operator')),
    state text NOT NULL CONSTRAINT linggan_ops_member_state_value_check
        CHECK (state IN ('invited', 'active', 'disabled')),
    password_hash text,
    version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    activated_at timestamptz,
    disabled_at timestamptz,
    CONSTRAINT linggan_ops_member_lifecycle_check CHECK (
        (state = 'invited' AND password_hash IS NULL AND activated_at IS NULL AND disabled_at IS NULL)
        OR (state = 'active' AND password_hash IS NOT NULL AND activated_at IS NOT NULL AND disabled_at IS NULL)
        OR (state = 'disabled' AND disabled_at IS NOT NULL)
    )
);

CREATE TABLE linggan_ops_session (
    session_ref uuid PRIMARY KEY,
    member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    token_sha256 bytea NOT NULL UNIQUE CHECK (octet_length(token_sha256) = 32),
    issued_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_seen_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    revoked_reason text CHECK (revoked_reason IN ('logout', 'member_disabled', 'credential_reset', 'administrator')),
    CHECK (expires_at > issued_at),
    CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);

CREATE INDEX linggan_ops_session_member_live_idx
    ON linggan_ops_session (member_ref, expires_at)
    WHERE revoked_at IS NULL;

CREATE TABLE linggan_ops_invite (
    invite_ref uuid PRIMARY KEY,
    member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    token_sha256 bytea NOT NULL UNIQUE CHECK (octet_length(token_sha256) = 32),
    invited_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    issued_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    revoked_at timestamptz,
    revoked_reason text CHECK (revoked_reason IN ('superseded', 'administrator', 'member_disabled')),
    CHECK (expires_at > issued_at),
    CHECK (NOT (accepted_at IS NOT NULL AND revoked_at IS NOT NULL)),
    CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);

CREATE UNIQUE INDEX linggan_ops_invite_one_pending_per_member_idx
    ON linggan_ops_invite (member_ref)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TABLE linggan_ops_password_reset (
    reset_ref uuid PRIMARY KEY,
    member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    token_sha256 bytea NOT NULL UNIQUE CHECK (octet_length(token_sha256) = 32),
    issued_at timestamptz NOT NULL DEFAULT scope_001_now(),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    revoked_at timestamptz,
    revoked_reason text CHECK (revoked_reason IN ('superseded', 'administrator', 'member_disabled')),
    CHECK (expires_at > issued_at),
    CHECK (NOT (consumed_at IS NOT NULL AND revoked_at IS NOT NULL)),
    CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);

COMMENT ON TABLE linggan_ops_member IS
    'Invite-only internal Operations members. Role names authorize operations; job titles never do.';
COMMENT ON TABLE linggan_ops_session IS
    'Revocable browser sessions. Only SHA-256 token digests persist.';
COMMENT ON TABLE linggan_ops_invite IS
    'One pending invite per member; historical accepted and revoked invites remain auditable.';
COMMENT ON TABLE linggan_ops_password_reset IS
    'Single-use reset token digests; raw tokens are never persisted.';
