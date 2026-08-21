# LIDS-AGENT-001 · UI Agent 执行清单

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: 所有生成、修改、审查或验收 Linggan Intelligence 用户可见 UI 的协作 Agent
> 事实来源: [system.md](system.md)、[tokens.md](tokens.md)、[primitives.md](primitives.md)、[patterns.md](patterns.md)、docs/agents/ui-execution-contract.md 与 Mog 指定的 LIDS v2.0 `ai-agent-guide.md`
> 冲突时以谁为准: AGENTS.md、UI 执行合同、用户最新确认、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE；本指南不扩大实现权限

## 不可跳过的开始门

1. 确认 Issue、Claim、独立 worktree、文件所有权和 UI Change Manifest。
2. 读 AGENTS.md、docs/README.md、docs/current-state.md、UI 执行合同和本 LIDS 目录。
3. 读取对应产品页面、PAGE/PAT/CMP、真实数据/权限/行动合同和当前 SCOPE。
4. 先回答页面核心任务，再定 L1/L2/L3，再选唯一 Pattern；没有答案立刻 `DECISION_REQUIRED`。
5. 只实现本事项明确批准的最小切片；当前不存在的组件、运行时、资产、数据或动作不得因 LIDS 提及而被预建。

## 强制决策树

**页面强度**：整体认知/重大 Topic/情报形成过程 → L3 Observatory；研究/证据比较/冲突核查/Agent 辅助判断 → L2；检索/列表/任务处置/设置 → L1。

**容器**：普通分组 → 留白+标题+Hairline；主面 → canvas Surface；次级 → soft；当前对象 → signal-soft/rail；警示 → warning-soft；错误 → danger-soft；命令/关键判断 → ink；列表/表格 → 连续工作面。禁止框套框、页面临时 clip-path、非交互卡 hover。

**颜色**：背景 canvas，内容面 canvas-hi，结构 ink，正文 body，辅助 muted，活跃/选中 signal，浅底小字 signal-ink，Valid/Completed success，Partial/Aging/Retrying warning，Failed/Invalid danger，链接/说明 info。Signal 绝不承担 warning/danger/success、普通装饰或所有增长数。

**状态**：来源/处理层级 → TruthTag；完成度 → CoverageTag；可用性 → ValidityTag；新鲜度 → FreshnessTag；执行阶段 → OperationTag。不得压成一个 `failed`，不得把未知当 0，`PARTIAL` 不等于 `INVALID`。

**按钮**：唯一真实主动作 → Primary；返回/取消/查看证据 → Secondary；展开/复制/筛选/切换 → Quiet；删除/停止/驳回 → Danger + Confirm/Undo；深色命令面 → Command。未接通能力不能成为可点击 Primary。

**动效**：按下/选中/展开 → 80–250ms；真实任务阶段 → 绑定 Operation；来源到情报的空间关系 → 真实 Scene State；只为“更酷” → 删除。永久禁止鼠标视差、漂浮、Ring Spin、代码雨、扫描线、弹跳、360° 旋转和 L1 环境循环。

**场景**：L3 主场景只能在单独的技术/资产/数据授权后按 Style Frame → 灰模 → 资产 → 性能/降级闭环推进；L2 只可受控局部静态/2.5D；L1 只可任务运行时的小型状态图；纯装饰 → 不使用。来源包 V3 Prism 不是生产资产。

## 每次交付前的检查

- [ ] PAGE 规格、强度、唯一 Pattern、3 秒答案、5 秒动作和页面非目标已明确。
- [ ] 所有视觉值来自 LIDS Token；没有硬编码 hex、pt、随机字号/间距、第二套 Header/Button/Status/Surface。
- [ ] 一个视觉组一个外层 Surface；同屏一个视觉核心；功能文字至少 11px。
- [ ] 真实页面分别表达 Truth/Coverage/Validity/Freshness/Operation；原声、来源、窗口、样本、冲突、复核和适用边界可查看。
- [ ] 合成页面在首屏和每个可能被误读为原声/事实的区域持续说明 `SYNTHETIC / NOT LIVE` 与限制；没有假趋势、假运行、假成功或本地状态冒充回执。
- [ ] hover/focus/active/disabled/loading/empty/success/warning/error/partial/stale 已按本次页面有依据地覆盖；零原生 alert/confirm/prompt，零假按钮。
- [ ] 1280×800、1440×900、1920×1080、390×844 及本页所需尺寸通过；无横向滚动；键盘顺序/Focus/Reduced Motion/状态双通道成立。
- [ ] 相关 PAGE/CMP、静态预览或运行时预览、migration log、UI Change Manifest、验收记录和项目索引已同步；检查/测试只声明实际验证层。
