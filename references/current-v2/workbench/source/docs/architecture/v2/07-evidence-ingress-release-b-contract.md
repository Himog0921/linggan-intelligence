# 07 — EvidenceIngress Release-B 实施合同

> 状态：已确认。用户于 2026-08-05 确认按审核主线推荐方案收口 DR-B1-002～005。
>
> 本文件只固定 Release-B 实施所需的精确协议与事务边界。若与历史设计稿或当前 V1 代码冲突，以本文件、`00-contract.md`、`01-decisions.md` 和 `content-workbench-v2-design-freeze.md` 为准。

## 1. 实施与切流边界

Release-B 可以分多个可审查的暗态代码提交，但生产运行流量只能在最终切换点一次性切换。最终切换前：

- 新 EvidenceIngress 不得被任何现役入口调用；
- execution、manual_import、recovery 继续只走原有 V1；
- migration 没有现役入口；
- 不得双写、双读、fallback 或在 V2 失败后调用 V1；
- 不得声称九工位已具备 V2，实际版本与探针仍按 UND-001 单独核验。

第一张暗态代码工单只允许建设协议、校验器、EvidenceIngress 核心与 Release-B schema prepare，不切调用方。四类调用方与真实合同注册完成后，才允许提交最终原子切流改动。

## 2. 唯一协议语言

### 2.1 JSON 值

```ts
type JsonPrimitive = string | number | boolean | null;
type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
```

所有数字必须是有限 JSON number；拒绝 `undefined`、`NaN`、`Infinity`、`BigInt`、函数、循环引用和非普通对象。

### 2.2 Canonical JSON

`canonicalJson(value)` 的唯一规则：

1. 普通对象的 key 使用 JavaScript `Object.keys(value).sort()` 顺序递归排序；
2. 数组保留原顺序；
3. 使用无空格的 `JSON.stringify`；
4. 输出 UTF-8 bytes；
5. package bytes 必须与解析后重新生成的 canonical bytes 完全一致，否则以 `package_not_canonical` 拒绝；
6. package checksum 与 RawRecord `payloadHash` 均使用小写 64 位十六进制 `sha256`。

### 2.3 首期平台

首期唯一合法平台是：

```ts
type EvidencePlatformV2 = "xhs";
```

Douyin 返回 `unsupported_platform`，不得进入 V2 首期，也不得 fallback。

## 3. CaptureSubmissionV2 精确合同

### 3.1 来源可控 Header

```ts
type CaptureTargetV2 = {
  expectedTargetKey: string;
  observedTargetKey: string | null;
};

type CaptureTerminalV2 = {
  state: "completed" | "blocked" | "cancelled" | "error";
  reason:
    | "source_exhausted"
    | "limit_reached"
    | "target_missing"
    | "login_required"
    | "platform_blocked"
    | "parser_failed"
    | "network_failed"
    | "user_cancelled";
  retryable: boolean;
};

type CaptureSlotV2 = {
  slotId: string;
  status: "observed" | "absent" | "unavailable" | "not_applicable" | "invalid";
  reason: string | null;
};

type CaptureCountersV2 = {
  requested: number;
  discovered: number;
  emitted: number;
  deduplicated: number;
  failed: number;
};

type CaptureReportV2 = {
  startedAt: string;
  completedAt: string;
  terminal: CaptureTerminalV2;
  slots: CaptureSlotV2[];
  counters: CaptureCountersV2;
  diagnostics: { [key: string]: JsonValue };
};

type CaptureHeaderCommonV2 = {
  protocolVersion: "capture-submission/v2";
  captureId: string;
  platform: "xhs";
  target: CaptureTargetV2;
  observedAt: string;
  collectorVersion: string;
  contractId: string;
  contractVersion: number;
  contractHash: string;
  report: CaptureReportV2;
};

type CaptureHeaderV2 = CaptureHeaderCommonV2 & (
  | {
      ingressKind: "execution";
      jobId: string;
      attemptId: string;
      leaseEpoch: number;
      executionPlanVersion: string;
    }
  | {
      ingressKind: "manual_import";
      sourceSummary: string;
    }
  | {
      ingressKind: "recovery";
      recoveryCaptureId: string;
    }
  | {
      ingressKind: "migration";
      sourceSummary: string;
    }
);
```

Header、Record、Artifact 的标识符和非 null reason 必须 trim 后非空；原始 `payload` 内的 JSON string 可以为空。全部时间必须是带时区的 ISO 8601 字符串并能解析为有效时间。`contractVersion` 必须是正整数，counter 必须是非负整数，`slotId` 在同一包内不得重复。

`expectedTargetKey` 必填且必须以 `xhs:` 开头。调用适配器必须先把平台目标规范化为正式 targetKey；EvidenceIngress 不做 URL、短链或别名推测。`observedTargetKey` 可为 null；非 null 时 trim 后必须与 `expectedTargetKey` 完全相同，否则以 `target_identity_mismatch` 拒绝且不创建 Evidence。

### 3.2 RawRecord 与 Artifact

```ts
type RawRecordSubmissionV2 = {
  idempotencyKey: string;
  recordKind: "note" | "comment" | "author" | "metric";
  platform: "xhs";
  targetKey: string | null;
  externalRecordId: string | null;
  sequence: number;
  payload: JsonValue;
  observedAt: string;
};

type CaptureArtifactSubmissionV2 = {
  kind:
    | "platform_response"
    | "page_snapshot"
    | "dom_fragment"
    | "media_inventory"
    | "context";
  encoding: "base64";
  artifactPayload: string;
  artifactChecksum: string;
  contentLength: number;
  restricted: boolean;
};
```

`sequence` 必须是非负整数且在同一包内唯一；`idempotencyKey` 必须非空且在同一包内唯一。每条 Record 的 platform 必须等于 Header platform；非 null targetKey 必须以 `xhs:` 开头。客户端不得提交 `payloadHash`，EvidenceIngress 只按 `canonicalJson(payload)` 自行计算。

Artifact 的 `artifactPayload` 使用与 package 相同的严格 padded base64 规则；`contentLength` 和 `artifactChecksum` 必须由解码后的 Artifact 原始 bytes 重算一致。Artifact 原始 bytes 只存在于 CapturePackage.packagePayload 内，CaptureArtifact 表只登记 kind/checksum/restricted 元数据，不建立第二 payload 存储。

### 3.3 解码后的唯一 CapturePackage payload

```ts
type CapturePackagePayloadV2 = {
  schemaVersion: "capture-package/v2";
  header: CaptureHeaderV2;
  records: RawRecordSubmissionV2[];
  artifacts: CaptureArtifactSubmissionV2[];
};

type CapturePackageSubmissionV2 = {
  encoding: "base64";
  packagePayload: string;
  checksumAlgorithm: "sha256";
  checksumValue: string;
  contentLength: number;
  restricted: boolean;
};

type CaptureSubmissionBodyV2 = {
  header: CaptureHeaderV2;
  capturePackage: CapturePackageSubmissionV2;
};
```

`packagePayload` 使用 RFC 4648 标准 padded base64，不接受 base64url、空白、缺失必要 padding 或宽松解码；解码后重新 `toString("base64")` 必须与输入完全相同。解码后的 bytes 必须是 `CapturePackagePayloadV2` 的 canonical JSON。外层 `header` 必须与包内 `header` canonical deep-equal。

所有对象使用 strict shape，未知 key 一律以 `invalid_submission` 拒绝。`contentLength` 是非负整数且等于解码后的 byte length；`checksumValue` 与 `artifactChecksum` 必须是小写 64 位十六进制，package checksum 等于原始 bytes 的 sha256。若任一 Artifact 为 restricted，则 package `restricted` 必须为 true；没有 restricted Artifact 时 package 仍可由调用方明确标为 restricted。

records 可以为空，用于合法的 blocked/cancelled/error 终态；当 terminal.state=`completed` 且 counters.emitted>0 时，records 不得为空。`counters.emitted` 必须等于 records.length。

### 3.4 服务端权威上下文

外部 body 不得提交或覆盖 `workspaceId`、`receivedAt`、`sourcePrincipal` 及下列权威字段。API/调用适配器必须从已验证身份构造：

```ts
type EvidenceIngressAuthorityV2 =
  | {
      ingressKind: "execution";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      stationId: string;
      leaseToken: string;
    }
  | {
      ingressKind: "manual_import";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      importerIdentity: string;
    }
  | {
      ingressKind: "recovery";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      recoveryAuthorizedBy: string;
    }
  | {
      ingressKind: "migration";
      workspaceId: string;
      receivedAt: Date;
      sourcePrincipal: string;
      migrationAuthorization: string;
    };

type CaptureSubmissionV2 = {
  body: CaptureSubmissionBodyV2;
  authority: EvidenceIngressAuthorityV2;
};
```

body.header.ingressKind 必须与 authority.ingressKind 相同。非 execution 的 `jobId`、`attemptId`、`stationId` 永远不会进入其类型或持久化列。

权威绑定：

- execution：workspace/sourcePrincipal/stationId 来自签名工位会话；leaseToken 来自签名 operation，只用于当前租约校验与之后的控制平面 CAS，不进入 CapturePackage、RawSnapshot、日志或返回体；
- manual_import：workspace/sourcePrincipal/importerIdentity 来自当前插件授权与用户会话；
- recovery：workspace/sourcePrincipal/recoveryAuthorizedBy 来自 owner/admin 会话；
- migration：没有网络入口，全部四项来自未来获授权的内部调用者；
- receivedAt 只使用服务端时钟。

DEC-B1-022 固定首批非 execution adapter 的具体绑定：

- 用户 principal 统一为 `user:<userId>`；不使用可变 email、显示名或 role 作为身份；
- manual_import 的 `importerIdentity` 为 `plugin-authorization:<PluginAuthorization.id>`，不使用仅在授权码范围内唯一的 deviceId；`PluginAuthorization.workspaceId` 必须非空并与已验证用户会话 workspace 相等；
- recovery 仅接受已验证 owner/admin 会话，`sourcePrincipal` 与 `recoveryAuthorizedBy` 均指向该会话的 `user:<userId>`；
- adapter 在内部调用服务端时钟生成 `receivedAt`，拒绝 body/header authority 与 execution identity，并返回与该次请求精确绑定的 validator；
- 本阶段只有暗态 adapter，不接现役 route、不改变 V1 流量；migration 仍无 caller。

DEC-B1-023 固定 execution adapter 的具体绑定：

- `sourcePrincipal = execution-station:<stationId>`；stationId 只能来自同时通过 stationToken 与请求 HMAC 的严格 V2 工位会话；verifier 自行读取当前 PluginAuthorization 行并验证 token、active 状态与 expiry；
- 严格会话必须证明 PluginAuthorization 绑定该 ExecutionStation，且 Station/Authorization workspace 都非空并相等；验签结果是模块私有注册的能力对象并绑定原始请求 method/path/body sha256，adapter 拒绝同形伪造对象和另一份 body；adapter 再证明 Job/Queue workspace 与会话相等，不使用 default workspace fallback；
- `ExecutionJob.executionPlanVersion` 是服务端不透明版本，同一 job 的所有尝试与重试复用；计划变化必须新建 job，`ExecutionPlannerSnapshot` 不参与该来源；历史 null 版本 job 不得提交 V2 execution Evidence；
- adapter 在 SERIALIZABLE 只读事务中验证 current job/attempt/queue lease，并返回会在 CapturePackage 解码前重新读取同一事实的 authority validator；租约失效或绑定改变均拒绝且不写 Evidence；
- 本阶段只有 schema expand、暗态 verifier/adapter 和隔离库证明，不接现役 route、不升级九工位、不改变 V1 流量。

暗态核心通过构造函数注入 `EvidenceIngressAuthorityValidator`，不内置假生产授权。execution、manual_import 与 recovery 已有生产形状的暗态 validator，但尚无现役 caller；migration 仍只使用测试 validator，直到另获具体内部授权。

## 4. CollectionContract 服务端真值

Release-B 使用只追加的代码注册表，不新建 Contract 数据库表，不信任客户端自报 hash。

```ts
type CollectionContractDefinitionV2 = {
  id: string;
  version: number;
  platforms: "xhs"[];
  recordKinds: Array<"note" | "comment" | "author" | "metric">;
  slots: Array<{
    slotId: string;
    requirement: "required" | "optional" | "conditional";
  }>;
  terminalPolicy: {
    allowedStates: Array<"completed" | "blocked" | "cancelled" | "error">;
    allowEmptyRecords: boolean;
  };
  mediaPolicy: "not_required" | "metadata_only" | "source_required";
};
```

注册表位置固定为：

```text
src/lib/evidence/contracts/collection-contract-registry.ts
```

contract hash 固定为：

```text
sha256(UTF-8(canonicalJson(CollectionContractDefinitionV2)))
```

`contractHash` 必须是小写 64 位十六进制。注册表按 `(id,version)` 唯一解析；unknown id/version 返回 `contract_not_registered`，hash 不同返回 `contract_hash_mismatch`，两者都不创建任何 Evidence。

注册表加载时必须拒绝重复 id/version、重复 slotId、重复 recordKind、非正 version 与非 XHS 平台。B1 解析成功后还要验证：每条 Record.kind 在 definition.recordKinds 内、每个上报 slotId 在 definition.slots 内、terminal.state 在 allowedStates 内，空 records 符合 allowEmptyRecords；否则返回 `contract_shape_mismatch`。

暗态核心允许通过构造函数注入测试 registry，但不得内置假生产合同。最终切流前，所有现役 XHS Workflow 使用的真实合同定义与入口映射必须在独立工单中注册并通过 fixture 验证；不得从 `jobType`、`profile` 或 requiredFields 猜映射。

## 5. EvidenceIngress 事务与结果

### 5.1 唯一职责

EvidenceIngress 只负责：协议校验、权威绑定、合同校验、包完整性、不可变 Evidence 持久化、replay/conflict。它不推进 ExecutionJob/QueueEntry/TaskAttempt/Runtime，不写旧 `raw_snapshot.committed` Outbox，不创建 Normalization、ContractEvaluation、Projection 或 Media。

### 5.2 单事务写入

单个 PostgreSQL SERIALIZABLE 事务原子写入：

```text
CapturePackage
RawSnapshot
RawRecord[]
CaptureArtifact[]
EvidenceIngressReceipt
```

V2 RawSnapshot：

- `targetKey = header.target.expectedTargetKey`；
- `expectedTargetKey` 与 `observedTargetKey` 按 header 原值登记；
- `checksumAlgorithm/checksumValue/contentLength` 与 CapturePackage 一致；
- `payloadClob/storageKey/source/schemaVersion/pluginVersion/qualityStatus/qualityReason` 不作为 V2 事实，写 null；
- execution 写真实 jobId/attemptId/stationId/leaseEpoch/executionPlanVersion；
- 其他三类 execution 字段全部写 null；
- 所有公共、专属、contract 与 authority 字段逐列登记；
- 新 identity 写 `integrityStatus=verified`、`integrityReason=null`；不同 hash 写 `integrityStatus=capture_identity_conflict`、`integrityReason=verified_variant_hash_mismatch`；
- EvidenceIngressReceipt 使用同一 workspace/snapshot、header captureId/ingressKind/collectorVersion 与服务端 receivedAt。

V2 RawRecord：

- `recordType/jobId/collectedAt/dedupeKey` 写 null；
- `recordKind` 来自包；
- `payloadHash` 由服务端计算；
- idempotency 只在所属 snapshot 内有效，不得跨 snapshot 合并或跳过。

Release-B schema prepare 必须把上述 legacy 字段放宽为可空，并把 RawRecord idempotency 唯一键改为 `(workspaceId,rawSnapshotId,idempotencyKey)`；删除旧的 `(workspaceId,idempotencyKey)` 跨快照唯一语义。RawRecord 必须以 `(workspaceId,rawSnapshotId) → RawSnapshot(workspaceId,id)` 复合 FK 拒绝跨 workspace 关联。

全表 V2 NOT NULL、RawSnapshot/RawRecord append-only trigger、default_app 撤权只在最终切流且历史退役清零后生效；暗态核心工单不得提前让现役 V1 写入失效。

### 5.3 返回合同

```ts
type EvidenceIngressResult =
  | {
      status: "committed";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "verified";
      checksumValue: string;
    }
  | {
      status: "conflict";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "capture_identity_conflict";
      checksumValue: string;
    }
  | {
      status: "replay";
      rawSnapshotId: string;
      receiptId: string;
      integrityStatus: "verified" | "capture_identity_conflict";
      checksumValue: string;
    }
  | {
      status: "rejected";
      reason:
        | "invalid_submission"
        | "unsupported_platform"
        | "invalid_base64"
        | "package_not_canonical"
        | "package_checksum_mismatch"
        | "package_length_mismatch"
        | "invalid_artifact_base64"
        | "artifact_checksum_mismatch"
        | "artifact_length_mismatch"
        | "header_package_mismatch"
        | "target_identity_mismatch"
        | "contract_not_registered"
        | "contract_hash_mismatch"
        | "contract_shape_mismatch"
        | "execution_authority_invalid"
        | "manual_import_authority_invalid"
        | "recovery_authority_invalid"
        | "migration_authority_invalid"
        | "concurrency_exhausted";
      retryable: boolean;
    };
```

validation/authority/contract/package 错误均 `retryable=false`；`concurrency_exhausted` 为 true。数据库连接等基础设施异常原样抛出，不得伪装为业务 rejection，也不得 fallback。

校验顺序固定为：strict 结构与基本值域 → protocol/platform → authority kind/结构 → base64/length/checksum/canonical JSON → 外内 header 一致 → target → contract registry/hash/shape。前序失败时不得执行后序数据库读取或写入。

### 5.4 Replay 与 conflict

identity 固定为 `(workspaceId,captureId)`，variant 固定为 `(workspaceId,captureId,"sha256",checksumValue)`：

- 已有相同 variant：返回既有 RawSnapshot 与唯一 Receipt，不创建任何行；若既有为 conflict，replay 仍返回其 conflict 状态；
- 无任何 verified variant：本提交创建 verified；
- 已有 verified 且 hash 不同：完整追加 conflict Package/Snapshot/Records/Artifacts/Receipt；
- 两个不同 hash 并发争夺首个 verified：唯一一个成为 verified，另一个在重试后成为 conflict；
- conflict 不推进 execution，也不产生任何旧 outbox 或投影输入。

## 6. SERIALIZABLE 重试

最多执行 3 次事务尝试（首次 + 2 次重试）。重试等待固定为 10ms、50ms，不使用随机抖动，便于测试。

可重试范围仅限：

- Prisma `P2034`；
- PostgreSQL SQLSTATE `40001`（serialization_failure）；
- PostgreSQL SQLSTATE `40P01`（deadlock_detected）；
- Prisma `P2002` 触发后的权威重读尚不能解析为 replay/conflict。

`P2002` 后必须先重读 exact variant 和 verified variant：相同 hash 立即 replay；不同 verified hash 进入 conflict 重试；仍不可见才消耗下一次尝试。第三次仍未收敛返回 `concurrency_exhausted`，不得调用 V1。

## 7. migration ingress 边界

首期不新增 migration API、CLI 或定时任务。`migration` 只作为 EvidenceIngress 内部受控能力与隔离测试合同存在。当前历史 RawSnapshot/RawRecord 不经 migration ingress 回填，仍按已确认规则退役。

未来确有外部迁移包时，必须另立编号工单，指定真实来源、授权记录和调用文件；该调用只能构造本文件定义的 CaptureSubmissionV2，不得新增第二写入口。

## 8. execution 控制平面

最终切流时 execution 调用顺序固定为：

1. execution 适配器验证真实 station/job/attempt/lease 与 workspace；
2. 调用 EvidenceIngress；
3. `conflict` 或非重试 rejection：不推进任何执行状态；
4. `committed` 或 verified `replay`：在独立、幂等的 execution 控制事务中 CAS 释放 lease，并推进 QueueEntry、ExecutionJob、TaskAttempt、Runtime；
5. Evidence 已提交但控制事务失败：返回 `evidence_committed_control_pending` 的可重试结果；插件重交相同 capture 时 EvidenceIngress replay，随后再次执行同一 CAS；
6. 控制事务不得修改 Evidence，也不得因失败删除 Evidence。

旧 `raw_snapshot.committed` Outbox 在最终切流时停发。B1 不以任何新 outbox 接力 V1 Projection；Normalization/ContractEvaluation/Projection 只在后续阶段接入。暗态核心工单不修改 execution 控制平面或 outbox。

## 9. 分阶段代码工单

### B1-B-02：暗态核心

- 本文件的 types、strict validator、canonical JSON、package decode/hash；
- 可注入 registry 的 EvidenceIngress 核心；
- Release-B schema prepare（只放宽/复合 FK/idempotency scope）；
- 单元测试与全新隔离库证明；
- 不接任何现役调用方，不改变 outbox/执行状态，不加最终 trigger/撤权。

### 后续工单

1. 注册真实 XHS CollectionContract 与 fixture；
2. execution/manual_import/recovery authority adapter 与暗态编排；execution 使用独立幂等 CAS，控制失败由同包 verified replay 续推；migration 保持无入口；
3. B2/B3 准备完成后，一次性移除旧写入口、停旧 outbox、切全部入口；
4. 历史退役经单独批准并清零后，执行 NOT NULL、append-only trigger 与数据库角色撤权；
5. 不满足任一最终门禁时不得部署切流。
