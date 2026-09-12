-- 一个观察目标可以同时有几条巡检规则，各自排序、各自周期。
--
-- 此前是**一个目标一条规则**：`collection_observation_target.active_monitor_rule_revision_ref`
-- 是单列，排期状态（`monitor_next_run_at` 等）也挂在目标行上。关键词因此被迫把排序刻进身份
-- （`考研自习::most_liked`、`考研自习::comprehensive` 是两个目标），同一个词在列表上占两行，
-- 而「这个词要同时盯综合榜和点赞榜」本来是一个词的两条规则，不是两个词。
--
-- **博主那条路不受影响**：它永远只有一条规则（首要槽），迁移给每个现有目标补一条，行为与
-- 今天完全一致。多出来的槽只有关键词会用。
--
-- 目标行上那一列与它的两条 CHECK **原样保留**——它们守的是「监控中的目标必须有规则」，
-- 这条不变量不该为了加功能而放松。它现在的含义是「首要规则的当前版本」。

CREATE TABLE collection_monitor_rule (
    rule_ref uuid PRIMARY KEY,
    target_ref uuid NOT NULL REFERENCES collection_observation_target(target_ref),
    -- 这条规则在这个目标底下的身份。关键词用排序口径（`most_liked`），博主固定 `primary`。
    -- 同一个目标不能有两条同口径的规则——那是同一件事配置了两遍，跑出来的东西无法区分。
    slot_key text NOT NULL,
    -- 首要规则就是目标行上那一列指着的那条。博主只有它。
    is_primary boolean NOT NULL DEFAULT false,
    active_revision_ref uuid,
    -- 排期状态按**规则**走：三条规则各有各的周期，共用一个 next_run_at 说不清谁该跑。
    monitor_next_run_at timestamptz,
    monitor_missed_run_count integer NOT NULL DEFAULT 0,
    last_patrol_dispatched_at timestamptz,
    last_patrol_succeeded_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    -- 停用一条规则不删它：它签发过的工单与材料还挂在它的版本上，删掉等于让那些材料
    -- 说不清是按什么口径取回来的。
    retired_at timestamptz,
    UNIQUE (target_ref, rule_ref),
    CHECK (length(btrim(slot_key)) > 0),
    CHECK (monitor_missed_run_count >= 0)
);

-- 同一个目标底下，同一个口径只能有一条**在用**的规则。停用过的不占位。
CREATE UNIQUE INDEX collection_monitor_rule_live_slot_idx
    ON collection_monitor_rule (target_ref, slot_key) WHERE retired_at IS NULL;

-- 首要规则每个目标只有一条。
CREATE UNIQUE INDEX collection_monitor_rule_primary_idx
    ON collection_monitor_rule (target_ref) WHERE is_primary AND retired_at IS NULL;

CREATE INDEX collection_monitor_rule_due_idx
    ON collection_monitor_rule (monitor_next_run_at) WHERE retired_at IS NULL;

-- 规则版本挂到规则身份下。版本号此前按目标计，现在按规则计——两条规则各自改过几次，
-- 是两条互不相干的历史。
ALTER TABLE collection_monitor_rule_revision
    ADD COLUMN rule_ref uuid;

-- 回填：每个现有目标一条首要规则，把它的生效版本与排期状态原样搬过去。
INSERT INTO collection_monitor_rule
    (rule_ref,target_ref,slot_key,is_primary,active_revision_ref,
     monitor_next_run_at,monitor_missed_run_count,
     last_patrol_dispatched_at,last_patrol_succeeded_at)
SELECT gen_random_uuid(),target_ref,'primary',true,active_monitor_rule_revision_ref,
       monitor_next_run_at,monitor_missed_run_count,
       last_patrol_dispatched_at,last_patrol_succeeded_at
FROM collection_observation_target;

UPDATE collection_monitor_rule_revision revision
   SET rule_ref = rule.rule_ref
  FROM collection_monitor_rule rule
 WHERE rule.target_ref = revision.target_ref AND rule.is_primary;

ALTER TABLE collection_monitor_rule_revision
    ALTER COLUMN rule_ref SET NOT NULL,
    ADD CONSTRAINT collection_monitor_rule_revision_rule_fk
        FOREIGN KEY (target_ref, rule_ref)
        REFERENCES collection_monitor_rule (target_ref, rule_ref);

ALTER TABLE collection_monitor_rule
    ADD CONSTRAINT collection_monitor_rule_active_revision_fk
        FOREIGN KEY (target_ref, active_revision_ref)
        REFERENCES collection_monitor_rule_revision (target_ref, rule_revision_ref);

COMMENT ON TABLE collection_monitor_rule IS
  '一个观察目标底下的一条巡检规则：口径、周期与排期状态；博主只有首要那一条';
COMMENT ON COLUMN collection_monitor_rule.slot_key IS
  '这条规则在这个目标底下的身份：关键词用排序口径，博主固定 primary';

-- 调度的「最近考虑过就先别再看」这道闸此前按**目标**算。多规则下它会把同一个词的第二条
-- 规则一起挡住：点赞榜刚跑过，综合榜就得等下一个窗口——两条规则各有各的周期，互相阻塞
-- 说不出理由。闸按规则算。
ALTER TABLE collection_scheduler_target_decision
    ADD COLUMN rule_ref uuid REFERENCES collection_monitor_rule(rule_ref);

UPDATE collection_scheduler_target_decision decision
   SET rule_ref = rule.rule_ref
  FROM collection_monitor_rule rule
 WHERE rule.target_ref = decision.target_ref AND rule.is_primary;

CREATE INDEX collection_scheduler_target_decision_rule_idx
    ON collection_scheduler_target_decision (rule_ref, next_eligible_at DESC);

COMMENT ON COLUMN collection_scheduler_target_decision.rule_ref IS
  '这次决定针对哪一条巡检规则。历史行回填为该目标的首要规则';
