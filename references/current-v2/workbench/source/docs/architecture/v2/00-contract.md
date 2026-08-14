# 00 — V2 协作契约

> 本文件只记录已由用户确认的不变规则。未经验证的具体模型、权限或 SQL 不写入本文件。
>
> Release-B 的精确 CaptureSubmissionV2、CapturePackage、Contract registry、事务与重试合同见 [07 — EvidenceIngress Release-B 实施合同](07-evidence-ingress-release-b-contract.md)。

---

## 已确认规则

1. **单轨**：Release-B 后，生产任一时刻只有一条写入路径和一条读取路径。
2. **禁止双写、双读、fallback**：不得为降低实施难度引入兼容层、旧字段补救或旧接口继续接收新数据。
3. **历史数据不伪造迁移**：历史数据不回填、不生成假 CapturePackage、不保留 legacy 例外。用户已接受放弃旧历史数据。
4. **新数据必须从 EvidenceIngress 进入**：四类 ingress（execution / manual_import / recovery / migration）全部经 EvidenceIngress → CapturePackage / RawSnapshot / RawRecord。
5. **文档通过 ≠ 架构通过**：design-freeze.md 审核通过不等于实施自动放行。B1 前必须通过隔离数据库证明（逐行忠实转录 DDL 编译通过）。
6. **分阶段门禁**：B1 不是一次性改完整个系统。每个阶段只受其直接 blocker 约束；EvidenceIngress、Canonical/Media、Projection、历史退役和物理删除不得互相越级。所有 Release-B blocker 关闭与线上门禁通过，才允许进入最终物理删除。

---

## 已确认的 Evidence 领域契约

本节固化用户已确认的逻辑边界，而不是表、字段、DDL、权限或实现方案。具体模型仍须由 blocker 的证据与验证程序收口。

### E01 — Evidence First（证据优先）

系统必须明确区分四个逻辑层：

```
External Sources
      |
Evidence Layer
      |
Derived Layer
      |
Canonical Business Model
      |
Projection / UI
```

- **Evidence** 是系统在某一时点真实观察到的原始事实。
- **Derived** 是基于事实产生的清洗、分析或 AI 推理结果。
- **Canonical Business Model** 是已经通过 Derived 规则认可的唯一业务事实。
- **Projection** 是面向页面的展示结果，不是新的业务事实来源。
- 页面/UI 只能消费 Projection；AI 与业务服务只能经受控服务消费 Derived 或 Canonical，任何一方都不得直接读取 Evidence。
- AI 推理不得写回或污染原始事实；不得通过改写历史事实代替重新计算加工结果。

### E02 — Immutable Evidence（证据不可静默修改）

- 不得覆盖既有证据的历史内容、采集时间、来源或原始 payload。
- 发现错误时，必须新增 Evidence，并据此产生新的 Derived 结果；不得更新旧 Evidence 伪装为修正。
- 这不等于无限期保存。合规、隐私或数据清理可以处理证据，但必须留下可审计的处置记录与原因，不能静默消失。
- 生命周期必须能够表达 `ACTIVE`、`ARCHIVED`、`REDACTED`、`PURGED` 四种逻辑状态。状态的具体枚举、字段、表和处置记录实现尚未在本文件决定。
- `ARCHIVED` 仍可被 Derived 分析和重算；`REDACTED`、`PURGED` 不得再被读取、分析或投影。

### E03 — Evidence Versioning（证据版本）

- 同一次执行的重复提交，若 capture identity 与 payload hash 都相同，不得新建 Evidence；必须作为重复提交关联到既有 Evidence。
- 不同时间的重新观察必须新建 Evidence，即使观察的是同一业务对象或内容；时间变化本身是业务事实，不能覆盖旧观察。
- 具体的 capture identity、哈希算法、关联结构与幂等实现仍待后续模型契约和验证证明，不得由执行者自行补定。

### E04 — Execution 与 Evidence 分离

- 真实采集执行的逻辑链为 `ExecutionJob → CaptureAttempt → Evidence`。
- 手动导入、数据恢复、历史迁移和人工修复等非执行来源不得伪造 `ExecutionJob` 或 `CaptureAttempt`。
- Evidence 必须能表达其来源类别（如 collection、import、migration、recovery、manual），但来源类别的字段名、值域及持久化模型尚未在本文件决定。

### E05 — Evidence Registry 与 Payload 分离

- PostgreSQL 中的 Evidence Registry 是证据身份、来源、采集时间、哈希、溯源、关系、生命周期及权限元数据的唯一事实来源。
- 本阶段不迁移现有本机媒体文件存储，也不引入新的数据库或对象存储实现；未来改变物理存储必须另经架构决策和迁移验证。
- 任一物理 payload 载体都只能由 Evidence Registry 或 Media Domain 登记和指向，不得成为第二个事实来源。

### E06 — Evidence Quality（证据质量）

- Evidence 必须能让后续处理判断质量、置信度和验证状态；Derived 与 AI 使用事实时必须能据此识别可信程度。
- 官方接口、插件采集、OCR 的高/中/低可信度仅是业务示例，不构成已批准的字段、枚举或评分算法。

### E07 — Evidence Retention Policy（证据留存策略）

- 证据采集和留存必须按对象类型形成策略，而非无差别无限增长。例如作者画像可按日观察、内容正文可按变化采集、评论可增量采集、指标可按小时采集。
- 策略的对象模型、调度器、周期、阈值和删除实现尚未决定；任何实施必须先进入 blocker 和验证矩阵。

### E10 — Media 是独立业务资产

- Media 不属于 Evidence，也不属于 `Content.coverImage`、`Author.avatar`、`Topic.videoUrl` 等业务字段。
- Media Domain 是媒体身份、来源、物化、交付和业务关系的唯一事实来源；业务模块不得直接创建媒体或自行拼接交付 URL。
- 内容、作者、选题等业务对象只通过受控关系引用 Media。关系被撤销时，只撤销该业务关系，不级联删除共享 Media Asset。
- 现有本机媒体物化继续使用既有 Media Domain；本阶段不借此引入新的 MediaRegistration 平台或多层 Resolver。

### E08 — 唯一 Evidence Registry 承载主体

- `RawSnapshot` 是 V2 中一次外部观察的唯一 Evidence Registry；`RawRecord` 只表示该次 `RawSnapshot` 下的原始片段与受控检索索引。
- 既有 `RawEvidence` 不参与 V2 新数据登记，也不得扩展为第二本 Evidence Registry。它的历史退役范围仍须单列证明。
- 这项规则不决定 `RawSnapshot` / `RawRecord` 的字段、键、表权限、退役顺序或 DDL。

### E09 — 包级幂等与观察级追加

- 同一次采集提交，以 capture identity 与完整 Capture Package 的 hash 共同判断重复；二者都相同只关联既有 `RawSnapshot`，不得新建快照或片段。
- 不同 capture identity 或不同时间的重新观察必须新建 `RawSnapshot` 及其 `RawRecord`；片段内容恰好相同也不得跨快照合并、移动或更新。
- 同一 capture identity 但完整包 hash 不同，不得覆盖既有证据或创建可投影的新观察；必须留下可审计的非投影结果。其具体状态、回执和持久化机制仍待模型契约决定。
- hash 算法、capture identity 的字段组成、并发约束和物理幂等实现尚未在本文件决定。

### V2 目标层次

```
AI Agent / Business Services        Page / UI
              |                        |
              +----- Canonical ----- Projection
                          |
                       Derived
                          |
                    Evidence Registry
                          |
             Xiaohongshu / CRM / Other Sources
```

---

## 状态机

```
OPEN → READY_FOR_PROOF → PROVED → CLOSED
  │
  └→ DECISION_REQUIRED ─→ OPEN
```

- **OPEN**：问题已识别，正在补齐可逐项来源化的模型、权限或删除规则；不代表仍需业务决策。
- **DECISION_REQUIRED**：需要用户或架构方做出决策才能继续。
- **READY_FOR_PROOF**：设计已完成，等待隔离数据库验证。
- **PROVED**：执行代理已按该条验证矩阵指定的环境完成实际验证：`ISOLATED_DB` 规则必须在隔离数据库通过正、反例；`SOURCE_AUDIT` 规则必须保存只读命令与输出。执行代理只能推进到此状态。
- **CLOSED**：只有审核方可从 PROVED 推进到此状态。

**隔离数据库验证规则**：当某验证所依赖的 blocker 均为 READY_FOR_PROOF 时即可执行该验证；不要求全部 blocker CLOSED。

**阶段门禁**：

- **Phase 0**：仅固化运行契约、阶段映射和禁止项；不改运行代码或数据库。
- **Phase 1（B1-E，EvidenceIngress）**：只等待 BLK-001、002、003、010、015、016 的模型契约和隔离证明；不等待后续页面或历史迁移项。
- **Phase 2（Canonical / Media）**：等待 BLK-004～009、012～014 的证明；不得保留由 Evidence 直写媒体的路径。
- **Phase 3（Projection）**：普通页面只需 Projection 收口；BLK-011 仅对发布、审核、强一致运营页面的额外展示回执适用。
- **Phase 4（历史迁移）**：仅迁移可证明关系；无法证明的历史记录不得进入展示。
- **Phase 5（物理删除）**：所有 Release-B blocker CLOSED，并连续 48 小时满足旧读、旧写、fallback 和页面绕过均为零。

---

## 协作纪律

- 冻结规范（design-freeze.md）是目标定义来源，但不是已验证可执行的事实。
- B0 审计报告是历史证据；如果审计结论与源码不一致，以源码为准并在 ledger 中记录更正。
- 不得自行解决 blocker；执行代理只能推进到 PROVED。
- 不得为让 DDL 通过而新增字段、表、索引、外键或业务规则。
