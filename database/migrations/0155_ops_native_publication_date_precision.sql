-- OPS-LAUNCH-001 D04: an internally registered publication can be known only to a
-- Beijing business day.  Do not manufacture an instant just to satisfy the old import shape.
-- Imported rows retain their timestamp and all native precision fields remain NULL.

ALTER TABLE linggan_ops_publication
    ALTER COLUMN published_at DROP NOT NULL,
    ADD COLUMN native_published_on date,
    ADD COLUMN native_published_precision text
        CHECK (native_published_precision IN ('day', 'instant'));

-- 0153 allowed native rows before precision was modelled.  Preserve their recorded instant and
-- establish the business-day fact before the stronger source boundary is validated.
UPDATE linggan_ops_publication
SET native_published_on = (published_at AT TIME ZONE 'Asia/Shanghai')::date,
    native_published_precision = 'instant'
WHERE source_system = 'linggan_native';

ALTER TABLE linggan_ops_publication
    DROP CONSTRAINT linggan_ops_publication_native_origin_check,
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
            AND native_published_on IS NULL
            AND native_published_precision IS NULL
            AND legacy_effective_publication IS NOT NULL
            AND legacy_data_sync_status IS NOT NULL
            AND published_at IS NOT NULL
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
            AND native_published_on IS NOT NULL
            AND native_published_precision IS NOT NULL
            AND (
                (native_published_precision = 'instant'
                 AND published_at IS NOT NULL
                 AND published_business_date = native_published_on)
                OR
                (native_published_precision = 'day' AND published_at IS NULL)
            )
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

-- Imported duplicate identities are historical evidence and cannot be silently rewritten by a
-- new constraint.  Native identity claims are nevertheless physically unique; the service also
-- checks historic rows before it accepts a claim.
CREATE UNIQUE INDEX linggan_ops_native_account_platform_identity_key
    ON linggan_ops_account (platform, platform_user_id)
    WHERE source_system = 'linggan_native' AND platform_user_id IS NOT NULL;

COMMENT ON COLUMN linggan_ops_publication.native_published_on IS
    'Native Beijing business date, including when no exact publication instant is known.';
COMMENT ON COLUMN linggan_ops_publication.native_published_precision IS
    'Native publication-time precision: day preserves a date-only report; instant carries published_at.';
