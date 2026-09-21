-- COLLECTION-UPGRADE-001 · 详情页会话的通道交付身份预登记
--
-- 「一次打开顺手读完冻结的四个通道」把交付推到了页面之外：正文、媒体卡槽、评论、回复各自
-- 在自己的本机 outbox 里慢慢投递，等到投递时最初那份租约可能早已结束。而现有实现里
-- Attempt 身份是**投递时**才由插件生成并登记的。两者相加就是一个真实的坑：
-- 「页面读完了，但从那一刻起服务端不可达」的新包在服务端没有任何身份——租约一关，
-- 起步登记只剩「活权不在了」这一条路，那份已经取回的材料再也进不来。
--
-- 这张表把身份登记提前到导航之前：在授权事务内，为冻结计划中的每个通道登记一个稳定的
-- attempt_id，插件在打开页面前把它持久化；此后无论隔多久投递，身份都还在。
--
-- 它登记的是**受权的准备**，不是执行。这里没有「通道已访问、已采集、已交付或已完成」，
-- 也没有任何状态列：某个通道到底走到哪一步，仍然只由 capture_package 与
-- submission_receipt 表达，本表不构成第二套任务账本，也不复制第二份状态真相。
-- 每个通道的 Task 仍来自工单租约，本表只为已存在的任务登记身份。

CREATE TABLE collection_detail_page_session_lane_preparation (
    preparation_ref uuid PRIMARY KEY,
    session_ref uuid NOT NULL REFERENCES collection_detail_page_session(session_ref),
    -- 登记时的工位。工位被替换（superseded）之后，这份准备不再构成「新的开始」依据。
    owner_installation_ref uuid NOT NULL REFERENCES plugin_installation(installation_ref),
    capability text NOT NULL CHECK (capability IN (
        'content_detail', 'media_slots', 'comments', 'replies'
    )),
    -- 该通道在工单租约里已经存在的任务。登记身份，不创建任务。
    task_id uuid NOT NULL REFERENCES linggan_runtime_task(task_id),
    -- 登记时所依据的租约。租约结束不会使这份准备消失：材料已经取回这件事，
    -- 不因租约到期而作废（晚到包按既有 LOST_AUTHORITY 语义接纳）。
    lease_ref uuid NOT NULL REFERENCES collection_work_order_lease(lease_ref),
    attempt_id uuid NOT NULL,
    -- 登记时冻结的计划摘要，用于事后核对这份身份属于哪一份计划。
    plan_hash text NOT NULL CHECK (plan_hash ~ '^[0-9a-f]{64}$'),
    prepared_at timestamptz NOT NULL DEFAULT scope_001_now(),
    -- 一个会话的一个通道只有一份准备：同一 grant_request_id 重放必须拿回同一个身份，
    -- 换了通道集合或目标就是另一份计划，而不是给旧计划再加几个身份。
    UNIQUE (session_ref, capability),
    UNIQUE (attempt_id)
);

CREATE INDEX collection_detail_page_session_lane_preparation_task_idx
    ON collection_detail_page_session_lane_preparation (task_id);

COMMENT ON TABLE collection_detail_page_session_lane_preparation IS
    '导航前登记的通道交付身份：服务端已受权为冻结计划的每个通道准备了稳定的 Attempt 身份。它不表示该通道已被访问、已采集、已交付或已完成，也不改变任何任务的执行状态。';
