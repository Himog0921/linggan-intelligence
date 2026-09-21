-- COLLECTION-UPGRADE-001 · S5：让两个回执表接受本域**完整**的原因码
--
-- 这条迁移修的是一个**已经在线上的**缺陷，不是新增能力。它由 S5 候选 ⑥ 的实现暴露出来：
-- 那条改动让「缺 schema」「目标不存在」「目标没指定领域」这类失败终于说得出自己的名字，
-- 结果写回执时被数据库的 CHECK 挡下（`23514`）——**拒绝不再留下任何回执**。
--
-- ## 缺陷的形状：同一份词表，第三份手抄副本
--
-- 「一条人工命令为什么没有按人期望的样子改变世界」这份词表在本仓库里有三个写处：
--   1. `crates/evidence/src/collection_control.rs` 的 `MONITOR_COMMAND_REASONS`（代码侧定义）；
--   2. 本表的 `collection_command_identity_reason_codes_ck`；
--   3. 回执表的 `collection_command_receipt_reason_codes_ck`。
--
-- 后两份是 `0065` 建表时对第 1 份的**手抄**。手抄的东西会和原件分头长大：`0065` 之后，
-- 代码侧陆续加了好些个码，两份 CHECK 一个都没跟上。差集里已经有一个**能在线触发**的码——
-- `target_domain_unassigned`：`manual_observe_error_reason` 专门为它写了显式分支（注释就在
-- 那个分支上面，写明了「缺领域必须单独报」，因为以前它被说成「数据库不可用」），可它对两份
-- CHECK 都是非法的。也就是说：对一个还没指定领域的目标点「立即观察」，命令不会留下一张
-- 写着「这个目标没有领域」的回执，而是在写回执这一步整笔失败。
--
-- 这正是 `0065` 自己那条注释要防的事（「receipts must persist that concrete wait reason
-- instead of collapsing it into account_unknown」）——同一个错误，换了一层。
--
-- ## 为什么是新开一条迁移，不是改 `0065`
--
-- migrations 是 append-only 的：`0065` 已经在共享库和所有证明库上应用过了，改它对已应用的
-- 环境不生效，只会让「仓库里的 0065」与「库里的 0065」变成两个东西。所以按纪律新开一条，
-- 只做一件事：把两份 CHECK 补成代码侧的完整词表。
--
-- ## 这条迁移只放宽、不收紧
--
-- 两份新 CHECK 都是旧 CHECK 的**超集**，因此不可能与任何既有行冲突——放宽 CHECK 不会让
-- 一条已经写下的回执变成非法。反向不成立：这份词表里的码以后若再有人在代码侧新增，必须
-- 同样再开一条迁移，否则又会回到今天这个形状。
--
-- ## 防复发的那一道闸不在 SQL 里
--
-- `crates/evidence/src/collection_control.rs` 的单元用例
-- `both_receipt_check_constraints_spell_out_the_whole_closed_vocabulary` 扫全部迁移，
-- 取这两条约束的**最终**定义，与本域词表逐码比对。它跑在 `--lib` 里，不需要证明库，
-- 也不用等一次真实的拒绝写不进去才发现：代码侧加一个码而没有配套迁移，那条用例直接变红
-- （两侧各变异验证过一次）。它挡不住的是「某条迁移把约束单独 DROP 掉而不重建」——那种
-- 改动会让这道闸和数据库那道闸一起消失，只能靠复核看 diff；限制写在用例自己的注释里。

-- 一、身份表：首条回执的原因码
--
-- 与回执表唯一的一处**有意**差异：身份表不收 `identity_conflict`。身份行记的是一条命令的
-- **首条**结果，而「身份冲突」按定义不可能发生在第一条——写入点用 `unreachable!` 表达了同一
-- 件事，这里保留 `0065` 的原意，不因为「顺手统一成一样」而把它删掉。
DO $$
DECLARE constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid='collection_monitor_rule_command_identity'::regclass
      AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%first_reason_code%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE collection_monitor_rule_command_identity DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;
ALTER TABLE collection_monitor_rule_command_identity
    ADD CONSTRAINT collection_command_identity_reason_codes_ck
    CHECK (first_reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision','invalid_mode',
        'invalid_interval','invalid_schedule','baseline_not_ready','target_not_requestable',
        'target_domain_unassigned','unknown_target','invalid_material_targets','database_unavailable',
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
  'COLLECTION-UPGRADE-001 S5/0101：与 MONITOR_COMMAND_REASONS 逐码一致，仅不含 identity_conflict（首条回执不可能是身份冲突）。';

-- 二、回执表：这一条回执的原因码
--
-- 比身份表多 `identity_conflict`。`reason_not_recognized` 也在这张表上（两份都有）：
-- 它的用途是**回滚**——新二进制按新的词表写下某个码，回滚到旧二进制后旧版本不认识它，
-- 读出时收敛成这个中性词。既然要把它写回去，CHECK 就必须收它；否则「说不出原因就说不出
-- 原因」这条兜底会在写库处变成一个 23514，比它要替代的那个错误说法更坏。
DO $$
DECLARE constraint_name text;
BEGIN
    SELECT conname INTO constraint_name
    FROM pg_constraint
    WHERE conrelid='collection_monitor_rule_command_receipt'::regclass
      AND contype='c'
      AND pg_get_constraintdef(oid) LIKE '%reason_code%';
    IF constraint_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE collection_monitor_rule_command_receipt DROP CONSTRAINT %I', constraint_name);
    END IF;
END $$;
ALTER TABLE collection_monitor_rule_command_receipt
    ADD CONSTRAINT collection_command_receipt_reason_codes_ck
    CHECK (reason_code IN (
        'rule_saved','monitor_paused','monitor_resumed','monitor_stopped',
        'manual_observe_created','manual_observe_reused','stale_revision','identity_conflict',
        'invalid_mode','invalid_interval','invalid_schedule','baseline_not_ready',
        'target_not_requestable','target_domain_unassigned','unknown_target','invalid_material_targets',
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
  'COLLECTION-UPGRADE-001 S5/0101：与 MONITOR_COMMAND_REASONS 逐码一致（含 identity_conflict 与 reason_not_recognized）。';

-- 三、采集调度的步骤结果不在这条迁移的范围里
--
-- `0065` 同批还改了 `collection_scheduler_target_decision` 的原因码约束。那一套是**调度**
-- 的词表（`step_report` 的受限码），与人工命令的回执不是同一份——本域词表的注释里写明了
-- 三套各自演进、不合并。本迁移只补人工命令这两张表。
