-- 调度的准入失败原因码不再压成一个 `target_not_requestable`。
--
-- 此前调度对准入的**直接报错**只有一句 `Err(_) => "target_not_requestable"`。一个字符串吞掉
-- 了全部情况：目标还没归属领域、授权没签、授权额度不够 200 篇、schema 没装、目标不存在。
-- 界面上只写「目标不可请求」，而真实原因是目标没有领域——2026-09 排查时为此多花了两轮。
--
-- **一个压平的原因码比没有原因码更坏：它看起来是个答案。**
--
-- 这一支把缺的词补进闭集。代码侧改成对错误逐种匹配、不留 `_` 兜底分支：再有新的失败形态
-- 时编译器会拦住，而不是让它悄悄落进一个通用桶里。
--
-- 其中三个（`authorization_bound_too_small`、`progressive_purpose_mismatch`、
-- `progressive_archive_not_ready`）目前走不到巡检这条路——它们来自创作者的渐进建档。
-- 仍然收进词表并逐种匹配：判据要覆盖整个错误集合，覆盖不到的那一块将来就是兜底。

-- **基线取自 `0065`，不是 `0037`。** `0065_account_observation_bootstrap` 在所有清单里都被
-- 刻意排在最后应用（它是账号观察的引导包），而它也重设了这条 CHECK。照 `0037` 的清单写、
-- 再按编号排在 0065 前面，结果是这一支的词表当场被 0065 盖掉——迁移全部「应用成功」，
-- 而新原因码在写入时被拒绝。所以本支登记在 0065 **之后**，词表以 0065 的为底。
ALTER TABLE collection_scheduler_target_decision
    DROP CONSTRAINT collection_scheduler_target_decision_reason_code_check;
ALTER TABLE collection_scheduler_target_decision
    ADD CONSTRAINT collection_scheduler_target_decision_reason_code_check
    CHECK (reason_code IN (
        'queued','dispatched','rule_missing','rule_revision_changed','manual_only','monitoring_paused','not_due',
        'dynamic_unavailable','baseline_not_ready','target_not_requestable',
        'in_flight_work_covers_it',
        'risk_paused','station_unavailable','station_not_accepting',
        'installation_credential_missing','plugin_version_unsupported','installation_stale',
        'capability_missing','account_unbound','account_binding_changed','account_binding_expired',
        'account_eligibility_stale','account_cooling','account_needs_login','account_restricted',
        'account_unknown','account_busy','station_busy','station_daily_budget_reached','platform_concurrency_reached',
        'capacity_unknown',
        'authorization_missing','authorization_purpose_mismatch','authorization_target_limit_reached',
        'authorization_scope_mismatch','authorization_work_unit_limit_reached',
        'authorization_expired_or_revoked','admission_refused','lease_issue_failed','database_error',
        -- 本支新增：准入在形成决策之前就报错的那几种。
        'acquisition_schema_unavailable','unknown_target','target_domain_unassigned',
        'invalid_material_targets','authorization_bound_too_small',
        'progressive_purpose_mismatch','progressive_archive_not_ready','acquisition_read_failed'
    ));
