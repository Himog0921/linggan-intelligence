-- OPS-LAUNCH-001 H01-H07: responsibility transfers use the same idempotent command
-- receipt ledger as the native account/publication/daily commands.  The actual before/after
-- relationship remains in linggan_ops_account_responsibility_change (0147).

ALTER TABLE linggan_ops_native_write_request
    DROP CONSTRAINT linggan_ops_native_write_request_command_kind_check,
    ADD CONSTRAINT linggan_ops_native_write_request_command_kind_check CHECK (command_kind IN (
        'account_create', 'account_update', 'account_archive', 'account_restore',
        'account_responsibility_transfer',
        'publication_create', 'publication_update', 'publication_revoke', 'publication_restore',
        'publication_locator_record', 'view_report_record', 'daily_report_record'
    ));

COMMENT ON TABLE linggan_ops_account_responsibility_change IS
    'Append-only current-responsibility assignment, transfer, or unassignment receipt. The matching native command receipt supplies idempotent HTTP replay; neither changes imported historical attribution.';
