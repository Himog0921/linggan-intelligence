# 开发阶段跟踪总表

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: Linggan Intelligence 从开发基线、合成事实链到真实运行与业务验收的阶段级跟踪
> 事实来源: `current-state.md`、已确认决定、当前活跃 SCOPE、真实代码/测试/数据库副作用与阶段计划
> 冲突时以谁为准: `AGENTS.md` 是最高约束；实现状态以可复现代码、测试、数据库副作用和真实运行结果为准；产品含义与授权边界以用户最新确认、ACCEPTED ADR 和当前活跃 SCOPE 为准；本表不替代任务级合同

## 这份文档解决什么

这是一张面向 Mog 的“项目施工总表”。它回答四个问题：

1. 项目现在走到哪一阶段；
2. 每个阶段要交付什么看得见的结果；
3. 什么证据齐全后才允许标记完成；
4. 哪些边界需要 Mog 决定，哪些由工程 Agent 按已批准合同执行。

四类跟踪入口分工如下，避免形成多套真相：

| 入口 | 负责回答 | 不负责回答 |
|---|---|---|
| 本文 | 全项目有哪些阶段、当前处于哪一阶段、阶段如何验收 | 单个工程任务的详细讨论和每日流水 |
| [`current-state.md`](current-state.md) | 当前唯一在办事项、眼下下一步和即时阻塞 | 展开完整多年路线 |
| GitHub Issues | 可以独立领取、实施和关闭的任务级工作单 | 改写产品、领域或阶段完成定义 |
| [`progress/`](progress/README.md) | 事情在什么时间真实发生，验证和提交结果是什么 | 代替当前状态或阶段路线 |
| [`migration/action-plan.md`](migration/action-plan.md) | 基础设计阶段形成的重建顺序与架构工作包参考 | 维护当前阶段状态或自动授予下一阶段开工 |

本文中的后续阶段只是路线和决策门，不自动授予实施权限。阶段必须先满足“进入条件”，再由对应 SCOPE 或明确授权开放代码、真实账号、插件、敏感材料、生产环境或外部行动。

## 当前阶段

> **当前处于 `DEV-02 / SCOPE-001：synthetic fact-kernel technical tracer`，状态为“执行中”；`ARC-001` 同时进行产品与系统架构决策，但不授权代码扩张。**

- 已完成：项目治理、开发环境、产品/领域/架构基础设计和代码前语义冻结。
- 已开始：只使用合成/脱敏材料的 `F01` 合同 TDD；已有 Package/Record hash、三类 canonicalization 攻击、RFC 8785 golden 和 F01 静态 manifest Oracle。
- 尚未完成：完整 F01 的 PostgreSQL atomic ingress、two Records、Observation/Current、loopback API、worker、minimal CLI 和本 tracer 验证门。
- 当前硬停止线：F01 完成后停止；`ARC-001` 未收口前，F02–F10、真实小红书访问、真实原文、插件升级、媒体、AI Agent crew、Topic/Corpus/Signal、Web 产品、生产部署和旧系统迁移均不开始。
- 当前权威执行文件：[`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md)。

## `ARC-001` 架构收口门

[`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) 是 F01 之后进入真实 producer、插件和首个用户可见切片前的决策门。它不重新讨论所有领域不变量，也不要求 Mog 判断框架、表、crate 或测试工具；只把真正改变产品形态、真实资源、隐私、成本和对外后果的问题交给 Mog。

`product-shell` 与 `primary-daily-job` 已 resolved：Linggan 是独立且完整的 Web 产品，旧内容工作台仅是历史原型和能力证据来源；日常先处理少量值得关注事项，Topic 负责深入。这不表示 Web、P0 或 Agent 已获实现授权。`p0-surface-prototype` 与 `first-producer-canary` 均已解锁但未回答、未授权实施。后续依赖顺序仍是：P0 页面原型 / producer Canary → Capture/Media 合同 → 第一阶段运行架构 → 首个用户可见 SCOPE。

## 状态和完成口径

### 阶段状态

| 状态 | 准确定义 |
|---|---|
| 未开始 | 只存在路线位置；没有当前实施授权，也没有可报告的执行结果 |
| 准备中 | 正在补来源、SCOPE、授权或验收条件；还不能报告代码阶段已开始 |
| 执行中 | 已获得有边界授权，正在按当前 SCOPE 实施 |
| 验证中 | 计划内实现已形成，正在运行完整验收与攻击性负例 |
| 已完成 | 该阶段全部退出条件均有可复现证据；不代表后续阶段或生产已完成 |
| 阻塞 | 已授权工作因外部条件无法继续，影响和恢复条件已记录 |
| 需要决定 | 必须由 Mog 决定产品含义、现实资源、敏感数据、对外行动或范围扩张 |
| 来源不完整 | 现有材料不足以安全设计或证明；先审计来源，不猜实现 |

### 六层完成证据

任何阶段都不能用一个“完成”覆盖下面六层。跟踪时分别记录：

| 层级 | 意义 | 当前项目示例 |
|---|---|---|
| 设计已确认 | 对应层次的含义和边界已经由用户确认 | DISC-001 只完成产品定义/领域不变量基线，不等于完整产品形态和物理架构已完成 |
| 代码已开始 | 已有按授权范围编写的实现 | F01 合同 TDD 已开始 |
| 自动验证通过 | 测试、静态门或合成 fixture 通过 | 只能证明相应测试范围 |
| 真实链路验证 | 真实 producer、插件、PostgreSQL 或真实对象按明确边界跑通 | SCOPE-001 目前只计划真实 PostgreSQL，不含真实平台 |
| 已部署 | 指定环境已部署并可访问 | 当前未部署新产品 |
| 用户/业务验收 | Mog 通过真实工作任务确认结果有用 | 当前未发生 |

只有对应阶段退出条件明确要求的层级全部有证据，阶段才可标记“已完成”。

## 全项目阶段总表

| 阶段 | 用户/业务结果 | 状态 | 进入条件 | 完成与验收条件 | 主要证据 | 决策/阻塞 |
|---|---|---|---|---|---|---|
| `DEV-00` 项目治理与开发底座 | 仓库可被新机器和 Agent 安全接手，环境和文档有唯一入口 | 已完成 | 项目启动 | GOV-001/002、ENV-001 的治理、Issue 跟踪、Rust/PostgreSQL 环境和验证均完成 | [`current-state.md`](current-state.md)、[`plans/completed/`](plans/completed/) | 无当前决策 |
| `DEV-01` 产品定义与领域不变量基线 | 确定 Linggan 要解决什么、相信什么、能说到哪一步 | 已完成 | 治理底座可用 | DISC-001 与 `USER-DEC-01`–`06` 确认产品内核、领域语言、责任和不变量 | [`product/domain-invariants.md`](product/domain-invariants.md)、[`plans/completed/disc-001-project-foundation-design.md`](plans/completed/disc-001-project-foundation-design.md) | 不代表第一版产品形态、页面数量或完整前端/后端/数据库/插件/媒体/Agent 架构已完成；这些由 ARC-001 收口 |
| `DEV-02` synthetic fact-kernel technical tracer | 在不接触真实平台数据的情况下，证明 Package → Record → Observation → Current → API/CLI 的最小事实内核可运行、可追溯 | 执行中 | SCOPE-001 已批准；代码门受控开放 | F01 的随机隔离 PostgreSQL atomic ingress、two Records、Observation/Current、loopback API、worker、minimal CLI 和三项主链保护全部通过 | [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md) | 不是用户可见产品切片；完成即 hard stop，F02–F10 不自动继续 |
| `DEV-03` 真实 producer 事实审计 | 知道插件在真实搜索页、作者页、详情和评论场景究竟能看到什么、缺什么、何时停止 | 未开始 | DEV-02 完成；ARC-001 的 product/workflow/Canary 边界 resolved；真实账号、对象、访问范围和原件处置获批 | 完成低风险探查；字段、时间、身份、终态、Coverage、账号差异和风控报告有真实证据；形成受限 Capture Contract | [`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md)、[`architecture/capture-plugin-architecture.md`](architecture/capture-plugin-architecture.md) | `AUD-XHS-001` 尚未授权；到达此门时需 Mog 批准真实访问与数据处置 |
| `DEV-04` 采集控制层与插件 Canary | 有限工位下先复用、去重、准入和拆解任务，插件只安全执行有界 Work Order | 未开始 | DEV-03 真实合同通过；插件升级 SCOPE 和单工位 Canary 获批 | 发现与深采分离；lease/reconcile/claim/renew/submit 链可恢复；部分结果入库且 Coverage 如实；单工位真实 Canary 通过 | [`architecture/capture-plugin-architecture.md`](architecture/capture-plugin-architecture.md) | 插件升级、账号/工位和真实平台访问均需单独授权 |
| `DEV-05` 真实 Observation 数据底座 | 对具有稳定来源身份的真实 Content/Author/Comment 等建立对象、历史 Observation、Current 和访问边界；身份不足材料保持 unresolved 或受限材料 | 未开始 | DEV-03/04 证明真实合同；按对象分切 SCOPE | 稳定身份对象的正常/失败/迟到/冲突/隐私处置可追溯；身份不足材料不产生 Source Object/Observation；Current 不覆盖历史；查询不把未知变成 0；真实 PostgreSQL 负例通过 | [`architecture/data-architecture.md`](architecture/data-architecture.md)、[`migration/action-plan.md`](migration/action-plan.md) | 禁止用 target、payload、昵称或文本 hash 兜底升级身份；物理模型逐 SCOPE 确认，不得整体迁入旧 schema |
| `DEV-06` 语义分析与受控 AI Agent | AI 能在固定材料、权限、预算和版本下分类、聚类、找反例并提出候选，不自行制造正式知识或扩采 | 未开始 | DEV-05 有可用真实材料；首个高价值 Agent 场景、材料权限和模型适配器获批 | 确定性分析与 Agent 解释分责；输出有引用/反例/未知；工具逐次授权；候选不自动发布；真实评测证明有用 | [`architecture/agent-architecture.md`](architecture/agent-architecture.md)、[`audits/pi-agent-kernel-upstream-assessment-2026-08-20.md`](audits/pi-agent-kernel-upstream-assessment-2026-08-20.md) | Pi 只是条件性 Adapter 候选；真实原文送第三方模型仍未授权 |
| `DEV-07` 情报应用工作台 | Mog 能从“今日关注”进入 Topic，查看原声、内容叙事、近期观察、依据/反例并决定下一步 | 未开始 | DEV-05/06 的读取合同和候选资格稳定；首个页面切片获批 | 页面通过同一 API 展示真实状态；Corpus/Topic Map/Radar/Claim/Brief/内容行动不复制第二真相；真实 5–10 分钟任务通过可用性验收 | [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md)、[`product/PRD.md`](product/PRD.md) | 页面名和首切片仍是草案；需按业务优先级逐项授权 |
| `DEV-08` 行动、Outcome 与外部 Agent | 决定、行动、可获得结果和评价可追溯；外部 Agent 只能在委托边界内读、分析、建议和申请 | 未开始 | 工作台和权限链稳定；明确哪些结果渠道真实可得 | Action/Outcome/Evaluation 分责；未接入渠道保持未知；外部 Agent 无数据库/插件旁路；真实调用与结果回执通过 | [`architecture/agent-architecture.md`](architecture/agent-architecture.md)、[`pages/product-interface-architecture.md`](pages/product-interface-architecture.md) | 深度评论、私信、咨询、销售、访谈和产品行为当前未打通；不能承诺自动闭环 |
| `DEV-09` 生产运行与旧系统退役 | 新系统承载真实日常工作；生产、业务验收和旧系统处置有清楚证据 | 未开始 | 前述阶段按真实工作流完成；生产/迁移/退役方案获批 | 生产部署可恢复；真实任务持续运行；Mog 完成业务验收；新旧系统不双写；旧库保持只读，关闭/删除另行决定 | [`migration/action-plan.md`](migration/action-plan.md)、[`migration/transfer-checklist.md`](migration/transfer-checklist.md) | 生产部署、历史数据迁移和旧系统退役均未授权 |

## `DEV-02 / SCOPE-001` 详细跟踪

F01 是当前唯一获准实施并计入 DEV-02 退出条件的场景。F02–F10 保留为经过设计的未来硬化/回归库存，不删除、不改写为通过，但在 ARC-001 收口和后续明确排期前均保持未开始；它们不能从 F01 自动开工。

### F01–F10 业务场景

| 场景 | 要证明什么 | 当前状态 | 当前证据 | 退出条件 |
|---|---|---|---|---|
| `F01` 完整合法包 | 已知集合完整交付后，Package、两个 Record、两个 Observation 和字段 Current 都可追溯 | 执行中 | 已有公开合同解析、Package/Record hash、duplicate key、unsafe integer、unpaired surrogate、RFC 8785 golden 与仅含 F01 的静态 manifest tracer | 完整合同/I-JSON/资源上限负例、数据库各阶段 Oracle、API/CLI 读取和真实 PostgreSQL 副作用全部通过 |
| `F02` 已知集合部分结果 | 目标 3 个只取得 2 个时保留两条好数据，并明确第三个未尝试，不冒充完成 | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 独立 fixture、数据库、API/CLI 都显示 `known_gap`，不为空缺对象造事实 |
| `F03` Replay | 同 identity/hash 重传只新增安全 delivery，复用原 Package/receipt | 未开始 | 只有 SCOPE 中的手工语义 Oracle | authority 过期后的合法 replay、行数和不可重复副作用通过 |
| `F04` Conflict | 同 identity、不同 hash 失败关闭且不覆盖历史 | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 冲突响应、审计和零业务副作用通过 |
| `F05` 接入拒绝族 | 多类错误按固定顺序拒绝，不泄露资源存在性、不产生假 Package/Observation | 未开始 | 只有 SCOPE 中的 15 类错误和 mutation 设计 | 每类独立失败测试、唯一错误码、pre-routing/routing 审计与事务故障通过 |
| `F06` 单 Record 隔离 | 一个坏 Record 不连坐同包好 Record；身份冲突和合同错误分责 | 未开始 | 只有 SCOPE 中的 F06A/F06B Oracle | 好成员形成 Observation，坏成员只形成自身处理结果，数据库副作用匹配 |
| `F07` 迟到 Observation | 迟到材料保留历史，但 Current 不按最后接收时间倒退 | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 两次独立 Work、append-only 历史和 Current 选择证明通过 |
| `F08` 来源差异 | 同一时刻不同来源冲突时保留双方，并把字段标为 unresolved | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 冲突来源集合、Current unresolved 和完整血缘通过 |
| `F09` 最大配额部分结果 | “最多 100、取得 50”表达剩余范围 unknown，而不是 50% 完成或平台共 100 | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 50 条合格数据入库，零伪造对象，Coverage/API/CLI 均保持 unknown |
| `F10` 混合 Record | 新对象、身份复用、unresolved、未观察字段在同包内各自正确处理 | 未开始 | 只有 SCOPE 中的手工语义 Oracle | 6 Record 的对象去重、5 个 Observation、1 个 unresolved 和 unknown 字段副作用通过 |

“SCOPE 中已有 Oracle”只表示设计已确认，不能计为 fixture、代码、数据库或真实链路通过。

### 实施层检查表

| 实施层 | 状态 | 完成证明 |
|---|---|---|
| 语义代码门 | 已完成 | 四轮攻击和 P0/P1 处置有记录；用户裁定 `CONTROLLED OPEN FOR TDD` |
| 合同与 fixture | 执行中 | 当前只要求 F01 主链使用的正负 Oracle、固定 golden 与保护通过；F02–F10 不在本 tracer 完成门中 |
| 空库 migration 与权限 | 未开始 | 随机 proof DB 从零重放；不可变历史、同属关系、DDL/UPDATE/DELETE 权限负例通过 |
| Package ingress | 未开始 | accepted/replay/conflict/rejected、authority、部分结果、事务故障和行数同时通过 |
| Record processing | 未开始 | F06/F10、identity 并发、unresolved、worker fence 和接管通过 |
| Observation/Current | 未开始 | append-only、F07/F08、字段来源和 pointer 原子发布通过 |
| API/worker/minimal CLI | 未开始 | 真实 loopback API + worker + PostgreSQL；CLI 只经 API 且诚实显示未完成/权限/断网 |
| 全部验证与文档 | 未开始 | SCOPE 的统一验证命令全部通过，证明与未证明范围同步记录 |

## Mog 如何查看项目

### 想在一分钟内知道“现在做到哪”

先看本文的“当前阶段”和“全项目阶段总表”，再看 [`current-state.md`](current-state.md) 的“当前下一步”。

### 想知道今天具体完成了什么

先看当月 [`progress/2026-08.md`](progress/2026-08.md)；如该工作已经拆出 GitHub Issue，再查看对应 Issue。2026-08-21 核对时仓库 Issues 为空，不为填满任务栏批量创建空 Issue；下一颗真实开发任务被拆分时才创建首个 Issue，并从 Tracker 或 `current-state.md` 回链。每项完成必须能打开代码、测试、数据库副作用或运行回执，而不是只看到一段总结。

### 什么时候需要 Mog 确认

Mog 只处理会改变下面任一事项的决定包：

- 产品可见含义或完成标准；
- 正式领域语言、Claim 或历史解释；
- 真实账号、工位、插件、敏感原文或第三方模型使用；
- 生产部署、历史数据迁移、外部 Agent 权限或现实行动；
- 明显扩大当前 SCOPE、成本、风险或传播范围。

纯技术实现、测试修复、同一 SCOPE 内的最小代码组织和可复现验证由工程 Agent 决定，不把技术试错推给 Mog。

## 后续 Agent 更新规则

1. 开始一个阶段前，先确认上游退出条件、当前授权和对应 SCOPE；路线存在不等于允许开工。
2. 阶段状态只在本文更新；当前下一步同步到 `current-state.md`，任务拆分写入 GitHub Issues，发生记录追加到当月 progress。
3. 每次状态变化必须附可复现证据链接，并分别说明设计、代码、自动验证、真实链路、部署和业务验收到哪一层。
4. 代码存在、测试绿、HTTP 200、任务结束、页面有卡片或 AI 生成文字都不能单独把阶段标为已完成。
5. 当前只更新并实施 F01；F02–F10 保留为未来硬化库存，ARC-001 收口和后续明确排期前不得自动开始。F01 tracer 通过也不能把整个合同、SCOPE-001 或 Evidence pipeline 标为完成。
6. 后续范围若需要真实 producer、Raw Artifact、插件、AI、Web、生产或旧数据，先建立新 SCOPE/授权，不把它塞入当前切片。
7. 遇到文档冲突按项目事实优先级裁定；无法裁定则标记“需要决定”或“来源不完整”，不综合出第三种含义。
8. 完成一个阶段后保留历史证据，只更新当前状态和路线，不重写当时失败、限制或未验证范围。

## 当前未授权或未验证清单

- 未验证真实小红书搜索、作者、详情、评论、账号差异、排序和七天生命周期规律；
- 未授权真实账号/工位、真实原文、插件升级或单工位 Canary；
- 未完成 SCOPE-001 的 PostgreSQL、API、worker、CLI 和 F02–F10；
- 未实现 Topic、Corpus、Signal、Claim/Brief、语义聚类、AI Agent Runtime 或 Web 工作台；
- Pi 仅为条件性执行 Adapter 候选，不是已引入的 Agent 内核；
- 未接通深度评论、私信、咨询、销售、访谈和产品行为等 Outcome 渠道；
- 未部署新产品，未完成用户/业务验收；
- 未授权生产部署、旧数据迁移、双写、旧系统关闭或删除。
