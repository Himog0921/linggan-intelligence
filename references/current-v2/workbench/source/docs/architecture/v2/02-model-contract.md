# 02 — 模型契约模板

> 每张表必须填写全部字段。未由 design-freeze.md 或已确认 DEC 明确定义的字段标记 PENDING。
> 不得从散文猜测补全。来源必须指向 design-freeze.md 行号或已确认的 DEC；后者不能被扩写为未确认的字段、DDL 或实现方案。

---

## 已确认的逻辑层边界（不是物理表契约）

| 逻辑层 | 已确认边界 | 本阶段明确不决定 |
|---|---|---|
| Evidence | 真实观察到的原始事实；不可静默覆盖；同次同身份同 hash 重复关联既有事实，不同时间重观察新增事实。 | 表数量、字段、主键、哈希算法、幂等实现与 DDL。 |
| Derived | 对 Evidence 的清洗、分析及 AI 推理结果；不得反向修改 Evidence。 | 是否拆为一个或多个物理表、运行记录、当前指针与写入流程。 |
| Canonical | 由接受的 Derived 形成的唯一业务事实；同一业务实体、同一观察版本只能有一个 Canonical 业务投影。 | 投影对象的物理模型、主键、外键和写入实现。 |
| Projection | 面向页面的展示结果；页面/UI 只能消费 Projection，不得直接读取 Evidence 或媒体表。 | 投影表、版本、缓存、接口和读取权限。 |

- Evidence、Derived、Canonical、Projection 是四个逻辑层，不强制一一对应为四张或多张独立表。
- 未来 Evidence 模型必须能表达观察质量、观察置信度、验证结果与可审计处置状态；名称、类型、值域与存储位置仍为 PENDING。
- PostgreSQL Evidence Registry 是证据身份与元数据的唯一事实来源；本阶段媒体文件继续使用本机现行存储，不在本合同中引入新数据库或对象存储实现。
- 真实执行与非执行来源必须区分，非执行来源不得伪造执行关系；具体字段和约束仍为 PENDING。

### 已确认的 Evidence 承载与重复边界（不是物理表契约）

- `RawSnapshot` 是 V2 一次外部观察的唯一 Evidence Registry；`RawRecord` 只能从属该快照，不能作为另一条独立 Evidence 总账（DEC-E08）。
- 既有 `RawEvidence` 不得承接 V2 新数据或与 `RawSnapshot` 并行充当 Registry（DEC-E08）。
- 包级的 capture identity + 完整 package hash 是同次重传的唯一重复判定边界；不同观察的新快照及其片段必须追加（DEC-E09）。
- `RawRecord` 的内容相同不构成跨快照合并依据；不得更新、移动或删除旧片段来表示新观察（DEC-E02、DEC-E09）。
- 同一 capture identity 与不同完整 package hash 的提交，不得修改既有证据或进入领域投影；必须保留完整冲突证据并提供可审计的非投影结果（DEC-B1-008）。
- Media 是独立的 Canonical 业务资产。内容、作者、选题等对象只保存受控媒体关系；媒体物理交付 URL 不是业务字段，也不是页面的直接读取来源。
- 上述规则只完成逻辑边界；RawSnapshot/RawRecord 仍为 PENDING，必须在字段、键、关系、角色和删除规则均可忠实来源化后，才可变为 `READY_FOR_PROOF`。

---

## 模板

每张表必须包含以下全部项：

```
表名：
来源（design-freeze 行号或已确认 DEC）：
| 字段 | 类型 | 可空 | 默认值 | 来源行号 |
|---|---|---|---|---|
主键：
唯一键：
外键：
索引：
写入者：
读取者：
删除规则：
来源决策 ID：
```

---

## 表清单（需逐个填写）

以下为冻结规范 §5 中定义的全部目标表。本轮状态只表示“是否能够逐字段转录”，不表示模型已经创建或可实施。

| # | 模型 | 来源状态 | 本文件落位 / 阻塞 |
|---:|---|---|---|
| 1 | EvidenceIngressReceipt | SOURCE_COMPLETE | 已转录；FK 依赖 BLK-001 |
| 2 | CapturePackage | SOURCE_COMPLETE | 已转录；本模型无出站 FK，RawSnapshot 持有反向关系 |
| 3 | CaptureArtifact | SOURCE_COMPLETE | 已转录；FK 依赖 BLK-001 |
| 4 | RawSnapshot | SOURCE_INCOMPLETE | BLK-001 / DEC-B1-001、008、009 |
| 5 | RawRecord | SOURCE_INCOMPLETE | BLK-001、BLK-002 / DEC-B1-001、008、009 |
| 6 | NormalizationRun | SOURCE_COMPLETE | 三列同源合同已确认；B2-B-01 隔离证明已通过 |
| 7 | NormalizationRunCurrent | SOURCE_COMPLETE | 三列同源合同已确认；B2-B-01 隔离证明已通过 |
| 8 | CanonicalObservation | SOURCE_COMPLETE | 物理合同与 XHS 首期输出语义已由 DEC-B2-002 确认并暗态实现 |
| 9 | ContractEvaluation | SOURCE_COMPLETE | 已转录；Snapshot FK 依赖 BLK-001 |
| 10 | ContractEvaluationCurrent | SOURCE_COMPLETE | 已转录 |
| 11 | ContractEvaluationNormalizationRun | SOURCE_COMPLETE | 已转录；B2-B-01 同源关系隔离证明已通过 |
| 12 | ContractEvaluationInput | SOURCE_COMPLETE | 已转录；B2-B-01 同源关系隔离证明已通过 |
| 13 | CanonicalMediaSlot | SOURCE_COMPLETE | 已转录 |
| 14 | ContentObservation | SOURCE_COMPLETE_DARK | DEC-B3-007/009 已确认物理身份、同源复合 FK、不可变边界与简单来源 URL；暗态实现和 55 项隔离证明已通过 |
| 15 | ContentCurrentProjection | SOURCE_COMPLETE_DARK | DEC-B3-007/008 已确认受控推进、Evaluation 指针、同实体 FK 与原子撤销；暗态实现和 55 项隔离证明已通过 |
| 16 | AuthorObservation | SOURCE_INCOMPLETE_IDENTITY_CONFIRMED | B3-A-02 已确认 XHS 身份/字段来源；新增列类型/可空性、同 workspace 关系仍受 BLK-006 阻塞 |
| 17 | AuthorCurrentProjection | SOURCE_COMPLETE_WITH_RELATION_GAP | 字段与状态所有权已转录；同实体 Current FK、onDelete 仍受 BLK-007、BLK-014 阻塞 |
| 18 | Comment | DECISION_REQUIRED_IDENTITY_CONFIRMED | B3-A-02 已确认 `(noteId,commentId)`；同一评论跨观察的不可变历史与 Current 推进仍待 DR-B3-004，BLK-008 阻塞 |
| 19 | ContentMetricSnapshot | NOT_IN_FIRST_XHS_SLICE | 六合同无 metric RawRecord；BLK-009 保持 OPEN，不为首期伪造来源 |
| 20 | ContentMetricLatest | NOT_IN_FIRST_XHS_SLICE | 同上；V1 指标链不能替代 V2 Canonical 来源 |
| 21 | TaskStatusProjection | SOURCE_INCOMPLETE | BLK-010 / DEC-B1-004 |
| 22 | PresentationRequirement | SOURCE_COMPLETE_WITH_AUTHORITY_GAP | 已转录；BLK-011、BLK-013 |
| 23 | PresentationReceipt | SOURCE_COMPLETE_WITH_AUTHORITY_GAP | 已转录；BLK-011、BLK-012、BLK-013 |
| 24 | EvidenceAccessAudit | SOURCE_COMPLETE_WITH_AUTHORITY_GAP | 字段已转录；`capturePackageId` 的同 workspace 关系与审计写入边界受 BLK-015 / DEC-B1-007 约束 |
| 25 | EvidenceReaderWorkspaceGrant | SOURCE_COMPLETE_WITH_AUTHORITY_GAP | 已转录；BLK-015 / DEC-B1-007 |

## 已逐字段转录的模型契约（含明确关系/权限缺口）

> 下列合同逐字段忠实转录自冻结规范的完整 `model` 块；它们不是 Prisma schema、DDL 或实施授权。`当前实现`一栏只说明经 `prisma/schema.prisma` 核实的 V2 表尚未创建，不能被误读为这些对象已运行。写入/读取/删除规则来自冻结规范的领域所有权与最小权限矩阵。表清单标记为 `SOURCE_COMPLETE_WITH_*_GAP` 的对象，字段虽已转录，但关系或权限仍保持 `PENDING`；不得据此进入实施。

### EvidenceIngressReceipt

来源：[冻结规范 §5.1](../content-workbench-v2-design-freeze.md#L108-L125)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `gen_random_uuid()::text` | freeze:L111-L112 |
| workspaceId | String | 否 | — | freeze:L113 |
| rawSnapshotId | String | 否 | — | freeze:L114 |
| captureId | String | 否 | — | freeze:L115 |
| ingressKind | String | 否 | — | freeze:L116 |
| collectorVersion | String | 否 | — | freeze:L117 |
| receivedAt | DateTime | 否 | `now()` | freeze:L118 |

- 主键：`id`；唯一键：`(workspaceId, rawSnapshotId)`、`(workspaceId, id)`；索引：`(workspaceId, receivedAt)`（freeze:L119-L121）。
- 外键：`(workspaceId, rawSnapshotId)` 指向 RawSnapshot 的同工作区 ID（freeze:L122；RawSnapshot 物理合同受 [BLK-001](04-blocker-ledger.md#L14-L24) 约束）。
- 写入者：仅 `evidence_writer`；读取者：本入口回执验证链；删除规则：不可 UPDATE/DELETE（freeze:L123-L124）。
- 来源决策：DEC-E01、DEC-E04、DEC-E08、DEC-E09。

### CapturePackage

来源：[冻结规范 §5.2](../content-workbench-v2-design-freeze.md#L128-L142)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L131-L132 |
| workspaceId | String | 否 | — | freeze:L133 |
| packagePayload | Bytes | 否 | — | freeze:L134 |
| checksumAlgorithm | String | 否 | `sha256` | freeze:L135 |
| checksumValue | String | 否 | — | freeze:L136 |
| contentLength | Int | 否 | — | freeze:L137 |
| restricted | Boolean | 否 | `false` | freeze:L138 |
| createdAt | DateTime | 否 | `now()` | freeze:L139 |

- 主键：`id`；唯一键：`(workspaceId, id)`（freeze:L140）。
- 外键（本模型持有）：无；冻结规范的完整模型没有列出出站 FK。`RawSnapshot.capturePackageId` 才是指向本模型的反向复合关系（freeze:L163-L169；其最终物理证明受 BLK-001 约束）。写入者：`evidence_writer`；读取者：只允许元数据读取，原始包读取边界另受权限合同约束（freeze:L604、L628）。
- 删除规则：全角色不可 UPDATE/DELETE（freeze:L141）。
- 来源决策：DEC-E02、DEC-E05、DEC-E09。

### CaptureArtifact

来源：[冻结规范 §5.3](../content-workbench-v2-design-freeze.md#L145-L160)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L148-L149 |
| workspaceId | String | 否 | — | freeze:L150 |
| rawSnapshotId | String | 否 | — | freeze:L151 |
| kind | String | 否 | — | freeze:L152 |
| artifactChecksum | String | 否 | — | freeze:L153 |
| restricted | Boolean | 否 | `false` | freeze:L154 |
| createdAt | DateTime | 否 | `now()` | freeze:L155 |

- 主键：`id`；唯一键：`(workspaceId, id)`、`(workspaceId, rawSnapshotId, id)`（freeze:L156-L157）。
- 外键：`(workspaceId, rawSnapshotId)` 指向 RawSnapshot 同工作区 ID（freeze:L158；受 BLK-001 约束）。
- 写入者：`evidence_writer`；读取者：仅 ID/快照/工作区元数据按权限读取（freeze:L607）；删除规则：不可 UPDATE/DELETE（freeze:L159）。
- 来源决策：DEC-E02、DEC-E05、DEC-E08。

### NormalizationRun

来源：[冻结规范 §5.6](../content-workbench-v2-design-freeze.md#L224-L250)；当前实现：B2-B-01 已完成 schema expand，B2-B-02 已实现暗态 writer，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L227-L228 |
| workspaceId | String | 否 | — | freeze:L229 |
| rawSnapshotId | String | 否 | — | freeze:L230 |
| rawRecordId | String | 否 | — | freeze:L231 |
| adapterId | String | 否 | — | freeze:L232 |
| adapterVersion | String | 否 | — | freeze:L233 |
| canonicalSchemaVersion | String | 否 | — | freeze:L234 |
| attemptNumber | Int | 否 | `1` | freeze:L235 |
| status | String | 否 | — | freeze:L236 |
| inputPayloadHash | String | 否 | — | freeze:L237 |
| outputPayloadHash | String | 是 | — | freeze:L238 |
| missingFields | Json | 是 | — | freeze:L239 |
| parseErrors | Json | 是 | — | freeze:L240 |
| startedAt | DateTime | 否 | `now()` | freeze:L241 |
| completedAt | DateTime | 否 | `now()` | freeze:L242 |
| createdAt | DateTime | 否 | `now()` | freeze:L243 |

- 主键：`id`；唯一键：`(rawRecordId, adapterId, adapterVersion, canonicalSchemaVersion, attemptNumber)`、`(workspaceId, id)`、`(workspaceId, rawSnapshotId, id)`（freeze:L244-L246）。
- 外键：Snapshot 与 Record 关系均携带同一 workspace、rawSnapshotId（freeze:L247-L248；DEC-B2-001）；[BLK-003](04-blocker-ledger.md)、[BLK-016](04-blocker-ledger.md) 隔离证明已通过。
- 写入/读取者：`canonical_writer` INSERT+SELECT（freeze:L609）；删除规则：完全不可变（freeze:L249、L628）。
- 来源决策：DEC-E01、DEC-E02、DEC-E08。

### NormalizationRunCurrent

来源：[冻结规范 §5.6](../content-workbench-v2-design-freeze.md#L252-L267)；当前实现：B2-B-01 已完成 schema expand，B2-B-02 已实现暗态 writer，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| workspaceId | String | 否 | — | freeze:L253 |
| rawRecordId | String | 否 | — | freeze:L254 |
| rawSnapshotId | String | 否 | — | freeze:L255 |
| normalizationRunId | String | 否 | — | freeze:L256 |
| adapterId | String | 否 | — | freeze:L257 |
| adapterVersion | String | 否 | — | freeze:L258 |
| canonicalSchemaVersion | String | 否 | — | freeze:L259 |
| updatedAt | DateTime | 否 | `now(), @updatedAt` | freeze:L260 |

- 主键：`(workspaceId, rawRecordId)`；唯一键：`(workspaceId, normalizationRunId)`、`(workspaceId, rawSnapshotId, rawRecordId)`（freeze:L261-L263）。
- 外键：NormalizationRun 与 RawRecord 关系均携带同一 workspace、rawSnapshotId（freeze:L264-L265；DEC-B2-001）；[BLK-003](04-blocker-ledger.md)、[BLK-016](04-blocker-ledger.md) 隔离证明已通过。
- 写入/读取者：`canonical_writer` INSERT+SELECT+UPDATE（freeze:L610）；删除规则：冻结规范未单列，指针更新以本行 `updatedAt` 表达（freeze:L260、L266）。
- 来源决策：DEC-E01、DEC-E08、DEC-E09。

### CanonicalObservation

来源：[冻结规范 §5.7](../content-workbench-v2-design-freeze.md#L270-L294)；当前实现：B2-B-01 已完成 schema expand；DEC-B2-002 的 XHS 技术 Canonical 已在 B2-B-02 暗态实现，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L273-L274 |
| workspaceId | String | 否 | — | freeze:L275 |
| normalizationRunId | String | 否 | — | freeze:L276 |
| rawSnapshotId | String | 否 | — | freeze:L277 |
| rawRecordId | String | 否 | — | freeze:L278 |
| observationKind | String | 否 | — | freeze:L279 |
| subjectKey | String | 否 | — | freeze:L280 |
| observedAt | DateTime | 否 | — | freeze:L281 |
| schemaVersion | String | 否 | — | freeze:L282 |
| payload | Json | 否 | — | freeze:L283 |
| payloadHash | String | 否 | — | freeze:L284 |
| qualityStatus | String | 否 | — | freeze:L285 |
| fieldPresence | Json | 是 | — | freeze:L286 |
| createdAt | DateTime | 否 | `now()` | freeze:L287 |

- 主键：`id`；唯一键：`(workspaceId, id)`、`(workspaceId, rawSnapshotId, id)`（freeze:L288-L289）。
- 外键：Run/Record 均要求同 workspace、同 snapshot（freeze:L290-L292）；BLK-003、BLK-016 隔离证明已通过。
- 写入/读取者：`canonical_writer` INSERT+SELECT，`default_app` 仅消费已脱敏 Canonical 数据（freeze:L611）；删除规则：不可变（freeze:L293、L628）。
- 来源决策：DEC-E01、DEC-E02、DEC-E06、DEC-E08。
- 运行语义：DEC-B2-002 已固定 identity、版本、技术 Canonical、fieldPresence、canonical JSON/SHA-256、诊断与 Current 规则；六份插件 fixture 的输出 hash 已由跨仓门禁锁定。暗态实现不等于已授权切流。

### ContractEvaluation

来源：[冻结规范 §5.8](../content-workbench-v2-design-freeze.md#L297-L321)；当前实现：B2-B-01 已完成 schema expand；DEC-B2-002 的 XHS 裁决语义已在 B2-B-02 暗态实现，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L300-L301 |
| workspaceId | String | 否 | — | freeze:L302 |
| rawSnapshotId | String | 否 | — | freeze:L303 |
| contractId | String | 否 | — | freeze:L304 |
| contractVersion | Int | 否 | — | freeze:L305 |
| evaluatorVersion | String | 否 | — | freeze:L306 |
| canonicalSchemaVersion | String | 否 | — | freeze:L307 |
| evaluationInputHash | String | 否 | — | freeze:L308 |
| decision | String | 否 | — | freeze:L309 |
| completeness | String | 否 | — | freeze:L310 |
| rejectionCode | String | 是 | — | freeze:L311 |
| rejectionReason | String | 是 | — | freeze:L312 |
| createdAt | DateTime | 否 | `now()` | freeze:L313 |

- 约束：accepted 仅配 full/partial，rejected 仅配 not_applicable（freeze:L314-L315）。
- 唯一键：评价输入版本组合、`(workspaceId,id,rawSnapshotId,contractId,contractVersion)`、`(workspaceId,rawSnapshotId,id)`（freeze:L316-L318）。
- 外键：`(workspaceId,rawSnapshotId)` → RawSnapshot（freeze:L319；依赖 BLK-001）。写入/读取者：`canonical_writer` INSERT+SELECT（freeze:L613）；删除规则：append-only（freeze:L320、L628）。
- 来源决策：DEC-E01、DEC-E02、DEC-E06、DEC-E08。

### ContractEvaluationCurrent

来源：[冻结规范 §5.9](../content-workbench-v2-design-freeze.md#L324-L340)；当前实现：B2-B-01 已完成 schema expand，B2-B-02 已实现暗态 writer，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| workspaceId | String | 否 | — | freeze:L328 |
| rawSnapshotId | String | 否 | — | freeze:L329 |
| contractId | String | 否 | — | freeze:L330 |
| contractVersion | Int | 否 | — | freeze:L331 |
| contractEvaluationId | String | 否 | — | freeze:L332 |
| revision | Int | 否 | — | freeze:L333 |
| updatedAt | DateTime | 否 | `now(), @updatedAt` | freeze:L334 |

- 主键：`(workspaceId,rawSnapshotId,contractId,contractVersion)`；外键：复合同工作区评价 ID/快照/合同/版本（freeze:L335-L337）。
- 写入/读取者：`canonical_writer` INSERT+SELECT+UPDATE（freeze:L614）；删除规则：冻结规范未单列，指针可更新（freeze:L338）。
- 来源决策：DEC-E01、DEC-E02。

### ContractEvaluationNormalizationRun

来源：[冻结规范 §5.10](../content-workbench-v2-design-freeze.md#L342-L360)；当前实现：B2-B-01 已完成 schema expand，B2-B-02 已实现暗态 writer，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L345-L346 |
| evaluationId | String | 否 | — | freeze:L347 |
| normalizationRunId | String | 否 | — | freeze:L348 |
| workspaceId | String | 否 | — | freeze:L349 |
| rawSnapshotId | String | 否 | — | freeze:L350 |
| status | String | 否 | — | freeze:L351 |
| inputPayloadHash | String | 否 | — | freeze:L352 |
| outputPayloadHash | String | 是 | — | freeze:L353 |
| createdAt | DateTime | 否 | `now()` | freeze:L354 |

- 主键：`id`；唯一键：`(evaluationId,normalizationRunId)`、`(workspaceId,id)`（freeze:L355-L356）。
- 外键：同工作区、同 snapshot 的 ContractEvaluation 与 NormalizationRun（freeze:L357-L358）；BLK-016 隔离证明已通过。
- 写入/读取者：`canonical_writer` INSERT+SELECT（freeze:L615）；删除规则：不可变（freeze:L359、L628）。
- 来源决策：DEC-E01、DEC-E02。

### ContractEvaluationInput

来源：[冻结规范 §5.11](../content-workbench-v2-design-freeze.md#L363-L381)；当前实现：B2-B-01 已完成 schema expand，B2-B-02 已实现暗态 writer，尚无 caller。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L366-L367 |
| workspaceId | String | 否 | — | freeze:L368 |
| rawSnapshotId | String | 否 | — | freeze:L369 |
| contractEvaluationId | String | 否 | — | freeze:L370 |
| canonicalObservationId | String | 否 | — | freeze:L371 |
| ordinal | Int | 否 | — | freeze:L372 |
| canonicalOutputHash | String | 否 | — | freeze:L373 |
| createdAt | DateTime | 否 | `now()` | freeze:L374 |

- 主键：`id`；唯一键：`(contractEvaluationId,canonicalObservationId)`、`(contractEvaluationId,ordinal)`、`(workspaceId,id)`（freeze:L375-L377）。
- 外键：同工作区、同 snapshot 的 ContractEvaluation 与 CanonicalObservation（freeze:L378-L379）；BLK-016 隔离证明已通过。
- 写入/读取者：`canonical_writer` INSERT+SELECT（freeze:L616）；删除规则：不可变（freeze:L380、L628）。
- 来源决策：DEC-E01、DEC-E02。

### B2 原子链与失败语义

来源：[冻结规范 §7 B2](../content-workbench-v2-design-freeze.md#L567-L579)。本节只转录已冻结的事务边界，不把 B2 拆成可独立提交的 Derived 与 Canonical 两个写事务：

1. `adapter_reader` / `contract_reader` 通过受控函数读取 CapturePackage 并写读取审计，独立事务提交。
2. `canonical_writer` 在单一 `SERIALIZABLE` 事务中执行 Adapter，创建终态 NormalizationRun；成功时同时创建 CanonicalObservation；推进 NormalizationRunCurrent；验证快照内全部 RawRecord 的 Current 指针；创建 ContractEvaluation 与 ContractEvaluationNormalizationRun；accepted 时创建 ContractEvaluationInput；最后推进 ContractEvaluationCurrent。
3. 该事务不创建 Content、Author、Comment、Metric、Media 或任何 CurrentProjection；这些属于 B3 DomainProjection。
4. 步骤 2 任一点失败时，数据库仅保留步骤 1 的读取审计；不得保留“Derived 已提交、Current 未推进”的半完成状态。重试必须重新从步骤 1 开始，并通过 attemptNumber 收敛；递增或复用规则须由后续运行合同明确，不能从本段自行推出。

CanonicalObservation 虽位于 Canonical 数据层，但它是 B2 合同裁决所需的技术解释结果；不得延迟到 B3 创建。

六份 XHS 输入对七表的逐字段来源已完成复核：27 个身份/关系字段直接绑定，14 个字段由持久化层生成；其余 34 个运行语义已由 DEC-B2-002 方案 A 确认并在暗态实现，不能由模型字段反推或另行改写。完整证据与合同见 [B2 XHS Normalization 来源审计](../../code-review/v2-b2-xhs-normalization-source-audit-2026-08-11.md)。

### CanonicalMediaSlot

来源：[冻结规范 §5.12](../content-workbench-v2-design-freeze.md#L384-L399)；当前实现：B3-MEDIA-SRC-002 已按七字段合同完成 `schema.prisma` expand、不可变 migration、受控 Artifact verifier 与无 caller 暗态 `observed` writer；B3-MEDIA-SRC-003 已用同一不可伪造能力暗态衔接现有 MediaItem/MediaOrigin/`media.processing_requested` 边界，但仍无 caller、业务媒体关系或 Projection 流量。底层数据库角色、EvidenceAccessAudit 与读取函数仍受 BLK-015 阻塞，因此生产代码只接受注入的审计 source，不提供绕过审计的默认数据库 reader。`absent/unavailable` 的来源仍为 DR-B3-005；`live_photo` 的物理映射仍为 DR-B3-006，两者均 fail-closed。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L387-L388 |
| workspaceId | String | 否 | — | freeze:L389 |
| canonicalObservationId | String | 否 | — | freeze:L390 |
| slotId | String | 否 | — | freeze:L391 |
| status | String | 否 | — | freeze:L392 |
| kind | String | 否 | — | freeze:L393 |
| ordinal | Int | 否 | — | freeze:L394 |

- 主键：`id`；唯一键：`(workspaceId,canonicalObservationId,slotId)`、`(workspaceId,id)`（freeze:L395-L396）。
- 外键：`(workspaceId,canonicalObservationId)` → CanonicalObservation（freeze:L397；当前源足以定义，最终依赖 Canonical 链）。
- 状态来源：`observed` 只由同包、同 subject 且通过 `xhs.media-inventory/v2` 的 candidate 产生；DR-B3-005 的方案 A 业务方向虽已确认，但真实 producer 尚不能从 candidate 缺失、合同 slot 或 terminal 结果唯一推出 `absent/unavailable`，实施保持 `SOURCE_INCOMPLETE`。
- Media Domain 暗态回执：每个已绑定 `observed` slot 的处理事件必须保存 `canonicalObservationId/slotId/originId/mediaItemId/generation` 五字段；stable locator 不含 URL，同一事件在 pending/processed 等任意状态下 replay 均不得重新入队。任一身份计划拒绝、槽绑定歧义、入队失败或 `live_photo` 均使整个媒体域事务零推进。
- 写入/读取者：`canonical_writer` INSERT+SELECT，`default_app` SELECT（freeze:L612）；删除规则：不可变（freeze:L398、L628）。
- 来源决策：DEC-E01、DEC-E02、DEC-E08。

## B3-A-02：XHS Domain Projection 合同

> 2026-08-12 硬切候选补充（WIP，非上线声明）：`V2DurableWork` 的 `lockedBy/leaseExpiresAt` 不再只是调度提示。worker 必须在 B2 与 B3 各自 SERIALIZABLE 写事务的首尾锁定同一 work 行，并以数据库 `clock_timestamp()` 重验 owner、processing 状态与未过期 lease；该事务 fence 失败必须回滚本事务全部 Derived/Canonical/Projection/Media mutation。canary 只能按 `(workspaceId,receiptId)` 定向 claim，不能消费任意 pending work。此处没有把 worker 状态解释为业务事实，也没有新增 Domain 模型。

来源：[冻结规范 §5.13、§7](../content-workbench-v2-design-freeze.md#L402-L474)、[B3-A-01 来源审计](../../code-review/v2-b3-domain-projection-source-audit-2026-08-11.md)、DEC-B3-001～009。当前候选实现：Content-only 模型、受控数据库函数、持久化 worker 与唯一 orchestration 已写入 schema/migration；Material 候选只读 V2 Projection，无双写/双读/fallback。尚未部署或执行正式硬切。

### B3 输入资格、身份与字段路径

- B3 只消费 `ContractEvaluationCurrent` 当前指向、且 `decision=accepted`、`completeness IN (full,partial)` 的 `ContractEvaluationInput → CanonicalObservation`。DEC-B3-007 将并发合同收口为 SERIALIZABLE 事务、锁定精确 CEC；B2 撤销协调器与 B3 projector 必须对数据库事实导出的 `(workspaceId,"xhs",platformContentId)` 调用同一个 transaction-scoped advisory lock，并在锁后原子推进内部 `ContentProjectionSubjectFence` 行，即使 `ContentAsset` 尚不存在。该行写入使锁前已建立的旧 SERIALIZABLE snapshot 与较新提交形成 write/write 冲突并返回 40001；projector 只能整事务重试取得新 snapshot，不能继续消费旧视图。B3 随后重验精确 CEC，并查询该 subject 跨所有 snapshot 的 current accepted 输入：已有严格更晚 `observedAt` 时旧输入不推进，等时不同 Observation fail-closed。普通读取不能替代该证明。
- 首切 public Content Projector 只接受恰好一个 note input。accepted Evaluation 的 note cardinality 不等于 1 时，B2 可以保存 Derived/CEC 事实并对涉及 subject 建立并发栅栏，但 Domain revoke 与 Usage mutation 必须为零；不得用 `LIMIT 1` 的某一 note 的 Observation/observedAt 裁决其它 subject。rejected 只可按 previous evaluation 精确关闭已存在的 Current/Usage。
- XHS note 必须同时提供非空 `noteId`、`platformContentId` 且二者完全相等；领域身份为 `(workspaceId,"xhs",platformContentId)`。`ContentAsset.assetKey` 使用现役 `buildContentAssetKey({workspaceId,platform,platformContentId})`，不得接受客户端自报键（`src/lib/data-foundation/content-identity.ts:44-55`；DR-B3-001 A）。B3-CONTRACT-SRC-001 已将该约束写入插件/工作台镜像的 `xhs.record-payload/v2`、六份 source-shaped fixture 与 B2 adapter identity/fieldPresence。
- XHS author 必须同时提供非空 `authorId`、`platformAuthorId` 且二者完全相等；领域身份为 `(workspaceId,"xhs",platformAuthorId)`。`Author.authorEntityId` 固定为现役 `buildAuthorCode({platform,platformAuthorId})` 的确定性结果（`src/lib/data-foundation/content-identity.ts:81-87`），不得接受客户端自报值（DR-B3-001 A）。B3-CONTRACT-SRC-001 已将该双身份约束写入同一版本化合同、fixture 与 B2 adapter。
- XHS comment 必须同时提供非空 `noteId`、`commentId`；领域身份为 `(workspaceId,"xhs",noteId,commentId)`。目标唯一键为 `(workspaceId,platform,noteSourceId,sourceId)`；现役三列唯一键必须在可证明切换时退役，不能作为 V2 合并依据（DR-B3-001 A）。
- 任一身份缺失或成对身份不相等，必须在 ContractEvaluation accepted 与 CEC 推进前失败，CEC/Domain 均零推进；B3 仍在任何 Domain 行写入前做纵深复核。不查第二候选、不绑定“最接近”的既有实体、不走 V1 mapper。B2 adapter `2.0.0` 已移除候选优先级并执行该前置拒绝。
- Canonical 业务字段的唯一读取根是 `CanonicalObservation.payload.sourcePayload`。note 当前已证明 `title/content/url`，comment 已证明 `text`，author 已证明 `name/profileUrl`；`payload.title`、旧 raw 字段、旧 URL 与宽松别名均不是合法来源（`src/lib/evidence/derived/xhs-derived-contract.ts:405-432`；DR-B3-002 A）。
- note 类型的唯一来源字段是 `payload.sourcePayload.type`，值域严格为 `normal | video`，值原样进入 `ContentObservation.contentType`；V1 `contentType/noteType/itemType` 别名、`note/unknown` 默认与媒体推断全部拒绝。该规则已由 `xhs.record-payload/v2`、六 fixture、B2 `fieldPresence.type`、adapter `2.0.0` / `xhs.canonical/2` 与跨仓 hash 锁证明（DR-B3-002 A）。
- recordKind 分支固定为 note → Content，comment → Comment，author → Author；一个分支不得生成另一种 Domain Observation。

### ContentObservation（B3 新建，不可变）

下表是 DEC-B3-007/009 确认后的 Content-only 暗态物理合同。

| 字段 | 类型 | 可空 | 默认 | B3 值来源 | 物理来源状态 |
|---|---|---:|---|---|---|
| id | String | 否 | `cuid()` | 持久化层 ID | DEC-B3-007 确认的首切物理细节 |
| workspaceId | String | 否 | 无 | CanonicalObservation.workspaceId | Canonical 复合 FK 要求同类型；freeze:L407,L412 |
| contentAssetId | String | 否 | 无 | 严格 note 身份确保的 ContentAsset.id | DR-B3-001 A；ContentAsset.id 为 String |
| canonicalObservationId | String | 否 | 无 | accepted ContractEvaluationInput.canonicalObservationId | freeze:L407,L412 |
| rawSnapshotId | String | 否 | 无 | CanonicalObservation.rawSnapshotId | freeze:L407,L412 |
| rawRecordId | String | 否 | 无 | CanonicalObservation.rawRecordId | freeze:L407；同名上游字段 |
| observedAt | DateTime | 否 | 无 | CanonicalObservation.observedAt | freeze:L408；同名上游字段 |
| contentType | String | 是 | 无 | note `payload.sourcePayload.type`，严格 `normal | video` | freeze:L408 的 `?`；DEC-B3-002；B3-CONTRACT-SRC-001 |
| title | String | 是 | 无 | note `payload.sourcePayload.title` | freeze:L408 的 `?`；B3-A-01 已核实路径 |
| bodyText | String | 是 | 无 | note `payload.sourcePayload.content` | freeze:L408 的 `?`；B3-A-01 已核实路径 |
| publishedAt | DateTime | 是 | 无 | 首切固定为 null；不得从 processing time 推断 | DEC-B3-007 的 Content-only 收口 |
| authorId | String | 是 | 无 | 首切固定为 null；不得伪造 Author 关系 | DEC-B3-007 的 Content-only 收口 |
| originalUrl | String | 是 | 无 | 同一 CanonicalObservation `payload.sourcePayload.url` 的原始 HTTPS XHS/xhslink 值 | DEC-B3-009 |
| fieldPresence | Json | 是 | 无 | CanonicalObservation.fieldPresence | freeze:L409 的 `?`；同名上游字段 |
| qualityStatus | String | 否 | 无 | CanonicalObservation.qualityStatus | freeze:L409；同名上游字段 |
| adapterVersion | String | 否 | 无 | CanonicalObservation.normalizationRunId → NormalizationRun.adapterVersion | freeze:L409；B2 同源链 |
| createdAt | DateTime | 否 | `now()` | 持久化时刻 | DEC-B3-007 确认的首切物理细节 |

- 主键：`id`。唯一键：`(workspaceId,id)`、`(workspaceId,contentAssetId,id)`、`(workspaceId,contentAssetId,canonicalObservationId)`。
- FK：`(workspaceId,contentAssetId) → ContentAsset(workspaceId,id)`；`(workspaceId,rawSnapshotId,canonicalObservationId,rawRecordId) → CanonicalObservation(workspaceId,rawSnapshotId,id,rawRecordId)`；全部 `ON DELETE RESTRICT`。INSERT trigger 逐字段反校 Canonical/NormalizationRun/ContentAsset，UPDATE/DELETE 以 SQLSTATE 55000 拒绝。
- 写入者：B3 DomainProjector 使用 `default_app` INSERT+SELECT；无 UPDATE（freeze:L622）。读取者：Projection writer/受控审计；普通页面不直接读取 Observation。
- 删除规则：不可变且不 DELETE（freeze:L404,L622）。重放同一 accepted CanonicalObservation 不新增第二行。

### ContentCurrentProjection（B3 新建，可更新）

| 字段 | 类型 | 可空 | 默认 | B3 值来源 | 来源 |
|---|---|---:|---|---|---|
| contentAssetId | String | 否 | 无 | ContentObservation.contentAssetId | freeze:L419-L420 |
| workspaceId | String | 否 | 无 | ContentObservation.workspaceId | freeze:L421 |
| currentObservationId | String | 否 | 无 | ContentObservation.id | freeze:L422 |
| contractEvaluationId | String | 否 | 无 | 当前 accepted ContractEvaluation.id | DEC-B3-007/008 |
| title | String | 是 | 无 | 同一 ContentObservation.title | freeze:L423 |
| bodyText | String | 是 | 无 | 同一 ContentObservation.bodyText | freeze:L424 |
| contentType | String | 是 | 无 | 同一 ContentObservation.contentType | freeze:L425 |
| publishedAt | DateTime | 是 | 无 | 同一 ContentObservation.publishedAt | freeze:L426 |
| authorId | String | 是 | 无 | 同一 ContentObservation.authorId | freeze:L427 |
| originalUrl | String | 是 | 无 | 同一 ContentObservation.originalUrl | DEC-B3-009 |
| lastObservedAt | DateTime | 否 | 无 | 同一 ContentObservation.observedAt | freeze:L428 |
| projectionVersion | Int | 否 | `1` | 成功推进时单调递增 | freeze:L429；B3-A-01 REQUIRED-B3-001 |
| lifecycleState | String | 是 | 无 | SOURCE_INCOMPLETE；不得承担 quarantine | freeze:L430 仅给候选值；D3-5 指定 visibilityState 为唯一机制 |
| visibilityState | String | 否 | `visible` | accepted B3 写 visible；rejected B2 写 quarantined | freeze:L431,L584-L593 |
| updatedAt | DateTime | 否 | `now(), @updatedAt` | 数据库更新时间 | freeze:L432 |

- 主键：`contentAssetId`。唯一键：`(workspaceId,contentAssetId)`、`(workspaceId,contentAssetId,currentObservationId)`。
- FK：`(workspaceId,contentAssetId) → ContentAsset`、`(workspaceId,contentAssetId,currentObservationId) → ContentObservation`、`(workspaceId,contractEvaluationId) → ContractEvaluation`；全部 `ON DELETE RESTRICT`。
- 写入者：B3 只能调用数据库受控 advance 函数；B2 在推进 CEC 的同一事务调用 revoke 函数。普通 INSERT/UPDATE、直接指针或 materialized 字段修改均由 trigger 以 SQLSTATE 55000 拒绝，DELETE 永远为 55000；deferred constraint trigger 在 COMMIT 阶段拒绝遗漏撤销的 CEC 变更。内部 GUC 只标记受控函数的转换阶段，不是授权凭证：即使伪造 GUC 或直接调用 revoke，若不存在合法 CEC supersede 数据库事实，或 quarantine 后仍有 active V2 Usage，事务必须回滚。仓库尚无已确认物理调用者角色/GRANT，故本阶段不宣称身份权限已落地，BLK-015 保持开放。
- 状态转换：首次 accepted 创建 visible/version=1；exact replay 完全不变；同一 Observation 被新 Evaluation 再接受时只更新 Evaluation/恢复可见且版本不增；不同新 Observation 必须 observedAt 严格递增并 version+1。accepted A→accepted B/rejected 先原子 quarantine A 并关闭 A 的 active V2 Usage，B 完整提交后才恢复 visible。较旧或等时不同 Observation 的 accepted CEC 可保留为其 snapshot 事实，但不得 quarantine 当前较新 Projection、关闭 active Usage 或推进 Current；quarantine 本身不增加版本。
- 媒体完整性：任一 visible Current 的 current CanonicalObservation 若有 `observed` CanonicalMediaSlot，则 COMMIT 时每个 slot 必须恰有一条 active V2 ContentMediaUsage，且 workspace/asset/evaluation/observation/slot 全部匹配；不得缺失、额外或重复。该 deferred 不变量同时覆盖 Current advance 和 Usage INSERT/UPDATE/DELETE；exact replay 返回 success 前也显式复核。没有 observed slot 时仍是合法的 media unknown，不合成 absent/unavailable。

### ContentProjectionSubjectFence（B3 内部协调表）

该模型不是业务事实、Canonical 资产或页面读模型，只承载 `(workspaceId,platformContentId)` 的数据库事务排序。字段固定为复合主键、单调 `generation` 与 `updatedAt`；同一 `lock_xhs_content_subject` 在 advisory lock 后执行原子 UPSERT/increment。页面、Projection reader 和 Agent 均不得读取它形成业务结论；它只用于让陈旧 SERIALIZABLE snapshot 可证伪地失败并整事务重试。

### AuthorObservation（由 AuthorSnapshot 演进，B3 不可变）

以下“保留”字段继承当前 AuthorSnapshot 的物理类型/default；冻结规范只明确 `workspaceId` 改为 NOT NULL、新增七类来源字段、移回三个用户字段并删除 `rawData`（freeze:L439-L441）。新增字段未给完整类型/可空性时明确标记缺口。

| 字段 | 类型 | 可空 | 默认 | B3 值来源 | 来源状态 |
|---|---|---:|---|---|---|
| id | String | 否 | `cuid()` | 现役 AuthorSnapshot ID | schema:L3453；演进保留 |
| workspaceId | String | 否 | 无 | CanonicalObservation.workspaceId | freeze:L441 明确 NOT NULL |
| authorId | String | 否 | 无 | 严格 author 身份确保的 Author.id | schema:L3455；DR-B3-001 A |
| authorEntityId | String | 否 | 无 | `buildAuthorCode({platform,platformAuthorId})` | schema:L3456；DR-B3-001 A |
| platformAuthorId | String | 否 | 无 | 相等校验后的 sourcePayload.platformAuthorId | schema:L3457；DR-B3-001 A |
| platform | String | 否 | 无 | 固定 `xhs` | schema:L3458；首期边界 |
| name | String | 否 | 无 | author `payload.sourcePayload.name` | schema:L3459；fixture 已证明字段，但 B2 partial 可缺失，缺失时是否拒绝仍 SOURCE_INCOMPLETE |
| description | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L3460；fixture/B2 fieldPresence 未登记 |
| fans | Int | 否 | `0` | SOURCE_INCOMPLETE；默认不能冒充已观察 0 | schema:L3461；V2 缺失语义未定义 |
| follows | Int | 否 | `0` | SOURCE_INCOMPLETE；默认不能冒充已观察 0 | schema:L3462；V2 缺失语义未定义 |
| interactions | Int | 否 | `0` | SOURCE_INCOMPLETE；默认不能冒充已观察 0 | schema:L3463；V2 缺失语义未定义 |
| ipLocation | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L3464；fixture/B2 fieldPresence 未登记 |
| profileUrl | String | 是 | 无 | author `payload.sourcePayload.profileUrl` | schema:L3465；当前 fixture 已证明 |
| handle | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L3466；fixture/B2 fieldPresence 未登记 |
| keywords | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L3467；fixture/B2 fieldPresence 未登记 |
| collectedAt | DateTime | 是 | 无 | SOURCE_INCOMPLETE | schema:L3472；不得用 B3 处理时刻代替采集时刻 |
| createdAt | DateTime | 否 | `now()` | 数据库创建时刻 | schema:L3473；演进保留 |
| canonicalObservationId | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | accepted CanonicalObservation.id | freeze:L441 只给字段名 |
| rawSnapshotId | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | CanonicalObservation.rawSnapshotId | freeze:L441 只给字段名 |
| rawRecordId | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | CanonicalObservation.rawRecordId | freeze:L441 只给字段名 |
| observedAt | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | CanonicalObservation.observedAt | freeze:L441 只给字段名 |
| adapterVersion | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | NormalizationRun.adapterVersion | freeze:L441 只给字段名 |
| fieldPresence | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | CanonicalObservation.fieldPresence | freeze:L441 只给字段名 |
| qualityStatus | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | SOURCE_INCOMPLETE | CanonicalObservation.qualityStatus | freeze:L441 只给字段名 |
| author | Author | 否 | 不适用 | `authorId` 关系对象 | schema:L3475；目标同 workspace relation 待定义 |

- 删除字段：`userGroup/userNote/userTags` 移回 Author；`rawData` 删除。历史关系无法 100% 证明时退役/人工审核，不把旧 rawData 伪造成 Canonical 来源。
- `workspaceId` 的目标 NOT NULL 不授权给历史 null 行伪造 workspace；历史行须经可证明归属迁移，无法证明者退役/人工审核，完成前不得直接执行 NOT NULL。
- 主键/唯一键/索引：当前 `id` 主键与索引可作为迁移现状，但目标 AuthorObservation 的同业务实体/同 CanonicalObservation 唯一键未在 freeze 给出，保持 `SOURCE_INCOMPLETE`（BLK-006）。
- FK/onDelete：当前 `authorId → Author.id ON DELETE CASCADE` 不带 workspace；目标同 workspace Author/Canonical/Raw 同源 FK 及 onDelete 未定义，保持 `SOURCE_INCOMPLETE`，不得原样沿用单列 FK 作为 V2 证明。
- 写入者：B3 DomainProjector 使用 `default_app` INSERT+SELECT，无 UPDATE（freeze:L624）；删除规则：不可变。Release-C rename 前不得让 V1 AuthorSnapshot writer 承接 V2 新数据。

### AuthorCurrentProjection（B3 新建，可更新）

| 字段 | 类型 | 可空 | 默认 | B3 值来源 | 来源 |
|---|---|---:|---|---|---|
| authorId | String | 否 | 无 | AuthorObservation.authorId | freeze:L446-L447 |
| workspaceId | String | 否 | 无 | AuthorObservation.workspaceId | freeze:L448 |
| currentObservationId | String | 否 | 无 | AuthorObservation.id | freeze:L449 |
| name | String | 是 | 无 | 同一 AuthorObservation.name | freeze:L450 |
| fans | Int | 是 | 无 | 同一 AuthorObservation.fans；仅已观察时 | freeze:L451 |
| follows | Int | 是 | 无 | 同一 AuthorObservation.follows；仅已观察时 | freeze:L452 |
| interactions | Int | 是 | 无 | 同一 AuthorObservation.interactions；仅已观察时 | freeze:L453 |
| ipLocation | String | 是 | 无 | 同一 AuthorObservation.ipLocation | freeze:L454 |
| profileUrl | String | 是 | 无 | 同一 AuthorObservation.profileUrl | freeze:L455 |
| handle | String | 是 | 无 | 同一 AuthorObservation.handle | freeze:L456 |
| lastObservedAt | DateTime | 否 | 无 | 同一 AuthorObservation.observedAt | freeze:L457 |
| projectionVersion | Int | 否 | `1` | 成功推进时单调递增 | freeze:L458；B3-A-01 REQUIRED-B3-001 |
| visibilityState | String | 否 | `visible` | accepted B3 写 visible；rejected B2 写 quarantined | freeze:L459,L584-L593 |
| updatedAt | DateTime | 否 | `now(), @updatedAt` | 数据库更新时间 | freeze:L460 |

- 主键：`authorId`（freeze:L447）；唯一键/索引：除主键外无已确认来源。
- FK：必须证明 `(workspaceId,authorId)` 属于同一 Author，且 `currentObservationId` 属于同一 workspace、同一 author；可直接转录的复合 target key/onDelete 尚无来源，保持 `SOURCE_INCOMPLETE`（BLK-007）。
- 写入者、删除与状态转换与 ContentCurrentProjection 同构：B3 `default_app` 正常推进，B2 `canonical_writer` 只做 quarantine；相同 Observation replay 不推进版本；default_app 不能直接解除 quarantine。rejected 是否增加 projectionVersion 与 onDelete 保持 `SOURCE_INCOMPLETE`。

### Comment（现有模型的 V2 演进）

冻结规范要求 `workspaceId` NOT NULL、新增 `canonicalObservationId String?`、其余列保留（freeze:L464-L466）。下表记录目标列和 V2 writer 语义；“现役可空”表示历史物理现实，不授权 V2 新行缺失来源。

| 字段 | 类型 | 目标可空 | 默认 | V2 值来源/规则 | 来源 |
|---|---|---:|---|---|---|
| id | String | 否 | `cuid()` | 数据库 ID | schema:L741；保留 |
| workspaceId | String | 否 | 无 | CanonicalObservation.workspaceId | freeze:L466 |
| canonicalObservationId | String | 是（V2 新行必须非空） | 无 | accepted comment CanonicalObservation.id | freeze:L466；DEC-B1-003 |
| ingestionId | String | 是 | 无 | V2 必须 null，不伪造 CommentIngestion | schema:L743；非执行关系边界 |
| platform | String | 否 | 无 | 固定 `xhs` | schema:L744；首期边界 |
| sourceId | String | 否 | 无 | `payload.sourcePayload.commentId` | schema:L745；DR-B3-001 A |
| noteSourceId | String | 现役是；V2 新行必须非空 | 无 | `payload.sourcePayload.noteId` | schema:L746；DR-B3-001 A |
| contentAssetId | String | 是 | 无 | SOURCE_INCOMPLETE；comment-probe 未提供可创建 ContentAsset 的已登记类型 | schema:L747；不得伪造父 Content |
| contentCode | String | 是 | 无 | 有可证明 ContentAsset 时复制其 contentCode，否则 null | schema:L748；现役关系字段 |
| sourceUrl | String | 是 | 无 | V2 必须 null；原文能力由受控 Projection action 提供 | schema:L749；DEC-B1-010 |
| authorId | String | 是 | 无 | SOURCE_INCOMPLETE；comment fixture 未证明评论者 Author 关系 | schema:L750 |
| authorName | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L751 |
| replyToCommentId | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L752 |
| replyToUserName | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L753 |
| level | Int | 否 | `1` | SOURCE_INCOMPLETE；默认不能冒充已观察层级 | schema:L754 |
| likes | Int | 否 | `0` | SOURCE_INCOMPLETE；默认不能冒充已观察点赞 | schema:L755 |
| content | String | 否 | 无 | `payload.sourcePayload.text` | schema:L756；路径已证明，但 B2 partial 可缺失，缺失时是否拒绝仍 SOURCE_INCOMPLETE |
| status | String | 否 | `ready` | SOURCE_INCOMPLETE；V2 状态值域/转换未定义 | schema:L757 |
| rawPayload | String | 是 | 无 | V2 必须 null；Raw 只留在 Evidence | schema:L758；DEC-E08 |
| normalizedText | String | 是 | 无 | null；后续受控处理可写 | schema:L760；保留 |
| redactedText | String | 是 | 无 | null；后续受控处理可写 | schema:L761；保留 |
| textSafetyVersion | String | 是 | 无 | null；后续受控处理可写 | schema:L762；保留 |
| piiStatus | String | 是 | 无 | null；后续受控处理可写 | schema:L763；保留 |
| safetyStatus | String | 是 | 无 | null；后续受控处理可写 | schema:L764；保留 |
| riskFlags | Json | 是 | 无 | null；后续受控处理可写 | schema:L765；保留 |
| writingEligibility | String | 是 | 无 | null；后续受控处理可写 | schema:L766；保留 |
| rootCommentId | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L769 |
| commentEntityId | String | 是 | 无 | SOURCE_INCOMPLETE；不得替代组合身份 | schema:L770 |
| profileUrl | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L773 |
| publishedAt | DateTime | 是 | 无 | SOURCE_INCOMPLETE | schema:L776 |
| collectedAt | DateTime | 是 | 无 | CanonicalObservation.observedAt 不是同义字段；保持 SOURCE_INCOMPLETE | schema:L777 |
| ipLocation | String | 是 | 无 | SOURCE_INCOMPLETE | schema:L780 |
| isStarred | Boolean | 否 | `false` | 用户态默认，非观察事实 | schema:L782；保留 |
| createdAt | DateTime | 否 | `now()` | 数据库创建时刻 | schema:L784；保留 |
| updatedAt | DateTime | 否 | `@updatedAt` | 数据库更新时间 | schema:L785；保留 |
| deletedAt | DateTime | 是 | 无 | 初始 null；非 projector 观察字段 | schema:L786；保留 |
| lifecycleStatus | String | 否 | `active` | SOURCE_INCOMPLETE；V2 状态转换未定义 | schema:L787 |
| archivedAt | DateTime | 是 | 无 | 初始 null | schema:L788；保留 |
| archiveReason | String | 是 | 无 | 初始 null | schema:L789；保留 |
| ingestion | CommentIngestion | 是 | 不适用 | V2 为 null；保留 V1 反向关系 | schema:L790；onDelete SetNull |
| cleaningReview | CommentCleaningReview | 是 | 不适用 | 保留现役关系 | schema:L791 |
| signals | CommentSignal[] | 否 | 不适用 | 保留现役关系 | schema:L792 |
| signalExtractionReceipts | CommentSignalExtractionReceipt[] | 否 | 不适用 | 保留现役关系 | schema:L793 |
| star | CommentStar | 是 | 不适用 | 保留现役关系 | schema:L794 |
| contentAsset | ContentAsset | 是 | 不适用 | 仅 contentAssetId 有证明时关联 | schema:L795；现役 onDelete SetNull |
| voiceAssets | VoiceAsset[] | 否 | 不适用 | 保留现役关系 | schema:L796 |
| voiceSnippets | VoiceSnippet[] | 否 | 不适用 | 保留现役关系 | schema:L797 |
| safetyProjectionBatchItems | CorpusSafetyProjectionBatchItem[] | 否 | 不适用 | 保留现役关系 | schema:L798 |

- 主键：`id`。V2 目标唯一键：`(workspaceId,platform,noteSourceId,sourceId)`（DR-B3-001 A）；现役 `(workspaceId,platform,sourceId)` 在可证明切换前仍会阻止跨 note 同 commentId，必须作为迁移 blocker 处理，不能双键承接新数据。
- FK：目标至少需要 `(workspaceId,canonicalObservationId) → CanonicalObservation(workspaceId,id)`；onDelete 未有来源。`contentAssetId` 的父内容创建/同 workspace FK 受类型来源缺口阻塞，不能从 noteId 空壳创建；其他现役关系与删除行为保留给 V1，但不构成 V2 同源证明。
- 历史 `workspaceId/noteSourceId/canonicalObservationId` 的 null 不得伪造回填。V2 新 writer 必须满足上表非空条件；物理 NOT NULL/旧唯一键退役要等历史行可证明处置和硬切顺序另行确认。
- 写入者：逻辑唯一 writer 为 B3 DomainProjector；冻结权限矩阵没有给 Comment 的物理角色权限，保持 `SOURCE_INCOMPLETE`（BLK-008）。V1 writer 不得承接 V2 新数据。
- 实施门禁：`content/name` 的必填缺失行为，以及 `level/likes/status/lifecycleStatus` 的非空默认与严格缺失语义必须先收口；不得把数据库默认解释为采集事实。身份相同 replay 不新增 Comment；不同 noteId 的同 commentId 必须可共存。

#### Comment 跨观察演进缺口（DR-B3-004）

DEC-B3-004 已确认方案 A：`Comment` 是稳定业务身份实体，唯一身份为 `(workspaceId,platform,noteSourceId,sourceId)`；它不是某一次采集结果。`CommentObservation` 是不可变事实层历史，每次新的采集、内容变化或状态变化均形成新的候选观察；页面/UI 不直接读取 Observation，只读取 `Comment.currentObservationId` 指向的当前有效版本。

- 同 identity + 同内容 hash：replay，不新增 CommentObservation，不推进 current；
- 同 identity + 内容或状态变化：新增不可变 CommentObservation；
- 新 Observation 未满足 accepted 条件：保留审计事实，但不得推进 `currentObservationId`；
- accepted Observation：在受控事务中单调推进 `currentObservationId`；
- 任何历史 CommentObservation 均禁止 UPDATE/DELETE，稳定 Comment 也不得以覆盖内容字段伪装历史推进。

完整 CommentObservation 字段、内容 hash 的精确输入、复合 FK、Current CAS/排序规则、writer 权限和 V1 唯一键退役顺序仍受 BLK-008 阻塞；确认方案 A 不授权猜测这些物理细节，因此本轮只登记领域合同，不进入 Comment schema/writer。

### 首期 Metric 明确排除

- 六份 XHS 合同的 `recordKinds` 均无 metric；`media_inventory` 是 CaptureArtifact，不是 metric RawRecord。B3 首期不得创建 `ContentMetricSnapshot`、不得推进 `ContentMetricLatest`，也不得读取 V1 metric 作为 fallback。
- BLK-009 保持 OPEN；现有 ContentMetricSnapshot/Latest 的字段、nullable raw 关系与 V1 writers 均不因首期排除而获得 V2 来源证明。

### B2 CEC owner、B3 accepted 消费与 rejected 撤销

1. B2 `canonical_writer` 仍是 ContractEvaluationCurrent 唯一推进者；B2 accepted 事务不创建任何 Domain Observation/CurrentProjection（freeze:L567-L579；B3-A-01 REQUIRED-B3-001）。
2. B3 在自己的事务中读取 current accepted CEC，严格按 ContractEvaluationInput 投影 Observation + CurrentProjection；提交前重验同一 CEC。B3 失败不回滚已提交的 accepted CEC。Content 可依冻结唯一键与 Current CAS 收敛；Author/Comment 的目标唯一键尚有上文所列来源/迁移缺口，缺口关闭前不得声称 replay 已有数据库保证。
3. accepted→rejected 时，B2 在推进 CEC 的同一 canonical_writer 事务中 quarantine 已存在的 Content/Author CurrentProjection，并令对应 PresentationRequirement 失效；媒体来源合同完成后还必须同事务关闭相应业务媒体关系，但不删除共享 Media Asset（freeze:L584-L593；DEC-B1-006）。
4. `default_app` 不得把 quarantined 直接改回 visible；只有之后新的 current accepted 经 B3 正常推进可以恢复。数据库强制机制与 canonical_writer 删除 requirement 的权限仍受 BLK-012/013/014 阻塞，不能用异步补偿或 fallback 替代。

### B3 不创建 ProjectionReceipt

用户已确认 DR-B3-003 方案 A：冻结规范 B3 流程中唯一一次出现、但没有模型合同的 `ProjectionReceipt` 名称删除；B3 以不可变 Observation 唯一键、CurrentProjection 指针/单调版本和事务结果证明幂等完成，不补造新回执表。§5.14 的 `PresentationReceipt` 仍只属于 B6 强一致页面，二者不得混用。

### PresentationRequirement

来源：[冻结规范 §5.14](../content-workbench-v2-design-freeze.md#L480-L496)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L485-L486 |
| workspaceId | String | 否 | — | freeze:L487 |
| subjectType | String | 否 | — | freeze:L488 |
| subjectId | String | 否 | — | freeze:L489 |
| routeId | String | 否 | — | freeze:L490 |
| policyVersion | String | 否 | — | freeze:L491 |
| projectionRevision | String | 否 | — | freeze:L492 |
| requiredAt | DateTime | 否 | `now()` | freeze:L493 |

- 主键：`id`；唯一键：`(workspaceId,subjectType,subjectId,routeId,policyVersion,projectionRevision)`（freeze:L494）。
- 外键：来源未定义；写入/读取者：`default_app` INSERT+SELECT+DELETE，D3-5 的撤销写入者与权限表冲突，见 [BLK-013](04-blocker-ledger.md#L146-L156)。
- 删除规则：D3-5 需要在 rejected 时由 B2 `canonical_writer` 推进 CEC 的同一事务删除旧 requirement（freeze:L584-L590）；事务 owner 已收敛，物理 DELETE 权限与精确关联条件仍受 BLK-013 阻塞。
- 来源决策：DEC-E01；Topic/Task revision 仍受 BLK-011 约束。

### PresentationReceipt

来源：[冻结规范 §5.14](../content-workbench-v2-design-freeze.md#L498-L510)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L499-L500 |
| workspaceId | String | 否 | — | freeze:L501 |
| routeId | String | 否 | — | freeze:L502 |
| subjectType | String | 否 | — | freeze:L503 |
| subjectId | String | 否 | — | freeze:L504 |
| policyVersion | String | 否 | — | freeze:L505 |
| projectionRevision | String | 否 | — | freeze:L506 |
| resolverVersion | String | 否 | — | freeze:L507 |
| createdAt | DateTime | 否 | `now()` | freeze:L508 |

- 主键：`id`；索引：`(workspaceId,subjectType,subjectId,routeId,policyVersion,projectionRevision)`（freeze:L509）。
- 外键、写入/读取者、删除规则：冻结规范仅定义本表字段/索引；B6 写入行为由 `default_app` Presentation 层描述（freeze:L593-L594），可实施权限与撤销语义受 BLK-011、BLK-012、BLK-013 约束。
- 来源决策：DEC-E01。

### EvidenceAccessAudit

来源：[冻结规范 §5.15](../content-workbench-v2-design-freeze.md#L513-L528)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `gen_random_uuid()::text` | freeze:L516-L517 |
| workspaceId | String | 否 | — | freeze:L518 |
| capturePackageId | String | 否 | — | freeze:L519 |
| accessedBy | String | 否 | — | freeze:L520 |
| accessReason | String | 否 | — | freeze:L521 |
| requestTraceId | String | 是 | — | freeze:L522 |
| accessedAt | DateTime | 否 | `now()` | freeze:L523 |
| restricted | Boolean | 否 | `false` | freeze:L524 |

- 主键：`id`；索引：`(workspaceId,capturePackageId)`（freeze:L525）。`capturePackageId` 的同 workspace FK 未由冻结规范给出，必须保持 PENDING，见 [BLK-015](04-blocker-ledger.md#L209-L221) / DEC-B1-007；不得从字段名称推测外键。
- 写入者：受控读取函数；读取者：`canonical_writer` 审计核验（freeze:L617）；删除规则：append-only、全角色不可 UPDATE/DELETE（freeze:L526、L628）。审计函数与底层访问权限的完整边界同样受 BLK-015 / DEC-B1-007 约束。
- 来源决策：DEC-E02、DEC-E05；权限/关系缺口：DEC-B1-007。

### EvidenceReaderWorkspaceGrant

来源：[冻结规范 §5.16](../content-workbench-v2-design-freeze.md#L530-L542)；当前实现：未在 `schema.prisma` 定义。

| 字段 | 类型 | 可空 | 默认 | 来源 |
|---|---|---:|---|---|
| id | String | 否 | `cuid()` | freeze:L533-L534 |
| workspaceId | String | 否 | — | freeze:L535 |
| readerRole | String | 否 | — | freeze:L536 |
| grantedAt | DateTime | 否 | `now()` | freeze:L537 |
| grantedBy | String | 否 | — | freeze:L538 |
| revokedAt | DateTime | 是 | — | freeze:L539 |

- 主键：`id`；唯一键：`(workspaceId,readerRole)`（freeze:L540）；外键（本模型持有）：无；冻结规范的完整模型没有列出出站 FK。角色授权由 `readerRole` 表达而不是外键，原始 Evidence 读取的权限证明仍受 BLK-015 / DEC-B1-007 约束。
- 写入者：来源未定义；读取者：reader 只能检查自身授权（freeze:L618）；删除规则：来源未定义。
- 来源决策：DEC-E05；完整权限边界受 BLK-015 约束。

## 来源不完整、不得补写的模型契约

| 模型 | 已有来源 | 缺少的不可猜测内容 | 对应 blocker |
|---|---|---|---|
| RawSnapshot | 当前 `schema.prisma:L4156-L4195` + freeze:L163-L207 | 已确认生命周期可用性、冲突包保留、观察级质量边界；仍缺逐字段、键、权限、删除规则的可证明来源 | BLK-001 |
| RawRecord | 当前 `schema.prisma:L4198-L4220` + freeze:L209-L222 + DEC-E09 | 已确认观察级追加；仍缺从属快照的完整键、权限和删除规则来源，不能在父模型缺失时 proof | BLK-001、BLK-002 |
| ContentObservation | freeze:L404-L413 + B3-A-02/B3-CONTRACT-SRC-001 已确认身份/type 路径 | `id/createdAt` 类型/default/PK、ContentAsset/Author 同 workspace FK、onDelete、publishedAt/author 来源 | BLK-004、BLK-005 |
| AuthorObservation | 当前 `schema.prisma:L3451-L3480` + freeze:L439-L441 + B3-A-02 已确认身份/路径 | 七个新增字段的类型/可空/default、目标唯一键、同 workspace Author/Canonical/Raw FK、onDelete、partial 对必填 name/计数的处理 | BLK-006、BLK-007 |
| Comment | 当前 `schema.prisma:L739-L808` + freeze:L464-L466 + B3-A-02 组合身份 | 现役唯一键退役顺序、Canonical/onDelete、comment-probe 父 Content 关系、必填默认的缺失语义、物理 writer 权限 | BLK-008 |
| ContentMetricSnapshot | 当前 `schema.prisma:L2671-L2736` + freeze:L468-L470 | 首期 XHS 明确排除；未来 V2 metric 仍缺 RawSnapshot/RawRecord 必需性与完整同源 FK | BLK-009 |
| ContentMetricLatest | 当前 `schema.prisma:L4225-L4245` + freeze:L472-L474 | 首期 XHS 明确排除；未来仍缺指向 Metric observation 的最终关系和历史处置 | BLK-009 |
| TaskStatusProjection | 当前 `schema.prisma:L4341-L4371` + freeze:L476-L478 | 四个新增状态的值域、终态语义、写入者和迁移顺序 | BLK-010 |
