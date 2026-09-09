-- CONTENT-AUTHOR-001 · 作品的作者归属不再依赖「我还在不在观察他」
--
-- 2026-09-09 用户问了一个对的问题：我在某个阶段决定不再观察这个博主，凭什么让语料库
-- 否认这些作品本来就存在的作者关系？
--
-- 不该。「这篇笔记是这个博主发的」是采集时从页面上读到的**世界事实**；「我要盯着这个人」
-- 是我的一个决定。两者被混在了一起——语料库按博主分组走的是
-- `collection_work_order.target_ref` 这条控制面的链，于是删掉观察目标就等于抹掉作者。
--
-- # 事实其实一直都在
--
-- 2026-09-09 实测：278 篇作品里 256 篇的详情记录自带 `author_external_id`，归属就写在作品
-- 自己身上，只是没有人从这里读。剩下 22 篇只有主页发现、还没取到详情——但那次扫描本身
-- 就是「扫某个博主的主页」，作者写在任务规格里。
--
-- 发现记录上的 `creator_display_name` 全库 781 条都是 UNKNOWN：插件只在详情里回填作者，
-- 主页发现不带。所以这里不用它。
--
-- # 为什么是视图而不是新表
--
-- 归属是从既有事实**推得**的，不是一个新观察。做成表就要考虑回填、要考虑写入时机、要考虑
-- 它和事实不一致时以谁为准——而这三个问题都是自找的。视图没有存储，永远与事实一致，
-- 也不需要一次性回填。
--
-- 它依赖的三张表（content_detail、capture_package、runtime_task）全部是 append-only，
-- **删除观察目标动不到它们**。这正是这次要的性质。

CREATE VIEW linggan_material_content_author AS
SELECT DISTINCT ON (content.public_ref)
       content.public_ref AS content_public_ref,
       content.platform,
       attribution.author_external_id,
       -- 来源要看得见：详情自带与「从主页扫描推得」不是同一种确信程度。
       attribution.attribution_source,
       attribution.observed_at
FROM linggan_material_content content
JOIN LATERAL (
    -- 一、详情记录自带的作者。这是插件在作品页上读到的，最直接。
    SELECT detail.author_external_id,
           'content_detail'::text AS attribution_source,
           detail.created_at AS observed_at
    FROM linggan_material_content_detail detail
    WHERE detail.content_public_ref = content.public_ref
      AND NULLIF(btrim(detail.author_external_id), '') IS NOT NULL
    UNION ALL
    -- 二、只有主页发现的：那一次扫描的对象就是某个博主的主页，作者写在任务规格里。
    -- 这条推理只对 `profile_discovery` 成立——关键词搜索面上的作品不属于任何一个博主主页。
    SELECT task.task_spec #>> '{target,authorExternalId}',
           'profile_discovery'::text,
           package.accepted_at
    FROM linggan_material_discovery_finding finding
    JOIN linggan_runtime_capture_package package
      ON package.package_ref = finding.package_ref
    JOIN linggan_runtime_task task ON task.task_id = package.task_id
    WHERE finding.content_public_ref = content.public_ref
      AND package.package_kind = 'profile_discovery'
      AND NULLIF(btrim(task.task_spec #>> '{target,authorExternalId}'), '') IS NOT NULL
) attribution ON true
-- 详情压过推得的；同一来源里取最近一次观察。
ORDER BY content.public_ref,
         (attribution.attribution_source = 'content_detail') DESC,
         attribution.observed_at DESC;

COMMENT ON VIEW linggan_material_content_author IS
    '作品与作者的归属，全部推自 append-only 事实；删除观察目标不影响它。';
