-- COLLECTION-UPGRADE-001 · S4b：tick 的步骤结果与就绪落点
--
-- 一个 tick 走四步（媒体投影 / 渐进档案 / 关键词建档 / 巡查），此前只有巡查写
-- `collection_scheduler_run`，另外三步只打日志。于是一步失败时，账本上只留下一句「这一轮
-- 没派活」，分不清是「没有到期的活」还是「这一步根本没跑成」。S4b 要修的就是这一句。
--
-- 这不是第二套巡检账本：run 仍是一 tick 一行，目标级决定仍住
-- `collection_scheduler_target_decision`；本表只是同一行 run 的步骤明细。
--
-- 结果三值，外加一个**未完成态**：
--
--   ok        这一步跑完了（计数见三个 count 列）
--   failed    这一步跑了并失败，`error_class` 记受限类别（如 SQLSTATE 类别），不记原始报文
--   skipped   这一步没轮到，`skipped_reason` 记受限原因
--   NULL      开始了但没有收尾（进程被杀 / 崩溃）——「未知」必须查得出来，不能写成 ok
--
-- 只有 `ok` 的行带计数，其余三种一律留空：一个崩在半路的步骤考虑过多少条、排出过几张工单，
-- 不是我们知道的事实。
--
-- `error_class` 与 `skipped_reason` 把字符集写死在 [a-z0-9_]{1,32}：让「受限」是机械可判的，
-- 而不是一句约定。报文、连接串、选择器串、页面文本都没有能通过这道 CHECK 的形状。
--
-- 心跳加三列：就绪判定是持续故障的**持久**落点——日志与 `/health` 都是即时视图，重启即丢，
-- 而「这台机器从上周起就一直没迁移完」是重启之后仍然要能查出来的事实。

CREATE TABLE collection_scheduler_run_step (
    scheduler_run_step_ref uuid PRIMARY KEY,
    scheduler_run_ref uuid NOT NULL REFERENCES collection_scheduler_run(scheduler_run_ref),
    step_key text NOT NULL CHECK (step_key IN (
        'media_acquisition', 'progressive_dossiers', 'keyword_details', 'patrol'
    )),
    started_at timestamptz NOT NULL DEFAULT scope_001_now(),
    completed_at timestamptz,
    outcome text CHECK (outcome IN ('ok', 'failed', 'skipped')),
    skipped_reason text CHECK (skipped_reason IS NULL OR skipped_reason ~ '^[a-z0-9_]{1,32}$'),
    error_class text CHECK (error_class IS NULL OR error_class ~ '^[a-z0-9_]{1,32}$'),
    -- 计数**可空**：`ok` 的行写出这一步数过的数，失败 / 跳过 / 未完成的行留空。
    -- 0 是一个事实（数过了，是零），不能拿它冒充「没数过」——一个崩在半路的巡查步
    -- 到底考虑过多少条规则、排出过几张工单，我们并不知道。下面那条 CHECK 让这件事
    -- 由数据库判，而不是靠写代码的人记得。
    considered_count integer CHECK (considered_count IS NULL OR considered_count >= 0),
    produced_count integer CHECK (produced_count IS NULL OR produced_count >= 0),
    skipped_count integer CHECK (skipped_count IS NULL OR skipped_count >= 0),
    -- 与 run 行同一条纪律：没有 completed_at 就没有 outcome。半行只能是「未知」。
    CHECK ((completed_at IS NULL) = (outcome IS NULL)),
    -- 没轮到的理由必须写出来；跑完的步骤不许留理由。
    CHECK ((outcome = 'skipped') = (skipped_reason IS NOT NULL)),
    CHECK (error_class IS NULL OR outcome = 'failed'),
    -- 只有跑完的步骤才有计数。失败与跳过的行必须三个都空。
    CHECK (outcome = 'ok' OR (considered_count IS NULL AND produced_count IS NULL
        AND skipped_count IS NULL)),
    -- 一步一轮只记一行：重复执行同一格是记账错误，不是新事实。
    UNIQUE (scheduler_run_ref, step_key)
);

COMMENT ON TABLE collection_scheduler_run_step IS
    'One row per tick step. Child of collection_scheduler_run; never a second scheduler ledger.';

COMMENT ON COLUMN collection_scheduler_run_step.started_at IS
    'Database clock at insert. The process clock only supplies the duration reported in events.';

COMMENT ON COLUMN collection_scheduler_run_step.outcome IS
    'ok / failed / skipped. NULL means started but never finished (killed mid-step) — unknown, not ok.';

-- 「失败与未完成」是查得最多的两类，普通成功不该被反复扫描。
CREATE INDEX collection_scheduler_run_step_unfinished_idx
    ON collection_scheduler_run_step (completed_at DESC)
    WHERE outcome IS DISTINCT FROM 'ok';

ALTER TABLE collection_scheduler_heartbeat
    ADD COLUMN readiness_state text NOT NULL DEFAULT 'unknown'
        CHECK (readiness_state IN (
            'unknown', 'not_configured', 'database_unreachable', 'migration_ledger_unreadable',
            'migrations_not_applied', 'schema_incompatible', 'ready'
        )),
    -- 只写第一个缺失的 migration id / 第一个缺失的对象名 / 探针失败的 SQLSTATE。
    ADD COLUMN readiness_detail text
        CHECK (readiness_detail IS NULL OR length(readiness_detail) <= 120),
    -- 判定时刻来自**数据库时钟**；问不到时钟时为空，不拿本地时间冒充。
    ADD COLUMN readiness_checked_at timestamptz;

COMMENT ON COLUMN collection_scheduler_heartbeat.readiness_state IS
    'Last readiness classification. Anything other than ready means this machine must not claim work.';
COMMENT ON COLUMN collection_scheduler_heartbeat.readiness_detail IS
    'Bounded reason: first missing migration id, first missing object, or probe SQLSTATE. Never a raw message.';
