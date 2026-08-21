# DESIGN-002 · Topic Intelligence Reference Page

> 状态: 活跃计划
> 最后核对: 2026-08-21
> 适用范围: `DESIGN-002` 的产品页面规格、最小设计语言、合成静态参考实现、验证与交接
> 事实来源: Mog 对 Issue #7 的直接确认、`docs/current-state.md`、ARC-001、Topic 产品边界与 UI 设计治理协议
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前 SCOPE、权威产品/数据合同与真实运行事实；本计划不扩大这些边界

## 目标与用户可见结果

为 Topic「任务启动困难」制作一份可在本地打开的 Reference Page。它让 Mog 在 5–10 分钟内走查一个窄问题：

> 在**明确的样本与限制**下，这个 Topic 最近有哪些值得继续研究的观察，下一步是继续探索还是暂时搁置？

交付的页面不是运行中的 Linggan，也不是未来产品的技术原型。它是一个真实可查看的设计基准：后续 Agent 必须引用其页面规格、基础规则和模式；尚未被第二个页面验证的局部块不得伪装成通用组件库。

## 已确认范围

- 页面：`Topic Intelligence Surface`；示例 Topic 是「任务启动困难」。
- 视觉方向：未来感 + ASCII + 新粗野主义 + 情报系统感。
- 载体：一份自包含静态 HTML 源文件，可本地用浏览器打开。
- 数据：全部为合成、脱敏式示例；页面第一屏和每个关键区都必须显示它不是实时系统、不能支撑真实结论。
- 交互：只演示时间窗口、证据展开和“继续探索／暂时搁置”的**本地意图**反馈；不创建研究、决定、采集或任何持久记录。

## 明确非目标

- 不回答完整 P0 的入口数量、首页结构、未来完整 Topic 工作区或技术架构。
- 不创建 Web 应用、React/Rust 前端、路由、API、数据库、登录、真实状态写入、真实 Trend/Coverage 或部署。
- 不将样本数、样本文案、图形或点击后页面反馈表述为真实 Observation、Signal、Claim、Coverage、趋势或任务回执。
- 不把首页草案、旧内容工作台或任何未登记外部参考转化为当前实现依据。

## 可追溯范围

| 问题 | 权威来源 | DESIGN-002 的处理 |
|---|---|---|
| Topic 的深入任务是什么 | `docs/pages/product-interface-architecture.md`、ARC-001 | 收窄为一次判断与下一步意图，不复制整个工作区 |
| 近期趋势能否声明 | `DEC-05`、产品不变量、当前状态 | 不显示增长/下降、百分比、趋势箭头或可比折线；仅演示带限制的样本观察 |
| 真实字段、权限、回执从哪里来 | 当前 SCOPE 与未来数据合同 | 尚不存在；所有对应位置展示 `SYNTHETIC`、`SOURCE_INCOMPLETE` 或“本地演示，无真实动作” |
| 页面怎样一致表达 | `docs/design/` 的 DESIGN-002 DS/PAT/PAGE | 只组合本计划列出的规则 |

## 交付物与文件边界

| 交付物 | 位置 | 目的 |
|---|---|---|
| 活跃计划 | 本文件 | 冻结范围、停止点与验收 |
| 产品页面规格 | `docs/pages/topic-intelligence-surface.md` | 冻结用户任务、状态、行动语义和非目标 |
| Foundation | `docs/design/foundation/topic-intelligence-visual-language.md` | 形成 DS-001–DS-007，仅适用于本参考页及其后明确采纳的页面 |
| Patterns | `docs/design/patterns/evidence-candidate-and-boundary-patterns.md` | 形成 PAT-001–PAT-004，阻止事实、候选与缺口混写 |
| Component lifecycle | `docs/design/components/component-promotion.md` | 规定局部块如何经第二页面验证后才晋升 |
| Page spec + reference source | `docs/design/pages/` | 固定 PAGE-TOPIC-001 的组合与视觉参考 |
| 验收记录 | `docs/design/pages/topic-intelligence-reference-acceptance.md` | 记录静态视觉、互动、自动检查和未证明边界 |
| 验证 | `scripts/verify-topic-intelligence-reference.sh` | 检查索引、关键诚实性标记和原型交互骨架 |

`apps/`、`crates/`、`database/`、真实数据合同、任何 F01 文件与 `references/` 均不在本事项范围内。

## 执行步骤与可证伪验收

1. 固定产品边界和状态矩阵。
   - 完成条件：产品页面能说明一个任务、进入/退出、每种状态的用户含义，以及当前不具备真实数据的原因。
2. 固定最小视觉语言、信息模式与组件晋升门。
   - 完成条件：每个规则都有 DS/PAT/PAGE ID、来源、适用范围、非目标和验收方式；不出现“任何页面默认使用”的越界规则。
3. 制作静态参考实现。
   - 完成条件：页面有醒目的合成标记、无趋势假象、可见限制、局部意图反馈和 `prefers-reduced-motion` 处理；窄视口只承诺可读参考，不承诺移动工作台。
4. 收口和交接。
   - 完成条件：索引、当前状态、ARC 关系、月度记录、自动检查、浏览器走查、draft PR、独立审查和独立集成均按 Issue #7 记录。

## 停止与升级

以下任一情况停止受影响部分并写 `DECISION_REQUIRED` 或 `SOURCE_INCOMPLETE`：需要真实字段、真实统计/趋势/Coverage 资格、身份或权限、持久化决定、产品动作回执、Web 技术选型、外部字体/图片、生产部署，或需要修改未授权文件。

## 验收与证明边界

| 层级 | 本事项目标 | 不能由本事项证明 |
|---|---|---|
| 设计规格 | 规则、模式和页面组合可追溯且互不冲突 | 未来页面一定适用，或审美已获 Mog 走查通过 |
| 参考实现 | 静态 HTML 在声明条件下能呈现并演示局部交互 | Web 应用、组件库、真实数据/API/权限/行动已实现 |
| 自动检查 | 结构、索引、关键边界标记可被脚本核验 | 样本文案正确、视觉质量、真实业务链路 |
| 真实链路 / 部署 | N/A | 任何实时 Evidence、Observation、Topic、Signal、Decision、Action 或生产环境 |
