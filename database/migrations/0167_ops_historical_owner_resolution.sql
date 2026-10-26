-- OPS-MIGRATION-001: keep a reviewed link between imported historical attribution and
-- Intelligence members without rewriting the two source fields on publication/daily rows.
-- A source name/ID disagreement remains explicitly pending instead of guessing an owner.
CREATE TABLE linggan_ops_historical_owner_resolution (
    resolution_ref uuid PRIMARY KEY,
    request_ref uuid NOT NULL UNIQUE,
    publication_ref uuid REFERENCES linggan_ops_publication(publication_ref),
    daily_ref uuid REFERENCES linggan_ops_daily_report(daily_ref),
    source_display_name text,
    responsible_user_source_id text,
    source_member_display_name text,
    resolved_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    state text NOT NULL CHECK (state IN ('matched', 'conflict_pending', 'unresolved')),
    import_batch_ref uuid NOT NULL REFERENCES linggan_ops_import_batch(batch_ref),
    rule_version text NOT NULL CHECK (rule_version IN (
        'ops-historical-owner.v1', 'ops-historical-owner-manual.v1'
    )),
    decided_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    decided_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_historical_owner_one_record CHECK (
        (publication_ref IS NOT NULL) <> (daily_ref IS NOT NULL)
    ),
    CONSTRAINT linggan_ops_historical_owner_state_shape CHECK (
        (state = 'matched' AND resolved_member_ref IS NOT NULL)
        OR (state IN ('conflict_pending', 'unresolved') AND resolved_member_ref IS NULL)
    ),
    CONSTRAINT linggan_ops_historical_owner_automatic_evidence CHECK (
        rule_version <> 'ops-historical-owner.v1'
        OR (
            responsible_user_source_id IS NOT NULL
            AND source_member_display_name IS NOT NULL
            AND (
                (state = 'matched' AND source_display_name IS NOT NULL
                    AND source_display_name = source_member_display_name)
                OR (state = 'conflict_pending' AND source_display_name IS NOT NULL
                    AND source_display_name <> source_member_display_name)
                OR (state = 'unresolved' AND source_display_name IS NULL)
            )
        )
    )
);

CREATE INDEX linggan_ops_historical_owner_publication_idx
    ON linggan_ops_historical_owner_resolution (publication_ref, decided_at DESC, resolution_ref DESC)
    WHERE publication_ref IS NOT NULL;
CREATE INDEX linggan_ops_historical_owner_daily_idx
    ON linggan_ops_historical_owner_resolution (daily_ref, decided_at DESC, resolution_ref DESC)
    WHERE daily_ref IS NOT NULL;
CREATE INDEX linggan_ops_historical_owner_member_idx
    ON linggan_ops_historical_owner_resolution (resolved_member_ref)
    WHERE resolved_member_ref IS NOT NULL;
CREATE INDEX linggan_ops_historical_owner_pending_idx
    ON linggan_ops_historical_owner_resolution (state)
    WHERE state <> 'matched';

-- A later correction is a separate reviewed decision; no historical evidence can be
-- silently changed by an upsert or by rerunning the one-time migration. Readers take
-- the latest event per publication_ref or daily_ref.
CREATE TRIGGER linggan_ops_historical_owner_resolution_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_historical_owner_resolution
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_historical_owner_resolution IS
    'One-time historical member matching for workbench publication/daily records. The source name and ID remain on their original rows; disagreements have no resolved member.';
