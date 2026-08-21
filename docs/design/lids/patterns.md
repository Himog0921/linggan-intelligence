# LIDS-PAT-001 · L1/L2/L3 页面 Pattern

> 状态: 权威当前
> 运行时状态: 定义页面组合约束，不声明任何页面已实现
> 最后核对: 2026-08-21
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
- 紧凑桌面无横向滚动；移动端不是简单缩放桌面结构。
