-- OPS-MIGRATION-001 D3 / Issue #409: weekly and monthly 加微 targets set in Linggan. The workbench
-- exports do not carry targets, so they start here. Every save appends a revision, and every save
-- request leaves a receipt, so a retried or stale save never overwrites someone else's change.
-- Contract: docs/data-contracts/ops-http-001.md §4; page: docs/design/pages/ops-overview-page.md §5.

CREATE TABLE linggan_ops_target_revision (
    revision_ref uuid PRIMARY KEY,
    period_type text NOT NULL CHECK (period_type IN ('week', 'month')),
    -- ISO week `2026-W42` or month `2026-10`, both on the Beijing calendar.
    period_key text NOT NULL,
    metric text NOT NULL CHECK (metric = 'followers_gained'),
    target_value integer NOT NULL CHECK (target_value BETWEEN 1 AND 1000000),
    -- The revision this one replaces; NULL only for the first revision of a period.
    previous_revision_ref uuid,
    request_ref uuid NOT NULL UNIQUE,
    -- Until login (D4) a save can only say it came from the local machine.
    set_by text NOT NULL CHECK (set_by = 'local_machine'),
    set_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_target_revision_period_key_check CHECK (
        (period_type = 'week' AND period_key ~ '^[0-9]{4}-W(0[1-9]|[1-4][0-9]|5[0-3])$')
        OR (period_type = 'month' AND period_key ~ '^[0-9]{4}-(0[1-9]|1[0-2])$')
    ),
    CONSTRAINT linggan_ops_target_revision_period_ref_key
        UNIQUE (revision_ref, period_type, period_key, metric),
    -- A revision can only replace one of the same period.
    CONSTRAINT linggan_ops_target_revision_previous_fk
        FOREIGN KEY (previous_revision_ref, period_type, period_key, metric)
        REFERENCES linggan_ops_target_revision (revision_ref, period_type, period_key, metric),
    -- One linear history per period: two saves that saw the same current revision (or both saw
    -- none) cannot both land.
    CONSTRAINT linggan_ops_target_revision_linear_key
        UNIQUE NULLS NOT DISTINCT (period_type, period_key, metric, previous_revision_ref)
);

CREATE TABLE linggan_ops_target_request (
    request_ref uuid PRIMARY KEY,
    period_type text NOT NULL CHECK (period_type IN ('week', 'month')),
    period_key text NOT NULL,
    metric text NOT NULL CHECK (metric = 'followers_gained'),
    requested_value integer NOT NULL CHECK (requested_value BETWEEN 1 AND 1000000),
    -- What the page saw as the current revision; NULL when it saw no target.
    expected_revision_ref uuid,
    outcome text NOT NULL CHECK (outcome IN ('accepted', 'unchanged', 'stale')),
    -- The current revision once this request was handled (the new one when accepted).
    current_revision_ref uuid REFERENCES linggan_ops_target_revision (revision_ref),
    requested_by text NOT NULL CHECK (requested_by = 'local_machine'),
    received_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_target_request_current_check
        CHECK (outcome = 'stale' OR current_revision_ref IS NOT NULL)
);

CREATE INDEX linggan_ops_target_revision_period_idx
    ON linggan_ops_target_revision (period_type, period_key, metric);

-- Revisions and receipts are history: a correction is a new revision, never an edit.
CREATE TRIGGER linggan_ops_target_revision_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_target_revision
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();
CREATE TRIGGER linggan_ops_target_request_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_target_request
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_target_revision IS
    'Weekly and monthly 加微 target revisions set in Linggan; the current target is the tip of the chain.';
COMMENT ON TABLE linggan_ops_target_request IS
    'One receipt per target save request: accepted, unchanged or stale, with the revision current after it.';
