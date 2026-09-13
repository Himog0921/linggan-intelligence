-- 跨行业样本的「详情已取得」是一条真实的材料事实，不再靠运行任务的状态推断。
--
-- 本领域侧一直有这条事实：`linggan_material_content_detail` 一行就代表「这篇的详情真的
-- 进来了」，读取侧只需 `EXISTS`。跨行业侧没有——`insert_detail` 只把标题、作者、封面和
-- 互动数 upsert 回样本行，**正文直接丢掉**，没有任何一行说明详情到过手。
--
-- 于是四处读取被迫从运行任务反推：`task_spec->'capabilitiesRequested'->>0='content_detail'`
-- 且 `task_spec #>> '{target,contentExternalId}'` 对上样本的外部 ID，再看任务状态。这条推断
-- 有三处必错：
--
-- 1. **任务跑完不等于材料进来。** 包身份与任务声明对不上时记录被判 `quarantined`、落库整段
--    跳过，而租约任务仍被标成 `completed`。2026-09-10 的 120 条整包隔离正是这个形状。
--    读取侧只好再补一条「这个包里没有任何隔离记录」的子查询——用运行痕迹给运行痕迹打补丁。
-- 2. **`blocked`／`unavailable` 被当成已取得**（补详情那条判据里）。那是「试过没成功」，
--    与「取到了」相反；结果是失败一次的笔记永久从待补清单里消失。
-- 3. **按 `target_ref` 划范围**，而样本行上的 `target_ref` 只在第一次插入时写定。同一篇被
--    另一个关键词先看到，这个目标就永远数不到它、也永远不会给它补详情。
--
-- 这一支不去修那条推断，而是把它要推的事实真的记下来。
--
-- **形状与本领域侧对齐**：一次详情落库追加一行，身份是 `(package_ref, record_ordinal)`，
-- 与记录处置表复合外键，把这一行钉在一条真实记录上。落库只对**未被隔离**的记录写
-- （`insert_detail` 只遍历 `accepted_ordinals`），所以整包被隔离时这里一行都不会有——
-- 这正是「任务 completed 但材料没进来」那种情况需要的答案，读取侧不必再自己排除隔离。
-- 「详情已取得」= 这篇样本有过这样的一行。

CREATE TABLE cross_industry_sample_detail (
    detail_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL,
    domain_ref uuid NOT NULL,
    package_ref uuid NOT NULL,
    record_ordinal integer NOT NULL,
    -- 详情页独有的东西。**正文是详情这一轮存在的全部理由**——此前它被丢掉，于是
    -- 「补详情」除了刷新一遍互动数之外什么也没留下。
    body_text text,
    body_state text NOT NULL CHECK (body_state IN ('KNOWN', 'UNKNOWN')),
    -- 平台给的发布时间原样保存，不解析成精确时刻。跨行业样本是参照物，没有任何判断
    -- 需要它精确到秒；而把「3天前」压成一个绝对时间，是给平台没给过的事实背书。
    published_at_source_text text,
    published_at_source_text_state text NOT NULL
        CHECK (published_at_source_text_state IN ('KNOWN', 'UNKNOWN')),
    observed_at text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT scope_001_now(),
    UNIQUE (package_ref, record_ordinal),
    -- **不写 `ON DELETE CASCADE`。** 这张表是追加式的（下面那个触发器对 DELETE 无条件
    -- 抛异常），级联删除照样要经过子行的 `BEFORE DELETE`——于是父行的删除连同整个事务
    -- 一起回滚，报出来的却是一句无关的「append-only」。CASCADE 在这里是一句做不到的承诺。
    -- 去掉它，删样本会被外键如实挡住，错误信息指向真正的阻挡者。
    --
    -- 挂账：`0073_cross_industry_sample_observation` 也是「追加式 + CASCADE」这个组合，
    -- 同样潜伏着这个矛盾。目前没有任何代码路径删除 `cross_industry_sample`（删目标由
    -- `collection_target.rs` 的受保护事实计数提前拦住），所以它没有被激活；迁移是追加式的，
    -- 改它要另起一支，不并进本支。
    FOREIGN KEY (sample_ref, domain_ref)
        REFERENCES cross_industry_sample(sample_ref, domain_ref),
    FOREIGN KEY (package_ref, record_ordinal)
        REFERENCES linggan_runtime_record_disposition(package_ref, record_ordinal),
    CHECK ((body_state = 'KNOWN') = (body_text IS NOT NULL)),
    CHECK ((published_at_source_text_state = 'KNOWN') = (published_at_source_text IS NOT NULL))
);

CREATE INDEX cross_industry_sample_detail_sample_idx
    ON cross_industry_sample_detail (sample_ref, created_at DESC);

CREATE TRIGGER cross_industry_sample_detail_is_append_only
    BEFORE UPDATE OR DELETE ON cross_industry_sample_detail
    FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

COMMENT ON TABLE cross_industry_sample_detail IS
  '跨行业样本的详情落库事实。一次详情接纳追加一行；「详情已取得」只看这张表有没有行，不看运行任务的状态';

-- **不回填。** 这是证据性事实表，回填等于替过去的采集作证——而恰恰是那条推断不可信才
-- 有本支。更实在的理由：此前那一轮**根本没把正文存下来**，库里不存在可回填的内容。
-- 后果如实说明：已采过详情的跨行业样本会重新显示为「待补详情」，重采时正文第一次真的
-- 会被存下来。
