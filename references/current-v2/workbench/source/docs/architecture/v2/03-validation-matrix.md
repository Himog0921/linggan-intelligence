# 03 — V2 验证矩阵

> 这是 blocker 的验收合同，不是“文档勾选表”。每条规则都必须有正例、反例、前置条件、可执行命令、预期结果和保存的实际证据。没有实际证据，状态必须保持 `PENDING`；不得写“理论上通过”。

## 通用执行规则

- `SOURCE_AUDIT` 是只读核对设计文本、当前 schema 与台账。
- `ISOLATED_DB` 只能在该规则依赖的 blocker 已 `READY_FOR_PROOF` 后执行；不得触碰 `127.0.0.1:54329/content_workbench_local`。
- 隔离库 DDL 必须逐行忠实转录已批准的模型契约；`$DDL_FILE` 是评审已批准的候选 DDL 文件，不能由验证者临时补列、补 FK、改权限来“跑通”。
- 每个 `ISOLATED_DB` 规则必须随已批准 DDL 一并提交两个**已审核、不可临时替换**的脚本：`$POSITIVE_SQL_FILE`（合法场景，必须成功）和 `$NEGATIVE_SQL_FILE`（本规则写明的非法场景，必须失败）。脚本 SHA-256 与固定点 SHA 一同登记到证据。没有这两个脚本，规则不得从 `PENDING` 变为 `PASS`。
- 下列命令中的 `v2_isolated_proof` 是唯一隔离验证执行方式：

```bash
v2_isolated_proof() {
  set -euo pipefail
  : "${ISOLATED_DATABASE_URL:?}" "${DDL_FILE:?}" "${POSITIVE_SQL_FILE:?}" "${NEGATIVE_SQL_FILE:?}"
  psql "$ISOLATED_DATABASE_URL" -v ON_ERROR_STOP=1 -f "$DDL_FILE"
  psql "$ISOLATED_DATABASE_URL" -v ON_ERROR_STOP=1 -f "$POSITIVE_SQL_FILE"
  if psql "$ISOLATED_DATABASE_URL" -v ON_ERROR_STOP=1 -f "$NEGATIVE_SQL_FILE"; then
    echo "negative case was accepted" >&2
    return 1
  fi
}
```

- 每次执行把命令输出、三个输入文件的 SHA-256、固定 SHA、数据库版本、执行时间与正/反例结果保存到对应 blocker 的证据链接；审核方据此将 `PROVED` 改为 `CLOSED`。

| 状态 | 含义 |
|---|---|
| `PENDING` | 规则尚未具备验证前置或尚未执行 |
| `PASS` | 正例和反例均有可复核实际证据 |
| `FAIL` | 验证命令运行但正例失败、反例未被拒绝或证据矛盾 |

---

## 已确认原则的未来验证门槛（非 DDL、非实施授权）

以下是已确认 Evidence 与 B1 决策对后续相关 blocker 验证的最低要求。它们不是已执行的测试，不补定任何表、字段、SQL 或实现路径；在对应模型与验证前置获批前，所有相关验证仍为 `PENDING`。

| 已确认规则 | 后续验证必须能证伪的反例 |
|---|---|
| DEC-E01 | 页面/UI 直接读取原始 Evidence，或 AI/Derived 流程覆盖原始 Evidence，必须被拒绝或被审计判定失败。 |
| DEC-E02 | 对既有 Evidence 覆盖历史内容、采集时间、来源或 payload，或无处置记录地删除 Evidence，必须失败。 |
| DEC-E03 / DEC-E09 | 同次同 capture identity、同完整 package hash 新建第二份 Evidence，或不同时间观察覆盖旧 Evidence，必须失败；同一 capture identity 但不同完整 package hash 也不得覆盖既有证据或进入投影。 |
| DEC-E04 | manual import、recovery、migration、人工修复伪造 ExecutionJob/CaptureAttempt，必须失败。 |
| DEC-E05 | 对象存储记录被当作 Evidence 身份或元数据的第二事实来源，必须失败。 |
| DEC-E06 | Derived/AI 无法获得 Evidence 的质量、置信度或验证状态时，相关处理不得标为可用。 |
| DEC-E07 | 未经对象类型留存策略的无差别采集或清理，不得被标为合规的策略执行。 |
| DEC-B1-001 / 008 / 009 | `ARCHIVED` 可重算、`REDACTED`/`PURGED` 不可用；冲突包保留但不可投影；质量只能表达本次观察。 |
| DEC-B1-005 / 010 / 014 | 普通页面绕过 Projection、直接读取媒体或自行拼接媒体/原文链接，必须失败；封面必须遵循“明确封面 → 同观察首张已证明图片 → 不可用”的统一次序；普通页面不要求 PresentationReceipt。 |
| DEC-B1-006 | 撤销内容与媒体的业务关系后，该内容不得展示该媒体；其它有效业务关系仍可展示同一 Media Asset。 |

每条未来验证都仍须满足本文件的正例、反例、命令和实际证据要求；不能仅因本文记录了原则而标记 PASS。

## 代码实施前准备状态（2026-08-05）

> DC-001～DC-009 已在 2026-08-05 获确认；但这不等于模型、键、权限和删除规则已经来源完整。所有 BLK 仍为 `OPEN`，尚无一项可进入 `READY_FOR_PROOF`。因此本节**不**新建 DDL、隔离数据库、候选 SQL、测试脚本或证明命令；下列 `VAL-BLK-*` 是未来验收合同，不能被误解为本轮实施指令。

| 已确认决策 | 对应验证规则 | 进入 READY_FOR_PROOF 前仍必须具备的来源 | 正例 / 反例状态 |
|---|---|---|---|
| DEC-B1-001、008、009 | VAL-BLK-001、002、003、016 | RawSnapshot/RawRecord 的逐字段、键、权限、删除规则来源完整。 | 已定义为未来验收；未执行。 |
| DEC-B1-002 | VAL-BLK-004、005、006、007 | Canonical→领域 Observation 的物理关系、稳定身份和唯一写入边界来源完整。 | 已定义为未来验收；未执行。 |
| DEC-B1-003 | VAL-BLK-008、009 | Comment/Metric 的 V2 溯源关系、可空性和同源边界来源完整。 | 已定义为未来验收；未执行。 |
| DEC-B1-004 | VAL-BLK-010 | 各状态的值域、终态和唯一写入者来源完整。 | 已定义为未来验收；未执行。 |
| DEC-B1-005 | VAL-BLK-011 | 仅对已登记的发布、审核、强一致运营页面确认 receipt 的 revision/workspace 来源。 | 已定义为未来验收；未执行。 |
| DEC-B1-006 | VAL-BLK-012、013、014 | 关系撤销、对象 Projection 与权限事务的唯一实施边界来源完整。 | 已定义为未来验收；未执行。 |
| DEC-B1-007 | VAL-BLK-015 | 数据库角色、受控入口、原始包读取和审计同源关系来源完整。 | 已定义为未来验收；未执行。 |

这满足“有完整模型契约来源的 blocker 才能准备隔离证明”的纪律：即使业务决策已经确认，也不得以看似合理的字段、外键、角色或 SQL 补出正反例，更不能把 `PENDING` 改为 `PASS`。

## Blocker 验证规则

### VAL-BLK-001 — RawSnapshot 完整模型

- 对应：BLK-001；环境：SOURCE_AUDIT → ISOLATED_DB；前置：RawSnapshot 在 `02-model-contract.md` 由已批准来源完整填写。
- 正例：候选定义明确全部字段、主键、唯一键、FK、写入者与删除规则；忠实转录的 DDL 可编译。
- 反例：候选 schema 缺失已批准契约中的任一字段、键或关系时，契约断言必须失败；仅 DDL 能编译不构成通过。
- 命令：`rg -n '^model RawSnapshot|RawSnapshot' docs/architecture/content-workbench-v2-design-freeze.md docs/architecture/v2/02-model-contract.md`；具备前置后执行 `v2_isolated_proof`。`$NEGATIVE_SQL_FILE` 必须验证“缺失已批准契约中任一必需字段、键或关系的 schema 不能通过契约断言”，而不是错误地假定“删 FK 后 PostgreSQL 会自行报错”。
- 预期/实际证据：SOURCE_AUDIT 证明契约无空项；ISOLATED_DB 的成功输出与负例失败输出。当前：PENDING，无实际证据。

### VAL-BLK-002 — RawRecord 完整模型与改名路径

- 对应：BLK-002；环境：SOURCE_AUDIT → ISOLATED_DB；前置：approved 契约明确 `recordType` 到终态字段的可执行迁移顺序。
- 正例：目标模型和改名/回填/约束顺序完整，忠实 DDL 成功。
- 反例：以当前只含 `recordType` 的 schema 直接执行终态字段约束必须被拒绝或在审计中标为缺少前置。
- 命令：`rg -n 'recordType|recordKind|RawRecord' prisma/schema.prisma docs/architecture/v2/02-model-contract.md docs/architecture/v2/04-blocker-ledger.md`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须从迁移前 schema 执行缺少改名/回填前置的终态约束，并以失败为通过条件。
- 预期/实际证据：改名路径与 schema 实际字段对齐；隔离库正、反例输出。当前：PENDING。

### VAL-BLK-003 — NormalizationRunCurrent FK 列数与目标键一致

- 对应：BLK-003；环境：SOURCE_AUDIT → ISOLATED_DB；前置：DEC-B2-001 已关闭，Release-A schema 已提供 `RawSnapshot(workspaceId,id)` 与 `RawRecord(workspaceId,rawSnapshotId,id)` 唯一键。BLK-001/002 的终态改名、权限与历史迁移缺口继续独立保持 OPEN，不阻塞本次纯 expand 结构证明。
- 正例：FK 源列数等于目标唯一键列数，且可在隔离库创建。
- 反例：将三列源 FK 指向两列目标键时，PostgreSQL 必须拒绝。
- 命令：`V2_B2_INTEGRATION_DB=1 npx vitest run src/lib/evidence/derived/b2-derived-schema.integration.test.ts`。测试必须在独立数据库中临时建立两列目标唯一键，并证明旧三列源→两列目标 FK 被 PostgreSQL 拒绝。
- 预期/实际证据：6 项测试各自在全新隔离库从固定点 schema 应用候选 migration；合法七表链成功，旧三列源→两列目标 DDL 以 SQLSTATE `42830` 拒绝并自动回滚，跨来源写入以 `23503` 拒绝。证明库从 `content_workbench_v2_b2_schema_1786363969513_1` 至 `content_workbench_v2_b2_schema_1786363978407_6` 均保留；单独过滤最后一项亦 1/1 通过。固定 SQL、SHA-256、PostgreSQL 版本、时间与输出见 [B2 隔离证明](../../progress/v2-b2-derived-schema-isolated-proof-2026-08-10.md)。当前：PASS（6/6）。

### VAL-BLK-004 — ContentObservation 完整模型

- 对应：BLK-004；环境：SOURCE_AUDIT → ISOLATED_DB；前置：ContentObservation 的字段、键、来源与唯一写入者均已获批准。
- 正例：模型可完整转录且 DDL 成功。
- 反例：缺少主键、workspace 隔离或 CanonicalObservation 关系的候选定义不能进入隔离库。
- 命令：`rg -n 'ContentObservation|canonicalObservationId' docs/architecture/v2/02-model-contract.md docs/architecture/v2/04-blocker-ledger.md`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须验证缺少已批准 PK、workspace 隔离或 Canonical 关系的 schema 不能通过契约断言。
- 预期/实际证据：完整契约及隔离库正、反例输出。当前：PENDING。

### VAL-BLK-005 — Content 当前投影引用完整性

- 对应：BLK-005；环境：SOURCE_AUDIT → ISOLATED_DB；前置：BLK-004 为 `READY_FOR_PROOF`。
- 正例：同 workspace 的有效 Observation 可以被当前投影引用。
- 反例：不存在或另一 workspace 的 Observation 不能被当前投影引用。
- 命令：`rg -n 'ContentCurrentProjection|currentObservationId|ContentObservation' docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须构造不存在与另一 workspace Observation 的两次引用写入，并以均被拒绝为通过条件。
- 预期/实际证据：插入正例成功、两个负例被约束拒绝。当前：PENDING。

### VAL-BLK-006 — AuthorObservation 完整模型

- 对应：BLK-006；环境：SOURCE_AUDIT → ISOLATED_DB；前置：AuthorObservation 的目标字段、键、来源与写入边界已获批准。
- 正例：模型可完整转录且 DDL 成功。
- 反例：缺少 PK、workspace 归属、唯一键或 Canonical 关系的候选不能进入 proof。
- 命令：`rg -n 'AuthorObservation|AuthorSnapshot' docs/architecture/content-workbench-v2-design-freeze.md docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须验证缺少已批准 PK、workspace 隔离、唯一键或 Canonical 关系的 schema 不能通过契约断言。
- 预期/实际证据：完整契约及隔离库正、反例输出。当前：PENDING。

### VAL-BLK-007 — Author 当前投影引用完整性

- 对应：BLK-007；环境：SOURCE_AUDIT → ISOLATED_DB；前置：BLK-006 为 `READY_FOR_PROOF`。
- 正例：同 workspace AuthorObservation 可以被 Author 当前投影引用。
- 反例：不存在或跨 workspace 的 Observation 引用失败。
- 命令：`rg -n 'AuthorCurrentProjection|currentObservationId|AuthorObservation' docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须构造不存在与跨 workspace Observation 的两次引用写入，并以均被拒绝为通过条件。
- 预期/实际证据：正例成功，两个负例失败。当前：PENDING。

### VAL-BLK-008 — Comment 与 CanonicalObservation 的关系

- 对应：BLK-008；环境：SOURCE_AUDIT → ISOLATED_DB；前置：Comment 的 V2 溯源语义已获批准。
- 正例：允许的 Comment 溯源关系可写入。
- 反例：若该关系被定义为强制，孤儿和跨 workspace 写入必须失败；若被定义为可空，只有非空非法引用必须失败。
- 命令：`rg -n 'model Comment|canonicalObservationId' prisma/schema.prisma docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须按已批准的可空语义构造非空孤儿与跨 workspace 引用，并以被拒绝为通过条件。
- 预期/实际证据：经批准的可空性与正、反例一致。当前：PENDING。

### VAL-BLK-009 — Metric 溯源完整性

- 对应：BLK-009；环境：SOURCE_AUDIT → ISOLATED_DB；前置：ContentMetricSnapshot 的 raw snapshot/record 关系已获批准。
- 正例：同 workspace、同快照的 metric 溯源可写入。
- 反例：孤儿、跨 workspace 或跨 snapshot 组合被拒绝。
- 命令：`rg -n 'ContentMetricSnapshot|rawSnapshotId|rawRecordId' docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须构造孤儿、跨 workspace 与跨 snapshot 组合，并以均被拒绝为通过条件。
- 预期/实际证据：约束证明同源性，负例被拒绝。当前：PENDING。

### VAL-BLK-010 — TaskStatusProjection 五维状态

- 对应：BLK-010；环境：SOURCE_AUDIT → ISOLATED_DB；前置：四个新增状态字段的值域、写入者、迁移顺序已获批准。
- 正例：合法五维状态写入、读取与门禁查询均成功。
- 反例：缺任一必需列或写入未定义状态时，约束/服务验证失败。
- 命令：`rg -n 'TaskStatusProjection|evidenceStatus|contractStatus|projectionStatus|mediaStatus' prisma/schema.prisma docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须写入未定义状态或缺失必需状态，并以约束/服务拒绝为通过条件。
- 预期/实际证据：正例成功，负例可复核失败。当前：PENDING。

### VAL-BLK-011 — 强一致页面的展示修订可失效

- 对应：BLK-011；环境：SOURCE_AUDIT → ISOLATED_DB；前置：某发布、审核或强一致运营页面被明确登记为需要回执，并有其 workspace 与 revision 事实来源。
- 正例：对象事实更新后，以旧 revision 写入的 PresentationReceipt 不再满足展示条件。
- 反例：`static` 或任何不随事实变化的 revision 不得继续令旧 receipt 通过。
- 命令：`rg -n "projectionRevision.*static|Topic|ExecutionJob" docs/architecture/content-workbench-v2-design-freeze.md prisma/schema.prisma`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须先写入旧 revision receipt，再更新 Topic/Task 事实并证明该旧 receipt 不再满足展示查询。
- 预期/实际证据：更新后的负例不能展示。普通页面不适用本规则，直接消费 Projection。当前：PENDING。

### VAL-BLK-012 — rejected 裁决撤销媒体展示

- 对应：BLK-012；环境：SOURCE_AUDIT → ISOLATED_DB；前置：媒体可见性事实来源及读取过滤规则已获用户确认。
- 正例：accepted 内容可得到已批准媒体展示关系。
- 反例：同一内容被新的 rejected 裁决替代后，该内容的媒体关系与对象 Projection 均不可展示；同一 Media Asset 被其它有效业务关系引用时仍可展示。
- 命令：`rg -n 'ContentMediaUsage|AuthorMedia|CanonicalMediaSlot|visibilityState|rejected' docs/architecture/content-workbench-v2-design-freeze.md docs/architecture/v2/{02-model-contract.md,04-blocker-ledger.md}`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须完成 accepted→rejected 切换后执行媒体读取，任何被撤销媒体仍可返回即为失败。
- 预期/实际证据：撤销后媒体不可展示的查询结果。当前：PENDING。

### VAL-BLK-013 — 撤销事务与权限一致

- 对应：BLK-013；环境：SOURCE_AUDIT → ISOLATED_DB；前置：撤销事务的唯一写入者与权限边界已获用户确认。
- 正例：批准的写入者能在同一事务完成所有必需撤销动作。
- 反例：未授权角色执行任一撤销动作时被 PostgreSQL 拒绝，且不会留下半完成状态。
- 命令：`sed -n '579,635p' docs/architecture/content-workbench-v2-design-freeze.md`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须以未授权角色执行任一撤销动作，并验证事务整体回滚、无半完成状态。
- 预期/实际证据：同一事务成功与未授权负例失败。当前：PENDING。

### VAL-BLK-014 — quarantined 不能被非裁决路径复活

- 对应：BLK-014；环境：SOURCE_AUDIT → ISOLATED_DB；前置：状态转换所有权与数据库强制机制已获用户确认。
- 正例：获授权的裁决路径可把可见对象转为 quarantined。
- 反例：普通投影路径尝试把 quarantined 改回 visible 必须失败；该负例不能仅靠约定或代码审查成立。
- 命令：`sed -n '582,635p' docs/architecture/content-workbench-v2-design-freeze.md`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须以普通投影身份尝试 quarantined→visible，并以数据库拒绝为通过条件。
- 预期/实际证据：执行身份与状态转换负例的数据库输出。当前：PENDING。

### VAL-BLK-015 — EvidenceIngress、原始包读取与审计同源性

- 对应：BLK-015；环境：SOURCE_AUDIT → ISOLATED_DB。DEC-B1-007 已确认数据库强制。
- 前置为角色、表/列权限、受控读取函数及 EvidenceAccessAudit 的同 workspace CapturePackage 关系均已完整来源化。正例：evidence_writer 的合法入口写入 RawSnapshot/RawRecord 并产生入口回执；获授权的受控读取函数读取同 workspace CapturePackage 并写入关联真实包的审计记录。反例：default_app 直接 INSERT/UPDATE RawSnapshot 或 RawRecord、直接读取 CapturePackage 原始 payload、或写入伪造/跨 workspace 的 EvidenceAccessAudit，均必须被拒绝。命令：`sed -n '513,528p;598,632p;1046,1056p' docs/architecture/content-workbench-v2-design-freeze.md`；具备前置后执行 `v2_isolated_proof`。`$NEGATIVE_SQL_FILE` 必须覆盖上述三类越权或不同 workspace 负例；不得只验证直写 RawSnapshot/RawRecord 而遗漏原始包和审计完整性。预期/实际证据：受控入口/读取正例、三类 default_app 或跨 workspace 负例、入口回执与审计关联。
- 当前：PENDING，无实际证据。

### VAL-BLK-016 — Normalization 同源链不可跨快照错连

- 对应：BLK-016；环境：SOURCE_AUDIT → ISOLATED_DB；前置：DEC-B2-001 已关闭，Release-A 同源唯一键已存在，BLK-003 候选结构可创建。BLK-002 的终态改名、权限与历史迁移缺口继续独立保持 OPEN。
- 正例：同 workspace、同 raw snapshot、同 raw record 的 run/current 链可以建立。
- 反例：A snapshot 的 current 指向 B snapshot 的 record 或 run，必须被数据库拒绝。
- 命令：`sed -n '224,268p' docs/architecture/content-workbench-v2-design-freeze.md`；具备前置后执行 `v2_isolated_proof`。 `$NEGATIVE_SQL_FILE` 必须构造 A snapshot 指向 B record 与 A snapshot 指向 B run 两种错配，并以均被拒绝为通过条件。
- 预期/实际证据：同 workspace/snapshot/record 的七表链写入成功；跨 workspace 及 Run→Record、Current→Run、Current→Record、Observation→Record 错配均以 SQLSTATE `23503` 拒绝，评价关联与 Current 合同漂移亦被拒绝。六项证明各用独立数据库，证明库范围见 VAL-BLK-003；单独过滤最后一项 1/1 通过。当前：PASS（6/6）。

## 协作治理验证规则

> 六项规则共用唯一可执行证明脚本：`bash scripts/verify-v2-g0-governance.sh`。脚本按 GOV 编号逐项输出断言结果；其完整标准输出必须保存到对应的进度证据中。不得用手工摘录替代脚本输出。

### VAL-GOV-001 — 状态机有可达终态且不产生验证死锁

- 对应：GOV-001；环境：SOURCE_AUDIT；前置：无。
- 正例：状态定义含 `PROVED`、`CLOSED`，隔离验证的前提是相关 blocker `READY_FOR_PROOF`，不是全部 CLOSED。
- 反例：任一文件仍声明“不得 CLOSED”或“全部 CLOSED 前不得建隔离库”。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-001]` 段）。
- 预期/实际证据：命令输出显示唯一状态机与可达验证路径。当前：PASS，见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。

### VAL-GOV-002 — 项目入口不授权 B1

- 对应：GOV-002；环境：SOURCE_AUDIT；前置：无。
- 正例：TODO 下一步只允许协作包 blocker/设计证明工作，明确 B1 禁止。
- 反例：`docs/TODO.md` 的“下一步”区块存在第二条 B1 行动项，或唯一 B1 行动项不是明确的“禁止启动 / 不得启动 EvidenceIngress”。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-002]` 段；该段同时断言“下一步”仅有一条编号行动项）。
- 预期/实际证据：入口和协作契约一致。当前：PASS，见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。

### VAL-GOV-003 — 只有直接用户确认的事项可进入 DEC

- 对应：GOV-003；环境：SOURCE_AUDIT；前置：无。
- 正例：DEC 表只含有直接用户确认的单轨与历史数据处置规则；其他项均为 UND。
- 反例：代理提示、审核建议或“推荐方案”被登记为用户确认。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-003]` 段）。
- 预期/实际证据：DEC-001、DEC-002 之外无未经确认的技术选择。当前：PASS，见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。

### VAL-GOV-004 — 每个 blocker 有可证伪验收合同

- 对应：GOV-004；环境：SOURCE_AUDIT；前置：无。
- 正例：BLK-001 至 BLK-016 与 VAL-BLK-001 至 VAL-BLK-016 一一对应，均有前置条件、正例、反例、命令与实际证据状态。
- 反例：任一 blocker 只以“02-model-contract 打勾”或“写入设计文档”作为完成条件。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-004]` 段；逐项断言前置条件、正例、反例、命令、证据字段及 ledger 中唯一对应的验证规则）。
- 预期/实际证据：命令退出 0，且人工核对各项字段完整。当前：PASS，见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。

### VAL-GOV-005 — 文档现实快照与治理检查一致

- 对应：GOV-005；环境：SOURCE_AUDIT；前置：依赖已安装的项目依赖。
- 正例：`docs/project-audit.md` 的 Markdown 数量与治理脚本实时输出一致。
- 反例：脚本报告 `count drift`。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-005]` 段；该段要求治理脚本仅以固定点既有的大文件债务失败）。
- 预期/实际证据：不出现 `docs/project-audit.md count drift`；其他既有治理债务必须逐项如实记录。当前：PASS（仅既有大文件债务），见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。

### VAL-GOV-006 — B0 误读被保留并更正

- 对应：GOV-006；环境：SOURCE_AUDIT；前置：无。
- 正例：B0 历史报告不修改；ledger 明确记录 B0 对 RawSnapshot/RawRecord 的权限结论与 design-freeze 源文不一致。
- 反例：修改历史 B0 报告，或以 B0 误读作为权限结论。
- 命令：`bash scripts/verify-v2-g0-governance.sh`（读取其 `[GOV-006]` 段）。
- 预期/实际证据：相对 G0 基线的 B0 文件无 diff，ledger 有校正条目。当前：PASS，见 [GOV 只读证明](../../progress/v2-g0-governance-proof-2026-08-04.md)。
