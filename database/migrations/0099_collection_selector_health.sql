-- COLLECTION-UPGRADE-001 · 插件上报的页面结构自检：受限快照，一处列
--
-- 页面结构变了，插件自己最先知道：每次采集前它都会查一遍关键选择器在不在，并且知道这些
-- 选择器上一次人工重验是哪一天。但这些事实此前**只留在浏览器里**——服务端能看到的只有
-- 「这张工单失败了」。于是「页面结构缺了一类信号」和「这台机器网络断了」在页面上长得
-- 一模一样（E10：有本地 preflight，缺服务端诊断）。
--
-- 这一列存的是那份自检的**受限快照**，不是页面副本：
--
--   * 不带选择器串、不带 DOM 文本、不带任何页面地址。插件在出门前就只保留受限形状，
--     服务端按同一套形状**再收一次**（`crates/evidence/src/selector_health.rs`），
--     收不下就整条不落库——这一列只存被收下的东西。
--   * 它说的是「这台机器的插件最近一次看页面看到了什么」，不证明页面内容、不证明采集
--     能不能成，也不改变任何准入或额度判定：那是 Attempt / Package / receipt 的事。
--
-- 两个时刻必须分开，因为它们是两件事：
--
--   * `checkedAt` 是**这次**看的时刻（每次检查都变）；
--   * `verifiedAt` 是这些选择器**上一次人工重验**的日期（可能是几个月前）。
--
-- 把后者写成前者，等于把「刚看了一眼」说成「刚验证过」。所以快照里两个都留。
--
-- 空值的意思是**这台安装没有可用记录**：从来没报到过诊断，或报来的那一份没被收下。
-- 它不是「一切正常」，页面也必须照这个意思显示。
ALTER TABLE plugin_installation
    ADD COLUMN selector_health jsonb;

-- 存储层只保证「是一个对象」，具体形状由运行时校验负责（各字段的闭集与上限写在
-- `selector_health` 模块里，随代码演进）。这里不复制一份字段清单：两份清单迟早会有一份过期。
ALTER TABLE plugin_installation
    ADD CONSTRAINT plugin_installation_selector_health_shape
    CHECK (selector_health IS NULL OR jsonb_typeof(selector_health) = 'object');

COMMENT ON COLUMN plugin_installation.selector_health IS
    'Last accepted selector self-check snapshot reported by this installation: restricted shape (platform, pageType, capability, checkedAt, verifiedAt, checked/stale/missing categories, consecutive failure counts). Never carries selectors, DOM text or page URLs; the page the check ran on is reported, not the page a task wanted. checkedAt is this check and must never be shown as the verification date. NULL means no accepted record from this installation - not "all healthy".';
