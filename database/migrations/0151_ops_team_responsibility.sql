-- OPS-LAUNCH-001 D01: stable team grouping and current account responsibility.
--
-- Historical workbench names remain source evidence on the imported rows.  These tables and
-- columns are Linggan-owned operational facts, so no native action needs to invent an import
-- batch or overwrite historical attribution.

CREATE TABLE linggan_ops_group (
    group_ref uuid PRIMARY KEY,
    display_name text NOT NULL UNIQUE
        CHECK (length(btrim(display_name)) BETWEEN 1 AND 160),
    state text NOT NULL CHECK (state IN ('active', 'disabled')),
    created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    disabled_at timestamptz,
    version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
    CONSTRAINT linggan_ops_group_lifecycle_check CHECK (
        (state = 'active' AND disabled_at IS NULL)
        OR (state = 'disabled' AND disabled_at IS NOT NULL)
    )
);

-- A disabled group is historical authorization context and must remain inspectable even if it
-- never received a membership event.  Active empty groups may be removed before use.
CREATE TRIGGER linggan_ops_disabled_group_is_retained
    BEFORE DELETE ON linggan_ops_group
    FOR EACH ROW WHEN (OLD.state = 'disabled')
    EXECUTE FUNCTION linggan_ops_forbid_mutation();

-- Membership is an append-only event log.  The current group is determined from a member's
-- latest event; therefore a later reorganisation never rewrites the group that authorized a
-- historical account handover.
CREATE TABLE linggan_ops_group_membership_event (
    event_ref uuid PRIMARY KEY,
    group_ref uuid NOT NULL REFERENCES linggan_ops_group(group_ref),
    member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    action text NOT NULL CHECK (action IN ('joined', 'left')),
    acted_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    reason text NOT NULL CHECK (length(btrim(reason)) BETWEEN 1 AND 1000),
    request_ref uuid NOT NULL UNIQUE,
    occurred_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX linggan_ops_group_membership_member_current_idx
    ON linggan_ops_group_membership_event (member_ref, occurred_at DESC, event_ref DESC);
CREATE INDEX linggan_ops_group_membership_group_current_idx
    ON linggan_ops_group_membership_event (group_ref, occurred_at DESC, event_ref DESC);

CREATE TRIGGER linggan_ops_group_membership_event_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_group_membership_event
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

-- This is intentionally separate from historical `responsible_user_source_id`: the latter is
-- evidence from a workbench export, while this is the presently accountable Linggan member.
ALTER TABLE linggan_ops_account
    ADD COLUMN current_responsible_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN responsibility_version bigint NOT NULL DEFAULT 1 CHECK (responsibility_version > 0);

-- Every responsibility decision, including first assignment and later transfer, receives its own
-- durable receipt.  The service is responsible for atomically comparing responsibility_version,
-- validating the actor's role/current group, inserting this row and advancing the account row.
CREATE TABLE linggan_ops_account_responsibility_change (
    change_ref uuid PRIMARY KEY,
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    previous_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    next_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    changed_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    reason text NOT NULL CHECK (length(btrim(reason)) BETWEEN 1 AND 1000),
    request_ref uuid NOT NULL UNIQUE,
    resulting_version bigint NOT NULL CHECK (resulting_version > 0),
    occurred_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_account_responsibility_changed_check CHECK (
        previous_member_ref IS DISTINCT FROM next_member_ref
    )
);

CREATE INDEX linggan_ops_account_responsibility_account_idx
    ON linggan_ops_account_responsibility_change (account_ref, occurred_at DESC, change_ref DESC);

CREATE TRIGGER linggan_ops_account_responsibility_change_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_account_responsibility_change
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_group IS
    'Linggan-owned active or disabled operating groups; names are not historical source identities.';
COMMENT ON TABLE linggan_ops_group_membership_event IS
    'Append-only join/leave events used to reconstruct current group membership and its history.';
COMMENT ON COLUMN linggan_ops_account.current_responsible_member_ref IS
    'Current Linggan team responsibility, distinct from imported responsible_user_source_id.';
COMMENT ON TABLE linggan_ops_account_responsibility_change IS
    'Append-only receipt for an account responsibility assignment, transfer or unassignment.';
