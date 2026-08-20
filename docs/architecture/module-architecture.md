# Rust 模块、接口与依赖架构

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 6 的模块化单体、Rust 模块接口、依赖方向、adapter seam、测试表面与文件规模门禁
> 事实来源: Gate 1–5 已确认边界与候选数据架构、当前 Bootstrap workspace、固定 V2 参考代码
> 冲突时以谁为准: 用户最新确认、ACCEPTED ADR、已确认领域/数据边界、真实 producer/fixture/测试与运行结果；当前 Bootstrap crate 名称不具有架构优先权

本文是 [`target-architecture.md`](target-architecture.md) 的 Gate 6 渐进披露子文档。它不授权创建 crate、移动源码或开始业务实现；先确定哪些复杂性值得被一个小接口隐藏，以及哪些接缝是真实存在的。

AI Agent Runtime 的委托、工具、预算、停止、恢复、结构化输出和评测继续进入 [`agent-architecture.md`](agent-architecture.md)；本文只保留模块 seam 与依赖方向。

采集准入、有限工位、Work Order/Attempt/lease、部分结果、MV3 插件、离线恢复与协议升级继续进入 [`capture-plugin-architecture.md`](capture-plugin-architecture.md)；本文只保留 Capture 深模块责任。

API/worker 运行、Durable Work、scheduler、重试/接管、Outbox、可观测性、数据库角色、部署和恢复继续进入 [`runtime-operations-architecture.md`](runtime-operations-architecture.md)；本文只保留组合入口与依赖方向。

## 一句话结论

> **第一阶段采用一个 Rust 模块化单体、一个 PostgreSQL 16 主数据库、API 与 worker 两个组合进程；按不变量和事务深度组织模块，不按页面、数据库表或每个领域名词创建 crate。**

浏览器插件继续是轻量 TypeScript/MV3 平台执行器；AI 模型供应商、插件协议和对象存储位于真实外部接缝。PostgreSQL 是本地可替代依赖，核心模块使用真实 PostgreSQL 16 集成测试，不为了 mock 建一层浅 Repository 接口。

## 当前 Bootstrap 的资格

当前 workspace 只有最小占位：

```text
apps/api
apps/worker
crates/contracts
crates/domain
crates/evidence
crates/observation
crates/intelligence
crates/storage-postgres
```

这些目录证明 Rust workspace 能编译，不证明最终模块边界。特别是：

- `evidence` 与 `observation` 是否保持两个 crate，必须看事务、依赖和接口深度；
- `intelligence` 不能成为 Topic、Corpus、聚类、Claim、Brief、Agent、Action 和 Outcome 的巨型垃圾桶；
- `storage-postgres` 不能演变成所有模块都穿透调用的万能 CRUD 层；
- `domain` 不能成为所有人随手放 enum、DTO 和 helper 的共享杂物箱；
- `contracts` 只保存真正跨进程/跨仓的版本化 wire contract，不复制内部领域模型。

Gate 6 确认前不重命名或新增 crate。首个垂直切片只创建它实际需要的模块；后续模块在有真实调用者、事务和测试表面时再加入。

## 部署形态

```text
                  Web / CLI / external Agent
                              │
                              ↓
                         linggan-api
                    HTTP/CLI adapter + composition
                              │
              ┌───────────────┼────────────────┐
              ↓               ↓                ↓
       deep application   read modules    agent invocation
          modules                            control
              │               │                │
              └───────────────┼────────────────┘
                              ↓
                       PostgreSQL 16
                              │
                        durable work
                              ↓
                        linggan-worker
                              │
             ┌────────────────┼────────────────┐
             ↓                ↓                ↓
      source processing   analysis jobs   privacy propagation

Browser MV3 plugin ── versioned contract ── linggan-api / capture module
Object storage      ── blob adapter seam ── evidence implementation
Model providers     ── model adapter seam ── agent runtime
```

API 与 worker 不是两个产品，也不各自拥有业务逻辑。它们只是不同组合入口：

- API 负责认证、输入限制、调用模块接口和把模块结果映射成产品回应；
- worker 领取数据库 durable work，调用相同模块接口并回写结果；
- 定时器只产生候选工作或唤醒 worker，不直接执行业务写入；
- 任何真实副作用都必须通过拥有该不变量的模块接口，不允许 route、cron、Agent tool 或插件回调直接写表。

## 模块地图

下面是逻辑模块，不等于立即创建同名 crate。每个模块只有一个面向调用者的接口；内部可以拥有多个私有 seam 和文件。

| 模块 | 小接口负责什么 | 隐藏的复杂性 | 明确不拥有 |
|---|---|---|---|
| Capture | 评估/领取有界工作、建立 Attempt、接入终态 Package、返回可审计结果 | Need/授权/准入、lease、replay/conflict、authority fence、Package 事务、Coverage、durable work | Source Current、Topic、AI 判断、业务 Action |
| World Observation | 从已接纳 Record 解析稳定来源身份，追加类型化 Observation，发布来源 Current | 并发 identity、unresolved、时间精度、来源差异、parser revision、迟到数据、Current resolution | Topic 分类、市场意义、采集授权 |
| Knowledge | 管理 Domain、Topic/Term/Relation revisions 与 Definition/Map releases | 改名/拆分/合并、循环验证、实验词表隔离、发布与 ready 分离 | 聚类结果、趋势、Corpus 原文 |
| Materials | 按用途选择/转换/检索材料，冻结 Material Pack 和实际 Usage | Fragment/hash、脱敏、转换血缘、零命中/过滤/unknown、当前使用资格 | Evidence 原文真相、Claim、Agent 权限决定 |
| Analysis | 冻结输入并运行分类、聚类、统计和解释候选 | 输入 manifest、模型/规则/参数版本、分片、Coverage、finalize、可比性限制 | 正式 Topic、Claim 采用、采集扩张 |
| Intelligence | 管理 Claim revision、Signal attention、Insight/Brief release | 支持/反例、冲突 Claim、影响评估、发布版本、不能证明什么 | Evidence、模型执行、现实 Action |
| Decision & Learning | 固定有后果的决定、Action plan/attempt/occurrence、Outcome 与 Evaluation | exact revision、事前预期、外部动作证明、迟到/缺失结果、避免幸存者偏差 | 直接修改 Topic/Claim、平台采集执行 |
| Access & Privacy | 评估 Actor/Delegation/用途访问，执行隐私阻断和传播 | 委托版本、原文资格、Disposition、消费者回执、最小 tombstone | 业务内容副本、跨模块万能状态 |
| Agent Runtime | 在明确委托、材料包、工具和预算下执行版本化 Agent invocation | 模型 adapter、结构化输出、工具循环、成本、停止、重试、评测与调用审计 | 直接发布正式知识、直接写数据库或指挥插件 |
| Read Models | 为页面、CLI 和 Agent 构建可重建投影与 provenance envelope | freshness、水位、版本组合、权限过滤、unknown 表达 | 任何无法从权威历史恢复的事实 |

首切片不需要同时实现全部模块。未被真实场景调用的模块只保留文档责任，不创建空 crate、空 trait 或预留表。

## 依赖方向

### 允许的方向

```text
apps/api, apps/worker
        ↓
application use-case composition
        ↓
Capture / World / Knowledge / Materials / Analysis /
Intelligence / Decision & Learning / Access & Privacy
        ↓
small stable primitives + internal adapters

Analysis ──→ Materials / World / Knowledge / Agent Runtime
Intelligence ──→ finalized Analysis results / Materials / World / Knowledge
Decision & Learning ──→ exact Intelligence or Idea revisions
Read Models ──→ read interfaces and immutable/projection records
```

Capture 可以产生“Record 已接纳”的 durable work，World 消费其不可变引用；World 不反向调用 Capture 决定任务状态。Decision 可以授权采集申请，但不直接创建 Work Order；它通过正常 Capture 接口进入准入。

### 禁止的方向

- 领域模块依赖 Axum route、CLI parser、Chrome 消息或页面 DTO；
- 插件直接依赖内部数据库模型或 SQL；
- Analysis/Agent Runtime 直接写 Knowledge Release、Claim adopted、Work Order 或 Action Occurrence；
- Read Models 被写回成为 Evidence/Observation；
- `storage-postgres` 暴露万能 `insert/update/find_by_id` 给所有调用者；
- `domain`/`common`/`utils` 反向依赖任何具体业务模块；
- 模块之间通过复制同一 DTO 避免清晰转换；
- 为消除编译依赖而改成不受约束的 JSON 或字符串 ID。

## 真正需要的接缝与 adapter

### 1. 浏览器插件：真实外部接缝

插件是独立进程、独立仓库/发布物、可能离线和版本落后，因此需要版本化 wire contract 与两个 adapter：

- 生产：HTTP/消息 adapter；
- 测试：fixture/in-memory producer adapter。

Capture 模块拥有 Work Order、Attempt、Package 和 receipt 语义；插件 adapter 只翻译协议，不复制准入、幂等或 Evidence 规则。

### 2. 模型供应商：真实外部接缝

Agent Runtime 定义最小 Model Port，例如结构化生成、embedding 或 rerank 按真实需要分别出现；不要先做万能 `AIProvider`。生产 provider adapter 与 deterministic fake/mock adapter 形成真实 seam。业务模块只看到版本化 invocation 结果，不看到供应商 SDK。

### 3. 对象存储：真实可替换接缝

当首个 Artifact 需要保存字节时定义 Blob Store Port；本地文件/测试 adapter 与正式对象存储 adapter 共同证明 seam。接口只表达 put/get/delete/hash/receipt 等需要，不泄漏某供应商 bucket 细节。

### 4. PostgreSQL：内部依赖，不建立外部 Repository 层

PostgreSQL 16 是部署内唯一权威数据库，并且本地 Docker 可运行。模块实现可以在私有 adapter 中使用 SQLx，测试通过真实临时 PostgreSQL schema/database 验证。模块外部接口不暴露 SQLx type/transaction，但也不为了单元测试给每张表创建 repository trait 和 in-memory fake。

只有一个存储行为确有第二种 adapter 时才提升成 seam。当前 `storage-postgres` 可以保留连接、事务/错误和测试辅助的共享实现候选，但业务查询与事务应靠近拥有不变量的模块；若它只转发 CRUD，应删除或收缩，而不是继续加层。

## 模块接口形状

以下只是接口形状，不是最终 Rust 名称或方法列表。

### Capture

```text
admit(request context) → Admission Outcome
claim(station capability) → Lease or No Work
submit(package envelope + bytes/reference) → Ingress Receipt
```

renew、pause、recover 等只有真实插件流程证明后才进入外部接口；能藏在 claim/submit 实现里的细节不暴露给所有调用者。

### World Observation

```text
process(accepted record reference) → Record Processing Outcome
resolve_current(source identity, read purpose) → Provenanced Current
```

parser、identity candidate、discrepancy 和 projection builder 是内部 seam，不让 API 调用者逐步指挥。

### Knowledge

```text
propose/change draft（由应用用例组织）
publish(decision + exact manifest) → Release Receipt
read(release/version query) → Versioned Knowledge
```

具体编辑命令应按真实 Topic 工作流形成，不先建立几十个 CRUD 方法。

### Analysis / Intelligence

```text
Analysis: prepare/finalize versioned run → typed result references
Intelligence: revise claim / publish brief → immutable revision or release receipt
```

Agent 不跨过这些接口直接更新表。模型输出只是 Analysis Result/Candidate，正式 Claim/Brief 仍经过 Intelligence 接口和治理。

### Access & Privacy

```text
authorize(actor + delegation + purpose + material/action) → bounded decision
dispose(authorized privacy case) → immediate block + propagation receipt
```

调用者不能传 `is_admin=true` 或自己声明用途即可读取原文。

## 应用组合层

需要跨多个模块的用户任务由少量 use-case module 组织，例如：

- 请求一项研究并判断是否需要新采集；
- 接收 Package 后安排来源处理；
- 在 Topic 页面读取当前知识、材料与近期观察；
- 保存 Agent 回答并用于 Decision；
- 依据 Intelligence 采用 Content Idea，再记录真实 Action/Outcome。

组合层只负责顺序、事务入口和返回产品结果，不复制模块不变量。若组合层出现大量“如果 status=... 就直接 update 表”，说明接口太浅或 seam 放错。

不建立一个万能 `AppService`、`CommandBus`、`UseCaseRegistry` 或全局事件路由器。跨模块 durable work 使用小而明确的 work kind 和 typed payload version；没有真实消费者时不创建事件。

## 错误与结果

- 每个模块使用封闭、可理解的错误族：无权限、前置条件变化、冲突、无合格材料、来源不完整、暂不可用、不可重试等；不把所有错误变成字符串。
- 模块接口返回业务 Outcome，而不是只返回 `()` 或 bool。调用者能区分首次成功、幂等 replay、条件未满足、未知和需要决定。
- `anyhow` 类上下文错误可以用于组合进程诊断，但不能成为跨模块或 wire contract。
- 外部 adapter 在最外层映射 HTTP/CLI/plugin 错误；领域模块不知道状态码或页面文案。
- error 不应泄漏 Cookie、Token、DSN、受限原文或完整第三方响应。

## 测试表面

模块接口就是主要测试表面。按依赖类型使用：

| 依赖 | 测试方式 | 不推荐 |
|---|---|---|
| 纯规则、版本选择、资格判断 | 直接通过模块接口的快速测试 | 测私有 helper 调用顺序 |
| PostgreSQL 事务与并发 | Docker PostgreSQL 16 集成测试，检查返回和数据库副作用 | mock repository 证明“事务成功” |
| 插件协议 | 脱敏 fixture adapter + contract tests；真实平台另过 Gate 4 | 测 TypeScript 类型就声称端到端成功 |
| 对象存储 | 本地 adapter + 正式 adapter contract suite | 只检查 URL 字符串 |
| 模型供应商 | deterministic adapter、结构化 schema 负例、少量脱敏 benchmark | 断言自然语言完全相同 |
| 页面/CLI | 从产品接口验证 provenance/unknown/权限 | 直接读取数据库绕过模块 |

旧浅模块测试一旦由深模块接口测试覆盖，应删除或迁移，不无限叠加两套测试。测试必须对实现内部重构稳定；如果改一个私有函数名就要重写大量测试，说明测试越过接口。

## 文件与模块规模门禁

行数不是设计质量，但可以作为漂移报警器。第一阶段候选门禁：

| 对象 | 预警 | 默认硬门 | 规则 |
|---|---:|---:|---|
| `lib.rs` / `main.rs` | 120 行 | 200 行 | 只做模块索引、组合和启动，不放业务实现 |
| 生产 `.rs` 文件 | 350 行 | 500 行 | 超过预警必须说明单一责任；超过硬门必须拆分或有时限明确的 ADR 例外 |
| 单个函数/方法 | 60 行 | 100 行 | 优先提取有名字的领域步骤；不为压行数制造浅 wrapper |
| 测试文件 | 600 行 | 900 行 | 按行为场景拆分；fixture builder 独立，不复制大 payload |
| 一个模块对外 public item | 15 个 | 25 个 | 超过时审查接口是否把内部步骤暴露给调用者 |

生成代码、经登记的 fixture 和 migration 受各自治理规则，不用行数硬拆；但不能用 `include!`、宏或 JSON 把巨型业务复杂性藏起来。任何例外必须说明负责人、原因、替代计划和复核日期。

实现阶段增加自动检查，至少拒绝：

- 业务逻辑进入 `main.rs/lib.rs`；
- 单文件超过硬门且没有登记例外；
- 新增 `common/utils/helpers` 无明确所有者目录；
- crate 循环依赖或应用 adapter 被领域模块依赖；
- SQLx/Axum/模型供应商类型穿过不应暴露的模块接口；
- 新模块只有 pass-through 方法，没有隐藏复杂性。

## 首个垂直切片对模块的要求

第一段代码仍建议证明：

```text
脱敏/合成 Package fixture
→ Capture submit（authority、hash、partial Coverage、replay/conflict）
→ PostgreSQL 真实副作用
→ World process（Source Identity + typed Content Observation）
→ provenanced Current read
→ API/CLI 返回来源、时间、Coverage 和 unknown
```

这个切片只需要：

- contracts；
- Capture；
- World Observation；
- Access 的最小读取/处置 gate；
- PostgreSQL 事务实现；
- API/worker 组合入口。

它不需要先实现 Topic Map、聚类、Agent、Signal、Brief、Outcome、pgvector 或真实插件升级。完成标准是 DB-P02–P10 中与切片有关的场景真正通过，而不是 crate 存在或 API 返回 200。

## Gate 6 仍需继续收口

1. Agent Runtime 的工具、结构化输出、模型/提示版本、成本、停止、评测和失败合同；
2. Capture 调度与插件协议的命令、租约、恢复、兼容性、安全和风险停止；
3. durable work、worker 领取、重试、死信/人工处置和可观测性；
4. API、CLI、Agent Interface 的认证/委托与 provenance envelope；
5. 当前 Bootstrap crate 如何最小改造成首切片，避免一次性创建全部目标模块；
6. 模块依赖和文件规模检查怎样进入 CI；
7. 是否需要 Python analysis worker、pgvector 或具体聚类算法，只能由真实样本 benchmark 决定。

上述完成并经过 Gate 5 关系反查后，才有资格确认 Gate 6。本文不因列出模块而授权开工。
