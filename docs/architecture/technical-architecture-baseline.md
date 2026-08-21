# Linggan Intelligence 技术架构基线

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: 全产品技术架构地图、责任归属、入口边界、数据与运行原则、确定性等级，以及后续 SCOPE 的架构符合性检查
> 事实来源: `AGENTS.md`、ADR-0001、用户确认的 `USER-DEC-01`–`06`、当前 Rust workspace、migration/测试事实、当前 SCOPE 与专题架构文档
> 冲突时以谁为准: 真实运行/数据库副作用与可复现测试优先于当前代码；当前代码、migration 与版本化合同优先于本文的实现描述；长期决定以更新且已接受的 ADR/用户决定为准；具体实施权限只来自当前 SCOPE

本文是全系统唯一的 **Technical Architecture Baseline** 入口。它回答三件事：

1. 整个产品的技术主干和责任如何连接；
2. 哪些方向已经确认，哪些只是推荐，哪些仍缺现实来源；
3. 后续 Agent 应怎样从架构进入 SCOPE、代码、测试和真实验收。

本文不复制每份专题架构，也不是一张可以自动生成全系统代码的施工图。专题细节仍在数据、采集、Agent、运行和产品接口文档中；任何 `PROVISIONAL` 内容仍须由具体 SCOPE 冻结后才能实施。

---

## 0. 怎么读这份手册

### 如果你是产品负责人

先读：

1. [一句话结论](#1-一句话结论)；
2. [产品含义](#2-这套技术架构对产品意味着什么)；
3. [端到端责任链](#6-端到端责任链)；
4. [当前实现差距](#21-当前实现差距矩阵截至-2026-08-21)；
5. [待决项](#23-仍待决定或补证的事项)。

### 如果你要写 SCOPE 或实施代码

按顺序读：

1. [权威机制](#4-架构权威机制)；
2. [逻辑模块](#8-逻辑深模块与所有权)；
3. 与本次能力直接相关的入口、数据、运行、安全章节；
4. 对应专题文档；
5. [架构符合性模板](#25-每个-scope-必须附带的架构符合性模板)。

### 如果你只处理一个专题

| 任务 | 进入的专题文档 |
|---|---|
| Rust 模块、Interface、依赖和文件边界 | [`module-architecture.md`](module-architecture.md) |
| 数据身份、版本、Current、Corpus、Claim | [`data-architecture.md`](data-architecture.md) |
| PostgreSQL 事务、并发和失败注入 | [`data-consistency.md`](data-consistency.md) |
| 采集控制层、工位、插件、部分结果 | [`capture-plugin-architecture.md`](capture-plugin-architecture.md) |
| Agent Invocation、工具、预算与评测 | [`agent-architecture.md`](agent-architecture.md) |
| API/worker、Durable Work、Outbox、部署与恢复 | [`runtime-operations-architecture.md`](runtime-operations-architecture.md) |
| 工作台、API、CLI 与外部 Agent 入口 | [`../pages/product-interface-architecture.md`](../pages/product-interface-architecture.md) |

---

## 1. 一句话结论

> **Linggan Intelligence 采用一个 Rust 模块化单体、一个 PostgreSQL 16 权威主库、API/worker 两个组合进程、一个轻量 MV3 平台执行插件和受控 Agent Runtime；代码按事实责任和事务深度组织，不按页面、表或每个领域名词拆 crate，更不提前拆微服务。**

这条方向由 `USER-DEC-05` 整体确认，确定性为 `ACCEPTED`；其中具体 crate 归属、部署包装、库选择、参数和物理表形态仍由各 SCOPE 渐进冻结。

系统的性能优势不来自“先堆更多服务”，而来自四条主线：

- 外部材料一次接入、可追溯复用，避免反复采集和反复解释；
- 事实历史与 Current 投影分离，写入可靠、读取可优化；
- 有界任务、幂等、Durable Work 和数据库围栏，让失败可以恢复而不制造重复事实；
- Agent 只做受控研究循环，不承担数据库、权限和正式知识，模型或供应商可替换。

---

## 2. 这套技术架构对产品意味着什么

| 产品问题 | 当前事实 | 目标状态 | 确定性 |
|---|---|---|---|
| 系统相信什么 | 项目已经确认 Evidence、Observation、Current、Analysis/Claim、Decision/Outcome 分责 | 任何页面、Agent 或报告都能回到来源、范围、时间、版本和缺失 | `INVARIANT` |
| 批次只取得部分数据怎么办 | 已确认“目标未达不连坐已安全取得材料”，known-set 与 quota 分开 | 保存资格、任务完成度、Coverage、用途适用性和 Claim 资格分别返回 | `INVARIANT` |
| 怎样持续运行 | 当前只有合成 SCOPE 的局部合同、migration 和 proof；API/worker 仍未形成业务链 | API 接受短命令，worker 消费数据库 Durable Work，失败可重试/接管/停止 | 架构方向 `ACCEPTED`；实现 `PROVISIONAL` |
| 插件是什么 | 新 Rust 系统未接入真实插件；历史 V2 只作参考 | 插件是轻平台执行端，服务端拥有准入、Work、Attempt、Evidence 接入和结果资格 | `ACCEPTED` |
| AI/Agent 是什么 | Pi 仅经上游审查，被裁定为条件性 Adapter 候选；没有运行时代码 | Linggan 拥有调用、权限、材料、工具、预算、回执和正式结果；Pi 只执行一次获准循环 | `PROVISIONAL`（`CONDITIONAL ADAPTER`） |
| 用户从哪里进入 | 第一阶段产品入口方向已确认；Web 尚未实现，CLI 尚未创建 | Web 是受控 client；CLI 只经 API；MCP 在 API/CLI 合同稳定且有真实调用者后再加 Adapter | 主方向 `ACCEPTED`；具体界面/协议 `PROVISIONAL` |
| 当前能否日常使用 | 不能。当前只有本地未提交的 F01 合同 tracer 和 PostgreSQL foundation proof | 需依次完成合成主链、真实 producer、插件 Canary、真实 Observation、工作台、部署和业务验收 | 当前实现事实 |

架构首先保护“系统不会说错强度”，其次才追求更快的吞吐。速度、搜索、向量、并行 worker 和拆服务都必须在这条可信链上演进。

---

## 3. 四级确定性

确定性等级描述的是“一个架构主张有多确定”，不描述代码是否已经完成。

| 等级 | 含义 | Agent 可以做什么 | Agent 不可以做什么 |
|---|---|---|---|
| `INVARIANT` | 已确认且跨模块长期成立的系统不变量 | 直接作为设计和测试硬门；发现违反时停止相关实现 | 在 SCOPE 中自行折中、降级或绕过 |
| `ACCEPTED` | 已由 ADR 或明确用户决定接受的长期方向 | 在具体 SCOPE 范围内实施；用测试决定已授权的技术细节 | 把“已接受方向”扩大成未授权能力，或静默改成另一种架构 |
| `PROVISIONAL` | 当前推荐的设计形状，仍需真实场景和 SCOPE 验证 | 比较方案、做小切片、冻结接口和验收 | 因为写在本文就创建表、crate、服务、依赖或生产资源 |
| `SOURCE_INCOMPLETE` | 现实来源、样本、合同或运行证据不足 | 记录缺口，完成不依赖该来源的工作，准备最小事实审计 | 从旧 schema、历史讨论、模型常识或目标数量补猜 |

补充规则：

- `权威当前` 表示本文对“架构地图、责任和确定性等级”是当前权威，不表示文中每个目标都已经实现。
- `PROVISIONAL` 不等于“可以先做”；它只表示可在获准 SCOPE 中验证。
- 当前代码可以证明“实现成了什么”，但不能静默改变已确认产品语义。
- 实现成熟度另按“设计、定义、实现、自动测试、真实链、部署、用户/业务验收”报告。

---

## 4. 架构权威机制

### 4.1 四层权威链

```text
Architecture Baseline
系统整体是什么；责任怎样连接；每项决定有多确定
        ↓
专题架构
数据 / 采集 / Agent / 模块 / 产品接口 / Runtime 怎样守住各自边界
        ↓
ADR + SCOPE
ADR 记录长期决定及代价；SCOPE 冻结本次允许实现的最小范围
        ↓
Code / DB / Tests / Run
实际实现、数据库约束、可复现测试、真实运行副作用和验收
```

这不是“上层文档可以否定运行事实”的顺序。四层回答不同问题：

| 层 | 只回答什么 | 不能替代什么 |
|---|---|---|
| Architecture Baseline | 全局地图、责任、依赖方向和确定性 | 具体字段、表、route、算法或实施授权 |
| 专题架构 | 一个专题的完整设计与备选 | 已接受长期决定、当前 SCOPE 或代码事实 |
| ADR | 为什么接受一项长期且难逆转的技术决定 | 一次切片的文件白名单与完成标准 |
| SCOPE | 这一次允许实现什么、怎样验收、哪些明确不做 | 全产品永久架构或生产完成证明 |
| Code/DB/Tests/Run | 当前真正实现和证明了什么 | 尚未实现但已经确认的产品含义 |

### 4.2 事实冲突顺序

出现冲突时按项目治理顺序裁定：

1. 真实运行、真实数据库副作用和可复现测试；
2. 当前代码、migration、跨边界合同和真实 producer fixture；
3. `ACCEPTED` ADR 与用户明确接受的决定；
4. 本文和其他“权威当前”文档；
5. 活跃计划、草案、一次性审计和完成计划；
6. `references/` 历史证据。

运行事实可以推翻“已经实现”的文档描述，但不能把产品权限改成“因为代码能做，所以允许做”。后者必须回到 ADR/用户决定和 SCOPE。

### 4.3 架构变更规则

| 变更类型 | 处理方式 |
|---|---|
| 违反 `INVARIANT` | 停止；需要用户重新决定并同步不变量、基线、专题文档和测试 |
| 改变 `ACCEPTED` 长期方向 | 新建或更新 ADR/明确决策；说明代价和迁移，不改写历史 |
| 验证 `PROVISIONAL` 细节 | 在 SCOPE 中写当前事实、方案、失败条件和退出证据 |
| 遇到 `SOURCE_INCOMPLETE` | 建立最小事实审计；未证明前相关能力失败关闭 |
| 代码与本文实现描述漂移 | 以代码/测试更新“当前事实”栏，同时保留目标与未证明边界 |

---

## 5. 产品技术目标与非目标

### 5.1 技术目标

| 目标 | 当前事实 | 目标状态 | 确定性 |
|---|---|---|---|
| 可追溯 | F01 合同和局部数据库关系已开始证明 hash、Package/Record 与 Coverage | 每个有后果的读取、判断和行动都能回到不可变来源与版本 | `INVARIANT` |
| 可纠错而不改写历史 | 已确认 Evidence/Observation 追加、Current 可重建 | parser 修订、来源差异、迟到材料和隐私处置各有独立历史 | `INVARIANT` |
| 有限资源下可恢复 | 控制层和 Runtime 已有设计，业务 runner 未实现 | 有界 Work、lease/epoch、幂等、有限重试、接管、停止与回执 | 方向 `ACCEPTED`，物理实现 `PROVISIONAL` |
| 读写可分别优化 | 当前尚无产品 Read Model | 权威写历史保持清晰，高频页面/CLI 使用带水位和来源的可重建投影 | `ACCEPTED` |
| Agent 可替换 | Pi 未接入 | 业务模块依赖 Linggan 自有窄 Interface；Pi/模型类型留在 Adapter 内 | `PROVISIONAL` |
| 渐进扩展 | 当前 Rust workspace 是 Bootstrap + F01 局部代码 | 先深模块和明确 seam，再按真实负载扩 worker、索引、对象存储或拆服务 | `ACCEPTED` |
| 可验证交付 | 已有治理、Bootstrap、合同和局部 PostgreSQL proof | 每项能力同时有正向、攻击性负例、真实数据库/边界副作用和证明范围 | `INVARIANT` |

### 5.2 第一阶段非目标

- 不把旧 Prisma schema、旧 migration、旧 dump 或 V1/V2 fallback 带入新数据库。
- 不构建微服务集群、Kafka、Temporal、Camunda、图数据库、数据湖或企业级 Agent Control Plane。
- 不按每个页面、表或名词创建一个 crate/服务。
- 不构建万能 `Object`、`Asset`、`Workflow`、`Repository`、`AIProvider`、`Agent Task` 或 JSON 状态机。
- 不把 Web、CLI、MCP、缓存、Corpus、Topic Map、Radar 或 Agent 输出变成第二事实源。
- 不承诺未获真实可比性证明的市场趋势、总体比例、需求规模或因果。
- 不为未来客户提前建设多租户、计费、复杂组织和跨租户共享。
- 不以插件自动化、验证码绕过、无限重试或静默换账号换取吞吐。
- 不在没有真实 Artifact、中文检索和规模 benchmark 前预选对象存储、向量技术、专用搜索引擎或 Python 分析进程。

---

## 6. 端到端责任链

这是一条责任链，不是数据库 ER 图，也不是每次工作都必须走到终点。

```text
真实世界
  ↓
Continuous Observation Objective / Research Question
  ↓
Information Need
  ↓
Acquisition Request → Human Authorization → Admission
  ↓
Work Order → Attempt / Lease → Producer（MV3 或未来受控 Adapter）
  ↓
Ingress Delivery → Capture Package / Record / Artifact / Coverage
  ↓
Evidence Ingress → Accepted Evidence
  ↓
Source Identity / Discovery → typed Observation
  ↓
Current Resolution / Read Projection
  ↓
Materials / Analysis Input Set → deterministic analysis / Agent Invocation
  ↓
Candidate → Claim Revision / Signal / Intelligence Release
  ↓
Decision → Action Plan / Attempt / Occurrence
  ↓
Outcome Observation → Evaluation
  ↓
校准后续问题、观察和判断（不改写过去）
```

### 责任交接硬门

| 从哪里到哪里 | 必须成立 | 不能偷换 |
|---|---|---|
| Need → Work Order | 已有 Evidence 不足、用途/权限/资源/风险当前仍适用 | 研究问题直接变插件命令 |
| Producer → Package | 当前有界执行、来源/时间/身份/终态如实冻结 | 插件自己扩采或补业务默认值 |
| Package → Evidence | authority、routing、合同、canonical/hash、成员边界通过 | 任务结束直接等于 Evidence |
| Evidence → Observation | Record 有合格来源身份和直接来源支持 | 缺失、AI 推断或目标字段补成对象事实 |
| Observation → Current | 明确规则、来源、冲突和输入水位 | 最后写入、最大值或平均值自动成为当前 |
| Current/Materials → Analysis | 冻结实际输入、版本、Coverage、权限和用途 | 当前页面查询条件充当可复现输入 |
| Analysis/Agent → Claim | 逐条支持、反例、限制和适用范围通过 | 自然语言流畅或模型置信度直接成正式知识 |
| Decision → Action | 决定固定确切 revision，原模块正常执行 | 批准冒充已执行 |
| Action → Outcome | 有合格来源证明实际结果 | 页面点击、发布尝试或未观察结果写成成功/0 |

任何一层都允许以“证据不足、没有必要、需要决定、权限阻断、部分结果或失败”诚实停止。

---

## 7. 总体代码与部署形态

### 7.1 已接受的形态

`USER-DEC-05` 已接受：

```text
Web / CLI / future MCP / external Agent
                    │
                    ↓
              linggan-api
         认证、用途、组合、产品合同
                    │
        ┌───────────┴───────────┐
        ↓                       ↓
  Deep Modules             Read Models
        │                       │
        └───────────┬───────────┘
                    ↓
              PostgreSQL 16
                    │
               Durable Work
                    ↓
              linggan-worker

MV3 Plugin ── versioned contract ── API / Capture Interface
Model/Pi   ── Linggan runtime port ── Agent Runtime
Blob store ── only when proven ───── Evidence implementation
```

| 决定 | 当前事实 | 目标状态 | 确定性 |
|---|---|---|---|
| 代码形态 | 一个 Cargo workspace 已存在 | 一个模块化单体；按深模块组织，不按部署幻想拆分 | `ACCEPTED` |
| 主数据库 | ADR-0001 接受全新 PostgreSQL；F01 有本地未提交 migration/proof | 一个 PostgreSQL 16 权威主库；旧库完全隔离 | `ACCEPTED` |
| 进程 | `apps/api`、`apps/worker` 当前只打印 Bootstrap 信息 | API 与 worker 两个组合进程，共用模块 Interface，不共享进程内真相 | `ACCEPTED` |
| 浏览器执行 | 新系统尚未接入插件 | TypeScript/MV3 轻执行器，只执行受控 Work Order | `ACCEPTED` |
| Agent | 无 Agent Runtime 实现 | 受控 Linggan Kernel + 可替换模型循环 Adapter | 方向 `ACCEPTED`；Pi 方案 `PROVISIONAL` |
| 物理主机/容器 | 尚未部署 | API/worker 可先同主机不同进程/容器；包装由部署 SCOPE 决定 | `PROVISIONAL` |

### 7.2 Workspace 演进规则

- 当前 workspace 名称是 Bootstrap 与 SCOPE-001 的放置护栏，不是永久领域地图。
- 一个逻辑模块不承诺一个 crate；一组紧密事务可以共享 crate，一个 crate 也可以暂时承载多个紧密模块。
- 只有出现真实调用者、独立 Interface、明显隐藏复杂性和稳定测试表面时才新增模块或 crate。
- `lib.rs`/`main.rs` 只组合、导出和启动，不承载业务实现。
- 不创建空 crate、空 trait、预留目录或未来也许会用的依赖。
- 物理拆分必须保留同一责任所有者、版本化合同、幂等、回执和可观测性；不能因“拆了”改变业务语义。

---

## 8. 逻辑深模块与所有权

这里的“模块”指拥有一个小 Interface、在内部隐藏大量业务复杂性的责任单元，不等于 crate、表、服务或页面。

| 逻辑模块 | 拥有的写责任 | 对外小 Interface 形状 | 允许依赖 | 明确不拥有 |
|---|---|---|---|---|
| Capture | Need 准入、Work/Attempt/lease、终态 Package 接入、Coverage、接入 receipt | `admit`、`claim`、`submit`；恢复能力按真实合同后加 | Access、PostgreSQL 私有 Adapter、版本化 producer contract | Source Current、Topic、AI 判断、Action |
| World Observation | Source Identity、Discovery、typed Observation、解释修订、Current 发布 | `process(accepted record)`、`resolve_current` | Accepted Evidence、Access、PostgreSQL 私有 Adapter | 采集授权、Topic 分类、市场意义 |
| Knowledge | Domain、Topic/Term/Relation revision、Definition/Map release | `publish(exact decision + manifest)`、版本化读取 | Decision、Access、历史定义 | 聚类、趋势、Corpus 原文 |
| Materials | Selection、Fragment、Transformation、Material Pack/Usage、受控检索 | `search_qualified`、`freeze_pack`、`read_context` | Evidence/Observation、Knowledge、Access | 第二份原文、Claim、Agent 授权 |
| Analysis | 冻结输入、分类/聚类/统计 Run、typed result、finalize | `prepare/finalize(versioned run)` | Materials、World、Knowledge、可选 Agent Runtime | 正式 Topic、Claim 采用、自动扩采 |
| Intelligence | Claim revision、支持/反例、Signal attention、Insight/Brief release | `revise_claim`、`publish_release` | finalized Analysis、Materials、World、Knowledge | 原始 Evidence、模型循环、现实 Action |
| Decision & Learning | Proposal、Decision、Action Plan/Attempt/Occurrence、Outcome、Evaluation | 按确切 revision 决定、记录行动/结果/评价 | Intelligence/Idea revision、Access | 直接改 Topic/Claim、平台采集执行 |
| Access & Privacy | Actor/Delegation/用途裁定、访问阻断、Disposition 与传播回执 | `authorize`、`dispose` | 身份、政策、血缘索引 | 业务内容副本、万能状态 |
| Agent Runtime | Invocation、Material/Tool/Budget freeze、模型循环、Tool Receipt、输出验证 | 窄 `ModelToolLoopPort` + 领域工具 Gate | Access、Materials、Analysis、领域工具 Adapter | 数据库直写、插件调度、正式知识/Action |
| Read Models | 页面/API/CLI 的可重建投影、freshness/watermark/provenance envelope | 面向用户任务的查询 Interface | 各模块的不可变历史/读 Interface、Access | 任何删除后无法恢复的事实 |

### 8.1 依赖方向

```text
apps/api, apps/worker
        ↓
application use cases / composition
        ↓
deep modules
        ↓
private PostgreSQL / plugin / model / blob adapters
```

跨模块只依赖对方 Interface 和稳定引用：

- Capture 接纳 Record 并产生 Durable Work；World 消费不可变 Record 引用，不回写 Capture 成功。
- Analysis 读取 Materials/World/Knowledge，可调用受控 Agent Runtime；不能发布 Knowledge/Claim。
- Intelligence 只采用 finalized Analysis/Materials/World/Knowledge；不读取任意原始 payload。
- Decision 固定 Intelligence/Idea 的确切 revision；执行仍回到拥有后果的模块。
- Read Models 读取历史和投影，永远不反写成为 Evidence、Observation 或 Claim。

### 8.2 Seam 与 Adapter 规则

> 确定性：下述 seam 的责任方向为 `ACCEPTED`；精确 trait、Adapter 形状和代码位置为 `PROVISIONAL`，必须由当前 SCOPE 与合同测试冻结，不能据本节直接施工。

优先为真实可变化的外部边界建立 seam：浏览器插件、模型供应商、对象存储、外部发布/通知。PostgreSQL 业务 SQL 靠近拥有事务不变量的模块，不为每张表创建浅 Repository。

一个 seam 至少要有两个可验证实现才真正有替换价值，例如 deterministic Agent Adapter 与 Pi Adapter。只有一个实现且只是透传时，不先建抽象。

---

## 9. 当前 crate 与目标模块的映射

以下“当前事实”特指 2026-08-21 本地工作副本；其中若干变更尚未提交到 `origin/main`。

| 当前位置 | 当前事实 | 可承载的目标责任 | 不应被解释为 | 确定性 |
|---|---|---|---|---|
| `crates/contracts` | 已有 F01 合成 Package parser、JCS/hash 与攻击性合同测试；本地未提交 | 真正跨进程/跨仓的版本化 wire contract | 全部内部领域模型或真实 producer contract | 当前代码事实 |
| `crates/domain` | 只有 `Unknown`/`Observed` 极小示例 | 少量稳定、纯领域 primitive 与不变量 | 所有模块共用的 DTO/helper 垃圾箱 | 映射 `PROVISIONAL` |
| `crates/evidence` | 生产实现仍声明未完成；有直接 SQL 的 F01 foundation 集成 proof | Capture/Evidence 的接入 Interface 与私有业务 SQL | 已完成 Package 原子接入或完整 Capture 控制层 | 映射 `PROVISIONAL` |
| `crates/observation` | 生产实现仍声明未完成 | World Observation、Source Identity、Current read/publish | 已有 Observation/Current 业务实现 | 映射 `PROVISIONAL` |
| `crates/intelligence` | 只有未实现占位常量 | 未来可承载一部分 Analysis/Intelligence 责任，是否拆分待真实接口 | Topic、Corpus、Agent、Claim、Action 的万能容器 | `PROVISIONAL` |
| `crates/storage-postgres` | 只有未实现占位常量 | pool、事务/错误和 proof DB helper 等共享基础 | 所有业务模块穿透的通用 CRUD Repository | `PROVISIONAL` |
| `apps/api` | 只打印“无生产 route” | 认证、输入限制、use-case 组合、HTTP/产品结果映射 | 业务规则所有者 | 组合形态 `ACCEPTED` |
| `apps/worker` | 只打印“无 job” | Durable Work runner 与模块 handler 组合 | 第二套业务逻辑或万能 workflow engine | 组合形态 `ACCEPTED` |
| `apps/cli` | 不存在 | 只经 API 的受控人类/Agent client | 数据库客户端、SQL/shell 入口 | 方向 `ACCEPTED` |

任何 SCOPE 改变这些物理位置前，都要先证明：当前 Interface 无法保持深度/事务局部性，或已有两个独立变化轴需要真实 seam。不得用“领域名词看起来应该独立”作为理由。

---

## 10. API、worker、CLI、Web、MCP、插件与 Agent 入口

| 入口 | 调用谁 | 可以承担 | 不可以承担 | 当前状态 |
|---|---|---|---|---|
| API | application use case / deep module Interface | 认证、schema/资源限制、用途授权、短事务、receipt、受控读取 | 长时间采集/模型循环、业务 SQL 散落在 route、虚假完整成功 | Bootstrap；业务未实现 |
| worker | Durable Work owner handler | 领取/续租/围栏、执行有界步骤、回写结果和后续工作 | 自行决定业务意义、在内存维持隐形链、复制 API 规则 | Bootstrap；业务未实现 |
| CLI | 版本化 HTTP API | 读取、解释、状态、候选和有边界申请；人类/JSON 共用结果模型 | DSN、任意 SQL、直接访问模块内部或提交未授权资源 | `ACCEPTED` 方向；尚不存在 |
| Web 工作台 | 版本化 API/Read Models | 呈现今天关注、Topic、Corpus、研究、行动、运行状态 | 成为事实源、从页面状态猜成功、直接访问数据库 | client 边界 `INVARIANT`；尚未实现 |
| MCP | 稳定 API/Agent Tool Interface 的 Adapter | 在出现真实调用者后复用已验证产品合同 | 首期独立事实/权限/任务系统或数据库工具 | 后置；`PROVISIONAL` |
| MV3 插件 | Capture wire contract / API | reconcile、claim、renew、执行、冻结 Package、submit、acknowledge | 研究优先级、自动扩采、Evidence 接纳、Topic/AI 判断 | 轻执行方向 `ACCEPTED`；新链未实现 |
| Agent | Linggan Agent Kernel / domain tools | 在委托、材料、工具、预算内读取、分析、找反例、提交 Candidate/Proposal | SQL/shell/任意 HTTP、数据库直写、插件直控、正式知识或现实动作 | 受控方向 `ACCEPTED`；Kernel/Pi 未实现 |

### 统一入口原则

- 所有写入都穿过拥有不变量的模块 Interface。
- 同一能力的 Web、CLI、Agent Tool 使用同一应用合同和权限判断，不分别重写事实口径。
- API 200 只证明 API 合同成功；202 只证明接纳后台工作；两者都不证明后续业务结果。
- 对外稳定的是产品能力和 public reference，不是表名、自增 ID、SQLx type、Axum type 或 Pi event。

### 禁止旁路清单

- 禁止 Web、CLI、MCP、插件或 Agent 绕过 API/领域 Interface 直连 PostgreSQL；运维诊断必须走单独授权的只读路径。
- 禁止 route、worker handler、页面或 Pi adapter 自己复制领域状态机、权限判断、Coverage 计算或 Current 选择规则。
- 禁止插件绕过 Capture Admission 自行扩采，或把 submit/acknowledge 当成 Evidence 已接纳、研究已满足。
- 禁止以 SQLx row、数据库表名、内部 UUID 或框架事件作为公开合同；外部只拿稳定 schema 和 public reference。
- 禁止为了“先跑通”静默把 unknown 写成 0、把 partial 写成 complete、覆盖 Evidence/Observation 历史或丢弃失败 receipt。
- 禁止模型输出、Agent 文本、搜索摘要或 UI 显示直接成为事实、权限决定、正式 Claim、现实动作或完成证明。

---

## 11. PostgreSQL 数据架构

### 11.1 一库、分责、历史优先

| 项目 | 当前事实 | 目标状态 | 确定性 |
|---|---|---|---|
| 数据库 | ADR-0001 接受全新 PostgreSQL；当前本地有一份 F01 migration | 单一 PostgreSQL 16 权威主库；旧库只读隔离 | `ACCEPTED` |
| 写模型 | F01 只覆盖接入侧局部表 | 按责任保存事实历史、版本历史和运行历史 | `INVARIANT` |
| Current/读取 | 尚未实现 | 类型化、可重建、有字段来源、带水位的投影 | `ACCEPTED` |
| JSONB | 当前 F01 Record payload 使用 JSONB | 仅容纳原样稀疏字段/受控扩展；核心身份、关系、状态、成员和分母关系化 | `ACCEPTED` |
| 对象存储 | 未启用 | 首个真实大 Artifact 证明需要后，作为受限 blob seam 引入 | `SOURCE_INCOMPLETE` |
| 向量/搜索 | 未实现、未 benchmark | 先使用合格材料的类型化查询/全文检索；向量或专用搜索由真实中文样本决定 | `SOURCE_INCOMPLETE` |

PostgreSQL 16 是当前已接受的权威存储，不是本阶段要维持“可随时换数据库”的 seam。SQLx type、表结构和查询计划必须封装在模块内部；需要替换的 seam 是外部 producer、模型、对象存储和外部交付，不是主数据库本身。

### 11.2 五种持久形态

| 形态 | 用途 | 例子 | 更新规则 |
|---|---|---|---|
| Identity | 长期指向谁/什么 | Source、Topic、Claim、Content Idea | 身份稳定；合并/拆分留治理历史 |
| Revision | 当时具体是什么 | Observation Interpretation、Topic Definition、Claim、Idea | 追加后继版本，不覆盖旧版本 |
| Release | 某次正式采用哪些版本 | Domain Definition、Brief/Insight release | 由明确 Decision 冻结 manifest |
| Run / Occurrence | 一次执行或现实发生了什么 | Attempt、Analysis Run、Action/Outcome Observation | 每次独立追加，失败也保留 |
| Projection | 当前为了读取怎样组织 | Source Current、Topic Map、Radar、页面 read model | 可删除重建；每个值回到权威历史 |

### 11.3 数据所有权层

```text
Control history
Work / Authorization / Attempt / Lease
        ↓
Evidence history
Delivery / Package / Record / Artifact / Coverage / Receipt
        ↓
World history
Source Identity / Discovery / typed Observation / Interpretation
        ↓
Knowledge + Materials + Analysis
Definition releases / selections / frozen inputs / versioned results
        ↓
Intelligence + Decision + Learning
Claim revisions / releases / decisions / actions / outcomes / evaluations
        ↓
Rebuildable read projections
```

每层保存自己有资格表达的事实。Corpus 只保存用途化选择和转换血缘，不复制第二份原文；Read Model 只缓存可恢复读取，不保存唯一业务含义。

### 11.4 事务边界

> 确定性（适用于 11.4–11.6）：核心一致性、unknown 保留和历史不得静默覆盖是 `INVARIANT`；具体 transaction、table、lock、index 和 query 形状为 `PROVISIONAL`，由当前 SCOPE、真实 PostgreSQL 测试和 `EXPLAIN` 证据冻结。

| 事务 | 必须共同提交 | 不纳入同一事务 |
|---|---|---|
| Package ingress | authority/routing/hash 判定、Delivery、Package、Record/Artifact 元数据、Coverage、receipt、后续 processing work | Record parser、Source Identity、Observation、Current |
| 单 Record processing | identity resolution、typed Observation/血缘、处理 receipt、Current 重算 work | 其他 Record；整个 Package 回滚 |
| Current publish | 完整候选 revision、字段来源、规则/水位、active pointer、刷新 work | 构建过程的长计算 |
| Definition/Brief release | Decision、exact manifest/revisions、active pointer、重算 work | 未完成分类/统计的虚假 ready |
| Privacy disposition | 处置、立即读取阻断、最小 tombstone、传播 work、receipt | 全部派生物的长时间物理清理 |
| 外部 delivery | 数据库内 intent/outbox、幂等键、attempt、receipt | 把第三方系统假装成数据库事务的一部分 |

### 11.5 并发与幂等

- 唯一业务身份用 database unique/foreign key/check 收敛，不使用“先查再插”。
- Work/Attempt、worker work 和 Current 发布使用 lease epoch、expected version 或行锁围栏；旧执行者迟到提交必须被数据库拒绝。
- 同 Capture Identity + 同 canonical hash 是 replay；不同 hash 是 conflict；新 Attempt 是新执行，不是 replay。
- Observation 只追加；迟到材料按实际观察/来源时间进入历史，不按接收时间倒退 Current。
- worker 至少一次执行；业务结果通过语义幂等键和状态前置条件收敛到一次。

### 11.6 索引与查询策略

索引从真实读写路径推导，不从“也许以后会查”推导：

1. 先为 identity、routing、idempotency、lease claim、active pointer 和外键建立必要唯一/查询索引；
2. 高频用户读取走类型化投影、稳定 cursor、输入水位和 freshness，不在页面临时跨几十张权威表拼接；
3. 每个新增索引必须说明支撑的查询、写入代价和删除条件，并用真实 `EXPLAIN`/运行基准验证；
4. 不在第一阶段预先分区、分片、上只读副库或创建通用 JSONB GIN；
5. 原始受限材料不是默认搜索面，优先检索经授权的 Fragment/Transformation；
6. 性能问题先修查询、索引、批次和投影，再讨论拆服务。

---

## 12. Capture V2 子架构在整体中的位置

“插件采集 V2”是整体技术架构中的 **Edge Execution / Capture 子架构**，负责可靠取得外部痕迹，不拥有后续真相。

```text
Research / Observation Need
        ↓
Linggan Capture Control
复用 → 合并 → 准入 → 有界 Work Order → Attempt/Lease
        ↓ versioned contract
MV3 Plugin / Producer
页面/API 访问 → 风险停止 → 冻结 Package → 本地 Outbox
        ↓ submit
Linggan Evidence Ingress
authority / routing / canonical / hash / replay / conflict / atomic receipt
        ↓
Record Processing → Observation → Current
```

### 12.1 服务端拥有

- 是否需要访问平台、是否复用已有 Evidence、是否合并在途工作；
- 时间价值、预算、账号/工位资格、风险和优先顺序；
- Work Order、Attempt、Capture Identity、lease/epoch；
- Package 接入、replay/conflict、Coverage、Evidence 资格；
- Record 处理、Observation、Current 和下游用途资格。

### 12.2 插件拥有

- 当前 Work Order 范围内的页面/响应读取；
- 真实来源表示、字段来源、观察时点、风险和终止事实；
- 本地 run/checkpoint/journal/outbox；
- 冻结终态 Package 的可靠重传；
- `reconcile → claim → renew → submit/ingest → acknowledge` 客户端行为。

### 12.3 插件不拥有

- 研究目标、Topic、正式优先级、下一阶段自动扩采；
- Work/Attempt 身份的自建或续权；
- Evidence 接受、Source Identity、Observation、Current；
- 账号静默切换、验证码/风控绕过、合同降级；
- 长期业务知识或第二工作台。

### 12.4 当前证据边界

历史 V2 源码/fixture 只提供行为参考和测试 Oracle，不是新 Rust 运行时依赖。真实搜索、作者、详情、评论、媒体、时间、分页、账号镜头和安全访问频率仍是 `SOURCE_INCOMPLETE`；SCOPE-001 只证明合成/脱敏事实内核。

---

## 13. Agent Kernel 与 Pi Adapter

### 13.1 长期责任形状

```text
Linggan Domain / Use Case
        ↓
Linggan Agent Kernel
Invocation / Delegation / Material / Tool Grant / Budget / Stop / Receipt
        ↓ ModelToolLoopPort
Deterministic Adapter  |  Pi Adapter  |  future provider Adapter
        ↓
模型与工具循环
        ↓ each tool call
Linggan Tool Gateway → domain module Interface
        ↓
Candidate / Proposal → validator → human Decision / owning module
```

### 13.2 Linggan 必须拥有

- 调用者、Actor/Delegation、目的、Domain、期限和传播边界；
- Material Pack、定义/Claim/Analysis 版本和隐私资格；
- exact Tool Grant、预算、幂等、停止和当前撤权；
- PostgreSQL Durable Work、Invocation/Attempt、Tool Receipt、成本和安全审计；
- 输出 schema、引用、反例、unknown、Claim Ceiling 和最终候选资格；
- Candidate、Decision、正式知识、插件准入和现实 Action。

### 13.3 Pi 只可以拥有

- 一次获准运行中的模型调用；
- 工具选择与有限循环；
- 流式事件、临时上下文和停止信号；
- Pi 内部 provider/session/event 类型，但这些类型不得穿过 Adapter seam。

Pi 不是事实内核、权限中心、持久任务主账、插件调度器、正式知识库或现实行动执行权。当前裁定是 `CONDITIONAL ADAPTER`，不是已安装或已实现。

### 13.4 首个 Agent 切片

在后续独立 SCOPE 中才允许：

1. 一个真实、低风险、脱敏研究任务；
2. 一个窄 `ModelToolLoopPort`；
3. deterministic Adapter 与 Pi Adapter 共用合同测试；
4. 少量领域工具，例如读取 Coverage、追踪 Claim 来源、寻找反例；
5. A1–A2 受控读取/候选分析；
6. Candidate/Proposal 输出，不直接成为 Claim adopted、Knowledge Release 或 Action。

不先建设 Profile 管理产品、Prompt CMS、Runtime Router、Tool Marketplace、多 Agent 编排、长期记忆或 Agent 运维大盘。

---

## 14. Durable Work、Outbox 与运行可靠性

### 14.1 三种工作分开

| 对象 | 回答什么 | 不能代表 |
|---|---|---|
| Business Need/Plan | 为什么要做、允许什么、何时停止 | 后台步骤已执行 |
| Durable Work | 某个模块还有哪项可恢复步骤 | 业务目标完成 |
| External Delivery | 外部存储/模型/通知/发布是否真正发生 | 数据库 work succeeded 即外部成功 |

### 14.2 Durable Work 规则

> 确定性：Durable Work 的模块所有权和可恢复责任方向为 `ACCEPTED`；事实与后续 work 不脱节、旧围栏不得 finalize 是 `INVARIANT`；物理表、handler、lease 和 retry 参数为 `PROVISIONAL`，由当前 SCOPE 与故障测试冻结。

- 每项 work 有唯一 owner module、typed input/ref、handler version、幂等结果和重试边界。
- 通用 runner 只提供 `claim / lease / renew / epoch fence / schedule / attempt / retry / stop / dead-letter / metrics`。
- 业务事实与数据库内后续 work 同事务写入，避免“事实已提交但事件丢失”。
- 外部调用才使用 outbox/intent + delivery attempt + receipt；Outbox 不是模块间默认消息总线。
- worker 崩溃后可接管；旧 epoch 不能 finalize；输入/授权/隐私变化时旧工作停止而不是继续。
- 需要人工决定时，本次 work 结束并形成 Decision Package；不能占用 lease 等待数天。
- 重试次数、总历时、退避和不可重试错误由 handler/SCOPE 有界定义，不使用全局万能值。

### 14.3 可观测性五分法

| 记录 | 用途 | 不能冒充 |
|---|---|---|
| 业务历史 | Evidence、Observation、Claim、Decision、Outcome 等权威事实 | 日志或运行状态 |
| 运行台账 | work、attempt、lease、retry、stop、delivery receipt | 业务结论 |
| 安全审计 | 谁以什么委托/用途读取、决定、授权和处置 | 普通调试日志 |
| 结构化 log/trace | 关联请求、模块、work、build、错误类别 | 可恢复业务真相 |
| metric/alert | backlog、水位、失败、锁、成本、传播残留、备份状态 | 市场趋势或业务成功 |

每个告警必须指向责任模块、影响和 runbook，不让 Mog 从原始调用栈猜根因。

---

## 15. 权限、隐私与秘密

### 15.1 权力模型

| 身份/权力 | 说明 | 边界 |
|---|---|---|
| Human/System Actor | 谁发起、代表谁 | 不因“内部用户”获得无边界原文/行动权 |
| Delegation | 在什么目的、数据、时间、输出和资源范围内代表行动 | 每次调用验证；不能跨任务永久继承 |
| Station/Account | 哪台浏览器、哪个平台观察身份 | 设备凭据不等于材料用途或研究授权 |
| Tool Grant | Agent 本次可以调用哪些领域工具 | 不是 SQL/shell/任意 HTTP 权限 |
| Database role | 哪个运行入口可以执行哪些最小数据库动作 | 应用运行身份无 DDL；具体物理角色数量待负向测试 |

### 15.2 敏感材料默认边界

- 公开可见不等于可无限保存、批量导出、训练模型或交给外部 Agent。
- ADHD、儿童、家庭、医疗和可反向识别材料按最小必要用途处理。
- 原始 Evidence 默认在内部受限区；普通工作台显示脱敏片段；外部 Agent 默认只读聚合、脱敏片段和受控引用。
- 第三方模型的数据处理、保留、训练、区域和删除合同未确认前，不发送原文。
- 获准删除/撤回/脱敏/限制访问后，权威读取立即阻断；随后沿血缘传播到 Corpus、全文索引、embedding/feature、摘要、引用、缓存、Material Pack 和 Agent 输出。
- 历史 Claim/Brief 不静默消失，而是标记来源资格变化并重新评估。

### 15.3 秘密处理

- 密码、DSN、Cookie、Token、生产 dump、平台凭据和未脱敏 payload 不进入 Git。
- secret 只通过受控环境/秘密存储进入进程，不写 CLI 参数、日志、trace、错误或 receipt。
- 插件设备授权、平台账号凭据、API 调用者身份和材料访问授权分别管理。
- migration 身份与日常 API/worker 身份分开；对象存储的读、写、删除按能力/路径最小化。
- 备份同样受加密、访问、保留和隐私处置规则约束。

具体角色数量、RLS、密钥系统、保留期限和法律适用性当前为 `SOURCE_INCOMPLETE / PROVISIONAL`，由首次真实数据/部署 SCOPE 决定。

---

## 16. 错误、unknown、partial 与 receipt envelope

### 16.1 先分清结果责任

| 责任 | 例子 | 不能压成 |
|---|---|---|
| Ingress Delivery | accepted / replay / conflict / rejected | 总 `ok` |
| Package Acceptance | 是否形成不可变接入事实 | Attempt 完整成功 |
| Attempt Terminal | target reached / risk stop / failure 等真实终态 | Coverage 充分 |
| Capture Satisfaction | 当前 Need/Work 是否满足 | Package accepted |
| Processing Runtime | ready / leased / finalized / stopped 等 | Observation formed |
| Processing Business Outcome | observation recorded / unresolved / contract invalid 等 | worker succeeded |
| Observation Formation | formed / not formed / not evaluated | Current selected |
| Current Field Resolution | selected / unresolved / unknown / not applicable | 0、空字符串或最后值 |

### 16.2 Unknown Preservation

下列状态必须保持可区分：

- 未观察；
- 已观察且值为 0/空集合；
- 尚未评估；
- 已评估但未知；
- 不适用于该对象/阶段；
- 身份 unresolved；
- 权限过滤；
- 数据/索引仍在处理；
- 当前捕获面合格零结果；
- 现实不存在（只有专门且合格的证据才可表达）。

不得用字段缺省、`null`、`[]`、404 或 `completed` 让调用者猜。

### 16.3 Partial 规则

- 批次目标未完成不自动否定已合格 Record。
- Package 的原子性只保证“这次冻结交付没有写一半”，不保证目标数量达成，也不保证每条 Record 形成 Observation。
- known-set 才能逐成员表示 `emitted / failed / not_attempted`；maximum-quota 的差额保持 unknown。
- 下游用途分别判断对象事实、内部检索、Corpus、集合内描述、代表性、趋势和市场外推。
- 补采产生新 Attempt/Package；旧包不追加，跨批用冻结 Input Set/Material Pack。

### 16.4 Receipt / response 必须携带的语义

具体 JSON key 由每个版本化合同/SCOPE 冻结；所有 API、CLI、Web 和 Agent Tool 至少必须覆盖这些责任：

| 语义块 | 必须回答 |
|---|---|
| result / receipt | 本次同步步骤到底发生了什么，权威引用是什么 |
| responsibilities | Delivery、Package、Attempt、Satisfaction、Processing、Observation、Current 各自状态 |
| processing | 仍在运行的阶段、水位、可查询引用 |
| scope / time / versions | 输入范围、观察/接收/接纳/读取时间和合同/规则/build 版本 |
| coverage | 单位、实际来源事实、unknown、停止原因 |
| applicability | 这份结果对当前用途可以怎样用、不能证明什么 |
| limitations / unknowns | 缺失、反例、不可比、权限和未实现边界 |
| provenance | exact Package/Record/Observation/Rule/Run 引用 |
| permissions | 当前调用者/用途实际获得的读取和下一动作范围 |
| next | 自动继续、可重试、需要决定、停止或无下一步 |
| safe error | 发生了什么、影响什么、下一步；不泄露 secret、原文或资源存在性 |

SCOPE-001 的精确 envelope 与八责任外部表示以当前 SCOPE 为准，本文不另建第二套 schema。

---

## 17. 测试金字塔与真实链证明

### 17.1 测试层级

| 层 | 证明什么 | 典型证据 | 不能证明 |
|---|---|---|---|
| 纯规则/合同 | 不变量、状态组合、canonical、hash、schema、错误优先级 | 快速 Rust/TS fixture tests，正负 Oracle | PostgreSQL/外部运行成立 |
| Adapter contract | 同一 Interface 的实现一致 | deterministic vs Pi、fixture vs plugin、local vs blob Adapter suite | 真实供应商/平台稳定 |
| PostgreSQL integration | 约束、事务、原子性、幂等、权限、并发、故障回滚 | 隔离 PostgreSQL 16 proof DB + 行/关系/回执断言 | API、进程恢复、真实 producer |
| Process/component | API、worker、CLI、重启、lease/epoch、水位、safe error | loopback 进程 + 真实 DB + fault injection | 真实页面、真实账号、生产 |
| Real boundary canary | 真实 producer/对象存储/模型/发布的来源、失败和副作用 | 获准小样本、脱敏 fixture、单工位 Canary、外部 receipt | 全量 Coverage、市场代表性 |
| Deployment/operations | 发布、迁移、备份、恢复、监控和回滚 | 指定环境部署、隔离恢复演练、权限负例 | 产品对用户有用 |
| User/business acceptance | Mog 用真实任务完成判断并认可结果 | 明确任务、可见结果、限制和实际决策/收益 | 后续所有场景都已完成 |

### 17.2 每项能力的最低证明包

1. 来源/输入合同；
2. 正常路径；
3. 攻击性负例；
4. 实际数据库或外部边界副作用；
5. 未出现的禁止副作用；
6. 版本、时间和环境；
7. `VERIFIED` 与 `NOT VERIFIED`；
8. 对应文档和月度进展更新。

mock、类型、编译、HTTP 200、worker `succeeded`、页面卡片、模型文本、CI 通过、部署 Ready 都只能证明各自一层。

---

## 18. 性能与容量策略

### 18.1 不预造吞吐数字

当前没有真实产品流量、producer 频率、材料体积、查询分布、Agent 成本或部署负载，因此不写无证据的 QPS、延迟、并发、存储增长或 SLA 数字。每个进入实现的 SCOPE 必须为自己的用户任务定义：

- 交互预算：用户等待什么必须同步、什么可以后台；
- 资源预算：Package 大小、批次、work、工具/模型、数据库连接和存储；
- 新鲜度预算：Read Model 水位和允许陈旧程度；
- 恢复预算：有限重试、最大总时长和需要人工的条件；
- 验证负载：用什么真实或合成分布证明。

### 18.2 优先优化顺序

1. 消除重复平台访问，复用 Evidence 和等价在途工作；
2. 限制 Work/Package/Material/Agent 输入，使用有界批次；
3. 缩短事务，修正唯一键、查询和索引；
4. 为高频读取构建类型化投影和稳定 cursor；
5. 增加同一 worker 进程的并发或水平增加 worker；
6. 将真实大 blob 移到对象存储；
7. 只有独立测量证明后，才评估专用搜索、向量、分析进程、读副本或服务拆分。

### 18.3 性能退化也要保持语义

- 超时不能把 unknown/partial 变成空结果或成功。
- 重试不能重复 Evidence、Action 或第三方计费副作用。
- 缓存不得绕过权限/隐私，也不得隐藏版本和 freshness。
- 降级只能减少能力并明确限制，不能换来源、降低合同或增强 Claim 强度。
- backpressure 应延期/停止新工作，不丢已经安全接纳的事实。

---

## 19. 扩展与未来拆服务触发条件

模块化单体是已接受的第一阶段架构，不是永不拆分的教条。只有同时出现可测量压力和稳定 seam 才拆。

### 19.1 拆分前必须满足

1. 模块已有小而稳定的 Interface、独立 owner 和合同测试；
2. 事务边界已经清楚，跨边界可以用 durable message/outbox + idempotent receipt；
3. 存在持续、可复现的独立扩缩容压力，无法通过查询、索引、批次、投影或增加 worker 解决；
4. 故障隔离、安全隔离、发布节奏或语言/运行环境差异产生真实价值；
5. 运维团队能承担新服务的部署、监控、兼容、备份和恢复成本；
6. 有对照基准证明拆分后的收益大于网络、数据一致性和治理成本；
7. 通过 ADR 和迁移 SCOPE，保留回退及双版本兼容窗口；不双写两套事实。

### 19.2 可能的未来候选，不是路线承诺

| 候选 | 只有何时才评估 | 当前等级 |
|---|---|---|
| 独立 Capture Runtime | 工位/插件调度负载、故障或安全隔离长期显著独立 | `SOURCE_INCOMPLETE` |
| 独立 Agent execution process | 模型依赖、资源/安全隔离和部署节奏明显独立，Kernel Interface 已稳定 | `SOURCE_INCOMPLETE` |
| 独立 analysis worker | 中文样本 benchmark 证明 Python/专用运行时有必要 | `SOURCE_INCOMPLETE` |
| 只读副库/搜索系统 | 查询负载已影响写入，投影/索引仍不足 | `SOURCE_INCOMPLETE` |
| 多数据库/微服务 | 单库事务边界已稳定分离且组织/负载明确要求 | `SOURCE_INCOMPLETE` |

“代码很多”“表很多”“团队觉得微服务更先进”都不是拆分触发条件。

---

## 20. 发布、迁移、恢复与回退

### 20.1 发布单元分开验收

| 发布对象 | 独立验收 |
|---|---|
| Rust API/worker | build、合同兼容、模块测试、DB 权限、进程健康、真实业务 receipt |
| PostgreSQL migration | 空库/升级路径、约束/索引、锁影响、角色负例、回退/恢复条件 |
| MV3 插件 | 源码、构建包、分发包、manifest/hash、已加载版本、服务端版本门、单工位 Canary |
| Agent/模型 Adapter | 固定版本、合同 suite、权限/预算/停止/输出负例、脱敏 benchmark、成本与数据处理决定 |
| Web/CLI/MCP | 同一 API 事实、认证/委托、unknown/partial/provenance、一致错误与版本 |
| 生产环境 | 指定部署、数据库、对象存储、秘密、监控、备份恢复、正式入口与用户任务 |

一个对象通过不能替代另一个对象通过。

### 20.2 Migration 原则

- migration 由专用身份显式执行，应用启动不自动改 schema。
- 优先 expand-first：先加兼容结构、回填/验证，再切读取；只有旧路径零使用且有恢复证据后才 contract。
- 没有 `db push`、 destructive auto migration、旧字段 fallback 或 V1/V2 双写。
- append-only 历史的修正以新记录/版本表达，不用 rollback SQL 改写已接纳事实。
- 重大删除、旧库迁移、生产数据回填或历史退役需要单独授权；默认不做。

### 20.3 回退与恢复

- 代码回退只有在 schema/合同仍兼容时进行；不以回退代码删除新事实。
- 外部副作用 outcome unknown 时先查幂等键/receipt，不盲目重做。
- PostgreSQL 和对象 Artifact 分别备份；“备份成功”不等于恢复可用。
- 在隔离环境真实恢复后核对 schema、Evidence hash、Current 可重建、Durable Work 水位、数据库角色、隐私阻断和对象引用。
- 隐私处置在备份/恢复中的具体策略必须在生产前确认。
- RPO、RTO、备份保留和密钥恢复当前为 `SOURCE_INCOMPLETE`，由 Mog 按可接受业务损失决定，工程 Agent提供方案与演练证据。

---

## 21. 当前实现差距矩阵（截至 2026-08-21）

### 21.1 快照边界

- 当前分支 `main` 的 `HEAD` 与本机 `origin/main` 都是 `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d`，分歧为 `0/0`。
- 工作区非干净；F01 contracts、migration、数据库 proof 和多份文档属于尚未提交的并行工作。
- 下表只描述本地工作副本；它不证明这些内容已提交、已推送、已部署或已获用户/业务验收。
- 根 `README.md`、`development-stage-tracker.md` 和生成 HTML 架构图的状态文字可能滞后；判断当前实现时以 [`current-state.md`](../current-state.md) 加真实代码、migration、测试与本地 Git 状态为准，不能用导航页或生成图覆盖代码事实。

| 能力 | 本地当前事实 | 已有证明 | 仍缺什么 | 状态 |
|---|---|---|---|---|
| Rust workspace | `apps/api`、`apps/worker` 与六个 crate 成员存在 | Cargo workspace/Bootstrap 检查 | 目标模块 Interface 和业务组合 | 局部实现 |
| Capture 合同 | `contracts` 已有 F01 合成 Package parser、Package/Record hash、duplicate key、unsafe integer、unpaired surrogate、JCS golden tests | 合同测试；静态 F01 Oracle | 完整 F01 envelope/资源门、F02–F10、真实 producer contract | 局部实现，未提交 |
| Capture 数据库 foundation | 本地有 `0001_scope_001_capture_evidence.sql`，覆盖 Work/Attempt/Target/Delivery/Package/Record/Coverage/processing work 的最小关系 | 当前状态记录：隔离 PostgreSQL 16 proof 曾真实通过并拒绝跨父 Delivery | 当前 migration 尚未完成 Coverage 与 known Target/Result 的全物理对账；仍缺原子 ingress handler、故障全回滚、完整权限/并发/重放/冲突 | 局部实现，未提交；本轮未重跑 DB |
| Evidence 生产模块 | `EVIDENCE_IMPLEMENTED=false`；测试直接执行 SQL | foundation integration test | 模块 Interface、私有 SQL Adapter、receipt、事务和 API 组合 | 未实现 |
| Source/Observation/Current | `OBSERVATION_IMPLEMENTED=false`，相关表明确尚不存在于 F01 foundation | 接入阶段“不得提前存在”的负向检查 | 第二份 migration、Record processor、identity、Observation、Current/field source | 未实现 |
| API | `main.rs` 只打印无 production route | Bootstrap 可执行入口 | 认证、capture/read routes、envelope、真实 DB receipt | 未实现 |
| worker | `main.rs` 只打印无 jobs | Bootstrap 可执行入口 | runner、claim/epoch、Record processing、恢复和水位 | 未实现 |
| CLI | `apps/cli` 不存在 | 无 | 只经 API 的 content/explain/processing client | 未实现 |
| Web 工作台 | 无新产品 Web 实现 | 只有产品接口草案/架构图 | Today/Topic/Corpus/Research/Action/Operations 真实页面与可用性 | 未开始 |
| 真实 Capture/插件 | 新 Rust 系统未接真实 producer，插件未升级 | 历史 V2 只读参考 | 真实字段/时间/身份/Coverage 审计、服务端控制层、单工位 Canary | 未授权、`SOURCE_INCOMPLETE` |
| Durable Work/Outbox | F01 表中只有 typed processing work 的最小形状 | migration/proof 局部关系 | 通用 runner、租约/接管、有限重试、外部 outbox/receipt | 未实现 |
| Knowledge/Materials/Analysis | 无业务代码/表 | 已确认领域与专题架构 | 独立 SCOPE、真实材料、输入冻结、分类/聚类/统计资格 | 未开始 |
| Agent Kernel/Pi | 无依赖、无 Runtime、无模型调用 | 一次性上游审查裁定 `CONDITIONAL ADAPTER` | Kernel、port、deterministic/Pi adapters、领域工具、安全/评测 | 未授权实施 |
| 权限/隐私 | 有项目级不变量；SCOPE-001 仅设计本地临时权限 | 文档与治理检查 | 真实 Actor/Delegation、数据库角色负例、处置传播、第三方合同 | 大部分未实现 |
| 对象存储/全文/向量 | 未启用 | 无 | 真实 Artifact/检索 benchmark、隐私/恢复和成本决定 | `SOURCE_INCOMPLETE` |
| 部署/生产/恢复 | 新产品未部署 | 开发环境与历史局部 proof | 发布流水、生产角色/秘密、备份恢复、RPO/RTO、正式入口 | 未开始 |
| 用户/业务验收 | 未发生 | 无 | 用真实研究任务证明产品帮助做出更可靠判断 | 未开始 |

### 21.2 当前最小下一步

当前唯一实施事项仍是 SCOPE-001/F01：

```text
有效合成 Package 原子接入
→ 两条 Record 独立处理
→ Source / Content / Observation / Current
→ loopback API
→ CLI 只经 API
```

完成时仍只能报告“F01 合成主链实现证明通过”，不能报告完整 SCOPE-001、真实平台、插件、Agent、Web 或生产完成。

---

## 22. 决策登记

### 22.1 `INVARIANT`

- Evidence 不可被业务逻辑静默覆盖；更正追加新版本/关系。
- Observation 只追加；Current 是有来源、可重建的读取结果。
- unknown/未观察/无权限/不适用/零值/不存在分别表达。
- Package 原子接入与下游逐 Record 隔离同时成立。
- 目标未达不连坐已安全取得材料；Coverage 限制解释范围，不自动提升或否定 Claim 资格。
- AI/Agent 不是事实来源、权限中心、数据库或正式知识发布者。
- Web/CLI/MCP/缓存/页面/Corpus/Topic Map/报告不成为第二事实源。
- Decision、Action、Outcome Observation、Evaluation 分开；结果不直接改写过去事实或 Claim。
- 受限材料按用途最小访问，隐私处置沿血缘传播。
- 真实副作用、部署和业务验收必须分别证明。

### 22.2 `ACCEPTED`

| ID | 决定 | 来源 | 备注 |
|---|---|---|---|
| `ARCH-A01` | Rust greenfield + 全新 PostgreSQL；旧系统只读隔离 | [`ADR-0001`](../decisions/0001-greenfield-rust-clean-db.md) | 已有正式 ADR |
| `ARCH-A02` | Rust 模块化单体 + 一个 PostgreSQL 16 + API/worker 两进程 + 轻 MV3 + 受控 Agent；不提前微服务/Kafka/Temporal/图数据库 | `USER-DEC-05`，见 [`discussion-decisions.md`](../context/discussion-decisions.md) | 已明确用户确认，但尚未单独建 ADR；不得伪称 ADR |
| `ARCH-A03` | 深模块按责任和事务组织，不按页面/表/名词建 crate；接口/文件设门禁 | `USER-DEC-05` 与模块架构 | 具体 crate 映射仍渐进 |
| `ARCH-A04` | 第一阶段入口以产品任务组织；CLI 不绕过产品合同直连数据库 | `USER-DEC-06` | 精确命令/UI 由原型/SCOPE 冻结 |
| `ARCH-A05` | 单主库中权威历史与可重建投影分开；PostgreSQL 约束承担核心一致性 | `USER-DEC-03/05` | 具体表/索引仍按 SCOPE |

### 22.3 `PROVISIONAL`

- 本文的十个逻辑模块及其物理 crate 归属。
- PostgreSQL Durable Work 使用共享运行表还是模块专属表。
- API/worker/ingress 的最终物理数据库角色数量。
- API/worker 在同主机上使用进程还是容器，以及首个部署平台。
- Agent Kernel 的 `ModelToolLoopPort` 精确 Interface、Pi Adapter 和首个真实调用场景。
- MCP 作为稳定 API/Agent Tool Interface 的后置 Adapter。
- 对象存储启用触发、可观测性技术栈和备份工具。
- 每项 handler 的 lease/retry/batch/timeout 参数和性能预算。

---

## 23. 仍待决定或补证的事项

| 编号 | 问题 | 当前等级 | 阻塞什么 | 由什么证据关闭 |
|---|---|---|---|---|
| `ARCH-S01` | 真实小红书各 lane 的字段、时间、身份、分页、终止和 Coverage | `SOURCE_INCOMPLETE` | 真实 Capture contract、插件升级、趋势资格 | 获准的最小真实 producer 审计 + 脱敏 fixture |
| `ARCH-S02` | 搜索在账号/工位/时间之间是否可比 | `SOURCE_INCOMPLETE` | 正式趋势和比较性观察范围 | 重复对照实验及已冻结观察条件 |
| `ARCH-S03` | 原始 Artifact/恢复包的保存、加密、访问、期限和删除传播 | `SOURCE_INCOMPLETE` | 真实原文/媒体、对象存储和 recovery | 隐私/安全/成本审查 + 真实小样本 |
| `ARCH-S04` | 对象存储供应商与启用时点 | `SOURCE_INCOMPLETE` | 大型原件持久化 | 第一个真实 Artifact 的大小、成本、恢复和隐私实验 |
| `ARCH-S05` | 全文、向量、embedding、rerank 或专用搜索技术 | `SOURCE_INCOMPLETE` | 规模化语料检索/聚类 | 真实中文脱敏样本 benchmark |
| `ARCH-S06` | Pi/provider 的选择、数据处理、成本、模型质量与停止参数 | `SOURCE_INCOMPLETE` | Agent Runtime 真实调用 | 固定版本合同 spike + 脱敏评测 + 供应商决定 |
| `ARCH-S07` | 生产数据库角色、秘密系统、RLS 与网络边界 | `PROVISIONAL` | 首次生产部署 | 威胁模型、PostgreSQL 负向测试和部署 SCOPE |
| `ARCH-S08` | RPO/RTO、备份保留和灾难恢复 | `SOURCE_INCOMPLETE / DECISION_REQUIRED` | 生产上线 | Mog 的损失容忍决定 + 隔离恢复演练 |
| `ARCH-S09` | 是否、何时拆服务 | `SOURCE_INCOMPLETE` | 不阻塞第一阶段 | 持续负载/故障/安全证据 + 稳定 Interface + ADR |
| `ARCH-S10` | 真实首期工作台信息结构、CLI 命令和 MCP 时点 | `PROVISIONAL` | DEV-07/08 | 低保真任务走查、两个真实 CLI/Agent 调用者 |

工程 Agent 应先准备可供 Mog 判断的方案和证据，不把底层猜测推回给 Mog。

---

## 24. 后续 Agent 如何使用本手册

### 24.1 开始前

1. 读 `AGENTS.md`、[`../README.md`](../README.md)、[`../current-state.md`](../current-state.md) 和本文。
2. 确认当前唯一事项、SCOPE、允许文件、真实资源授权和脏工作区。
3. 在本文找到相关责任、确定性等级和专题入口。
4. 读取实际代码、migration、合同和测试，建立“当前事实”栏；不从本文目标推断已实现。
5. 若触及 `SOURCE_INCOMPLETE`，只做获准事实审计或停止依赖它的部分。

### 24.2 设计/实施时

1. 用一个用户可见结果定义切片，不按技术层横向铺满。
2. 指定唯一 owner module、外部 Interface、事务和错误/receipt。
3. 证明是否需要新 seam、crate、表、依赖或部署单元；没有真实调用者不预建。
4. 同时设计 normal、partial、unknown、conflict、unauthorized、failure 和 recovery。
5. 用正向/负向 Oracle 先让测试失败，再写最小实现。
6. 每个写入/读取值都能回到来源、范围、时间、版本和权限。
7. 不改变 `INVARIANT`/`ACCEPTED`；确需改变则停止并走决定/ADR。

### 24.3 结束时

1. 更新代码/合同、相关专题说明和当月 progress。
2. 运行与风险相称的合同、PostgreSQL、进程、治理和真实边界验证。
3. 分开报告本地工作区、提交、`origin/main`、CI、部署、正式入口和业务验收。
4. 用 `VERIFIED / NOT VERIFIED` 报告证明范围。
5. 若只完成一个下层步骤，准确说明用户现在能看到什么和还不能看到什么。

---

## 25. 每个 SCOPE 必须附带的架构符合性模板

后续 SCOPE 可以复制以下模板。没有内容的项不得删除，应写 `not applicable` 并说明原因。

```markdown
## Architecture Conformance

### 1. 用户结果与范围

- SCOPE ID:
- 用户完成后能看到/做到:
- 明确不包含:
- 当前事实:
- 目标状态:

### 2. 权威与确定性

- 相关 INVARIANT:
- 相关 ACCEPTED 决定/ADR:
- 本次验证的 PROVISIONAL 项:
- SOURCE_INCOMPLETE / DECISION_REQUIRED:
- 若与 Technical Architecture Baseline 冲突，处理方式:

### 3. 责任与模块

| 能力 | owner module | Interface | 调用者 | 明确不拥有 |
|---|---|---|---|---|
| | | | | |

- application use case owner:
- 是否新增 seam；两个 Adapter/变化轴是什么:
- 是否新增/移动 crate；为什么现有位置不能承载:

### 4. 入口合同

- API:
- worker:
- CLI（必须只经 API）:
- Web/MCP/Plugin/Agent 影响:
- 认证、Delegation、用途与 public refs:

### 5. 数据与事务

- 新增/修改的 Identity / Revision / Release / Run / Projection:
- 权威写模型:
- 可重建投影:
- foreign key / unique / check / append-only 约束:
- 原子事务:
- 并发对象与 fence:
- 幂等键:
- Durable Work / Outbox / external receipt:
- migration、回退与恢复:

### 6. Truth / Error Contract

- normal:
- partial:
- unknown / not evaluated / not applicable:
- replay / conflict:
- unauthorized / privacy blocked:
- retryable / permanent / external unknown:
- receipt/response 的 scope、time、version、Coverage、applicability、limitations、provenance、permissions、next:
- 禁止出现的错误结论和副作用:

### 7. 安全与隐私

- Actor / Delegation / Tool Grant:
- 数据敏感级别与最小用途:
- secret 进入方式:
- 日志/trace/metric 脱敏:
- 处置与派生传播:
- 数据库/对象存储最小权限负例:

### 8. 性能与扩展

- 用户交互/新鲜度/资源/恢复预算:
- 真实验证负载与查询:
- 索引/投影/批次策略:
- backpressure 和语义不降级规则:
- 本次是否触发拆服务条件；证据:

### 9. 测试与证明

- 正向 Oracle:
- 攻击性负例:
- PostgreSQL 真实副作用:
- 进程/重启/故障注入:
- 真实外部边界（若获准）:
- 部署/恢复（若在范围）:
- 用户/业务验收（若在范围）:

### 10. Completion Report

VERIFIED
- contract:
- runtime/environment:
- scenarios:
- effects:
- verified_at:

NOT VERIFIED
-

- local worktree:
- commit:
- origin/main:
- CI:
- deployed environment:
- user/business acceptance:
```

---

## 26. 详细依据与维护边界

| 主题 | 权威/详细来源 |
|---|---|
| 当前阶段和实现事实 | [`../current-state.md`](../current-state.md) 加当前代码/migration/tests/Git 状态；[`../development-stage-tracker.md`](../development-stage-tracker.md) 用于阶段导航但状态可能滞后 |
| Rust 与干净数据库长期决定 | [`../decisions/0001-greenfield-rust-clean-db.md`](../decisions/0001-greenfield-rust-clean-db.md) |
| USER-DEC-01–06 | [`../context/discussion-decisions.md`](../context/discussion-decisions.md)、[`../plans/completed/disc-001-project-foundation-design.md`](../plans/completed/disc-001-project-foundation-design.md) |
| 总体领域责任 | [`target-architecture.md`](target-architecture.md)、[`../product/domain-invariants.md`](../product/domain-invariants.md) |
| 模块、依赖和文件门 | [`module-architecture.md`](module-architecture.md) |
| 数据与 PostgreSQL | [`data-architecture.md`](data-architecture.md)、[`data-consistency.md`](data-consistency.md) |
| Capture/插件 | [`capture-plugin-architecture.md`](capture-plugin-architecture.md) |
| Agent/Pi | [`agent-architecture.md`](agent-architecture.md)、[`../audits/pi-agent-kernel-upstream-assessment-2026-08-20.md`](../audits/pi-agent-kernel-upstream-assessment-2026-08-20.md) |
| Runtime/部署/恢复 | [`runtime-operations-architecture.md`](runtime-operations-architecture.md) |
| 产品接口 | [`../pages/product-interface-architecture.md`](../pages/product-interface-architecture.md) |
| 当前实施切片 | [`../plans/active/scope-001-content-evidence-vertical-slice.md`](../plans/active/scope-001-content-evidence-vertical-slice.md)、[`../agents/scope-001-execution-contract.md`](../agents/scope-001-execution-contract.md) |

维护本文时只更新“全局地图、责任、确定性、当前差距和跨专题规则”。字段清单、详细状态机、具体 UI、SQL、算法、供应商版本和单次审计证据继续留在专题文档、合同、SCOPE、代码和测试中，避免 Baseline 膨胀成第二份实现规格。
