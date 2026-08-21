# DS-REF-TOPIC-001 · Topic 参考页的 LIDS 采用记录

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: DESIGN-002 的静态、合成 Topic Intelligence Reference Page；不授予任何真实运行页面的实现
> 事实来源: [LIDS 主标准](../lids/system.md)、[LIDS Token](../lids/tokens.md)、[LIDS Pattern](../lids/patterns.md)、PAGE-TOPIC-001 与 Issue #7
> 冲突时以谁为准: 用户最新确认、当前产品/数据/权限/行动合同、AGENTS.md 和 LIDS；本记录不替代这些来源

本文件取代 DESIGN-002 首版 DS-001–DS-007 中“暗色酸绿、ASCII 新粗野主义”的局部数值与构图。它们不再是 Linggan 的设计标准。保留下来的不是某种视觉皮肤，而是首个切片对**真实边界**的要求；视觉表达统一改由 LIDS v2.0 约束。

## 必须采用的 LIDS 规则

| 参考页区域 | LIDS 依据 | 本页采用方式 | 不得做什么 |
|---|---|---|---|
| 工作面与层级 | LIDS-SYS-001 §3/§4；LIDS-TOK-001 | L2 暖灰 Precision Canvas、煤黑结构、一个主 Surface、Hairline/rail 分层 | 暗底霓虹、卡片墙、第二套主题、三层容器 |
| 阅读与机器标签 | LIDS-TOK-001；LIDS-PRI-001 | Topic/原声使用 Sans；模式、范围、来源/限制使用 Mono | 全局 Mono、功能文字 <11px、ASCII 承担事实 |
| Signal | LIDS-SYS-001 §4 | 小面积橙红只用于焦点、当前本地选择和边界定位 | 把 Signal 当趋势、错误、成功或“系统正在运行” |
| 页面组合 | LIDS-PAT-001 Topic Intelligence Detail | 观察 → 原声 → 候选解释 → 边界 → 本地意图 | 统计卡 → 图表 → Agent 总结的假情报骨架 |
| 互动 | LIDS-PRI-001、LIDS-AGENT-001 | 仅 disclosure、合成窗口和 Local Intent，短暂原位反馈 | 请求、写入、真实成功、toast、假加载/扫描/计数 |
| 可访问性 | LIDS-SYS-001 §6 | 可键盘、可见焦点、状态文字双通道、Reduced Motion | 只依靠颜色/hover/动效，或隐藏限制 |

## 合成参考页额外边界

1. 首屏持续显示 `SYNTHETIC / NOT LIVE` 与 `SOURCE_INCOMPLETE`；它们不是可关闭、可滚过或需点击才看见的脚注。
2. 每一条模拟原声旁持续显示 `SYNTHETIC / NOT EVIDENCE`，且“展开”只能增加它不能证明什么，不能首次揭示其合成性质。
3. 本页无真实数据合同，故不得使用 `OBSERVED`、`VALID`、`FRESH`、`RUNNING`、`COMPLETED` 等真实五轴状态来描述样本或页面。
4. ASCII 只用于范围、索引或可由中文替代的机器标签；不得生成趋势线、模拟终端、数据图形、运行日志或进度。
5. 所有互动都在同一局部区域返回 `LOCAL_INTENT_ONLY` / “未创建真实记录”；不能把本地选择描述为 Research、Decision、Action、通知或待办。

## 本页不证明什么

该页只验证一份静态合成阅读参考能以 L2 设计语言清楚呈现边界。它不证明 LIDS 已经有运行时主题/组件、Topic 有真实数据、任何候选成立、用户已执行真实动作、完整移动工作台可用或正式产品审美已获验收。
