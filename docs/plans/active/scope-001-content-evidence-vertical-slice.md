# SCOPE-001 Content Evidence 首个可证伪垂直切片

> 状态: 活跃计划
> 最后核对: 2026-08-20
> 适用范围: 第一段已获准实施的 Rust + PostgreSQL 16 合成事实内核
> 事实来源: 已确认的 USER-DEC-01–06、DISC-001、SCOPE 前置矩阵、领域不变量与当前 Bootstrap workspace
> 冲突时以谁为准: `AGENTS.md`、用户最新确认、ACCEPTED ADR、真实 fixture/测试/数据库副作用；本文不能把合成测试扩大成真实 XHS 证明

## 当前结论与授权状态

> **定位：** SCOPE-001 是第一条端到端“实现证明切片”，用于证明事实内核与数据库约束可以真实成立；它不是首个真实市场情报产品版本，也不证明 Linggan 已可用于日常真实研究。

SCOPE-001 选择以下最小垂直切片：

```text
服务端预置的合成 content-detail Work Order / Attempt
→ 一个冻结的终态 Capture Package
→ PostgreSQL 原子接入 Package、Record、Coverage、receipt 与处理工作
→ worker 逐 Record 解析 Source Identity 和类型化 Content Observation
→ 原子发布有字段来源的 Content Current revision
→ API 与最小 CLI 返回 provenance、Coverage、applicability、限制和处理水位
```

采用 **API + minimal CLI**。CLI 只调用 API，不连接数据库；Web UI 不进入本切片。

六项产品/架构决定已经确认，DISC-001 已退出。最终独立 Agent 对抗审查、问题吸收和 Mog 最后实施确认均已于 2026-08-20 完成。用户授权在代码前设计基线提交推送后，按本文范围创建 migration、fixture、Rust 业务实现、测试数据库角色和验证脚本；授权不包含真实平台、真实原文、插件改动、AI Agent 或后续产品切片。

## 这条切片要证明什么

对 Mog 可见的结果不是“HTTP 200”或“表里有行”，而是能够查询一个合成 Content，并看到：

```text
对象：synthetic-note-001
当前标题/正文：来自哪些不可变 Observation
观察时间、服务端接收时间、接纳时间、读取时间：分别是什么
本轮目标语义：已知对象集合，或最大配额
实际 Coverage：取得多少、失败多少、哪些成员明确未尝试、哪些范围仍未知
当前选择规则：content-current-policy-v1
处理水位：Package 已接纳、Record 已处理、Current 已发布到哪里
用途边界：可以说明哪些对象事实，明确不能证明平台总体、市场趋势或 ADHD 家庭需求
```

这条切片同时证明三件事：

1. **保存资格：** 批次不完整不会连坐已安全取得的 Record。
2. **事实资格：** 只有通过来源身份解析的 Record 才形成 Source Observation；unresolved 不被伪造成对象。
3. **读取资格：** Current、API 和 CLI 每个采用字段都能回到 Observation → Record → Package；Coverage 和 applicability 不被压成一个 `ok`。

## 明确不证明什么

本切片只使用手工维护的合成 fixture，因此不证明：

- 真实小红书页面、接口、账号、工位、字段、排序、分页、评论或媒体合同；
- 插件已经完成 `claim/renew/reconcile/submit/acknowledge` 升级；
- 搜索结果完整、跨账号可比或“7 天规则”成立；
- Topic、Corpus、分类、聚类、Agent、Signal、Claim、Intelligence、选题或 Outcome 已实现；
- 市场正在增长/下降，材料代表 ADHD 家庭，或现实中不存在未捕获对象；
- 生产部署、备份恢复、RPO/RTO、多租户、MCP 或第三方 Agent 权限已经就绪。

## 入口与用户合同

### API routes

| Route | 用途 | 同步结果 | 明确不做 |
|---|---|---|---|
| `POST /v1/capture/packages` | 为服务端已存在且仍有 authority 的 Attempt 提交一个终态 Package | accepted/replay/conflict/rejected receipt；accepted 时 Package、Record、Coverage 与 processing work 同事务成立 | 不创建 Work Order/Attempt，不接受客户端自授予 authority，不运行 Record 解析 |
| `GET /v1/capture/receipts/{receipt_ref}` | 查询权威接入回执 | Package 接入、replay/conflict、Coverage 与处理水位 | 不把 Package accepted 写成对象已观察完成 |
| `GET /v1/processing/{work_ref}` | 查询一个 Record processing work | ready/leased/succeeded/stopped/dead-letter 与业务结果类型 | 不把 worker succeeded 写成市场或研究成功 |
| `GET /v1/contents/{content_ref}` | 读取当前 Content | 有来源的 title/body、版本、水位和限制 | 不返回数据库自增 ID 或原始任意 payload |
| `GET /v1/contents/{content_ref}/explain` | 解释 Current | 字段来源 Observation/Record/Package、时间、Coverage、policy、applicability | 不生成 AI 总结或趋势结论 |

普通 API 不提供“创建 synthetic Work Order/Attempt”route。集成测试通过专用 test harness 在 proof database 预置获准目标；生产 build 不暴露这个入口。

### CLI commands

新增一个极小 `apps/cli`，二进制名为 `linggan`：

```text
linggan content get <content-ref> [--json]
linggan content explain <content-ref> [--json]
linggan processing status <work-ref> [--json]
```

CLI 只通过版本化 HTTP API 访问。它不能接收数据库 DSN，不能执行任意 SQL，不能提交 Package，不能创建 Work Order/Attempt，也不能绕过用途和访问检查。人类输出与 JSON 使用同一 response model，不分别计算一套事实。

### 最小本地认证与权限

本切片不建设完整 Agent Delegation，但网络 API 不能以“本地开发”为由裸奔：

- 默认只绑定 `127.0.0.1`；非 loopback 监听不在本 SCOPE；
- test harness 每次运行生成临时 secret，只进入进程环境或私有临时文件，不提交 Git、不写普通日志；
- 权限固定为 `scope-001:submit-synthetic-package`、`scope-001:read-synthetic-content`、`scope-001:read-processing`；
- CLI 从环境或受控运行时配置取得 API URL 与临时凭据，不接受 DSN，也不把凭据写入命令输出；
- submit、receipt、content、explain、processing 分别有无认证和错权限负例；失败时不泄露资源是否存在，不产生 Package/Record/processing work，只允许不含 token/body 的最小安全审计；
- response 中的 `permissions` 只说明本地合成 proof 权限，不宣称完整外部 Agent 委托已经实现。

### API/CLI envelope

所有读取响应至少包含：

```json
{
  "data": {},
  "scope": {},
  "asOf": {},
  "versions": {},
  "coverage": {},
  "applicability": {
    "allowed": [],
    "notEstablished": []
  },
  "limitations": [],
  "provenance": [],
  "processing": {},
  "permissions": {}
}
```

`applicability` 只针对本次读取目的说明允许用途与尚未证明事项，不写回 Evidence 永久属性。unknown 必须显式表达，不能省略成 `0`、`false`、空数组或完整成功。

## Capture Package v1 合同

### 唯一来源

新合同由本 SCOPE、后续 `docs/data-contracts/capture-package-v1.md` 和仓库内手工 fixture 共同维护。`references/` 中的 V2 合同只用于对照，不自动生成新 fixture，不成为隐形上游。

### 顶层 envelope

首切片只接受 `content-detail.synthetic.v1`：

```json
{
  "schemaVersion": "capture.package.v1",
  "workOrderRef": "UUID",
  "attemptRef": "UUID",
  "captureIdentity": "UUID",
  "leaseEpoch": 1,
  "contractVersion": "content-detail.synthetic.v1",
  "target": {},
  "terminal": {},
  "coverage": {},
  "knownTargetResults": [],
  "records": [],
  "packageHash": "sha256:<hex>"
}
```

服务端生成 `workOrderRef`、`attemptRef`、`captureIdentity` 和 `leaseEpoch`。producer 只能回传，不能自行创建或更换。

### 首切片支持的两种目标语义

已知对象集合：

```json
{
  "basis": "known_set",
  "unit": "content_detail",
  "targetManifestHash": "sha256:<hex>",
  "knownTargetCount": 3
}
```

最大配额：

```json
{
  "basis": "maximum_quota",
  "unit": "content_detail",
  "limit": 100
}
```

`source_exhaustion`、`time_budget`、`risk_budget` 和 `probe` 的产品语义已经由 USER-DEC-02 保护，但不进入 v1 wire enum；需要真实 lane 时再以新合同版本加入，避免先造一个万能枚举。

`known_set` 的 `targetManifestHash` 精确覆盖服务端冻结并按 `ordinal` 排序的数组：

```json
[
  {"ordinal": 1, "externalId": "synthetic-note-a"},
  {"ordinal": 2, "externalId": "synthetic-note-b"},
  {"ordinal": 3, "externalId": "synthetic-note-c"}
]
```

数组使用与 Package 相同的 JCS 规则计算 SHA-256。Package 必须携带被 hash 覆盖的逐目标结果：

```json
"knownTargetResults": [
  {
    "targetOrdinal": 1,
    "targetExternalId": "synthetic-note-a",
    "outcome": "emitted",
    "recordOrdinal": 1,
    "reason": null
  },
  {
    "targetOrdinal": 2,
    "targetExternalId": "synthetic-note-b",
    "outcome": "failed",
    "recordOrdinal": null,
    "reason": "page_error"
  },
  {
    "targetOrdinal": 3,
    "targetExternalId": "synthetic-note-c",
    "outcome": "not_attempted",
    "recordOrdinal": null,
    "reason": "risk_control"
  }
]
```

规则：每个冻结目标恰好出现一次；`emitted` 必须关联同 Package 唯一 Record ordinal；`failed/not_attempted` 不得关联 Record；逐成员聚合必须与 Coverage 完全一致。`maximum_quota` 的 `knownTargetResults` 必须缺省，不得用配额差额伪造成员身份。

### terminal 与 Coverage

首切片 terminal reason 只支持 fixture 真正使用的 `target_reached` 与 `risk_control`。Coverage 固定同一个单位：

```json
{
  "unit": "content_detail",
  "attempted": 2,
  "emitted": 2,
  "failed": 0,
  "knownNotAttempted": 1,
  "remainingScope": "known_members"
}
```

或：

```json
{
  "unit": "content_detail",
  "attempted": 50,
  "emitted": 50,
  "failed": 0,
  "knownNotAttempted": null,
  "remainingScope": "unknown"
}
```

约束：

- 所有计数为非负整数，`emitted + failed = attempted`；
- `known_set` 必须与服务端冻结 target manifest 完全一致，且 `attempted + knownNotAttempted = knownTargetCount`；
- `maximum_quota` 必须满足 `attempted <= limit`，`knownNotAttempted` 必须为 null，未证明来源穷尽时 `remainingScope` 必须为 `unknown`；
- `target_reached` 只能在已知集合 `knownNotAttempted = 0` 且全部成员已有 emitted/failed 结果，或最大配额确实达到上限时使用；
- 不能跨单位相减搜索卡片、详情、评论、回复或页面访问次数。

### Record envelope

每个成员至少包含：

```json
{
  "ordinal": 1,
  "recordKind": "content_detail",
  "targetExternalId": "synthetic-note-001",
  "source": {
    "system": "synthetic",
    "namespace": "scope-001",
    "objectType": "content",
    "externalId": "synthetic-note-001",
    "channel": "synthetic_page"
  },
  "observedAt": {
    "value": "2026-08-20T08:00:00Z",
    "precision": "exact",
    "basis": "fixture"
  },
  "payload": {
    "schemaVersion": "content-detail.synthetic.v1",
    "sourceExternalId": "synthetic-note-001",
    "fields": {
      "title": {"observed": true, "value": "..."},
      "body": {"observed": true, "value": "..."}
    }
  },
  "recordHash": "sha256:<hex>"
}
```

Package 最小硬门只验证：authority、Attempt/Capture/epoch、合同与目标、canonical/hash、manifest/ordinal/成员边界，以及 Record envelope 能被安全枚举和重放。`payload.sourceExternalId`、title/body 等平台语义在接纳后逐 Record 解析；一条 Record 的来源关系错误不得回滚其他成员。

F-10 允许 `source.externalId` 为 null，以证明“原料可接纳但身份 unresolved”；这不会创建 Source Identity。F-06 使用 envelope 的 target 与 payload 内部来源陈述冲突，证明错误属于 Record 处理，不属于 Package 身份硬门。

### Canonical 与 hash

- 使用 RFC 8785 JSON Canonicalization Scheme，并把 v1 输入限制为 I-JSON；拒绝重复 key、非法 Unicode、浮点数、NaN、Infinity、超出 JavaScript 安全整数范围的数值和未声明扩展字段；字符串不做 Unicode normalization，原始码点序列就是合同语义；UUID、时间和 hash 使用固定词法形式；
- `recordHash` 由去除 `recordHash` 后的完整 Record canonical bytes 计算 SHA-256；
- `packageHash` 由去除 `packageHash` 后的完整 Package canonical bytes 计算 SHA-256；
- 服务端必须重算，不能信任客户端提供值；
- 同 Capture Identity + 同 package hash 返回同一权威 accepted receipt；同 Capture Identity + 不同 hash 返回 conflict，旧 Package 不变；
- 实现若需要更换 canonical library，必须先证明所有 golden fixture 字节与 hash 不变，否则发布新合同版本，不能静默 fallback。
- golden fixture 保存预先固定的 canonical UTF-8 bytes 或稳定文本表示和 expected hash，测试启动时不得由被测 Rust 函数生成期望值；同时使用 RFC 8785 官方向量和一个独立 Node/reference checker 复核。该 checker 只证明合成合同，未来真实 TypeScript producer 仍须重新做跨语言兼容验收；
- HTTP body、Record 数量、单字段字符串和 JSON 嵌套深度必须有服务端硬上限，并同时受 Work Order target/quota 约束；重复 key、大整数、未知字段、错误 record/package hash、超限 Record 和超大 body 都有负例。

## 十组 fixture 与预期结果

固定目录：

```text
crates/contracts/tests/fixtures/capture-v1/
  manifest.json
  valid-complete-content-package.json
  valid-partial-content-package.json
  replay-same-hash.json
  conflict-different-payload.json
  invalid-package-identity.json
  invalid-record-source-identity.json
  late-observation.json
  source-disagreement.json
  quota-target-partial-unknown.json
  mixed-record-processing.json
```

`manifest.json` 记录 fixture 编号、合成性质、合同版本、fresh database seed、ordered steps、mutation cases、每步 expected package/record/observation/current 结果、数据库不变量、固定 canonical bytes/hash 和禁止外推范围。每个场景从独立 seed 启动，不依赖测试文件顺序或其他场景留下的状态。

| Fixture | 必须证明 | 数据库禁止副作用 |
|---|---|---|
| F-01 完整合法包 | 2 个目标、2 个 Record、2 个处理工作、2 个 Observation，Current 可解释 | 不得少成员或用 `ok` 代替行/约束断言 |
| F-02 已知集合部分结果 | 3 个冻结目标中 2 个合格接入，1 个明确未尝试，Attempt 不显示完整完成 | 不得丢弃 2 个 Record；不得为空缺成员建对象/Observation |
| F-03 replay | 同 identity/hash 返回原 receipt；首次 accepted 后 authority 过期再重传同 hash，仍返回原 receipt | Package/Record/work/Observation 均不增加；只追加安全 replay delivery |
| F-04 conflict | 同 identity、不同 hash fail closed | 不覆盖旧 hash/payload，不创建第二 Package/Evidence |
| F-05 Package 身份错误 | 从合法 base fixture 分别变异 Attempt/Work/capture/epoch/contract/authority，每项独立拒绝 | 不产生 Package/Record/Observation；只留最小安全 delivery/failure audit |
| F-06 单 Record 来源身份错误 | 合格 Package 内坏 Record 终止为 source identity conflict，其他成员正常 | 不回滚整包，不为坏 Record 建 Source Identity/Observation |
| F-07 迟到 Observation | 较早观察后接入，历史保留，Current 不倒退 | 不按 received/accepted/last-write 决定 Current |
| F-08 来源差异 | 同一 observed time 不同来源对 title/body 给出不同值，field resolution 为 unresolved | 不取最大、平均或最后写入；双方 Observation 均保留 |
| F-09 最大配额部分结果 | quota 100、取得 50、剩余范围 unknown | 不建 50 个未尝试对象，不显示 50% 完成或平台共 100 个 |
| F-10 混合 Record | 6 个 Record 原子接入；4 个新对象、1 个复用身份并形成新 Observation、1 个 unresolved | 不等待全部解析才接入；不把身份复用记作 replay；不重复计数对象 |

fixture 只含合成英文/中文短文本和虚构身份，不包含真实 ADHD、儿童、账号、Cookie、Token、DSN 或旧 Evidence。

## PostgreSQL 16 物理范围

### migration 拆分

获准实现后只创建两份 migration：

```text
database/migrations/0001_scope_001_capture_evidence.sql
database/migrations/0002_scope_001_content_observation.sql
```

第一份拥有 Work/Attempt、Package/Record/Coverage/receipt 与 Record processing work；第二份拥有 Source Identity、Content Observation 与 Current。migration 一旦提交不回改，后续修正追加新 migration。默认不维护 destructive down migration；回退以全新 proof database 重放为准。

### 表清单与责任

| 表 | 责任 | 关键约束 |
|---|---|---|
| `capture_work_order` | 服务端有界 content-detail 工作、合同与目标语义 | public UUID 唯一；v1 lane/contract/target basis check；不保存研究或 Topic |
| `capture_work_order_target` | `known_set` 的冻结目标成员 | `(work_order_id, ordinal)` 与 `(work_order_id, external_id)` 唯一；quota 不建成员 |
| `capture_attempt` | 一次独立执行权、Capture Identity 与执行终态 | Work 1:N；capture identity 唯一；同一 Work 同时最多一个 active authority；lease epoch/authority deadline 使用 DB 时间；terminal outcome 不等于 Package accepted |
| `capture_ingress_delivery` | 每次 HTTP 交付发生、accepted/replay/conflict/rejected 最小审计 | 不保存被拒绝的完整任意载荷；可关联权威 receipt 或安全 incident ref |
| `capture_package` | 一个 Attempt 的唯一冻结终态业务包 | `(attempt_id, capture_identity)` 组合引用同一 Attempt；`attempt_id` 与 `capture_identity` 各自唯一；canonical hash 不可更新；public receipt UUID 唯一 |
| `capture_record` | Package 内不可变、可重放的 Record envelope 与合成 payload | `(package_id, ordinal)` 唯一；record hash；payload JSONB 只保存合同原料，不承载 Current |
| `capture_package_target_result` | known-set 每个冻结成员在本 Package 的 emitted/failed/not-attempted 结果 | 每 Package/target 唯一；target 必须属于 Package 的 Work；emitted 必须组合引用同 Package Record；quota 没有该行族 |
| `capture_package_coverage` | 本 Package 同单位 Coverage 与终止事实 | Package 1:1；basis-specific CHECK；unknown 不能被差额制造 |
| `record_processing_work` | 只处理一个 accepted Capture Record 的持久工作 | `capture_record_id` 唯一；processor 固定 `content-detail-processor-v1`；typed FK，不使用任意 job payload JSON |
| `record_processing_attempt` | worker claim/lease/epoch/finalize 历史 | `(work_id, epoch)` 唯一；旧 epoch 不能 finalize；业务 outcome 与运行状态分开列 |
| `source_identity` | 极小来源身份注册 | `(source_system, namespace, object_type, external_id)` 唯一；无正文/Topic/指标/业务状态 |
| `source_content` | 类型化 Content anchor 与公开引用 | `source_identity_id` 唯一；`public_ref` 唯一；current revision pointer 可空但只能指向自身 revision |
| `content_observation` | 一次 Record 支持的类型化 Content 状态 | `capture_record_id` 唯一；只追加；title/body 各有 observed flag；时间与 parser version 分开 |
| `content_current_revision` | 可重建、不可变的当前读取版本 | typed title/body 值、三态、policy/version/watermark；不做 EAV；来源集合由下表固定 |
| `content_current_revision_field_source` | 固定每个 Current 字段当时采用或冲突的 Observation 集合 | `(revision_id, field_kind, observation_id)` 唯一；role 为 selected_support/conflicting_candidate；三者必须属于同一个 Source Content |

`capture_work_order` 的目标满足程度与 Package 接入结果分责：Package 只说明交付是否 accepted/replay/conflict/rejected；Attempt 保存本次为何停止及实际 Coverage；Work Order satisfaction 从全部 Attempt/Package 重建为 satisfied/known_gap/unknown/needs_decision，不把部分 Package 冒充目标已满足。补采创建新 Attempt/Package，旧执行历史不改。

下列跨表关系必须由数据库组合 FK、唯一/排除约束、deferred constraint trigger 或消除冗余字段直接保护，并有绕过 Rust facade 的 SQL 负例：Attempt/Capture 同属、Package/Work Target 同属、emitted Result/Record 同 Package、content-detail Package 内目标身份不重复、Coverage 与 Target Result/Record 聚合一致、Current/Observation/Content 同属、同一 Work 同时最多一个 active authority。不能只靠应用层“先查再写”。`source_content ↔ content_current_revision` 的创建顺序固定为先建可空 pointer 的 content，再建 observation/revision/source relation，最后增加或验证组合 FK；运行期先插完整 revision 再原子更新 pointer。

### Current policy v1

`content-current-policy-v1` 对 title 和 body 分别解析：

1. 只考虑对应字段 `observed=true` 且 Record 处理合格的 Observation；
2. 按来源 `observed_at` 选择最新资格集合，不使用 received/accepted/insert 时间；
3. 最新资格集合只有一个不同值时选择该值，并把所有给出该值的 Observation 固定为 `selected_support`，不能依处理顺序任挑一个；
4. 同一最新观察时刻出现多个不同值时，该字段为 `unresolved`，value 为空，并把全部冲突 Observation 固定为 `conflicting_candidate`；
5. 较新 Observation 没观察某字段时，不清空旧字段；
6. 没有合格值时为 `unknown`；
7. 每次重算新建不可变 revision；在一个短事务中插入完整 revision 并更新 `source_content.current_revision_id`，读者只跟随已发布 pointer，不看半成品。

每个字段使用 `selected | unknown | unresolved` 三态和配套 CHECK：`selected` 必须有值和至少一个同值 `selected_support`；`unresolved` 必须无值且有至少两个同一最新时刻、不同值的 `conflicting_candidate`；`unknown` 必须无值且无伪造来源。revision 固定精确输入 watermark，历史 explain 只读取该 revision 的固定来源集合，不随以后新增 Observation 改写。`content-detail.synthetic.v1` 只接受 `observedAt.precision = exact`；非精确时间留给后续策略版本。title/body 是本切片唯一 Current 字段，author、publishedAt、metric、media 等后置。

### authority fence

首切片采用失败关闭的最小规则。接入事务先认证调用者、重算 canonical/hash，并按 Capture Identity 锁定 Attempt 与可能存在的 Package：若已有 accepted Package，同 hash 返回原 receipt 并追加最小 replay delivery，不同 hash 返回 conflict；只有尚无 accepted Package 的首次接纳，才在取得锁后使用数据库当前时钟检查 capture identity、lease epoch、authority 状态和 `authority_valid_until`，随后写入 Package/Evidence。事务外检查、客户端时钟和先查后写均无资格；不得用事务开始时的陈旧时间越过等待锁后的 deadline。

这意味着“在 authority 有效时采到、但过期后才首次提交”的材料不会走普通首次接入；只保留最小安全失败回执，完整冻结载荷仍由 producer 本地保留并等待未来独立 recovery/import 设计。已经接纳过的同 hash replay 不受后来 authority 过期影响，但仍要求调用者认证。首切片不伪造晚到合法性，也不因为这一保守规则删除本地原料。真实插件恢复边界在插件 SCOPE 中单独证明。

### Durable work 形态

首切片选择专用 `record_processing_work`，不建立共享万能 `durable_work`：当前只有一个 owner、一个 typed input 和一个 handler，提前抽象通用 Workflow 没有收益。

claim 使用 PostgreSQL 行锁/`SKIP LOCKED` 或等价原子更新；每次领取增加单调 epoch 并建立 `record_processing_attempt`。finalize 在同一事务验证当前 epoch、lease 未过期、输入仍有效，然后追加 Source/Observation/Current 和工作结果。worker 崩溃时 lease 到期可由新 epoch 接管；旧 epoch 后到必须被数据库拒绝。

SCOPE-001 只允许固定的 `content-detail-processor-v1`。同一 Record 不在本切片以新 parser version 重解释；任何第二 processor version 必须失败关闭，不得 UPDATE 旧 Observation 或伪造新世界 Observation。parser 重处理等到后续 SCOPE 明确引入 Interpretation Revision 后再开放。

### 数据库角色

第一切片只证明一个最小运行角色，不提前设计生产 IAM：

- migration/测试管理员：只用于创建 proof database、执行 migration、建立/回收测试角色；
- `scope_001_runtime` 测试角色：API 与 worker 首切片共用，按表授予所需 DML，不得 CREATE/ALTER/DROP，不得 UPDATE/DELETE Package、Record、Coverage、Observation 或 Current revision；
- CLI 没有数据库角色和 DSN，只访问 API。

API/worker 是否在生产拆成两个数据库角色留给首次部署安全 SCOPE；本切片必须先用负向 SQL 证明运行角色不能修改不可变历史或执行 DDL。

## 事务、并发与失败注入

### Package ingress 事务

accepted 路径同一事务完成：

```text
caller authentication + canonical/hash
→ lock Attempt/Capture Identity and replay/conflict check
→ first-ingress authority/epoch/contract/target fence
→ capture_ingress_delivery
→ capture_package
→ capture_record(s)
→ known target result(s)
→ capture_package_coverage
→ record_processing_work(s)
→ freeze Attempt terminal relation and re-evaluate Work satisfaction
→ authoritative receipt
```

逐 Record parser、Source Identity、Observation 和 Current 不在该事务内。Package 事务故障注入点至少覆盖 Package 后、Record 中途、Coverage 前和 work 中途；任一点失败时 accepted 业务行全部为 0，安全请求日志不能冒充 accepted receipt。合法 replay 只返回原 receipt 并追加最小 delivery，不重写 Attempt 终态或 Work satisfaction。

### Record processing 事务

每个 Record 独立：

```text
claim/epoch fence
→ parser contract validation
→ resolve or create Source Identity under unique constraint
→ create/reuse Source Content
→ append Content Observation when qualified
→ lock Source Content and build Current revision
→ publish current pointer
→ finalize processing outcome
```

identity unresolved/conflict 是该 Record 的可解释业务结果，不回滚其他 Record，也不生成空 Source Identity。相同 Record 的 worker retry/lease 接管返回或完成同一 `content-detail-processor-v1` 结果；第二 processor version 在本切片失败关闭，不能覆盖旧结果或生成第二份 Observation。

### 必须通过的并发/故障场景

1. 100 个并发 Package replay 只有一个 Package/Record/work 集合；
2. 首次 accepted 后 authority 过期，同 hash replay 仍返回原 receipt 且业务行不增加；
3. 同 Capture Identity 不同 hash 并发时最多一个 accepted，其他 conflict，不能 last-write-wins；
4. authority 在首次接入前失效时所有 accepted 业务副作用为 0；
5. 两个 worker claim 同一 work 只有一个有效 epoch；
6. 旧 epoch 在新 epoch 接管后不能写 Observation 或 finalize；
7. 同一来源身份并发解析只产生一个 `source_identity` 与一个 `source_content`；
8. 绕过 Rust 直接提交串错 Attempt/Capture、Package/Target/Record 或 Current/Observation/Content 的 SQL 全部被数据库拒绝；
9. F-06/F-10 的坏 Record 不撤销其他成员；
10. F-07 迟到历史保留且 Current 不倒退；
11. F-08 同时刻不同值使字段 unresolved 且保留全部冲突来源；同时间同值按完整支持集合得到确定结果；
12. 较新 Observation 未观察字段时保留旧字段及旧来源；
13. Current revision 构建中途失败时 pointer 不变化，未发布 revision 不可读。

## Rust 模块与允许文件

只修改或创建以下责任；没有真实调用者的文件不提前生成。

### `crates/contracts`

```text
src/lib.rs
src/canonical.rs
src/capture/mod.rs
src/capture/package.rs
src/capture/record.rs
src/capture/target.rs
src/capture/coverage.rs
src/capture/validation.rs
tests/capture_contract.rs
tests/capture_golden.rs
tests/fixtures/capture-v1/*
```

### `crates/evidence`

```text
src/lib.rs
src/ingress/mod.rs
src/ingress/authority.rs
src/ingress/idempotency.rs
src/ingress/transaction.rs
src/work_order.rs
src/receipt.rs
tests/ingress_postgres.rs
tests/ingress_concurrency.rs
```

### `crates/observation`

```text
src/lib.rs
src/processing.rs
src/source_identity.rs
src/content/mod.rs
src/content/observation.rs
src/content/current.rs
src/read.rs
tests/content_postgres.rs
tests/content_current_concurrency.rs
```

### `crates/storage-postgres`

```text
src/lib.rs
src/pool.rs
src/error.rs
src/transaction.rs
src/testing.rs
```

`storage-postgres` 只提供 pool、事务/错误和 proof DB helper；业务 SQL 留在拥有不变量的 evidence/observation 私有 adapter，不创建万能 CRUD repository。

### 组合入口

```text
apps/api/src/main.rs
apps/api/src/app.rs
apps/api/src/routes/capture.rs
apps/api/src/routes/content.rs
apps/api/src/routes/processing.rs
apps/api/src/http/envelope.rs
apps/api/src/http/error.rs

apps/worker/src/main.rs
apps/worker/src/app.rs
apps/worker/src/runner.rs
apps/worker/src/process_capture_record.rs

apps/cli/src/main.rs
apps/cli/src/client.rs
apps/cli/src/commands/content.rs
apps/cli/src/commands/processing.rs
apps/cli/src/output/human.rs
apps/cli/src/output/json.rs
```

`crates/domain` 与 `crates/intelligence` 不因本切片创建空业务模块或预留 trait。若实现证明 `evidence` 与 `observation` 必须共享一个数据库事务，优先通过明确 application use case 和受控 transaction context 组合，不把所有逻辑移进 `storage-postgres` 或 `domain`。

### 必要支撑文件

上面的清单限制业务 `.rs` 文件；实现还允许且只允许按实际需要修改或创建以下支撑文件：

```text
Cargo.toml
Cargo.lock
crates/contracts/Cargo.toml
crates/evidence/Cargo.toml
crates/observation/Cargo.toml
crates/storage-postgres/Cargo.toml
apps/api/Cargo.toml
apps/worker/Cargo.toml
apps/cli/Cargo.toml
database/migrations/0001_scope_001_capture_evidence.sql
database/migrations/0002_scope_001_content_observation.sql
docs/data-contracts/capture-package-v1.md
scripts/verify-capture-fixtures.mjs
scripts/test-scope-001-postgres.sh
scripts/check-rust-boundaries.sh
database/README.md
docs/runbooks/development-environment.md
docs/README.md
docs/current-state.md
docs/progress/2026-08.md
```

新增其他业务文件、临时目录或第三份 migration 前必须回到 SCOPE 说明为何现有责任无法承载，不能用“顺手重构”扩大范围。

## 依赖与文件门禁

允许按实际使用引入：`serde`、`serde_json`、`uuid`、`time`、SHA-256、JCS、`sqlx`、`tokio`、`axum`、`thiserror`、`clap` 和一个 HTTP client。每个依赖进入 workspace 前要说明调用点、功能开关和最小版本依据；实现时以当前官方文档/锁文件验证，不在 SCOPE 中猜精确 crate 版本。

不引入 ORM、GraphQL、Kafka、Redis、Temporal、Camunda、pgvector、LLM SDK、Python bridge、对象存储 SDK、OpenTelemetry 全家桶、通用 DI/CommandBus/Workflow crate 或万能 repository mock。

首个实现 PR 同时增加自动门：

- `lib.rs/main.rs` 120 行预警、200 行失败；
- 生产 `.rs` 350 行预警、500 行失败；
- 函数/方法 60 行预警、100 行失败；优先使用可靠的 Clippy lint，无法可靠自动化时作为明确 review 门，不为此自建 Rust 语法分析器；
- 测试文件 600 行预警、900 行失败；
- 单模块 public item 15 个预警、25 个失败；首切片先生成 warning/report，只有可靠工具证明后才升级硬失败，不用脆弱正则误伤宏和属性；
- 禁止新增无所有者的 `common/`、`utils/`、`helpers/`；
- `contracts` 不依赖 evidence/observation/storage；
- apps 不含业务 SQL；
- SQLx/Axum/CLI 类型不穿过深模块公共接口；
- 例外只能使用有负责人、原因、替代计划和复核日期的 ADR。

## TDD 执行顺序与阶段退出

1. **合同与 fixture** → verify：F-01–F-10 独立场景、known-set 逐成员结果、JCS 固定 golden + Node/reference 复核、I-JSON/资源上限、unknown/目标语义负例先失败再通过。
2. **空库 migration 与权限** → verify：随机 proof DB 从零重放两份 migration；运行角色不能 DDL、UPDATE/DELETE 不可变历史，组合 FK/聚合/同属约束能拒绝绕过 Rust 的串错 SQL。
3. **Package ingress** → verify：认证、accepted/replay/conflict/rejected、authority 过期后的合法 replay、F-02/F-09、事务中途故障和数据库行数/约束同时通过。
4. **Record processing** → verify：F-06/F-10、identity 并发、unresolved、worker epoch/fence 和重启接管通过。
5. **Observation/Current** → verify：append-only、F-07/F-08、字段来源和 pointer 原子发布通过。
6. **API/CLI** → verify：真实 loopback API + worker + PostgreSQL 运行；CLI 只经 API 返回相同 envelope，断网/无认证/错权限/processing 未完成均诚实显示，secret 不进 Git/日志/输出。
7. **边界与文档** → verify：文件/依赖门、合同文档、数据库 README、runbook、月度记录、治理与 Bootstrap 全部通过。

每一步必须先看到与预期一致的失败证据，再写最小实现。不得一次创建全部表和代码后补测试，也不得因为早期 crate 编译通过跳过真实 PostgreSQL 副作用。

## 统一验证命令

实现期要交付并实际运行：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
./scripts/test-scope-001-postgres.sh
./scripts/check-rust-boundaries.sh
./scripts/check-project-governance.sh
./scripts/verify-bootstrap.sh
```

`test-scope-001-postgres.sh` 必须精确创建随机 `linggan_intelligence_proof_<suffix>`，运行真实 PostgreSQL 16 测试并在成功/失败后只删除该 proof database；不得 reset 开发库、恢复旧 dump、打印 DSN/密码或删除 Docker 数据卷。

## 完成标准

只有以下全部成立，SCOPE-001 的实现才可报告完成：

1. F-01–F-10 每组都有独立 seed/步骤/预期；F-05 mutation、authority 过期 replay 和 JCS/I-JSON 攻击性负例可以单独运行；
2. API/module receipt 与真实 PostgreSQL 行、唯一约束、权限和事务副作用同时成立；
3. known-set 逐目标 emitted/failed/not_attempted 与 Record 映射可追溯，2/3 与 quota 50/100 在数据库、API、CLI 中保持不同语义；
4. accepted Package 不等待逐 Record 解析；坏 Record 不连坐其他成员；
5. replay 不重复、authority 过期后的同 hash replay 可返回原 receipt、conflict 不覆盖、normal re-observation 不被误叫 replay；
6. authority/worker 旧 epoch 都不能写入受保护事实；
7. Source Identity 并发唯一，unresolved 不制造对象；
8. Content Observation 只追加，迟到历史不丢，Current 不按最后写入倒退；
9. title/body 的 selected/unresolved 都固定完整支持或冲突来源集合，并能追到 Observation、Record、Package 与合同版本；
10. API/CLI 明确返回 scope、四类时间、版本、Coverage、applicability、limitations、provenance 和 processing；
11. API、worker 或 CLI 重启后历史仍存在，未完成 work 可安全接管；
12. 运行角色不能修改不可变历史、串接不同父对象或执行 DDL；API 有最小本地认证，CLI 无数据库旁路；
13. 文件规模与依赖方向自动门通过，没有巨型文件或万能模块；
14. 没有真实平台访问、旧库连接、未脱敏数据、secret、fallback 或静默降级；
15. 所有统一验证命令通过，且数据库副作用证据被记录为脱敏计数/hash/receipt，不提交 dump；
16. 交付措辞只允许“合成事实链实现证明通过”，不得报告 Linggan 已可日常使用、真实采集已接通或市场情报闭环已上线。

任何一项缺失，只能报告“完成到哪一层”，不能把 HTTP 200、类型、编译、mock、fixture schema 或 worker succeeded 单独称为端到端完成。

## 失败、停止与回退

- 合同无法稳定跨 Rust/JSON canonical：停止 ingress 实现，修订合同或发布新版本，不做静默兼容。
- PostgreSQL 约束无法表达已确认语义：停止对应表实现，回到 SCOPE 修订，不用应用层先查后写掩盖。
- F-02/F-09/F-10 任一失败：不得继续 API/CLI 包装，先修正部分结果责任。
- authority/current 并发负例无法失败关闭：不得宣布 Evidence 或 Current 可用。
- 文件边界需要例外：先证明为何不能简化；无有期限 ADR 不允许越过硬门。
- 实现回退只删除精确 proof database、撤销未提交代码或追加修正 migration；不 reset 开发库，不覆盖已接纳历史，不执行 broad `rm` 或数据库 destructive fallback。

## 后续切片与硬停止线

SCOPE-001 完成也不自动授权：

- 真实 producer 实验或真实账号/工位访问；
- Capture 插件升级；
- Author/Comment/Media/Metric 全模型；
- Topic/Corpus/AI Agent/Signal/Intelligence；
- Web 页面与正式趋势；
- 生产部署和旧系统退役。

下一切片必须根据 SCOPE-001 的真实副作用和缺口重新立项，不从本计划自动扩张。

## 最终对抗审查门

用户要求在代码开工前由独立 Agent 攻击本计划。审查必须覆盖：范围是否仍过宽、Package/Record 原子边界、known-set/quota 语义、canonical/hash、表/约束、authority/worker epoch、Current 选择、API/CLI 旁路、权限、fixture 是否自证、真实数据库验收、文件规模和隐藏的后续授权。

审查结论必须写回本文、当月进度和当前状态；发现 P0/P1 时先修计划再请求代码开工确认。独立审查通过不等于代码已获授权。

### 2026-08-20 独立审查结果

独立只读审查原结论为“条件通过”，发现 1 个 P0、7 个 P1 和 5 个 P2。P0/P1 已全部修入本文：known-set 逐目标结果、authority 过期后的合法 replay、最小本地认证、独立 JCS/I-JSON 证明、组合数据库约束、Current 固定来源集合、单一 processor v1，以及 Package/Attempt/Work satisfaction 分责。P2 的场景独立、支撑文件白名单、可靠代码门、草案状态和实现证明口径也已吸收。

完整记录见 [`../../audits/scope-001-final-adversarial-audit-2026-08-20.md`](../../audits/scope-001-final-adversarial-audit-2026-08-20.md)。用户随后已明确批准按修订后的 SCOPE-001 开始实现；本次授权仍受本文的合成数据、文件白名单、数据库安全与后续切片硬停止线约束。
