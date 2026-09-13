# TARGET-INSPECTOR-PERFORMANCE-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-13
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

## 2026-09-13 · 表现页参考复刻回执

- 来源：Mog 提供的检查器侧边栏静态参考稿只提供信息层级、留白和视觉密度；其中目标、日期、趋势和数值不进入运行页。运行页继续消费 creator target-scoped lifecycle projection。
- 表达：creator 的「作品｜表现」改为时间／指标控制、三项覆盖读数、发布密度与同桶指标中位数趋势、可核验摘要和最近四条作品证据；单篇散点分布保留在渐进展开中。Drawer header 只显示身份、持续观察配置、来源链接与关闭，不把 scheduler 或 Attempt 伪装成档案身份。
- 反例：没有合格 lifecycle point 时只显示不可成图理由与实际排除计数，不渲染中位数、密度或巡查新增的零值摘要。`dismissed`、`paused` 与从未启用的持续观察配置在 header 分开表达，不能由布尔开关互相推导。
- LIDS：不新增 Token、CMP、全局壳层或领域数据；页面 CSS 只使用既有 `--lgi-*` Token，功能文字不低于 11px，窄趋势列中的图例可折行而不横向溢出。

## 2026-09-13 · 趋势与逐篇分布同级复核（已实施，source preview 已验证）

- 来源：Mog 对已发布表现页的直接反馈。现状把逐篇散点降为折叠的渐进披露，未形成“趋势 + 分布”两种同级阅读逻辑；时间趋势折线也缺少用于表达连续性的受控色阶。
- 范围：同一 creator lifecycle projection 同时提供时间桶趋势和逐篇散点分布；两图保持相同的时间窗、指标、as-of 和 qualified / KNOWN point 集。逐篇散点继续是到具体作品证据的链接，不能由趋势桶替代。
- 表达：不新增颜色 Token 或页面主题。趋势 SVG 在定义内使用既有 LIDS signal/ink/canvas 颜色构成由低饱和至 signal 的语义渐变；渐变只属于数据墨线，不作为背景、文字、状态或营销装饰。散点用既有归属和巡查新增三通道表达，并在窄宽度改为纵向堆叠而非消失或横向滚动。
- 非目标：不新增指标、算法、跨创作者比较、主题分类、预测、运行态或真实数据写入；不改变列表、巡查或外部链接权限。

## 验收

详见 `docs/design/acceptance/target-inspector-performance-001-acceptance.md`。本变更同步更新 LIDS migration log 和 2026-09 progress；未部署页面与共享数据库均不能作为已验收事实。
