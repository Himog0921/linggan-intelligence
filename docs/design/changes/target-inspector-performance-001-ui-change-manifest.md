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

## 2026-09-13 · 同画布趋势／分布切换（源码已实施，未发布）

- 来源：Mog 最新确认的“确定分布语义”与两张静态视觉参考。参考图只提供阅读顺序、视觉密度与信号层级；其中日期、数值和判断不进入运行页。最新确认替代上一节“趋势与逐篇分布同时常驻”的布局结论。
- 范围：`作品｜表现` 在同一分析画布内提供可 URL 恢复的 `life_chart=trend|distribution`，默认 `trend`。两视图严格共享时间窗、当前指标、`as_of` 与 qualified / KNOWN 逐篇作品集；切换不改变作品选择或既有筛选上下文。趋势视图按时间桶呈现发布密度、当前指标中位数、当前窗口中位基准及可复核信号；仅趋势可选按周／按月。分布视图逐篇呈现“发布时间 × 当前指标”，每个点仍是到精确作品的链接，不把评论／点赞率改成纵轴。
- 复核信号：第一阶段只实现“高讨论率（评论／点赞 > 20%）”覆盖层。它是分布图的橙红外圈，而非新的指标或统计结论；最近巡查新增采用不同的轻量标记，两个信号可并存。评论或点赞尚未取得、或点赞为 Known zero 的作品不参与该规则，右侧复核栏单独计入“讨论率未可判”，不得按零或低讨论率处理。
- 右侧阅读辅助：趋势显示当前指标中位数、按所选粒度计算的发布密度、最近巡查新增与“先查证逐篇作品”的边界；分布显示当前纳入作品数、当前指标的典型区间、高讨论率计数及讨论率未可判计数。典型区间只在实现中明确为已纳入作品的第 25–75 百分位后显示，不把它命名为行业常态、预测或作品质量。
- 表达：复刻“一个大画布 + 窄复核栏”的信息关系，不复制参考图的虚构内容。趋势主线回到单一墨色数据墨线，淡色填充只帮助读取同一序列；橙红只标注审查信号、活动 tab 与其图例，不再把渐变当成趋势含义。所有 CSS 色彩、间距、文字与 surface 继续消费既有 LIDS token。
- 数值与页面规格：`PAGE-COLLECTION-001` 已同步裁定本次图例／信号与 P25–P75 阅读辅助，替代早期的归属点填充和“禁用所有分位数”表述。`<10,000` 保留带千分位的精确值；`≥10,000` 可用中文“万”缩写，但趋势轴、基准、当前中位数和典型区间都提供精确值的 SVG title / accessible label 或原生 title，缩写不是唯一事实。趋势／新增环 SVG 描边只使用 LIDS 的 2px 档位。
- 非目标：不新增数据写入、计算型评分、跨创作者比较、主题／内容分类、预测、通用规则配置器、共享运行时部署、插件重载或外部采集。URL 的 `life_chart` / `life_grain` 只属于本地页面阅读状态，不进入 lifecycle API 查询合同。

## 验收

详见 `docs/design/acceptance/target-inspector-performance-001-acceptance.md`。本变更同步更新 `PAGE-COLLECTION-001`、LIDS migration log 和 2026-09 progress；未部署页面与共享数据库均不能作为已验收事实。

## 2026-10-26 · 已确认交互原型正式接入

用户已确认独立原型结构，并授权实现、测试、独立审查后合并及本机3000发布；仍禁止真实采集与模型调用。本节替代旧「概览」及作品默认视图的布局结论，不改变领域数据与命令合同。

工作包 `TARGET-INSPECTOR-UI-20261026`，协调者 root；base `05c24175`，分支 `codex/target-inspector-ui-20261026`，仓库 `.worktrees/target-inspector-ui-20261026`。root独占检查器TSX/CSS、直接相关测试和本页规格；独立审阅者只读。后端、迁移、插件、其他页面和共享视觉参数不在修改范围；跨包冲突才升级。

采用 L2 Split Inspector，最大960px，身份/导航固定，正文局部滚动。默认「最近观察」并列显示首次发现与所选两轮的精确指标变化；作品提供目录/图表与2–3篇对照，轮次、对照、轨迹作为可返回的展开。巡查集中既有规则与资料维护。采用LIDS B字级、M2按钮/下拉、C焦点和S1返回；控件为现有ActionButton、SelectField的页面接入（implementation_fix），不新建共享标准或Token。

创作者与关键词均读取现有rounds/round-comparison。只有明确同ruleRef且更早的轮次自动作参照；未知不冒充同规则。变化使用服务端comparable及delta；部分覆盖、账号限制、未知、已知零分别保留。首次发现不等于刚发布，未命中不表示删除；主作品以有序作品引用改变服务端参照统计。非必要解释折叠或删除。

验收覆盖默认首屏、历史轮次/最早轮次/无规则、未知与零、部分/失败、2–3篇及主作品切换、24h缺失、搜索分页、图表明细、返回焦点/滚动、下拉键盘/Esc、长文本、桌面与390px。自动测试与浏览器GET读取证明分别记录；不执行采集/恢复/模型按钮。私有日志与截图保留在artifacts/private/target-inspector-ui-20261026，不入Git。正式发布身份与Mog视觉验收单独记录。

验证直接依赖：本机已有大量合法worktree时，必跑预检的 `git worktree list | awk ... exit` 在pipefail下实测返回141，未完成后续门禁。本包只将该awk改成读完输入、仍只输出首项；不删除或放宽任何检查，已在相同环境通过完整preflight。该一行与UI候选一起独立审阅。
