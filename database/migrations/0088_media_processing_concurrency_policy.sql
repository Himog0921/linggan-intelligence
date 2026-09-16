-- 媒体处理的并发上限此前不存在，既没有配置项也没有闸。
--
-- worker 进程内部是严格串行的（`media_worker.rs` 的 `for _ in 0..MAX_JOBS_PER_TICK` 逐条
-- `.await`；那个常量是「每 tick 最多取几条」的吞吐，不是并行），launchd 也只托管一个进程。
-- 所以「并发是 1」一直是**进程数的副产品**，不是任何一处写下来的规定。
--
-- 2026-09-16 线上校准：实际有 6 个 media-worker 进程在跑同一个库——另外 5 个是从已被删除的
-- worktree（`.worktrees/audit`、`.worktrees/per-rule-pause`）遗留的孤儿进程，跑着目录都不存在
-- 的旧二进制，各自持有到生产库的连接并持续认领工单。并发上限既不是 1，也没有任何办法设成 1。
--
-- **上限必须存在数据库里，不能放在各进程的环境变量里**：两个进程各设各的，加起来必然超；
-- 而且读取点替执行做的决定会随进程重启漂移。这和「冻结的作用域行才是执行权威」是同一条
-- 道理——一条工单此刻能不能被认领，由库里的策略说，不由谁在跑说了算。
--
-- 初值一律 1：worker 本来就是串行的，所以这不降低任何现有吞吐；但无论有几个进程、是谁起的、
-- 跑的是哪一版二进制，每个 processor_kind 同时在跑的都越不过 1 条。要放宽直接改这张表，
-- 不需要重新部署。
--
-- **表里没有的 processor_kind 一律不可认领**（Closed World，与 SCOPE-001 一致）：认领查询用
-- INNER JOIN，未登记的 kind 取不到候选。
--
-- 漏登记有两道，缺一不可：
--   1. 证明侧——`media_processing_concurrency_postgres.rs` 的
--      `every_processor_kind_the_schema_allows_is_registered_for_concurrency` 会变红。
--   2. 运行侧——`read_claim_gate_readiness` 在 worker 启动时把没登记的 kind 点名打出来。
--      少了这一道，「漏登记」的表现只是**永远领不到活且一行日志都不打**：进程看着健康，
--      界面照常，而它空转。本包要消灭的就是这种失败形态，闸门自己不能也这样藏起来。

CREATE TABLE linggan_media_processing_concurrency (
    processor_kind text NOT NULL PRIMARY KEY,
    max_in_flight integer NOT NULL CHECK (max_in_flight >= 1),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now()
);

COMMENT ON TABLE linggan_media_processing_concurrency IS
    'How many Work Items of one processor kind may be leased across the whole database at once. Enforced inside the claim transaction, so it holds no matter how many worker processes exist.';
COMMENT ON COLUMN linggan_media_processing_concurrency.max_in_flight IS
    'Upper bound on concurrently leased Work Items of this kind. A processor kind with no row here can never be claimed.';

INSERT INTO linggan_media_processing_concurrency (processor_kind, max_in_flight) VALUES
    ('thumbnail', 1),
    ('image_ocr', 1),
    ('audio_extract', 1),
    ('asr', 1),
    ('video_frame_ocr', 1);
