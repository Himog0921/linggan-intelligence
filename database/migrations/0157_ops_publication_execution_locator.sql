-- OPS-LAUNCH-001 S01-S20: a signed execution locator is an operator-provided, immutable
-- input fact. It is not a discovery finding, CapturePackage, material row, or execution.

ALTER TABLE linggan_ops_native_write_request
    DROP CONSTRAINT linggan_ops_native_write_request_command_kind_check,
    ADD CONSTRAINT linggan_ops_native_write_request_command_kind_check CHECK (command_kind IN (
        'account_create', 'account_update', 'account_archive', 'account_restore',
        'publication_create', 'publication_update', 'publication_revoke', 'publication_restore',
        'publication_locator_record', 'view_report_record', 'daily_report_record'
    ));

CREATE TABLE linggan_ops_publication_execution_locator (
    locator_ref uuid PRIMARY KEY,
    publication_ref uuid NOT NULL REFERENCES linggan_ops_publication(publication_ref),
    account_ref uuid NOT NULL REFERENCES linggan_ops_account(account_ref),
    platform text NOT NULL CHECK (platform = 'xhs'),
    expected_content_id text NOT NULL CHECK (expected_content_id ~ '^[0-9a-f]{24}$'),
    expected_author_id text NOT NULL CHECK (length(btrim(expected_author_id)) > 0),
    publication_version bigint NOT NULL CHECK (publication_version > 0),
    locator_url text NOT NULL CHECK (length(btrim(locator_url)) BETWEEN 1 AND 8192),
    locator_sha256 bytea NOT NULL CHECK (octet_length(locator_sha256) = 32),
    provided_by_member_ref uuid NOT NULL REFERENCES linggan_ops_member(member_ref),
    validation_state text NOT NULL CHECK (validation_state = 'accepted_xhs_signed_locator'),
    provided_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (publication_ref, locator_sha256)
);

CREATE INDEX linggan_ops_publication_execution_locator_current_idx
    ON linggan_ops_publication_execution_locator (publication_ref, publication_version, provided_at DESC, locator_ref DESC);

CREATE TRIGGER linggan_ops_publication_execution_locator_is_append_only
    BEFORE UPDATE OR DELETE ON linggan_ops_publication_execution_locator
    FOR EACH ROW EXECUTE FUNCTION linggan_ops_forbid_mutation();

COMMENT ON TABLE linggan_ops_publication_execution_locator IS
    'Immutable operator-provided XHS signed execution input. It binds a publication version to parsed content and expected author identities; it is not evidence or a collection queue.';
