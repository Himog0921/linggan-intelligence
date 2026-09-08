# TARGET-INSPECTOR-PERFORMANCE-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-08
> 适用范围: `/collection/targets` 目标工具栏、creator Inspector 与作品表现
> 事实来源: Mog 当前确认、Issue #158 Claim、TARGET-INSPECTOR-PERFORMANCE-001 计划、PAGE-COLLECTION-001、LIDS v7 与真实 read model
> 冲突时以谁为准: 用户最新确认、真实代码/事实合同与 Issue Claim；本清单不授权新事实、共享运行或外部动作

## 来源与分类

本次为展示、交互和状态语义混合变更，主 Pattern 是 L1 Collection Control 的目标档案变体，Drawer 采用 L2 Split Inspector。已核对 AGENTS、UI execution contract、PAGE-COLLECTION-001、LIDS Token/Primitive/Pattern/Data Truth/Language 与 #158 现有 manifest。既有作品列表和 creator lifecycle read model 可复用；参考截图与旧对话只提供待验证的产品问题，不是运行事实。

## 变更边界

- 影响：目标筛选/新建/批量栏、creator/keyword 目录的可见操作、Drawer header、概览、作品列表/表现、巡查。
- 状态：常用、Known zero、Unknown、queued、running、partial、blocked、无需处理、需人工处理、无可绘点与查询无效。
- LIDS：不新增 Token/主题/CMP；消费唯一 token。文字 tab 用 signal underline；硬边、hard shadow 只落在选中与主动作；连续作品仍是表格。
- 数据：新增一个 target-scoped、single-as-of read model，不新增 schema；表现图只使用 qualified/KNOWN lifecycle points。
- 明确不做：内容分类、主题 × 表现、传统 BI KPI 阵列、通用评论洞察、生产部署、插件或采集行为。

## 状态诚实性

- 排队只写排队；没有 live Attempt 不写执行中。
- 系统自动处理时不给人工按钮；需要人工时最多一个主动作。
- `0` 只来自 Known zero；Unknown 以未知/尚未取得显示。
- 无分类合同时直接说明尚未建立内容分类，不生成主题或运营结论。
- 生命周期图是创作者自己的作品分布，不是跨账号评分或“监控价值”。

## 验收

详见 `docs/design/acceptance/target-inspector-performance-001-acceptance.md`。本变更同步更新 LIDS migration log 和 2026-09 progress；未部署页面与共享数据库均不能作为已验收事实。
