# DESIGN-002 · Topic Intelligence Reference Page

> 状态: 活跃计划
> 最后核对: 2026-08-21
> 适用范围: `DESIGN-002` 的产品页面规格、最小设计语言、合成静态参考实现、验证与交接
> 事实来源: Mog 对 Issue #7 的直接确认、`docs/current-state.md`、ARC-001、Topic 产品边界与 UI 设计治理协议
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前 SCOPE、权威产品/数据合同与真实运行事实；本计划不扩大这些边界

## 目标与用户可见结果

为 Topic「任务启动困难」制作一份可在本地打开的 Reference Page，并把 Mog 指定的 LIDS v2.0 融入项目的 UI 治理。它让 Mog 在 5–10 分钟内走查一个窄问题：

> 在**明确的样本与限制**下，这个 Topic 最近有哪些值得继续研究的观察，下一步是继续探索还是暂时搁置？

交付的页面不是运行中的 Linggan，也不是未来产品的技术原型。它是一个真实可查看的设计基准：后续 Agent 必须引用 LIDS、页面规格、基础规则和模式；尚未被第二个页面验证的局部块不得伪装成通用组件库。

## UI Change Manifest

### 1. 事项

- Issue / SCOPE：Issue #7 / `DESIGN-002`；用户于 2026-08-21 追加明确指定 `Linggan_Intelligence_Design_System_v2.0` 为全项目设计表达标准。
- Agent 与 worktree：`/root` / `/Users/moglenny/proma/.worktrees/linggan-intelligence/issue-7-design-002`。
- 目标：将 LIDS 的 Token → Primitive → Component → Pattern → Page、L1/L2/L3、状态诚实、Agent 决策树、模板/迁移纪律和机器检查纳入 Linggan 的设计治理；将合成 Topic 参考页改为 L2 LIDS 表达。
- 用户可见结果：可本地打开的暖灰精密情报风格静态参考页；用户能持续看见样本为合成且没有真实动作；未来 UI Agent 有统一、可执行的标准入口。
- 明确非目标：不创建 Web/React/Rust 前端、运行时 Token CSS、正式组件、真实数据/状态、3D/WebGL、真实 Agent、权限、网络请求、持久化、部署或完整 P0。

### 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `docs/current-state.md` | 已读 | 事实优先、Issue/PR、无 UI 实现授权与 ARC/P0 边界 | 2026-08-21 |
| UI execution contract | 已读 | UI 写入的闭集、变更分类、停止与验收合同 | 2026-08-21 |
| 产品页面文档 | 已读 | Topic 的单一任务、无趋势/真实行动的限制 | 2026-08-21 |
| 既有设计治理/PAGE/PAT/CMP | 已读并更新 | 保留事实/候选/边界与组件晋升门，消除旧视觉规则冲突 | 2026-08-21 |
| Mog 提供的 LIDS v2.0 包 | 已完整核对 | 设计表达、Token、Primitive、Pattern、Agent 规则、模板、原型审计与迁移标准 | 2026-08-21 |
| 数据/权限/行动合同 | 当前不存在 | 确认所有演示必须保持 synthetic/not-live | 2026-08-21 |
| 当前代码/测试 | 已核对 | 当前没有可继承的 Web UI、主题或真实状态实现 | 2026-08-21 |

### 3. 分类

- 分类：混合（视觉表达 + 本地演示互动 + 状态/语义）。
- 最高风险类别：状态/语义。
- LIDS 层级 / 主 Pattern：L2（Topic Intelligence Detail，含 L2 工作台信息密度和局部 L3 深度阅读感）；主 Pattern 为“观察变化 → 阅读原声 → 区分候选解释与证据缺口 → 形成仅本地的下一步意图”。L1 只提供未来入口语义，本页不实现；L3 只在单条原声/限制的深读节奏中被引用，不创建大场景、3D 或独立 L3 工作区。
- 跨层触及判定：Token **是**（将 `LIDS-TOK-001` 全量镜像进静态页并由脚本对照）；Primitive **是**（页面仅演示 Text / Quiet button / InstrumentSurface / Readout / Focus 的规则）；CMP **否**（未建立或晋升正式组件）；Scene **否**（L2 禁止大场景，未添加 3D/WebGL）；Motion **是，局部**（仅使用 LIDS 交互时长及 Reduced Motion 回退）；Data Truth **是，展示边界**（只展示 `SYNTHETIC` / `SOURCE_INCOMPLETE`，不声明任何真实 Truth / Coverage / Validity / Freshness / Operation 值）。
- 对应来源：PAGE-TOPIC-001、PAT-001–PAT-004、LIDS-SYS-001、LIDS-AGENT-001、当前产品边界与 UI execution contract。
- 风险理由：页面中的原声、窗口、候选、边界和下一步意图可能被误读为真实 Evidence、趋势、系统判断或已发生动作；因此视觉变化按状态诚实标准执行。
- `DECISION_REQUIRED`：真实数据状态、Trend/Coverage、真实 Topic/Source/Evidence、真正的研究/行动、运行时 Web 技术、场景资产与部署均仍需要独立决定。

### 4. 影响边界

- 受影响页面：仅 `PAGE-TOPIC-001` 静态参考；未来 UI Agent 读取入口和治理约束。
- 受影响组件：无运行时组件；仅更新候选/Primitive/CMP 晋升规则。
- 受影响状态：合成/来源不足/本地意图；无真实五轴数据状态。
- 数据、权限、敏感展示、真实行动：不改变且不接入。
- 禁止修改的文件/能力：`apps/`、`crates/`、`database/`、F01 代码/测试/migration、`references/`、部署/配置、外部资产与任何真实 Producer/API。
- 停止条件：需声明/实现真实数据、权限、回执、产品动作、框架/技术路径、外部资产、生产部署或未被当前文档支持的视觉/组件规则时，停下并记录 `DECISION_REQUIRED` 或 `SOURCE_INCOMPLETE`。

### 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 页面规格与浏览器走查 | 2026-08-21 本机 Chrome 静态走查：首屏可持续读出 Topic、合成模式、限制与本地意图；7D/30D 只改变本地示例文本 | 非真实产品任务/工作流 |
| 状态诚实 | 每条合成样本与本地意图的持续边界检查 | 三条原声在展开前均有 `SYNTHETIC / NOT EVIDENCE`；窗口/意图都标为 synthetic/local，源文件无网络或写入代码 | 真实状态/权限/回执 |
| 视觉一致 | LIDS 专项检查、静态浏览器走查、响应式/Reduced Motion | LIDS Token 全量镜像和排版 Token 消费由专项脚本检查；Chrome 已在 1440px、375px 与 Reduced Motion 下走查 | 运行时组件/完整 L1/L2/L3/用户审美验收 |
| 真实后果 | 禁止任何网络/写入并检查源文件 | N/A：设计事项明确禁止 | 真实链路和部署 |

### 6. 交接

- 修改文件：本计划列出的 DESIGN-002 文档、LIDS 标准目录、设计治理/Agent 合同/模板/索引/进度、静态参考页与专项检查。
- 验证：`git diff --check`、LIDS/UI/Topic/项目治理检查、静态浏览器桌面/窄屏/键盘/Reduced Motion 走查。
- 例外与替代：旧 DESIGN-002 局部暗色视觉规则由 LIDS 替代；事实边界和第二独立页面晋升门保留。
- PR / reviewer / integration owner：draft PR #9；更新后必须新建独立 reviewer 审查最新 head，另由不同 integration owner 合并并在 main 复验。

## 已确认范围

- 页面：`Topic Intelligence Surface`；示例 Topic 是「任务启动困难」。
- 视觉方向：采用 LIDS v2.0 的“暖灰纸面上的精密情报基础设施”；ASCII 只作机器语义，L2 不使用 L3 大场景。此前“未来感 + ASCII + 新粗野主义 + 情报系统感”仅保留为已被 LIDS 收敛后的品牌意图，不再单独定义色值/页面骨架。
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
| LIDS foundation | `docs/design/lids/` | 全项目 Token、Primitive、Pattern、Agent、原型审计与迁移标准；成熟度仍为 Proposed |
| Reference foundation | `docs/design/foundation/topic-intelligence-visual-language.md` | 记录 PAGE-TOPIC-001 如何采用 LIDS，并加固合成边界 |
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
