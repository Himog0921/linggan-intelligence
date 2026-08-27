-- COLLECTION-001 · 真实执行闸门
--
-- 合同 §12 列了进入真实 Canary 的 7 条退出门，其中第 3 条要求一份新的真实 Canary SCOPE
-- 明确平台、镜头、lane、目标语义、最大范围、停止与恢复。那份文档由人授权，不由代码推断。
--
-- 这张表让「闸门关着」成为一个**可查询的事实**，而不是「代码里还没写派发」。两者的区别
-- 很实在：后者会在某次重构里被无意打开，而且没人说得清它当初为什么是关的。
--
-- 空表 = 关闭。与风险暂停同一个思路：空表本身就是有效答案。

CREATE TABLE collection_execution_gate (
    gate_ref uuid PRIMARY KEY,

    -- 闸门覆盖范围。NULL 表示全部——但开一个全平台全 lane 的闸门应当是刻意为之。
    platform text CHECK (platform IS NULL OR platform IN ('xhs')),
    lane text CHECK (lane IS NULL OR lane IN ('deep_archive', 'patrol')),

    -- 这一次开闸依据哪份 Canary SCOPE。必填：没有文档依据的开闸，事后无法回答
    -- 「当时批准的范围到底是什么」。
    scope_reference text NOT NULL CHECK (length(btrim(scope_reference)) > 0),

    -- 只有人能开闸。系统不得因为「条件看起来都满足了」就自行放行。
    opened_by text NOT NULL CHECK (opened_by = 'person'),
    opened_at timestamptz NOT NULL DEFAULT scope_001_now(),

    -- 必填。一个不会自己关上的闸门，等于把 Canary 变成常态。
    expires_at timestamptz NOT NULL,

    closed_at timestamptz,
    close_reason text,

    CHECK (expires_at > opened_at),
    CHECK ((closed_at IS NULL) = (close_reason IS NULL))
);

CREATE INDEX collection_execution_gate_open_idx
    ON collection_execution_gate (platform, lane, expires_at DESC) WHERE closed_at IS NULL;
