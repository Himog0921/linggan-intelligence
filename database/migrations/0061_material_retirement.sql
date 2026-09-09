-- MATERIAL-RETIREMENT-001 · 人确认一篇作品已经取不到了
--
-- 2026-09-09 的实情：木可可的作品目录里有 13 篇，其中 3 篇在平台上已被作者删除。系统连续
-- 读取失败后停止了自动重试——这是对的，不该无限重试一个不存在的页面——但它**没有地方
-- 沉淀「这篇没了」这个结论**。于是这个目标永远停在 10/13、永远挂着「档案有问题」，而
-- 「处理异常」按钮点进去只看到一句「存在已知阻塞」，没有任何可做的事。
--
-- 缺的不是重试，是一个**结论**。而这个结论机器给不了：读不到可能是被删了，也可能是这次
-- 网络不好、登录过期、平台限流。只有人能看着页面说「它确实没了」。
--
-- # 为什么留在目录里而不是删掉
--
-- 删掉分母会更好看：13 变 10，档案立刻「完成」。但这个博主当时**确实发过**这 3 篇，
-- 把它们从目录里抹掉是改写历史。留着并标明「已失效」，数字是 10/13 且其中 3 篇已确认
-- 失效——读的人看到的是真相，而不是一个被修饰过的分母。
--
-- # 它不是隔离，也不是失败记录
--
-- 隔离（`quarantined`）说的是「这条记录不可信」，失败（`blocked`）说的是「这次没读到」。
-- 这张表说的是第三件事：「人看过了，它在平台上已经不存在」。三者都保留，互不覆盖。

CREATE TABLE collection_material_retirement (
    retirement_ref uuid PRIMARY KEY,
    -- 失效是**针对某个观察目标的作品目录**说的。同一篇内容若出现在别的目标目录里，
    -- 那是另一次需要各自确认的判断——一个人只为自己看过的那一份负责。
    target_ref uuid NOT NULL REFERENCES collection_observation_target (target_ref),
    content_public_ref uuid NOT NULL REFERENCES linggan_material_content (public_ref),
    reason_code text NOT NULL CHECK (reason_code IN ('page_gone', 'page_unreadable')),
    -- 只有人能下这个结论。写死为 `person` 不是冗余：它让「机器自动判定失效」这条路
    -- 在数据库层就走不通。
    decided_by text NOT NULL CHECK (decided_by = 'person'),
    decided_at timestamptz NOT NULL DEFAULT scope_001_now(),
    note text CHECK (note IS NULL OR length(btrim(note)) > 0)
);

-- 同一个目标下的同一篇作品只确认一次。重复点击是同一个结论，不该变成两条事实。
CREATE UNIQUE INDEX collection_material_retirement_once_idx
    ON collection_material_retirement (target_ref, content_public_ref);

-- 按目标读整份清单是唯一的读法。
CREATE INDEX collection_material_retirement_target_idx
    ON collection_material_retirement (target_ref, decided_at DESC);

-- 已确认失效不是终点：作者可能把作品恢复，之后详情就采到了。读取侧因此让「已有详情」
-- 压过「已失效」——两者同时成立时按已有详情算。这条注释写在这里，是因为下一个读这张表
-- 的人最可能在这里问「那要不要把失效记录删掉」：不要，它记录的是当时那个判断成立过。
COMMENT ON TABLE collection_material_retirement IS
    '人确认某个观察目标的作品目录里某一篇在平台上已不存在；作品保留在目录中，只是不再计入待补齐。';
