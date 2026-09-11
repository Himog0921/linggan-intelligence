-- 一篇跨行业样本被看到过几次、每次是什么样。
--
-- `cross_industry_sample` 是**一篇一行**：那张表回答「这篇笔记是什么」，靠 upsert 保持
-- 当前最好的一份事实。但它答不了「这篇被看到过几次、热度怎么变」——每次观察都把上一次
-- 覆盖掉了。
--
-- 本行业侧不需要这张表：那边每一轮采集都产生一条新的 discovery_finding，时间序列天然
-- 存在（`linggan_material_engagement_observation` 只是它的一个视图）。跨行业侧走 upsert，
-- 于是必须把每一次观察单独记下来。
--
-- 这里**只记观察当时读到了什么**，不重复样本自身的事实：标题、作者、链接仍然只在
-- `cross_industry_sample` 上有一份。互动数是例外，而且必须是例外——「这篇现在有多少赞」
-- 与「上周五看到它时有多少赞」是两个不同的事实，后者不能由前者推出来。

CREATE TABLE cross_industry_sample_observation (
    observation_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL,
    domain_ref uuid NOT NULL,
    -- 哪一轮采集看到的。同一个包里同一篇只会看到一次。
    package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
    -- 从这个词的这个榜上看到的。没有口径的那一轮（详情面、评论面）不写观察记录——
    -- 它们不是「从某个榜上看到这篇」，而是「按已知作品去取它的详情」。
    keyword text NOT NULL,
    sort_order text NOT NULL,
    -- 观察当时读到的互动数。`NULL` 是「这次没读到」，不是 0——卡片上没有数字与
    -- 数字确实是零，是两件事。
    like_count bigint,
    comment_count bigint,
    collect_count bigint,
    -- 插件在那一轮结果流里发现它时的位次，**原样保存插件报告的 `_discoveryOrder`**：
    -- 从 0 开始计，不加一转成「第几名」。位次本身是信号（同一篇这周在第 3、下周在
    -- 第 40 是有意义的），但把 0 改写成 1 就等于伪造了一个平台没给过的数。
    --
    -- 这不是排序后的名次：按点赞取前 20 时，插件报告的仍是这 20 篇各自在原始结果流里
    -- 的位置（实测取值可到 206），排序后的名次由点赞数自己说明，不必再存一份。
    discovery_order integer,
    observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    FOREIGN KEY (sample_ref, domain_ref)
        REFERENCES cross_industry_sample(sample_ref, domain_ref) ON DELETE CASCADE,
    UNIQUE (sample_ref, package_ref),
    CHECK (length(btrim(keyword)) > 0),
    CHECK (like_count IS NULL OR like_count >= 0),
    CHECK (comment_count IS NULL OR comment_count >= 0),
    CHECK (collect_count IS NULL OR collect_count >= 0),
    CHECK (discovery_order IS NULL OR discovery_order >= 0),
    CHECK (sort_order = ANY (ARRAY[
        'comprehensive'::text,
        'latest'::text,
        'most_liked'::text,
        'most_commented'::text,
        'most_collected'::text
    ]))
);

-- 「这个领域这个词底下，最近看到了什么」与「这一篇的历次观察」是两种主要读法。
CREATE INDEX cross_industry_sample_observation_board_idx
    ON cross_industry_sample_observation (domain_ref, keyword, sort_order, observed_at DESC);
CREATE INDEX cross_industry_sample_observation_sample_idx
    ON cross_industry_sample_observation (sample_ref, observed_at DESC);

CREATE TRIGGER cross_industry_sample_observation_is_append_only
    BEFORE UPDATE OR DELETE ON cross_industry_sample_observation
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE cross_industry_sample_observation IS
  '一篇跨行业样本的每一次被观察：什么时候、从哪个词的哪个榜、当时读到多少赞';

-- `cross_industry_sample_lane`（0072）回答的「这篇上过哪些榜、第一次和最近一次是什么
-- 时候」，是上面这张明细的聚合。**两份都存就是两处真相**：一次 upsert 漏更新，两边就会
-- 各说各话。改成视图，聚合永远跟着明细走。
--
-- 0072 建成表之后至今没有写入过任何一行（那两个写入过的目标已在同日的证据清理中删除），
-- 所以这里直接替换，不涉及数据迁移。
DROP TABLE cross_industry_sample_lane;

CREATE VIEW cross_industry_sample_lane AS
SELECT sample_ref,
       domain_ref,
       keyword,
       sort_order,
       min(observed_at) AS first_seen_at,
       max(observed_at) AS last_observed_at,
       count(*) AS observed_times
FROM cross_industry_sample_observation
GROUP BY sample_ref, domain_ref, keyword, sort_order;

COMMENT ON VIEW cross_industry_sample_lane IS
  '由观察明细聚合出的榜单留痕：这篇在这个词的这个榜上出现过几次、首次与最近一次';
