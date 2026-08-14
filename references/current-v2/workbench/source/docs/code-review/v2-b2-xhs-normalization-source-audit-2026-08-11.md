# B2 XHS Normalization / ContractEvaluation 来源审计

> 日期：2026-08-11
> 工作台固定点：`07364d65f970aa819205506482a332bd0c95842b`
> 插件固定点：`227e365192d2141407599fefbf066aa66bfa0b26`
> 状态：`SOURCE_AUDIT_COMPLETE / DECISION_CONFIRMED / DARK_IMPLEMENTATION_PROVED`
> 范围：六份 XHS CollectionContract、七个 B2 模型与 B2-B-02 暗态服务；不包含 caller、迁移或流量切换。

## 1. 审核结论

六份插件 CapturePackage 与工作台合同镜像一致，且均能通过工作台 B1 validator；这只证明 **Evidence 输入合同** 可读，不证明 B2 的 Normalization 输出、Current 选择或 ContractEvaluation 裁决语义已经存在。

固定点没有 B2 writer，也没有从插件 `xhs-terminal-mapper.cjs` 调用工作台 Normalization 的运行链。因此，只有 27 个关系/来源字段和 14 个持久化层生成字段能从固定点直接确认；其余 34 个运行语义字段已由用户在 2026-08-11 确认 DR-B2-002 方案 A，并由 B2-B-02 暗态实现，不能从字段名、fixture 或唯一键另行反推。

| 状态 | 字段数 | 含义 |
|---|---:|---|
| `VERIFIED_DIRECT` | 27 | 从已接受 Evidence、同事务新建行或复合 FK 原样复制的身份/关系 |
| `DATABASE_GENERATED` | 14 | 工单规定的状态标签，准确含义是“持久化层生成”：5 个 Prisma Client CUID、7 个 PostgreSQL `now()` INSERT 默认、2 个 Current 时间戳的 PostgreSQL INSERT 默认 + Prisma Client UPDATE 维护；不存在数据库 trigger |
| `CONFIRMED_DECISION` | 34 | 已由 DEC-B2-002 方案 A 确认的 adapter、canonical、hash、重试、Current 与裁决运行语义 |
| `VERIFIED_DERIVED` | 0 | 固定点没有已确认的 B2 派生公式 |
| `NOT_APPLICABLE` | 0 | 七表字段均适用于至少一种首期记录 |

## 2. 六合同与真实生产链

### 2.1 已证明的共同链

1. 服务端 `collectionProfile` 进入插件：`taskLeaseClient.js::inferTaskTypeFromReservation`、`collectionProfile.js::resolveCollectionProfile`。
2. 任务转为插件内部 payload：`taskEnvelopeMapper.js::buildInternalPayload`。
3. XHS collector/controller 把结果按 `collectionRunId` 写入 `noteStore/commentStore/authorStore/mediaAssetStore`。
4. `resultPackager.js::packageByCollectionRunId` 从四个 store 重读真实行，原样形成 `terminal.records.notes/comments/authors/mediaAssets`；`resultSummaryBuilder.js::buildResultSummary` 只形成计数/摘要。
5. 现役 V1 是另一条消费者链：`taskPoller.js::buildWorkbenchResultSummary` 先 sanitizer，再由 `buildWorkbenchRecordDeltas` 构造 delta。它能解释 V1 writeback shape，但不是 V2 mapper 的 payload producer。
6. 暗态 V2 `xhs-terminal-mapper.cjs::mapTerminalToCaptureSubmissionV2` 直接读取 resultPackager 终态，`buildRecords` 深拷贝 store payload，并将 media 转为 `media_inventory` Artifact；固定点没有该 mapper 的运行 caller，也没有 B2 writer。

### 2.2 逐合同路径与记录边界

| 合同 | profile / 插件任务 | collector/handler 事实 | V2 recordKinds | 已确认边界 |
|---|---|---|---|---|
| `xhs.list-scan@1` | `list_scan` / `xhs.list_scan` | `BatchNoteController` surface/list → `noteStore` → resultPackager → V2 `buildRecords` | `note` | surface 行可能只有 identity/title/url；不得假定正文存在 |
| `xhs.note-detail@1` | `note_detail` / `xhs.note_full` | `BatchNoteController` → `collectNote`/`noteStore`；条件 comments → `collectComments`/`commentStore` → resultPackager → mapper | `note`,`comment` | 与 note-full 共用插件任务型，差异由 collectionProfile/合同表达 |
| `xhs.note-full@1` | `note_full` / `xhs.note_full` | `BatchNoteController` → `collectNote`/`noteStore` + `collectComments`/`commentStore` → resultPackager → mapper | `note`,`comment` | note/comments 均为 required slot；B1 可保存失败终态，B2 裁决由方案 A 定义 |
| `xhs.comment-probe@1` | `comment_probe` / `xhs.comment_scan` | collection handler / `BatchCommentController` → `collectComments`/`commentStore` → resultPackager → mapper | `comment` | V1 sanitizer 要求 id/noteId/text；V2 mapper保留 store 原文，由 B2 自行 fail closed |
| `xhs.author-profile@1` | `author_profile` / `xhs.author_profile` | collection handler → `collectAuthor`/`authorStore`；可选 surface notes→`noteStore` → resultPackager → mapper | `author`,`note` | author 必填，note_list 可选；store 事实常以 `platformAuthorId/userId` 表达 identity |
| `xhs.author-links@1` | `author_links` / `xhs.author_links` | author-note-links handler → profile surface discovery/`noteStore` → resultPackager → mapper | `note` | 只证明链接/列表 payload，不等同 note-detail 完整正文 |

### 2.3 稳定 JSON 路径

| recordKind | 固定点 identity 候选 | 稳定 payload 子集 | 首期合同适用 |
|---|---|---|---|
| `note` | `noteId → platformContentId → id`（V2 mapper）；V1 delta 另有 URL 末位候选 | store shape 随 surface/detail 不同；identity 最稳定，`title/content/url` 必须逐行做 fieldPresence，不是假定必有 | list-scan、note-detail、note-full、author-profile、author-links |
| `comment` | `commentId → id` | `commentId,noteId,text,author,authorId,likes,level,url` | note-detail、note-full、comment-probe |
| `author` | `authorId → platformAuthorId → userId → id` | `authorId,platformAuthorId,name,profileUrl`；生产 sanitizer 还保留头像、简介、地域、指标等字段 | author-profile |
| `media` | `assetId → sourceUrl → localPath` | V1 delta 可产生 media；V2 不产生 RawRecord，改为 `media_inventory` Artifact | metadata_only 合同可带 Artifact；not_required 也不得丢弃实际观察到的媒体 |

Fixture 是脱敏代表包，不是各 workflow store 行的完整快照；尤其 surface/link note 可以没有 detail 正文。它不能证明所有生产字段、null 规则、时间语义或 B2 canonical 输出。任一 fixture 通过 B1 validator 也不能把 B2 字段升级为 `VERIFIED_DERIVED`。

## 3. 七模型逐字段来源矩阵

适用性缩写：`ALL` 为六合同所有实际 RawRecord；`NOTE`、`COMMENT`、`AUTHOR` 对应上表合同集合。

矩阵最后一列保留固定点当时为什么无法从代码推出该字段；状态 `CONFIRMED_DECISION` 表示该缺口随后已由 DEC-B2-002 方案 A 确认，不再是待决项。

### 3.1 NormalizationRun（16/16）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `id` | ALL | DATABASE_GENERATED | Prisma Client `@default(cuid())`；migration 无列 DEFAULT |
| `workspaceId` | ALL | VERIFIED_DIRECT | `RawRecord.workspaceId`；三列 FK 约束同源 |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | `RawRecord.rawSnapshotId` |
| `rawRecordId` | ALL | VERIFIED_DIRECT | 被处理的 `RawRecord.id` |
| `adapterId` | ALL | CONFIRMED_DECISION | 固定点无 B2 adapter registry/命名所有者 |
| `adapterVersion` | ALL | CONFIRMED_DECISION | 固定点无版本常量与升级规则 |
| `canonicalSchemaVersion` | ALL | CONFIRMED_DECISION | 固定点无 canonical schema registry |
| `attemptNumber` | ALL | CONFIRMED_DECISION | unique 只限制重复，不能推出分配、replay 或显式重跑规则 |
| `status` | ALL | CONFIRMED_DECISION | 值域注释存在，进入各终态的算法不存在 |
| `inputPayloadHash` | ALL | CONFIRMED_DECISION | 未确认 hash 输入是 JSON 值、包内字节还是重序列化字节 |
| `outputPayloadHash` | ALL | CONFIRMED_DECISION | canonical output 尚未定义 |
| `missingFields` | ALL | CONFIRMED_DECISION | 必填字段集合、排序和 JSON shape 未定义 |
| `parseErrors` | ALL | CONFIRMED_DECISION | 错误码、路径、排序和可重试语义未定义 |
| `startedAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |
| `completedAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP`；是否由 writer 显式覆盖不影响当前物理来源 |
| `createdAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |

### 3.2 NormalizationRunCurrent（8/8）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `workspaceId` | ALL | VERIFIED_DIRECT | 被选择 run 的 workspace |
| `rawRecordId` | ALL | VERIFIED_DIRECT | 被选择 run 的 record |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | 被选择 run 的 snapshot |
| `normalizationRunId` | ALL | VERIFIED_DIRECT | FK 指向实际 run；“选择哪个”仍属未决规则 |
| `adapterId` | ALL | CONFIRMED_DECISION | 是否及何时从被选择 run 复制未冻结 |
| `adapterVersion` | ALL | CONFIRMED_DECISION | 同上 |
| `canonicalSchemaVersion` | ALL | CONFIRMED_DECISION | 同上 |
| `updatedAt` | ALL | DATABASE_GENERATED | Prisma Client 在 UPDATE/UPSERT 时维护 `@updatedAt`；不是 PostgreSQL trigger |

### 3.3 CanonicalObservation（14/14）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `id` | ALL | DATABASE_GENERATED | Prisma Client `@default(cuid())`；migration 无列 DEFAULT |
| `workspaceId` | ALL | VERIFIED_DIRECT | NormalizationRun.workspaceId |
| `normalizationRunId` | ALL | VERIFIED_DIRECT | 创建 observation 的 run |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | NormalizationRun.rawSnapshotId |
| `rawRecordId` | ALL | VERIFIED_DIRECT | NormalizationRun.rawRecordId |
| `observationKind` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | RawRecord.recordKind 不能未经决策直接成为 canonical 值域 |
| `subjectKey` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | identity 候选存在，但稳定 key 格式与缺失处理未确认 |
| `observedAt` | ALL | CONFIRMED_DECISION | RawRecord/header/collector 时间均存在，优先级未确认 |
| `schemaVersion` | ALL | CONFIRMED_DECISION | 与 canonicalSchemaVersion 的一致性规则未确认 |
| `payload` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | canonical DTO、保真边界和 null 规则未定义 |
| `payloadHash` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | payload 未定义，hash 字节也未定义 |
| `qualityStatus` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | 值域及 missing/parse 对质量的映射未定义 |
| `fieldPresence` | NOTE/COMMENT/AUTHOR | CONFIRMED_DECISION | JSON shape、字段集合与排序未定义 |
| `createdAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |

`CanonicalObservation` 虽在后续由 B3 消费，但由 B2 事务创建；其运行合同不能推迟到 B3 后再补。

### 3.4 ContractEvaluation（13/13）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `id` | ALL | DATABASE_GENERATED | Prisma Client `@default(cuid())`；migration 无列 DEFAULT |
| `workspaceId` | ALL | VERIFIED_DIRECT | RawSnapshot.workspaceId |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | 被评估的 RawSnapshot.id |
| `contractId` | ALL | VERIFIED_DIRECT | RawSnapshot.contractId |
| `contractVersion` | ALL | VERIFIED_DIRECT | RawSnapshot.contractVersion |
| `evaluatorVersion` | ALL | CONFIRMED_DECISION | evaluator 所有者与升级规则未定义 |
| `canonicalSchemaVersion` | ALL | CONFIRMED_DECISION | 与输入 runs/observations 的一致性规则未定义 |
| `evaluationInputHash` | ALL | CONFIRMED_DECISION | 输入集合、顺序和 canonical bytes 未定义 |
| `decision` | ALL | CONFIRMED_DECISION | 何时 accepted/rejected 未定义 |
| `completeness` | ALL | CONFIRMED_DECISION | full/partial/not_applicable 判定未定义 |
| `rejectionCode` | ALL | CONFIRMED_DECISION | 错误码集合与优先级未定义 |
| `rejectionReason` | ALL | CONFIRMED_DECISION | 稳定说明格式未定义 |
| `createdAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |

### 3.5 ContractEvaluationCurrent（7/7）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `workspaceId` | ALL | VERIFIED_DIRECT | ContractEvaluation.workspaceId |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | ContractEvaluation.rawSnapshotId |
| `contractId` | ALL | VERIFIED_DIRECT | ContractEvaluation.contractId |
| `contractVersion` | ALL | VERIFIED_DIRECT | ContractEvaluation.contractVersion |
| `contractEvaluationId` | ALL | CONFIRMED_DECISION | Current 替换条件尚未确认 |
| `revision` | ALL | CONFIRMED_DECISION | 初值、递增与 replay 规则尚未确认 |
| `updatedAt` | ALL | DATABASE_GENERATED | Prisma Client 在 UPDATE/UPSERT 时维护 `@updatedAt`；不是 PostgreSQL trigger |

### 3.6 ContractEvaluationNormalizationRun（9/9）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `id` | ALL | DATABASE_GENERATED | Prisma Client `@default(cuid())`；migration 无列 DEFAULT |
| `evaluationId` | ALL | VERIFIED_DIRECT | 同事务 ContractEvaluation.id |
| `normalizationRunId` | ALL | VERIFIED_DIRECT | 实际参与评估的 run.id |
| `workspaceId` | ALL | VERIFIED_DIRECT | evaluation/run 同源 workspace |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | evaluation/run 同源 snapshot |
| `status` | ALL | CONFIRMED_DECISION | 是否必须精确复制 run 终态未确认 |
| `inputPayloadHash` | ALL | CONFIRMED_DECISION | 是否精确复制、重读比较还是重算未确认 |
| `outputPayloadHash` | ALL | CONFIRMED_DECISION | 同上；nullable 规则未确认 |
| `createdAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |

### 3.7 ContractEvaluationInput（8/8）

| 字段 | 适用 | 状态 | 当前唯一来源 / 缺口 |
|---|---|---|---|
| `id` | ALL | DATABASE_GENERATED | Prisma Client `@default(cuid())`；migration 无列 DEFAULT |
| `workspaceId` | ALL | VERIFIED_DIRECT | evaluation/observation 同源 workspace |
| `rawSnapshotId` | ALL | VERIFIED_DIRECT | evaluation/observation 同源 snapshot |
| `contractEvaluationId` | ALL | VERIFIED_DIRECT | 同事务 evaluation.id |
| `canonicalObservationId` | ALL | VERIFIED_DIRECT | 实际参与评估的 observation.id |
| `ordinal` | ALL | CONFIRMED_DECISION | 输入排序及重复处理未确认 |
| `canonicalOutputHash` | ALL | CONFIRMED_DECISION | 是否复制 observation.payloadHash 及重读校验未确认 |
| `createdAt` | ALL | DATABASE_GENERATED | PostgreSQL `DEFAULT CURRENT_TIMESTAMP` |

## 4. DR-B2-002 最小决策包

未决项不能缩成四个字符串；它们属于同一套可观察行为，应一次确认，避免 adapter、evaluator 与 Current 各自发明规则。

### 方案 A（推荐）：版本化、无损、确定性首期合同

1. **版本所有者**：adapterId 分为 `xhs.note`、`xhs.comment`、`xhs.author`；adapterVersion 与 evaluatorVersion 使用各自源码常量的 semver，首版 `1.0.0`；canonicalSchemaVersion 使用 `xhs.canonical/1`。任何改变 canonical payload、必填字段、hash 输入、裁决或排序结果的代码变更必须显式升对应版本。
2. **首期 canonical payload**：不伪造跨平台字段。payload 固定为 `{ platform, recordKind, subjectKey, observedAt, sourcePayload }`，其中 `sourcePayload` 是 RawRecord.payload 的 JSON 深快照；这是一层可版本化的技术 Canonical，不是 B3 领域 Projection。
3. **identity**：每个 segment 先取 exact trimmed string，再用 JavaScript `encodeURIComponent` 百分号编码，不改大小写。note=`xhs:note:<noteId|platformContentId>`；comment=`xhs:comment:<noteId>:<commentId>`，必须同时存在 noteId 与 commentId，不能假定 commentId 跨笔记全局唯一；author=`xhs:author:<authorId|platformAuthorId|userId>`。按已列优先级取第一个非空候选；缺失则 run=`rejected`，不创建 observation。
4. **时间与质量**：observedAt 精确复制 RawRecord.observedAt。fieldPresence 的固定 key 为 note=`noteIdentity,title,content,url`，comment=`noteId,commentId,text`，author=`authorIdentity,name,profileUrl`；`noteIdentity`/`authorIdentity` 按各自候选组“任一存在”计算，其余逐字段计算。对象按 key 排序；全部为 true 时 `complete`，identity 有效但非身份字段缺失时 `partial`。missingFields 使用同一组固定 key。首期不凭推测补值。
5. **hash**：统一使用 B1 `canonicalJson` 的 UTF-8 字节与 lowercase SHA-256。input=RawRecord.payload；output=完整 canonical payload；evaluationInput 固定包含 Evidence eligibility/integrity、合同 identity/version、terminal state/reason/retryable、按 slotId 排序的 slots、evaluator/schema version，以及按 `(RawRecord.sequence nulls-last, RawRecord.id)` 排序的 evaluation member 数组。member 完整 shape 见第 8 条。写入前后重读值必须一致，任何能改变裁决的输入都必须改变 evaluationInputHash。
6. **run 终态与诊断**：`recordKind` 不在 note/comment/author、payload 不是 JSON object 或 identity 缺失时为 `rejected`，不创建 observation；canonical/hash 重读不一致或同源不变量失败时为 `quarantined`；其余为 `normalized`。`missingFields` 是按字典序排列的字段名数组；`parseErrors` 是按 `(path,code)` 排序的 `{path,code}` 数组，首期 code 只允许 `record_kind_unsupported`、`payload_not_object`、`identity_missing`、`canonicalization_failed`、`hash_mismatch`、`source_binding_mismatch`。空诊断写 null，不写空数组；outputPayloadHash 仅 normalized 非空。
7. **attempt/Current**：相同 record+adapter+version+schema+inputHash 的 terminal run 重放返回既有结果；只有显式 retry 才在同一 SERIALIZABLE 事务使用 `max(attemptNumber)+1`。仅 `normalized` run 推进 NormalizationRunCurrent；rejected/quarantined 保留但不替换已存在 Current。Current 的三版本字段必须精确复制目标 run。
8. **唯一评估成员**：每个 RawRecord 恰有一个 member，按以下优先级且只取一项：(a) Current 精确匹配 expected adapterId/version/schema → `current`，即使之后有失败 retry 也继续使用该 accepted Current；(b) 存在 Current 但三版本不匹配 → `current_version_mismatch`，取该 Current run，不回捞历史 expected normalized run；(c) 无 Current，取 expected 三版本下 attemptNumber 最大的 terminal run，按其状态分别为 `latest_rejected_attempt`、`latest_quarantined_attempt` 或 `normalized_current_missing`；(d) 完全没有 expected run → `missing`。唯一键保证同 attempt 唯一。descriptor 固定为 `{recordId,memberKind,runId,attemptNumber,status,adapterId,adapterVersion,canonicalSchemaVersion,inputHash,outputHash}`；missing 除 recordId/memberKind 外全部为 null。CENR 只为 descriptor 中非 null 的 runId 写一行并精确复制值；missing 不伪造 run 或 CENR。
9. **裁决**：slot→recordKind 固定为 `note_list/note/note_links→note`、`comments→comment`、`author→author`。受控 Evidence reader 必须先明确给出可分析 lifecycle 且 integrity=`verified`；不合格时在读取 CapturePackage 前返回 service-level `evidence_ineligible`，七表零写入。通过后，只有 terminal=`completed`、全部 memberKind=`current` 且 status=`normalized`、每个 required slot=`observed` 并至少有一个对应 recordKind 时才 accepted。required 满足后，optional/conditional 的 `observed` 或 `not_applicable` 视为满足；全部声明 slot 满足为 `accepted/full`，其余 optional/conditional 缺口为 `accepted/partial`。其它为 `rejected/not_applicable`。固定点尚无 lifecycle 物理字段，因此暗态服务必须依赖 fail-closed 的 eligibility capability；BLK-001 未提供该能力前不得接 caller。
10. **裁决错误码与优先级**：进入 evaluation 后按 `terminal_not_completed`、`required_slot_not_observed`、`required_slot_without_record`、`normalization_rejected`、`normalization_quarantined`、`normalization_current_missing`、`schema_version_mismatch`、`evaluation_input_mismatch` 的顺序选择第一个；`latest_rejected_attempt`/`latest_quarantined_attempt` 分别映射对应错误，`missing`/`normalized_current_missing` 映射 current_missing，`current_version_mismatch` 映射 schema mismatch。accepted 时 code/reason 必须为 null，rejected reason 为 `canonicalJson({code,details})`，details 只含排序后的稳定对象 identity，不写自由文本堆栈。
11. **evaluation Current**：相同 evaluation identity/hash 重放不增 revision；新的 terminal evaluation（accepted 或 rejected）都以事务 CAS 替换 Current，首次 revision=`1`，后续 `revision+1`。rejected 必须能替换旧 accepted，供 B3 撤销旧投影。CEI 只为 accepted 写入，按 member 稳定顺序写 ordinal，并精确复制 observation.payloadHash。
12. **事务与边界**：遵守冻结稿单事务：读取审计成功后，Run/Observation/Evaluation/关联/两个 Current 在一个事务完成；任何失败零 Derived 残留。首期只暗态调用，不接 route/scheduler/outbox，不双写、不 fallback。

### evaluationInput 精确 DTO 与测试向量

外层对象必须且只能含 `snapshot`、`eligibility`、`contract`、`terminal`、`slots`、`evaluator`、`members`。所有 key 必须存在；nullable 值显式写 `null`，禁止省略。slots 按 `slotId` 排序；members 按 `(RawRecord.sequence nulls-last, RawRecord.id)` 排序。对象交给 B1 `canonicalJson` 排 key，数组不得二次重排。

```json
{"contract":{"id":"xhs.list-scan","version":1},"eligibility":{"integrityStatus":"verified","lifecycleStatus":"ACTIVE"},"evaluator":{"canonicalSchemaVersion":"xhs.canonical/1","version":"1.0.0"},"members":[{"adapterId":"xhs.note","adapterVersion":"1.0.0","attemptNumber":1,"canonicalSchemaVersion":"xhs.canonical/1","inputHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","memberKind":"current","outputHash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","recordId":"rr-1","runId":"nr-1","status":"normalized"}],"slots":[{"reason":null,"slotId":"note_list","status":"observed"}],"snapshot":{"rawSnapshotId":"rs-1","workspaceId":"ws-fixture"},"terminal":{"reason":"limit_reached","retryable":false,"state":"completed"}}
```

上述 UTF-8 bytes 的 lowercase SHA-256 必须为：

```text
3074e075177840ea9c27d356c6225fc2ab319350901372727a94e65405fc3fa4
```

任一 terminal、slot、member、version、eligibility 或排序变化必须改变 hash；只改变对象构造时的 key 插入顺序不得改变 hash。

### 方案 B：只保存 run，不产生 Canonical 或 Evaluation

只能证明 RawRecord 被读取，不能推进 B3，也不能验证六合同；相当于推迟全部产品价值，不推荐。

### 不确认方案 A 的影响

B2-B-02 继续阻塞。任何服务实现都会自行决定 identity、payload、hash、重试、Current 和裁决行为，后续已有数据将无法可靠重算或比较。

## 5. 可证伪验收

- 六份插件 fixture 必须直接进入同一 adapter/evaluator 测试；六合同全部给出 deterministic 输出 hash。
- 任意篡改 payload、identity、observedAt、sequence、adapter/evaluator/schema version 或跨 workspace/snapshot 关系，必须在数据库写入前或约束层失败。
- 两篇 note 使用相同 commentId 必须产生不同 comment subjectKey；缺 noteId 的 comment 必须 rejected。
- 同输入 replay 不新增 run/evaluation/revision；显式 retry 新增 attempt；失败事务七表零残留。
- 同一 record 有多个失败 attempt 时只选择最大 attempt；交换/新增更大 attempt 必须改变 member、evaluationInputHash 与 CENR，历史失败 run 不得重复关联。
- 历史 expected normalized run 存在但 Current 已指向另一版本时必须选择 `current_version_mismatch`；无 Current 的孤立 normalized run 必须选择 `normalized_current_missing`，两者都不得被误当 accepted。
- rejected/quarantined run 不替换已接受 Normalization Current；新的 accepted 或 rejected terminal evaluation 均可 CAS 推进 Evaluation Current，确保旧 accepted 可被撤销。
- B2 仍无现役 caller、无 V1/V2 双写、无正式数据库写入。

## 6. 已复现门禁

工作目录：工作台 `/Users/gongyong/Services/content-workbench/v2-b2-derived-readiness`；插件 `/Users/gongyong/Services/linggan-boom`。

| 命令 | 结果 |
|---|---|
| `npx tsx src/lib/evidence/contracts/cross-repo-verify.ts /Users/gongyong/Services/linggan-boom` | 6/6 accepted |
| `npx vitest run src/lib/evidence/contracts/xhs-collection-contracts.test.ts` | 63/63 passed |
| `npm --prefix /Users/gongyong/Services/linggan-boom run check:contracts` | 161/161 passed |
| `npx vitest run ...xhs-derived-contract.test.ts ...xhs-contract-evaluation.test.ts ...b2-derived-service.test.ts` | 18/18 passed |
| `V2_B2_SERVICE_INTEGRATION_DB=1 npx vitest run ...b2-derived-service.integration.test.ts` | 9/9 passed；保留证明库 `content_workbench_v2_b2_service_1786417135276` |
| `npx tsc --noEmit --incremental false` | exit 0 |
| `npm test -- --no-file-parallelism` | 666 files / 4,581 passed；3 files / 41 tests 按 opt-in 设计 skipped |
| `npx eslint src/lib/evidence/derived src/lib/evidence/contracts/cross-repo-verify.ts` | exit 0，0 warning |
| `npx prisma validate` | exit 0 |
| 本地占位 auth secret + `npm run build` | exit 0；Prisma generate、Next production build 与 trace/fingerprint guard 通过 |
| `node scripts/check-project-governance.mjs --json` | 仅固定点既有 TODO 行数阈值与 `execution-sync-service.ts` 体量债务；本轮 TODO 行数不高于固定点 |
| `git diff --check`（覆盖 tracked；untracked 由 type/lint/test 直接读取） | exit 0 |

本文件不把 B1 合同测试写成 B2 运行证明；方案 A 的运行证明来自 B2 定向测试、六份插件 fixture 的独立输出 hash 锁与全新隔离数据库。受控 reader 的测试证明访问审计在 writer 事务失败后仍独立留存；writer 查询不包含 CapturePackage。
