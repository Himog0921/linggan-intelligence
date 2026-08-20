# SCOPE-001 前置准备矩阵：首个 Content Evidence 垂直切片

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: 在不创建 SCOPE-001、不写业务代码和 migration 的前提下，预演首个合成/脱敏 Content Evidence 切片的用户结果、fixture、失败路径、数据库副作用、模块与文件预算
> 事实来源: 当前 Rust workspace、Gate 4–7 候选、数据/模块/运行架构与合成任务走查
> 冲突时以谁为准: 用户确认后的 DISC-001、正式 SCOPE-001、真实代码/SQL/fixture/测试与数据库副作用；本文不是实施授权

> 后续状态: `USER-DEC-01`–`06` 已于 2026-08-20 全部确认，正式 [`../plans/active/scope-001-content-evidence-vertical-slice.md`](../plans/active/scope-001-content-evidence-vertical-slice.md) 已创建。以下内容保留为当时的准备快照；当前范围以正式 SCOPE 为准。

## 结论

当前仓库具备从空白开始实施首个事实内核切片的结构条件，但尚未获得开工资格。现有 workspace 是占位，不需要先大规模重构；首切片可以只使用当前 `contracts`、`evidence`、`observation`、`storage-postgres`、`api`、`worker`，并根据用户是否要求首期证明外部 Agent 调用决定是否新增一个极小 `apps/cli`。

推荐切片：

```text
合成/脱敏 content-detail Work Order + Attempt
→ 一个终态 Package（含部分结果、Coverage、replay/conflict）
→ PostgreSQL 原子接入 + durable record processing
→ 最小 Source Identity + typed Content Observation
→ 有来源 Content Current
→ API/CLI explain provenance envelope
```

它不依赖真实 XHS、插件、Topic、Corpus、AI、正式趋势、选题或 Outcome，因此不会把未确认平台事实写进第一版数据库。

## 当前 workspace 事实

当前只有：

```text
apps/api
apps/worker
crates/contracts
crates/domain
crates/evidence
crates/observation
crates/intelligence
crates/storage-postgres
```

全部是 Bootstrap 占位：

- API/worker 只打印“尚未启用”；
- contracts 只有 `unimplemented`；
- domain 只有 unknown 与 observed 的示例；
- evidence/observation/intelligence/storage-postgres 都明确未实现；
- Cargo workspace 没有业务依赖；
- 当前 2 项测试只证明 Bootstrap 不冒充实现。

因此不得把现有 crate 名称当最终架构，但首切片也没有理由先全部重命名。

## 首切片用户可见结果

### 对 Mog/操作者

通过最小 CLI 或同一 API 的人类可读输出，能够输入一个 synthetic content public reference，看到：

```text
对象：合成内容 synthetic-note-001
当前状态：由 2 次不可变 Observation 重建
标题：……
正文：……
来源观察时间：……
接入时间：……
当前选择规则：content-current-policy-v1
字段来源：各自 Observation reference
本轮 Coverage：目标未完整完成，2/3 合格
限制：合成 fixture；不能代表真实 XHS producer
```

### 对外部 Agent

结构化输出包含：

- stable public resource reference；
- current content fields；
- observed/received/accepted/read times 分责；
- source/contract/parser/policy versions；
- field-level 或明确 snapshot-level provenance；
- package/record/observation references；
- target basis、actual input basis、Coverage unit/count/stop reason；
- request-specific applicability：本次能用于什么、不能证明什么；
- limitations/unknowns；
- processing watermarks。

### 不能看到

- “市场增长”“需求很大”“代表 ADHD 家庭”；
- 真实小红书原文或账号；
- 内部数据库自增 ID；
- Cookie、Token、DSN；
- AI 总结；
- Topic、Signal、Intelligence、选题或 Outcome。

## 推荐的入口范围

### 必需

1. 创建或预置一个 synthetic Work Order 和当前 Attempt；
2. 提交 terminal Capture Package；
3. 查询 ingress receipt；
4. worker 处理已接纳 Record；
5. 查询 Content Current；
6. `explain` 当前字段、来源、Coverage 和限制。

### 两种入口方案

| 方案 | 内容 | 优点 | 缺点 |
|---|---|---|---|
| A API only | HTTP 提交与读取，集成测试调用模块 | 文件最少 | 对 Mog 和外部 Agent 的产品价值不直观 |
| B API + minimal CLI | CLI 只组合 API，提供 `content get/explain` 与 JSON | 同时证明产品/Agent contract，推荐 | 新增一个很小的组合 app |
| C API + Web UI | 再做运行/内容页面 | 用户直观 | 首切片过宽，会把前端状态与事实内核同时调试 |

推荐 B。CLI 不直接依赖 storage/evidence/observation crate，只调用版本化 API；这样能够证明它不是数据库旁路。Web UI 后置到第二个产品切片。

## 手工维护的合成 fixture 套件

fixture 是测试源，不是生成物，也不是现实 Evidence。建议未来放置：

```text
crates/contracts/tests/fixtures/capture-v1/
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

每个 fixture 配套一个说明 manifest，记录：测试目的、是否脱敏/合成、预期 package/record/observation/current 结果和禁止外推范围。fixture 不从 `references/` 自动生成，避免旧合同变成新真相。

### F-01 完整合法包

```text
target unit: content_detail
target: 2
attempted: 2
emitted: 2
failed: 0
terminal: target_reached（仅因 fixture 明确）
```

证明：原子接入、两个 Evidence Record、两个处理 work、两个 Source Content/Observation。

### F-02 合法部分结果

```text
target: 3 content_detail
attempted: 2
emitted: 2
failed: 0
not_attempted: 1
terminal: risk_control
```

证明：Package 可接受、两个 Record 可处理、Attempt/Work Order 目标未完成、第三个对象不生成 Source Identity/Observation，也不显示不存在。

### F-03 replay

同 Capture Identity、相同 canonical hash 重交。证明返回同一权威 receipt，不增加 Package/Record/Observation/durable work。

### F-04 conflict

同 Capture Identity、payload/hash 不同。证明 fail closed，原 Package 不变，不选择最后写入，不生成新 Evidence。

### F-05 Package 身份错误

Attempt、Work Order、capture、contract 或 authority 不匹配。证明整个接入拒绝且不留下半包；安全失败记录与 Evidence 分开。

### F-06 单 Record 来源身份错误

Package header 合格，但一个 content Record 的 target/internal source identity 冲突。若该错误只有接纳后才能确定，则 Package 仍接入，下游该 Record 隔离；另一个 Record 正常形成 Observation。必须与 F-05 区分。

### F-07 迟到 Observation

先接入较晚观察，再接入较早但合法的观察。证明两份历史都保留，Current 不因迟到自动倒退；选择规则和理由可解释。

### F-08 来源差异

同一内容两个合格来源陈述在某字段不同。证明不按最大值/最后写入静默合并；第一切片可以返回 unresolved 或使用明确 policy，并保留双方来源。

### F-09 配额目标与未知剩余范围

```text
target basis: maximum quota
quota: 100 content_detail
attempted/emitted: 50
terminal: risk_control
remaining source range: unknown
```

证明：配额差额不是 50 个已知 `not_attempted` 对象，不生成 50 个空目标、Source Identity 或 Observation，也不返回“完成 50%”。与 F-02 的已知三目标集合必须使用不同语义。

### F-10 混合 Record 下游处理

一个通过 Package 最小硬门的合成包包含 6 个可重放 Record envelope：4 个形成新 Content Observation，1 个解析到已有 Source Identity 并保留本次来源/观察关系，1 个身份 unresolved 并获得有原因的处理回执。证明 Package/6 个 Evidence Record/工作原子接入；逐 Record 结果随后独立追加；不丢失原料、不把对象复用叫网络 replay、不重复计算现实对象，也不因 unresolved Record 撤销其他成员。

## 数据库候选责任簇

这不是最终表清单，只说明首切片必须持久化哪些不同责任。

### Capture Runtime

- Work Order identity、lane、target scope/unit、contract revision；
- target basis：已知成员、最大配额、来源穷尽、时间/风险预算或探针目的所需的最小表达；
- Attempt identity、capture identity、lease epoch/authority；
- terminal outcome 与目标完成度；
- 不与 Package acceptance 混状态。

### Evidence Ingress

- Package identity、attempt uniqueness、canonical hash、manifest；
- Delivery occurrences/replay/conflict receipt；
- Record ordinal/type/source references/payload or artifact reference/hash；
- Coverage source facts with unit and terminal reason；
- accepted transaction and durable processing work。

### Source World

- minimal Source Identity Registry：source system、namespace、object type、stable external id；
- typed Content anchor；
- typed Content Observation：accepted record、observed time/source/time precision、parser contract、structured source state；
- unresolved/quarantined processing outcome。

### Current

- Current Resolution Run/policy version/input watermark；
- typed Content Current Projection revision；
- current fields 能指向 source Observation/Assertion；
- 原子发布；不完成版本不可见。

### Runtime/Audit

- durable work、worker attempt、lease/fence/result；
- controlled API receipt and correlation；
- 不用日志替代业务历史。

## 首切片 schema 候选推演

正式 SCOPE 可以在两种物理策略中选择，必须用真实 SQL/约束测试，而不是按本文直接建表。

### Strategy 1：明确表族（推荐）

可能包括：

```text
capture_work_order
capture_attempt
capture_package
capture_ingress_delivery
capture_record
capture_coverage_fact
record_processing_work / durable_work envelope
source_identity
source_content
content_observation
content_current_resolution
content_current_projection
content_current_field_source
```

优点：约束和查询明确。风险：若逐字段 provenance 表过早，首切片可能复杂。

### Strategy 2：Observation snapshot + snapshot source（可作为第一步）

Current 先选择一个完整、同源、合格 Content Observation snapshot，projection 引用该 observation。只有出现真实跨来源字段组合需求时再增加 field source。

优点：更简单。风险：不能满足已经推荐的字段级 Current，且未来会因作者/正文/指标不同步重新拆。

当前推荐：正式 SCOPE 使用一个很小的类型化 Content Current，字段数量控制在 fixture 真正证明的范围；对每个 Current 字段保存 Observation source FK，而不做通用 field-key/value。这样保持字段来源原则，又不建立 EAV。

## PostgreSQL 必须证明的副作用

### 正向

1. 从全新 proof database 执行 baseline migrations；
2. 创建 Work Order/Attempt 后提交 F-01；
3. 一个事务产生 Package、Records、Coverage、receipt、durable work；
4. worker claim 后产生 Source Identity、Content anchor、Observations；
5. Current Resolution 原子发布；
6. API/CLI explain 能追到每个 current field 的 Observation 和 Record/Package；
7. 重启 API/worker 后结果仍存在且可查询。

### 负向

1. Package 事务中注入失败，相关行全为 0；
2. 两个请求并发 F-03，只一份 Evidence；
3. F-04 返回 conflict，原 hash/内容不变；
4. 两个 worker 同时 claim，只一个 epoch 有权 finalize；
5. lease 在 finalize 前失效，业务结果和后续 work 不写入；
6. F-05 不生成 Package/Evidence；
7. F-06 只隔离坏 Record，不撤销其他接纳 Evidence；
8. 同 platform/type/external id 并发解析只产生一个 Source Identity；
9. F-07 保留迟到历史但 Current 不倒退；
10. 未完成 Current revision 不可读；
11. 隐私阻断候选最少要证明 Read Model 不绕过（即便首切片只用合成数据）。

### 证明方式

- 使用真实 PostgreSQL 16 proof database，不用 SQLite/in-memory fake；
- 每项断言同时检查 API/module receipt 和数据库行/约束副作用；
- 测试结束删除精确 proof database，不修改开发库；
- migration rollback 不作为默认策略，优先从空库重放；
- 任何 destructive 命令精确指向临时 proof database。

## API 候选合同

不是最终 route，但首切片至少需要这些能力：

```text
create synthetic work/attempt（测试/受控入口）
submit terminal package
get ingress receipt
get processing status
get content current by public ref
explain content current provenance
```

生产 API 不允许客户端自由伪造 Work Order/Attempt authority。合成入口应只在测试 harness 或明确开发模式出现；普通 package submit 必须引用服务端已经存在的 Attempt。

### 建议结果 envelope

```text
data/result
scope
asOf times by meaning
versions
coverage
limitations
provenance refs
processing watermarks
allowed next actions
```

`ok: true` 不足以证明任何首切片成功。

## Rust 模块与文件预算

以下是职责预算，不是要求一次创建所有文件。

### `crates/contracts`

```text
src/lib.rs                         只 re-export，小于 120 行
src/capture/mod.rs                 公共 wire types
src/capture/envelope.rs            Work/Attempt/Package identity
src/capture/record.rs              typed content record contract
src/capture/coverage.rs            unit/terminal/source facts
src/capture/validation.rs          runtime boundary validation
src/canonical.rs                   canonical bytes/hash
tests/capture_contract.rs
tests/fixtures/capture-v1/*
```

禁止把内部 domain model 复制成 wire DTO，禁止一个 1000 行 `types.rs`。

### `crates/evidence`

```text
src/lib.rs                         Capture facade
src/ingress/mod.rs                 submit interface
src/ingress/authority.rs           transaction-time fence
src/ingress/idempotency.rs         replay/conflict
src/ingress/transaction.rs         package atomic write
src/coverage.rs                    source fact validation
src/work_orders.rs                 minimal testable Work/Attempt ownership
tests/ingress_postgres.rs
tests/ingress_concurrency.rs
```

如果 `evidence` 同时承担 admission、station scheduling、plugin protocol 全部职责，切片过宽；首切片只实现已有 synthetic Attempt 的接入与最小 runtime ownership。

### `crates/observation`

```text
src/lib.rs                         World facade
src/source_identity.rs             minimal registry resolution
src/content/mod.rs
src/content/observation.rs         typed append
src/content/current.rs             resolution + atomic publish
src/processing.rs                  accepted record handler
tests/content_postgres.rs
tests/content_current_concurrency.rs
```

不创建 Author/Comment/Media 空模块。

### `crates/storage-postgres`

```text
src/lib.rs
src/pool.rs
src/error.rs
src/transaction.rs
src/testing.rs                     proof DB helper（只在 test feature）
```

业务 SQL 留在拥有不变量的 evidence/observation 私有 adapter，不把 `storage-postgres` 做成 CRUD repository。

### `apps/api`

```text
src/main.rs                        composition only
src/app.rs
src/routes/capture.rs
src/routes/content.rs
src/http/envelope.rs
src/http/error.rs
```

route 只做认证/schema/映射，不直接 SQL 或复制 ingress/current 规则。

### `apps/worker`

```text
src/main.rs                        composition only
src/app.rs
src/runner.rs
src/handlers/process_capture_record.rs
```

如果只有一个 handler，不先建通用 plugin registry/Workflow/CommandBus。

### 可选 `apps/cli`

```text
src/main.rs
src/commands/content.rs
src/output/human.rs
src/output/json.rs
src/client.rs
```

CLI 只调用 API。首批只做 `content get`、`content explain`、`processing status`；不做数据库连接或万能 query。

## 依赖预算

正式 SCOPE 必须逐项说明引入原因。候选只包括真实需要：

- `serde` / `serde_json`：wire contract；
- `uuid`：服务端 identities；
- `time` 或等价：有精度/来源的时间类型；
- `sha2` 或等价：canonical hash；
- `sqlx`：PostgreSQL；
- `tokio`：API/worker runtime；
- `axum`：HTTP adapter；
- `thiserror` 或显式 error：模块错误；
- CLI 若纳入，`clap` 与一个明确 HTTP client。

不预加：Kafka/Redis/Temporal SDK、ORM、GraphQL、pgvector、LLM SDK、Python bridge、OpenTelemetry 全家桶、通用 DI 容器或大型 workflow crate。

## 文件规模与依赖自动门

首个代码 PR 必须新增可运行检查，而不只写规则：

1. `lib.rs/main.rs` 超过 120 提示、200 失败；
2. 生产 `.rs` 超过 350 提示、500 失败；
3. 函数 60/100 行门可先使用 lint/脚本近似并人工复核；
4. 测试文件 600/900；
5. 单模块 public item 15/25；
6. `apps/*` 不依赖 SQLx 业务查询模块以外的内部 adapter；
7. `contracts` 不依赖 evidence/observation/storage；
8. `domain` 不依赖 infrastructure；
9. `intelligence` 首切片不新增空实现；
10. 例外需要带到期时间的 ADR。

具体脚本文件在 SCOPE 确认后创建；本报告不登记生成物或 CI 产物。

## TDD 顺序候选

正式实现时建议按失败证据推进：

1. TS/Rust-independent JSON fixture schema/canonical golden 先失败；
2. F-02/F-09 部分结果与目标语义合同先失败；
3. 空库 migration/约束测试先失败；
4. Package replay/conflict 并发测试先失败；
5. authority fence 交错测试先失败；
6. F-06/F-10 Record → Source Identity/Observation 独立处理测试先失败；
7. late observation/Current 测试先失败；
8. API receipt 与数据库副作用 E2E 先失败；
9. CLI explain（若纳入）contract test 先失败；
10. 文件规模/依赖规则门最后加入但在首 PR 内通过。

每一步先看到与预期相符的失败，再写最小实现；不一次编写所有表和代码后补测试。

## 首切片完成的可证伪标准

只有同时满足以下条件才可宣布完成：

1. 从空 PostgreSQL 16 proof DB 重放 migration 成功；
2. F-01 至 F-10 均有正负结果；
3. Package/Records/Coverage/receipt/durable work 的原子副作用可查；
4. 已知集合与最大配额两种 50/100 语义分开；合格原料保留但 Work/Attempt 不显示完整完成，配额差额不制造已知未尝试对象；
5. replay 不重复，conflict 不覆盖；
6. 失去 authority 的旧执行者不能写 Evidence；
7. Source Identity 并发唯一且无万能业务属性；
8. Content Observation 只追加；迟到历史不丢且 Current 不倒退；
9. Current 每个字段或本切片定义的最小粒度能追到来源；
10. API/CLI 输出时间、版本、目标/实际输入基础、Coverage、针对本次请求的 applicability、限制和 provenance；
11. 进程重启后仍可查询/继续 durable work；
12. 数据库、API、worker 和 CLI 负向权限测试通过；
13. 文件规模和依赖方向门通过；
14. `cargo fmt`、Clippy、workspace tests、治理和 bootstrap 校验通过；
15. 没有真实平台、旧数据库、Cookie/Token、未脱敏数据或静默 fallback；
16. 真实运行结果与数据库副作用共同证明，不拿类型/编译/HTTP 200 代替。

## 不在首切片

- 真实插件 claim/renew/reconcile；
- XHS selector/API 和真实账号；
- Author、Comment、Media、Metric 的完整模型；
- Topic、Term、Map、Classification；
- Corpus/search/embedding；
- Agent/LLM；
- Signal/Claim/Brief（除非只用于 API 限制文案，不持久化）；
- 正式趋势；
- 选题/Action/Outcome；
- Web UI；
- 多租户、MCP、移动端；
- 生产部署。

## 对正式 SCOPE-001 的输入

一旦 `USER-DEC-01`–`06` 确认并关闭 DISC-001，正式 SCOPE 必须逐项裁定：

1. 采用 API only 还是 API + minimal CLI（推荐后者）；
2. 精确的用户可见结果和命令/route；
3. fixture 的唯一来源和 canonical contract；
4. 首批 Current 字段及 field provenance 粒度；
5. 最小 schema/表/约束、migration 数量和角色；
6. authority fence 的协议时间语义；
7. durable work 物理形态；
8. 上述文件清单中哪些真实需要创建；
9. 测试与故障注入命令；
10. 完成、失败、回退和不包含范围。

然后才能执行用户要求的最终子代理对抗审查。审查必须攻击正式 SCOPE 的具体表、模块、fixture、API、CLI、并发、权限和文件预算，而不是再泛泛评论架构原则。

## 隐藏待定项复查

已对现行 `AGENTS.md`、README、数据库说明和 `docs/` 中的 `DECISION_REQUIRED`、`SOURCE_INCOMPLETE`、待确认与待决定表述进行反向扫描。后续关于“部分数据入库后怎样使用”的讨论证明原 `USER-DEC-02` 内容不完整：它必须覆盖目标语义、原料保存、Package 硬门、逐 Record 处理、用途适用、Claim 推断和补采组合。该问题扩展现有 `USER-DEC-02`，无需新增平级 `USER-DEC-07`；扩展版后来已获确认并进入正式 SCOPE。

### 当时阻塞正式 SCOPE 的决定（现已确认）

只有 `USER-DEC-01`–`06`：来源纪律、部分数据保存与使用、Source Identity/Current、首期统计承诺、Gate 6 技术方向与 Gate 7 产品入口。它们决定首切片能否作为正确系统的起点，而不是只决定实现参数。

### 进入 SCOPE 后由 Agent 用测试裁定

- authority/lease fence 的精确事务时点；
- durable work 的最小物理表形态；
- Current 首批字段与来源引用的最小粒度；
- migration 拆分、约束和索引；
- retry/lease 数值、错误码和故障注入方式；
- API route、CLI 命令和文件清单的最小实现；
- 文件规模门禁脚本的实现方式。

这些不要求 Mog 猜技术答案，但正式 SCOPE 必须列出推荐、失败证据和验收方法。

### 不阻塞合成首切片的后续决定或来源缺口

- 真实 XHS 账号、工位、作者/笔记与实验访问授权；
- 搜索可比性、完整 Coverage、7 天衰减、安全频率和真实 lane 字段；
- 评论/作者媒体和原始 Artifact 的真实保留策略；
- 未脱敏原文、第三方模型处理、外部 Agent 全文权限与法律审查；
- 模型/embedding/聚类技术选型与真实语料 benchmark；
- 生产备份 RPO/RTO、对象存储、部署和恢复演练；
- MCP、移动端和真实插件升级。

它们继续保持 `SOURCE_INCOMPLETE` 或后续 `DECISION_REQUIRED`，不得被合成 fixture 偷偷替代。

### 提交与发布侧待定

GitHub 账号对应的正式提交邮箱仍未确认。它不阻塞 SCOPE 设计、测试设计或本地实现准备，但在未来首次代码提交/推送前必须收口，不能继续使用本机历史占位邮箱冒充正式身份。

## 当前资格

- 设计准备：足够进入用户决定；
- SCOPE-001：已创建，处于最终对抗审查；
- migration/代码：未授权；
- fixture 文件：尚未创建；
- 数据库：未修改；
- 最终独立审查：已完成，见 [`scope-001-final-adversarial-audit-2026-08-20.md`](scope-001-final-adversarial-audit-2026-08-20.md)；
- 下一步：等待用户对修订后正式 SCOPE 的最后实施确认。
