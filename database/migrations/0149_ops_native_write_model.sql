-- OPS-LAUNCH-001 D01-D11: one logical Operations ledger can now contain an
-- imported historical row or a Linggan-native row.  Historical import evidence
-- retains its exact source/batch semantics; native rows never invent either.

ALTER TABLE linggan_ops_account
    DROP CONSTRAINT linggan_ops_account_source_system_check,
    DROP CONSTRAINT linggan_ops_account_ledger_state_check,
    DROP CONSTRAINT linggan_ops_account_ledger_fields_check,
    ALTER COLUMN source_created_at DROP NOT NULL,
    ALTER COLUMN source_updated_at DROP NOT NULL,
    ALTER COLUMN first_batch_ref DROP NOT NULL,
    ALTER COLUMN last_changed_batch_ref DROP NOT NULL,
    ADD COLUMN native_record_state text
        CHECK (native_record_state IN ('active', 'archived')),
    ADD COLUMN native_version bigint CHECK (native_version > 0),
    ADD COLUMN native_created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_updated_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_archived_at timestamptz,
    ADD COLUMN native_archive_reason text,
    ADD CONSTRAINT linggan_ops_account_source_system_check
        CHECK (source_system IN ('content_workbench', 'linggan_native')),
    ADD CONSTRAINT linggan_ops_account_ledger_state_check
        CHECK (ledger_state IN (
            'in_ledger', 'absent_from_export', 'native_active', 'native_archived'
        )),
    ADD CONSTRAINT linggan_ops_account_ledger_fields_check CHECK (
        (
            source_system = 'content_workbench'
            AND ledger_state IN ('in_ledger', 'absent_from_export')
            AND native_record_state IS NULL
            AND native_version IS NULL
            AND native_created_by_member_ref IS NULL
            AND native_updated_by_member_ref IS NULL
            AND native_archived_at IS NULL
            AND native_archive_reason IS NULL
            AND (
                ledger_state = 'absent_from_export'
                OR (
                    status IS NOT NULL AND is_retained IS NOT NULL AND review_status IS NOT NULL
                    AND source_created_at IS NOT NULL AND source_updated_at IS NOT NULL
                    AND first_batch_ref IS NOT NULL AND last_changed_batch_ref IS NOT NULL
                )
            )
        )
        OR (
            source_system = 'linggan_native'
            AND source_record_id = account_ref::text
            AND ledger_state = CASE native_record_state
                WHEN 'active' THEN 'native_active'
                WHEN 'archived' THEN 'native_archived'
            END
            AND native_version IS NOT NULL
            AND native_created_by_member_ref IS NOT NULL
            AND native_updated_by_member_ref IS NOT NULL
            AND source_created_at IS NULL AND source_updated_at IS NULL
            AND first_batch_ref IS NULL AND last_changed_batch_ref IS NULL
            AND responsible_user_source_id IS NULL AND created_by_user_source_id IS NULL
            AND (
                (native_record_state = 'active'
                 AND native_archived_at IS NULL AND native_archive_reason IS NULL)
                OR
                (native_record_state = 'archived'
                 AND native_archived_at IS NOT NULL
                 AND length(btrim(native_archive_reason)) BETWEEN 1 AND 1000)
            )
        )
    );

ALTER TABLE linggan_ops_publication
    DROP CONSTRAINT linggan_ops_publication_source_system_check,
    ALTER COLUMN legacy_effective_publication DROP NOT NULL,
    ALTER COLUMN legacy_data_sync_status DROP NOT NULL,
    ALTER COLUMN source_created_at DROP NOT NULL,
    ALTER COLUMN source_updated_at DROP NOT NULL,
    ALTER COLUMN first_batch_ref DROP NOT NULL,
    ALTER COLUMN last_changed_batch_ref DROP NOT NULL,
    ADD COLUMN native_record_state text
        CHECK (native_record_state IN ('active', 'revoked')),
    ADD COLUMN native_version bigint CHECK (native_version > 0),
    ADD COLUMN native_publisher_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_registered_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_business_domain_ref uuid REFERENCES observation_domain(domain_ref),
    ADD COLUMN native_revoked_at timestamptz,
    ADD COLUMN native_revoked_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_revocation_reason text,
    ADD CONSTRAINT linggan_ops_publication_source_system_check
        CHECK (source_system IN ('content_workbench', 'linggan_native')),
    ADD CONSTRAINT linggan_ops_publication_native_origin_check CHECK (
        (
            source_system = 'content_workbench'
            AND native_record_state IS NULL
            AND native_version IS NULL
            AND native_publisher_member_ref IS NULL
            AND native_registered_by_member_ref IS NULL
            AND native_business_domain_ref IS NULL
            AND native_revoked_at IS NULL
            AND native_revoked_by_member_ref IS NULL
            AND native_revocation_reason IS NULL
            AND legacy_effective_publication IS NOT NULL
            AND legacy_data_sync_status IS NOT NULL
            AND source_created_at IS NOT NULL AND source_updated_at IS NOT NULL
            AND first_batch_ref IS NOT NULL AND last_changed_batch_ref IS NOT NULL
        )
        OR (
            source_system = 'linggan_native'
            AND source_record_id = publication_ref::text
            AND native_record_state IS NOT NULL
            AND native_version IS NOT NULL
            AND native_publisher_member_ref IS NOT NULL
            AND native_registered_by_member_ref IS NOT NULL
            AND native_business_domain_ref IS NOT NULL
            AND legacy_effective_publication IS NULL
            AND legacy_data_sync_status IS NULL
            AND legacy_data_synced_at IS NULL
            AND source_created_at IS NULL AND source_updated_at IS NULL
            AND first_batch_ref IS NULL AND last_changed_batch_ref IS NULL
            AND publisher_name IS NULL
            AND responsible_user_source_id IS NULL AND created_by_user_source_id IS NULL
            AND missing_since_batch_ref IS NULL AND merged_into_ref IS NULL
            AND (
                (native_record_state = 'active'
                 AND native_revoked_at IS NULL AND native_revoked_by_member_ref IS NULL
                 AND native_revocation_reason IS NULL)
                OR
                (native_record_state = 'revoked'
                 AND native_revoked_at IS NOT NULL AND native_revoked_by_member_ref IS NOT NULL
                 AND length(btrim(native_revocation_reason)) BETWEEN 1 AND 1000)
            )
        )
    );

ALTER TABLE linggan_ops_publication
    DROP CONSTRAINT linggan_ops_publication_entry_source_check,
    ADD CONSTRAINT linggan_ops_publication_entry_source_check
        CHECK (entry_source IN (
            'manual', 'csv_import', 'topic_publication_sync', 'linggan_ops_manual'
        ));

CREATE FUNCTION linggan_ops_native_publication_account_platform_matches() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN
        IF NEW.source_system = 'linggan_native' AND NEW.account_ref IS NOT NULL
           AND NOT EXISTS (
                SELECT 1 FROM linggan_ops_account
                WHERE account_ref = NEW.account_ref AND platform = NEW.platform
           ) THEN
            RAISE EXCEPTION 'native Operations publication platform must match its account';
        END IF;
        RETURN NEW;
    END $$;

CREATE TRIGGER linggan_ops_native_publication_account_platform_matches
    BEFORE INSERT OR UPDATE OF source_system, account_ref, platform ON linggan_ops_publication
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_native_publication_account_platform_matches();

ALTER TABLE linggan_ops_view_report
    DROP CONSTRAINT linggan_ops_view_report_source_kind_check,
    ALTER COLUMN batch_ref DROP NOT NULL,
    ADD COLUMN reported_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN request_ref uuid UNIQUE,
    ADD CONSTRAINT linggan_ops_view_report_source_kind_check
        CHECK (source_kind IN ('workbench_import', 'linggan_ops_manual')),
    ADD CONSTRAINT linggan_ops_view_report_origin_check CHECK (
        (source_kind = 'workbench_import'
         AND batch_ref IS NOT NULL AND reported_by_member_ref IS NULL AND request_ref IS NULL)
        OR
        (source_kind = 'linggan_ops_manual'
         AND batch_ref IS NULL AND reported_by_member_ref IS NOT NULL AND request_ref IS NOT NULL)
    );

ALTER TABLE linggan_ops_daily_report
    DROP CONSTRAINT linggan_ops_daily_report_source_system_check,
    ALTER COLUMN links_opened_count DROP NOT NULL,
    ALTER COLUMN followers_gained DROP NOT NULL,
    ALTER COLUMN source_created_at DROP NOT NULL,
    ALTER COLUMN source_updated_at DROP NOT NULL,
    ALTER COLUMN first_batch_ref DROP NOT NULL,
    ALTER COLUMN last_changed_batch_ref DROP NOT NULL,
    ADD COLUMN native_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_version bigint CHECK (native_version > 0),
    ADD COLUMN native_created_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD COLUMN native_updated_by_member_ref uuid REFERENCES linggan_ops_member(member_ref),
    ADD CONSTRAINT linggan_ops_daily_report_source_system_check
        CHECK (source_system IN ('content_workbench', 'linggan_native')),
    ADD CONSTRAINT linggan_ops_daily_report_native_origin_check CHECK (
        (
            source_system = 'content_workbench'
            AND native_member_ref IS NULL AND native_version IS NULL
            AND native_created_by_member_ref IS NULL AND native_updated_by_member_ref IS NULL
            AND links_opened_count IS NOT NULL AND followers_gained IS NOT NULL
            AND source_created_at IS NOT NULL AND source_updated_at IS NOT NULL
            AND first_batch_ref IS NOT NULL AND last_changed_batch_ref IS NOT NULL
        )
        OR (
            source_system = 'linggan_native'
            AND source_record_id = daily_ref::text
            AND native_member_ref IS NOT NULL AND native_version IS NOT NULL
            AND native_created_by_member_ref IS NOT NULL AND native_updated_by_member_ref IS NOT NULL
            AND source_created_at IS NULL AND source_updated_at IS NULL
            AND first_batch_ref IS NULL AND last_changed_batch_ref IS NULL
            AND responsible_user_source_id IS NULL AND created_by_user_source_id IS NULL
            AND missing_since_batch_ref IS NULL
        )
    );

CREATE UNIQUE INDEX linggan_ops_native_daily_member_day_idx
    ON linggan_ops_daily_report (native_member_ref, business_date)
    WHERE source_system = 'linggan_native';

-- A request receipt carries the exact command payload fingerprint and its terminal outcome.
-- Services must return the stored result on an identical retry and reject a payload mismatch.
CREATE TABLE linggan_ops_native_write_request (
    request_ref uuid PRIMARY KEY,
    command_kind text NOT NULL CHECK (command_kind IN (
        'account_create', 'account_update', 'account_archive', 'account_restore',
        'publication_create', 'publication_update', 'publication_revoke', 'publication_restore',
        'view_report_record', 'daily_report_record'
    )),
    actor_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    payload_sha256 bytea NOT NULL CHECK (octet_length(payload_sha256) = 32),
    outcome text NOT NULL CHECK (outcome IN ('accepted', 'unchanged', 'stale', 'rejected')),
    record_ref uuid,
    resulting_version bigint CHECK (resulting_version > 0),
    outcome_code text NOT NULL CHECK (length(btrim(outcome_code)) BETWEEN 1 AND 120),
    created_at timestamptz NOT NULL DEFAULT scope_001_now()
);

CREATE INDEX linggan_ops_native_write_request_actor_idx
    ON linggan_ops_native_write_request (actor_member_ref, created_at DESC);

CREATE TRIGGER linggan_ops_native_write_request_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_native_write_request
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

-- Native daily data is a current record plus append-only accepted revisions.  Nullable values
-- mean not filled; zero is a real value.  The base row never conceals who actually saved it.
CREATE TABLE linggan_ops_daily_report_revision (
    revision_ref uuid PRIMARY KEY,
    daily_ref uuid NOT NULL REFERENCES linggan_ops_daily_report(daily_ref),
    previous_version bigint CHECK (previous_version IS NULL OR previous_version > 0),
    resulting_version bigint NOT NULL CHECK (resulting_version > 0),
    links_opened_count integer CHECK (links_opened_count IS NULL OR links_opened_count >= 0),
    followers_gained integer CHECK (followers_gained IS NULL OR followers_gained >= 0),
    changed_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    request_ref uuid NOT NULL UNIQUE REFERENCES linggan_ops_native_write_request(request_ref),
    recorded_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CONSTRAINT linggan_ops_daily_report_revision_version_check CHECK (
        previous_version IS NULL OR resulting_version = previous_version + 1
    )
);

CREATE INDEX linggan_ops_daily_report_revision_daily_idx
    ON linggan_ops_daily_report_revision (daily_ref, resulting_version DESC);

CREATE TRIGGER linggan_ops_daily_report_revision_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_daily_report_revision
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_native_write_request IS
    'Append-only command receipt for native Operations writes; payload hash supports retry-versus-conflict decisions.';
COMMENT ON COLUMN linggan_ops_daily_report.links_opened_count IS
    'Imported rows retain mandatory historical values; native rows use NULL for unfilled and 0 for a reported zero.';
COMMENT ON TABLE linggan_ops_daily_report_revision IS
    'Accepted native daily report revisions; current values remain on linggan_ops_daily_report for one formal read path.';
