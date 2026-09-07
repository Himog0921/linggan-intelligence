-- CORPUS-CROSS-INDUSTRY-001 · 跨行业评论与落库分流的双向隔离
--
-- 设计文档把评论列在详情级采集里（「精确发布时间、正文、媒体、评论」），并且专门
-- 说明「最多评论」这条排序是选题富矿——评论区是用户原话。而 0041 只建了样本表，
-- 里面有评论条数、没有评论内容。跨行业侧因此无评论可读，评论研究页在外部领域下
-- 只能永远空着。本迁移补上那张表。
--
-- 同时补上隔离缺的另一半。0041 只拦住了一个方向：CHECK (is_own_domain = false) 与
-- 复合外键咬合，保证本领域内容写不进跨行业表。反方向没有任何约束——跨行业内容
-- 落进证据表，数据库不会拦。那正是本卡最怕的事（参照物被当成证据参与 ADHD 判断），
-- 却只靠落库那一刻的应用层判断守着，与「一个接口加参数」同构：漏一次即破且无声。
-- 这里给证据侧作品身份表补上对称的一半，两个方向都由结构守住。

-- 1. 跨行业评论。
--
-- 结构对齐证据侧 linggan_material_comment 的诚实度：正文缺失记 UNKNOWN，不折成空串。
-- 跨行业样本大量来自列表面，评论缺失是常态，把「没看到」写成「确实没有」会让后续
-- 的工艺归纳建立在假事实上。
--
-- 不脱敏（Mog 2026-09-01 决定）：自用、看工艺不看隐私、不外传。该判断的成立前提是
-- 这些数据不交给外部 Agent 读、也不对外引用；前提变化时这条要重新裁定。
CREATE TABLE cross_industry_comment (
    comment_ref uuid PRIMARY KEY,
    sample_ref uuid NOT NULL,
    domain_ref uuid NOT NULL,
    is_own_domain boolean NOT NULL DEFAULT false,
    comment_external_id text NOT NULL,
    parent_comment_external_id text,
    is_reply boolean NOT NULL DEFAULT false,
    body_text text,
    body_state text NOT NULL CHECK (body_state IN ('KNOWN', 'UNKNOWN')),
    like_count bigint,
    author_external_id text,
    author_display_name text,
    observed_at timestamptz NOT NULL DEFAULT scope_001_now(),
    CHECK (is_own_domain = false),
    CHECK (length(comment_external_id) > 0),
    -- 正文状态与正文本身必须一致，和证据侧同一条规则。
    CHECK ((body_state = 'KNOWN') = (body_text IS NOT NULL)),
    -- 是不是回复由父评论决定，不许两者各说各话。
    CHECK (is_reply = (parent_comment_external_id IS NOT NULL)),
    CHECK (like_count IS NULL OR like_count >= 0),
    FOREIGN KEY (domain_ref, is_own_domain)
        REFERENCES observation_domain (domain_ref, is_own_domain),
    -- 同一篇样本下的同一条评论只有一行；重复观察更新它，不新增。
    UNIQUE (sample_ref, comment_external_id)
);

-- 评论必须与它所属的样本在同一个领域。只用 sample_ref 单列外键的话，一条「考研自习」
-- 的评论可以挂到「自闭症干预」的样本上而数据库毫不知情。复合键让这件事写不进去。
ALTER TABLE cross_industry_sample
    ADD CONSTRAINT cross_industry_sample_ref_domain_key UNIQUE (sample_ref, domain_ref);

ALTER TABLE cross_industry_comment
    ADD CONSTRAINT cross_industry_comment_sample_fk
        FOREIGN KEY (sample_ref, domain_ref)
        REFERENCES cross_industry_sample (sample_ref, domain_ref)
        ON DELETE CASCADE;

CREATE INDEX cross_industry_comment_domain_idx
    ON cross_industry_comment (domain_ref, observed_at DESC);
CREATE INDEX cross_industry_comment_sample_idx
    ON cross_industry_comment (sample_ref);

-- 封面的来源地址。
--
-- 0041 只给了 cover_local_asset_path，语义是**本地资产路径**。媒体链路接通前，采回来的
-- 封面 URL 无处可放，于是被丢掉——将来接通时还得为已有样本重采一轮。这一列把它留下：
-- 记住「在哪」与「已下载到本地」是两件事，不能拿远端 URL 冒充本地路径，那会让读取侧
-- 以为图片已经在手上。
ALTER TABLE cross_industry_sample
    ADD COLUMN cover_source_url text;

-- 2. 证据侧的对称隔离。
--
-- 既有 107 行作品全部回填为本领域：它们是在只有 ADHD 的时候采进来的，标成别的
-- 会改写历史。这里与 0041 对观察目标的处置不同——那边保持可空 + 读取时回落，因为
-- 「没标过」与「标了本领域」是两件不同的事；而这张表要承担隔离职责，可空的外键在
-- SQL 里根本不触发检查（任一列为 NULL 即放行），隔离就成了摆设。所以此列 NOT NULL。
ALTER TABLE linggan_material_content
    ADD COLUMN domain_ref uuid,
    ADD COLUMN is_own_domain boolean NOT NULL DEFAULT true;

UPDATE linggan_material_content
SET domain_ref = (SELECT domain_ref FROM observation_domain WHERE is_own_domain)
WHERE domain_ref IS NULL;

-- 归属由数据库自己填，不要求每个写入点记得带上。
--
-- 让 domain_ref 成为一个必须由调用方提供的列，等于要求「所有写证据表的地方都记得
-- 填本领域」——那正是本卡反复拒绝的那种约定：漏一次即破。证据表只装本领域作品是
-- **表的性质**，不是某次调用的选择，所以由表自己兜住。
--
-- 隔离并没有因此变松：下面的 CHECK 与复合外键照旧。触发器只负责补上省略的那一半，
-- 显式写入外部领域仍然会被拒绝。
CREATE FUNCTION linggan_material_content_home_domain()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.domain_ref IS NULL THEN
        NEW.domain_ref := (SELECT domain_ref FROM observation_domain WHERE is_own_domain);
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER linggan_material_content_home_domain_default
    BEFORE INSERT ON linggan_material_content
    FOR EACH ROW
    EXECUTE FUNCTION linggan_material_content_home_domain();

ALTER TABLE linggan_material_content
    ALTER COLUMN domain_ref SET NOT NULL,
    -- 证据表只装本领域的作品。跨行业内容有自己的表，共用一张表等于把参照物混进证据。
    ADD CONSTRAINT linggan_material_content_home_domain_only
        CHECK (is_own_domain = true),
    ADD CONSTRAINT linggan_material_content_domain_fk
        FOREIGN KEY (domain_ref, is_own_domain)
        REFERENCES observation_domain (domain_ref, is_own_domain);
