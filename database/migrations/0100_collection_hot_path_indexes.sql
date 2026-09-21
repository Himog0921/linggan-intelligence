-- COLLECTION-UPGRADE-001 · S5：两条被测量证实的顺序扫描
--
-- 这一版只为两条**已经量出成本**的语句加索引。两条语句的共同形状是「按一个外键取最近
-- 一行」，而它们各自的表上都没有能直接回答这个问题的索引：主键在外键的另一侧，既有的
-- 部分索引又都带 `WHERE` 条件，覆盖不到「不管状态、只要最近那条」这个问法。
--
-- 一条纪律先写在这里：本仓库的 migration runner 把每个文件放在 BEGIN/COMMIT 里，因此
-- 不能用 `CREATE INDEX CONCURRENTLY`。这两条就是普通事务内的常规索引（同 `0077`）。

-- 一、媒体物化：按下载尝试取最近一次物化
--
-- 两个调用点，都是同一条问法：
--   * 上传会话 finalize 时确认「这次下载已经物化过了吗」；
--   * 采集页每一行渲染时的封面查找（`ORDER BY … verified_at DESC LIMIT 1`）。
--
-- 第二个键就是语句里的排序键，让 `ORDER BY … DESC LIMIT 1` 直接落在索引首行，不必取回
-- 全部候选再排序。第一条键**不加条件**：这条语句问的是「最近一次物化」，不论它属于哪次
-- 下载尝试；`materialization_ref` 的主键查询本来就不缺索引，这里不为它再造一条。
CREATE INDEX linggan_media_materialization_download_attempt_latest_idx
    ON linggan_media_materialization (download_attempt_ref, verified_at DESC);

COMMENT ON INDEX linggan_media_materialization_download_attempt_latest_idx IS
  'COLLECTION-UPGRADE-001 S5：按 download_attempt_ref 取最近一次物化（finalize 与采集页封面）。';

-- 二、工单租约：取某张工单的最近一份租约
--
-- 采集控制面渲染每一行冻结工单时，都要问「它最近那份租约是谁发的、什么时候到期、释放了
-- 没有」。既有的两条索引答不了：`_live_idx` 是**唯一约束**（一单一活租约），`_station_idx`
-- 按工位排队——两条都带 `WHERE released_at IS NULL`，而这条问法要看**已释放的**租约，
-- 于是每次都退化成整表扫描（实测每次渲染扫 100 次，是运行库上顺序扫描量最大的表）。
--
-- 同样不加 `WHERE`：已释放的租约正是这条语句要显示的东西。既有的两条索引继续保留——
-- `_live_idx` 是约束不是优化，`_station_idx` 服务的是另一个问法（按工位看到期）。
CREATE INDEX collection_work_order_lease_work_order_latest_idx
    ON collection_work_order_lease (work_order_ref, issued_at DESC);

COMMENT ON INDEX collection_work_order_lease_work_order_latest_idx IS
  'COLLECTION-UPGRADE-001 S5：取某张工单的最近一份租约（采集控制面冻结物逐行读取）。';
