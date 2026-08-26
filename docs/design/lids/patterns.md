# LIDS-PAT-001 · L1/L2/L3 页面 Pattern

> 状态: 权威当前
> 运行时状态: 定义页面组合约束；LOCAL-001A 已有受限 L1 Evidence Library 页面，但本文件不把它声明为通用 Pattern 实现
> 最后核对: 2026-08-26
> 适用范围: Linggan Intelligence 已获批准页面的结构、信息顺序、视觉强度和 Pattern 选择
> 事实来源: Mog 指定的 LIDS v2.0 `patterns.md`（SHA-256: `94fe15b47e7d38b1be15b0369c143f51859a1aa79a1ca6c2dba0db9dfa2a2719`）、[system.md](system.md)、已批准产品页面与 PAGE 规格
> 冲突时以谁为准: 用户最新确认、当前产品任务、真实数据/权限/行动合同与 SCOPE；Pattern 不能自行增加模块或操作

Pattern 是页面级组合而非可任意拼贴的组件清单。每个新页面先确定 L3/L2/L1，再选择唯一主 Pattern；只可参数化或轻度组合，不能自造第二种页面骨架。

| Pattern | 强度 | 核心任务 | 核心限制 |
|---|---|---|---|
| Immersive Observatory | L3 | 看世界变化怎样进入系统并形成情报 | 唯一主视觉；真实对象、变化、覆盖、原声/证据和状态都在首屏 |
| Topic Intelligence Detail | L2/L3 | 深入读懂一个长期 Topic 的变化、原声、证据边界和机会 | 原声优先于 AI；`PARTIAL + VALID` 直接可见 |
| Split Evidence Inspector | L2 | 浏览对象、证据、版本、来源与推导链 | 右侧只是当前对象的检查器，不是第二个首页 |
| Research Workspace | L2 | 围绕问题组织证据、冲突、缺口与人工复核 | Evidence 区大于 Agent 区；Agent 结论不自动升级 |
| Corpus Explorer | L1 | 检索、筛选、阅读、比较真实语料 | 20 行以上同构对象用表格，不做卡片瀑布流 |
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

原声是 Sans；数字、时间、来源是 Mono；批量操作仅选中后出现；无品牌级背景动画。

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

## 页头收回：页面只陈述一次自己的名字（DESIGN-003，2026-08-26 扩展到全部 Pattern）

一个页面在内容开始之前，已经把自己的名字说了三遍：上下文行的面包屑、左侧导航的当前项、页面自己的 `h1`。第三遍不增加任何信息，却吃掉首屏约 90px。**所有 Pattern 一律不保留视觉可见的页面标题块。**

| 元素 | 处置 | 理由 |
|---|---|---|
| `h1` | 必须存在，但用 `.v7-sr-only` 视觉隐藏 | 文档结构与读屏软件需要它；视觉上是第三次重复 |
| 标题上方的英文 eyebrow（`COLLECTION / OBSERVATION TARGETS` 一类） | 删除 | 面包屑已陈述同一件事，只是换了语言 |
| 标题旁的计数块 | 上移到上下文行，写成 `.v7-kpi` | 计数是页面状态，属于状态行，不需要专属区域 |
| 标题下的常驻说明句 | 删除 | 页面的身份不靠一句话解释（DESIGN-005 已定） |

约束细则：

- `h1` 的文字保持页面真名，不得为了隐藏而改写或留空；`main` 用 `aria-labelledby` 指向它。
- 计数上移后**每个读数仍是独立的 `.v7-kpi`**，不得为了省位置合并成一个总数——两个 `UNKNOWN` 加不出一个已知值。
- 计数与系统状态之间用 `.v7-vr` 分隔，让「这一页的数」和「整个系统的状态」在同一行里仍然分得开。
- **计数标签一律用中文。** `.v7-kpi em` 是 11px Sans 的阅读槽位，同一行右侧的系统词是 9px Mono 大写。把英文标签放进 Sans 槽位，一行里就出现两种规格的英文（11px 细体 / 9px 粗体带字距），正是「标准不一致」的来源。英文系统标签只走 Mono 那一档（见 [primitives.md](primitives.md) 排版表的 `lgi-caps`），中文不进 Mono。
- 例外只有一个方向：`PARTIAL`、`VALID` 这类 LIDS 正式状态术语在**表示状态**时保持英文原形；当它只是「部分完成的任务数」这类计数标签时，按上一条读中文。
- `.v7-sr-only`、`.v7-kpi`、`.v7-vr` 属于 `shell.css` 共享层，不得在任何单页样式表里重新定义：上下文行本身就是所有页面共用的同一段渲染代码。
- 页面骨架不要依赖固定的 grid 行数（`grid-template-rows: auto auto 1fr` 一类）。收回页头后，同一 Pattern 下有的页面有工具栏、有的没有，正文必须在两种情况下都占满剩余高度。

验收方式：页面源码里搜不到 `c-title`、`c-eyebrow`、`c-head`、`v7-title`、`v7-eyebrow`，且能搜到 `<h1 class="v7-sr-only">`。

## 继承与组合

| 元素 | L3 | L2 | L1 |
|---|---|---|---|
| 大网格背景 | 强 | 弱 | 关/极弱 |
| Calibration Rail | 可 | 禁止 | 禁止 |
| 完整等距场景 | 可 | 局部 | 禁止 |
| 深色 Command Dock | 可常驻 | 紧凑/按需 | 快捷入口 |
| Raw Voice | 大型编辑式 | 标准阅读块 | 列表/Drawer |
| ASCII Fingerprint | 主视觉可用 | 分析辅助 | 通常不用 |
| 环境循环动画 | 极少 | 仅真实状态 | 禁止 |
| 表格/连续列表 | 辅助 | 常用 | 主体 |

允许 Topic Detail 嵌入小型 Split Evidence Inspector、Research 调用 Raw Voice、Collection 调用 Evidence Chain、L3 Observatory 下方进入 L2 内容区。禁止 L3 嵌 L3、Corpus 首屏放完整场景、同页三种 Header、Pattern 内再套完整同级页头，或右 Inspector 再生第二套筛选/导航。

## Pattern 验收

- 已选择唯一主 Pattern 和 L1/L2/L3；3 秒答案、5 秒主动作明确。
- 只有一个视觉核心；内容顺序符合真实工作流；状态/证据不被装饰遮挡。
- 不自由新增第二套 Header、Primary、状态、Command Dock、Evidence Chain、临时场景或容器语法。
- 页头已收回：没有视觉可见的页面标题块，`h1` 仅以 `.v7-sr-only` 存在，计数在上下文行。
- 紧凑桌面无横向滚动；移动端不是简单缩放桌面结构。
