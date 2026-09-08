-- KEYWORD-SAMPLING-POLICY-001 · 关键词观察的采样口径
--
-- 现有的关键词目标只有「词 + 排序」两项事实：`ADHD（综合排序）` 就是全部信息了。
-- 采回来 20 篇，无法回答「这 20 篇是怎么来的」——翻了几次页？是从多少篇里挑的？
-- 只要一周内的还是不限？**没有口径的样本无法复核**，而复核不了的数字不该拿去比较。
--
-- 口径落在规则版本上，不落在目标上：它决定每一轮巡检怎么采，改了要留下新版本，
-- 与 mode / 间隔 / 时间窗是同一类东西。目标只承载身份。
--
-- 这三项对本领域与跨行业**是同一套能力**。跨行业规格要求它，本领域同样需要——
-- 现在那条 ADHD 关键词采回来的东西一样说不清出处。

ALTER TABLE collection_monitor_rule_revision
    -- 下拉刷新几次。**按次数控制，不按条数控制**：页面每次加载出多少条不由我们决定，
    -- 只有「拉了几次」是能说准的事实。
    ADD COLUMN scroll_rounds integer,
    -- 从加载出来的内容里取点赞最高的几篇。
    ADD COLUMN top_by_likes integer,
    -- 只要发布在这些天以内的。为空表示不限。
    ADD COLUMN published_within_days integer;

ALTER TABLE collection_monitor_rule_revision
    ADD CONSTRAINT collection_monitor_rule_scroll_rounds_bounded
        CHECK (scroll_rounds IS NULL OR scroll_rounds BETWEEN 0 AND 20),
    ADD CONSTRAINT collection_monitor_rule_top_by_likes_bounded
        CHECK (top_by_likes IS NULL OR top_by_likes BETWEEN 1 AND 200),
    ADD CONSTRAINT collection_monitor_rule_published_within_bounded
        CHECK (published_within_days IS NULL OR published_within_days BETWEEN 1 AND 365),
    -- 口径只属于关键词搜索面。创作者主页没有「排序」也没有「取前 N」可言，给它编一个
    -- 会让复核基于一个不存在的事实——与跨行业样本表「要么整套要么没有」是同一条规矩。
    ADD CONSTRAINT collection_monitor_rule_sampling_belongs_to_search
        CHECK (
            surface_key = 'keyword_search'
            OR (scroll_rounds IS NULL AND top_by_likes IS NULL AND published_within_days IS NULL)
        ),
    -- 下拉与取前 N 是一对：拉了页却不说取几篇，或说取 20 篇却没说从哪里取，
    -- 都无法还原这一轮采样。要么都给，要么都不给。
    ADD CONSTRAINT collection_monitor_rule_sampling_is_whole
        CHECK ((scroll_rounds IS NULL) = (top_by_likes IS NULL));

-- 既有关键词规则补上口径。
--
-- 这里可以用 UPDATE：规则版本表不是 append-only 事实表（它没有那个触发器），而且
-- **既有值本来就是「未记录」而非「记录为无」**——补上的是当时实际在跑的口径，不是
-- 改写历史。写死的这组数字来自跨行业规格的统一采集动作：下拉 3 次、取点赞前 20。
-- 发布时间不限：本领域那条 ADHD 关键词一直在采全部时间的内容，写成一周会是假话。
UPDATE collection_monitor_rule_revision
SET scroll_rounds = 3,
    top_by_likes = 20
WHERE surface_key = 'keyword_search'
  AND scroll_rounds IS NULL;
