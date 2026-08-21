# 前端设计手册

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: Linggan Intelligence 未来用户可见 Web UI 的设计规则、页面/组件规格、协作入口与验收追踪
> 事实来源: 用户对 DESIGN-001 / DESIGN-002 的明确授权、AGENTS.md、docs/governance/、docs/agents/ui-execution-contract.md、已确认产品边界与当前文件树
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE；本手册不覆盖这些来源

本目录是未来 UI 工作的唯一设计入口。它的作用不是替产品决定页面要做什么，也不是把一组视觉偏好变成无需验证的实现命令；它把已经获准的界面规则、它们的来源、适用范围、验收方式和未决空白整理成 Agent 可以执行与复核的合同。

当前已有两层内容：DESIGN-001 建立设计治理骨架；DESIGN-002 为一个明确标为合成、非运行时的 Topic Reference Page 建立最小视觉语言、表达模式、组件晋升规则与页面规格。它们不构成通用 Web 技术选型、真实数据合同或完整前端实现授权。

## 固定读取路径

每次 UI 事项按以下顺序读取，且只进入与本次事项直接相关的下一层：

1. 根目录 AGENTS.md、docs/README.md 与 docs/current-state.md；
2. docs/agents/ui-execution-contract.md；
3. 对应的产品页面文档；它回答用户任务、事实含义、权限、状态与行动后果；
4. 本目录中状态为“权威当前”的相关基础规则、模式、组件或页面规格；
5. 已确认的数据合同、当前 SCOPE、真实代码或测试；只有要核对实际事实时才读取；
6. 明确登记的设计参考。参考只能辅助提出候选，不能覆盖前五层。

如果第 3–5 层没有给出足以实施的答案，必须停止并记录 DECISION_REQUIRED。不得用截图、旧项目、通用设计惯例、模型偏好或“页面看起来更完整”补齐缺口。

## 本手册与其他书架的边界

| 位置 | 回答的问题 | 不得承担的责任 |
|---|---|---|
| docs/pages/ | 为什么有这个入口、用户完成什么任务、信息和状态是什么意思 | 决定具体视觉细节或组件实现 |
| docs/design/ | 在既有产品边界内，页面、模式、组件和验收怎样保持一致 | 改写事实、权限、数据口径或产品目标 |
| docs/data-contracts/ | 页面可读取和写入什么、状态/版本/回执的合同是什么 | 按视觉需要发明字段或修改事实 |
| docs/agents/ | 协作 Agent 的读取、执行、停工、交接与审查流程 | 复制产品或设计真相 |
| docs/decisions/ | 长期有效的取舍、例外与替代关系 | 承担一次页面变更说明 |
| docs/progress/ | 已实际发生的变更和验证边界 | 成为当前设计标准 |
| references/ | 可核验的历史或外部证据 | 直接成为现行产品或设计指令 |

## 当前权威地图

| 文档 | 当前职责 | 状态 |
|---|---|---|
| [design-governance.md](design-governance.md) | 来源优先级、闭集执行、变更分类、例外、追踪与验收规则 | 权威当前 |
| [reference-register.md](reference-register.md) | 设计参考登记与引用边界 | 权威当前 |
| [../agents/ui-execution-contract.md](../agents/ui-execution-contract.md) | UI 协作 Agent 的强制工作合同 | 权威当前 |
| [templates/page-spec-form.md](templates/page-spec-form.md) | 首个获准页面切片的规格表单 | 权威当前 |
| [templates/component-spec-form.md](templates/component-spec-form.md) | 首个真实复用组件的规格表单 | 权威当前 |
| [templates/ui-change-manifest-form.md](templates/ui-change-manifest-form.md) | 实施前的最小来源与影响清单 | 权威当前 |
| [templates/visual-acceptance-form.md](templates/visual-acceptance-form.md) | 设计、状态、任务和真实后果的验收记录 | 权威当前 |
| [foundation/topic-intelligence-visual-language.md](foundation/topic-intelligence-visual-language.md) | DS-001–DS-007：Topic 参考页的视觉语言、ASCII 边界、动效与可访问性 | 权威当前 |
| [patterns/evidence-candidate-and-boundary-patterns.md](patterns/evidence-candidate-and-boundary-patterns.md) | PAT-001–PAT-004：观察、候选、来源限制与无副作用意图的表达模式 | 权威当前 |
| [components/component-promotion.md](components/component-promotion.md) | Reference Page 局部块如何经第二页面验证后才可晋升为 CMP | 权威当前 |
| [pages/topic-intelligence-reference-page.md](pages/topic-intelligence-reference-page.md) | PAGE-TOPIC-001：Topic Reference Page 的获准组合、状态与验收 | 权威当前 |
| [pages/topic-intelligence-reference.html](pages/topic-intelligence-reference.html) | 可本地打开的合成静态参考实现；不读取、写入或声称真实系统事实 | 权威当前 |
| [pages/topic-intelligence-reference-acceptance.md](pages/topic-intelligence-reference-acceptance.md) | ACC-TOPIC-001：静态视觉、互动和证明边界的实际验收记录 | 权威当前 |

## 未来按需扩展的书架

以下书架只在出现第一个已获批准、确有内容的对应规格时建立。DESIGN-002 已创建四个书架；这不等于它们已经拥有完整组件库或全局设计系统。

| 未来位置 | 创建触发条件 | 内容边界 |
|---|---|---|
| foundation/ | 已建立：DESIGN-002 有明确来源、范围和验收的 DS-001–DS-007 | 只放跨页面可采纳规则，不放某一页面的临时实现细节 |
| patterns/ | 已建立：DESIGN-002 的 PAT-001–PAT-004 可被明确采纳 | 只放稳定的表达责任，不放无来源的“通用 UI 习惯” |
| components/ | 已建立：组件晋升门槛；没有实际 CMP | 不得把 page-local candidate 当成公共组件或业务规则容器 |
| pages/ | 已建立：PAGE-TOPIC-001 是合成参考页的组合规格 | 只说明页面如何组合既有规则，不复制产品语义 |

## 版本、状态与替代

- 重要设计文档必须保持仓库统一的五项状态头，并使用稳定、无日期的文件名。
- 只有状态为“权威当前”的内容可约束新的实施；“草案”只能用于探索、讨论或原型。
- 每条可执行规则必须有稳定 ID，并在实施清单和验收记录中被引用。
- 当规则、组件或页面规格被替代时，旧文档保留并明确被谁替代；不得并存两个当前答案。
- 视觉截图、浏览器导出和自动测试产物属于生成物。没有登记前默认只放系统临时目录或项目允许的私有证据区，不得放入本目录或提交 Git。

## 当前未获授权的事项

DESIGN-002 只在 Topic 合成参考页的边界内决定 DS-001–DS-007、PAT-001–PAT-004 和 PAGE-TOPIC-001。它不决定全局品牌、完整组件库、字体资产、前端技术路径、真实数据字段、权限/行动合同、完整 P0 或任何生产页面实现。它同样不把现有页面草案、首页形态、架构图、旧工作台或外部产品转化为实现许可。

当首个 UI 切片进入明确的产品原型和 SCOPE 后，先创建最小的页面规格以及它实际依赖的基础/模式/组件规则，再开始编码。
