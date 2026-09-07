-- CORPUS-CROSS-INDUSTRY-001 · 观察领域与跨行业样本
--
-- 系统此前只观察 ADHD 一个领域。本迁移把「领域」提升为一等对象：语料页的每个子页都在
-- 某一个当前观察领域下工作，换领域就是换观察对象，页面结构不变。
--
-- **采集链路完全复用**（Mog 2026-09-07 决定）。外部领域的博主与关键词也是普通的观察
-- 目标，走同一套 目标 → 准入 → 工单 → 租约 → 任务 → Package 的路；插件不需要知道
-- 领域的存在。领域只在**落库那一刻**决定材料去哪张表。
--
-- 跨行业内容是**参照物，不是证据**。它回答「别人怎么写标题、怎么做封面、怎么搭结构」，
-- 不参与任何关于本领域的判断。这个边界由数据库自己守住，见下方复合外键——不靠任何
-- 应用层的 `WHERE` 条件，因为那与「一个接口加参数」同构：漏写一次即破，且破得无声。

CREATE TABLE observation_domain (
    domain_ref uuid PRIMARY KEY,
    -- 领域就是该行业最大且核心的那个一级关键词，本身可以直接拿去搜索。
    -- 不是抽象概念：「ADHD」「考研自习」可以搜，「注意力经济」不行。
    name text NOT NULL,
    -- 只服务切换器展示与下方复合外键的组成，**本身不承担隔离**。
    is_own_domain boolean NOT NULL,
    status text NOT NULL DEFAULT 'active',
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (length(name) > 0),
    CHECK (status IN ('active', 'paused')),
    -- 供 cross_industry_sample 的复合外键引用。
    UNIQUE (domain_ref, is_own_domain)
);

CREATE UNIQUE INDEX observation_domain_name_idx ON observation_domain (name);
-- 本领域在全系统里只能有一个。
CREATE UNIQUE INDEX observation_domain_own_idx
    ON observation_domain (is_own_domain) WHERE is_own_domain;

INSERT INTO observation_domain (domain_ref, name, is_own_domain) VALUES
    ('00000000-0000-4000-8000-000000000001', 'ADHD', true),
    ('00000000-0000-4000-8000-000000000002', '考研自习', false),
    ('00000000-0000-4000-8000-000000000003', '自闭症干预', false);

-- 观察目标归属到一个领域。
--
-- 既有目标全部属于本领域：它们是在只有 ADHD 的时候建立的，把它们标成别的会改写历史。
-- 列可空 + 读取时回落到本领域，比 NOT NULL 更诚实——「没标过」和「标了本领域」是两件事，
-- 只是当前处置相同。
ALTER TABLE collection_observation_target
    ADD COLUMN domain_ref uuid REFERENCES observation_domain(domain_ref);

UPDATE collection_observation_target
   SET domain_ref = '00000000-0000-4000-8000-000000000001'
 WHERE domain_ref IS NULL;

CREATE INDEX collection_observation_target_domain_idx
    ON collection_observation_target (domain_ref, lifecycle_state);

-- 跨行业样本：本领域的行**物理上写不进来**。
--
-- 写入一条 ADHD 的样本时：CHECK 把 is_own_domain 锁死为 false，复合外键随即去
-- observation_domain 里找 (ADHD, false)——而 ADHD 那行是 (ADHD, true)，无匹配，
-- 数据库直接拒绝。不需要触发器，不需要应用层记得加过滤条件。
--
-- 它不进 linggan_material_content：那是证据侧的作品身份表，两边共用一张表就等于把
-- 参照物混进了证据。同一套采集链路把材料带回来，落库时按目标所属领域分流到这里。
CREATE TABLE cross_industry_sample (
    sample_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL,
    is_own_domain boolean NOT NULL DEFAULT false,
    -- 这条样本是哪个观察目标带回来的。采集链路与本领域完全相同，差别只在落库去向。
    target_ref uuid REFERENCES collection_observation_target(target_ref),
    platform text NOT NULL,
    content_external_id text NOT NULL,
    title text,
    author_external_id text,
    author_name text,
    cover_local_asset_path text,
    like_count bigint,
    collect_count bigint,
    comment_count bigint,
    published_at timestamptz,
    -- 采样口径。**只有关键词来源才有**：博主监控带回来的样本没有排序口径可言，
    -- 给它编一个会让后续复核基于一个不存在的事实。
    keyword text,
    sort_order text,
    scroll_rounds integer,
    -- 「实际取得数」不可省略：采不满是已知会发生的情况，只记目标数会让复核的基数是错的。
    requested_count integer,
    actual_count integer,
    first_seen_at timestamptz NOT NULL DEFAULT scope_001_now(),
    last_observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (is_own_domain = false),
    CHECK (platform IN ('xhs', 'douyin')),
    CHECK (sort_order IS NULL OR sort_order IN ('most_liked', 'most_collected', 'most_commented', 'latest')),
    CHECK (length(content_external_id) > 0),
    CHECK (keyword IS NULL OR length(keyword) > 0),
    CHECK (scroll_rounds IS NULL OR scroll_rounds >= 0),
    CHECK (requested_count IS NULL OR requested_count >= 0),
    CHECK (actual_count IS NULL OR actual_count >= 0),
    -- 采样口径要么整套都有（关键词来源），要么整套都没有（博主来源）。半套是无法解释的。
    CHECK (
        (keyword IS NULL AND sort_order IS NULL AND scroll_rounds IS NULL
             AND requested_count IS NULL AND actual_count IS NULL)
        OR (keyword IS NOT NULL AND sort_order IS NOT NULL)
    ),
    FOREIGN KEY (domain_ref, is_own_domain)
        REFERENCES observation_domain (domain_ref, is_own_domain),
    -- 一个领域下的同一篇作品只有一行；重复观察更新它，不新增。
    UNIQUE (domain_ref, platform, content_external_id)
);

CREATE INDEX cross_industry_sample_domain_idx
    ON cross_industry_sample (domain_ref, last_observed_at DESC);
CREATE INDEX cross_industry_sample_target_idx
    ON cross_industry_sample (target_ref);
CREATE INDEX cross_industry_sample_keyword_idx
    ON cross_industry_sample (domain_ref, keyword, sort_order)
    WHERE keyword IS NOT NULL;

-- 随手记。第一版不做自动归纳，但没有出口的积累和一次性的区别只是失效得慢一点，
-- 所以搜索/筛选/导出三个能力从第一版就要有——全文检索索引在此备好。
CREATE TABLE cross_industry_note (
    note_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL REFERENCES cross_industry_sample(sample_ref) ON DELETE CASCADE,
    body text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (length(body) > 0)
);

CREATE INDEX cross_industry_note_sample_idx ON cross_industry_note (sample_ref);
CREATE INDEX cross_industry_note_body_idx
    ON cross_industry_note USING gin (to_tsvector('simple', body));

-- 关键词的生命周期。没有终点的关键词会长期把预算喂给已经榨干的词。
-- 退役由人决定、系统只提示：季节性关键词（如考试季前后的「考研」）会被自动退役误杀。
CREATE TABLE cross_industry_keyword (
    keyword_ref uuid PRIMARY KEY,
    domain_ref uuid NOT NULL REFERENCES observation_domain(domain_ref),
    keyword text NOT NULL,
    status text NOT NULL DEFAULT 'active',
    retired_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (length(keyword) > 0),
    CHECK (status IN ('active', 'retired')),
    CHECK ((status = 'retired') = (retired_at IS NOT NULL)),
    UNIQUE (domain_ref, keyword)
);

-- 每轮采集的新增率。连续两轮低于 20% 时系统提示退役，但不自动执行。
CREATE TABLE cross_industry_keyword_round (
    round_ref uuid PRIMARY KEY,
    keyword_ref uuid NOT NULL REFERENCES cross_industry_keyword(keyword_ref) ON DELETE CASCADE,
    sort_order text NOT NULL,
    fresh_count integer NOT NULL,
    total_count integer NOT NULL,
    ran_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (sort_order IN ('most_liked', 'most_collected', 'most_commented', 'latest')),
    CHECK (fresh_count >= 0 AND total_count >= 0 AND fresh_count <= total_count)
);

CREATE INDEX cross_industry_keyword_round_idx
    ON cross_industry_keyword_round (keyword_ref, ran_at DESC);
