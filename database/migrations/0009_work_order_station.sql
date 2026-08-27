-- COLLECTION-001 · 工单记下它被准入时认定的工位
--
-- 准入第 5 问认定「有兼容工位」之后，必须把认定的是哪一台写下来。否则「这台工位今天
-- 已经承诺了多少篇」无从计算，每日额度就只是一个写在工位上、永远不会被触发的数字。
--
-- 可空：0006 建表时准入还答不出第 5 问，那时的工单不可能带工位。历史行保持为空，
-- 而不是补一个猜出来的值。
ALTER TABLE collection_work_order
    ADD COLUMN station_ref uuid REFERENCES execution_station(station_ref);

CREATE INDEX collection_work_order_station_day_idx
    ON collection_work_order (station_ref, created_at DESC)
    WHERE station_ref IS NOT NULL;
