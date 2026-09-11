-- 一篇笔记可以同时出现在同一个词的多个排序榜上。
--
-- 「同时上了综合榜和点赞榜」本身就是识别真爆款的信号——恰恰是关键词建档想要的东西。
-- 但样本行只有一组口径列，第二个维度再看到同一篇时会把第一个维度的记录覆盖掉，于是
-- 只剩「最近一次是从哪个维度看到它的」。
--
-- 这个信息**不记就补不回来**：实测同词同排序隔两天重合率只有 5%~20%，重采换回来的是
-- 另一批笔记，不是同一批。所以按维度逐条留痕，一篇一榜一行。
--
-- 它不复制样本事实：标题、作者、互动数、链接仍然只在 cross_industry_sample 上有一份。
-- 这里只回答「这篇在哪个榜上出现过、第一次和最近一次是什么时候」。

CREATE TABLE cross_industry_sample_lane (
    sample_ref uuid NOT NULL,
    -- 领域随样本一起绑死。`sample_ref` 本身已经唯一确定领域，但「按关键词查这张表」是
    -- 主要读法，而两个领域完全可能用同一个词；只按 keyword 建索引，读的人不 JOIN 回
    -- 样本表就会把两个领域的榜混在一起。`0044` 给证据侧补对称约束时用的就是这一手：
    -- 让数据库拒绝，而不是指望每个读取点都记得加条件。
    domain_ref uuid NOT NULL,
    -- 与 cross_industry_sample.sort_order 同一套取值，也与服务端的排序词表一致。
    sort_order text NOT NULL,
    -- 关键词随排序一起记：同一个领域下可以有多个词，各自的榜是分开的。
    keyword text NOT NULL,
    first_seen_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    PRIMARY KEY (sample_ref, keyword, sort_order),
    -- 复合外键：这一行的领域必须与它所属样本的领域一致，写错会被数据库拒绝。
    FOREIGN KEY (sample_ref, domain_ref)
        REFERENCES cross_industry_sample(sample_ref, domain_ref) ON DELETE CASCADE,
    CHECK (length(btrim(keyword)) > 0),
    CHECK (sort_order = ANY (ARRAY[
        'comprehensive'::text,
        'latest'::text,
        'most_liked'::text,
        'most_commented'::text,
        'most_collected'::text
    ]))
);

-- 「这个领域的这个词底下哪些笔记上过榜、各上过几个榜」是主要读法，领域必须领头。
CREATE INDEX cross_industry_sample_lane_keyword_idx
    ON cross_industry_sample_lane (domain_ref, keyword, sort_order, last_observed_at DESC);

COMMENT ON TABLE cross_industry_sample_lane IS
  '一篇跨行业样本在某个关键词的某个排序榜上出现过的留痕；上过几个榜＝这里有几行';
