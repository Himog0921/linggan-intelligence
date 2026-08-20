# Gate 4–7 跨文档一致性审计

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: DISC-001 在进入统一决定与 SCOPE-001 前的采集、数据、Rust/Agent/插件、运行、页面与 CLI 一致性审查
> 事实来源: 当前 Gate 4–7 草案、已确认共同语言/不变量、固定 V2 只读审计与当前 Git 差异
> 冲突时以谁为准: 用户最新确认、ACCEPTED ADR、权威当前状态、真实 producer/fixture/PostgreSQL 副作用；本报告不是权威决定源

## 结论

当前设计已经形成一条可裁切的共同主链：

```text
用户目的/持续目标
→ Need / Request / Authorization / Admission
→ Work Order / Attempt / terminal Package
→ Evidence / Source Identity / typed Observation / Current
→ Materials / Analysis / Claim / Brief
→ Decision / Action / Outcome / Evaluation
→ 工作台、API、CLI 的有来源读取
```

没有发现必须推翻 Gate 1–3 的结构性漏洞，也没有重新引入旧数据库、TypeScript 产品运行时、万能 Object、万能 Workflow、一页面一服务或正式趋势捷径。

但项目目前**还不能开始业务代码或业务 migration**。原因不再是缺少更多架构文字，而是六个用户决定包尚未确认：Gate 4 来源纪律、部分结果、Source Identity/Current、首期统计承诺、Gate 6 技术方向和 Gate 7 产品入口。它们已经去重登记在 DISC-001 活跃计划。

本报告由主 Agent 完成。用户要求的最终子代理对抗审查仍必须等上述决定和 SCOPE-001 草案收口后单独执行，当前没有提前使用。

## 一致性检查

| 检查 | 结果 | 说明 |
|---|---|---|
| 产品核心没有因语料/内容应用漂移 | 通过 | 领域情报研究系统仍是内核，应用是视图/工作能力 |
| Objective 与 Research Question 未重新合成万能 Intent | 通过 | 不同生命周期，只共享 Observation 使用关系 |
| Evidence 不等于现实真相 | 通过 | 限定为某 producer/时间/条件下接纳的材料 |
| 部分执行与 Record 资格分开 | 条件通过 | 数据/采集/页面一致推荐保留合格 50；用户尚未正式确认 |
| Attempt/Package 关系一致 | 通过 | Work Order 1:N Attempt、Attempt 0..1 terminal Package；Delivery/replay 不产生多包 |
| Package 原子与 Record 隔离不矛盾 | 通过 | 数据库半包回滚；接纳后 Record/Object 独立处理 |
| 身份统一未退化万能 Object | 条件通过 | 仅最小 Source Registry + typed anchors/observations；待确认 |
| Current 未覆盖历史 | 条件通过 | typed projection + resolution run + 字段来源；待确认 |
| 正式趋势未提前开放 | 通过方向 | PRD/数据/Agent/页面均只允许描述与候选变化 |
| Agent 无旁路权力 | 通过 | 无 SQL、正式知识、插件或现实行动直写 |
| 插件无第二任务真相 | 通过 | 本地状态只为恢复；服务端拥有权力与接入 |
| 页面/CLI 无第二事实源 | 通过 | Read Model provenance envelope；按能力而非按表 CRUD |

## 本轮发现并修正的漂移

1. `page-map.md` 的情报详情仍使用统一“置信度”，已改为 Claim、支持、反例、不确定性和适用范围。
2. `page-map.md` 仍使用旧 `Observation Target/Plan`，已改为 Observation Objective、Collection Plan revision 与实际 Coverage。
3. `migration/action-plan.md` 把 Intelligence 压成一个置信度，已改为固定 Claim revisions、支持、反例、缺口和适用范围。
4. `target-architecture.md` 仍把早期 Observation Target/Plan 当当前候选，已标明被 Gate 3 的 Objective/Collection Plan 取代。

## 对“目标 100、实际 50”修订的审计

该修订成立，但必须同时保留：

1. 接受的是分别满足来源、身份、合同和权限的材料，不是整次任务成功；
2. 100/attempted/emitted/failed/not-attempted 必须是同一单位；
3. unknown 不能用减法或期望数量制造；
4. 部分结果仍通过一个冻结终态 Package，不能让同一 Attempt 多包追加；
5. 50 条可用于存在性、具体对象和材料研究，不自动用于完整 Coverage、总体比例或趋势。

推荐把它正式命名为：

> **执行完整度与记录资格分离。**

## 数据库层级

当前不是最终表设计，但责任层级已清楚：

```text
运行/权力
Actor、Delegation、Objective/Question、Need、Request/Auth/Admission、Work/Attempt

采集/Evidence
Package、Delivery Receipt、Record、Artifact、Coverage Source Fact

来源世界
Minimal Source Identity Registry、typed Author/Content/Comment anchor、typed Observation

当前读取
Resolution Run、typed Current Projection、field/source links

知识与分析
Topic Identity/Definition/Release、Analysis Input/Run/Result、Classification/Adjudication

判断
Claim Identity/Revision、typed support/challenge、Brief Release

材料
Fragment、Transformation、Corpus Selection、Material Pack、Usage

决定与学习
Proposal、Decision、Action Plan/Attempt/Occurrence、Expected/Observed Result、Evaluation

访问与处置
Authorization Audit、Privacy Disposition、Propagation Receipt
```

这不是九个 schema 或服务。一个 PostgreSQL 主库中由深模块拥有关系和事务；Read Model 可重建。核心关系不使用自由 `(type,id)`、EAV 或任意 JSON 多态键。

共享 durable table、索引、分区、具体字段和数据库角色数，可在首切片用真实 SQL/并发测试决定，不要求 Mog 猜测。

## 架构效率与代码边界

当前保持简单：一个模块化单体、一个 PostgreSQL、API/worker 两进程；插件仍是 TypeScript/MV3；PostgreSQL durable work；Agent 首期 A1–A2；正式趋势、pgvector、Python 分析进程和对象存储按证据后置。

复杂度闸门：

- 无万能 AppService/CommandBus/EventBus/Workflow/Object/Repository；
- 每个深模块只有小 facade，SQL/状态机在内部；
- `lib.rs/main.rs` 120/200、生产文件 350/500、函数 60/100、测试 600/900、public item 15/25 候选门；
- 硬门例外需要有期限 ADR；
- 插件、Agent、runner 分责任目录，不建巨型控制器。

未来需要重点防止：`storage-postgres` 变成 CRUD 大层；`domain/common/utils` 变垃圾箱；全局 `JobKind` dispatcher 吞业务；Agent 用通用 HTTP/shell 越权；Read Model 被写回；页面和 CLI 各建一套状态；首切片一次创建全部空 crate/表。

## 开工资格

| 工作 | 当前能否开工 | 缺什么 |
|---|---|---|
| 文档一致性、合成/脱敏 fixture 设计 | 可以 | 继续不写业务代码、不访问平台 |
| 第一条 Rust/PostgreSQL Evidence 切片 | 不可以 | USER-DEC-01–06 + DISC-001 退出 + SCOPE-001 |
| 业务数据库 migration | 不可以 | 部分结果、身份/Current、统计边界与 SCOPE schema/负例 |
| AI Agent 内核 | 不可以 | Evidence/Materials/Access 运行链、脱敏 benchmark、模型数据处理决定 |
| 插件升级 | 不可以 | 真实 producer 实验授权/结果、服务端合同先行 |
| 正式趋势 | 不可以 | 真实可比性与统计资格，只按证明范围开放 |
| UI/CLI 实现 | 不可以 | Gate 7 方向确认与 SCOPE 用户可见结果 |
| 生产部署 | 不可以 | 业务切片、备份恢复、RPO/RTO、安全角色和发布门 |

## 下一步

1. Mog 对活跃计划中的 `USER-DEC-01` 至 `USER-DEC-06` 整体确认或指出要修改的项；
2. 主 Agent 回写权威共同语言、不变量、当前状态和 Gate 4–7 状态；
3. 创建 SCOPE-001，定义首个合成/脱敏 Content Evidence 垂直切片；
4. SCOPE 明确代码/表/fixture/失败/验收后，启动用户要求的最终子代理对抗审查；
5. 修复审查问题并取得用户最后确认；
6. 之后才创建业务 migration 和 Rust 代码。

## 当前状态

- 结构一致性：通过；
- 用户决定：待 `USER-DEC-01`–`06`；
- 真实 producer：未执行，保持 `SOURCE_INCOMPLETE`；
- 数据库：概念/关系/事务候选完整，未授权 DDL；
- Rust/Agent/插件：模块责任完整，未授权实现；
- 页面/CLI：任务与合同候选完整，未做原型；
- 最终子代理审查：按用户要求保留，尚未执行。
