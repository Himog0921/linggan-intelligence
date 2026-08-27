-- COLLECTION-001 · 工单租约：把一张工单变成一次有界、可撤销的执行许可
--
-- 合同 §「每次 Attempt 由服务端冻结 Capture Identity 与有限 lease。lease 只允许当前工位
-- 在有效期内进行规定访问，不能修改 Work 的 lane、目标、观察身份或止损边界」。
--
-- 现有的 linggan_runtime_attempt 没有任何租约字段——它服务的是插件自己发起的手动任务，
-- 那种任务的边界由发起它的人当场决定。服务端派发的任务不同：下发与执行之间隔着时间，
-- 没有到期时间的许可一旦发出就再也收不回来。
--
-- 这张表**不派发任何东西**。它记录的是「谁、在什么范围内、到什么时候为止，被允许执行
-- 这张工单」。插件是否真的去跑，是后面的事。

CREATE TABLE collection_work_order_lease (
    lease_ref uuid PRIMARY KEY,

    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    -- 发租给哪台工位。租约不可转让：换工位必须换新租约（合同：换账号/工位须新建 Attempt）。
    station_ref uuid NOT NULL REFERENCES execution_station(station_ref),

    -- 由本租约创建的派发任务。插件将来按这个 task 执行，规格在发租时即冻结。
    task_id uuid REFERENCES linggan_runtime_task(task_id),

    -- 发租那一刻冻结的执行身份：平台、目标、lane、许可能力、篇数上限、止损条件。
    -- 冻结的意义是执行期间它不会跟着目标或授权的后续变化而漂移——事后追责时，
    -- 依据的是当时批准的那份，不是现在这份。
    capture_identity jsonb NOT NULL,

    issued_at timestamptz NOT NULL DEFAULT scope_001_now(),
    -- 必填。没有到期时间的租约等于永久授权，与没有租约无异。
    expires_at timestamptz NOT NULL,

    released_at timestamptz,
    -- 为什么结束。到期与被撤销必须分得开：前者是正常边界，后者是有人踩了刹车。
    release_reason text CHECK (release_reason IN (
        'completed', 'expired', 'revoked', 'station_unavailable'
    )),

    CHECK (expires_at > issued_at),
    CHECK ((released_at IS NULL) = (release_reason IS NULL))
);

-- 一张工单同时只能有一份未结束的租约。两份并存就是同一份工作被派了两次。
CREATE UNIQUE INDEX collection_work_order_lease_live_idx
    ON collection_work_order_lease (work_order_ref) WHERE released_at IS NULL;

CREATE INDEX collection_work_order_lease_station_idx
    ON collection_work_order_lease (station_ref, expires_at DESC) WHERE released_at IS NULL;
