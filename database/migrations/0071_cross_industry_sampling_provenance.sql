-- 跨行业样本要说得清「这条是怎么来的」。
--
-- 两件事在此之前都做不到：
--
-- 1. **采样口径始终为空**。表里早就有 keyword / sort_order / scroll_rounds /
--    requested_count / actual_count 五列，但接纳侧从未写过——插件的回执只回显身份，
--    不回显下发给它的口径，而接纳侧当时只看回执。于是同一个词按「最多点赞」和按
--    「综合」采回来的笔记混在一张表里，分不出哪条属于哪个口径，也就拼不出这个词的
--    面貌。口径的真实来源是**工单冻结的那张任务单**，本迁移不改表结构，只是让
--    `0044` 的「要么整套要么没有」这条 CHECK 终于有机会成立。
--
-- 2. **没有来源链接**。表里只有 content_external_id。小红书的作品链接必须携带来源页
--    返回的短期 xsec_token 才能打开，裸 /explore/{id} 会被平台拒绝（2026-08 真实
--    canary 已证明，见 docs/progress/2026-08.md）。没有链接就既看不了原作，也无法
--    接着采详情。
--
-- source_url 是**短期签名链接**，不是永久地址：它会随 token 过期而失效，配合
-- last_observed_at 才能判断还值不值得一试。这里如实保存观察当时拿到的那一条，不做
-- 任何拼接或推断——拼一个自己造的链接等于制造一个从未被平台返回过的事实。

ALTER TABLE cross_industry_sample
  ADD COLUMN source_url text;

ALTER TABLE cross_industry_sample
  ADD CONSTRAINT cross_industry_sample_source_url_check
  CHECK (source_url IS NULL OR length(source_url) > 0);

-- sort_order 此前只允许四种排序，独缺「综合」。而综合是小红书搜索的默认排序，也是
-- 观察一个关键词面貌时最基本的那一维：不含它，一个词就只剩下几个极端切片。
-- 服务端的排序词表用 comprehensive 表示综合（插件侧再映射到平台的 general），
-- 这里与服务端保持同一套用词，不在数据库里另造一个同义词。
DO $$
DECLARE
  sort_order_constraint name;
BEGIN
  SELECT conname INTO sort_order_constraint
  FROM pg_constraint
  WHERE conrelid = 'cross_industry_sample'::regclass
    AND contype = 'c'
    AND pg_get_constraintdef(oid) LIKE '%sort_order%'
    AND pg_get_constraintdef(oid) LIKE '%most_liked%';
  IF sort_order_constraint IS NULL THEN
    RAISE EXCEPTION 'expected cross_industry_sample sort_order check constraint is missing';
  END IF;
  EXECUTE format('ALTER TABLE cross_industry_sample DROP CONSTRAINT %I', sort_order_constraint);
END $$;

ALTER TABLE cross_industry_sample
  ADD CONSTRAINT cross_industry_sample_sort_order_check
  CHECK (sort_order IS NULL OR sort_order = ANY (ARRAY[
    'comprehensive'::text,
    'latest'::text,
    'most_liked'::text,
    'most_commented'::text,
    'most_collected'::text
  ]));

-- requested_count 的来源不止一个，落在同一列上容易被读窄：设了「取赞前 N」时它是 N，
-- 没设时退回这一单的篇数上限。列的语义始终是「这一轮打算要几篇」，不是「取赞前 N 的
-- 那个 N」——读到 sort_order='comprehensive' 配 requested_count=200 时不要当成前者。
COMMENT ON COLUMN cross_industry_sample.requested_count IS
  '这一轮打算要几篇：优先取任务单的 topByLikes，缺失时为该单的 maximumQuota';

COMMENT ON COLUMN cross_industry_sample.source_url IS
  '平台返回的短期签名链接（xsec_token 会过期），只保存实际返回的那条，不由作品 ID 拼接';
