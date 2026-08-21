# SCOPE-001 Content Evidence 首个可证伪垂直切片

> 状态: 活跃计划
> 最后核对: 2026-08-21
> 适用范围: 第一段已获准实施的 Rust + PostgreSQL 16 合成事实内核
> 事实来源: 已确认的 USER-DEC-01–06、DISC-001、SCOPE 前置矩阵、领域不变量与当前 Bootstrap workspace
> 冲突时以谁为准: `AGENTS.md`、用户最新确认、ACCEPTED ADR、真实 fixture/测试/数据库副作用；本文不能把合成测试扩大成真实 XHS 证明

## 当前结论与授权状态

> **定位：** SCOPE-001 是第一条端到端“实现证明切片”，用于证明事实内核与数据库约束可以真实成立；它不是首个真实市场情报产品版本，也不证明 Linggan 已可用于日常真实研究。

SCOPE-001 选择以下最小垂直切片：

```text
服务端预置的合成 content-detail Work Order / Attempt
→ 一个冻结的终态 Capture Package
→ PostgreSQL 原子接入 Delivery、Package、Record、Coverage、originalAcceptedDeliveryRef、acceptedReceiptRef 与处理工作
→ worker 逐 Record 解析 Source Identity 和类型化 Content Observation
→ 原子发布有字段来源的 Content Current revision
→ API 与最小 CLI 返回 provenance、Coverage、applicability、限制和处理水位
```

采用 **API + minimal CLI**。CLI 只调用 API，不连接数据库；Web UI 不进入本切片。

六项产品/架构决定已经确认，DISC-001 已退出。最终独立 Agent 对抗审查、问题吸收和 Mog 最后实施确认均已于 2026-08-20 完成。用户当时要求在业务代码前继续消除 Agent 的未经授权推断，并确认采用 Closed World、正负语义 Oracle 和证明边界；当时的开工口径是先完成本文的 G1–G5 语义冻结，五项全部通过后才按本文范围创建 migration、fixture、Rust 业务实现、测试数据库角色和验证脚本。该历史口径及其历史 FAIL 保留；当前 F01 的实施顺序以紧随其后的 2026-08-21 用户确认裁定为准。授权不包含真实平台、真实原文、插件改动、AI Agent 或后续产品切片。

### 2026-08-21 用户确认：F01 主链优先

用户于 2026-08-21 明确确认，F01 不再等待 F02–F10 的全量 fixture、全量攻击输入或每一项未来 manifest 细节才进入 PostgreSQL、worker、API 和 CLI。先完成一条可运行、可查询的合成 F01 主链：一份有效 Package 经原子接入建立 Package/Record/Coverage/processing work，两条 Record 分别处理为 Source/Content/Observation/Current，随后由 loopback API 和只经 API 的 CLI 读取。

F01 主链的最小验收只包括成功主路径和三项保护：accepted ingress 不得提前建立 Observation/Current；接入事务失败不得留下半写；坏 Record 不得撤销同 Package 合格 Record 的后续事实。Package accepted、processing、Observation、Current 的分责，unknown 保留，以及 hash/replay/conflict 不可静默覆盖，仍是不可协商约束。

F02–F10 的表、真值、行数 Oracle 与禁止项完整保留。它们从 F01 的共同开工/完成门改为 F01 主链之后逐项启动的硬化与回归场景库；没有任何一个被删除、改写为通过或扩大为真实 producer/生产证明。此裁定只改变顺序和 F01 主链完成口径；SCOPE-001 整体仍未完成。

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

## 代码前 Agent 执行门

任何 SCOPE-001 实现或审查先遵守 [`../../agents/scope-001-execution-contract.md`](../../agents/scope-001-execution-contract.md)。当前是 Closed World：本文未列出的业务状态、表、route、能力、兼容路径和副作用均未获授权。

代码门只由五项条件开启：当前实施的 F01 主链语义唯一；未列语义有停止规则；F01 主链具备正负 Oracle；读取与结果能回到对应责任；验证声明同时携带 VERIFIED 与 NOT VERIFIED。F02–F10 在各自开始前必须满足同一门槛，但不是 F01 主链进入业务代码的前置。任一当前场景条件未满足时，只允许修订规格、fixture 预期和测试设计，不创建该场景的业务 migration 或 Rust 业务实现。

本切片对状态采用分责而非统一状态机：Ingress Delivery、Package Acceptance、Attempt Terminal、Capture Satisfaction、Record Processing Runtime、Record Processing Business Outcome、Observation Formation 和 Current Field Resolution 必须分别表达。不得用单一 `status`、`ok`、`completed` 或成功布尔值替代。

### 当前代码门状态

| Gate | 主线自审 | 当前证据 | 独立复核 |
|---|---|---|---|
| G1 语义唯一性 | YES（第四轮发现完成最终收口） | ingress/payload 分层、audit union、Work 1:1 Attempt、联合类型与固定 proof refs/time 已封闭 | 第四轮曾 FAIL；其 4 个 P1 已修入，用户决定不再以第五轮独立复核作为前置门 |
| G2 禁止推断 | YES（第四轮发现完成最终收口） | 17 步 ingress、safe/linked/internal、payload owner、确定性 proof harness、精确 response scope 与文件白名单已经写明 | 同上；实现遇到新歧义仍按执行合同局部停止 |
| G3 负向约束 | YES（第四轮发现完成最终收口） | F01–F10 每阶段 delta、F05 两类失败/audit union、F06B 层级负例、processing epoch 与 exact API Oracle 已写明 | 同上；由先失败的合同测试继续验证，不以文档复核代替测试 |
| G4 证据边界 | YES（第四轮发现完成最终收口） | original accepted delivery typed relation、单 Attempt Satisfaction 输入、watermark、三态 field provenance 与 route 范围均已封闭 | 同上；SCOPE-001 不证明跨 Attempt Satisfaction |
| G5 证明边界 | YES | Agent 合同要求每次报告同时给出 VERIFIED 与 NOT VERIFIED | 四轮独立复核始终 PASS |

9801fdf 的第一轮独立只读复核发现 6 个 P1；第一轮修订后的第二次独立复核又发现 7 个 P1；第三次复核发现 7 个 P1；第四次复核确认第三轮问题已经关闭，又把 payload owner、pre-routing audit union、动态 snapshot Oracle 与跨 Attempt Satisfaction 四处歧义推到台前。四项已经按最小范围收口。2026-08-20 用户明确要求停止重复的文档审查循环并尽快进入代码阶段，因此不再发起第五次代码前独立复核；代码门从 **CLOSED** 改为 **CONTROLLED OPEN FOR TDD**。这不是把第四轮 FAIL 改写为 PASS，而是接受其发现后，以失败测试、数据库约束和真实副作用继续验证当前合同。任何新的产品含义或未获授权范围仍不得进入实现。

## 入口与用户合同

### API routes

| Route | 用途 | 同步结果 | 明确不做 |
|---|---|---|---|
| `POST /v1/capture/packages` | 为服务端已存在的 Attempt 提交一个终态 Package；首次接纳要求当前 authority，既有合法同 hash replay 按幂等规则处理 | accepted/replay/conflict 或 routing-confirmed rejected delivery；accepted 时 Package、Record、Coverage 与 processing work 同事务成立 | 不创建 Work Order/Attempt，不接受客户端自授予 authority，不运行 Record 解析 |
| `GET /v1/capture/deliveries/{delivery_ref}` | 查询一次已经形成可查询交付的结果 | 该 delivery 的 outcome、关联的权威 accepted receipt、Coverage 与处理水位 | 不把 delivery、Package accepted 或 replay 写成对象已观察完成；pre-routing safe error 没有可查询 delivery route |
| `GET /v1/processing/{processing_work_ref}` | 查询一个 Record processing work | `ready/leased/finalized` 运行水位与独立业务结果 | 不把运行完成写成 Observation、市场或研究成功；本切片不预设未证明的 stopped/dead-letter 策略 |
| `GET /v1/contents/{content_ref}` | 读取已经发布 Current revision 的 Content | 有来源的 title/body、版本、水位和限制；Content 不存在或 current pointer 为空都返回同一个安全 404 | 不返回数据库自增 ID 或原始任意 payload，不泄露“对象存在但尚无 Current” |
| `GET /v1/contents/{content_ref}/explain` | 解释已经发布的 Current | 字段来源 Observation/Record/Package、时间、Coverage、policy、applicability；没有 Current 时同样安全 404 | 不生成 AI 总结或趋势结论 |

普通 API 不提供“创建 synthetic Work Order/Attempt”route。集成测试通过专用 test harness 在 proof database 预置获准目标；正常运行二进制不编译或暴露这个入口，“正常运行”不等于已经生产部署。

Ingress delivery 的 outcome 使用封闭词表 `accepted | replay | conflict | rejected`。`conflict` 只表示同一 Capture Identity 已存在另一份合法 hash；routing-confirmed `rejected` delivery 必须带下列一个封闭 `rejectionCode`。pre-routing safe error 使用同一列表中的 `externalIngressCode` 作为 `error.code`，但不伪造 delivery `rejectionCode`。两种字段共享封闭词表和优先级，不共享资源语义，不能用自由字符串代替程序判断：

```text
unauthenticated
forbidden
body_limit_exceeded
malformed_json
canonicalization_invalid
package_schema_invalid
package_hash_invalid
record_hash_invalid
routing_reference_not_found
work_attempt_mismatch
attempt_capture_mismatch
lease_epoch_mismatch
target_mismatch
authority_revoked
authority_expired
```

具体 parser 错误可写入受控内部诊断，但不能扩张外部业务枚举、泄露凭据或让 `rejected` 自动终止 Attempt。无法归入上述代码的新失败先停止受影响入口并修订合同。

#### Ingress 唯一判定顺序

每个请求只能返回下表中最先命中的结果；实现不得交换顺序。拒绝响应的 HTTP status、外部 code 和副作用也是合同一部分。步骤 1–11 属于 `pre_routing_error`：服务端还没有得到一条可信且同属的 Work/Attempt/Capture 链，只写一行不关联这些业务对象、也不保存 token/body 的最小 `capture_ingress_delivery` 审计，并返回安全 error envelope。步骤 12–14、16 属于 `routing_confirmed_rejection`：同属链已成立，才可以产生可查询 delivery 与 linked rejected response。步骤 15、17 分别产生 replay/conflict delivery 或 accepted delivery。

| 顺序 | 检查 | 唯一结果 | HTTP | 业务副作用 |
|---:|---|---|---:|---|
| 1 | 缺失或无效身份凭据 | `unauthenticated` | 401 | 必须写 1 行 unlinked delivery audit；其他正式表 0 |
| 2 | 身份有效但缺少 route 权限 | `forbidden` | 403 | 必须写 1 行 unlinked delivery audit；不泄露资源存在性 |
| 3 | HTTP body 超过 1,048,576 bytes | `body_limit_exceeded` | 413 | 必须写 1 行 unlinked delivery audit；不得继续解析 body |
| 4 | 请求不是 RFC 8259 合法 JSON、JSON 被截断或无法完成 JSON 语法解析；`NaN`、`Infinity` 和 `-Infinity` 固定属于本步 | `malformed_json` | 400 | 必须写 1 行 unlinked delivery audit |
| 5 | JSON 语法可解析，但出现重复 key、unpaired surrogate/非法 Unicode scalar或超出 JavaScript 安全整数范围的整数 | `canonicalization_invalid` | 422 | 必须写 1 行 unlinked delivery audit |
| 6 | 已通过第 5 步的唯一 JSON 对象不符合 Package/Record **接入 envelope**：Package、knownTargetResults、Record envelope 或 `source/observedAt` 缺字段、类型/枚举错误、未声明字段、数组关系非法；`payload` 在本步只要求是受资源上限约束的 JSON object，其内部 key、schemaVersion、fields 及值形状由接纳后的逐 Record processor 唯一负责；接入 envelope 没有浮点字段，因此 envelope 中任一浮点 token 固定属于本步；或 Record/target/result 任一数组超过 100 项、任一 JSON 字符串超过 65,536 UTF-8 bytes、JSON 深度超过 16 | `package_schema_invalid` | 422 | 必须写 1 行 unlinked delivery audit |
| 7 | 服务端重算 package hash 不一致 | `package_hash_invalid` | 422 | 必须写 1 行 unlinked delivery audit |
| 8 | 任一 Record hash 重算不一致 | `record_hash_invalid` | 422 | 必须写 1 行 unlinked delivery audit |
| 9 | `workOrderRef` 或 `attemptRef` 是合法 UUID，但服务端没有对应对象 | `routing_reference_not_found` | 404 | 必须写 1 行 unlinked delivery audit；固定消息不说明哪个引用缺失 |
| 10 | Work 与 Attempt 均存在但不同属 | `work_attempt_mismatch` | 409 | 必须写 1 行 unlinked delivery audit |
| 11 | Attempt 存在，但请求 Capture Identity 与该 Attempt 冻结值不同 | `attempt_capture_mismatch` | 409 | 必须写 1 行 unlinked delivery audit |
| 12 | 提交 epoch 不是 Attempt 当前获准 epoch | `lease_epoch_mismatch` | 409 | linked rejected delivery；其余业务行不变 |
| 13 | target basis/manifest/member/result 与 Work 冻结目标不同 | `target_mismatch` | 409 | linked rejected delivery；其余业务行不变 |
| 14 | authority 已明确撤销 | `authority_revoked` | 409 | linked rejected delivery；其余业务行不变 |
| 15 | 上述 immutable routing tuple 全部合法，且已有 accepted Package | 同 hash 为 `replay`；不同合法 hash 为 `conflict` | 200 / 409 | 新增 delivery；原 Package/accepted receipt 与其余业务行不变 |
| 16 | 无 accepted Package，且数据库当前时间已超过 authority deadline | `authority_expired` | 409 | linked rejected delivery；其余业务行不变 |
| 17 | 全部通过 | `accepted` | 201 | 新增 accepted delivery，并只执行本 SCOPE 明列的原子接入 |

`authority_expired` 只阻止首次接纳；已有 accepted Package 的合法同 hash replay 不因时间过期失效。`authority_revoked` 是明确撤权，优先于 replay/conflict。请求同时存在多个错误时只报上表最早 code，内部诊断不得改变外部判定。数据库/事务在 commit 前发生内部故障不是 `rejected`，必须整体回滚并返回安全 `internal_error` 500；正式数据库行（包括 delivery）增加 0。若 commit 已成功而 HTTP 响应丢失，权威行保持已提交，调用者以同 hash 重试并得到 replay，不能回滚或重复写。

`capture_ingress_delivery` 的审计联合类型必须精确区分两类行，不能把 safe error 压成业务 `rejected`：

- `auditKind=pre_routing_error` 当且仅当步骤 1–11：`deliveryRef/workOrderRef/attemptRef/captureIdentity/outcome` 全为 null，`externalCode` 必须是步骤 1–11 对应 code；该行不可 GET，也不进入八责任；
- `auditKind=public_delivery` 当且仅当步骤 12–17：`deliveryRef/workOrderRef/attemptRef/captureIdentity/outcome` 全非 null并且同属；`outcome=rejected` 当且仅当 `externalCode` 为步骤 12–14 或 16 对应 code，`accepted|replay|conflict` 当且仅当 `externalCode=null`；该行才可以通过 delivery route 查询。

任何 `pre_routing_error + outcome`、`pre_routing_error + public/routing ref`、`public_delivery + null outcome/ref`、`rejected + null code` 或非 rejected outcome + code 的组合都必须在数据库和 response adapter 负例中失败。`internal_error` 没有 delivery audit 行；安全请求日志不属于正式 delivery 表。

Capture Identity 不是独立可查询资源，而是 Attempt 上冻结的执行身份；因此不存在第三种“Capture Identity 对象不存在”。Attempt 存在而值不同唯一命中 `attempt_capture_mismatch`。SCOPE-001 的 Work 与 Package 合同都只能是 `content-detail.synthetic.v1`：任何其他 `contractVersion` 在第 6 步唯一命中 `package_schema_invalid`；v1 不保留不可构造的 `contract_mismatch` 分支。

`payload` 的责任边界同样是 Closed World：ingress 只验证它存在、是 JSON object、参与 Record canonical/hash 且不突破统一资源上限，不枚举或拒绝它的内部 key。只有 `content-detail-processor-v1` 可以判断 payload 内部 `schemaVersion/sourceExternalId/fields` 及未知字段；因此 payload 未声明字段唯一形成 accepted Package 中该 Record 的 `record_contract_invalid`，绝不回跳为 ingress `package_schema_invalid`。Package、Record envelope、`source`、`observedAt` 或 knownTargetResults 的未声明字段仍唯一属于第 6 步。

### CLI commands

新增一个极小 `apps/cli`，二进制名为 `linggan`：

```text
linggan content get <content-ref> [--json]
linggan content explain <content-ref> [--json]
linggan processing status <processing-work-ref> [--json]
```

CLI 只通过版本化 HTTP API 访问。它不能接收数据库 DSN，不能执行任意 SQL，不能提交 Package，不能创建 Work Order/Attempt，也不能绕过用途和访问检查。人类输出与 JSON 使用同一 response model，不分别计算一套事实。

### 最小本地认证与权限

本切片不建设完整 Agent Delegation，但网络 API 不能以“本地开发”为由裸奔：

- 默认只绑定 `127.0.0.1`；非 loopback 监听不在本 SCOPE；
- test harness 每次运行生成临时 secret，只进入进程环境或私有临时文件，不提交 Git、不写普通日志；
- 权限固定为 `scope-001:submit-synthetic-package`、`scope-001:read-synthetic-content`、`scope-001:read-processing`；route 唯一映射见下表；
- CLI 从环境或受控运行时配置取得 API URL 与临时凭据，不接受 DSN，也不把凭据写入命令输出；
- submit、delivery、content、explain、processing 分别有无认证和错权限负例；失败时不泄露资源是否存在，不产生 Package/Record/processing work，只允许不含 token/body 的最小安全审计；
- response 中的 `permissions` 只说明本地合成 proof 权限，不宣称完整外部 Agent 委托已经实现。

| Route | 必需权限 | 边界 |
|---|---|---|
| `POST /v1/capture/packages` | `scope-001:submit-synthetic-package` | 提交并读取该交付的当次 delivery；pre-routing error 不产生可查询 delivery ref |
| `GET /v1/capture/deliveries/{delivery_ref}` | `scope-001:submit-synthetic-package` | 只查询调用者有权提交、且已形成 public delivery ref 的单次交付及其 Record 处理水位 |
| `GET /v1/processing/{processing_work_ref}` | `scope-001:read-processing` | 查询单个 processing work，不因关联 Content 而自动获得内容读权 |
| `GET /v1/contents/{content_ref}` | `scope-001:read-synthetic-content` | 可返回当前字段及支撑该字段的 processing 水位，不暴露任意 processing work |
| `GET /v1/contents/{content_ref}/explain` | `scope-001:read-synthetic-content` | 可返回当前字段的固定来源链；如需查其他 work 仍调 processing route |

### API/CLI envelope 与八责任外部表示

所有资源读取响应与 POST 的 linked delivery 响应必须且只能包含 `data/scope/asOf/versions/coverage/applicability/limitations/provenance/responsibilities/permissions` 十个顶层 key。这里不是允许 `{}` 或任意 JSON 的空壳 schema；每个字段、基数和顺序由下文唯一规定。pre-routing、404 与 internal failure 使用后文独立的 safe error envelope，不伪造这十个 key。

#### applicability

`applicability` 精确为 `{allowed:[<closed-use>],notEstablished:[<closed-limit>]}`，不写回 Evidence 永久属性。数组严格按下列词表顺序输出，不按数据库或插入顺序：

```text
allowed
1. inspect_synthetic_delivery
2. inspect_synthetic_observed_content
3. verify_synthetic_provenance
4. inspect_synthetic_processing

notEstablished（每个成功 envelope 都完整返回以下六项）
1. real_platform_observation
2. source_completeness
3. representativeness
4. market_trend
5. user_need
6. market_opportunity
```

| response scope | `allowed` 精确成员 |
|---|---|
| linked delivery，无 accepted Package | `[inspect_synthetic_delivery]` |
| linked delivery，有 accepted Package | `[inspect_synthetic_delivery,verify_synthetic_provenance,inspect_synthetic_processing]` |
| processing | `[verify_synthetic_provenance,inspect_synthetic_processing]` |
| content / explain | `[inspect_synthetic_observed_content,verify_synthetic_provenance]` |

#### responsibilities

`responsibilities` 必须且只能包含以下八个 camelCase 机器 key：`ingressDelivery/packageAcceptance/attemptTerminal/captureSatisfaction/recordProcessingRuntime/recordProcessingBusinessOutcome/observationFormation/currentFieldResolution`。每个 key 只能是 `{applicability:"applicable",entries:[...]}`，或 `{applicability:"not_applicable",reason:<closed-reason>,entries:[]}`。closed reason 只允许 `no_record_in_response_scope | no_current_revision_in_response_scope | current_not_in_response_scope`。

| machine key | entry 唯一结构 | 稳定排序 |
|---|---|---|
| `ingressDelivery` | `{deliveryRef,state:accepted|replay|conflict|rejected,rejectionCode:null|<closed-code>}`；只有 rejected 必须带 code | `deliveryRef` |
| `packageAcceptance` | `{state:accepted|not_created,packageRef:<ref>|null,acceptedReceiptRef:<ref>|null}`；两个 ref 同时有值或同时为 null | `acceptedReceiptRef`；not_created 唯一 entry 在后 |
| `attemptTerminal` | `{attemptRef,state:open|terminal,reason:null|target_reached|risk_control}` | `attemptRef` |
| `captureSatisfaction` | `{workOrderRef,state:not_evaluated|satisfied|known_gap|unknown}` | `workOrderRef` |
| `recordProcessingRuntime` | `{processingWorkRef,recordRef,state:ready|leased|finalized,epoch:<integer>}` | `recordRef`，再 `processingWorkRef` |
| `recordProcessingBusinessOutcome` | `{processingWorkRef,recordRef,state:not_evaluated|observation_recorded|source_identity_unresolved|source_identity_conflict|record_contract_invalid}` | `recordRef`，再 `processingWorkRef` |
| `observationFormation` | `{recordRef,state:not_evaluated|formed|not_formed,observationRef:<ref>|null,reason:null|source_identity_unresolved|source_identity_conflict|record_contract_invalid}` | `recordRef` |
| `currentFieldResolution` | `{contentRef,revisionRef,field:title|body,state:selected|unknown|unresolved,value:<string>|null,sourceObservationRefs:[<ref>]}` | `contentRef`，同 Content 固定 title 后 body；来源 ref 字典序 |

上述 entry 还必须满足联合约束，不能只让字段各自通过类型校验：`ingressDelivery.state=rejected` 当且仅当 `rejectionCode` 为前文封闭 code，其他三个 state 的 `rejectionCode` 必须为 null；`packageAcceptance.state=accepted` 当且仅当两个 ref 均非 null，`not_created` 当且仅当两个 ref 均为 null；`attemptTerminal.state=open` 当且仅当 `reason=null`，`terminal` 当且仅当 reason 为 `target_reached|risk_control`；Current `selected` 必须有非 null value 和至少一个 sourceObservationRef，`unknown` 必须是 `value=null/sourceObservationRefs=[]`，`unresolved` 必须是 `value=null` 且至少两个来源。空字符串仍是一个已观察 string，不等于 null。任何不符合联合约束的组合都是合同错误，不得由 adapter 猜测修复。

response scope 不是 adapter 自由选择的“相关数据”。它的成员固定为：linked delivery 只含当前查询/提交的 delivery，并可沿其 routing chain 读取一个 Attempt/Work 和当时已有的唯一权威 Package；processing 只含该 processing work、它的 Record、权威 Package、原 accepted delivery、Attempt/Work 及该 Record 所属 Content；content/explain 只含当前 revision watermark 中明确冻结的 Package、每个 Package 的原 accepted delivery、同属 Attempt/Work、全部输入 Record/Observation 和被发布的唯一 Content revision，不纳入后来 replay/conflict/rejected delivery。设上述固定范围中去重后的 delivery、Package、Attempt、Capture Work Order、Record、已发布 Current Content 数分别为 `D/P/A/W/R/C`；因此 content/explain 中 `D=P`，不能把该 Capture Identity 的全部交付历史扩入解释结果：

| response scope | 前四项 entries | Runtime / Business / Formation | Current Field |
|---|---|---|---|
| linked delivery，无 accepted Package | 各恰好 1；Package 为 not_created，Attempt=open，Satisfaction=not_evaluated | 三项均 not-applicable/no_record | not-applicable/current_not_in_response_scope |
| linked delivery，有 accepted Package（包括 rejected delivery 发生在已有 Package 之后） | 各恰好 1；Package/Attempt/Satisfaction 返回当前权威真值，不能被本次 delivery 改写 | 各恰好 `R` | not-applicable/current_not_in_response_scope |
| processing | 各恰好 1 | 各恰好 1 | not-applicable/current_not_in_response_scope |
| content / explain | Ingress=`D`、Package=`P`、Attempt=`A`、Satisfaction=`W`，各自按 ref 去重 | 对 watermark 的唯一 Record 各 1 条，共 `R` | 恰好 2 |

`not_applicable/no_record` 是上表 `no_record_in_response_scope` 的人类简写，`not_applicable/no_current_revision` 同理；`current_not_in_response_scope` 表示该 route 有意不承担 Current 读取责任。只有整个 response scope 的 Record 数为 0 时，Runtime/Business/Formation wrapper 才能 not-applicable；不得给某个无 Record target 伪造局部 Formation entry。没有已发布 revision 不伪造 Current unknown。F02 第三个冻结目标没有 Record，因此不会在 Formation 中产生 entry 或局部 N/A；该目标的缺口只由 target result、Coverage 和 Capture Satisfaction 表达。

#### runtime/business/formation 联合真值

`epoch` 表示该 processing work 已签发的最新 epoch，初始值为 0，永不为 null：

| runtime | epoch | business | formation state / observationRef / reason |
|---|---:|---|---|
| 初始 ready | 0 | not_evaluated | `not_evaluated / null / null` |
| lease 过期后的 ready | 最近已签发值，≥1 | not_evaluated | `not_evaluated / null / null` |
| leased | 当前有效值，≥1 | not_evaluated | `not_evaluated / null / null` |
| finalized | 完成该结果的值，≥1 | observation_recorded | `formed / <ref> / null` |
| finalized | 完成该结果的值，≥1 | source_identity_unresolved | `not_formed / null / source_identity_unresolved` |
| finalized | 完成该结果的值，≥1 | source_identity_conflict | `not_formed / null / source_identity_conflict` |
| finalized | 完成该结果的值，≥1 | record_contract_invalid | `not_formed / null / record_contract_invalid` |

#### data、时间、版本与 Coverage

- delivery data 精确为 `{delivery:{deliveryRef,deliveryOutcome,rejectionCode,captureIdentity,acceptedReceiptRef,authoritativePackageRef}}`。accepted/replay/conflict 均引用权威 Package 的原 `acceptedReceiptRef` 与 `authoritativePackageRef`；linked rejected 在尚无 Package 时两者为 null，在已有 Package 时返回既有两者。每次请求的 `deliveryRef` 永远不同，replay 只复用 acceptedReceiptRef，不复用 deliveryRef。`originalAcceptedDeliveryRef` 是创建该权威 Package 的首次 accepted delivery 的 `deliveryRef`，是与 `acceptedReceiptRef` 不同的强类型引用；它只在 provenance/watermark 出现，不用当前 replay/conflict/rejected delivery 替代。
- processing data 精确为 `{processing:{processingWorkRef,recordRef}}`；content 为 `{content:{contentRef,revisionRef,fields:[<两条 Current Field entry>]}}`。explain 必须且只能同时包含前述 content 与 `{explain:{policyVersion,watermark,fieldSources:[<两条 current_field provenance>]}}`；fieldSources 固定 title 后 body，并与顶层 provenance 中同 field 的两条 current_field entry 逐字段相同。
- `watermark` 精确为 `{packages:[{packageRef,originalAcceptedDeliveryRef,acceptedReceiptRef,recordRefs:[<ref>],observationRefs:[<ref>]}]}`。packages 按 packageRef 排序，每个 ref 数组去重后字典序；成员必须且只能是该 Current revision 重算时的完整输入 Package、创建它的首次 accepted delivery/receipt、输入 Record 和由这些 Record 形成的 Observation。每个 Record/Observation 恰好归入自己的 Package entry；不得加入 replay/conflict/rejected delivery、后来新增事实或只与 Content“看起来相关”的材料。unknown 字段也保留完整 revision watermark，用于证明规则检查过哪些输入，而不是伪造字段支持来源。
- `scope` 精确为 `{"scopeId":"scope-001","dataClass":"synthetic","contract":"content-detail.synthetic.v1"}`。
- `asOf` 精确包含 `readAt/receivedAt/acceptedAt/observedAt`。`readAt` 是 RFC3339；其他三项均为 `{applicability:"applicable",entries:[...]}` 或 `{applicability:"not_applicable",reason:<closed-time-reason>,entries:[]}`。received entries 为 `{deliveryRef,state:"known",value:<RFC3339>}`；accepted entries 为 `{packageRef,acceptedReceiptRef,state:"known",value:<RFC3339>}`；observed entries 为 `{observationRef,state:"known",value:<RFC3339>}`。closed time reason 只允许 `no_delivery_in_response_scope | no_accepted_package_in_response_scope | no_observation_in_response_scope`。三类 entries 各按自身 ref 字典序；content/explain 使用 revision watermark 中每个 Package 的原 accepted delivery，不使用 replay/conflict 时间。F07/F08 必须各返回两个带来源 ref 的 received/accepted/observed entries，不能折成最早、最晚或单值。
- `versions` 精确为 `{api:"v1",capturePackage:"capture.package.v1",processor:"content-detail-processor-v1",currentPolicy:{state:"known"|"not_applicable",value:"content-current-policy-v1"|null}}`；只有 content/explain 的已发布 Current 返回 known/value，delivery/processing 以及无 Current 时必须 not_applicable/null。
- `coverage` 有 Package 时为 applicable wrapper，每个 entry 精确为 `{packageRef,unit,attempted,emitted,failed,knownNotAttempted,remainingScope}`，值来自不可变 `capture_package_coverage`，按 packageRef 排序；无 Package 时为 `{applicability:"not_applicable",reason:"no_accepted_package",entries:[]}`。
- `limitations` 每个成功 envelope 都必须按固定顺序完整返回 `[synthetic_fixture_only,no_real_platform_observation,no_source_completeness,no_representativeness,no_market_claims]`。

`asOf` 的 route 基数不允许实现自行聚合：

| response scope | receivedAt | acceptedAt | observedAt |
|---|---|---|---|
| linked delivery | 当前 delivery 恰好 1 条 | 有 Package 时恰好 1 条，否则 not-applicable | 该 Package 在查询时已形成的每个 Observation 各 1 条，0 条时 not-applicable |
| processing | Package 原 accepted delivery 恰好 1 条 | 恰好 1 条 | 本 Record formed 时恰好 1 条，否则 not-applicable |
| content / explain | watermark 每个 Package 的原 accepted delivery，各 1 条 | watermark 每个 Package 各 1 条 | watermark 每个 Observation 各 1 条 |

#### provenance

`provenance` 只允许：`package_record`=`{kind:"package_record",originalAcceptedDeliveryRef,packageRef,recordRef,contractVersion}`；`current_field`=`{kind:"current_field",contentRef,field,revisionRef,observationRefs,recordRefs,packageRefs,policyVersion}`。所有 ref 数组去重后字典序固定，不返回数据库自增 ID。

`current_field` 的三个 ref 数组不是自由摘要，必须按字段状态唯一生成：

| field state | observationRefs | recordRefs | packageRefs |
|---|---|---|---|
| selected | 恰好为该 revision/field 的全部 `selected_support` Observation | 由 observationRefs 反向得到的唯一 Record | 由 recordRefs 反向得到的唯一 Package |
| unresolved | 恰好为该 revision/field 的全部 `conflicting_candidate` Observation | 由 observationRefs 反向得到的唯一 Record | 由 recordRefs 反向得到的唯一 Package |
| unknown | `[]` | `[]` | `[]` |

unknown 不表示没有输入；完整输入只在 explain watermark 和前置 package_record provenance 中表达。F10 unknown title 的 current_field 必须精确为 `{kind:"current_field",contentRef:<existing-content>,field:"title",revisionRef:<revision>,observationRefs:[],recordRefs:[],packageRefs:[],policyVersion:"content-current-policy-v1"}`；其 explain watermark 仍含该 Package、原 accepted delivery/receipt、Record 和 `fields.title.observed=false` 的 Observation。body 的 current_field 则按 selected 规则返回一组来源。

| response scope | provenance 精确集合与顺序 |
|---|---|
| linked delivery，无 Package | `[]` |
| linked delivery，有 Package | 按 recordRef 返回恰好 `R` 条 package_record；不返回 current_field |
| processing | 恰好 1 条 package_record；不返回 current_field |
| content / explain | 先返回 watermark 每个唯一 Record 的 package_record，再返回恰好 2 条 current_field；分别按上述 ref/field 顺序 |

#### permissions 与 safe error

成功 envelope 的 `permissions` 精确为 `{"required":["<one-route-permission>"],"satisfied":true}`。pre-routing error、401/403、404 与 commit 前 internal failure 只能返回 `error/scope/permissions` 三个顶层 key，不返回八责任、资源 ref、provenance 或 Coverage。

| HTTP | closed error code | fixed message | permissions.satisfied | 正式 delivery 行 |
|---:|---|---|---|---:|
| 401 | `unauthenticated` | `Authentication required` | false | 1，unlinked |
| 403 | `forbidden` | `Permission denied` | false | 1，unlinked |
| 413 | `body_limit_exceeded` | `Request rejected` | true | 1，unlinked |
| 400 | `malformed_json` | `Request rejected` | true | 1，unlinked |
| 404/422/409（POST pre-routing） | 对应步骤 5–11 的 externalIngressCode | `Request rejected` | true | 1，unlinked pre_routing_error audit |
| 404（GET） | `not_found` | `Resource not found` | true | 0 |
| 500 | `internal_error` | `Internal error` | true | 0；事务内正式行全部回滚 |

error 顶层结构固定为 `{error:{code,message},scope:{scopeId:"scope-001",dataClass:"synthetic",contract:"content-detail.synthetic.v1"},permissions:{required:[<one-route-permission>],satisfied:<bool>}}`。unlinked pre-routing audit 只保存 server request ref、`auditKind=pre_routing_error`、externalCode、时间和不敏感调用者类别，不保存/关联 Work、Attempt、Capture Identity、outcome、任意 body、token 或客户端可查询 deliveryRef。步骤 12–14、16 不使用 safe error envelope，而使用十 key linked delivery envelope，其 ingressDelivery entry 唯一为 `state=rejected/rejectionCode=<对应 code>`。

#### CLI 唯一人类表达

CLI 只将同一 JSON model 映射为下列固定词，不重新判断：

| 结构值 | 人类输出 |
|---|---|
| `not_evaluated` | `尚未评估` |
| `not_applicable/no_record_in_response_scope` | `不适用（本次范围内没有 Record）` |
| `not_applicable/no_current_revision_in_response_scope` | `不适用（尚无 Current revision）` |
| `not_applicable/current_not_in_response_scope` | `不适用（该命令不读取 Current）` |
| Runtime `ready` | `待处理` |
| Runtime `leased` | `处理中` |
| Runtime `finalized` | `处理结果已固定` |
| Business `not_evaluated` | `尚未评估` |
| Business `observation_recorded` | `Observation 已记录` |
| Business `source_identity_unresolved` | `来源身份未解析` |
| Business `source_identity_conflict` | `来源身份冲突` |
| Business `record_contract_invalid` | `Record 合同无效` |
| Attempt `open` | `执行尚无终态 Package` |
| Attempt `terminal(target_reached)` | `执行终止（达到冻结目标）` |
| Attempt `terminal(risk_control)` | `执行终止（风险控制）` |
| Satisfaction `satisfied` | `采集目标已满足` |
| Satisfaction `known_gap` | `存在已知缺口` |
| Satisfaction `unknown` | `剩余范围未知` |
| Current `selected` | `已选择` |
| Current `unknown` | `未知（没有合格 Observation 观察到该字段）` |
| Current `unresolved` | `无法裁定（最新同一观察时刻存在冲突）` |

F10 中明确未观察 title 的 JSON 必须是 `{"field":"title","state":"unknown","value":null,"sourceObservationRefs":[]}`，CLI 必须显示上表的“未知”；不允许“无标题”、空字符串或字段缺省。`--json` 是唯一机器协议，必须做 exact snapshot。human mode 不构成第二协议：三个命令都按 `命令对象 → scope/limitations → data → 八责任 → coverage/asOf → provenance → permissions` 固定栏目顺序映射同一 JSON，数组顺序不变；safe error 只按 `错误 code/message → scope → required permission/satisfied` 输出。human 测试固定栏目顺序、上述状态词、全部 ref/时间/Coverage/provenance/permission 是否出现和所有禁止词，不冻结空格、列宽、ANSI color 或终端换行 snapshot。F01 主链 manifest 必须先固定其实际 route 的 exact JSON 和 human 语义 Oracle；F02–F10 在各自硬化场景开启时固定同类 Oracle，测试不得由实现反向生成预期。

#### proof harness 的确定性时间与引用

exact snapshot 不允许删除动态字段、使用正则放宽或由被测实现回写期望。F01 主链的手工 `manifest.json` 必须为其每个 ordered step 明列：固定 `proofNow`、每一个将由服务端创建的 public UUID/ref（delivery、accepted receipt、Package、Record、processing work、Source、Content、Observation、revision）以及完整 expected JSON。F02–F10 在各自硬化场景开启时遵守同一规则；它们不阻塞 F01 主链。test-only `ProofClock` 和按类型分组的 `ProofRefSource` 只能逐项消费当前场景的手工值；缺值、多消费、少消费、类型用错或两个应不同的 ref 相等都立即失败。数组仍按实际 ref 字典序，跨响应相同实体必须复用同一个 manifest ref。

proof database 的时间函数固定为 `scope_001_now()`：普通运行角色唯一实现为数据库 `clock_timestamp()`；只有专用、不可用于正常二进制的 proof test role 可以在事务内设置 manifest 的 `proofNow`，authority deadline、receivedAt、acceptedAt、readAt 和数据库时间列都从该函数读取。observedAt 仍来自手工 fixture。测试必须同时断言时间列与 response 完全一致、ordered step 的先后关系符合 manifest；proof override 不编译进或授权给正常 API/worker/CLI 运行角色。这样 exact snapshot 比较完整 JSON 原值，不做 placeholder normalization，也不让 Rust 和数据库各自产生两套时钟。

## Capture Package v1 合同

### 唯一来源

本 SCOPE 是代码前语义的唯一权威来源。实施阶段的 `docs/data-contracts/capture-package-v1.md` 和手工 fixture 只能按本文落地为机器/人可读合同，不得另行发明语义；三者冲突时停止实现并先修订本 SCOPE。`references/` 中的 V2 合同只用于对照，不自动生成新 fixture，不成为隐形上游。

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

上例是 `known_set` variant；`maximum_quota` variant 必须整个缺省 `knownTargetResults` key，不允许空数组代替 variant 选择。两个 variant 的其余顶层 key 必须且只能是上例列出的成员；`terminal` 必须精确为 `{"reason":"target_reached"}` 或 `{"reason":"risk_control"}`。服务端生成 `workOrderRef`、`attemptRef`、`captureIdentity` 和 `leaseEpoch`，producer 只能回传，不能自行创建或更换；前三者使用 canonical lowercase UUID 词法，`leaseEpoch` 是正整数。

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

Record v1 是 Closed World，不是“至少包含”的扩展对象。每个成员必须且只能包含下列字段：

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

Package 最小硬门只验证：authority、Attempt/Capture/epoch、合同与目标、canonical/hash、manifest/ordinal/成员边界，以及 Record envelope 能被安全枚举和重放。`payload.sourceExternalId`、title/body 等来源内容语义在接纳后逐 Record 解析；一条 Record 的来源关系错误不得回滚其他成员。

#### 三种身份陈述的唯一权责

| 字段 | 唯一责任 | 能否单独创建 Source Identity |
|---|---|---|
| `targetExternalId` | producer 对“这条 Record 交付哪个执行目标”的陈述；known-set ingress 必须对上冻结 target | 否 |
| `source.externalId` | Record envelope 对平台稳定对象身份的权威候选陈述；字段必须存在，可为 null | 只有它非 null 且通过下表一致性检查后才可 |
| `payload.sourceExternalId` | payload 内部对自己主体身份的交叉校验陈述；字段必须存在，可为 null | 否，不得 fallback 补齐 `source.externalId` |

Record parser 严格按下表裁定，不存在其他优先级：

| `targetExternalId` | `source.externalId` | `payload.sourceExternalId` | 唯一 business outcome |
|---|---|---|---|
| A | A | A | 继续解析；合格时 `observation_recorded` |
| A | A | null | 继续解析；payload 没有交叉校验值，但不得抹掉 envelope 的权威候选；合格时 `observation_recorded` |
| A | A | B，且 B ≠ A | `source_identity_conflict` |
| A | null | null | `source_identity_unresolved` |
| A | null | A | `source_identity_conflict`；payload 不得 fallback 创建身份 |
| A | null | B，且 B ≠ A | `source_identity_conflict` |
| A | B，且 B ≠ A | null | `source_identity_conflict` |
| A | B，且 B ≠ A | A | `source_identity_conflict` |
| A | B，且 B ≠ A | B | `source_identity_conflict` |
| A | B，且 B ≠ A | C，且 C ≠ A、C ≠ B | `source_identity_conflict` |

F10 的 unresolved Record 必须固定 `targetExternalId="synthetic-note-unresolved"`、`source.externalId=null`、`payload.sourceExternalId=null`；不得从 target 或 payload 补建 Source Identity，也不得把这种明示 nullable 合同判为 `record_contract_invalid`。F06A 必须使用非 null 但不一致的三种陈述产生 `source_identity_conflict`。

#### payload 字段合同

- `ordinal` 必须是 1–100 的整数并在 Package 内唯一；`recordKind` 必须且只能是 `content_detail`；`targetExternalId` 必须是非空 string；
- `source` 必须且只能包含 `system/namespace/objectType/externalId/channel`；`system=synthetic`、`namespace=scope-001`、`objectType=content`、`channel=synthetic_page`，`externalId` 必须存在且只能是 string 或 JSON null；
- `observedAt` 必须且只能包含 `value/precision/basis`；`value` 是固定 UTC RFC3339 词法，`precision=exact`、`basis=fixture`；
- `recordHash` 必须是小写 `sha256:` 加 64 位小写十六进制字符；
- `schemaVersion` 必须且只能是 `content-detail.synthetic.v1`；
- `sourceExternalId` 必须存在，值只能是 string 或 JSON null；
- `fields` 必须且只能包含 `title` 和 `body`；
- 每个字段对象必须且只能包含 `observed` 和 `value`；`observed=true` 时 `value` 必须是 string，`observed=false` 时 `value` 必须是 JSON null；其他组合均为 `record_contract_invalid`；
- `knownTargetResults.reason` 不是自由字符串：`emitted` 必须 `reason=null` 且有 `recordOrdinal`；`failed` 必须 `reason=page_error`且 `recordOrdinal=null`；`not_attempted` 必须 `reason=risk_control`且 `recordOrdinal=null`。

F06B 在同一 fresh seed 上分成三个独立 mutation：未知 `schemaVersion`、非法 `observed/value` 组合、以及 payload 未声明字段。三者都必须唯一得到 `record_contract_invalid`，且不影响同 Package 的合格 Record。

### Canonical 与 hash

- 使用能够在丢弃原始 token 前报告 duplicate key 和 Unicode 问题的 lossless JSON reader，再使用 RFC 8785 JSON Canonicalization Scheme，并把 v1 输入限制为 I-JSON。错误集合严格互斥：RFC 8259 语法外的 `NaN/Infinity/-Infinity` 是 `malformed_json`；语法合法但含 duplicate key、unpaired surrogate/非法 Unicode scalar 或超安全整数的是 `canonicalization_invalid`；接入 envelope 没有浮点字段，因此其中合法 JSON 浮点 token、未声明字段和字段类型错误是 `package_schema_invalid`；payload subtree 的未声明字段和字段合同错误只由 processor 产生 `record_contract_invalid`。字符串不做 Unicode normalization，原始码点序列就是合同语义；UUID、时间和 hash 使用固定词法形式；
- `recordHash` 由去除 `recordHash` 后的完整 Record canonical bytes 计算 SHA-256；
- `packageHash` 由去除 `packageHash` 后的完整 Package canonical bytes 计算 SHA-256；
- 服务端必须重算，不能信任客户端提供值；
- 只有 Work/Attempt/Capture/epoch/固定 v1 contract/target 全部合法且 authority 未明确撤销，才进入幂等判定：同 Capture Identity + 同 package hash 产生新的 replay delivery，并返回原权威 acceptedReceiptRef；同 Capture Identity + 不同合法 hash 产生新的 conflict delivery，旧 Package 与 acceptedReceiptRef 不变；
- 实现若需要更换 canonical library，必须先证明所有 golden fixture 字节与 hash 不变，否则发布新合同版本，不能静默 fallback。
- golden fixture 保存预先固定的 canonical UTF-8 bytes 或稳定文本表示和 expected hash，测试启动时不得由被测 Rust 函数生成期望值；同时使用 RFC 8785 官方向量和一个独立 Node/reference checker 复核。expected bytes/hash 是手工维护的权威测试输入，checker 只验证不生成或回写 fixture；如果未来引入任何生成型 fixture，必须先登记到 `docs/governance/generated-artifacts-registry.md`。该 checker 只证明合成合同，未来真实 TypeScript producer 仍须重新做跨语言兼容验收；
- 服务端硬上限固定为：HTTP body 1,048,576 bytes，Record/target/result 任一数组 100 项，任一 JSON 字符串 65,536 UTF-8 bytes，JSON 嵌套深度 16；并同时受 Work Order target/quota 约束。NaN/Infinity、浮点、重复 key、lone surrogate、超安全整数、未知字段、错误 record/package hash、超限 Record 和超大 body 都有独立负例，唯一 externalIngressCode 按前文优先级表判定；duplicate key + schema 错误固定先命中第 5 步，浮点 + 未知字段固定命中第 6 步。

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
| F-03 replay | 同 identity/hash 产生新 deliveryRef 并返回原 acceptedReceiptRef；首次 accepted 后 authority 过期再重传同 hash，规则不变 | Package/Record/work/Observation 均不增加；只追加安全 replay delivery |
| F-04 conflict | 同 identity、不同 hash fail closed | 不覆盖旧 hash/payload，不创建第二 Package/Evidence |
| F-05 Ingress rejection mutation 族 | 从合法 base fixture 分别制造前文 15 个 externalIngressCode 的唯一输入：步骤 1–11 是没有 public Delivery 的 pre-routing safe error，步骤 12–14、16 是 linked rejected Delivery；每项 fresh seed 独立拒绝，多故障按固定优先级；另含 accepted seed 后的两类失败 | 不产生新 Package/Record/Observation；只追加一次对应种类的 delivery audit，既有权威 Package 真值不被失败交付改写 |
| F-06 单 Record 隔离 | 基础场景证明 source identity conflict；独立 mutation 证明 record contract invalid；其他成员正常 | 不回滚整包，不为坏 Record 建 Source Identity/Observation |
| F-07 迟到 Observation | 较早观察后接入，历史保留，Current 不倒退 | 不按 received/accepted/last-write 决定 Current |
| F-08 来源差异 | 同一 observed time 不同来源对 title/body 给出不同值，field resolution 为 unresolved | 不取最大、平均或最后写入；双方 Observation 均保留 |
| F-09 最大配额部分结果 | quota 100、取得 50、剩余范围 unknown | 不建 50 个未尝试对象，不显示 50% 完成或平台共 100 个 |
| F-10 混合 Record | 6 个 Record 原子接入；4 个新对象、1 个复用身份并形成新 Observation、1 个身份 unresolved；一个合格对象的 title 明确未观察 | 不等待全部解析才接入；不把身份复用记作 replay；不重复计数对象；不把未观察 title 变成空字符串 |

fixture 只含合成英文/中文短文本和虚构身份，不包含真实 ADHD、儿童、账号、Cookie、Token、DSN 或旧 Evidence。

### F01–F10 状态真值

`manifest.json` 必须把下表逐层冻结；不允许测试或实现自行推导另一种总状态。表中的 Capture Satisfaction 只评价 producer 对冻结采集目标的交付，不评价 Record 是否最终形成 Observation。

本表使用以下封闭表达：Ingress Delivery 只允许 `accepted/replay/conflict/rejected`；权威 Package 只允许“已存在 accepted Package”或“未创建”；Attempt 对读取者只允许 `open` 或 `terminal(target_reached|risk_control)`；`open` 只表示尚无终态 Package，不表示 authority 仍有效；processing runtime 只允许 `ready/leased/finalized`。Observation Formation 只是已存在 Record 的读取责任：`ready/leased` 时为 `not_evaluated`，finalized 后由业务结果唯一确定为 `formed/not_formed`；不另建数据库总状态。无 Record 时没有 Formation 实例，fixture/API 只能表达 `not_applicable(no_record)`。每个已发布 Current 字段只允许 `selected/unknown/unresolved`。

| Fixture | Ingress Delivery | 权威 Package | Attempt | Capture Satisfaction | Processing runtime / business outcome | Observation Formation | Current Field Resolution |
|---|---|---|---|---|---|---|---|
| F-01 | accepted | accepted | terminal(target_reached) | satisfied | finalized×2；`observation_recorded`×2 | formed×2 | 两个 Content 的 title/body 均 selected |
| F-02 | accepted | accepted | terminal(risk_control) | known_gap | finalized×2；`observation_recorded`×2 | formed×2；第 3 个目标没有 Record，因此没有 Formation entry | 两个 Content 的 title/body 均 selected |
| F-03 | replay（发生在首次 accepted 后） | 保持原 accepted | 保持 terminal(target_reached) | 保持 satisfied | 不创建新 work/attempt/outcome | 不新增 | 保持原 revision/pointer |
| F-04 | conflict（发生在首次 accepted 后） | 保持原 accepted | 保持 terminal(target_reached) | 保持 satisfied | 不创建新 work/attempt/outcome | 不新增 | 保持原 revision/pointer |
| F-05A fresh-seed pre-routing（步骤 1–11） | 无 public Ingress Delivery；safe error + 1 行 pre_routing_error audit | 未创建 | 未进入可信 routing scope，不返回 Attempt | 未进入可信 routing scope，不返回 Satisfaction | 无 processing work/outcome | 八责任不存在于 safe error envelope | 无 Content/Current |
| F-05B fresh-seed routing-confirmed rejection（步骤 12–14、16） | rejected | 未创建 | open | not_evaluated | 无 processing work/outcome | `not_applicable(no_record)`；无 Formation 实例 | 无 Content/Current |
| F-06A 身份冲突 | accepted | accepted | terminal(target_reached) | satisfied | finalized×2；`observation_recorded`×1；`source_identity_conflict`×1 | formed×1；not_formed×1 | 合格 Content 的 title/body selected |
| F-06B Record 合同错误 mutation | accepted | accepted | terminal(target_reached) | satisfied | finalized×2；`observation_recorded`×1；`record_contract_invalid`×1 | formed×1；not_formed×1 | 合格 Content 的 title/body selected |
| F-07 | 两个独立 Work 各 accepted 一次 | accepted×2 | 两个 Attempt 均 terminal(target_reached) | 两个 Work 均 satisfied | finalized×2；`observation_recorded`×2 | formed×2 | 第二次重算仍选择 observedAt 较新的第一次 Observation |
| F-08 | 两个独立 Work 各 accepted 一次 | accepted×2 | 两个 Attempt 均 terminal(target_reached) | 两个 Work 均 satisfied | finalized×2；`observation_recorded`×2 | formed×2 | 最新同一时刻存在不同值，title/body 均 unresolved |
| F-09 | accepted | accepted | terminal(risk_control) | unknown | finalized×50；`observation_recorded`×50 | formed×50 | 50 个 Content 的 title/body 均 selected；来源剩余范围仍 unknown |
| F-10 | accepted | accepted | terminal(target_reached) | satisfied | finalized×6；`observation_recorded`×5；`source_identity_unresolved`×1 | formed×5；not_formed×1 | 4 个 Content 的 title/body selected；第 5 个 Content 的 title unknown、body selected |

F-06A 与 F-06B 是同一 fixture 族中的两个独立 fresh-database mutation，不在同一个 Package 内同时运行。`record_contract_invalid` 仅用于：Record envelope 已通过 Package 最小硬门，但固定 `content-detail.synthetic.v1` payload 在逐 Record parser 中出现未知 payload schemaVersion、非法字段形状或违反已声明字段合同。Package 顶层 schema/contract 未知、任意顶层扩展字段或 Record envelope 无法安全枚举仍在 ingress 失败关闭，不能下沉到该 outcome。

### F01–F10 数据库副作用真值

下表记录每个场景全部 ordered steps 完成后的全部正式表权威行数；`delivery` 包括 replay/conflict/rejected 的最小安全审计，`Coverage` 指 `capture_package_coverage`，`proc attempt` 指 `record_processing_attempt`。F10 的 fresh seed 预置且只预置一个没有 Observation/Current 的 `source_identity + source_content`，用于证明身份复用；其他场景不预置业务对象。

| Fixture | Work / Attempt | Work target | delivery | Package | Record | target result | Coverage | proc work / attempt | Source Identity / Content | Observation | Current revision | field source |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| F-01 | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F-02 | 1 / 1 | 3 | 1 | 1 | 2 | 3 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F-03 | 1 / 1 | 2 | 2 | 1 | 2 | 2 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F-04 | 1 / 1 | 2 | 2 | 1 | 2 | 2 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F-05 | 见下方 mutation seed | 见下表 | 1 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F-06A | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 1 / 1 | 1 | 1 | 2 |
| F-06B（每个 mutation） | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 1 / 1 | 1 | 1 | 2 |
| F-07 | 2 / 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2 / 2 | 1 / 1 | 2 | 2 | 4 |
| F-08 | 2 / 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2 / 2 | 1 / 1 | 2 | 2 | 6 |
| F-09 | 1 / 1 | 0 | 1 | 1 | 50 | 0 | 1 | 50 / 50 | 50 / 50 | 50 | 50 | 100 |
| F-10 | 1 / 1 | 6 | 1 | 1 | 6 | 6 | 1 | 6 / 6 | 5 / 5 | 5 | 5 | 9 |

为避免实现者从 final total 反推事务边界，F01/F02/F06/F09/F10 的 fresh seed、accepted ingress delta、Record processing delta 与 final 必须逐行直接断言。下表列名与上表完全相同；`Δ` 只表示该阶段新增行：

| Fixture / stage | Work / Attempt | target | delivery | Package | Record | result | Coverage | proc work / attempt | Identity / Content | Observation | Current | field source |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| F01 fresh seed | 1 / 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F01 accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 2 | 2 | 1 | 2 / 0 | 0 / 0 | 0 | 0 | 0 |
| F01 all processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 2 | 2 / 2 | 2 | 2 | 4 |
| F01 final | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F02 fresh seed | 1 / 1 | 3 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F02 accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 2 | 3 | 1 | 2 / 0 | 0 / 0 | 0 | 0 | 0 |
| F02 all processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 2 | 2 / 2 | 2 | 2 | 4 |
| F02 final | 1 / 1 | 3 | 1 | 1 | 2 | 3 | 1 | 2 / 2 | 2 / 2 | 2 | 2 | 4 |
| F06A fresh seed | 1 / 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F06A accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 2 | 2 | 1 | 2 / 0 | 0 / 0 | 0 | 0 | 0 |
| F06A all processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 2 | 1 / 1 | 1 | 1 | 2 |
| F06A final | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 1 / 1 | 1 | 1 | 2 |
| F06B each mutation fresh seed | 1 / 1 | 2 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F06B accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 2 | 2 | 1 | 2 / 0 | 0 / 0 | 0 | 0 | 0 |
| F06B all processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 2 | 1 / 1 | 1 | 1 | 2 |
| F06B final | 1 / 1 | 2 | 1 | 1 | 2 | 2 | 1 | 2 / 2 | 1 / 1 | 1 | 1 | 2 |
| F07/F08 fresh seed | 2 / 2 | 2 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F07/F08 Work A ingress Δ | 0 / 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 / 0 | 0 / 0 | 0 | 0 | 0 |
| F07/F08 Work A processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 1 | 1 / 1 | 1 | 1 | 2 |
| F07/F08 Work B ingress Δ | 0 / 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 / 0 | 0 / 0 | 0 | 0 | 0 |
| F07 Work B processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 1 | 0 / 0 | 1 | 1 | 2 |
| F07 final | 2 / 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2 / 2 | 1 / 1 | 2 | 2 | 4 |
| F08 Work B processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 1 | 0 / 0 | 1 | 1 | 4 |
| F08 final | 2 / 2 | 2 | 2 | 2 | 2 | 2 | 2 | 2 / 2 | 1 / 1 | 2 | 2 | 6 |
| F09 fresh seed | 1 / 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 0 / 0 | 0 | 0 | 0 |
| F09 accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 50 | 0 | 1 | 50 / 0 | 0 / 0 | 0 | 0 | 0 |
| F09 all processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 50 | 50 / 50 | 50 | 50 | 100 |
| F09 final | 1 / 1 | 0 | 1 | 1 | 50 | 0 | 1 | 50 / 50 | 50 / 50 | 50 | 50 | 100 |
| F10 fresh seed | 1 / 1 | 6 | 0 | 0 | 0 | 0 | 0 | 0 / 0 | 1 / 1 | 0 | 0 | 0 |
| F10 accepted ingress Δ | 0 / 0 | 0 | 1 | 1 | 6 | 6 | 1 | 6 / 0 | 0 / 0 | 0 | 0 | 0 |
| F10 五个 formed Record processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 5 | 4 / 4 | 5 | 5 | 9 |
| F10 一个 unresolved Record processing Δ | 0 / 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 / 1 | 0 / 0 | 0 | 0 | 0 |
| F10 final | 1 / 1 | 6 | 1 | 1 | 6 | 6 | 1 | 6 / 6 | 5 / 5 | 5 | 5 | 9 |

每个 accepted ingress 行都必须证明 `proc attempt/Identity/Content/Observation/Current/field source` 仍为 0（F10 仅保留 seed 的 Identity/Content 1/1）；每个坏 Record 的 processing delta 必须证明它只增加自己的 processing attempt/outcome，不提前或伪造下游事实。

F03/F04 的 fresh seed、首次 accepted ingress 与全部 processing 三行逐字复用 F01 的三阶段 Oracle；最后一步各只增加 `delivery +1`，分别产生 replay/conflict，所有其他表 delta 为 0。它们不是“从未知已完成状态开始”的隐式测试。

F05 的每个 rejection mutation 从 fresh database 独立启动，除下列 seed 差异外，最终行数都严格为上表 F05 的零副作用：

| mutation | seed Work / Attempt / Work target | 最终 delivery | Package 及全部下游行 |
|---|---:|---:|---:|
| `unauthenticated`、`forbidden`、`body_limit_exceeded`、`malformed_json`、`canonicalization_invalid`、`package_schema_invalid`、`package_hash_invalid`、`record_hash_invalid` | 1 / 1 / 2 | 1，`auditKind=pre_routing_error`，outcome/public/routing refs 全 null，externalCode 为对应 code，不可 GET | 0 |
| `routing_reference_not_found`：不存在 Work | 0 / 0 / 0 | 1，同上 pre-routing union | 0 |
| `routing_reference_not_found`：Work 存在、Attempt 不存在 | 1 / 0 / 2 | 1，同上 pre-routing union | 0 |
| `work_attempt_mismatch` | 2 / 1 / 4；Attempt 只属于第 2 个 Work | 1，同上 pre-routing union | 0 |
| `attempt_capture_mismatch` | 1 / 1 / 2；请求使用合法同属 Work/Attempt 与一个不等于 Attempt 冻结值的 Capture Identity；Capture Identity 不是独立对象，不另测“对象不存在” | 1，同上 pre-routing union | 0 |
| `lease_epoch_mismatch`、`target_mismatch`、`authority_expired`、`authority_revoked` | 1 / 1 / 2 | 1，`auditKind=public_delivery`，public/routing refs 与 outcome=rejected 全非 null，externalCode 为对应 code，可 GET | 0 |

F05 还必须有三类不共享 fresh seed 的 ordered mutation：

| ordered mutation | seed | delivery Δ | 其他正式表 Δ | response Oracle |
|---|---|---:|---:|---|
| accepted 后提交 schema/hash 非法请求 | F01 final | +1 unlinked | 全部 0 | safe error；不返回 deliveryRef；既有 Package/Attempt/Satisfaction 仍保持 F01 真值 |
| accepted 后 authority_revoked 再提交合法 hash | F01 final | +1 linked rejected | 全部 0 | 新 deliveryRef；返回原 acceptedReceiptRef/Package，Attempt terminal、Satisfaction satisfied |
| accepted 事务 commit 前 internal fault | F01 fresh seed | 0 | 全部 0 | 500 internal_error；事务中任何 delivery/Package/Record/Coverage/work 均不存在 |

F05 状态总表的“未创建/open/not_evaluated”只适用于 fresh-seed rejection。accepted-seed 的 pre-routing safe error 没有 public delivery response，也无权覆盖既有状态；随后查询原 accepted delivery 仍得到 F01 的 Package accepted、Attempt terminal(target_reached)、Satisfaction satisfied。accepted-seed 的 authority_revoked linked rejection 明确返回 `Ingress=rejected + Package accepted + Attempt terminal(target_reached) + Satisfaction satisfied`，并保持原 processing/Formation/Current 真值。

processing lease/epoch 负例使用一个独立 fresh seed，不隐藏在上表正常 processing attempt 计数中：

| ordered step | processing work | processing attempt | runtime | Observation / Current revision |
|---|---:|---:|---|---:|
| accepted seed，未 claim | 1 | 0 | ready(epoch 0) | 0 / 0 |
| epoch 1 第一次 claim | 1 | 1 | leased(epoch 1) | 0 / 0 |
| epoch 1 lease 过期后读取 | 1 | 1 | ready(epoch 1) | 0 / 0 |
| epoch 2 重新 claim | 1 | 2 | leased(epoch 2) | 0 / 0 |
| epoch 1 迟到 finalize 被拒绝 | 1 | 2 | leased(epoch 2) | 0 / 0 |
| epoch 2 合法 finalize | 1 | 2 | finalized(epoch 2) | 1 / 1 |

最终 epoch 1 attempt 的 `finalizedAt` 仍为 null，epoch 2 非 null；business outcome 只固定在 processing work，不在 attempt 复制第二状态。任何旧 epoch 拒绝都不得增加 Source/Observation/Current 行。

`field source` 只计算已发布 revision 的 title/body 固定来源行。F-07 第二个 revision 仍引用 observedAt 较新的第一次 Observation；F-08 的第一个 revision 有 2 条 selected source，第二个 unresolved revision 有 4 条 conflicting source。任何实现若产生不同数量，必须先修订本 Oracle，不能把差异解释成等价实现。

F-05 的每个 mutation 必须单独冻结 Attempt 的 authority 前置状态：hash/身份等请求错误在 authority 仍有效时允许修正后重新提交；authority 已过期时继续拒绝首次接入。F05 另有三个互斥解析 mutation：`NaN/Infinity` 只得到 malformed、整数槽浮点只得到 schema invalid、duplicate key + schema 错误只得到 canonical invalid。引用 mutation 必须分别覆盖 Work 不存在、Attempt 不存在、存在但不同属和 Capture Identity 值不匹配；多个缺失引用同时出现时只得到第 9 步同一个安全 code，不泄露哪个资源存在。一次 rejected HTTP delivery 不是终态 Package，也不能自行把 Work 标成失败、满足或完成。

### Capture Satisfaction v1

Capture Satisfaction 是从 accepted Package 的目标结果与 Coverage 重建的独立评估，不是 Work Order 的运行状态。尚无 accepted terminal Package 时显式为 `not_evaluated`，不能使用缺省字段表达。

v1 只允许：

| 值 | 唯一生成规则 |
|---|---|
| `not_evaluated` | 尚无 accepted terminal Package 可以评估；rejected delivery 不改变它 |
| `satisfied` | known-set 每个冻结成员至少有一个 emitted 结果且没有未解决成员；或单一 maximum-quota Package 确实达到 limit、failed=0 |
| `known_gap` | known-set 中仍有明确 failed/not_attempted 成员；或已经达到明确范围边界但存在可计数 failed 缺口 |
| `unknown` | maximum-quota 在 limit 前停止且 remainingScope=unknown；任何已知失败计数也不能消除剩余范围未知 |

`needs_decision` 不进入 v1 数据库枚举、API 或 fixture。是否值得补采属于后续 Acquisition Admission/Decision，不是采集目标满足程度。SCOPE-001 的 Work 只有一个 Attempt，known-set 与 maximum-quota 都只从这个 Attempt 的唯一 accepted Package 重建；跨 Attempt 合并、对象去重和来源范围可加性均未证明，后续放宽前必须新增独立 provenance 与 Oracle。

### 正向与负向 Oracle

每个 fixture 除了上表的正向结果，还必须在 manifest 固定负向 Oracle：

- 所有场景：不得出现替代分责状态的总 `completed/ok/success`，不得让 Package accepted 自动生成 Observation；
- 所有 API/CLI 场景：`workOrderRef` 与 `processingWorkRef` 必须使用不同强类型和 fixture 字典，任何互换、同名 `workRef` 或把 Capture Work 当 processing work 查询都必须在合同层失败；
- Content 读取：不存在的 contentRef 与已存在但 current pointer 为空的 contentRef 都必须返回同一个 safe 404；不得返回半成品 Content、Current unknown、空 fields 或暴露两者差异；
- F-02：不得出现 2/3、66.7% 等完成比例，不得为空缺成员创建 Record、Source Identity 或 Observation；
- F-03/F-04：不得增加或覆盖权威业务行；
- F-05：fresh seed 不得创建 acceptedReceiptRef 或任何 Package 下游行；accepted seed 后的拒绝不得覆盖既有 Package/Attempt/Satisfaction；pre-routing error 不得返回可查询 deliveryRef；不得因一次拒绝自动终止仍有 authority 的 Attempt；
- F-06：不得因一条来源身份冲突回滚其他 Record，也不得给坏 Record 建空身份或空 Observation；Record envelope（payload subtree 之外）增加未知字段必须在 ingress 得到 `package_schema_invalid` 且 Package 为 0，payload 内增加未知字段必须先 accepted，再由该 Record 唯一得到 `record_contract_invalid`，两者不得互换；
- F-07：不得按 receivedAt、acceptedAt、insert 顺序或最后写入推进 Current；
- F-08：不得选择最大值、平均值、任一来源或最后处理值；
- F-09：不得出现 `completionRate=0.5`、`remaining=50`、`knownNotAttempted=50`、`sourceExhausted=true`、`target_reached` 或“平台共 100 个”；
- F-10：不得把 Source Identity 复用记作 Package replay，不得把 unresolved 原料伪造成 Source Object/Observation；明确未观察的 title 必须保持 unknown，不得变成 `""`、`null` selected、`false` 或“无标题”。

每个 externalIngressCode 必须有独立请求、唯一预期和“除规定的一行 delivery audit 外，Package 及全部下游零新增/零改写”断言；pre-routing 验证 `error.code`，routing-confirmed rejected delivery 才验证 `rejectionCode`，不得用一个 generic invalid 测试代表全部拒绝原因。合法但不同 hash 才是 conflict，客户端 hash 错误必须是 pre-routing package_hash_invalid，二者不得互换。合同负例还必须拒绝四种相反联合值：accepted + null refs、not_created + non-null refs、open + terminal reason、terminal + null reason。

上述禁止项必须覆盖合同、数据库、API/CLI JSON 与人类输出；字段改名不能绕过语义 Oracle。

## PostgreSQL 16 物理范围

### migration 拆分

获准实现后只创建两份 migration：

```text
database/migrations/0001_scope_001_capture_evidence.sql
database/migrations/0002_scope_001_content_observation.sql
```

第一份拥有 Work/Attempt、Delivery、Package/Record/Coverage/accepted receipt 与 Record processing work；第二份先补齐 `capture_record` 的 typed Record envelope 与 `record_processing_work` 的封闭 business outcome，再拥有 record processing attempt、Source Identity、Content Observation、Current 与字段来源。migration 一旦提交不回改，后续修正追加新 migration。默认不维护 destructive down migration；回退以全新 proof database 重放为准。

### 表清单与责任

| 表 | 责任 | 关键约束 |
|---|---|---|
| `capture_work_order` | 服务端有界 content-detail 工作、合同与目标语义 | public UUID 唯一；v1 lane/contract/target basis check；不保存研究或 Topic |
| `capture_work_order_target` | `known_set` 的冻结目标成员 | `(work_order_id, ordinal)` 与 `(work_order_id, external_id)` 唯一；quota 不建成员 |
| `capture_attempt` | 一次独立执行权、Capture Identity 与执行终态 | SCOPE-001 中 `work_order_id` 唯一，即 Work 1:1 Attempt；capture identity 唯一；lease epoch/authority deadline 使用 `scope_001_now()`；terminal outcome 不等于 Package accepted；未来补采多 Attempt 必须由后续 SCOPE 迁移放宽并补齐 provenance |
| `capture_ingress_delivery` | 每次 HTTP 交付的最小审计 | `audit_kind=pre_routing_error|public_delivery`；pre-routing 行的 public/routing refs 与 outcome 全为 null、external_code 必须是步骤 1–11 且不可 GET；public 行的 public/routing refs 与 outcome 全非 null并同属，只有 outcome=rejected 带步骤 12–14/16 code；accepted/replay/conflict 的 code 为 null；CHECK/组合 FK 必须拒绝全部相反组合；不保存被拒绝的完整任意载荷 |
| `capture_package` | 一个 Attempt 的唯一冻结终态业务包 | `(attempt_id, capture_identity)` 组合引用同一 Attempt；`attempt_id` 与 `capture_identity` 各自唯一；canonical hash 不可更新；`accepted_delivery_id` 非空且唯一引用创建本 Package 的首次 accepted delivery；`accepted_receipt_ref` UUID 非空唯一；两者在 Package INSERT 时一次写入且不可更新 |
| `capture_record` | Package 内不可变、可重放的 Record envelope 与合成 payload | `(package_id, ordinal)` 唯一；record hash；envelope 逐字段类型化保留 record kind、`source` 五字段与 `observedAt` 的 value/precision/basis，其中 `source_external_id` 列可空只表示合同值可为显式 null，字段缺失仍在 ingress 失败关闭；payload JSONB 只保存合同原料，不承载 Current |
| `capture_package_target_result` | known-set 每个冻结成员在本 Package 的 emitted/failed/not-attempted 结果 | 每 Package/target 唯一；target 必须属于 Package 的 Work；emitted 必须组合引用同 Package Record；quota 没有该行族 |
| `capture_package_coverage` | 本 Package 同单位 Coverage 与终止事实 | Package 1:1；basis-specific CHECK；unknown 不能被差额制造 |
| `record_processing_work` | 只处理一个 accepted Capture Record 的持久工作 | `capture_record_id` 唯一；processor 固定 `content-detail-processor-v1`；typed FK，不使用任意 job payload JSON |
| `record_processing_attempt` | worker claim/lease/epoch/finalize 历史 | `(work_id, epoch)` 唯一；旧 epoch 不能 finalize；运行水位与封闭业务 outcome 分开列 |
| `source_identity` | 极小来源身份注册 | `(source_system, namespace, object_type, external_id)` 唯一；无正文/Topic/指标/业务状态 |
| `source_content` | 类型化 Content anchor 与公开引用 | `source_identity_id` 唯一；`public_ref` 唯一；current revision pointer 可空但只能指向自身 revision |
| `content_observation` | 一次 Record 支持的类型化 Content 状态 | `capture_record_id` 唯一；只追加；title/body 各有 observed flag；时间与 parser version 分开 |
| `content_current_revision` | 可重建、不可变的当前读取版本 | typed title/body 值、三态、policy/version 与前文精确 watermark；watermark 保存固定 public refs，并由 deferred trigger 验证成员恰好等于本次重算锁定的同 Content 输入 Package/Record/Observation 与原 accepted delivery/receipt；不做 EAV |
| `content_current_revision_field_source` | 固定每个 Current 字段当时采用或冲突的 Observation 集合 | `(revision_id, field_kind, observation_id)` 唯一；role 为 selected_support/conflicting_candidate；三者必须属于同一个 Source Content |

`capture_work_order` 的 Capture Satisfaction 与 Package 接入结果分责：Package 只说明交付是否 accepted/replay/conflict/rejected；Attempt 保存本次为何停止及实际 Coverage；Capture Satisfaction 按上文封闭规则表达 not_evaluated/satisfied/known_gap/unknown，不把部分 Package 冒充目标已满足。SCOPE-001 的 Satisfaction 只能由该 Work 唯一 Attempt 的唯一 accepted Package、target results 与 Coverage 重建，因此 response scope 已包含其全部输入。未来补采创建新 Attempt/Package的长期原则不变，但不在本切片建模；放宽 1:1 前必须先增加跨 Attempt Satisfaction provenance、route、时间和 Oracle，旧执行历史仍不得改写。

本切片不创建通用 `evidence` 表。通过最小接入硬门的 `capture_package`、`capture_record` 及其不可变血缘共同承担合成 Evidence 原料；Evidence 是接入资格与用途中的角色，不复制第二份 payload。SCOPE-001 也不创建 Raw Artifact 表、对象存储、HTML/截图/媒体或真实平台响应；真实 producer 合同证明需要 Artifact 时再开后续 SCOPE。

长期领域模型允许一个 Observation 引用多份互补 Evidence；本合成 proof 有意收窄为一个 accepted Capture Record 最多形成一个 `content_observation`。该唯一约束只证明 `content-detail.synthetic.v1`，不得据此否定未来多 Evidence 血缘，也不得为未来提前创建万能连接图。

下列跨表关系必须由数据库组合 FK、唯一/排除约束、deferred constraint trigger 或消除冗余字段直接保护，并有绕过 Rust facade 的 SQL 负例：Work 1:1 Attempt、Attempt/Capture 同属、Package/Work Target 同属、emitted Result/Record 同 Package、content-detail Package 内目标身份不重复、Coverage 与 Target Result/Record 聚合一致、Current/Observation/Content 同属，以及 Package 的 `accepted_delivery_id` 必须指向同一 Work/Attempt/Capture Identity 且 outcome=accepted 的首次 delivery。数据库必须拒绝 Package 引用 replay/conflict/rejected delivery、另一 Attempt 的 delivery 或后来 delivery，并在事务提交时拒绝一个没有唯一 Package 反向引用的 accepted delivery；`originalAcceptedDeliveryRef` 由该 typed relation 唯一读取，绝不等于 acceptedReceiptRef。不能只靠应用层“先查再写”。`source_content ↔ content_current_revision` 的创建顺序固定为先建可空 pointer 的 content，再建 observation/revision/source relation，最后增加或验证组合 FK；运行期先插完整 revision 再原子更新 pointer。

### Current policy v1

`content-current-policy-v1` 对 title 和 body 分别解析：

1. 只考虑对应字段 `observed=true` 且 Record 处理合格的 Observation；
2. 按来源 `observed_at` 选择最新资格集合，不使用 received/accepted/insert 时间；
3. 最新资格集合只有一个不同值时选择该值，并把所有给出该值的 Observation 固定为 `selected_support`，不能依处理顺序任挑一个；
4. 同一最新观察时刻出现多个不同值时，该字段为 `unresolved`，value 为空，并把全部冲突 Observation 固定为 `conflicting_candidate`；
5. 较新 Observation 没观察某字段时，不清空旧字段；
6. 没有合格值时为 `unknown`；
7. 每次重算新建不可变 revision；在一个短事务中插入完整 revision 并更新 `source_content.current_revision_id`，读者只跟随已发布 pointer，不看半成品。

每个字段使用 `selected | unknown | unresolved` 三态和配套 CHECK：`selected` 必须有非 null 值和至少一个同值 `selected_support`，空字符串仍是一个被观察值；`unresolved` 必须无值且有至少两个同一最新时刻、不同值的 `conflicting_candidate`；`unknown` 必须无值且无伪造字段支持来源。revision 固定精确输入 watermark，历史 explain 只读取该 revision 的固定来源集合，不随以后新增 Observation 改写；unknown 通过 watermark + policy 解释检查过的输入，不把 input Observation 伪装成该字段的支持来源。`content-detail.synthetic.v1` 只接受 `observedAt.precision = exact`；非精确时间留给后续策略版本。title/body 是本切片唯一 Current 字段，author、publishedAt、metric、media 等后置。

### authority fence

首切片采用失败关闭的最小规则。接入严格复用前文 17 步“Ingress 唯一判定顺序”：认证/授权、body、JSON 语法、canonical 词法/schema/hash、Work/Attempt/Capture 同属全部先成立；随后锁定同属 routing 行并验证 epoch、固定 v1 contract、target 与 revocation；只有这些 immutable fence 全部合法，才检查 accepted Package。同 hash 创建新 replay delivery 并返回原 acceptedReceiptRef，不同合法 hash 创建 conflict delivery；只有尚无 Package 的首次接纳才在取得锁后使用数据库当前时钟检查 `authority_valid_until`，随后原子写入 accepted delivery、Package/Evidence。事务外检查、客户端时钟和先查后写均无资格；不得用事务开始时的陈旧时间越过等待锁后的 deadline。

这意味着“在 authority 有效时采到、但过期后才首次提交”的材料不会走普通首次接入；只保留最小安全失败回执，完整冻结载荷仍由 producer 本地保留并等待未来独立 recovery/import 设计。已经接纳过的同 hash replay 不受后来 authority 过期影响，但仍要求调用者认证。首切片不伪造晚到合法性，也不因为这一保守规则删除本地原料。真实插件恢复边界在插件 SCOPE 中单独证明。

### Durable work 形态

首切片选择专用 `record_processing_work`，不建立共享万能 `durable_work`：当前只有一个 owner、一个 typed input 和一个 handler，提前抽象通用 Workflow 没有收益。

claim 使用 PostgreSQL 行锁/`SKIP LOCKED` 或等价原子更新；每次领取增加单调 epoch 并建立 `record_processing_attempt`。finalize 在同一事务验证当前 epoch、lease 未过期、输入仍有效，然后追加 Source/Observation/Current 和工作结果。worker 崩溃时 lease 到期可由新 epoch 接管；旧 epoch 后到必须被数据库拒绝。

运行水位只允许 `ready | leased | finalized`。`finalized` 表示 handler 已经原子固定一个业务结果，不等于必然形成 Observation。业务结果只允许：

```text
observation_recorded
source_identity_unresolved
source_identity_conflict
record_contract_invalid
```

运行水位属于 `record_processing_work` 的权威读取结果，不在 `record_processing_attempt` 再维护第二个可漂移状态机：尚未 finalize 且没有未过期 lease 为 `ready`；尚未 finalize 且当前 epoch 的 lease 未过期为 `leased`；工作已固定业务结果为 `finalized`。每条 processing attempt 只记录 epoch、claim/lease 时间、可空 finalizedAt 和本次运行错误审计。崩溃后旧 lease 过期，Work 重新读取为 `ready`；旧 attempt 历史保留，不能改写成业务 outcome。

后面三种是处理器成功识别出的确定业务结果，不伪装成运行失败，也不创建空 Source Identity/Observation。进程崩溃、数据库暂不可用或旧 epoch 被拒绝属于运行尝试失败；lease 到期后回到可接管状态，不在没有重试预算和真实故障策略的情况下发明 stopped/dead-letter。API/CLI 必须同时返回运行水位和业务结果，不能用 `succeeded` 一个词吞并二者。

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
authentication / permission / body / JSON syntax / canonical lexical constraints / schema / hash
→ resolve Work/Attempt and verify Work/Attempt/Capture same-routing tuple
→ lock the verified Work/Attempt/Capture routing rows
→ verify epoch / fixed v1 contract invariant / target / authority_revoked
→ check existing Package: replay or conflict ends here with one new delivery
→ for first acceptance only, verify authority deadline with DB current time
→ pre-generate one deliveryRef and one distinct acceptedReceiptRef in memory
→ INSERT accepted capture_ingress_delivery with deliveryRef
→ INSERT capture_package once with accepted_delivery_id + acceptedReceiptRef already non-null
→ capture_record(s)
→ known target result(s)
→ capture_package_coverage
→ record_processing_work(s)
→ freeze Attempt terminal relation and re-evaluate Capture Satisfaction
→ assemble response from committed delivery/package refs; no receipt UPDATE
```

逐 Record parser、Source Identity、Observation 和 Current 不在该事务内。Package INSERT 时 `accepted_receipt_ref` 与 `accepted_delivery_id` 已经非空，此后运行角色不能更新；最后一步只是组装响应。Package 事务故障注入点至少覆盖 ref 预生成后、accepted delivery 后、Package 后、Record 中途、Coverage 前和 work 中途；任一点在 commit 前失败时该事务内 delivery 与 accepted 业务行全部为 0，返回 `internal_error`，安全请求日志不能冒充 accepted delivery/receipt。合法 replay 创建新 deliveryRef、返回原 acceptedReceiptRef 和 originalAcceptedDeliveryRef，不重写 Attempt 终态或 Capture Satisfaction。

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
2. 首次 accepted 后 authority 过期，同 hash replay 仍创建新 deliveryRef、返回原 acceptedReceiptRef，除 delivery 外业务行不增加；
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
docs/governance/generated-artifacts-registry.md
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

0. **语义代码门** → verify：四轮独立只读攻击及其已知 P0/P1 均有处置记录，G1–G5 在当前正文有可检查证据；按用户最新裁定不再等待第五轮文档复核，门为 CONTROLLED OPEN FOR TDD。实现若暴露会改变产品含义或范围的新问题才停止受影响部分。
1. **F01 合同与 fixture** → verify：有效 F01 合成 Package、已实现的 hash/canonical tracer 与 F01 主链所需的正负 Oracle 先失败再通过；不等待 F02–F10 或未被 F01 主链使用的 canonicalization/资源边界。
2. **F01 空库 migration 与权限** → verify：随机 proof DB 从零重放两份 migration；运行角色不能 DDL、UPDATE/DELETE 不可变历史，F01 所需组合 FK/同属约束能拒绝绕过 Rust 的串错 SQL。
3. **F01 Package ingress** → verify：有效 Package 的 accepted、接入后尚无 Observation/Current、事务中途故障全回滚，以及 F01 数据库行数/约束同时通过。
4. **F01 Record processing** → verify：两条有效 Record 分别形成 Observation/Current；再以一条坏 Record 验证其不撤销同 Package 合格 Record。
5. **F01 API/CLI** → verify：真实 loopback API + worker + PostgreSQL 运行；CLI 只经 API 返回同一 F01 事实，secret 不进 Git/日志/输出。
6. **F01 边界与文档** → verify：文件/依赖门、合同文档、数据库 README、runbook、月度记录、治理与 Bootstrap 全部通过，并报告真实范围以外的 NOT VERIFIED。
7. **F02–F10 硬化** → verify：在 F01 主链完成后，按风险逐项为 replay/conflict、部分结果、处理隔离、Current 冲突等场景补齐独立 seed、Oracle 和真实 proof；它们不倒灌为 F01 主链的前置门。

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

### F01 当前 runtime 信任边界

本切片的应用 runtime 只使用受限 credential，不能表级直接读取或写入事实；其领取、处理与 run-error 通过三项窄 PostgreSQL function 完成。migration/schema owner、proof administrator 与未来运维管理员属于可信 control-plane principal：数据库不能被表述为能防止自身 owner/admin 恶意改写。生产 credential 分离、secret 保管、部署 IAM 与真实运行身份隔离均为 **NOT VERIFIED**，不由本地 synthetic proof 外推。

## F01 主链完成标准

只有以下全部成立，才可报告“F01 合成主链实现证明通过”：

1. 一份有效 F01 合成 Package 在随机、隔离的 PostgreSQL proof database 中完成原子接入，形成 Package、两条 Record、Coverage 与两条 processing work；
2. accepted ingress 阶段的 Source Identity、Content、Observation、Current revision 与 field source 均为 0；
3. 两条 F01 Record 经独立处理后形成 Source、Content、Observation、Current 和字段来源，且读取可追到 Package/Record；
4. 接入事务在约定故障点失败时，delivery、Package、Record、Coverage 与 processing work 均不留下半写；
5. 一条坏 Record 的处理结果不撤销同 Package 合格 Record 的 Observation/Current；
6. loopback API 与只经 API 的 CLI 读取同一合成 F01 事实，并保留 Package accepted、processing、Observation、Current 的分责与 unknown 规则；
7. 哈希不可变、同 hash replay 与不同 hash conflict 不静默覆盖既有事实；
8. F01 相关统一验证命令、治理检查和独立只读复核通过，报告同时列出 VERIFIED 与 NOT VERIFIED。

该完成声明只适用于 F01 合成主链；它不表示 SCOPE-001 整体、F02–F10、真实 producer、平台、插件、AI 或生产已完成。

## SCOPE-001 全量硬化完成标准

只有以下全部成立，SCOPE-001 的全量硬化才可报告完成；这些条件不回溯为 F01 主链的开工或完成前置：

1. G1–G5 已在代码前全部通过，冻结版本经过一次独立对抗复核且没有未关闭的 P0/P1 语义歧义；
2. F-01–F-10 每组都有独立 seed/步骤/预期；F-05 mutation、authority 过期 replay 和 JCS/I-JSON 攻击性负例可以单独运行；
3. API/module receipt 与真实 PostgreSQL 行、唯一约束、权限和事务副作用同时成立；
4. known-set 逐目标 emitted/failed/not_attempted 与 Record 映射可追溯，2/3 与 quota 50/100 在数据库、API、CLI 中保持不同语义；
5. accepted Package 不等待逐 Record 解析；坏 Record 不连坐其他成员；
6. replay 不重复、authority 过期后的同 hash replay 创建新 deliveryRef 并返回原 acceptedReceiptRef、conflict 不覆盖、normal re-observation 不被误叫 replay；
7. authority/worker 旧 epoch 都不能写入受保护事实；
8. Source Identity 并发唯一，unresolved 不制造对象；
9. Content Observation 只追加，迟到历史不丢，Current 不按最后写入倒退；
10. title/body 的 selected/unresolved 都固定完整支持或冲突来源集合，并能追到 Observation、Record、Package 与合同版本；
11. API/CLI 明确返回 scope、四类时间、版本、Coverage、applicability、limitations、类型化 provenance、八个必有责任 key 和 route 权限；
12. API、worker 或 CLI 重启后历史仍存在，未完成 work 可安全接管；
13. 运行角色不能修改不可变历史、串接不同父对象或执行 DDL；API 有最小本地认证，CLI 无数据库旁路；
14. 文件规模与依赖方向自动门通过，没有巨型文件或万能模块；
15. 没有真实平台访问、旧库连接、未脱敏数据、secret、fallback 或静默降级；
16. 所有统一验证命令通过，且数据库副作用证据被记录为脱敏计数/hash/receipt，不提交 dump；
17. 交付措辞只允许“合成事实链实现证明通过”，不得报告 Linggan 已可日常使用、真实采集已接通或市场情报闭环已上线。

任何一项缺失，只能报告“完成到哪一层”，不能把 HTTP 200、类型、编译、mock、fixture schema 或 worker succeeded 单独称为端到端完成。

## 失败、停止与回退

- 合同无法稳定跨 Rust/JSON canonical：停止 ingress 实现，修订合同或发布新版本，不做静默兼容。
- PostgreSQL 约束无法表达已确认语义：停止对应表实现，回到 SCOPE 修订，不用应用层先查后写掩盖。
- 当前正在实施的 F01 主链若失败：不得继续其 API/CLI 包装，先修正主链责任。F02/F09/F10 在各自硬化切片启动后若失败，停止受影响切片并修正其部分结果责任；它们不阻塞尚未启动的 F01 主链。
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

独立只读审查原结论为“条件通过”，发现 1 个 P0、7 个 P1 和 5 个 P2。P0/P1 已全部修入本文：known-set 逐目标结果、authority 过期后的合法 replay、最小本地认证、独立 JCS/I-JSON 证明、组合数据库约束、Current 固定来源集合、单一 processor v1，以及 Package/Attempt/Capture Satisfaction 分责。P2 的场景独立、支撑文件白名单、可靠代码门、草案状态和实现证明口径也已吸收。

完整记录见 [`../../audits/scope-001-final-adversarial-audit-2026-08-20.md`](../../audits/scope-001-final-adversarial-audit-2026-08-20.md)。用户随后已明确批准按修订后的 SCOPE-001 开始实现；本次授权仍受本文的合成数据、文件白名单、数据库安全与后续切片硬停止线约束。

### 2026-08-20 语义冻结后的复核与开工裁定

实施就绪终审随后指出：早先审查虽然裁定了范围和结构，但仍可能让不同 Agent 把分责状态压成总成功、用默认值吞掉 unknown、越过 Claim 层级或放大证明范围。用户已确认以 G1–G5、正负 Oracle 和 [`../../agents/scope-001-execution-contract.md`](../../agents/scope-001-execution-contract.md) 消除这些执行歧义。

因此早先独立审查不替代冻结后的对抗复核。项目实际完成了四轮只读攻击；复核只判断 G1–G5 是否真正唯一、是否仍需 Agent 猜测，不借机扩大到真实插件、Raw Artifact、AI Agent 或全产品实现。

commit `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 之后共完成四轮冻结复核；每轮的 FAIL 和发现均保留在审计记录中，不能改写成历史 PASS。第四轮最终发现 payload/ingress owner、pre-routing audit union、动态 snapshot Oracle、跨 Attempt Satisfaction 四处 P1，现已分别通过分层 owner、封闭 audit 联合类型、固定 proof clock/ref 和 Work 1:1 Attempt 收口。

2026-08-20，用户明确要求停止重复的“修订—再审查”循环并尽快进入代码阶段。该决定将 SCOPE-001 代码门裁定为 **CONTROLLED OPEN FOR TDD**：不再要求第五轮独立文档复核作为创建失败测试、fixture、migration 和 Rust 业务代码的前置条件；实现必须从 F01 的失败合同测试开始，并继续受 G1–G5、文件白名单、合成数据和硬停止线约束。若测试暴露新的纯技术歧义，Agent 在不改变产品含义的前提下做最小合同修正并继续；只有改变用户可见含义、权限、F01–F10 结论或扩入真实 producer/Raw Artifact/插件/AI/生产时才使用 `DECISION_REQUIRED`。
