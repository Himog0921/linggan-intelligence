-- COLLECTION-001 · 一个 lease 顺序承载多个冻结的 Producer task
--
-- 0010 的 `collection_work_order_lease.task_id` 只能指向首个 task；creator 工单实际会生成
-- `author_profile` 与 `profile_discovery` 两步，第二步因此失去 lease 归属。保留旧列只为读取
-- 已存在的数据库行；从本迁移起，`collection_work_order_lease_task` 是唯一任务绑定与执行状态
-- 真源，新租约不得再同时写旧列。

CREATE TABLE collection_work_order_lease_task (
    lease_ref uuid NOT NULL REFERENCES collection_work_order_lease(lease_ref),
    task_id uuid NOT NULL UNIQUE REFERENCES linggan_runtime_task(task_id),
    sequence_no integer NOT NULL CHECK (sequence_no > 0),

    -- pending 尚未交给任何安装；in_progress 已被一次原子 claim 独占；completed 表示对应
    -- Capture Package 已被服务端接纳。claim 不是 Attempt，也不是完成回执。
    execution_state text NOT NULL DEFAULT 'pending'
        CHECK (execution_state IN ('pending', 'in_progress', 'completed')),
    claimed_at timestamptz,
    claimed_by_installation_ref uuid REFERENCES plugin_installation(installation_ref),
    completed_at timestamptz,

    PRIMARY KEY (lease_ref, sequence_no),
    CHECK (
        (execution_state = 'pending' AND claimed_at IS NULL AND completed_at IS NULL)
        OR (execution_state = 'in_progress' AND claimed_at IS NOT NULL AND completed_at IS NULL)
        OR (execution_state = 'completed' AND claimed_at IS NOT NULL AND completed_at IS NOT NULL)
    )
);

-- 旧 lease 最多只有首个 task。按已有 Attempt/Package 事实回填它的当前执行阶段；历史 claim
-- 没有安装标识可追溯，因此 claimed_by_installation_ref 保持 NULL，不猜造。
INSERT INTO collection_work_order_lease_task (
    lease_ref,
    task_id,
    sequence_no,
    execution_state,
    claimed_at,
    completed_at
)
SELECT
    lease.lease_ref,
    lease.task_id,
    1,
    CASE
        WHEN package.package_ref IS NOT NULL THEN 'completed'
        WHEN attempt.attempt_id IS NOT NULL THEN 'in_progress'
        ELSE 'pending'
    END,
    CASE
        WHEN attempt.attempt_id IS NOT NULL THEN attempt.started_at
        ELSE NULL
    END,
    package.accepted_at
FROM collection_work_order_lease lease
LEFT JOIN LATERAL (
    SELECT candidate.attempt_id, candidate.started_at
    FROM linggan_runtime_attempt candidate
    WHERE candidate.task_id = lease.task_id
    ORDER BY candidate.started_at DESC
    LIMIT 1
) attempt ON true
LEFT JOIN linggan_runtime_capture_package package
    ON package.attempt_id = attempt.attempt_id
WHERE lease.task_id IS NOT NULL;

-- 在旧模型里，首个 task 的 Package 被接纳就代表整份 lease 应当结束。若此前 API 在包提交
-- 后、租约收尾前崩溃，这类行会留下“已有终态 Package 但 lease 仍活着”的断点；迁移按旧合同
-- 收口它，避免 completed 回填行永远无法再从 in_progress 路径结束。
UPDATE collection_work_order_lease lease
SET released_at = completed.completed_at,
    release_reason = 'completed'
FROM collection_work_order_lease_task completed
WHERE completed.lease_ref = lease.lease_ref
  AND completed.execution_state = 'completed'
  AND lease.released_at IS NULL;

CREATE INDEX collection_work_order_lease_task_pending_idx
    ON collection_work_order_lease_task (lease_ref, sequence_no)
    WHERE execution_state = 'pending';

COMMENT ON COLUMN collection_work_order_lease.task_id IS
    'Legacy compatibility only. New leases leave this NULL; collection_work_order_lease_task is authoritative.';
