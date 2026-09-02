# 后台运行、Durable Work、可观测性与安全运维架构

> 状态: 代码事实优先
> 最后核对: 2026-09-01
> 适用范围: DISC-001 Gate 6 的 API/worker 组合、持久工作、调度、重试/接管、外部副作用、运行可观测性、数据库角色、部署与恢复
> 事实来源: 当前 Rust/PostgreSQL/LaunchAgent 代码与运行回执、`/health`、`capture-plugin-architecture.md`、已确认 Gate 3–5 边界
> 冲突时以谁为准: 用户最新确认、ACCEPTED ADR、真实 PostgreSQL 16 并发/故障测试、实际部署与恢复结果；本文状态名和候选参数不是最终物理合同

本文是 [`module-architecture.md`](module-architecture.md) 的运行层渐进披露。已实现部分以当前代码与真实运行为准；候选数据库角色、备份/RPO/RTO 和长期可观测方案仍是待独立证明的目标边界。

## 一句话结论

> **第一阶段用一个 API 进程、一个 worker 进程和一个 PostgreSQL 16 主库构成可恢复模块化单体；业务模块拥有自己的工作语义，通用 runner 只提供领取、租约、围栏、有限重试、接管、停止和运行观测，不建立万能 Workflow 或第二业务状态机。**

当前规模不需要 Kafka、Temporal、Camunda、Redis Queue、Kubernetes 或微服务。若 PostgreSQL durable work 后续出现真实吞吐、隔离或运维瓶颈，再用测量证据决定演进。

## 2026-09-01 本机运行拓扑快照

| 责任 | 当前运行事实 |
|---|---|
| 代码快照 | detached runtime `linggan-intelligence-origin-main-d7e7220`，对应 `main@d7e722018f4f4cfa217c9cf5c0cac6fbcdcaacb3` |
| API | loopback API 从上述快照运行，`/health` 报告 `LINGGAN_BROWSER_PRODUCER_RUNTIME` |
| 调度/巡检 | 巡检 worker 从同一快照运行，scheduler = `running` |
| 媒体 | 媒体 worker 从同一快照运行；媒体原件/派生的业务状态不与 worker 进程存活混用 |
| PostgreSQL | `PLUGIN_RUNTIME_002_SCHEMA_READY / READY`，additive `0030` 已进入当前 readiness 门 |
| Browser Producer | 工位 `1` 已正式认领 `0.8.28`；插件通过服务端批准的 TaskSpec/Attempt 执行，不是调度或分析权威 |

该快照证明当前本机 loopback 运行链与一次受控真实标准详情可以通过，不等于多工位、生产环境、备份恢复、长期稳定性或所有平台/lane 已验收。

## 三种不同的“工作”

### 1. Business Plan / Need

回答为什么要做。例如持续观察某入口、主动研究问题、重新分类某个固定输入集。它有业务范围、版本和结果资格，不由 runner 所有。

### 2. Durable Work

回答哪个模块还有一项可恢复的后台步骤。例如：

- 处理一个已接受 Capture Record；
- 解析一个来源身份并追加 Observation；
- 重建某个类型化 Current；
- 对固定 Material Manifest 运行一批分类；
- 传播一个隐私处置；
- 重新评估一组 Claim 的受影响状态。

Durable Work 是运行承诺，不是业务成功。

### 3. External Action Delivery

回答系统是否真的向外部边界发出一个动作，例如对象存储写入、模型调用、插件交付回执、通知或未来真实发布。它需要独立回执和幂等，不用 `job succeeded` 冒充外部结果。

三者不得共享一个万能 `status`。

## API 与 worker 的责任

```text
HTTP / CLI / Plugin / Agent caller
            ↓
          API
认证、运行时验证、用途授权、调用深模块、返回 receipt
            ↓ 同一短事务
业务历史 + 必需 durable work
            ↓
       PostgreSQL 16
            ↓ claim/lease
          Worker
装载固定输入、调用模块 handler、提交结果/下一项工作
```

### API 负责

- 调用者身份、Delegation、用途和请求 schema；
- 同步完成短、确定、必须立即反馈的模块命令；
- 在同一业务事务中追加权威历史与必要 durable work；
- 返回“已接受/已拒绝/已完成同步步骤/后续处理中”的准确 receipt；
- 不在 HTTP 请求中运行长时间采集、聚类、批量重算、模型循环或隐私全链传播；
- 不因 worker 尚未完成就返回虚假的完整业务结果。

### worker 负责

- 从 PostgreSQL 领取当前有资格的 durable work；
- 以冻结输入和当前 authority 执行一个有界 handler；
- 续租、记录进度、提交结果或明确停止；
- 对预期临时失败有限重试；
- 对失去租约、授权或隐私资格立即失败关闭；
- 将后续工作作为新 durable work 原子写入，不在内存里维持隐形链；
- 不拥有 API、页面、Topic、Claim 或 Outcome 的业务决定。

API 与 worker 调用相同的模块 facade；不能分别实现两套不变量。

## Durable Work 的所有权

### 不建立万能工作流对象

不要设计：

```text
Workflow
WorkflowStep
WorkflowNode
WorkflowEdge
WorkflowVariable
```

然后把 Capture、Observation、Analysis、Privacy、Agent 和 Action 都塞进去。它会把业务不变量变成字符串配置和 JSON，最后任何模块都能绕过责任边界。

### 推荐责任

每项 durable work 必须有唯一 owner module。owner 定义：

- 工作为什么存在；
- 固定输入引用和版本；
- 资格/authority 检查；
- 幂等结果；
- 可重试与永久失败；
- 成功后可以产生哪些后续工作；
- 用户看到的业务含义。

通用 runner 只提供：

- claim/lease/renew/fence；
- scheduled availability；
- attempt history；
- retry policy primitives；
- heartbeat/progress envelope；
- structured error/stop reason；
- handler dispatch；
- metrics/log/trace correlation。

物理上采用共享运行表还是模块专属工作表，留到 schema 设计时以事务、查询和权限证明决定。即使共享运行表，模块输入也必须是类型化、受外键约束的 owner record，不允许把全部业务 payload 塞进一个任意 JSON。

## 最小运行生命周期

```text
PENDING
已持久化，尚未到执行时间或依赖未满足
        ↓
READY
当前有资格被领取
        ↓ claim
LEASED
某个 worker attempt 拥有临时执行权
        ↓
SUCCEEDED
该后台步骤的合同完成

或

RETRY_WAIT
临时失败，已确定下次时间和剩余预算

或

STOPPED
取消、过期、权限撤回、输入失效或业务上已无必要

或

DEAD_LETTER
超过有限重试或出现需要调查的永久失败
```

这些是运行生命周期，不表示相关 Capture、Observation、Analysis 或 Claim 已成功。模块业务结果必须另有类型化 receipt。

### Worker Attempt

每次领取建立独立 worker attempt，至少记录：

- durable work identity 与 owner type；
- runner/worker identity、build 和 handler version；
- lease epoch、开始/结束/心跳时间；
- 固定输入 revision/manifest；
- authority/隐私检查版本；
- result/stop/error category；
- 资源使用和外部调用 receipt；
- 是否重试、接管或人工处置。

一次业务 Work Order 的 producer Attempt、一次 durable work 的 worker attempt、一次 Agent Invocation 和一次外部 Action Attempt 是不同对象，名称再像也不能混表或混状态。

## Claim、lease 与数据库围栏

### 领取

在真实 PostgreSQL 16 短事务内：

1. 选择 `READY` 且当前时间、依赖、owner 状态和 shard 有资格的工作；
2. 使用行锁/跳过锁或等价条件更新，避免多个 worker 同时获得权力；
3. 生成新 worker attempt 与单调 lease epoch；
4. 设置有限 lease 到期时间；
5. 提交后才开始外部或长时间处理。

精确 SQL 留到实现，但验收必须证明两个 worker 并发最多一个获得有效权力，而不是仅靠应用先查后写。

### 续租

- handler 必须在约定时间内 heartbeat；
- renew 只能由当前 epoch 完成；
- renew 不改变输入、handler version 或业务目标；
- 数据库不可达、权限撤回或隐私阻断时不得继续产生新的外部副作用；
- 长处理应按有业务意义的小批次切开，不能靠无限续租维持数小时大事务。

### 提交围栏

最终业务写入必须在同一事务内确认：

- work/attempt 仍是当前 epoch；
- owner input/revision 仍适用；
- 相关授权和隐私没有失效；
- 幂等结果尚未由其他 attempt 提交；
- 结果与必要后续 durable work 同时提交。

事务外的 `is_authorized=true` 或 worker 内存中的 lease 不能保护不可变事实。

## 幂等与去重

### 幂等键来自业务语义

不能把随机 `job_id` 当成幂等证明。每类工作要定义稳定语义，例如：

```text
process capture record
= accepted_record_id + processor_contract_version

build content current
= source_content_id + resolution_policy_version + observation_watermark

classify material set
= input_manifest_hash + topic_release_id + classifier_version

propagate privacy disposition
= disposition_id + consumer_kind + consumer_revision
```

相同语义重跑返回相同结果/receipt；输入或版本改变才产生新工作。并发唯一性最终由数据库约束或条件更新证明。

### 运行去重不删除历史

- 多个上游用途可以等待同一等价工作结果；用途关系分别保留；
- 已完成工作可以被新的调用者复用，但不能把新的用途假装成旧时就存在；
- retry 创建新 worker attempt，不创建新业务结果；
- reprocess 使用新 processor/version，保留旧结果和血缘；
- 手工“重新执行”必须说明是 replay、retry、reprocess 还是新业务需要。

## 重试、接管与 Dead Letter

### 失败分类

| 分类 | 示例 | 默认处理 |
|---|---|---|
| transient | 短暂网络/数据库连接、供应商 5xx | 有限退避重试 |
| rate/resource | 模型限流、当前无工位、预算暂不可用 | 延迟到明确时间，执行前复评 |
| lost_authority | lease/授权/隐私/owner revision 失效 | 立即停止，不自动重试旧输入 |
| invalid_input | schema、身份、manifest、引用不合格 | 永久失败，修正源头后新工作 |
| deterministic_bug | 同输入稳定崩溃、违反不变量 | Dead Letter + 阻断相同版本批量扩散 |
| external_unknown | 调用超时但无法判断外部动作是否发生 | 先查幂等/回执，不盲目重复副作用 |
| policy_denied | 用途、材料、行动不获授权 | 终止或形成决策包，不能自动扩权 |

### 重试预算

每类 handler 显式定义：

- 最大 attempt 数；
- 最大总历时；
- 退避和抖动；
- 哪些错误不可重试；
- 输入过期后的处理；
- Dead Letter 的告警和恢复责任；
- 人工恢复后使用旧 attempt 还是新 work/revision。

第一阶段不把具体次数写成全局常量。协议测试必须证明“有限”和“到期停止”，真实运行后再校准参数。

### 接管

worker 崩溃或 lease 过期后，新的 attempt 可以接管尚未提交的 durable work，但：

- 先检查是否存在已发生但未确认的外部副作用；
- 使用外部幂等键或状态查询，不直接重做；
- 老 attempt 的迟到提交会被 epoch fence 拒绝；
- 接管不改变冻结输入；输入需要变化时结束旧 work 并创建新 work。

## Scheduler 与持续观察

Scheduler 只负责把已批准的时间规则转成到期 occurrence/need，不直接访问平台或执行分析。

逻辑关系：

```text
Observation Objective / approved schedule revision
        ↓ due calculation
Schedule Occurrence
        ↓ creates or re-evaluates
Information Need / Acquisition Request
        ↓ normal admission
Work Order
```

必须保留：

- 时区和日历规则；
- schedule revision 与生效区间；
- 计划到期时间和实际评估时间；
- missed/overlap/catch-up policy；
- 暂停、恢复和过期原因；
- 一次漏跑不通过“补跑多次”污染观察面。

系统时钟、平台来源时间和业务窗口分开。数据库时间可作为 lease/调度权威候选，具体精度和时钟策略进入实现 ADR 与故障测试。

## 人工决定不占用 worker

当分析、Agent、采集或 Action 需要人决定：

1. 当前 handler 生成有明确后果的 Decision Package；
2. 原子保存候选、依据、反例、资源影响和当前运行 stop reason；
3. 当前 durable work 成功结束为“已形成待决定事项”，不是一直 `RUNNING`；
4. 人确认/拒绝/过期形成新的 Decision；
5. 需要继续时，由 Decision 产生新的、带新授权的 durable work。

不维持占用 lease 的“等待人工”任务，也不让 Agent 在内存中挂起数天。

## 内部后续工作与 Outbox

### 数据库内后续工作

如果来源历史与下游 durable work 位于同一 PostgreSQL 数据库，应在同一事务直接写入两者，不为了形式统一先发一个内存事件再异步补写。

例如：

```text
Capture Package accepted
同一事务：
  Evidence records
  ingress receipt
  record-processing durable work
```

这样不会出现“Evidence 已有，但事件丢了”的裂缝。

### Outbox 的使用边界

只有跨出当前数据库边界、无法同一事务完成的副作用才需要 outbox/receipt，例如：

- 对象存储最终写入或删除；
- 外部模型调用（通常保存 invocation 后由 worker 执行）；
- 外部通知；
- 将来获准的发布/第三方系统调用。

Outbox 不是模块之间的默认通信总线，也不是 Kafka 模拟器。每条 delivery 要有 destination、幂等键、payload/version、attempt、receipt 和 unknown outcome 处理。

## Analysis 与 Agent 的后台运行

### Analysis Run

- 先冻结 input manifest、算法/模型/参数、Topic release 和用途；
- 大输入按固定 shard 切分，每个 shard 有独立 durable work 和幂等结果；
- finalize 只在所需 shard 状态、Coverage 和 stop policy 满足时原子发布 run result；
- 某些 shard 失败可以形成明确 partial result，但不能自动发布为完整统计；
- 新算法产生新 run，不覆盖旧结果。

### Agent Invocation

- 一次 Invocation 自身是有预算的运行对象，不等同于 durable runner attempt；
- worker 可以执行一次 Invocation step，但工具调用仍逐项过权限 gate；
- provider 超时、schema 错误和预算耗尽使用 Agent 的 stop contract；
- 人工决定会结束 Invocation；
- 模型 fallback 必须预先授权且产生新的 provider/model attempt，不静默伪装成原模型结果。

## 隐私处置的运行优先级

隐私阻断不是普通低优先级批处理。

```text
Disposition authorized
        ↓ 同一事务
立即阻断新的权威读取/输出
+ 产生各 consumer propagation work
        ↓
索引 / embedding / feature / selection / summary / cache / Agent output
逐项处置并回执
        ↓
全部完成或明确残留/失败
```

- 即时阻断与后续物理传播分开；传播未完成时旧投影也不能继续对外可读；
- 每个消费者定义自己的处置行为，不能一个通用 delete SQL 猜关系；
- 传播失败进入高优先级告警，不静默完成 Case；
- 历史 Claim/Brief 保留最小审计与“来源资格已变化”，是否重评由明确工作完成；
- backup 中的处置策略、保留期限和法律要求仍为 `DECISION_REQUIRED`。

## 可观测性：五类记录分开

### 1. 业务历史

Evidence、Observation、Definition、Claim、Decision、Action、Outcome 等权威历史。不能从日志恢复业务真相。

### 2. 运行台账

Durable work、worker attempt、lease、retry、stop、dead letter、delivery receipt。用于证明后台链是否完成。

### 3. 安全审计

谁以什么 Delegation/用途读取受限材料、改变正式知识、授权采集/行动或执行隐私处置。不可与普通调试日志混在一起。

### 4. 结构化日志与 trace

用于诊断一次请求/工作跨模块经历什么。至少关联：request、actor/delegation、work/attempt、module operation、source/evidence/analysis/claim 的受限引用、build/version 和 error category。

日志避免：完整原文、Cookie/Token、任意 payload dump、数据库 DSN、供应商 secret 和未经脱敏的模型 prompt/result。

### 5. 指标与告警

运行指标候选：

- API 接受/拒绝/耗时与错误类别；
- READY backlog、最老等待时间、lease 丢失、接管、重试、dead letter；
- 各 owner/handler/build 的吞吐与失败；
- Package → Observation/Current 水位；
- Analysis shard/finalize 水位；
- Agent 调用成本、tool 次数、stop reason、schema/引用失败；
- 隐私阻断与传播残留；
- outbox 未确认和 external unknown；
- 数据库连接、锁等待、事务失败、存储增长和备份状态。

告警必须指向可处理的责任模块和 runbook，不以“某日志出现 error”代替。业务趋势、评论量或模型 confidence 不是系统健康指标。

## API 返回与用户可见状态

API 不返回模糊 `ok: true` 作为完整成功。建议统一产品 envelope 表达：

```text
result
同步已确定的结果/receipt

processing
仍在执行的阶段、可查询引用和当前水位

limitations
Coverage、unknown、权限、来源和未证明范围

provenance
数据时间、release/run/build 和依据引用

next
系统会自动继续、需要决定、可以重试或已经停止
```

HTTP 202 只说明请求已接纳为后台工作，不说明业务结果成功。HTTP 200 也只说明该 API 合同成功，不自动证明数据库副作用或外部行动。

## 数据库角色与运行权限

第一阶段保持少量、真实有差异的角色，不按每张表创建账号：

| 角色候选 | 可以 | 不可以 |
|---|---|---|
| migration owner | 在受控发布中执行已审查 migration | 作为 API/worker 日常账号 |
| API app | 调用所需函数/表完成同步业务事务 | 直接修改不可变 Evidence bytes、绕过审计读取受限原文 |
| worker app | claim work、调用所需模块写入派生/运行结果 | 获得 migration 或无边界原文权限 |
| evidence ingress | 接受合同范围内 Package 与 receipt | 发布 Topic/Claim/Decision |
| controlled reader | 按 actor/delegation/purpose 受审计读取 | 通用表直读与批量导出 |
| backup/ops | 执行备份/恢复和最小诊断 | 作为产品调用身份 |

最终是否合并 API/worker/ingress 角色由真实操作和 PostgreSQL 负向测试决定。最小权限需要数据库拒绝证明，不是 Rust 代码里一个 `if`。

## 部署拓扑

第一阶段推荐：

```text
一套版本化发布物
  linggan-api
  linggan-worker
  migration binary/command（只在发布门执行）

一个 PostgreSQL 16 主库
一个对象存储 seam（只有真实 Artifact 需要时启用）
一个或少量受控浏览器工位
外部模型 provider（Agent 阶段再接）
```

- API/worker 可以先部署在同一主机的不同进程/容器；
- 二者使用相同代码版本或明确兼容矩阵；
- 不依赖进程内共享内存；
- 扩 worker 数量只提高同一 durable-work 消费能力，不改变业务语义；
- 不在第一阶段增加只读副库、分片、多区域、服务网格或独立调度服务；
- 开发环境继续本机 Rust + Docker PostgreSQL 16，与旧数据库隔离。

## 发布、迁移与恢复门

### 发布顺序

1. 备份/恢复能力和当前 schema version 可验证；
2. expand-first migration（如有）由专用身份执行；
3. migration 的约束、索引、角色和负向测试通过；
4. 部署兼容 API/worker；
5. 观察 backlog、error、lock、version mismatch；
6. 仅在代码已不读旧形态且有真实证明后做 contract/retirement；
7. 插件与外部 Agent 分别通过版本门，不与服务端发布隐式捆绑。

没有 `db push`、自动 destructive migration、启动时自动改 schema 或旧字段 fallback。

### 备份与恢复

- PostgreSQL 备份和对象 Artifact 备份分别验证；
- 加密、访问、保留期限和隐私处置策略需明确；
- “备份任务成功”不等于恢复可用，必须定期在隔离环境真实恢复并核对关键不变量；
- 恢复后检查 schema、Evidence hash、Current 可重建、durable work 水位、权限角色和隐私阻断；
- 不把生产 dump 提交仓库或交给 Agent；
- 第一阶段具体 RPO/RTO 需要用户根据业务损失可接受度确认。

## 建议代码结构与巨型文件防护

```text
runtime/
  runner/
    claim.rs
    lease.rs
    dispatch.rs
    attempt.rs
  retry/
    policy.rs
    classification.rs
  scheduling/
    due.rs
    occurrence.rs
  delivery/
    outbox.rs
    receipt.rs
  observability/
    correlation.rs
    metrics.rs
    audit.rs
  testing/
    deterministic_clock.rs
    fault_injection.rs
```

这不表示立即创建一个 `runtime` crate。runner 应是小执行底座；owner handler 仍放在各深模块内部。禁止：

- 一个包含所有 `JobKind` 和全部业务分支的 2000 行 dispatcher；
- 一个全局 `job_payload JSONB` 由 handler 自己猜 schema；
- 通用 `retry_everything`；
- route/cron/worker 各写一套状态更新；
- 仅为测试创建与 PostgreSQL 行为不一致的内存队列。

文件规模继续受 [`module-architecture.md`](module-architecture.md) 门禁约束。

## 可证伪验收矩阵

### Durable runner

1. 100 个并发 claim 对一个 work 最多产生一个有效 lease；
2. 老 epoch 的 renew/finalize 均被数据库拒绝；
3. worker 在提交业务结果前崩溃，lease 到期后可接管且无重复结果；
4. 业务结果已提交但响应丢失，重试得到相同 receipt；
5. 输入 revision/隐私资格变化后旧 attempt 不能 finalize；
6. retry 次数和总历时到达上限后进入明确终态；
7. deterministic bug 不会被无限重试淹没；
8. Dead Letter 恢复必须生成有审计的新决定/工作，而非直接改状态。

### Scheduler

1. 时区/DST/暂停/恢复/漏跑产生可解释 occurrence；
2. 两个 scheduler 并发不会重复产生同一 occurrence；
3. 漏跑补偿不制造多次同一观察；
4. schedule revision 后旧 occurrence 保持当时规则；
5. 到期只产生 Need/准入，不直接创建越权插件任务。

### Analysis/Agent

1. shard 缺失时 finalize 不发布完整结果；
2. partial 结果带实际 manifest/Coverage，不能冒充 full；
3. 相同 input+version 重跑幂等；新版本保留双历史；
4. provider unknown outcome 不盲目重复计费/行动；
5. 人工决定不占用 lease，确认后新 invocation/work 使用新授权。

### 隐私与安全

1. disposition 生效后，API、worker、read model、Agent tool 均立即阻断；
2. 传播某消费者失败时 Case 不显示全部完成；
3. API/worker/reader 使用错误数据库身份时由 PostgreSQL 拒绝；
4. 日志/trace/metric fixture 不泄漏 secret、完整原文或 DSN；
5. 恢复备份后仍保留隐私阻断和审计最小痕迹。

### 运行验收

1. 真实 API 请求同时证明返回 receipt、数据库历史和 durable work；
2. worker 完成同时证明业务结果、副作用和后续水位；
3. 进程 kill、数据库短暂不可用、网络断开、模型超时、对象存储超时均有预期恢复；
4. 日志、指标和数据库台账对同一 correlation identity 可互相核对；
5. 一条用户可见“处理中/完成/部分/失败”能追到真实状态而非页面猜测。

## 仍需收口的决定

| 编号 | 决定 | 当前推荐 | 后续证明 |
|---|---|---|---|
| `DEC-G6-OPS-01` | Durable work 共享运行表还是模块专属表 | 先冻结 owner/typed-input 契约，物理形态在首切片 schema 推演决定 | claim 查询、事务与权限测试 |
| `DEC-G6-OPS-02` | lease/schedule 权威时钟 | PostgreSQL 时钟作为候选 | 故障、时钟漂移和 DST 测试 |
| `DEC-G6-OPS-03` | retry/lease 默认参数 | 不设万能值；按 handler 有界策略 | synthetic fault 与真实时长分布 |
| `DEC-G6-OPS-04` | API/worker/ingress DB 角色是否分开 | 保留逻辑分责，按负向测试决定最小物理角色 | PostgreSQL role attack tests |
| `DEC-G6-OPS-05` | 对象存储启用时点 | 首个需要完整 Artifact 字节且本地/DB 不合适时再引入 | 大小、成本、隐私和恢复实验 |
| `DEC-G6-OPS-06` | RPO/RTO 与备份保留 | 用户按可接受业务损失决定 | 隔离恢复演练 |
| `DEC-G6-OPS-07` | 可观测性技术选型 | 先冻结结构化事件/指标合同，不先选重平台 | 本地与首部署运维需求 |

## Gate 6 Runtime 退出条件

1. API、worker、business plan、durable work、external delivery 与业务结果分责；
2. owner module 拥有工作语义，runner 不成为万能 Workflow；
3. claim/lease/epoch/finalize 在真实 PostgreSQL 并发下失败关闭；
4. 每类活动幂等、有界重试、可接管、可停止、可观察；
5. 人工决定不占用 worker，确认后以新授权继续；
6. 数据库内后续工作与历史同事务，Outbox 只用于真实外部边界；
7. 隐私立即阻断与异步传播分别可靠；
8. API 返回、日志、运行台账、业务历史和安全审计不互相冒充；
9. 发布、数据库 migration、插件、模型和备份恢复都有独立验收门；
10. 没有无限重试、静默 fallback、内存队列真相、万能 JSON job、巨型 dispatcher 或提前分布式化。
