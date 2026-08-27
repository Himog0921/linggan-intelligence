-- COLLECTION-001 · 风险暂停
--
-- 准入第 5 问要求回答「此前风险暂停是否仍在生效」（合同 §2.1）。没有这张表，这一问
-- 只能靠「我们没有暂停机制」来搪塞——那不是回答，是回避。有了它，答案变成一次可核对的
-- 查询：表里没有生效中的记录，就是真的没有暂停。
--
-- 空表本身就是有效答案。这张表存在的意义不是「以后会用到」，而是让「没有暂停」这句话
-- 现在就有依据。

CREATE TABLE collection_risk_pause (
    pause_ref uuid PRIMARY KEY,

    -- 暂停范围。NULL 表示全局：所有 lane、所有平台一律停。
    platform text CHECK (platform IS NULL OR platform IN ('xhs')),
    lane text CHECK (lane IS NULL OR lane IN ('deep_archive', 'patrol')),

    -- 为什么停。必填：一个没写原因的暂停，以后没人敢解除，因为没人知道当初怕的是什么。
    reason text NOT NULL CHECK (length(btrim(reason)) > 0),

    -- 谁停的。人或系统都可以停；只有人能解除——系统不该自行判定风险已经过去。
    paused_by text NOT NULL CHECK (paused_by IN ('person', 'system')),
    paused_at timestamptz NOT NULL DEFAULT scope_001_now(),

    lifted_at timestamptz,
    lifted_reason text,

    CHECK ((lifted_at IS NULL) = (lifted_reason IS NULL))
);

CREATE INDEX collection_risk_pause_active_idx
    ON collection_risk_pause (platform, lane) WHERE lifted_at IS NULL;
