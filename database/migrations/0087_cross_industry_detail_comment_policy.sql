-- 跨行业详情补采的作用域表此前少了「这一单被授权读到什么」那两列。
--
-- `0074` 把这张表造成与证据侧 `collection_work_order_material_target` 同构的样子，但只留了
-- 身份与顺序（work_order_ref / sample_ref / ordinal），策略列一个都没有。于是读取点
-- （`work_order_lease.rs::load_cross_industry_targets`）替冻结的作用域做了决定：写死
-- `comment_limit=0, reply_expand_limit=0`——只展开详情。
--
-- 这个分工是错的。**冻结的作用域表才是执行权威**：一次采集能读到什么，必须写在工单上，
-- 而不是由每个读取点临场决定。关键词的单篇详情采集两侧要一致（ADR-0002 的固定详情窗口：
-- 一次标准笔记详情读，至多 30 条一级评论与 2 层回复，`comment_limit=0` 明确表示只读详情），
-- 所以这两列必须真的存在，与证据侧同名、同义、同样边界。
--
-- **存量行保持它们当时被授权的语义**：默认 0（只读详情），不回填。把一条已经发出去的
-- 作用域就地改成 30，等于追认一次从未被授权的评论读取。新建工单由
-- `write_cross_industry_targets` 按当时的决定显式写入。
--
-- 跨行业这一侧不加媒体列：详情补采不下载媒体，也没有对应的产品决定。

ALTER TABLE collection_work_order_cross_industry_target
    ADD COLUMN comment_limit integer NOT NULL DEFAULT 0,
    ADD COLUMN reply_expand_limit integer NOT NULL DEFAULT 0,
    ADD CONSTRAINT collection_work_order_cross_industry_target_comment_limit_check
        CHECK (comment_limit BETWEEN 0 AND 30),
    ADD CONSTRAINT collection_work_order_cross_industry_target_reply_requires_comment_check
        CHECK (comment_limit > 0 OR reply_expand_limit = 0);

COMMENT ON COLUMN collection_work_order_cross_industry_target.comment_limit IS
    '0 means detail-only; 1..30 authorizes that many top-level comments for this exact cross-industry sample.';

COMMENT ON COLUMN collection_work_order_cross_industry_target.reply_expand_limit IS
    'How many reply levels may be expanded for the authorized comments; 0 when comments are not authorized.';
