# Pi 作为 Linggan Agent 内核的上游适配审查

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: Pi 作为 Linggan 统一 Agent 执行内核候选的上游成熟度、责任边界、接入方式与验证门
> 事实来源: Pi 官方发布版与 main 源码、安全/协议说明、Linggan 当前 Agent 架构、SCOPE 与实施就绪终审
> 审查对象: [`earendil-works/pi`](https://github.com/earendil-works/pi) 与 Linggan 当前 Agent 架构草案
> 上游快照: 发布版 `v0.84.2` / commit `914cf1472e715297caa30db4b9535d534a9eb718`；同时核对 `main` commit `5133c9284fe023436f4251fac2a6e8fb00a883b4`
> Linggan 快照: commit `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 加当前未提交文档修订
> 冲突时以谁为准: 当前 Linggan 业务不变量、权限/隐私合同、真实代码与数据库副作用，高于本报告；Pi 的具体能力以固定发布版源码和运行验证为准

## 给主线 Agent 的一句话结论

> **接受“Pi 作为统一 Agent 执行内核”的产品方向，但只建议把已发布的 `@earendil-works/pi-agent-core` 与按需使用的 `@earendil-works/pi-ai` 接在 Linggan 自有控制层之后；不接受 Pi 成为权限中心、业务事实来源、持久任务主账、插件调度者或正式知识裁定者。当前裁定是 `CONDITIONAL ADAPTER`，不是 `CORE OWNERSHIP`，也不授权现在开始 Agent Runtime 编码。**

更直白地说：Pi 可以成为“发动机”，但车辆的钥匙、路权、仪表、刹车、行驶记录和最终目的地仍必须由 Linggan 掌握。

## 1. 用户意图与本报告处理方式

用户提出：计划使用 Pi 作为整个内容情报系统的 Agent 内核，承接各个模块涉及 Agent 的工作。

本报告把这句话拆成两个层次，避免主线 Agent 将意图误写成过宽的实现授权：

1. **已明确的方向**：各模块不再各造一套模型循环，而是复用一个统一 Agent 执行能力；Pi 是首选上游实现。
2. **仍需技术边界保护的方式**：各模块不得直接依赖 Pi、直接把工具暴露给模型，或让 Pi 会话库成为业务主账；Pi 只能通过 Linggan 定义的窄接口工作。

本报告没有新增依赖、没有创建 Agent 进程、没有发送真实原文给模型、没有改数据库，也没有扩大当前 `SCOPE-001`。

## 2. 最终裁定

| 问题 | 裁定 | 解释 |
|---|---|---|
| 是否可以选 Pi 作为统一 Agent 执行内核 | **可以，条件化接受** | 低层 Agent、工具调用、流式事件、多模型适配有真实实现 |
| 是否让每个业务模块直接调用 Pi | **不可以** | 会把供应商类型、权限判断和会话状态散进各模块，形成多套事实 |
| 是否把 Pi 当 Linggan 完整 Agent 治理内核 | **不可以** | Pi 官方明确不提供内建权限隔离；Linggan 所需的委托、隐私、业务资格与正式化边界不由 Pi 提供 |
| 是否采用 `AgentHarness` 作为持久运行主干 | **当前不可以** | 发布版和核对的 main 中，大量核心入口显式返回 `HarnessNotImplemented` |
| 是否采用 `pi-server` / `pi-protocol` 作为首版服务合同 | **当前不可以** | 官方标记为 experimental，协议无兼容性保证，服务鉴权仍由应用实现 |
| 是否采用 Pi 的 SQLite 会话作为业务数据库 | **不可以** | 会与 Linggan PostgreSQL 产生第二事实来源；Pi transcript 只能是可丢弃执行材料或缓存 |
| 是否采用 Pi coding-agent 的 shell/文件系统工具 | **不可以** | 与 Linggan 永不暴露通用 shell/HTTP/数据库旁路的边界冲突 |
| 是否现在实施 | **不可以** | 当前 `SCOPE-001` 明确不含 Agent Runtime，且 Agent 的材料、权限、隐私、评测和低风险任务前置条件尚未满足 |

## 3. 上游事实证据

### 3.1 已经存在、可以利用的能力

Pi 的根 README 把 `@earendil-works/pi-agent-core` 定义为带工具调用和状态管理的 Agent runtime，把 `@earendil-works/pi-ai` 定义为统一多模型接口。发布版包为 `0.84.2`，要求 Node `>=22.19.0`，许可证为 MIT。

低层实现不是占位说明：

- `AgentOptions` 提供上下文转换、动态 API key、工具调用前/后钩子、每轮停止判断、steering/follow-up 队列和工具执行模式；
- `Agent` 持有当前 transcript、生命周期事件和工具执行；
- 每个工具使用类型化输入 schema，先验证参数，再进入 `beforeToolCall`；钩子可以阻断并终止；
- 工具执行后进入 `afterToolCall`，允许清洗或替换返回内容、详情、用量和终止标记；
- 工具可以顺序或并行执行，支持中间进度事件。

这组能力与 Linggan 需要的“受控模型/工具循环”有真实重合，因此重写一套同类 TypeScript 循环没有价值。

官方源码位置：

- [`README.md`：包定位](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/README.md#L13-L35)
- [`agent.ts`：AgentOptions 与低层 Agent](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/agent/src/agent.ts#L97-L237)
- [`agent-loop.ts`：工具参数验证、调用前拦截、执行和调用后处理](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/agent/src/agent-loop.ts#L600-L758)
- [`types.ts`：类型化工具合同](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/agent/src/types.ts#L360-L419)
- [`packages/agent/package.json`：版本、许可证与 Node 要求](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/agent/package.json#L1-L67)

### 3.2 Pi 不提供 Linggan 所需的安全边界

Pi 官方明确说明：它没有内建的文件系统、进程、网络或凭证权限系统，默认继承启动它的用户/进程权限；需要更强边界时，由使用者放入容器或沙箱。

官方安全策略进一步说明：

- 使用者负责监控或隔离；
- 本地用户、可写文件和 Pi 进程处于同一信任边界；
- `AGENTS.md`、注释、技能等造成的 prompt injection 可以轻易影响 coding agent，官方不承诺从机制上防住；
- sandbox、prompt injection、恶意模型输出等不属于其安全承诺范围。

因此，Pi 的 `beforeToolCall` 可以作为第二道拦截，但不能成为 Linggan 的唯一授权检查。每个工具请求都必须回到 Linggan 服务端，重新核对 Actor、Delegation、用途、对象范围、数据敏感度、预算、幂等键和当前撤权状态。

官方源码位置：

- [`README.md`：Permissions & Containerization](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/README.md#L38-L46)
- [`SECURITY.md`：信任边界与 prompt injection](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/SECURITY.md#L6-L22)
- [`SECURITY.md`：不在安全承诺内的项目](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/SECURITY.md#L48-L68)

### 3.3 高层 `AgentHarness` 当前不能承担持久运行主干

`AgentHarness` 暴露了一套看起来完整的接口：prompt、skill、compact、resume、abort、steer、follow-up、run-to-completion、watch 和 lanes。但源码实际状态是：

- 恢复已有 session 时直接抛出 `HarnessNotImplemented("create.restore")`；
- `prompt`、`skill`、`compact`、`navigateTree`、`resume`、`abort`、`steer`、`followUp`、`nextRun`、队列取消、用量记录、等待 idle、动作执行、run-to-completion、watch 和 lane 操作均调用 `unavailable(...)`；
- 本轮核对发布版 `v0.84.2` 与 main，上述文件没有差异。

所以，不能因为接口名称齐全就把它报告为可用的持久 Agent Harness。当前可采用的是低层 `Agent`/tool loop，而不是这层未完成的外观。

官方源码位置：[`agent-harness.ts`：create/restore 与核心入口未实现](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/agent/src/harness/agent-harness.ts#L305-L451)

### 3.4 Server 与 Protocol 仍是实验性组件

`@earendil-works/pi-server` 的 README 第一段明确写着 experimental、可能随时变化或删除、API 和行为尚不稳定；它要求 transport 在把连接交给 server 前自行完成认证与授权，并且不提供独立服务，应用仍须提供 `PiServerService`。

`@earendil-works/pi-protocol` 也是实验性协议。它的 snapshot/progress 分责是一个好的设计参考，但官方同时写明协议没有兼容性保证。

所以首版 Linggan 不应把这两个包公开成跨模块或长期兼容合同。即使内部试验，也必须被 Linggan 自有接口包住。

官方源码位置：

- [`packages/server/README.md`：实验状态、鉴权责任和应用自带 service](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/server/README.md#L1-L40)
- [`packages/protocol/README.md`：实验协议与无兼容保证](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/protocol/README.md#L1-L10)
- [`packages/protocol/README.md`：no compatibility guarantees](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/packages/protocol/README.md#L65-L69)

### 3.5 供应链表现较好，但不能代替本项目固定版本

Pi 上游记录了发布源码 checksum、精确锁定直接外部依赖、lockfile 审查、`npm ci --ignore-scripts`、npm audit/signature 和发布 smoke test。这是正面信号。

但 Linggan 仍必须：

1. 固定确切发布版和包完整性，不跟 `main`；
2. 不使用 `^0.84.2` 让安装时自动漂移；
3. 保留 Linggan 自己的 lockfile；
4. 每次升级重新跑接口、工具权限、停止、隐私和恢复的适配测试；
5. 不把“上游安全措施存在”误报为“Linggan 使用方式已安全”。

官方源码位置：[`README.md`：release source 与 supply-chain hardening](https://github.com/earendil-works/pi/blob/914cf1472e715297caa30db4b9535d534a9eb718/README.md#L63-L88)

## 4. 与 Linggan 当前架构的责任映射

### 4.1 Pi 可以拥有的范围

只限一次已获准 Invocation 内部的执行机制：

- 调用选定模型并接收流式输出；
- 维护本次运行所需的临时 transcript；
- 解析、验证工具参数 schema；
- 调用 Linggan 提供的窄工具 wrapper；
- 产生工具调用与生命周期事件；
- 执行有上限的模型/工具循环；
- 接受停止信号、steering 或 follow-up；
- 返回结构化的执行结果、错误和用量原始信息。

### 4.2 Linggan 必须拥有的范围

以下责任不得下放给 Pi：

- Invocation 身份、Actor、Agent Identity 和 Delegation revision；
- 任务目的、允许的结论强度和传播范围；
- exact Material Pack、Claim/Analysis/Definition 版本与 Coverage；
- Tool Grant、对象边界、数据敏感级别和每次工具调用的实时授权；
- token、金额、工具次数、敏感读取、墙钟时间和总步骤预算；
- 业务幂等键、Tool Receipt、Attempt、checkpoint 与 durable work；
- PostgreSQL 中的权威 Invocation、Attempt、Receipt、Candidate、Decision 和状态；
- 输出 schema、引用存在性、Claim 强度、unknown、反例和 qualification 验证；
- Candidate 是否提交、Proposal 是否获批、Decision 是否执行；
- 隐私撤回、材料到期、日志脱敏和审计保留；
- Capture/插件 Work Order、正式 Knowledge、Source Current 和外部现实行动。

### 4.3 各模块如何承接 Agent 工作

推荐的调用关系是：

```text
业务模块
  只提交 Agent Invocation Request
        ↓
Linggan Agent Runtime 控制层（Rust / PostgreSQL）
  准入、冻结上下文、预算、工具授予、持久记录
        ↓
Pi Adapter（Node / TypeScript，内部实现）
  使用 pi-agent-core 执行本次模型/工具循环
        ↓
Linggan Tool Gateway
  每次重新鉴权并调用原业务模块接口，保存 Tool Receipt
        ↓
Pi 返回候选输出
        ↓
Linggan Output Validator + 原业务模块
  验证、保存 Candidate/Proposal，必要时交给人决定
```

关键规则：

- 不建立“Topic Pi Agent”“Corpus Pi Agent”“市场 Pi Agent”等各自拥有状态的系统；
- 使用同一个 Runtime、不同 Invocation Profile、Tool Grant 和 Output Contract；
- 各模块依赖 Linggan 的 `AgentRuntime` 接口，不依赖 Pi 的 TypeScript 类型；
- Pi Adapter 可以替换，业务模块和 PostgreSQL 数据合同不随之改变；
- Pi 的会话/transcript 不成为业务事实。需要恢复时，由 Linggan 的权威记录重新组装最小上下文。

## 5. 建议的最小接口，不是实施授权

未来进入正式 SCOPE 后，业务侧只需要类似以下语义，不应看到 Pi：

```text
AgentRuntime
  submit(frozen_invocation) -> invocation_id
  status(invocation_id) -> authoritative_status
  cancel(invocation_id, actor, reason) -> outcome

ModelToolLoopPort
  run(execution_request) -> execution_outcome
```

`ModelToolLoopPort` 至少有两个实现，证明 seam 真实存在：

1. deterministic fake：用于权限、预算、停止、重放、输出资格和恢复测试；
2. Pi adapter：固定版本的真实执行适配器。

Pi Adapter 的输入应是已冻结、不可自行扩张的 `execution_request`；输出是标准化 `execution_outcome`。不得把 Pi 的 `AgentState`、session schema、provider response 或内部事件类型直接存成 Linggan 领域合同。

## 6. 首次试点的允许与禁止清单

### 可以进入候选试点的包

- `@earendil-works/pi-agent-core@0.84.2`：低层 Agent/tool loop；
- `@earendil-works/pi-ai@0.84.2`：仅在实际多 provider 需求成立时使用；
- telemetry 类型或事件：只有映射成 Linggan 自有安全事件后才能用。

### 当前不进入主干的部分

- `pi-coding-agent` CLI；
- coding-agent 自带 shell、文件系统、通用网络或任意命令能力；
- `AgentHarness` 作为持久运行/恢复实现；
- Pi SQLite session backend 作为 Linggan 主账；
- `pi-server`、`pi-client`、`pi-protocol` 作为公开或长期服务合同；
- Pi extension/skill 自动发现和执行；
- 把工作区 `AGENTS.md`、评论、网页原文直接当可信 system instruction；
- 任何 Pi 自动写正式 Knowledge、Source Current、数据库、插件 Work Order 或外部行动的路径。

## 7. 分阶段验证门

### Gate PI-0：决定冻结

主线必须确认这一句：

> Pi 是 Linggan 统一 Agent 执行适配器；Linggan 自己保留业务控制面和 PostgreSQL 事实面。

如果用户要求 Pi 同时拥有业务权限、任务主账或正式知识权力，需要重新审查，不能沿用本报告的 `CONDITIONAL ADAPTER`。

### Gate PI-1：离线 contract spike

只用完全合成、无敏感内容的 fixture，验证：

- 固定版本可以构造低层 Agent；
- 未授予工具必然被 Linggan gateway 拒绝；
- 参数 schema 错误不执行工具；
- 工具成功、失败、超时、重复和取消都有一份权威 Receipt；
- 达到步数/金额/时间上限必然停止；
- 输出不合格不能成为 Candidate；
- 删除 Pi 临时状态后，可以根据 Linggan 权威记录判断真实终态；
- deterministic fake 与 Pi adapter 通过同一组端口合同测试。

### Gate PI-2：低风险 A1–A2 试点

前提仍沿用当前 Agent 架构：World/Materials 有 provenance、Coverage 和权限读取；Candidate/Proposal 不会被误当正式资产；Access/Delegation 可运行；已有脱敏评测；provider 数据处理条件已明确。

首个任务只能是类似“基于固定脱敏 Material Pack，生成逐条引用的候选结构、反例和信息缺口”。不允许真实插件控制、补采执行、正式知识发布或原文批量输入。

### Gate PI-3：可靠性与替换证明

必须证明：

- 进程在模型调用前、工具执行中、工具成功后但回包前、输出验证前崩溃，都不会产生未知的重复副作用；
- 并发 cancel/撤权能关闭后续工具调用；
- provider 失败不会静默切换模型；
- Pi 升级失败可以回退到旧固定版本；
- 换成 deterministic adapter 或第二实现时，业务模块和数据库 schema 不变；
- 日志、trace、错误和 transcript 不泄漏 secret 或未授权原文。

只有 PI-0 至 PI-3 通过，才讨论扩大到 A3 Proposal；A4 永远不是 Pi 自有权限。

## 8. 版本与升级门禁

首次实现时建议记录：

| 项目 | 必须固定 |
|---|---|
| Git source | tag、full commit SHA、release date |
| npm packages | exact version，不用 caret/tilde |
| 完整性 | registry、tarball integrity、lockfile diff |
| Node | 实际生产 Node 大版本与最低要求 |
| API surface | 实际导入的 symbol 清单，不依赖未使用的上游面积 |
| 行为 | contract/eval/故障注入测试结果 |
| 安全 | 工具清单、运行用户、容器/网络/文件权限、secret 注入方式 |
| 数据 | provider、区域、保留/训练政策、发送字段和日志字段 |
| 回退 | 上一固定版本、回退步骤和不可回退的数据变化 |

每次升级只比较固定版本之间的 diff。以下任一变化必须阻断自动升级：工具执行顺序、生命周期事件、停止/取消、消息转换、重试、provider fallback、session schema、日志/telemetry、包导出、Node 版本或安全说明。

## 9. 对当前 Linggan 终审结论的影响

这项新增方向不会推翻正式编码前终审，也不会打开当前代码门：

- 它**减少未来重复实现风险**：Linggan 不必自行重写低层模型/工具循环；
- 它**新增一个必须冻结的外部 seam**：需要 Pi Adapter、版本门禁和上游替换条件；
- 它**没有解决现有 P0/P1**：真实 producer、Raw Artifact、数据库闭环、权限、隐私、fixture 和运行证据仍必须由本项目解决；
- 它**不能进入 `SCOPE-001`**：该 SCOPE 明确排除 Agent Runtime 和 LLM SDK；
- 它**不授权真实数据处理**：真实原文是否可发送给任何 provider 仍是独立的隐私与数据处理决定。

因此当前 Gate 结论仍是：`Agent Runtime implementation = CLOSED`；`Pi upstream direction = CONDITIONALLY ACCEPTABLE FOR A FUTURE SCOPE`。

## 10. 主线 Agent 复核清单

主线核实时不要只确认仓库存在或 README 自称 Agent harness。至少逐项回答：

| 复核项 | 必须看到的证据 | 不合格时的结论 |
|---|---|---|
| 发布基线 | tag、full SHA、npm exact version、integrity | 版本未冻结 |
| 低层能力 | Agent/tool schema/before/after/stop 的实际源码或运行测试 | 不得采用 |
| Harness 成熟度 | 核心方法是否仍返回 `HarnessNotImplemented` | 不得作为 durable 主干 |
| Server/Protocol 稳定性 | 官方 experimental/compatibility 状态 | 不得成为长期公开合同 |
| 权限边界 | 每次工具调用由 Linggan 服务端重验并留 Receipt | P0，禁止真实工具 |
| 事实边界 | PostgreSQL 是唯一权威主账，Pi session 可丢弃 | P0，禁止进入主干 |
| 输出资格 | schema、引用、Coverage、unknown、Claim 强度由 Linggan 验证 | P0，禁止形成正式候选 |
| 故障恢复 | 四个崩溃窗口、取消、重放和幂等有测试 | P1，禁止扩大范围 |
| 数据处理 | provider、用途、原文字段、保留、日志和删除已批准 | P0，禁止真实原文 |
| 可替换性 | fake/第二 adapter 通过同一合同，业务模块不依赖 Pi 类型 | P1，禁止称为深模块 seam |

推荐复核状态词：

- `CONFIRMED`：官方固定版本与本地测试仍支持本报告；
- `STALE`：上游新版本已完成 Harness 或稳定协议，但需要新审查，不能自动改成通过；
- `DISPROVED`：固定版本或源码证据与本报告相反；
- `DECISION_REQUIRED`：用户要求扩大 Pi 所有权，或真实数据/provider 边界尚未决定。

## 11. 需要用户确认的唯一架构句

为避免非技术产品决定被实现 Agent 扩写，建议用户只确认下面这句，不需要决定具体进程、文件或协议：

> **整个系统统一使用 Pi 承接 Agent 的模型与工具循环；所有业务权限、材料边界、预算、持久任务、工具回执、结果资格、正式知识和现实动作仍由 Linggan 自己控制。各模块通过 Linggan 的统一 Agent Runtime 调用，不直接依赖 Pi。**

确认这句后，主线可以在未来独立 SCOPE 中修订 `agent-architecture.md` 的内部实现候选，把原本可能重复建设的 Rust `loop.rs`/provider 实现改成 Linggan 控制层 + Pi Adapter；仍不能跳过 PI-1 至 PI-3。

## 附录 A：对《统一 Agent 运行时架构》讨论的对抗性归并

### A.1 材料身份

用户提供的 ChatGPT 讨论《统一Agent运行时架构》属于产品与架构思考输入，不是官方 Pi 能力证明、当前代码事实或自动生效的实施合同。本附录只吸收其中经当前 Linggan 领域合同和 Pi 官方源码支持的内容；讨论中的引用标记、产品例子和未来名称不作为独立证据。

### A.2 可以直接采纳的洞察

| 讨论观点 | 审查结论 | 落地含义 |
|---|---|---|
| 不要为未来治理先建设企业级 Agent 平台 | **采纳** | 首个 Agent SCOPE 只做一条低风险研究闭环和真实需要的最小 Module，不创建通用编排平台、管理后台、工作流引擎或多租户控制面 |
| Pi 是推理执行器，不是知识库、记忆库、决策系统或情报系统 | **采纳** | Pi 只拥有本次执行所需的模型/工具循环；Linggan PostgreSQL 与领域模块拥有事实、历史、资格和正式状态 |
| 不是 Agent 驱动系统，而是系统驱动 Agent | **采纳** | Linggan 先形成有目的、有材料、有权限的研究/分析请求，再允许 Pi 执行；Pi 不自行寻找全库任务或扩大研究范围 |
| 不建设 Corpus/Topic/Market 等多个自治 Agent | **采纳** | 使用同一 Runtime，不同 Invocation Profile、Tool Grant、Material Scope 和 Output Contract |
| 工具应表达领域能力，不暴露 SQL/文件/通用 shell | **采纳** | 工具调用进入原 Module interface，例如读取带资格的 Topic 历史或 Coverage；Pi 不理解表结构，也不能绕过 Access |
| Pi 输出先是候选解释，不是正式情报 | **采纳** | 输出必须经过引用、Coverage、Claim 强度、反例、unknown 和权限验证，再由原模块或人决定是否长期保存/采用 |
| 每次有后果调用需要可追溯账 | **采纳，但不新增万能 Ledger** | 使用 Linggan 已设计的 Invocation、Attempt、Tool Receipt、Material Usage、Candidate/Decision 关系保留审计，不预设单张万能 ledger 表 |
| 先验证业务价值，再扩大抽象 | **采纳** | 先完成合成 contract spike 和一个脱敏低风险 A1–A2 任务；只有第二个真实调用方出现共同需求时才加深或扩大接口 |

这些洞察把 Pi 的长期价值从“模型更聪明”修正为三件更稳定的事：

1. 统一一次 Agent Execution 的机制；
2. 让工具、材料、停止和输出资格可验证；
3. 让模型或 Pi 将来可替换，而不重写 Linggan 领域。

### A.3 必须修正的讨论结论

#### 修正 1：不新增万能 `Agent Task` / `Research Task` 核心对象

讨论提出：

```text
Research Task
    ↓
Agent Invocation
    ↓
Pi Execution
```

它对产品语言有启发，但不能直接变成新的持久对象或统一状态机。Linggan 当前已经分开：

- `Research Question`：这次要弄清什么、范围和停止语义；
- `Information Need`：当前判断还缺什么；
- `Analysis Run`：一次确定性或模型分析实际运行了什么；
- `Agent Invocation`：一次受控 Agent 调用；
- `Research Project`：只有跨多轮问题、多人或长期材料组织时才出现的可选工作容器，且是否需要持久身份仍由真实生命周期决定。

如果再增加万能 `Agent Task`，它很容易吞并问题、Need、Run、Invocation 和状态，重新制造“一个 completed 代表五种完成”的语义压缩。

因此产品界面可以把一次研究工作显示成“研究任务”，但 Agent 和数据实现必须映射到上述现有责任，不据此自动创建 `agent_tasks` 表、Rust crate、任务中心或统一状态机。

关系也不应固定成一对一：

```text
Research Question  0..N  Analysis Run
Research Question  0..N  Agent Invocation
Analysis Run        0..N  Agent Invocation（按具体方法）
Agent Invocation    1..N  Pi Execution Attempt
```

一个问题可能完全由现有 Evidence 和确定性分析回答，不需要 Pi；一次临时受控 Agent 查询也不必创建 Research Project。

#### 修正 2：Pi 不只位于名为 `Analysis Candidate` 的单一阶段

讨论用“Pi 只在 Analysis Candidate 阶段工作”表达不让模型直接形成正式情报，这个动机正确，但位置描述过窄。

Pi 在未来 A1–A2 范围内可以协助：

- 受控材料检索和问题澄清；
- 候选解释、反例、信息缺口和比较方案；
- Evidence/Claim 审计候选；
- 形成 Acquisition 或 Knowledge Change Proposal 草案。

统一边界不是“只能在哪个页面或阶段出现”，而是：**Pi 只能返回临时结果、Candidate 或 Proposal；不能直接改变 Observation、Current、Claim adopted、Brief release、Work Order、Decision 或现实 Action。**

#### 修正 3：“先做真实案例”不能跳过合成安全门

讨论建议先做一个 ADHD 作业拖延真实案例再抽象 Runtime。业务价值导向正确，但执行顺序必须改为：

```text
冻结最小合同
  ↓
合成 fixture + deterministic adapter
  ↓
Pi adapter 离线 contract spike
  ↓
权限、预算、幂等、停止、输出资格负例通过
  ↓
脱敏、低风险 A1–A2 用户任务
  ↓
人工评估是否产生实际研究价值
  ↓
出现第二个真实调用方后再决定进一步抽象
```

未经 provider 数据处理决定、Access/Delegation、Material provenance 和攻击性评测，不能直接把真实评论或其他原文交给 Pi。

#### 修正 4：`Agent Profile` 是版本化调用模板，不是新权限主体

讨论中的 Research Analyst、Evidence Auditor、Collection Planner 可以成为 Profile 候选，但 Profile 只能预填目的、默认工具集合、输出 schema 和预算上限。最终权限仍由本次 Actor/Delegation、Material Scope、Tool Grant 和当前隐私状态共同决定。

Profile 不能：

- 永久持有原文访问权；
- 替代调用者或 Delegation；
- 静默继承上一次 Invocation 的工具；
- 因名为 Auditor 就自动获得全库读取；
- 因名为 Planner 就直接创建 Work Order。

为了减少首期面积，第一条真实链甚至只需一个明确的 Invocation Profile；没有第二种稳定差异前，不先建立 Profile 管理系统。

#### 修正 5：Invocation 账本是一组责任，不先实现成平台

讨论列出的调用者、材料、Coverage、模型、Pi/prompt/policy 版本、工具、成本、验证、限制和 downstream disposition 都值得追踪。但首期不需要：

- 单独的 Agent 管理后台；
- 通用事件总线；
- 任意查询的完整思维链保存；
- 把 provider 原始响应、secret 或未批准原文永久落库；
- 一张容纳所有字段和状态的万能 ledger 表。

只保存复现业务后果和审计所需的最小安全事实。模型隐藏推理不是证明材料；可核实的输入、工具回执、结构化输出、引用、验证和最终处置才是。

### A.4 最小首期形态

讨论中的六层架构可以作为长期责任图，但首期物理实现必须收缩为三个实际模块和一个测试 Adapter：

```text
调用方（先选一个 Research Question / Analysis 场景）
        ↓
Linggan Agent Kernel Module
  冻结 Invocation、检查权限/预算、保存最小权威状态、验证输出
        ↓
ModelToolLoopPort
   ├── Deterministic Adapter（测试）
   └── Pi Adapter（固定发布版）
        ↓
少量 Linggan Domain Tools
  只读材料/证据；首期最多保留 Candidate，不执行正式动作
```

首期明确不创建独立的：

- Agent Control Plane 微服务；
- Policy/Profile 管理产品；
- Tool Marketplace；
- Runtime Router；
- Prompt CMS；
- 多 Agent 编排；
- Agent 记忆系统；
- Agent 运维大盘；
- 通用 Research Task engine。

删除测试适配器后，如果复杂度会重新散到 Pi Adapter 和调用方，`ModelToolLoopPort` 才是真 seam；只有 Pi 一个实现且没有合同测试时，它仍只是包装层。

### A.5 归并后的架构原则

讨论里最有价值的一句经过本轮证据修订后，建议冻结为：

> **Linggan 建设可追溯的研究与判断系统，不建设泛化 Agent 平台。Pi 只执行一次已获准的智能推理循环；Linggan 定义问题、提供合格事实、限制能力、记录业务后果、验证候选并决定是否沉淀知识。**

这句话比“Pi 是整个系统的大脑”更准确，也比“先建设完整 Agent Control Plane”更符合当前项目阶段。

---

## 审查限制

- 本轮检查了官方仓库源码、发布 tag、当前 main 与安全/协议说明；没有执行真实 provider 调用或上游全量测试。
- 没有评估模型质量、成本、中文表现或真实 ADHD 原文效果；这些必须在获得数据处理授权后用脱敏 benchmark 单独验证。
- 上游变化很快。任何实施必须重新锁定当时的发布版，并把本报告标记为当前或过期，不能只复用 `2026-08-20` 的结论。
