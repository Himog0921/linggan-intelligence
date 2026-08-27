-- COLLECTION-001 · 观察目标的巡检节奏
--
-- 设计取自内容工作台的 `MonitorConfig`（`interval` 秒级间隔 + `lastCheckedAt` +
-- `lastScheduledAt`），那套在真实环境跑了数月。**但不复制它的表结构**：内容工作台把
-- 监控配置单列一张表，是因为那边没有一等的观察目标对象；灵感这边目标本身就是一等对象，
-- 再建一张配置表只会让「同一个博主」有两个身份。
--
-- 这张迁移只给观察目标加上「多久看一次、上次什么时候看的」。

ALTER TABLE collection_observation_target
    -- 巡检开关。与 lifecycle_state 分开：一个目标可以「已建档」但被人暂停巡检，
    -- 那是两件事，压成一个状态就分不清「没在跑」是因为暂停还是因为还没建档。
    ADD COLUMN monitoring_enabled boolean NOT NULL DEFAULT false,

    -- 巡检间隔（秒）。默认 86400 = 24 小时，与产品规则 §4 一致。
    --
    -- 可配置是硬要求：规则文档明写旧项目的 `dailyLimitForPlatform()` 收了参数却硬返回
    -- 常量，是半成品。上下限 6 小时 ~ 7 天同样来自规则文档 DECISION-03。
    ADD COLUMN patrol_interval_seconds integer NOT NULL DEFAULT 86400
        CHECK (patrol_interval_seconds BETWEEN 21600 AND 604800),

    -- 上一次真的派出巡检的时间。到期判断只看它，不看采集是否成功——采集失败也算「看过了」，
    -- 否则一个持续失败的目标会被无限重试，把当天额度吃光。
    ADD COLUMN last_patrol_dispatched_at timestamptz,

    -- 上一次成功拿回材料的时间。与上一条分开，因为「派过」与「成了」是两个事实：
    -- 只记其一，就无法回答「这个目标是一直在跑但一直失败，还是根本没被派过」。
    ADD COLUMN last_patrol_succeeded_at timestamptz;

-- 到期扫描只关心开着巡检的目标。
CREATE INDEX collection_observation_target_due_idx
    ON collection_observation_target (last_patrol_dispatched_at NULLS FIRST)
    WHERE monitoring_enabled;
