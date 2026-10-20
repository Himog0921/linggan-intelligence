# LIDS-PAT-001 · L1/L2/L3 页面 Pattern

> 状态: 权威当前
> 运行时状态: 定义页面组合约束；LOCAL-001A 已有受限 L1 Evidence Library 页面，但本文件不把它声明为通用 Pattern 实现
> 最后核对: 2026-10-19
> 适用范围: Linggan Intelligence 已获批准页面的结构、信息顺序、视觉强度和 Pattern 选择
> 事实来源: Mog 于 2026-09-02 指定的 `linggan-design-system-v7.html`（§04 壳层区位、§12 页面模式、ADR-10/11）、Mog 指定的 LIDS v2.0 `patterns.md`（SHA-256: `94fe15b47e7d38b1be15b0369c143f51859a1aa79a1ca6c2dba0db9dfa2a2719`）、[system.md](system.md)、已批准产品页面与 PAGE 规格；LIDS-ALIGN-001 本次授权与对齐记录
> 冲突时以谁为准: 用户最新确认、当前产品任务、真实数据/权限/行动合同与 SCOPE；Pattern 不能自行增加模块或操作

Pattern 是页面级组合而非可任意拼贴的组件清单。每个新页面先确定 L3/L2/L1，再选择唯一主 Pattern；只可参数化或轻度组合，不能自造第二种页面骨架。

| Pattern | 强度 | 核心任务 | 核心限制 |
|---|---|---|---|
| Immersive Observatory | L3 | 看世界变化怎样进入系统并形成情报 | 唯一主视觉；真实对象、变化、覆盖、原声/证据和状态都在首屏 |
| Topic Intelligence Detail | L2/L3 | 深入读懂一个长期 Topic 的变化、原声、证据边界和机会 | 原声优先于 AI；`PARTIAL + VALID` 直接可见 |
| Split Evidence Inspector | L2 | 浏览对象、证据、版本、来源与推导链 | 右侧只是当前对象的检查器，不是第二个首页 |
| Research Workspace | L2 | 围绕问题组织证据、冲突、缺口与人工复核 | Evidence 区大于 Agent 区；Agent 结论不自动升级 |
| Corpus Explorer | L1 | 检索、筛选、阅读、比较真实语料 | 同构密集行优先表格；P04用户问题按ALIGN-COMPOSITION-009保留已确认卡片，不机械套表格 |
| Collection Control | L1/L2 | 监控采集、失败救援、调度与运行状态 | Failed 优先；`PARTIAL + VALID` 不进入失败区 |
| Signal Review Queue | L1/L2 | 批量判断信号升级、继续观察、补样本或驳回 | 每条解释为何出现；人和 Agent 决定分开 |
| Settings / Governance | L1 | 管理规则、映射、系统设置 | 无大场景；一个外层 Form Surface；错误定位字段 |

## 结构约束

### Immersive Observatory（L3）

```text
Observatory Header
Identity / Readout Matrix
Topic Identity | Isometric Intelligence Scene | Runtime / Provenance / Evidence
Raw Voice Strip
Agent Command Dock
```

等距场景是唯一主视觉；左 Topic 不超过视觉宽度 28%，右 Runtime 不超过 24%，Signal 约 5% 以内。不要再加 KPI 卡片行、第二张大图/装置、常规聊天侧栏或全页深色。

### Topic Intelligence Detail（L2/L3）

```text
Workbench Header / Topic No. / Freshness / Actions
Topic Identity + Truth / Coverage / Validity
Current Signal / Change     | Observation Boundary / Source / Window / Coverage
Raw Voices
Content Angles / Opportunities | Evidence Chain / Conflicts
Next Observation / Research Actions
```

阅读顺序是“变化 → 原声 → 解释 → 证据 → 机会 → 下一步观察”，不是“统计卡 → 图表 → Agent 总结 → 更多统计卡”。Topic Identity 是事实对象不是营销 Hero；Content Angle 必须能回到证据；冲突/缺口不能藏最深；L2 局部等距不超过主内容 40%。

### Split Evidence Inspector（L2）

```text
Object Index | Evidence Workspace | Provenance Inspector
```

左侧为索引，中央是连续工作面，右侧只显示所选对象的来源、时间、Coverage、推导。紧凑屏右侧可进 Drawer；原始文本、清洗文本和 AI 标注必须视觉区分；关键选择/筛选的真实实现需要可恢复状态。

### Research Workspace（L2）

```text
Research Brief / Hypothesis / Boundaries | Evidence Canvas | Agent Trace / Human Review
Decision / Next Collection / Save as Intelligence
```

Research Brief 写范围和不回答什么；Agent Trace 默认折叠到关键阶段；人工判断与 Agent 建议用不同 TruthTag；任何采集建议/正式升级都须另有用户确认和真实合同。

### Corpus Explorer（L1）

```text
Compact Header
Search / Filters / Saved View / Result Count
Continuous Table or List
Detail Drawer / Pagination / Selected Actions
```

原声字体按typography的ALIGN-VOICE-004统一OPPO Sans，Q03已关闭；普通数字/时间用Manrope，技术原样字符串用Mono。批量操作仅选中后出现；无品牌级背景动画。

### Collection Control（L1/L2）

```text
Header / Active Workstations / Risk / Next Window
CollectionStatusMatrix
Task Queue | Task Detail / Reason / Impact / Retry
Processing Timeline / Logs
```

排序优先为 `FAILED/NEEDS ATTENTION → RETRYING → RUNNING → QUEUED → COMPLETED`。每个失败有原因、影响、救援动作；已采集有效数据及其缺口直接可见，不把 Partial 简化成红色 Failed。

### Signal Review Queue（L1/L2）

每行至少包含 Change、Evidence、Coverage、Conflict、Recommended Act。批量通过可撤销；驳回有原因；Warning 使用琥珀，不能用 Signal。

### Settings / Governance（L1）

```text
Header / Current Environment / Save State
Content Index | One Form Surface
Sticky Save / Validation Summary
```

不放大型等距动画或海报标题。规则用编号目录；保存范围、生效时间与影响范围直接可见；危险设置放独立 Danger Zone。

## 页面模式：一个世界，不同仪器

相同的全局语言下，不同业务域拥有不同画面。四种模式各自绑定一个域的真实工作方式，**不是四套可以自由挑选的皮肤**：

| 域 | 模式 | 主 Pattern | 首屏必须先回答 |
|---|---|---|---|
| 证据库 | 连续档案 | Corpus Explorer | 库里有什么、这批受什么限制 |
| 采集运行 | 控制 + 实时 | Collection Control | 什么坏了、什么在跑 |
| 目标档案 | 身份 + 基线 | Topic Intelligence Detail | 这个对象是谁、基线是什么、变了多少 |
| 编年记忆 | 事件脊柱 | Signal Review Queue 的时间轴变体 | 何时发生了什么、按时间倒序 |

选择模式的依据是这个域的用户任务，不是"换个样子好看"。同一个域内不得并存两种模式。

## 壳层区位：见 LIDS-SHELL-001

页头与 Context Bar 的**区位划分、内容类型与数量上限**由 [shell-zones.md](shell-zones.md) 冻结（`ADR-10`）。所有 Pattern 都在这两条带之下工作，没有 Pattern 可以要求增加区位或加第二行。

下一节的"外壳所有权"规定外壳样式**写在哪**；shell-zones.md 规定外壳里**能放什么、放几个**。两者配套，缺一条另一条就会被绕开。

## 外壳组件的所有权：页头与二级导航不属于任何一个页面（2026-08-26）

全局页头（品牌区、一级导航、系统边界、命令入口）、上下文行、左侧二级导航，由 `apps/web/src/app/AppShell.tsx` 及对应React Frame为所有正式页面渲染共享结构。**它们的样式必须整份住在 `shell.css`，任何单页样式表都不得声明它们。**

这条不是洁癖，是已经发生过的事故：DESIGN-003 的中文优先排版只写进了语料页的样式表，于是同一段 HTML 在语料页渲染成 11px、在采集页渲染成 8px——**一个共用页头长出了两套字号标准**，而页面本身看不出问题在哪。同样漏掉的还有四个断点的响应式：采集页当时完全没有页头适配。

| 归属 | 内容 | 写在哪 |
|---|---|---|
| 外壳 | `.v7-global-*`、`.v7-primary-nav`、`.v7-nav-*`、`.v7-context-*`、`.v7-side*`、`.v7-app`、`.v7-shell`、`.v7-sr-only`、`.v7-kpi`、`.v7-vr`，以及它们全部断点的响应式 | `shell.css` |
| 页面 | 只有这个路由自己的工作区组件 | 该页样式表 |

执行细则：

- 页面需要改外壳的表现时，**改 `shell.css` 让所有页面一起改**，而不是在自己表里覆盖。若某页确实需要不同表现，那是外壳缺一个参数，应在React AppShell/Frame增加明确属性，不是在 CSS 里各写各的。
- 外壳规则必须同时覆盖同一组件的所有 HTML 形态。一级导航混用 `<a>`（已接通路由）与 `<button>`（未接通责任），只写 `button` 的规则会静默地只改一半。
- 判断方法很机械：拿 `shell.css` 声明过的类名去搜单页样式表，命中即违规。React Token/外壳纪律测试及正式浏览器`shell-layout.mjs`承接检查；新增页面仍须核对外壳所有权，不能依赖已删除的Rust旧DOM测试。

### 三级导航的物理位置（ALIGN-NAV-003）

当前二级页面下的真实分区Tab统一由共享壳／Frame在Z02面包屑模块承载，页面声明配置，主工作区保持查询、主动作与内容。旧Z04-N只作职责坐标，物理host为Z02。详情内部Tab与集合／展示切换按各自职责保留。规则及键盘／窄屏边界见[shell-zones](shell-zones.md)。

### 一级导航只指向职责入口，永不指向子面（2026-08-26）

每个一级职责拥有自己的**入口路由**（`/corpus`、`/collection`……），该路由是这个职责的默认落点**唯一的决定处**。全局页头的一级导航必须链接到入口路由。

**页头链接到某个具体子面，等于把「默认落点是哪个子面」这个决定复制了一份。** 两份决定会在默认落点第一次变动时立刻分叉——`DESIGN-006` 把采集默认入口改到 `/collection/attention` 时就是这样：`/collection` 改了，页头还指着 `/collection/targets`，同一个「采集」从两处进入落到两个面。

- 新增一级职责时，先建入口路由，再把页头指向它。不要因为"现在只有一个子面"就跳过入口路由——语料当初就是这么少建的，于是页头只能指向 `/corpus/evidence`。
- 少一次 302 跳转不是省，是把一个决定拆成两处维护。
- 由 `AppShell.test.tsx` 的职责入口登记检查自动执行：一级导航里任何多于一层路径的 href 都会失败。

## 页面身份与区域（ALIGN-PATTERN）

导航已定位当前页面时，工作区不再创建同名的单项Tab或无必要标题块；正确的可访问页面名称保留。真实对象标题、编辑对象身份与两个以上真实内容分区继续显示，不机械隐藏所有h1。

计数与数据截至归Z04-R或有业务必要的对象内容；筛选归工具区；集合/对象/表单动作按作用对象归位。旧“所有计数上移上下文行”的规定被 [regions](regions.md)替代，不能把统计重新堆进面包屑。

采集任务采用A连续账本方向，一任务一行，解释进入详情；不将任务页与工位诊断强制组合。旧Collection Control排序/首屏结构与11候选PT的视觉组合按ALIGN-COMPOSITION-009采用；沿现有PAGE合同，不在本次自动改写排序或状态资格。

PT01–PT11、P01–P17/26模式是有来源的组合候选，详见 [逐项对齐表](review-alignment.md)。Q08的外部组合与操作位置已采用；组件白名单/认证及业务适配分别保持Q10和真实合同。

## Pattern 验收

- 已选择唯一主 Pattern 和 L1/L2/L3；3 秒答案、5 秒主动作明确。
- 只有一个视觉核心；内容顺序符合真实工作流；状态/证据不被装饰遮挡。
- 不自由新增第二套 Header、Primary、状态、Command Dock、Evidence Chain、临时场景或容器语法。
- 页面身份按ALIGN-PATTERN去重；可访问名称保留，对象标题不机械隐藏，计数归Z04-R。
- 整页不溢出；密集表格在自身容器横向滚动，移动端保留身份和动作等价入口。
- 壳层按shell-zones的ALIGN-SHELL-007采用A及图标折叠；PAGE专用材料槽和Q12例外保留。
- 视图切换使用文字 Tab + 信号下划线，页面内不存在方格分段控件（`ADR-11`）。
- 材料预算成立：白场约 70%，连续阅读区背后为纯白，校准场的既有范围与本次控件材质采用边界按[materials.md](materials.md)；候选像素值不自动推广。

WEB-REACT-001 H补记：选中导轨悬停保持墨底、390窄屏六项职责完整、1024/1280个人菜单不出屏，现由`apps/web/test/e2e/shell-layout.mjs`在真实生产构建上验证，接入正式B浏览器链路。


## ALIGN-COMPOSITION-009 · Q08已采用

唯一机器规则源为[composition-layout.json](composition-layout.json)，通过sync-lids-composition生成隔离镜像；不是按整个模块或PT统一套一套布局，而是逐page:mode执行。先判断是否为含搜索的列表，再选组合；普通功能页里有查找控件不等于列表页。新模式没有映射时先明确任务与来源，不自行选择另一个骨架。

| 已采用规则 | 当前模式 | 布局与操作位置 |
| --- | --- | --- |
| B：所有含搜索的列表 | 证据库、评论检索、研究批次、用户问题、创作者、观察目标、采集任务、作品库、发布记录 | 宽屏查询/视图合带；收窄恢复T→V→R→B→P，列表自身横滚；集合主动作在工具带右侧，低频设置在视图区 |
| C：其余功能 | 评论/运营概览、主题图谱/旅程、材料阅读、选题/画布/正文/图页、配置、工位诊断、账号目录、每日矩阵、导出 | 当前工作区独立滚动，工具在工作区顶部常驻；对象/表单动作仍归对象/表单，不搬进全局壳 |
| A：入口表单豁免 | P01:entry | 没有工作区，不创造空工具栏或常驻条 |

领域和三级Tab始终由Z02承载。页面需要的区域才呈现，概览大读数不机械复制到列表。P09保持L1/44px/名称144px及字段自定义/局部横滚。已采用动作的展示位置不授权真实命令、停止资格、浮层退出默认或组件正式认证；Q09/Q10继续单独核对。C的620px只是隔离预览视窗，正式页面使用壳层以下可用工作区高度，不作为新全局Token。

### 用户问题卡片保留

Mog明确喜欢真实评论研究用户问题的现有卡片。以`ProblemCards.tsx`的ProblemResultCard/ProblemClueCard及`Results.module.css`为结构/材质来源，保留黑色编号、方角细边/硬投影、问题标题、定义/原声分区、切角原声区、指标或线索说明、时间和整卡详情入口，保留阅读/紧凑密度。卡片按可用宽度收为3/2/1列；不因B列表标准、数量增多或无边框V2而改为通用表格。已确认B字体/字号角色、OPPO原声与Manrope数字继续应用。样例不得冒充已建档问题、把线索计入问题数或拿原声预览替代整段定义验证。

本次只迁移独立审阅稿；真实3000当前ADHD无已建档问题，未归并线索可读，审阅使用合成文本。旧简化问题表格被卡片替代，旧三案和ledger-adoption-composition-12保留采用前历史。Q12的M-02E/M-06/V8_workbench例外继续有效（工作台动作按钮外观除外，见ALIGN-WORKBENCH-015）；创作专用壳不因本次公共壳预览而取消。运营账号/发布记录导出按业务，每日数据不分业务。
