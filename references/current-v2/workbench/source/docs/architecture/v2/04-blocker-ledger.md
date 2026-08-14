# 04 — Blocker 台账

> 本文件是 V2 唯一问题状态机。每条 blocker 含编号、已核实事实、证据行号、影响、状态、待澄清问题和对应验证规则。
> 状态只能是：`OPEN`、`DECISION_REQUIRED`、`READY_FOR_PROOF`、`PROVED`、`CLOSED`。执行者只能将具备实际验证证据的条目推进至 `PROVED`；仅审核方可以推进至 `CLOSED`。不得用文档勾选代替验证。

所有 BLK-001 至 BLK-016 都是 **Release-B 最终收口 blocker**；但它们按 Phase 0～5 分阶段生效，而不是互相阻塞 B1-E 的开始。对应的正例、反例、命令与证据状态见 [03-validation-matrix.md](03-validation-matrix.md)。

本轮对 BLK-001 至 BLK-016 完成代码、schema、冻结规范和正式测试库只读水位交叉核实。2026-08-10 的 DEC-B2-001 已修正 B2 四条同源复合 FK，B2-B-01 候选 schema/migration 已在独立隔离库完成正反例并通过 Spec/Standards 双轴复核，因此 BLK-003、BLK-016 已由审核方推进至 `CLOSED`；其余 blocker 仍按各自来源与验证状态管理，不得把“决策已确认”误写成“已实施”。

## 本轮全量收口摘要

> 当前事实、来源完整性和缺口均以本轮只读审计为准。完整的查询水位、源码链路和逐项解释见 [代码实施前准备审计](../../code-review/v2-precode-readiness-audit-2026-08-05.md)；本表不把“冻结规范写过一段散文”误记为可执行模型来源。

| Blocker | 当前事实 | 代码 / Schema / 文档证据 | 是否已有足够来源完成模型契约 | 缺少什么 | 最终状态 |
|---|---|---|---|---|---|
| BLK-001 | 现有快照带执行必填关系与可更新 payload；冻结规范只写增量列。 | `raw-snapshot-ingest-service.ts:L240-L485`；`execution-task-observation-service.ts:L587-L649`；schema:L4156-L4195；freeze:L163-L207。 | 否 | 已确认生命周期、冲突、观察质量语义；仍缺逐字段、键、权限和删除规则来源。 | OPEN（Phase 1） |
| BLK-002 | 现有片段以 workspace 级幂等键去重；导入路径曾更新/移动片段。 | `execution-task-observation-service.ts:L470-L660`；schema:L4198-L4220；freeze:L209-L222；DEC-E09。 | 否 | 已确认追加语义；仍缺从属快照的完整键、权限和删除规则来源。 | OPEN（Phase 1） |
| BLK-003 | 三列同源 FK 已由 DEC-B2-001 修正并由 PostgreSQL 隔离库证明，双轴复核通过。 | freeze:L221、L265；DEC-B2-001；VAL-BLK-003。 | 是 | 无。 | CLOSED（Phase 1） |
| BLK-004 | ContentObservation 只有字段速记，无完整身份、关系或写入者。 | freeze:L404-L413；DEC-E01。 | 否 | 已确认观察版本唯一性；仍缺完整身份、关系和唯一写入者来源。 | OPEN（Phase 2） |
| BLK-005 | 当前内容投影的 `currentObservationId` 未有同 workspace FK。 | freeze:L419-L436。 | 否 | BLK-004 的可转录同源领域观察合同与 FK 来源。 | OPEN（Phase 2） |
| BLK-006 | AuthorObservation 只留一条散文说明。 | freeze:L439-L441；schema:L3414-L3480。 | 否 | 已确认观察版本唯一性；仍缺完整身份、关系和唯一写入者来源。 | OPEN（Phase 2） |
| BLK-007 | 当前作者投影的 `currentObservationId` 未有同 workspace FK。 | freeze:L445-L461。 | 否 | BLK-006 的可转录同源领域观察合同与 FK 来源。 | OPEN（Phase 2） |
| BLK-008 | Comment 的 Canonical 来源为可空且未有 FK。 | schema:L739-L808；freeze:L464-L466；DEC-B3-004。 | 否 | 稳定 Comment、不可变 CommentObservation 与 accepted Current 语义已确认；仍缺完整物理字段、复合 FK、权限与迁移顺序来源。 | OPEN（Phase 2） |
| BLK-009 | 指标 raw 关联是可空字符串、没有 FK；历史已有缺失来源行。 | schema:L2671-L2736；freeze:L468-L474；只读水位见审计报告。 | 否 | 已确认 V2 必须溯源；仍缺物理关系、可空性和同源 FK 来源。 | OPEN（Phase 2） |
| BLK-010 | 当前任务投影只有 execution/writeback/analysis 三轴；冻结规范只列四个新列。 | schema:L4341-L4371；freeze:L476-L478。 | 否 | 已确认状态职责；仍缺值域、终态、唯一写入者和迁移来源。 | OPEN（Phase 1） |
| BLK-011 | Topic/Task 门禁使用 `static` revision；Topic workspace 在 schema 中可空。 | freeze:L953-L964；schema:L358-L460；`topic-service.ts:L953-L1041`。 | 否 | 仅对明确登记的强一致页面，需要其 revision/workspace 来源；普通页面不适用回执。 | OPEN（Phase 3） |
| BLK-012 | rejected 撤销未覆盖媒体关系或历史 receipt。 | freeze:L582-L591；媒体读链见 [Evidence 边界审查](../../progress/v2-evidence-boundary-review-2026-08-04.md)。 | 否 | 已确认关系撤销、不删资产；仍缺可转录关系与对象 Projection 撤销合同。 | OPEN（Phase 2） |
| BLK-013 | rejected 流程要求 canonical_writer 删除 requirement，但权限表只授权 default_app。 | freeze:L584-L589、L624。 | 否 | 已确认撤销目标；仍缺同一事务的唯一写入者及权限来源。 | OPEN（Phase 2） |
| BLK-014 | default_app 投影更新可能覆盖 canonical_writer 的 quarantined 状态。 | freeze:L586、L621。 | 否 | 已确认不可绕过撤销；仍缺状态转换的数据库强制来源。 | OPEN（Phase 2） |
| BLK-015 | 冻结权限表仍允许 default_app 正常读写原始证据；EvidenceAccessAudit 的 CapturePackage 同 workspace 关系也未定义。 | freeze:L516-L525、L604-L608；DEC-E01、DEC-E02、DEC-E04、DEC-E05、DEC-E08。 | 否 | 已确认数据库强制；仍缺角色、受控入口、读取审计同源关系的完整来源。 | OPEN（Phase 1） |
| BLK-016 | 四条关系的完整三列同源合同已由 DEC-B2-001 确认并由 PostgreSQL 隔离库证明，双轴复核通过。 | freeze:L224-L294；DEC-B2-001；VAL-BLK-016。 | 是 | 无。 | CLOSED（Phase 1） |

---

## Blocker 列表

### BLK-001：RawSnapshot 无完整目标 model 块

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md §5.4 :165-169 以散文列举 V2 新增列，无完整 `model RawSnapshot { ... }` 文本块。`@@unique([workspaceId, id])` 只在 prose :169 提到"Release-A expand 新增"，无 ALTER DDL。 |
| 证据 | design-freeze.md :165-169 |
| 影响 | 无法无猜测生成 RawSnapshot 的 V2 目标 DDL |
| 状态 | OPEN（Phase 1） |
| 已确认决策 | DEC-B1-001、DEC-B1-008、DEC-B1-009 |
| 待完成证明 | 决策已确认；仍须完整来源化生命周期/处置、质量、冲突提交、键、权限与删除合同。不得猜测字段或另建第二 Registry。 |
| 验证规则 | VAL-BLK-001 |
| 来源审计 | [V2 BLK-001～003 原始证据模型来源审计](../../progress/v2-blk-001-003-raw-evidence-source-audit-2026-08-04.md)；[V2 Evidence 边界审查](../../progress/v2-evidence-boundary-review-2026-08-04.md)；[Evidence 决策准备包](../../progress/v2-evidence-decision-preparation-2026-08-05.md)。用户已确认 DEC-E08、DEC-E09；本轮只读水位进一步将剩余缺口收敛为 DC-001（生命周期）、DC-008（冲突包）和 DC-009（质量粒度），当前状态见本表。 |

### BLK-002：RawRecord 无完整目标 model 块

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md §5.5 :211 以散文描述 recordType→recordKind 和删除列，无完整 `model RawRecord { ... }` 块。recordType→recordKind 的 rename 无 ALTER TABLE RENAME COLUMN DDL。 |
| 证据 | design-freeze.md :211 |
| 影响 | 无法无猜测生成 RawRecord 的 V2 目标 DDL |
| 状态 | OPEN（Phase 1） |
| 已确认决策 | DEC-E09、DEC-B1-001、DEC-B1-008、DEC-B1-009 |
| 待完成证明 | 观察级追加、生命周期、冲突包和质量边界已确认；仍须来源化完整模型、权限与隔离反例。 |
| 验证规则 | VAL-BLK-002 |
| 来源审计 | [V2 BLK-001～003 原始证据模型来源审计](../../progress/v2-blk-001-003-raw-evidence-source-audit-2026-08-04.md)；[V2 Evidence 边界审查](../../progress/v2-evidence-boundary-review-2026-08-04.md)；[Evidence 决策准备包](../../progress/v2-evidence-decision-preparation-2026-08-05.md)。用户已确认 DEC-E09 的包级幂等与片段追加边界；本轮只读水位确认当前片段没有有效 payload hash，不能用历史数据伪造此合同。 |

### BLK-003：NormalizationRunCurrent FK 目标列数不匹配

| 项 | 值 |
|---|---|
| 事实 | 原冻结稿的三列源→两列目标已按 DEC-B2-001 修正为 `(workspaceId, rawSnapshotId, rawRecordId) → RawRecord(workspaceId, rawSnapshotId, id)`。 |
| 证据 | design-freeze.md :265、:221；DEC-B2-001 |
| 影响 | 合同已可生成 PostgreSQL FK，仍需隔离库证明候选 DDL 与负例。 |
| 状态 | CLOSED（Phase 1） |
| 已确认决策 | DEC-E09、DEC-B1-001、DEC-B1-008、DEC-B1-009、DEC-B2-001 |
| 待完成证明 | 已完成：候选 schema/migration 可创建；合法同源链成功，跨 workspace/snapshot 关系均被 PostgreSQL 以 SQLSTATE `23503` 拒绝。 |
| 验证规则 | VAL-BLK-003 |
| 来源审计 | [V2 BLK-001～003 原始证据模型来源审计](../../progress/v2-blk-001-003-raw-evidence-source-audit-2026-08-04.md)；[V2 Evidence 边界审查](../../progress/v2-evidence-boundary-review-2026-08-04.md) 已确认同源键必须服务于一次观察与其片段；仍等待 BLK-001、BLK-002、BLK-016 的完整键契约。 |

### BLK-004：ContentObservation 无完整 model 块

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md §5.13 :404-413 以逗号分隔速记字段，无 `model ContentObservation { ... }` 块；无 `@id @default`；无写入者声明。 |
| 证据 | design-freeze.md :404-413 |
| 影响 | 无法无猜测生成 DDL |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-002 |
| 待完成证明 | 投影基数已确认；仍须来源化稳定 ContentAsset 关系、完整字段、键和唯一写入者。 |
| 验证规则 | VAL-BLK-004 |

### BLK-005：ContentCurrentProjection 无 FK 到 ContentObservation

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md :419-436 中 `currentObservationId String` 无 FK 定义。无法保证 Projection 引用的 Observation 存在且同 workspace。 |
| 证据 | design-freeze.md :419-436 |
| 影响 | 数据库层无法阻止孤儿投影 |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-002 |
| 待完成证明 | 依赖 ContentObservation 的完整可转录合同；当前冻结规范没有可证明的 FK。 |
| 验证规则 | VAL-BLK-005 |

### BLK-006：AuthorObservation 无 model 块

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md §5.13 :441 只有散文描述（"V2 目标 NOT NULL"、"新增列..."），无 model 块、无 PK、无 @@unique、无 FK。 |
| 证据 | design-freeze.md :441 |
| 影响 | 无法无猜测生成 DDL |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-002 |
| 待完成证明 | 投影基数已确认；仍须来源化稳定 Author 关系、完整字段、键和唯一写入者。 |
| 验证规则 | VAL-BLK-006 |

### BLK-007：AuthorCurrentProjection 无 FK 到 AuthorObservation

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md :445-461 中 `currentObservationId String` 无 FK 定义。 |
| 证据 | design-freeze.md :445-461 |
| 影响 | 同 BLK-005 |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-002 |
| 待完成证明 | 依赖 AuthorObservation 的完整可转录合同；当前冻结规范没有可证明的 FK。 |
| 验证规则 | VAL-BLK-007 |

### BLK-008：Comment 无 FK 到 CanonicalObservation

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md :466 只说"新增 canonicalObservationId String?"，无 FK 定义。 |
| 证据 | design-freeze.md :466 |
| 影响 | Comment 门禁 SQL join ContentObservation 但无 FK 保证 |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-003、DEC-B3-004 |
| 待完成证明 | 稳定 Comment、不可变 CommentObservation、hash replay 与 accepted Current 推进语义已确认；仍须来源化完整物理字段、复合 FK、onDelete、数据库权限与 V1 唯一键退役顺序。 |
| 验证规则 | VAL-BLK-008 |

#### DR-B3-004 — 同一评论跨观察的历史与 Current 语义

| 项 | 值 |
|---|---|
| 事实 | DEC-B3-001 已确认 Comment 稳定身份为 `(workspaceId,platform,noteSourceId,sourceId)`；freeze:L464-L466 只要求在现有 Comment 增加一个可空 `canonicalObservationId`，没有表达同一身份对应多个 CanonicalObservation 的历史。 |
| 冲突 | 不同 accepted 观察直接 INSERT 会被稳定身份唯一键拒绝；UPDATE 现有 Comment 会覆盖上一观察的 canonicalObservationId 与内容，无法保留不可变领域观察。 |
| 状态 | **CONFIRMED — 方案 A（DEC-B3-004）** |
| 非技术问题 | 同一条评论被多次观察时，是否像内容/作者一样保留每次观察的历史，同时让页面只读一个当前版本？ |
| 方案 A（推荐） | `Comment` 保持稳定身份与当前读模型；新增不可变 `CommentObservation` 保存每个候选事实观察（包含未 accepted 的观察），只有 accepted Observation 才可由 Comment 的 `currentObservationId` 单调推进。现有业务引用继续指向稳定 Comment.id。 |
| 方案 B | 每个观察创建独立 Comment 行，并另建 `CommentCurrentProjection` 指向当前行；现有所有 Comment 外键/业务引用需迁移到新的稳定实体或 Current 模型。 |
| 推荐依据 | A 最接近现役 Comment 业务主键和 freeze“演进现有 Comment”的方向，迁移面更小；两方案都不得覆盖历史或 fallback。 |
| 可证伪验收 | 同一 identity + 同内容 hash replay 不新增；同一 identity + 内容或状态变化保留新的不可变 Observation；未 accepted Observation 仍保留但不推进 current，accepted Observation 才推进 current；既有 accepted current 的保留/失效遵守其所属 Content Projection 可见性；跨 workspace FK 拒绝。 |
| 用户补充合同 | Comment 是稳定业务身份，不是采集结果；CommentObservation 是每次采集、内容变化或状态变化产生的事实记录。页面只读 `Comment.currentObservationId`；同 identity + 同内容 hash 为 replay，同 identity + 内容变化新增 Observation；未 accepted 的 Observation 保留但不推进 current，accepted 后才推进；禁止删除或覆盖历史 Observation。 |

### BLK-009：ContentMetricSnapshot 无 FK 在 rawRecordId/rawSnapshotId

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md :470 只说"新增 rawSnapshotId/rawRecordId"，无 FK DDL。 |
| 证据 | design-freeze.md :470 |
| 影响 | Metric 门禁 SQL join NormalizationRunCurrent on rawRecordId 但无 FK 保证 |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-003 |
| 待完成证明 | V2 新指标必须溯源已确认；仍须来源化物理关系、可空性与同源约束。历史缺失不得被伪造回填。 |
| 验证规则 | VAL-BLK-009 |

### BLK-010：TaskStatusProjection 无 ALTER DDL

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md :478 只说"新增 evidenceStatus/contractStatus/projectionStatus/mediaStatus"，无 ALTER TABLE ADD COLUMN DDL。 |
| 证据 | design-freeze.md :478 |
| 影响 | 无法无猜测生成 DDL |
| 状态 | OPEN（Phase 1） |
| 已确认决策 | DEC-B1-004 |
| 待完成证明 | 状态职责与“任务成功”边界已确认；仍须来源化值域、终态和唯一写入者。 |
| 验证规则 | VAL-BLK-010 |

### BLK-011：Topic projectionRevision 来源未定义

| 项 | 值 |
|---|---|
| 事实 | Presentation 门禁 §10⑨ :953-964 对 Topic 使用 `projectionRevision='static'`，但 Topic 模型（schema.prisma :358-360）中无 projectionVersion 字段。Topic.workspaceId 当前为 `String?`（可空），门禁 SQL `t."workspaceId"=${ws}` 会排除 NULL 行。 |
| 证据 | design-freeze.md :953-964；prisma/schema.prisma :358-360 |
| 影响 | Topic Presentation 门禁可能遗漏无 workspaceId 的 Topic；'static' revision 来源不在模型中定义 |
| 状态 | OPEN（Phase 3） |
| 已确认决策 | DEC-B1-005 |
| 待完成证明 | 普通页面不适用 receipt。只有被明确登记为发布、审核或强一致运营页面时，才转录其 workspace/revision 来源并证明旧回执失效。 |
| 验证规则 | VAL-BLK-011 |

### BLK-012：媒体撤销在 D3-5 中缺失

| 项 | 值 |
|---|---|
| 事实 | design-freeze.md D3-5 :584-589 撤销步骤只提 ContentCurrentProjection/AuthorCurrentProjection visibilityState 和 PresentationRequirement DELETE。未提 ContentMediaUsage 或其他媒体关系的隔离或撤销。 |
| 证据 | design-freeze.md :584-589 |
| 影响 | rejected 裁决后媒体仍可展示 |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-006 |
| 待完成证明 | 转录唯一关系撤销/恢复语义，并证明被拒绝对象不可展示其媒体、其它有效关系不受影响。 |
| 验证规则 | VAL-BLK-012 |

### BLK-013：CONTRADICTION — canonical_writer DELETE PresentationRequirement 与权限表冲突

| 项 | 值 |
|---|---|
| 事实 | D3-5 :588 说 canonical_writer 在同事务中 DELETE PresentationRequirement。权限表 :624 给 DELETE 只给 default_app，不给 canonical_writer。 |
| 证据 | design-freeze.md :588（D3-5 步骤 4）vs :624（权限表） |
| 影响 | canonical_writer 无法在裁决切换事务中完成 DELETE |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-006 |
| 待完成证明 | 撤销语义已确认；仍须取得同一事务唯一所有者与权限的可转录来源，而非凭冲突文字任选一方。 |
| 验证规则 | VAL-BLK-013 |

### BLK-014：CONTRADICTION — default_app 可覆盖 canonical_writer 的 visibilityState=quarantined

| 项 | 值 |
|---|---|
| 事实 | D3-5 :586 说 canonical_writer UPDATE visibilityState=quarantined。权限表 :621 给 default_app `UPDATE(投影推进)` 在同一 ContentCurrentProjection 表。当前规范未定义任何经数据库验证的机制，证明 default_app 的正常投影推进不能把 quarantined 覆盖回 visible。 |
| 证据 | design-freeze.md :586（D3-5 步骤 2）vs :621（权限表 default_app） |
| 影响 | canonical_writer 设置 quarantined 后，default_app 新投影可覆盖回 visible |
| 状态 | OPEN（Phase 2） |
| 已确认决策 | DEC-B1-006 |
| 待完成证明 | 转录唯一状态转换所有者，并以普通投影路径尝试恢复为 visible 的反例证明。 |
| 验证规则 | VAL-BLK-014 |

### BLK-015：default_app 被写为可正常读写 RawSnapshot/RawRecord

| 项 | 值 |
|---|---|
| 事实 | 权限表 :605 写 RawSnapshot default_app = "正常读写"；:606 写 RawRecord default_app = "正常读写"。EvidenceAccessAudit 的 `capturePackageId`（:519）只有索引、没有同 workspace FK（:525）。但 DEC-001 单轨原则要求只有 evidence_writer 可写证据表，DEC-E05 要求原始包访问可审计。 |
| 证据 | design-freeze.md :516-525、:605-606 |
| 影响 | default_app 可绕过 EvidenceIngress 直接写 RawSnapshot/RawRecord，读取审计也无法在数据库层证明归属真实同 workspace CapturePackage。 |
| 状态 | OPEN（Phase 1） |
| 已确认决策 | DEC-B1-007 |
| 待完成证明 | 采用数据库强制：来源化角色、受控入口、原始包读取与审计同源边界，并以 default_app 直写/直读、伪造或跨 workspace 审计负例证明。 |
| 验证规则 | VAL-BLK-015 |
| 决策影响 | DEC-E01、DEC-E02、DEC-E04、DEC-E05 与 DEC-B1-007 要求 Evidence 不能被绕过、覆盖、伪造或脱离 Registry 审计。冻结规范场景 9 要求数据库拒绝越权操作；实现只允许该证明链。 |

### BLK-016：Normalization 链路无法在 DDL 层证明 run、record、snapshot 同源

| 项 | 值 |
|---|---|
| 事实 | DEC-B2-001 已将 Run→Record、Current→Run、Current→Record、Observation→Record 四条关系全部修正为携带同一 workspace/rawSnapshotId 的三列复合 FK。 |
| 证据 | design-freeze.md :247-265、:290-292；DEC-B2-001 |
| 影响 | 合同层已封闭跨 snapshot 错绑，仍需由真实 PostgreSQL 约束证明。 |
| 状态 | CLOSED（Phase 1） |
| 已确认决策 | DEC-E09、DEC-B1-001、DEC-B1-008、DEC-B1-009、DEC-B2-001 |
| 待完成证明 | 已完成：合法同源链写入成功；四条关系的跨 workspace/snapshot 组合均以 SQLSTATE `23503` 拒绝。 |
| 验证规则 | VAL-BLK-016 |

---

## B2 Decision Registry

### DR-B2-001 — B2 同源复合外键的权威修正

| 项 | 值 |
|---|---|
| ID | DR-B2-001 |
| 当前事实 | 冻结规范要求 NormalizationRun、Current、CanonicalObservation 的 workspace、snapshot、record 同源，但四条 FK 的源/目标列数不一致或遗漏 rawSnapshotId；PostgreSQL 无法据此建立完整同源证明。 |
| 唯一可实施候选 | 将 NormalizationRun→RawRecord、NormalizationRunCurrent→NormalizationRun、NormalizationRunCurrent→RawRecord、CanonicalObservation→RawRecord 四条关系全部写成携带 `(workspaceId, rawSnapshotId, …Id)` 的三列复合 FK，并引用冻结规范已经声明的三列唯一键。 |
| 推荐方案 | A：授权审核主线修正冻结稿中的 FK 拼写，并以隔离库正例、跨 workspace/snapshot 反例证明后再推进 BLK-003/016。 |
| 不采用 A 的结果 | 保持当前矛盾，B2 schema 不得实施；不得由执行者在 migration 中私自纠正。 |
| 状态 | **CLOSED** |
| 确认来源 | 用户于 2026-08-10 明确授权方案 A；已登记 DEC-B2-001 并修正冻结稿。 |
| 阻塞范围 | B2-B-01 schema expand；不影响已完成的 B1 暗态代码。 |

### DR-B2-002 — XHS 首期 Normalization 输出合同

| 项 | 值 |
|---|---|
| ID | DR-B2-002 |
| 当前事实 | 固定点六份 XHS CapturePackage 均通过 B1 跨仓 validator；七表 75 个物理字段中 27 个身份/关系字段直接绑定、14 个由持久化层生成，其余 34 个运行字段现已由 DEC-B2-002 方案 A 固定。B2-B-02 暗态 writer 已实现，但仍没有现役 caller。 |
| 禁止推断 | 不得把 RawRecord.recordKind、targetKey、externalRecordId 或 V1 payload 字段直接当成 Canonical 语义；不得用通用测试夹具自证。 |
| 推荐方案 | A：采用版本化、无损、确定性的首期合同。三个 recordKind 使用独立 adapter 身份；Canonical 只包装稳定 identity、observedAt 与原始 JSON 深快照，不伪造跨平台领域字段；统一 canonical JSON + SHA-256；显式 retry 才增加 attempt；Normalization Current 仅由 normalized 推进，Evaluation Current 则由 accepted/rejected 两种 terminal evaluation 推进以支持撤销旧 accepted；裁决严格绑定 eligibility、terminal、slots、全部 RawRecord Current 和稳定输入顺序。完整字段、版本、hash、revision、错误码与篡改验收见 [来源审计与决策包](../../code-review/v2-b2-xhs-normalization-source-audit-2026-08-11.md#4-dr-b2-002-最小决策包)。 |
| 不采用 A 的结果 | 历史候选，不再适用；DEC-B2-002 已确认方案 A。 |
| 状态 | **CLOSED** |
| 证据进度 | DECISION_CONFIRMED；DARK_IMPLEMENTATION_PROVED。六份插件 fixture 均通过 B2 adapter/evaluator 固定 hash；新隔离库证明原子提交、replay、explicit retry、Current 撤销、漂移拒绝、并发收敛及晚期故障七表零残留。 |
| 阻塞范围 | B2-B-02 已解除；BLK-001/BLK-015 的受控 Evidence reader 能力和单独切流授权未具备前仍禁止接 caller。 |

---

## 协作治理 blocker

> GOV 条目约束协作包的可信度。它们不授权 B1，也不替代 BLK-001 至 BLK-016 的架构证明。

---
## Authority Decision Registry (B1-B-07)

> Authority blockers track the provenance of every field required by `EvidenceIngressAuthorityV2` per 07 §3.4. Each card records what IS traceable, what is NOT, and what must be decided before an ingress adapter can be implemented. Cards are referenced by the static audit modules in `src/lib/evidence/adapters/`.
>
> **Rule**: No card may be closed until the corresponding `DECISION_REQUIRED` question has a confirmed answer with a verifiable source (schema column, validated code path, or confirmed contract). Cards without verified sources remain `DECISION_REQUIRED`.

### Execution Authority

#### DR-B1-005-001 — sourcePrincipal

| 项 | 值 |
|---|---|
| ID | DR-B1-005-001 |
| 字段 | `EvidenceIngressAuthorityV2.sourcePrincipal` (execution) |
| 当前实际来源 | `verifyExecutionStationIngressSession` 产出的不可伪造、请求 body 绑定的严格签名工位会话；adapter 编码 `execution-station:<ExecutionStation.id>` |
| 已验证但不充分的候选 | 已由 DEC-B1-023 取代：不新增 principal 列，不接受 caller 字符串，不继承 V1 签名宽限 |
| 权威资料 | 07 §3.4；DEC-B1-023；`src/lib/services/execution-station-signature-service.ts`；`src/lib/evidence/adapters/execution-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：`sourcePrincipal = execution-station:<stationId>`，stationId 只能来自严格签名会话 |
| 前置条件 | 已满足：stationToken + mandatory HMAC + 当前授权 token/状态 + 不可伪造能力对象 + A/B body 绑定正反例 |

#### DR-B1-005-002 — executionPlanVersion

| 项 | 值 |
|---|---|
| ID | DR-B1-005-002 |
| 字段 | `CaptureHeaderV2.executionPlanVersion` (execution variant) |
| 当前实际来源 | `ExecutionJob.executionPlanVersion`；新 Prisma job 由服务端生成 cuid，历史 null 行不得提交 V2 execution Evidence |
| 已验证但不充分的候选 | 已由 DEC-B1-023 排除：`RawSnapshot` 是存储结果，`ExecutionPlannerSnapshot` 是容量规划快照，均不是 job plan provenance |
| 权威资料 | DEC-B1-023；`prisma/schema.prisma`；`prisma/migrations/20260810143000_add_execution_job_plan_version/migration.sql` |
| 状态 | **CLOSED** |
| 问题 | 已决：版本归属 ExecutionJob；同 job 重试复用，计划变化新建 job |
| 前置条件 | 已满足：header/job 精确绑定、null/不同版本负例和隔离库持久化证明 |

#### DR-B1-005-003 — 签名工位会话与 job workspace 同源绑定

| 项 | 值 |
|---|---|
| ID | DR-B1-005-003 |
| 字段 | `ExecutionJob.workspaceId` 与签名工位会话的 workspace binding |
| 当前实际来源 | 严格 verifier 自行读取当前 `PluginAuthorization` 行并验证 token/status/expiry，再绑定 `ExecutionStation.workspaceId`；adapter 再读取 `ExecutionJob.workspaceId`/`ExecutionQueueEntry.workspaceId` |
| 已验证但不充分的候选 | schema 继续允许历史 null，但 V2 verifier/adapter 对任一 null、错绑授权或跨 workspace 均 fail closed；不使用 default workspace fallback |
| 权威资料 | DEC-B1-023；`src/lib/services/execution-station-signature-service.ts`；`src/lib/evidence/adapters/execution-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：Station、PluginAuthorization、Job、Queue 的 workspace 必须全部非空且严格相等 |
| 前置条件 | 已满足：伪造/撤销授权、null、错绑、跨 workspace、陈旧 lease 正反例与隔离库零写证明 |

---

### Manual Import Authority

#### DR-B1-006-001 — sourcePrincipal

| 项 | 值 |
|---|---|
| ID | DR-B1-006-001 |
| 字段 | `EvidenceIngressAuthorityV2.sourcePrincipal` (manual_import) |
| 当前实际来源 | 已验证用户会话 `userId`；`manual-import-adapter.ts` 编码为 `user:<userId>` |
| 已验证但不充分的候选 | 已由 DEC-B1-022 取代：不使用 deviceId、email、显示名或 role 作为 principal |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/manual-import-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：manual_import `sourcePrincipal = user:<userId>` |
| 前置条件 | 已满足：authenticated workspace session + request-bound validator 测试 |

#### DR-B1-006-002 — importerIdentity

| 项 | 值 |
|---|---|
| ID | DR-B1-006-002 |
| 字段 | `EvidenceIngressAuthorityV2.importerIdentity` (manual_import) |
| 当前实际来源 | bearer token 验证得到的 `PluginAuthorization.id`；编码为 `plugin-authorization:<id>` |
| 已验证但不充分的候选 | 已由 DEC-B1-022 取代：deviceId 只在 authorizationCode 范围内唯一，不作为 importer identity |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/services/plugin-authorization-service.ts:1222-1253` |
| 状态 | **CLOSED** |
| 问题 | 已决：`importerIdentity = plugin-authorization:<PluginAuthorization.id>` |
| 前置条件 | 已满足：token-authenticated authorization row + exact binding test |

#### DR-B1-006-003 — workspaceId 双源同 workspace 绑定

| 项 | 值 |
|---|---|
| ID | DR-B1-006-003 |
| 字段 | `PluginAuthorization.workspaceId` 与 session `workspaceId` 的等同性 |
| 当前实际来源 | `PluginAuthorization.workspaceId` + authenticated session `workspaceId` |
| 已验证但不充分的候选 | 已实现 fail-closed：authorization workspace 为空或与 session 不相等均拒绝 |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/manual-import-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：adapter 在 authority 构造前强制两个已验证来源严格相等 |
| 前置条件 | 已满足：null 与跨 workspace 负例测试 |

#### DR-B1-006-004 — receivedAt 服务端时钟注入

| 项 | 值 |
|---|---|
| ID | DR-B1-006-004 |
| 字段 | `EvidenceIngressAuthorityV2.receivedAt` (manual_import) |
| 当前实际来源 | `manual-import-adapter.ts` 内部服务端 clock；body/header 无输入位 |
| 已验证但不充分的候选 | 已实现 request-bound exact validator；绑定后修改 timestamp 会失败 |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/manual-import-adapter.test.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：adapter 内部调用 clock 生成并复制有效 Date |
| 前置条件 | 已满足：固定时钟正例、body 泄漏与绑定后篡改负例 |

#### DR-B1-006-005 — 禁止 execution 字段泄漏到 manual_import Evidence

| 项 | 值 |
|---|---|
| ID | DR-B1-006-005 |
| 字段 | `jobId`/`attemptId`/`stationId`/`leaseToken`/`leaseEpoch`/`executionPlanVersion` 在 manual_import 上为 null |
| 当前实际来源 | manual_import 判别联合 + `request-bound-authority.ts` runtime guard |
| 已验证但不充分的候选 | 当前暗态 adapter 与 EvidenceIngress non-execution 隔离库证明组合覆盖；现役 V1 route 尚未切换 |
| 权威资料 | 07 §3.4、§5.2；DEC-B1-022；adapter tests；`evidence-ingress.integration.test.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：body/header 任一 execution identity 均在 authority 构造前拒绝 |
| 前置条件 | 已满足：runtime 负例 + non-execution 持久化 null 证明；仍不授权 route 切流 |

---

### Recovery Authority

> Recovery route: `POST /api/execution-tasks/recovery-import`
> Authority: `requireRequestWorkspaceRole(request, ["owner", "admin"])` → `context.workspaceId` + `context.userId`
> No plugin authorization required. Body provides `export` payload + optional `jobId`.

#### DR-B1-006-R-001 — sourcePrincipal

| 项 | 值 |
|---|---|
| ID | DR-B1-006-R-001 |
| 字段 | `EvidenceIngressAuthorityV2.sourcePrincipal` (recovery) |
| 当前实际来源 | `src/app/api/execution-tasks/recovery-import/route.ts:25-29` `requireRequestWorkspaceRole(request, ["owner", "admin"])` |
| 已验证但不充分的候选 | 已由 DEC-B1-022 取代：role 只用于授权，不进入稳定身份 |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/recovery-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：recovery `sourcePrincipal = user:<userId>` |
| 前置条件 | 已满足：owner/admin 正例与非授权角色负例 |

#### DR-B1-006-R-002 — recoveryAuthorizedBy

| 项 | 值 |
|---|---|
| ID | DR-B1-006-R-002 |
| 字段 | `EvidenceIngressAuthorityV2.recoveryAuthorizedBy` (recovery) |
| 当前实际来源 | owner/admin session `userId`；V2 adapter 不使用 V1 `"workbench"` fallback |
| 已验证但不充分的候选 | 已实现空 userId 拒绝；授权者与 principal 指向同一会话用户 |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/recovery-adapter.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：`recoveryAuthorizedBy = user:<userId>`，缺失 userId 必须拒绝 |
| 前置条件 | 已满足：owner/admin role gate + missing user negative test |

#### DR-B1-006-R-003 — receivedAt

| 项 | 值 |
|---|---|
| ID | DR-B1-006-R-003 |
| 字段 | `EvidenceIngressAuthorityV2.receivedAt` (recovery) |
| 当前实际来源 | `recovery-adapter.ts` 内部服务端 clock；导出包时间只属于 observation body |
| 已验证但不充分的候选 | 已实现 request-bound exact validator；caller 不能提交或覆盖 receivedAt |
| 权威资料 | 07 §3.4；DEC-B1-022；`src/lib/evidence/adapters/recovery-adapter.test.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：receivedAt 由 recovery adapter 内部服务端时钟生成 |
| 前置条件 | 已满足：固定时钟正例、body 泄漏与绑定后篡改负例 |

#### DR-B1-006-R-004 — body authority isolation

| 项 | 值 |
|---|---|
| ID | DR-B1-006-R-004 |
| 字段 | recovery body 不得覆盖 authority 或携带 execution identity |
| 当前实际来源 | `request-bound-authority.ts` 在 authority 构造前扫描 outer body/header，并精确绑定生成值 |
| 已验证但不充分的候选 | 现役 V1 body 仍未切换；暗态 V2 adapter 不调用 V1 recovery service |
| 权威资料 | 07 §3.4、§5.2；DEC-B1-022；`src/lib/evidence/adapters/recovery-adapter.test.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：V2 recovery adapter 是 body 与 authority 的唯一隔离边界 |
| 前置条件 | 已满足：authority 泄漏、execution identity 与绑定后篡改负例 |

#### DR-B1-006-R-005 — execution identity separation

| 项 | 值 |
|---|---|
| ID | DR-B1-006-R-005 |
| 字段 | execution 专属字段在 recovery Evidence 上必须为 null |
| 当前实际来源 | recovery 判别联合 + runtime guard；V2 adapter 无 DB/service dependency |
| 已验证但不充分的候选 | 当前暗态 adapter 与 EvidenceIngress non-execution 隔离库证明组合覆盖；V1 route 尚未切换 |
| 权威资料 | 07 §3.4、§5.2；DEC-B1-022；adapter tests；`evidence-ingress.integration.test.ts` |
| 状态 | **CLOSED** |
| 问题 | 已决：V2 recovery submission 不含 execution identity，也不调用控制平面 |
| 前置条件 | 已满足：runtime 负例 + persisted null proof；仍不授权 route 切流 |

---

### Migration Authority

> 07 §3.4: "migration：没有网络入口，全部四项来自未来获授权的内部调用者"
> 07 §7: "首期不新增 migration API、CLI 或定时任务"
> Migration 当前没有任何 route、service 或 runtime caller。所有 authority 字段均未实现。

#### DR-B1-006-M-001 — workspaceId

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-001 |
| 字段 | `EvidenceIngressAuthorityV2.workspaceId` (migration) |
| 当前实际来源 | 无——migration 是内部受控能力且当前无 ingress/caller |
| 已验证但不充分的候选 | 仅存在 V2 authority 类型字段；测试 fixture 不是 runtime provenance |
| 权威资料 | 07 §3.4、§7；`src/lib/evidence/ingress/types.ts:186-192`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 哪个获授权的内部 migration caller 提供并绑定 workspaceId？ |
| 前置条件 | 必须存在可验证的内部调用边界及 workspace 绑定；不得由 body 或未绑定参数推断 |

#### DR-B1-006-M-002 — sourcePrincipal

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-002 |
| 字段 | `EvidenceIngressAuthorityV2.sourcePrincipal` (migration) |
| 当前实际来源 | 无——migration authority 仅存在于未来内部调用者合同 |
| 已验证但不充分的候选 | `src/lib/evidence/ingress/types.ts:186-192` 与 fixture 只证明字段形状，不证明 runtime identity provenance |
| 权威资料 | 07 §3.4；`src/lib/evidence/ingress/types.ts:186-192`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 哪个内部调用者及其身份格式是 migration sourcePrincipal 的权威来源？ |
| 前置条件 | 内部调用者身份格式和不可伪造绑定必须先获确认 |

#### DR-B1-006-M-003 — migrationAuthorization

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-003 |
| 字段 | `EvidenceIngressAuthorityV2.migrationAuthorization` (migration) |
| 当前实际来源 | 无——migration 没有 route/service/caller |
| 已验证但不充分的候选 | validator 仅检查非空形状，不证明授权 provenance；fixture 不是授权来源 |
| 权威资料 | 07 §3.4；`src/lib/evidence/ingress/types.ts:186-192`；`src/lib/evidence/ingress/validator.ts:526-527`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 内部 migration caller 获得的授权引用是什么格式，且在 V2 构造前何处验证？ |
| 前置条件 | 必须存在唯一内部授权引用和可验证校验边界 |

#### DR-B1-006-M-004 — receivedAt

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-004 |
| 字段 | `EvidenceIngressAuthorityV2.receivedAt` (migration) |
| 当前实际来源 | 无——仅存在类型字段；当前无 runtime adapter 注入服务端时钟 |
| 已验证但不充分的候选 | fixture 不能证明服务端时钟注入 |
| 权威资料 | 07 §3.4；`src/lib/evidence/ingress/types.ts:173-178`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 哪个内部 migration adapter 注入服务端时钟 receivedAt，且在哪个测试/运行路径证明？ |
| 前置条件 | receivedAt 必须由服务端时钟注入且不可由 caller body 覆盖 |

#### DR-B1-006-M-005 — body authority isolation

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-005 |
| 字段 | migration Evidence 不得接受 body-controlled authority 字段 |
| 当前实际来源 | 仅有判别联合类型隔离；无 migration runtime ingress 证明 |
| 已验证但不充分的候选 | body 与 authority 分离的类型/fixture 不是 runtime caller 证明 |
| 权威资料 | 07 §7、§5.2；`src/lib/evidence/ingress/types.ts:156-192`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 唯一授权的内部 migration 调用边界是什么，以及如何阻止 body 覆盖 authority 字段？ |
| 前置条件 | adapter 必须在 authority 构造前隔离 body-controlled authority；不能以类型或 fixture 代替运行时证明 |

#### DR-B1-006-M-006 — execution identity separation

| 项 | 值 |
|---|---|
| ID | DR-B1-006-M-006 |
| 字段 | migration Evidence 不得携带 `jobId`/`attemptId`/lease/task-attempt execution identity |
| 当前实际来源 | 仅有 migration header 判别联合类型无 execution 字段；无 runtime caller 证明 |
| 已验证但不充分的候选 | 类型与 fixture 不能证明运行时不会进入 execution identity |
| 权威资料 | 07 §3.4、§5.2、§7；`src/lib/evidence/ingress/types.ts:84-105`；`src/lib/evidence/ingress/test-fixtures.ts:191-195` |
| 状态 | **DECISION_REQUIRED** |
| 问题 | 哪个内部 migration caller 证明 jobId、attemptId、lease 和 task-attempt 关系永不进入 migration Evidence？ |
| 前置条件 | migration adapter 必须拒绝 execution identity，并由隔离测试证明 |

---

### GOV-001：原状态机无可达终态且造成验证死锁

| 项 | 值 |
|---|---|
| 事实 | G0 初版只允许 OPEN、DECISION_REQUIRED、READY_FOR_PROOF，且明示不得 CLOSED；但 `00-contract.md` / `05-execution-plan.md` 又要求 CLOSED 才能进入 B1。初版 `03-validation-matrix.md` 还要求全部 CLOSED 前不创建隔离库。 |
| 证据 | commit `3d68ebc4` 的 `00-contract.md`、`03-validation-matrix.md`、`04-blocker-ledger.md`、`05-execution-plan.md`；本文件上方状态机修正 |
| 影响 | 无论设计是否完成，验证与关闭均无法发生。 |
| 状态 | CLOSED |
| 待澄清问题 | 状态机是否在所有协作文件中唯一、可达，并允许仅对已具备依赖的规则做隔离验证？ |
| 验证规则 | VAL-GOV-001 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |

### GOV-002：项目入口曾与 B1 禁止规则冲突

| 项 | 值 |
|---|---|
| 事实 | `docs/TODO.md` 初版“下一步”写“只启动 EvidenceIngress 证据收口”，而协作包禁止 B1。 |
| 证据 | `docs/TODO.md` :20-29（当前入口）；commit `3d68ebc4` 的 `00-contract.md` / `05-execution-plan.md` |
| 影响 | 后续执行者可能按主入口提前实施 schema、代码或数据库。 |
| 状态 | CLOSED |
| 待澄清问题 | 项目唯一入口是否只允许设计证明与 blocker 收口，而不包含 B1 实施授权？ |
| 验证规则 | VAL-GOV-002 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |

### GOV-003：未获用户确认的技术选择被误登记为 DEC

| 项 | 值 |
|---|---|
| 事实 | G0 初版将 CapturePackage 存储、事务边界、ReleaseException、可见性、Policy 存放、Normalization 模型、门禁回执与审计 ID 等技术选择写为 `DEC-003` 至 `DEC-010`；来源是历史提示/审核建议，不能证明是用户直接确认。本轮新增的 DEC-E01～E07 均可定位到用户直接确认，因而不属于这项历史误登记。 |
| 证据 | `01-decisions.md` 的 DEC-001、DEC-002、DEC-E01～DEC-E07 均有用户确认来源；未决项 UND-007 至 UND-013 仍未获确认。 |
| 影响 | 架构尚未裁决的选择会被误作不可变规则，导致执行者越权实施。 |
| 状态 | CLOSED |
| 待澄清问题 | `DEC` 是否只保留能定位到用户直接确认的决策，其他是否都必须保留在 UND 直至确认？ |
| 验证规则 | VAL-GOV-003 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |

### GOV-004：初版 blocker 没有可证伪完成条件

| 项 | 值 |
|---|---|
| 事实 | 初版多数 blocker 的“完成验证”只是 `02-model-contract.md` 由 PENDING 改为 ✓，没有正例、反例、命令或实际证据。 |
| 证据 | commit `3d68ebc4` 的 `04-blocker-ledger.md`；当前 [03-validation-matrix.md](03-validation-matrix.md) |
| 影响 | 文档看似完成，DDL、权限和跨 workspace 负例仍可能失败。 |
| 状态 | CLOSED |
| 待澄清问题 | 每个 blocker 是否均有一一对应的正例、反例、前置条件、命令、预期和实际证据状态？ |
| 验证规则 | VAL-GOV-004 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |

### GOV-005：文档规模快照曾未随协作包更新

| 项 | 值 |
|---|---|
| 事实 | G0 新增 6 个 Markdown 文件后，`docs/project-audit.md` 仍写 331；治理脚本报告实际为 338。 |
| 证据 | `docs/project-audit.md` :75；`node scripts/check-project-governance.mjs --json` 的 G0 审核输出 |
| 影响 | 后续审计将无法区分既有治理债务和本批文档漂移。 |
| 状态 | CLOSED |
| 待澄清问题 | 现实快照是否已依据本次治理脚本实际输出更新，且治理失败是否被如实保留？ |
| 验证规则 | VAL-GOV-005 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |

### GOV-006：B0 对 RawSnapshot/RawRecord 权限的历史误读

| 项 | 值 |
|---|---|
| 事实 | B0 报告称 default_app“不能”写 RawSnapshot/RawRecord，理由是权限表只给 evidence_writer INSERT；但冻结规范权限表实际写 `RawSnapshot default_app = 正常读写`、`RawRecord default_app = 正常读写`。B0 报告必须保留，不能为消除矛盾而篡改。 |
| 证据 | `reviews/v2-b0-d3-transcription-audit.md` :71；`content-workbench-v2-design-freeze.md` :605-606（以当前行号为准） |
| 影响 | 若按 B0 误读实施，EvidenceIngress 的单一写入口会被错误视为已被权限保证。 |
| 状态 | CLOSED |
| 待澄清问题 | ledger 是否明确采用冻结规范/源码作为事实，保留 B0 历史原文并单独记录这项更正？ |
| 验证规则 | VAL-GOV-006 |
| 证明证据 | [V2-G0 协作治理只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)（SOURCE_AUDIT） |
| 审核关闭 | 2026-08-04 17:50 Asia/Shanghai；固定点 `79ec02a3`，两条独立审查均 PASS。 |
