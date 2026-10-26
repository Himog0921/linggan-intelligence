-- OPS-LAUNCH-001 A03: one controlled console action establishes the first administrator.
-- There is deliberately no HTTP route for this transition and no personal email in source.

CREATE TABLE linggan_ops_first_admin_bootstrap (
    bootstrap_ref uuid PRIMARY KEY,
    singleton boolean NOT NULL DEFAULT true UNIQUE CHECK (singleton),
    member_ref uuid NOT NULL UNIQUE REFERENCES linggan_ops_member(member_ref),
    invite_ref uuid NOT NULL UNIQUE REFERENCES linggan_ops_invite(invite_ref),
    request_ref uuid NOT NULL UNIQUE,
    control_channel text NOT NULL CHECK (control_channel = 'local_runtime_console'),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE FUNCTION linggan_ops_validate_first_admin_bootstrap() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN
        IF NOT EXISTS (
            SELECT 1
            FROM linggan_ops_member AS member
            JOIN linggan_ops_invite AS invite ON invite.invite_ref = NEW.invite_ref
            WHERE member.member_ref = NEW.member_ref
              AND invite.member_ref = NEW.member_ref
              AND invite.invited_by_member_ref = NEW.member_ref
              AND member.role = 'admin'
              AND member.state = 'invited'
              AND invite.accepted_at IS NULL
              AND invite.revoked_at IS NULL
              AND invite.expires_at > scope_001_now()
        ) THEN
            RAISE EXCEPTION 'first Ops administrator bootstrap must reference its live invited admin and self-issued invite';
        END IF;
        IF EXISTS (SELECT 1 FROM linggan_ops_member WHERE member_ref <> NEW.member_ref) THEN
            RAISE EXCEPTION 'first Ops administrator bootstrap requires an empty team';
        END IF;
        RETURN NEW;
    END $$;

CREATE TRIGGER linggan_ops_first_admin_bootstrap_is_consistent
    BEFORE INSERT ON linggan_ops_first_admin_bootstrap
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_validate_first_admin_bootstrap();

CREATE TRIGGER linggan_ops_first_admin_bootstrap_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_first_admin_bootstrap
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_first_admin_bootstrap IS
    'The sole audited controlled bootstrap of an invited first administrator; never an HTTP self-registration path.';
COMMENT ON COLUMN linggan_ops_first_admin_bootstrap.singleton IS
    'A permanent one-row guard: a deleted or disabled member cannot reopen first-admin bootstrap.';
