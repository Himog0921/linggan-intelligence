# AI Agent 内核、工具权限与评测架构

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 6 的 Agent Invocation、模型/提示版本、工具授权、材料输入、结构化输出、成本/停止、恢复、评测与候选治理
> 事实来源: 已确认产品/领域边界、DEC-01/03/05、Gate 5 数据候选、`module-architecture.md`
> 冲突时以谁为准: 用户最新确认、已确认委托/隐私/Claim/Decision 边界、真实脱敏样本 benchmark、运行时验证与数据库副作用；模型宣传和自然语言表现不构成成功证明

本文是 [`module-architecture.md`](module-architecture.md) 中 Agent Runtime 的下一层渐进披露。它不选择模型供应商，不创建 Agent、prompt、工具代码，也不授权把真实原文发送给第三方模型。

用户于 2026-08-20 提出把 [`earendil-works/pi`](https://github.com/earendil-works/pi) 作为全系统统一 Agent 内核的方向。当前上游适配审查裁定为候选 `CONDITIONAL ADAPTER`：Pi 可在未来 SCOPE 中承接模型与工具循环，Linggan 继续拥有准入、委托、材料、预算、持久任务、工具回执、隐私、结果资格和正式状态；各业务模块不直接依赖 Pi。版本、成熟度、禁用清单和验证门见 [`../audits/pi-agent-kernel-upstream-assessment-2026-08-20.md`](../audits/pi-agent-kernel-upstream-assessment-2026-08-20.md)。这项候选不改变本文的权限边界，也不授权当前实现。

用户补充的《统一Agent运行时架构》讨论强化了“系统驱动 Agent、同一 Runtime + 不同 Profile、领域工具优先、先业务价值后平台抽象”的方向。对抗归并后不新增万能 `Agent Task`：用户看到的研究工作继续由 `Research Question`、`Information Need`、`Analysis Run`、`Agent Invocation` 和必要时的 `Research Project` 分责；Profile 只是版本化调用模板，不是权限主体。首期只实现一个调用场景、一个 Linggan Agent Kernel Module、一个窄 `ModelToolLoopPort`、deterministic/Pi 两个 Adapter 和少量领域工具，不建设 Agent Control Plane 微服务、Profile 管理平台、Runtime Router 或多 Agent 编排。

## 一句话结论

> **Linggan 的 Agent 是一次有调用者、有目的、有材料边界、有工具权限、有预算、有停止条件和结构化结果的受控研究运行；它不是拥有数据库、插件和正式知识写权限的永久自治角色。**

AI 可以主动发现、解释、找反例、提出补证和行动候选，但不能靠提示词保证安全。每次工具调用、正式化、采集资源和对外行动都必须经过运行时授权和原责任模块。

## Agent 与算法流水线分责

```text
确定性/可重算分析
embedding、分类、聚类、统计、去重、相似性、异常检测
        ↓
有版本的 Analysis Result
        ↓
Agent 解释、质疑、组合、提出候选 Claim/补证/行动
        ↓
结构化 Candidate Output
        ↓
原模块验证 + 必要 Decision
```

- 聚类负责发现结构，Agent 可以命名、比较和找边界样本；聚类名称与 Agent 解释都不自动成为 Topic。
- 统计负责在明确输入、单位、分母和 Coverage 下计算；Agent 不能重新描述成更强的总体趋势。
- Agent 可以指出当前证据不足，不需要为了“完成回答”编造结论。
- 同一个 Agent 不同时改变观察面并用新数据证明自己；采集建议只形成 Acquisition Request proposal，进入正常准入链。

第一阶段不创建“语料 Agent、Topic Agent、市场 Agent、选题 Agent”等一页一个自治系统。不同产品任务使用同一 Runtime、不同 Invocation Profile、工具授予和输出合同；只有运行逻辑真正不同且接口更深时才形成新模块。

## Agent Invocation 的冻结输入

一次有后果的 Invocation 至少固定：

| 输入 | 必须说明 |
|---|---|
| 调用者与委托 | Human/System Actor、Agent Identity、Delegation revision、Domain、目的、期限与传播边界 |
| 任务 | 明确问题、允许结论强度、成功/停止条件、不能做什么 |
| 材料 | exact Material Pack 或受控 retrieval budget；每个成员的资格、版本和隐私状态 |
| 知识 | Domain Definition Release、采用的 Claim revisions、分析版本与 read-model 水位 |
| 工具 | exact Tool Grant manifest、每个工具允许的资源/数据/动作范围 |
| 模型 | provider/model、能力类别、参数、结构化输出方式和 fallback policy version |
| 提示与规则 | system/task prompt、政策、输出 schema、事实核查规则的不可变版本/hash |
| 预算 | token、金额、工具次数、材料数量、敏感访问、墙钟时间和最大步骤 |
| 结果去向 | 临时回答、保存候选、生成决策包、允许传播的受众 |

普通无后果浏览不强制永久冻结全部候选上下文，但仍记录必要的 Actor/Delegation、受限访问、模型/资源和回执审计。一旦回答被保存、共享、引用、用于 Decision 或触发资源申请，必须冻结足够复现链。

## 权限层级

权限按实际后果授予，不按“内部 Agent/外部 Agent”二分后永久继承：

| 等级 | 允许 | 不允许 |
|---|---|---|
| A0 生成 | 在调用者直接提供的脱敏输入上生成结构化草稿 | 读取系统材料、写入长期资产 |
| A1 受控读取 | 读取聚合、脱敏片段、Topic/Claim/Analysis 和受控 Evidence 引用 | 读取未授权原文、改变任何正式状态 |
| A2 候选分析 | 运行不改变正式世界的分析，保存 Candidate Claim/Topic/Narrative/Idea | 自动发布正式知识或判断 |
| A3 有边界申请 | 提交采集、知识修订、内容行动或补证申请 | 直接占用工位、改基线、发布内容、执行隐私处置 |
| A4 人确认后的模块动作 | 不是 Agent 自有权限；由 Decision 触发相应模块按正常合同尝试执行 | 把批准当成功、跳过运行时验证 |

A4 不是“超级 Agent 等级”。人批准后，Capture、Knowledge、Decision & Learning 等模块获得执行一项确切 revision 的资格；Agent Runtime 本身不因此得到直接写表能力。

## 工具目录

工具不是任意函数暴露。每个 Tool interface 包含：名称/version、输入/输出 schema、权限要求、资源成本、幂等语义、可能错误、敏感级别和审计规则。

### 首期可提供的读取工具候选

- 查询已发布 Topic/Term/Relation 版本；
- 检索经资格过滤的 Material Pack；
- 读取 exact Observation/Current provenance；
- 读取已 finalized Analysis Result；
- 读取 Claim revision、支持、反例和限制；
- 检查 Coverage、输入范围与结果渠道可获得性。

### 首期可提供的候选写入工具

- 保存 Candidate Claim；
- 提交 Knowledge Change Proposal；
- 提交有界 Acquisition Request；
- 保存 Content Idea revision；
- 提交补证或行动 Proposal；
- 把回答冻结成待人查看的研究/决策包。

### 永不直接暴露给 Agent 的能力

- 任意 SQL 或数据库连接；
- 未经 Access 模块授权的原始 blob/评论全文；
- 直接创建/领取插件 Work Order；
- 直接修改 Source Current、Topic Release、Claim adopted 或比较基线；
- 直接发布外部内容、发送消息、购买、删除或修改现实资源；
- 直接授予自己更高权限、更多预算或新的工具；
- 通过通用 HTTP/shell 工具绕过上述限制。

工具 adapter 调用拥有事实的模块接口。工具成功只证明该接口返回的具体 Outcome，不把“proposal saved”显示成“采集完成”或“知识成立”。

## Invocation 执行流程

```text
1. Admission
   验证 Actor、Delegation、目的、材料、模型政策与预算
        ↓
2. Context Freeze
   固定 Material Pack、定义/Claim/Analysis 版本和 Tool Grant
        ↓
3. Model Attempt
   产生结构化计划或下一工具请求
        ↓
4. Tool Gate
   每次重新验证工具、对象、用途、预算与当前隐私状态
        ↓
5. Tool Receipt
   持久化输入 hash、结果引用、错误、成本和幂等键
        ↓
6. Loop / Stop
   预算、完成、证据不足、等待决定、错误或安全停止
        ↓
7. Output Validation
   schema、引用、主张强度、反例、权限和传播检查
        ↓
8. Finalize
   冻结结果/候选/决策包，或明确 partial/failed/no-qualified-answer
```

模型不能自己宣布步骤 7 通过。结构化验证和业务模块负责核实引用存在、材料仍可用、Claim 资格没有被扩大、工具动作与输出一致。

## 结构化输出合同

第一阶段的研究型输出至少分开：

```text
Observed References
实际引用的 Observation / Material / Analysis

Candidate Claims
每条主张、范围、时间、支持、反例、限制和可推翻条件

Unknowns
当前材料不能回答什么

Alternative Explanations
观察方式、模型、来源集中或其他解释

Information Gaps
缺什么，以及已有 Evidence 为什么不够

Proposals
建议继续观察、补证、知识修订或行动，但不代表获准

User-facing Summary
只组织以上结构，不新增隐藏事实
```

重要事实性陈述必须逐条引用已有适用 Claim/Observation/Analysis，或被提取为 Candidate Claim。不能把整篇自由文本只因“输入有 Evidence”就标成有证据。

产品回应还要带 provenance envelope：数据/材料时间、捕获范围、Definition/Analysis/Model 版本、Coverage、权限过滤、已知缺失、允许结论等级和明确不能证明什么。

## 预算与停止

每次 Invocation 使用显式上限，不允许模型自己延长：

- 最大模型 attempts；
- 最大工具 calls 和每工具上限；
- 最大输入/输出 token 与金额；
- 最大材料成员、全文片段和敏感访问次数；
- 最大墙钟时间和循环步骤；
- 是否允许并行只读工具；
- 是否允许提交哪类 Proposal。

终止原因至少能够表达：

- 完成且输出合格；
- 证据不足，返回 unknown/gap；
- 等待人的 Decision；
- 材料被隐私阻断；
- 预算耗尽；
- 工具/模型暂时失败；
- 输出 schema 或事实引用持续不合格；
- 授权过期或撤回；
- 安全政策停止；
- 部分结果可保留但不能升级。

“最大步数到达”不伪装成研究已完成；“没有生成结果”不推断现实不存在。

## 重试、恢复和长任务

第一阶段使用 PostgreSQL durable work 和有限状态，不引入 Temporal、Camunda、Kafka 或复杂 Saga：

- 模型调用的每次 Attempt 独立记录 provider request/response 安全元数据、版本、成本和终态；
- transient retry 有有限次数和退避；结构化错误可以在相同输入/版本下有限修复；
- Tool call 使用幂等键和 receipt；恢复时先读取已完成 receipt，不重复执行有后果调用；
- Invocation checkpoint 只保存已完成步骤和待继续位置，不成为 Analysis/Claim/Evidence；
- 人工确认不让一个 Agent workflow 长期挂起。本次 Invocation 以 `awaiting_decision` 结果结束并产生决策包；确认后新 Invocation 引用旧结果和 exact Decision；
- model/provider fallback 只有政策预先允许且新 provider/model version 明确记录时发生，不能静默换模型后把结果当同一次可比运行；
- 无法自动恢复时形成可见失败/人工处置项，不无限重试烧预算。

如果以后真实工作证明存在跨天、多系统、复杂补偿和大量人工任务，再独立评估 durable workflow engine；不能因为“Agent”二字提前引入。

## 隐私与第三方模型

DEC-01 当前默认继续生效：

- 第三方模型处理合同未确认前，不发送受限原文；
- 默认使用经过用途选择和脱敏的 Material Pack；
- 脱敏不能只删昵称，还要考虑原话反向搜索和组合识别；
- 每次 Tool call 与 finalize 重新检查 Privacy Disposition；
- provider 请求日志、缓存、批处理文件和 tracing 不保存未批准原文；
- 隐私处置传播到 Invocation 输入、输出、引用、缓存和派生 Candidate；
- 已处置材料曾被 Agent 使用时，历史 Invocation 保留非敏感审计并标记受影响，当前输出停止返回相关原文。

是否允许任何 provider 处理原文、供应商是否用于训练/保留、区域、加密和删除回执必须在 Gate 5 隐私决定及专业审查后单独开放。

## Agent Runtime 模块结构候选

只有进入实现时才创建；文件建议按责任分开：

```text
agent-runtime/
├── lib.rs                 # 小接口与导出
├── invocation.rs          # Invocation identity / lifecycle
├── admission.rs           # delegation, purpose, policy, budget
├── context.rs             # exact material/knowledge version freeze
├── budget.rs              # token/cost/tool/time counters
├── loop.rs                # bounded orchestration
├── output.rs              # structured result + validation
├── audit.rs               # attempts, receipts, usage
├── model/
│   ├── mod.rs             # internal Model Port
│   ├── deterministic.rs   # tests
│   └── provider_*.rs      # only when chosen
├── tools/
│   ├── mod.rs             # registry + runtime gate
│   ├── read_*.rs
│   └── propose_*.rs
└── eval/
    ├── fixtures.rs
    ├── assertions.rs
    └── runner.rs
```

`loop.rs`、`tools/mod.rs` 和 provider adapter 同样受文件规模门禁；不创建一个几千行 `agent.rs`。Prompt 文本进入版本化资源/配置位置并有 hash，不散落在 Rust 字符串、route 和 worker 中。

## 评测体系

Agent 不能用“回答看起来不错”验收。评测分四层：

### 1. 合同与权限负例

- 缺 Actor/Delegation；
- 请求越 Domain、越用途或过期；
- Agent 尝试调用未授予工具；
- 读取受限原文或调用通用 HTTP 绕过；
- 提交 Proposal 后声称执行成功；
- 隐私处置与运行并发。

### 2. 证据忠实度

- 每条事实引用是否存在且适用于当前范围；
- 是否把一条评论外推总体；
- 是否把 Coverage 不完整、零命中或权限过滤解释成不存在；
- 是否把新关键词/模型变化解释成市场增长；
- 是否主动呈现最强反例和替代解释；
- 是否新增诊断、效果、因果或商业承诺。

### 3. 研究能力

- 是否识别真正影响结论的信息缺口；
- 是否复用已有 Evidence 而不是默认扩采；
- 是否区分用户表达、求助、不满足、行动和支付意愿；
- 是否在未知结构发现与已知 Topic 分类之间保持边界；
- 是否在预算内停止，并把不足说清。

### 4. 运行质量

- schema 合格率；
- 工具错误恢复率与重复副作用数；
- unsupported claim rate；
- 引用精确率/覆盖率；
- 反例检出与范围限定；
- token、金额、时间和工具调用分布；
- privacy/authorization violation 必须为 0；
- 人工实际采用、修改、拒绝和原因，但不把人工采用率当真理分数。

评测不要求自然语言逐字命中 golden answer。使用脱敏 fixture、结构化断言、攻击用例和人工 rubric；同一固定输入比较模型/提示变化，同一固定模型比较新世界材料变化。

真实 ADHD 评论 benchmark 必须在样本、用途、访问、输出和结束处置得到授权后进行。未执行前，具体模型、聚类算法、阈值和成本均保持 `SOURCE_INCOMPLETE / DECISION_REQUIRED`。

## 失败关闭与产品呈现

| 失败 | 系统返回 | 不能做 |
|---|---|---|
| retrieval 零命中 | 区分无候选、权限过滤、Coverage 不足和真正合格零结果 | 输出“用户没有这个需求” |
| 模型不可用 | 明确失败/稍后重试；保留已完成工具回执 | 静默换模型或输出模板假答案 |
| schema 不合格 | 有限修复后失败，保存诊断 | 把自由文本塞进正式对象 |
| 引用不存在/越界 | finalize 失败或删除对应 Candidate Claim | 只显示“AI 置信度低”继续发布 |
| 预算耗尽 | partial + gap + stop reason | 自动申请更多预算 |
| 需要采集 | 形成有边界 Request proposal | 直接创建插件任务 |
| 需要人工意义判断 | 决策包并结束本次 Invocation | 无限等待或自动确认 |
| 隐私撤回 | 立即停止读取/输出并触发传播 | 等缓存自然过期 |

## 第一阶段 Agent 能力边界

首个 Evidence 垂直切片不需要 Agent Runtime。Agent 的第一段实现必须等待：

1. World/Materials 能返回带 provenance、Coverage、unknown 和权限的结构化读取；
2. Claim Candidate 与 Proposal 不会被误当正式资产；
3. Access/Delegation 和隐私 gate 可运行；
4. 至少一套脱敏 fixture 与攻击性评测存在；
5. 选定一个真实、低风险用户任务，例如“基于固定 Material Pack 生成有引用的候选原声结构与信息缺口”；
6. provider/model 和数据处理条件经过明确决定。

第一版只开放 A1–A2：受控读取和候选分析。A3 采集/行动申请在 Capture 与 Decision 接口成熟后再开放；任何正式知识、资源执行和现实行动始终需要人决定和原模块执行。

## Gate 6 Agent 退出条件

1. Invocation 输入、工具、预算、停止、版本、结果和用途均有可持久化责任；
2. Agent 无直接数据库、插件、正式知识或外部行动旁路；
3. deterministic adapter 能覆盖权限、工具、结构化输出和恢复测试；
4. 真实模型 benchmark 有脱敏样本、成本、质量和失败证据，而非模型宣传；
5. 新事实性主张能够逐条验证，反例与 unknown 不被总结文本吞掉；
6. 隐私处置和授权撤回可以在运行中与运行后阻断；
7. 无无限循环、无限重试、无限扩采或静默 fallback；
8. 未为了 Agent 引入 Temporal、Kafka、微服务或一页一个 Agent；
9. 文件/接口门禁可以阻止巨型 `agent.rs`、万能 Tool 和供应商类型泄漏。
