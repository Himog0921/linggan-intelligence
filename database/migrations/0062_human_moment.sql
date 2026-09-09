-- HUMAN-MOMENT-001 · 给人看的时间只有一种写法
--
-- 2026-09-09 用户反馈：`2026-08-31T02:20:39.916Z`、`2026-09-04 06:08:22.03442+08` 这类时间
-- 在语料库和采集模块里到处都是。他要知道的只是哪年哪月哪日、几点几分；毫秒、时区偏移与
-- 那个 `T` 对他没有意义，只是占位置并且要人在脑子里解析一遍。
--
-- 此前全项目有五种写法同时存在：`YYYY-MM-DD HH24:MI`、`MM-DD HH24:MI`（缺年份）、
-- 两种 ISO、以及直接 `::text` 把 timestamptz 原样倒出来。同一个页面上并排出现三种格式，
-- 读的人得先判断这是哪一种。
--
-- # 为什么放在数据库里
--
-- 这个项目没有引入任何日期库，时间在 Rust 侧一律是字符串。要在 Rust 里格式化就得自己
-- 解析偏移量并做时区换算——那是把一件数据库本来就会做、而且做得对的事，用手写代码重做
-- 一遍。放在这里还有一个好处：**它只有一处定义**，改格式改一个地方。
--
-- 会话时区固定为 Asia/Shanghai，因此这里出来的就是北京时间。
--
-- # 它不适用于机器合同
--
-- `/health`、Producer 契约、凭据回执里的时间仍然是 ISO：那些是给机器读的，精度和时区
-- 偏移都有意义。这个函数只用在人会看到的地方。

CREATE FUNCTION linggan_human_moment(value timestamptz) RETURNS text
    LANGUAGE sql
    -- STABLE 而非 IMMUTABLE：结果取决于会话时区。声明成 IMMUTABLE 会让它可以被索引固化，
    -- 而那份固化值在另一个时区的会话里就是错的。
    STABLE
    AS $$ SELECT to_char($1, 'YYYY-MM-DD HH24:MI') $$;

-- 采集包里的时间是插件写下的 ISO 字符串，存成 text。这个重载让读取侧不必在每一处
-- 自己写一遍 `::timestamptz` 转换。
CREATE FUNCTION linggan_human_moment(value text) RETURNS text
    LANGUAGE plpgsql
    STABLE
    AS $$
    BEGIN
        RETURN to_char(value::timestamptz, 'YYYY-MM-DD HH24:MI');
    EXCEPTION WHEN others THEN
        -- 认不出来就原样返回。编一个时间出来比显示一个看不懂的字符串更糟：
        -- 前者会被当成事实。
        RETURN value;
    END;
    $$;

COMMENT ON FUNCTION linggan_human_moment(timestamptz) IS
    '人可读时间的唯一格式：YYYY-MM-DD HH:MM，按会话时区（Asia/Shanghai）。机器合同仍用 ISO。';
