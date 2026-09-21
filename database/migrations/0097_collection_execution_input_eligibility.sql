-- COLLECTION-UPGRADE-001 · 执行资格台账：输入缺失是停止条件，不是可重试的失败
--
-- 现在有两件事共用同一段代码（`record_recoverable_dispatch_failure_in_transaction`）：
-- 「页面读了一次没读成」和「这一篇根本没有可用的执行地址」。它们被当成同一件事处理——
-- 释放租约、按 60/120/240/480/900 秒阶梯把**整张工单**放回队列。于是缺地址的成员每一轮
-- 都重新消耗一次租约，同批里地址完好的作品永远轮不到；而每重排一次就多一条失败事件，
-- 看起来像「一直在重试」，其实从来没开始过。共享库 2026-09-21 的 3 张工单、35 条
-- `execution_locator_unavailable` 就是这条路径连转三小时的产物。
--
-- 分开它们的判据不是「谁记的失败」，而是**输入还在不在**：
--
--   * 缺 locator / 必要输入  → `input_blocked`，不自动重试。新有效输入或被审计的输入修正
--                              才让它重新可执行。这一条不消耗页面失败预算——它根本没读页面。
--   * 页面读取临时失败       → 有界退避，且预算按**需求范围跨工单**累计，而不是每张新工单
--                              从零开始。
--
-- 台账管的是「还能不能执行」，它不复制 Package、Receipt 或材料事实。它上面有**两条不同的键**，
-- 混成一条就会得出两种相反的错：
--
--   * 身份键（含 locator 指纹）：`target_ref + domain_scope + object_kind/object_ref +
--     capability + input_fingerprint + retry_epoch`。它管「同一件事有没有被记两遍」——
--     「当初一条地址都没有」与「后来有了地址」是两条各自成立的事实，都留下。
--   * 当前键（不含指纹）：`target_ref + domain_scope + object_kind/object_ref + capability`，
--     只覆盖非停止行。它管「此刻谁说了算」——同一需求范围在同一时刻至多一条当前资格，
--     地址变了是更新那一条的输入，不是再派生一条并行的可执行资格。
--
-- 失败预算按**当前键**累计（不含 locator 指纹，行上的 `deduplicated_failure_count` 就是它）：
-- 真实的另一个地址能解除输入阻断，但刷新同一个地址的签名 token 不能清零页面失败次数。
-- `retry_epoch` 只能由明确的故障修复证明或受控重新准入开启，新建工单、地址变化、普通版本
-- 升级都不增加它。

CREATE TABLE collection_execution_input_eligibility (
    eligibility_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    -- 材料住哪一侧：本领域的关键词材料写证据侧，外部领域的写跨行业语料（`0044` 的隔离）。
    domain_scope text NOT NULL CHECK (domain_scope IN ('own_domain', 'cross_industry')),
    object_kind text NOT NULL CHECK (object_kind IN ('material_content', 'cross_industry_sample')),
    -- `linggan_material_content.public_ref` 或 `cross_industry_sample.sample_ref`。
    -- 刻意不做外键：两种对象住两张表，而这里记的是**这一侧的这个对象**。
    object_ref uuid NOT NULL,
    capability text NOT NULL CHECK (capability IN (
        'content_detail', 'media_slots', 'comments', 'replies'
    )),
    state text NOT NULL CHECK (state IN ('eligible', 'input_blocked', 'budget_exhausted')),
    reason_code text,
    -- 当前执行输入的指纹：已解析出的带签名地址的 sha256。空值表示「此刻没有可解析的输入」，
    -- 它是一条真实观察到的事实，不是一个待补的默认值。
    input_fingerprint text CHECK (input_fingerprint IS NULL OR input_fingerprint ~ '^[0-9a-f]{64}$'),
    -- 输入从哪来：证据侧的发现记录或跨行业样本行。它让「输入变了没有」可核对，
    -- 而不是靠时间戳重新判断。
    input_source_kind text CHECK (input_source_kind IS NULL OR input_source_kind IN (
        'discovery_finding', 'cross_industry_sample'
    )),
    input_source_ref uuid,
    -- 冻住输入的解析规则版本。规则变了就是另一套输入语义，不能与旧指纹直接比较。
    resolver_version text,
    -- `frozen` 是本表建立之后新工单的正常状态；`legacy_input_unfrozen` 是本表建立之前就已
    -- 存在的工单——它们的输入从来没被冻结过，只能如实这么记，不补造历史 URL，也不声称
    -- 它们从出生起就没有地址。
    input_source_status text NOT NULL CHECK (input_source_status IN (
        'frozen', 'legacy_input_unfrozen'
    )),
    -- 去重后的失败次数。同一个 failure_ref 重报只算一次（调用方按事件身份去重）。
    deduplicated_failure_count integer NOT NULL DEFAULT 0 CHECK (deduplicated_failure_count >= 0),
    next_retry_at timestamptz,
    -- 只有明确的故障修复证明或受控重新准入才增加。
    retry_epoch integer NOT NULL DEFAULT 0 CHECK (retry_epoch >= 0),
    last_event_ref uuid,
    policy_version text,
    -- **写在前驱行上的是 NULL，写在后继行上的是被它接替的那一条**：这一列存的是「我从哪
    -- 一条接过来的」。方向这样定，是因为前驱行一旦落库就是历史事实、不再改写，而「谁接替了
    -- 它」这件事只有在后继产生时才成立——写在后继那一行上，两个方向都不会出现「先写后改」。
    successor_eligibility_ref uuid REFERENCES collection_execution_input_eligibility(eligibility_ref),
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    -- 输入资格的身份**含指纹**（`01-目标合同` 的建表建议）：同一需求范围里，「没有地址时的
    -- 那一条」与「后来有了地址的那一条」是两条各自成立的事实，只留一条就得改写历史。
    -- 空值必须参与这条键：停止行的指纹是空的，而默认语义下空值互不相等——同一范围再记一次
    -- 「当时没有地址」就会多出一行，停止这件事于是按次数堆叠而不是按事实留一条。**这是一道
    -- 守卫，不是一条被用例证明过的必经路径**：今天「同一个范围再停一次」走不到写库这一步，
    -- 发租时的 `blocked_object_refs_in_transaction` 已经把输入未变的停止成员排除在任务之外，
    -- 所以去掉 `NULLS NOT DISTINCT` 现有用例不会变红（见 `docs/plans/active/collection-upgrade-001.md` §8）。
    CONSTRAINT collection_execution_input_eligibility_input_identity
        UNIQUE NULLS NOT DISTINCT (
            target_ref, domain_scope, object_kind, object_ref, capability, input_fingerprint, retry_epoch
        )
);

-- 但**「现在能不能执行」只有一条**。
--
-- 上面那条键管的是「事实有没有重复」，这条管的是「此刻谁说了算」：一个需求范围在任一时刻
-- 至多一条非停止行（`eligible` 或 `budget_exhausted`），否则输入指纹一变就能让同一篇作品
-- 同时挂着两条可执行资格，两个 scheduler 各派一张工单——正是「不能因输入指纹不同就并行
-- 派发同一缺口」要挡住的事。停止行不在索引里：它是历史，一个范围可以留下多条。
CREATE UNIQUE INDEX collection_execution_input_eligibility_current_idx
    ON collection_execution_input_eligibility (target_ref, domain_scope, object_kind, object_ref, capability)
    WHERE state <> 'input_blocked';

CREATE INDEX collection_execution_input_eligibility_pending_idx
    ON collection_execution_input_eligibility (target_ref, state, capability)
    WHERE state <> 'eligible';

COMMENT ON TABLE collection_execution_input_eligibility IS
    '执行资格与重试台账：一条记录回答某个需求范围（目标+领域+对象+通道）当前能不能执行、为什么不能、失败累计了几次、下次什么时候可以再试。它不复制 Package、Receipt 或材料事实，也不是第二套任务账本；执行权与交付事实仍只由 lease/task/attempt/package/receipt 表达。';

-- 「从未执行」需要一个如实的任务状态。
--
-- 现有两个终态都不能表达它：`unavailable` 是生产方确认页面不在，`blocked` 是读过但有界次
-- 数内没读成——两者都意味着**开过页面**。缺地址的成员在浏览器 Attempt 之前就被停下，
-- 既没有 Attempt 也没有 Package；把它记成 `blocked` 会把「从未开始」说成「试过没成功」，
-- 记成 `completed` 更是凭空制造成功。所以单列一个值，并明确它不持有任何 Evidence。
ALTER TABLE collection_work_order_lease_task
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_execution_state_check;
ALTER TABLE collection_work_order_lease_task
    ADD CONSTRAINT collection_work_order_lease_task_execution_state_check
    CHECK (
        (execution_state = 'pending' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'in_progress' AND claimed_at IS NOT NULL AND completed_at IS NULL)
        OR (execution_state = 'completed' AND claimed_at IS NOT NULL AND completed_at IS NOT NULL)
        OR (execution_state = 'unavailable' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'blocked' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'input_blocked' AND claimed_at IS NULL AND completed_at IS NULL)
    );

COMMENT ON COLUMN collection_work_order_lease_task.execution_state IS
    'Current execution eligibility only: pending/in_progress are runnable; completed has an accepted Package and Receipt; unavailable is a producer-confirmed absent page; blocked is a bounded page-read failure without accepted Evidence; input_blocked is a member whose required execution input (a signed locator) was missing before any Attempt — it never opened a page and holds no Evidence.';

-- 这条约束是 `0047` 的列内 CHECK，由 PostgreSQL 自动命名，不是迁移里写明的名字。自动
-- 命名在超长时**截断表名**（`collection_work_order_lease_task_disp`）而不是截尾部，所以
-- 按「表名_列名_check」直拼出来的名字根本不存在——`DROP CONSTRAINT IF EXISTS` 会静默跳过，
-- 旧约束继续拦人，而新约束只是躺在旁边。名字必须逐字用 PostgreSQL 实际生成的那个。
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_disp_failure_disposition_check;
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD CONSTRAINT collection_work_order_lease_task_disp_failure_disposition_check
    CHECK (failure_disposition IN ('requeued', 'unavailable', 'blocked', 'input_blocked'));

COMMENT ON COLUMN collection_work_order_lease_task_dispatch_failure.failure_disposition IS
    'Durable replay outcome for a pre-Attempt dispatch failure. It is not source Evidence and never stores raw browser or platform error text. input_blocked means the required execution input was missing: the range stopped instead of being requeued.';

-- 缺输入是一次**停止**，不是一次「试过没成功」。给它自己的失败码，而不是继续复用
-- `execution_locator_unavailable`：后者描述的是「这一轮没拿到地址，进冷却，过一会儿再来」，
-- 恰好是这次要废掉的语义。两个码同名，运维看板上就永远分不出「在退避」和「已经停了」。
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_task_dispatch_failure_failure_code_check;
ALTER TABLE collection_work_order_lease_task_dispatch_failure
    ADD CONSTRAINT collection_work_order_lease_task_dispatch_failure_failure_code_check
    CHECK (failure_code IN (
        'capability_not_executable_here','target_incomplete','tab_unavailable','page_timeout',
        'page_unavailable','page_receipt_missing','page_receipt_identity_mismatch','page_read_failed',
        'detail_page_url_invalid','account_observation_blocked','execution_locator_unavailable',
        'execution_input_missing',
        'detail_page_session_grant_unavailable','detail_page_session_recovery_required',
        'capture_delivery_rejected'
    ));

-- 「这一张租约是因为成员缺输入而停的」需要与既有几个原因分开记。
--
-- `partial` 的意思是「一部分交付完成、一部分没有」；一张成员全缺输入的租约并没有交付任何
-- 东西，记成 `partial` 会让人以为有材料进来了。`execution_locator_unavailable` 是**可重试**
-- 冷却的那个原因，而缺输入恰恰是「不该再自动重试」，两者同名会继续把两件事混成一件。
ALTER TABLE collection_work_order_lease
    DROP CONSTRAINT IF EXISTS collection_work_order_lease_release_reason_check;
ALTER TABLE collection_work_order_lease
    ADD CONSTRAINT collection_work_order_lease_release_reason_check
    CHECK (release_reason IN (
        'completed','partial','expired','revoked','station_unavailable','dispatch_start_failed',
        'execution_locator_unavailable','input_blocked'
    ));

COMMENT ON COLUMN collection_work_order_lease.release_reason IS
    'Why this lease stopped holding execution permission. input_blocked means the lease ended because its remaining members had no resolvable execution input — it is a stop, not a delivery and not a retryable cooldown.';
