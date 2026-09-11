-- 关键词建档的详情补采：这一张工单要补哪几篇跨行业样本的详情。
--
-- 博主那条路的作用域表是 `collection_work_order_material_target`，它的外键指向
-- `linggan_material_content`——证据侧的作品。跨行业样本**不在**那张表里，而且按
-- `0044` 的隔离设计也不该被塞进去：证据库只装本领域的材料，参照物不进去。
--
-- 所以这里另立一张同构的作用域表，指向 `cross_industry_sample`。两张表各管一侧，
-- 发租时一起读出来展开成逐篇详情任务。**不合并成一张带两个可空外键的表**：那样每个
-- 读取点都要记得判断「这一行是哪一侧的」，漏判一次就会把参照物当证据处理，而这正是
-- `0044` 用复合外键防住的那件事。

CREATE TABLE collection_work_order_cross_industry_target (
    work_order_ref uuid NOT NULL REFERENCES collection_work_order(work_order_ref),
    sample_ref uuid NOT NULL REFERENCES cross_industry_sample(sample_ref) ON DELETE CASCADE,
    -- 派发顺序。与证据侧那张表同义：建档时先看到的先补详情。
    ordinal integer NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (work_order_ref, sample_ref),
    UNIQUE (work_order_ref, ordinal),
    CHECK (ordinal > 0)
);

-- 「这一篇是不是已经排进某张在途工单了」是挑选下一批时的主要读法。
CREATE INDEX collection_work_order_cross_industry_target_sample_idx
    ON collection_work_order_cross_industry_target (sample_ref, created_at DESC);

COMMENT ON TABLE collection_work_order_cross_industry_target IS
  '一张详情补采工单覆盖哪几篇跨行业样本；与证据侧的 material_target 同构而不同库';
