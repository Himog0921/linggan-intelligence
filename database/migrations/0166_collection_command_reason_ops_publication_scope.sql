-- OPS-LAUNCH-001：Ops 精确发布范围不匹配也是人工观察的确定性拒绝，
-- 不是数据库不可用。它的回执必须能持久化，否则拒绝事实会在写库时被 CHECK 吞掉。

ALTER TABLE collection_monitor_rule_command_identity
    DROP CONSTRAINT IF EXISTS collection_command_identity_reason_codes_ck;
ALTER TABLE collection_monitor_rule_command_identity
    ADD CONSTRAINT collection_command_identity_reason_codes_ck
    CHECK (first_reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision','invalid_mode',
        'invalid_interval','invalid_schedule','baseline_not_ready','target_not_requestable',
        'target_domain_unassigned','unknown_target','invalid_material_targets','invalid_ops_publication_scope','database_unavailable',
        'schema_unavailable','account_binding_changed','account_binding_expired','account_busy',
        'account_cooling','account_eligibility_stale','account_needs_login','account_restricted',
        'account_unbound','account_unknown','authorization_expired_or_revoked','authorization_missing',
        'authorization_purpose_mismatch','authorization_scope_mismatch','authorization_target_limit_reached','authorization_work_unit_limit_reached',
        'available','capability_missing','capacity_unknown','in_flight_work_covers_it',
        'installation_credential_missing','installation_risk_cooldown','installation_stale','need_already_satisfied',
        'platform_concurrency_reached','plugin_version_unsupported','progressive_archive_authorization_missing','progressive_archive_authorization_too_small',
        'progressive_archive_not_ready','progressive_archive_purpose_mismatch','question_unanswerable','queueable',
        'risk_paused','station_busy','station_daily_budget_reached','station_not_accepting',
        'station_unavailable','within_authorization','reason_not_recognized'
    ));

COMMENT ON CONSTRAINT collection_command_identity_reason_codes_ck ON collection_monitor_rule_command_identity IS
  'OPS-LAUNCH-001/0166：与 MONITOR_COMMAND_REASONS 逐码一致，仅不含 identity_conflict（首条回执不可能是身份冲突）。';

ALTER TABLE collection_monitor_rule_command_receipt
    DROP CONSTRAINT IF EXISTS collection_command_receipt_reason_codes_ck;
ALTER TABLE collection_monitor_rule_command_receipt
    ADD CONSTRAINT collection_command_receipt_reason_codes_ck
    CHECK (reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision','identity_conflict',
        'invalid_mode','invalid_interval','invalid_schedule','baseline_not_ready',
        'target_not_requestable','target_domain_unassigned','unknown_target','invalid_material_targets','invalid_ops_publication_scope',
        'database_unavailable','schema_unavailable','account_binding_changed','account_binding_expired',
        'account_busy','account_cooling','account_eligibility_stale','account_needs_login',
        'account_restricted','account_unbound','account_unknown','authorization_expired_or_revoked',
        'authorization_missing','authorization_purpose_mismatch','authorization_scope_mismatch','authorization_target_limit_reached',
        'authorization_work_unit_limit_reached','available','capability_missing','capacity_unknown',
        'in_flight_work_covers_it','installation_credential_missing','installation_risk_cooldown','installation_stale',
        'need_already_satisfied','platform_concurrency_reached','plugin_version_unsupported','progressive_archive_authorization_missing',
        'progressive_archive_authorization_too_small','progressive_archive_not_ready','progressive_archive_purpose_mismatch','question_unanswerable',
        'queueable','risk_paused','station_busy','station_daily_budget_reached',
        'station_not_accepting','station_unavailable','within_authorization','reason_not_recognized'
    ));

COMMENT ON CONSTRAINT collection_command_receipt_reason_codes_ck ON collection_monitor_rule_command_receipt IS
  'OPS-LAUNCH-001/0166：与 MONITOR_COMMAND_REASONS 逐码一致（含 identity_conflict 与 reason_not_recognized）。';
