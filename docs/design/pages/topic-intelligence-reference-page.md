# PAGE-TOPIC-001 · Topic Intelligence Reference Page

> 状态: 权威当前
> 运行时状态: L2 静态合成参考，不是运行页面
> 最后核对: 2026-08-21
> 适用范围: DESIGN-002 的「任务启动困难」Topic Intelligence 本地 Reference Page
> 事实来源: docs/pages/topic-intelligence-surface.md、[LIDS-SYS-001](../lids/system.md)、[LIDS-PAT-001](../lids/patterns.md)、DS-REF-TOPIC-001、Issue #7 与当前产品边界
> 冲突时以谁为准: 用户最新确认、产品页面、真实数据/权限/行动合同、当前 SCOPE 与 AGENTS.md；本规格不替代这些来源

## 1. 页面核心任务、答案和动作

- **核心任务**：在清楚的合成范围与限制下，理解样本中围绕「任务启动困难」出现了哪些表达，并选择“继续探索”或“暂时搁置”的本地意图。
- **三秒答案**：这是一个 `SYNTHETIC / NOT LIVE`、`SOURCE_INCOMPLETE` 的 L2 参考，不包含已验证的趋势、覆盖、系统结论或真实研究。
- **五秒动作**：用户可找到“查看示例限制”，然后选择本地的继续探索/暂时搁置；该选择不创建任何真实记录。
- **非目标**：真实趋势/Evidence/Source/Topic 管理/检索/采集/权限/写入/研究创建/行动执行/移动工作台/完整 P0。

## 2. 强度、Pattern、真相边界

| 字段 | 固定值 |
|---|---|
| 视觉强度 | L2 · Research / Analysis |
| 主 Pattern | LIDS Topic Intelligence Detail 的受限静态阅读段落 |
| 数据真源 | 无；全部为 `SYNTHETIC_REFERENCE`，不触碰数据/权限/行动合同 |
| 状态策略 | 合成/来源不足标记；不冒充 LIDS 五轴真实状态 |
| 主动作 | `LOCAL_INTENT_ONLY` 的局部意图回显 |
| 禁止元素 | KPI 卡、趋势百分比/箭头/曲线、Coverage 数字、正式状态 Tag、假运行日志、Agent 命令、真实成功/错误回执、常驻场景/动效 |

## 3. 页面结构与信息顺序

```text
L2 Workbench Header / mode / synthetic boundary
Topic identity + explicit scope and limitation
Synthetic sample window
Observation (limited sample) | Interpretation boundary
Raw voice samples (each explicitly synthetic / not evidence)
Candidate explanation | Unknown / source gap
Local intent (no persistence)
```

顺序服从 LIDS 的“变化/观察 → 原声 → 解释 → 证据边界 → 下一步”，但当前没有真实变化/证据资格，所以首个区块只能称为有限样本观察，不能称 Trend 或 Current Signal。

## 4. 区域组合与互动

| 区域 | LIDS / 本页来源 | 可以做 | 必须持续显示 | 不得替代 |
|---|---|---|---|---|
| Header/Boundary | LIDS-SYS-001、DS-REF-TOPIC-001 | 显示模式、范围、限制 | `SYNTHETIC / NOT LIVE`、`SOURCE_INCOMPLETE` | 真实系统状态、营销 Hero |
| Topic identity | LIDS Topic Detail | 说明样本 Topic 与唯一任务 | 合成范围/不推断趋势 | 真实 Topic 对象或官方结论 |
| Sample window | Page-local candidate | 切换两组合成阅读片段 | `DEMO WINDOW / NOT QUERY` | 实际筛选或刷新 |
| Observation/voice | Raw Voice 原则 + PAT-001 | 展开“不能证明什么” | 每条 `SYNTHETIC / NOT EVIDENCE` | 来源合格、真实 Evidence 或统计 |
| Candidate/boundary | PAT-002/PAT-003 | 提出待核验方向、列未知 | 支持样本和缺口 | Claim、Signal、Intelligence、Confidence |
| Local intent | PAT-004 | 局部回显选择 | `LOCAL_INTENT_ONLY` 与未创建记录 | Decision/Action/通知/待办/成功 toast |

互动键盘可达、Focus 可见、原位短过渡，`prefers-reduced-motion` 取消过渡。当前不承诺移动产品；窄视图只验证连续阅读与键盘，不得从中推断正式移动交互。

## 5. 验收和未证明边界

1. 首屏和每条样本均不需要点击即可看到合成/非 Evidence 边界。
2. 页面不出现真实五轴状态、趋势/增长/完整 Coverage/真实回执或网络读取/写入。
3. 任意局部互动只改变宣称范围内的本地文本，且可键盘到达、Reduced Motion 后仍有意义。
4. 视觉仅使用 LIDS 暖灰/煤黑/Signal、Sans/Mono 和 L2 Pattern；没有旧暗色主题或外部依赖。
5. 本页局部块仍是候选，不能声称正式组件库、真实 Topic 产品、P0 或部署。
