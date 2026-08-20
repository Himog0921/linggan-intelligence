# Linggan Intelligence 正式编码前实施就绪终审

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: Linggan Intelligence 正式业务编码前的项目级实施就绪审查，以及主线 Agent 的独立复核入口
> 事实来源: 当前工作副本、origin/main、实际 Rust 代码、数据库目录、权威文档、活跃 SCOPE、固定 V2 证据与本轮验证输出
> 冲突时以谁为准: AGENTS.md 的事实优先级；真实代码、migration、producer fixture、测试与数据库副作用高于本报告

## 给主线 Agent 的使用说明

这是一份审查证据包，不是新的产品决定、实施授权或第二份权威架构。它的用途是让没有参加本轮对话的主线 Agent 能够：

1. 理解为什么本轮结论是 NO-GO，而不是依赖聊天摘要；
2. 使用相同文件、行号、命令和判定标准复核每个问题；
3. 对每个问题给出 CONFIRMED、DISPROVED、STALE 或 DECISION_REQUIRED；
4. 在证据发生变化时推翻本报告，而不是维护本报告的结论。

主线 Agent 不应先修改代码来证明本报告错误。正确顺序是：

1. 复现仓库和验证快照；
2. 按 IRR-P0/P1 编号逐项检查当前最高优先级事实；
3. 对已确认的问题先收敛唯一合同和验收测试；
4. 所有 P0 关闭后重新判断 GO、CONDITIONAL GO 或 NO-GO；
5. 只有新的权威文档、真实 producer fixture、migration、测试或数据库副作用能够改变本报告的事实等级。

## 2026-08-20 主线复核处置

本节记录报告提交后的主线裁定，不改写本报告在原快照下的发现。用户确认：先消除 Agent 的未经授权推断，但不把真实 producer、Raw Artifact、AI Agent 或插件升级反向扩进当前合成 SCOPE。

| 原问题 | 主线裁定 | 当前处置 |
|---|---|---|
| IRR-P0-01 实施授权与代码/数据库注释冲突 | `CONFIRMED` | 更新当前状态、数据库边界和 Bootstrap 模块注释；代码门由 G1–G5 控制，不再由过期注释隐式控制 |
| IRR-P0-02 50/100 无法表达客观来源穷尽 | `CONFIRMED FOR REAL PRODUCER / DISPROVED AS SYNTHETIC-SCOPE BLOCKER` | 当前切片只区分 known-set 与 maximum-quota；quota 50/100 保持 remainingScope=unknown，不声称真实来源穷尽。真实穷尽仍为后续 producer 合同问题 |
| IRR-P0-03 synthetic-only 排在真实 producer 审计之前 | `DISPROVED AS SYNTHETIC-SCOPE BLOCKER` | 合成切片只证明事实内核，不证明真实平台；Proof Scope 必须随验证传播。真实 producer、插件和日常产品仍为 NO-GO |
| IRR-P0-04 authority 过期后的本地有效原料缺少恢复/import | `CONFIRMED FOR PLUGIN INTEGRATION / OUT OF SCOPE FOR SCOPE-001` | 当前切片只处理服务端获准 Package 接入和明确拒绝；不发明本地恢复/import。真实插件接入前该问题仍必须解决 |

因此本报告的项目级 NO-GO 不能被缩写成“当前合成 SCOPE 永远不允许编码”。主线当前结论是：**SCOPE-001 为条件开放，G1–G5 和冻结后独立复核通过前代码门关闭；真实 XHS、Raw Artifact、插件、AI Agent 和全产品运行闭环继续 NO-GO。** 本报告仍是一份一次性证据包，不取代当前 SCOPE 和 [`../agents/scope-001-execution-contract.md`](../agents/scope-001-execution-contract.md)。

## 审查快照

| 项目 | 本轮快照 |
|---|---|
| 仓库 | /Users/moglenny/proma/linggan-intelligence |
| 分支 | main |
| HEAD | 680eff35a1c5babae70eac642aee5afe22435e1f |
| origin/main | 680eff35a1c5babae70eac642aee5afe22435e1f |
| 分歧 | 0/0；HEAD 与 origin/main 相同 |
| 工作区 | 非干净；存在 GOV-002 相关未提交文档变更 |
| Rust 业务实现 | 仍是 Bootstrap 骨架 |
| 业务 migration | 未找到 |
| 真实 producer fixture | 新仓库中未找到 |
| 当前 PostgreSQL 运行证明 | 本轮未取得；Docker daemon 未运行 |
| 静态验证 | governance、bootstrap、fmt、clippy、workspace tests 通过 |
| 测试含义 | 只证明 Bootstrap；实际仅有 2 个极小单元测试，不能证明业务闭环 |

本报告核对时，工作区已有以下不属于本审查的用户变更，本报告没有覆盖或回退：

~~~text
M  AGENTS.md
M  docs/README.md
M  docs/current-state.md
M  docs/governance/file-placement-standard.md
M  docs/progress/2026-08.md
?? docs/agents/
?? docs/plans/completed/gov-002-agent-skills-configuration.md
~~~

# 1. 最终结论

| 项目 | 结论 |
|---|---|
| 正式判断 | **NO-GO** |
| 判断置信度 | **94%** |
| 一句话核心原因 | 项目已经形成较强的领域原则和合成事实切片，但当前唯一获准实施的 SCOPE 与更高优先级代码/数据库边界冲突，并且不能表达必审的客观来源穷尽、迟到有效数据恢复和第一条真实插件闭环。 |
| 当前真正阶段 | 设计基线、环境 Bootstrap、Rust 空骨架、固定 V2 证据盘点和详细合成 SCOPE；还不是第一条真实数据闭环已具备合同、实现与验收证据的阶段。 |
| 是否允许正式业务编码 | **不允许。** 先关闭本报告 4 个 P0；可以继续只读事实审计、合同裁定、验收用例编写和环境恢复验证。 |

这里的 NO-GO 不是说现有设计毫无价值，也不是说必须先完成整个产品。它只表示：按照原始终审标准，当前还不能让 Agent 依据现有材料开始正式业务 migration、Capture v1、Evidence/Observation 或真实插件链实现，因为不同 Agent 仍可能合理地实现出互不兼容的系统。

## 四种成熟度的总体判断

| 层级 | 当前状态 | 证据 |
|---|---|---|
| 设计意图已经写明 | 较强 | AGENTS.md、领域不变量、Gate 设计文档、SCOPE-001 |
| 数据或接口已经定义 | 仅合成切片有详细候选 | SCOPE-001；但真实 producer、恢复、完整数量与原始 Artifact 合同未冻结 |
| 代码已经实现 | 否 | crates 与 apps 仍是 Bootstrap 常量或打印入口 |
| 测试或真实运行已经证明 | 否 | 只有 2 个 Bootstrap 测试；本轮无真实 PostgreSQL、真实插件或真实 producer 证明 |

# 2. 审查范围与证据清单

## 2.1 证据分类

本报告使用以下标签：

- **已证实事实**：可由当前文件、实际命令、代码或测试直接复核。
- **合理推断**：由多个已证实事实推导出的工程后果；可被更高优先级新证据推翻。
- **缺少证据**：已检查相关位置，但没有找到所需合同、实现或验证。
- **文档冲突**：两个现行材料对同一动作给出不同结论，且会导致不同实施。
- **尚未定义**：文档明确后置，或只写方向而没有可执行规则。

## 2.2 实际检查的主要正式材料

| 类别 | 文件或位置 | 本轮用途 |
|---|---|---|
| 仓库合同 | AGENTS.md | 文件治理、事实优先级、不可违反规则、交付纪律 |
| 当前状态 | docs/current-state.md | 当前阶段、实施授权、真实实现状态 |
| 产品与领域 | docs/product/PRD.md；docs/product/domain-invariants.md；docs/context/domain-language.md | 产品边界、领域语言、对抗性不变量 |
| 当前系统事实 | docs/context/current-system-inventory.md | 固定 V2 producer、合同、Coverage、原料和恢复能力边界 |
| 迁移路线 | docs/migration/action-plan.md | 合成实现、事实审计、真实插件接入的阶段顺序 |
| 合成切片 | docs/plans/active/scope-001-content-evidence-vertical-slice.md | 唯一获准实施的合同、数据、状态、事务、测试和文件白名单 |
| 采集插件 | docs/architecture/capture-plugin-architecture.md | claim、renew、reconcile、恢复、风险、安全与真实 producer 门 |
| 数据设计 | docs/architecture/data-architecture.md；data-relations.md；data-consistency.md | 原始层、观察、多时点、Current、事务和并发设计意图 |
| AI/运行/UI | docs/architecture/ai-agent-architecture.md；runtime-operations-architecture.md；docs/pages/product-interface-architecture.md | AI 权限、后台运行、下游显示边界 |
| 数据库边界 | database/README.md；database/ 目录 | 当前是否有业务 DDL/migration、数据库授权边界 |
| 代码 | crates/*/src/lib.rs；apps/api/src/main.rs；apps/worker/src/main.rs | 当前实现成熟度和代码自述约束 |
| 既有终审 | docs/audits/scope-001-final-adversarial-audit-2026-08-20.md | 比较上一轮“合成 SCOPE 条件通过”与本次“全项目真实闭环就绪”口径 |
| 来源保护 | references/README.md；固定 checksum/Bootstrap 验证 | 历史 V2 只能作证据，不能直接晋级现行合同 |

本轮还对仓库执行了文件枚举和关键字扫描，检查了 database、crates、apps、docs、scripts、tests/fixture 相关路径。没有把 references 中的历史实现当成新仓库已经实现。

## 2.3 实际检查的代码与数据库事实

| 位置 | 已证实事实 |
|---|---|
| crates/contracts/src/lib.rs:1-3 | 明写没有真实 producer fixture 就不增加合同；当前合同版本是 unimplemented |
| crates/evidence/src/lib.rs:1-3 | 明写字段与合同审计后才开始实现；当前 false |
| crates/observation/src/lib.rs:1-3 | 只有防止推断不存在的 Bootstrap 常量；当前 false |
| crates/storage-postgres/src/lib.rs:1-3 | 明写模型决定关闭后才增加 baseline migration；当前 false |
| crates/domain/src/lib.rs:1-20 | 只有 Unknown 与 Observed 的 Bootstrap 枚举和一个测试 |
| crates/intelligence/src/lib.rs:1-3 | 当前 false |
| apps/api/src/main.rs:1-3 | 只打印没有生产 route |
| apps/worker/src/main.rs:1-3 | 只打印没有 job |
| apps/cli | 目录不存在 |
| database/ | 只有 README 和 legacy archive manifest；没有业务 migration |

## 2.4 本轮验证输出

| 检查 | 结果 | 能证明什么 | 不能证明什么 |
|---|---|---|---|
| git rev-parse HEAD / origin/main | 两者均为 680eff35... | 本地提交点与远端相同 | 工作区内容可复现；因为存在未提交变更 |
| ./scripts/check-project-governance.sh | 通过 | 当前文件治理规则通过静态检查 | 业务合同一致、真实闭环可用 |
| ./scripts/verify-bootstrap.sh | 通过 | 固定来源 checksum 和 Bootstrap 边界仍成立 | 历史 V2 等于新系统合同 |
| cargo fmt --all -- --check | 通过 | 当前 Rust 骨架格式正常 | 业务实现存在 |
| cargo clippy --workspace --all-targets --all-features --locked -- -D warnings | 通过 | 当前骨架没有 Clippy 警告 | 业务边界、事务或数据可靠性正确 |
| cargo test --workspace --all-targets --all-features --locked | 通过 | 2 个 Bootstrap 单元测试通过 | Capture、PostgreSQL、插件、AI 或真实数据链通过 |
| docker compose ps postgres | 失败：无法连接 Docker daemon | 本轮无法取得实时 PostgreSQL 证明 | 不能据此否定历史 ENV-001 验收；也不能把历史验收当成本轮实时证明 |

## 2.5 没有获得的关键材料

- 新仓库真实 XHS producer fixture；
- 当前 Capture Contract v1 的实际跨语言合同文件；
- 业务 migration、实际表、唯一约束、外键、权限或回滚脚本；
- 真实插件 claim/renew/reconcile/submit/acknowledge 集成测试；
- 迟到有效数据 recovery/import 的服务端合同和持久化证明；
- 完整数量语义的数据字段、API 输出和数据库约束；
- 原始页面/API Artifact 的保留、加密、期限、重放和处置合同；
- 当前运行中的 PostgreSQL 16 副作用证据；
- API、worker、CLI 的业务代码；
- AI 输入集合、版本化结果和不完整样本拒绝/降级的可执行测试；
- 生产备份、恢复、凭证轮换和部署验证。

# 3. 当前唯一事实基线

以下内容可以作为 Baseline v1.0 的候选输入，但必须注意“已确认原则”与“已实现事实”不同。

| 范围 | 建议保留的唯一基线 | 事实等级 |
|---|---|---|
| 产品定义 | 面向垂直领域的持续情报研究系统；第一阶段服务产品负责人及小团队 | 已确认设计 |
| 核心不变量 | 原始事实不覆盖；缺失不等于不存在；部分结果不因任务未达标被丢弃；事实、观察、派生判断和行动分层 | 已确认设计 |
| 领域对象 | Work Order、Attempt、Capture Identity、Package、Record/Artifact、Evidence、Source Identity、Observation、Current revision、Coverage、Claim/Intelligence 等分责 | 已确认语义；物理模型未实现 |
| 数据层级 | 原始来源材料 → 规范化 Observation → Current/read model → AI/Claim/Intelligence；后层不能覆盖前层 | 已确认设计 |
| 核心架构 | Rust 模块化单体 + PostgreSQL 16 + API/worker；浏览器插件是轻量平台执行端 | 已确认方向；未实现 |
| 任务语义 | Work Order 可有多次 Attempt；每次 Attempt 有独立 Capture Identity；同身份同 hash 是 replay，不同 hash 是 conflict | 已确认设计；仅合成 SCOPE 详细化 |
| 插件责任 | 服务端决定任务和权限；插件执行有界 lane；唯一主链为 reconcile → claim → renew → submit/ingest → acknowledge | 已确认方向；真实合同未冻结 |
| AI 权限 | AI 只能产生版本化派生结果，必须带输入范围、Coverage、版本、不确定性和证据引用；不能改原始事实 | 已确认设计；未实现 |
| 第一阶段 | 应先证明一条小而真实的内容详情采集闭环，再扩到评论、作者、媒体、Topic、Corpus 和情报 | 本报告建议；需要用户确认是否替代 synthetic-first 正式基线 |
| 明确不做 | 不迁移旧 Prisma/migration；不双写；不接旧库 fallback；不提前做多租户、Kafka/Temporal、全量 UI 或生产部署 | 已确认边界 |

当前尚不能把 SCOPE-001 直接称为完整 Baseline v1.0，原因是它明确只使用合成 fixture，并排除真实平台、插件、source exhaustion、恢复和多类原始事实；这些排除项恰好包含本次终审要求必须能表达的第一条真实闭环。

# 4. P0 / P1 阻断问题

## 4.1 汇总表

| ID | 等级 | 所属层 | 问题 | 证据 | 真实后果 | 最小修正方案 | 阻止正式编码 |
|---|---|---|---|---|---|---|---|
| IRR-P0-01 | P0 | 事实治理 | 当前文档同时说可以开始 migration/Rust TDD，又说没有真实 producer fixture/字段审计就不能建合同和 DDL | current-state:11；SCOPE:26；contracts:1-3；database/README:11-13；AGENTS:37-44 | 不同 Agent 会分别以“已授权”和“代码事实优先”为依据，实施出不同基线 | 冻结业务编码；一次性裁定并同步 current-state、SCOPE、action-plan、database README 和代码自述 | 是 |
| IRR-P0-02 | P0 | Capture/Coverage | v1 明确排除 source_exhaustion，terminal 只有 target_reached/risk_control，无法表达“目标 100、客观可观察范围只存在 50” | SCOPE:150-173、215-249；原始终审场景 1 | 客观不足会被写成未知剩余、错误达标、风险中断或临时自由文本；下游无法区分场景 A/B | 在正式合同中冻结 observable scope、source exhausted 与 interrupted/remaining 的互斥语义及测试 | 是 |
| IRR-P0-03 | P0 | 实施路径 | 唯一实施切片是 synthetic-only，且被安排在真实 producer 事实审计之前 | SCOPE:49-58、659-668；action-plan:32-47；inventory:24-39 | 可以花费大量工程实现一套自洽合成协议，但真实页面字段、终止、原料、账号镜头和分页一接入就返工 | 先做最小真实 producer 审计并冻结一条 3 个已知公开内容详情的 walking skeleton | 是 |
| IRR-P0-04 | P0 | 恢复/数据保存 | authority 过期后的首次有效回传被拒绝，完整载荷只留 producer 本地，recovery/import 明确后置 | SCOPE:386-390；capture architecture:240-261、641-652 | 浏览器清理、插件卸载、磁盘故障或人工误操作会永久丢失已经取得的真实原料 | 在真实插件接入前冻结加密持久化、reconcile 决策、人工 recovery/import、幂等与到期处置合同 | 是 |
| IRR-P1-01 | P1 | 数量语义 | 没有一套端到端字段区分 target/discovered/visited/requested/returned/valid/persisted/duplicate/invalid/remaining | repo 关键字扫描；SCOPE:215-249；inventory:36 | 插件成功数、回传数、有效数和入库数可能被混用，状态与下游显示失真 | 建立数量词典、单位、归属层、可加性和数据库/API 对账规则 | 是 |
| IRR-P1-02 | P1 | 原始层/重放 | SCOPE Record 只保存合成结构化字段；固定 V2 打包的是本地规范化记录，不是完整原始页面/API；parser v2 重处理后置 | SCOPE:251-286、392-399；inventory:37 | 清洗错误或字段来源争议后不能从原始事实可靠重算；可能只能相信已污染规范数据 | 冻结 Raw Artifact 与 Record/Observation 血缘、保留和 parser interpretation revision | 是 |
| IRR-P1-03 | P1 | 事务/模块责任 | Record processing 需要跨 Source、Observation、Current 和 work finalize 的单事务，但明确 application use case 的所有者未落到唯一模块 | SCOPE:432-447、465-549 | Agent 可能把事务放进 worker、evidence、observation 或 storage，产生循环依赖、半写和重复事实源 | 指定 apps/worker 的 application use case 负责事务编排；领域 adapter 只执行本域 SQL | 是 |
| IRR-P1-04 | P1 | 可复现基线 | HEAD 与 origin/main 相同，但权威 current-state、AGENTS 和索引存在未提交 GOV-002 修改 | git status；current-state 工作副本与 origin/main 不同 | 主线 Agent 在不同 checkout 上读取到不同“当前事实”，无法复核同一结论 | 先独立完成、提交或隔离 GOV-002，再在干净 commit 上重新跑本报告命令 | 是 |
| IRR-P1-05 | P1 | 真实插件状态机 | 插件架构给出候选状态和方向，但状态名可调整，真实恢复窗口、账号镜头、Artifact 和 lane 合同仍待决定 | capture architecture:203-220、641-652 | 进入/退出条件、允许操作、数据结果和恢复规则可能由实现者临时决定 | 为最小真实 lane 冻结一张事件/状态转换表和每条转换的数据库、插件、日志预期 | 是 |
| IRR-P1-06 | P1 | 安全/身份 | SCOPE 只有 loopback 临时 secret；真实插件的设备、账号镜头、凭证存储/轮换、伪造回传和日志边界尚无可执行合同 | SCOPE:86-95；capture architecture:520-531 | 可能混淆账号观察镜头、接受伪造数据，或把 Cookie/token 写入日志和恢复包 | 冻结真实 lane 的设备授权、账号绑定、签名/认证、轮换、脱敏日志和负向测试 | 是 |

## 4.2 IRR-P0-01：现行事实对是否能开工给出相反指令

**证据类型：文档冲突 + 已证实事实。**

- docs/current-state.md:11 说 SCOPE-001 已批准，下一步进入 fixture、migration 与 Rust TDD。
- docs/plans/active/scope-001-content-evidence-vertical-slice.md:26 明确授权创建 migration、fixture、Rust 业务实现。
- crates/contracts/src/lib.rs:1-3 明确说没有真实 producer fixture 就不增加合同，当前仍为 unimplemented。
- crates/evidence/src/lib.rs:1-3 说字段和合同审计后才开始实现。
- crates/storage-postgres/src/lib.rs:1-3 说模型决定关闭后才建立 baseline migration。
- database/README.md:11-13 说 Capture Contract、Coverage、Author/Comment/Media Observation 仍需事实审计，因此不创建业务 DDL；草案不授权 migration。
- AGENTS.md:37-44 规定真实运行、代码、migration、合同和 producer fixture 高于活跃计划与一次性报告。

**具体故障场景：**

Agent A 读取 current-state 和 SCOPE 后开始写 0001 migration；Agent B 读取代码自述、database README 和事实优先级后拒绝创建合同。两者都能声称遵循当前仓库。即使二者分别写得正确，也不会形成同一基线。

**最小修正：**

在任何业务文件变化前形成一次明确裁定：

1. SCOPE-001 是可丢弃的实验室 spike，还是未来真实合同的正式基线；
2. 如果是 spike，文件、表、API 和交付措辞必须与正式命名隔离，不得成为 0001 正式 migration；
3. 如果是正式基线，必须先完成最小真实 producer fixture 审计，并同步更新上述相反指令；
4. 裁定后把被替代文字明确降级，不允许两个最高版本并存。

## 4.3 IRR-P0-02：场景 A 在 v1 中不可表示

**证据类型：已证实事实 + 尚未定义。**

SCOPE-001:173 明确把 source_exhaustion、time_budget、risk_budget 和 probe 排除在 v1 wire enum 外。SCOPE-001:217 只允许 target_reached 与 risk_control 两种 terminal reason。最大配额例子只能写 attempted 50、emitted 50、remainingScope unknown。

**具体故障场景：**

一个真实搜索 lane 上限 100，实际页面/API 在已定义入口和分页规则内已经穷尽，只发现并保存 50。现有 v1：

- 不能写 target_reached，因为没有达到 100；
- 不能写 risk_control，因为没有风险中断；
- 不能证明 remainingScope unknown，因为已定义的可观察范围已经穷尽；
- 不能用 target - emitted 伪造 50 个具体未尝试对象。

因此实现者只能新增自由文本、误用状态或自行扩展 enum。三种做法都会破坏跨插件、服务端、数据库和下游的一致性。

**最小修正：**

正式 Capture 合同至少冻结以下互不替代的维度：

- 目标语义：known set、maximum quota；
- 观察镜头：平台、lane、入口、账号/工位、页/cursor 边界和时间窗；
- 终止原因：target reached、observable scope exhausted、risk stopped、execution interrupted、authorization lost；
- 剩余范围：none within frozen observable scope、known members、unknown；
- 数量单位：发现卡片、详情对象、评论根、回复等不得混减。

必须用“目标 100、客观 50”与“目标 100、中断于 50”两个独立 fixture 和数据库/API/CLI 断言证明不同。

## 4.4 IRR-P0-03：合成证明不能承担第一条真实闭环

**证据类型：已证实事实 + 合理推断。**

- SCOPE-001:49-58 明确不证明真实页面、接口、账号、工位、排序、分页、评论、媒体或插件升级。
- SCOPE-001:659-668 明确完成后也不自动授权真实 producer 或插件。
- docs/migration/action-plan.md:32-40 把合成内核列为当前 Phase 0.75。
- 同文件:42-47 才在后续 Phase 1 审计真实插件字段和 Capture Contract。
- docs/context/current-system-inventory.md:24-39 已列出真实缺口：详情字段来源、评论合成身份、粗粒度 Coverage、规范化打包、observedAt 失真、恢复与真实风控均未闭合。

**具体故障场景：**

团队先把合成 Record、Coverage、JCS、两份 migration、API/CLI 和 worker 全部实现。之后真实插件表明：

- 原始页面/API 需要独立 Artifact；
- observedAt 不是 fixture 的 exact time；
- 搜索发现与详情访问单位不同；
- 评论身份不能稳定；
- source exhaustion 需要新状态；
- 晚到包需要 recovery/import。

这会使第一批表、唯一约束、合同 enum、API envelope 和测试一起返工。合成测试仍可能全部通过。

**最小修正：**

先做一个受控、最小、真实但非生产的 producer audit：单一获准账号/工位、3 个已知公开内容链接、只采详情标题/正文和原始 Artifact，不含搜索、评论、媒体、AI、Topic 或 UI。审计产物应是脱敏 fixture、字段来源表、时间/身份/终止/失败清单和隐私处置记录。它先冻结正式合同输入，再进入 Rust/SQL TDD。

## 4.5 IRR-P0-04：迟到有效原料存在不可恢复丢失窗口

**证据类型：已证实事实 + 合理推断。**

SCOPE-001:388-390 规定 authority 过期后的首次提交不进入普通接入，只保留最小失败回执，完整载荷留在 producer 本地等待未来 recovery/import。capture-plugin-architecture.md:240-261 只有 reconcile 决策名，其中 EXPORT_FOR_RECOVERY 仍是概念方向；:641-652 又明确真实恢复窗口、Artifact 处置和 lane 合同待决定。

**具体故障场景：**

插件在有效租约内采到 50 条，冻结完成时网络中断；服务端租约随后过期。插件恢复后普通提交被拒。若浏览器 profile 被清理、插件升级删除 IndexedDB、机器损坏或用户误清缓存，已经取得的真实原料永久消失。

“本地保留”不是持久化恢复合同。它没有定义：

- 保存到哪里、是否加密、保留多久；
- 哪个身份/hash 与旧 Attempt 对齐；
- 谁能导出、谁能批准导入；
- 导入后如何幂等、如何保留原权力失效事实；
- 何时允许删除本地副本；
- 日志如何避免泄漏原文或凭证。

**最小修正：**

真实插件编码前先冻结最小 recovery/import：

1. 插件对冻结包做持久化、加密和 hash；
2. reconcile 返回 SUBMIT_FROZEN_PACKAGE 或 EXPORT_FOR_RECOVERY；
3. 普通 ingress 仍可拒绝过期权力，但 recovery 以独立身份、审批和审计接纳原料；
4. recovery 不把旧 Attempt 改写成成功，不更换原 Capture Identity，也不制造新观察时间；
5. 服务端 receipt 与本地 acknowledge 成功后才按处置规则清除；
6. 重复导入只得到同一结果，不重复 Evidence。

## 4.6 P1 关闭标准

P1 不能只通过增加一段说明关闭。每个 P1 至少需要：

- 一个唯一负责人或模块；
- 一个明确合同/字段/转换表；
- 正常路径；
- 一个会在旧设计下失败的攻击性负例；
- 数据库、API/CLI 或插件可观察结果；
- 文档中不再存在相反定义。

# 5. 文档冲突与定义歧义

| 冲突或歧义 | 文件 | 可能产生的不同实现 | 推荐保留 | 应降级或修订 |
|---|---|---|---|---|
| 是否已经可以开始正式 migration/业务代码 | current-state:11；SCOPE:26 对比 contracts:1、storage-postgres:1、database/README:11-13 | synthetic-first 正式表 vs producer-first 合同 | 真实 producer fixture 高于合成计划；正式表等待最小事实审计 | “已批准正式实现”应收缩为可丢弃 spike，或在事实审计后重新授权 |
| 项目当前阶段 | current-state 说进入 TDD；实际代码和 DB 仍是空骨架 | 把设计完成当实现开工 vs 继续事实收敛 | “设计基线完成，但实施就绪待 IRR-P0 关闭” | 不再把合成 SCOPE 审查通过等同全项目 IRR 通过 |
| source exhaustion | AGENTS:60 保护来源穷尽语义；SCOPE:173 把它排除在 v1 | 正确区分客观不足 vs 只能写 unknown/risk | 保留 AGENTS 的业务语义并进入首个真实合同 | v1 的排除只可用于纯合成 spike |
| 原始数据与 Record | 产品原则要求原始事实可重放；SCOPE:251-286 只定义结构化 payload；inventory:37 说旧打包不是完整原始页面/API | 保存完整原料 vs 只存规范字段 | 原始 Artifact 与规范 Record 分层并保留血缘 | 不得把 synthetic payload 命名为原始平台事实 |
| 迟到数据 | AGENTS:59/原始终审要求安全原料不丢；SCOPE:390 只本地等待未来方案 | 服务端可恢复 vs 浏览器临时留存 | 独立 recovery/import + 不改写原 Attempt | 不把“本地还在”写成数据已经安全保存 |
| 插件状态 | capture architecture:207 说状态名可调；退出条件又要求语义明确 | 各实现者自由命名/合并状态 | 冻结最小 lane 的事件与状态转换表 | 架构候选继续作为方向，不当可编码协议 |
| 上一份终审的适用范围 | scope-001-final-adversarial-audit:5、11-13 只审正式 SCOPE；本次提示词审整个项目和第一条真实闭环 | “合成切片条件通过”被理解为“整个项目 GO” | 两份报告并存但明确不同审查对象 | 索引说明中不得省略 synthetic-only 口径 |

# 6. 需求追踪矩阵

成熟度缩写：I=设计意图，D=合同/数据定义，C=代码，T=测试或真实运行。

| 核心能力 | 业务规则 | 领域对象 | 数据模型 | 接口/协议 | 状态机 | 异常规则 | 测试 | 当前缺口 |
|---|---|---|---|---|---|---|---|---|
| 关键词监控 | 有限资源、不能把零结果当不存在 | Objective/Plan/Work Order | 草案 | 草案 | 草案 | 有原则 | 无新系统测试 | I；无 D/C/T |
| 博主监控 | 作者身份、作品索引和详情分开 | Author/Content/Observation | 草案 | 固定 V2 仅作证据 | 草案 | SOURCE_INCOMPLETE | 固定 V2 部分测试 | 无新合同/代码 |
| 博主深度建档 | 先链接后详情/评论；Coverage 可解释 | Work Order/Attempt/Package/Author/Content/Comment | 草案 | 架构方向 | 不完整 | 有边界原则 | 无 | 无第一条闭环 |
| 搜索页面探查 | 发现单位不等于详情单位；来源穷尽需证明 | Plan/lane/Coverage | 草案 | 真实 lane 未冻结 | 未冻结 | 终止原因缺失 | 无真实 fixture | P0-02 |
| 作品链接发现 | 链接、平台 ID、对象身份需关联 | Source Identity/Record | 草案 | V2 固定证据 | 未冻结 | 身份冲突有原则 | 新仓库无 | 缺正式 producer fixture |
| 作品详情采集 | 页面类型、目标 ID、原始字段、观察时间 | Record/Artifact/Evidence/Observation | 合成切片只含 title/body | synthetic.v1 | 合成状态详细 | 合成异常较强 | 计划中的 fixture，尚不存在 | 可作为最小真实 skeleton，但尚未冻结 |
| 评论/回复采集 | 主评论、回复、分页和合成 ID不能混用 | Comment Identity/Observation/Coverage | 草案 | V2 有缺陷证据 | 未冻结 | 已知合成 ID 风险 | 固定 V2 部分测试 | 后置，不能开工 |
| 同一作品多时点更新 | Observation 追加；Current 不覆盖历史 | Source Content/Observation/Current revision | 详细草案 | synthetic explain 候选 | worker 状态候选 | 同时刻冲突规则较强 | 尚无 DB 测试 | I/D；无 C/T |
| 手动补充数据 | 人工来源必须保留来源和用途 | Evidence/Observation | 草案 | 未冻结 | 未冻结 | 未定义 | 无 | 后置 |
| 任务拆分与调度 | 有界 Work Order；复杂任务分 lane | Objective/Plan/Work Order | 草案 | 插件架构方向 | 候选 | 有原则 | 无新系统集成 | 无真实 lane |
| 插件领取任务 | capability、账号镜头、唯一 lease | Station/Account/Attempt/lease | 草案 | claim/renew 方向 | 候选 | epoch/fence 原则 | 无真实集成 | P1-05/P1-06 |
| 部分数据回传 | 合格 Record 不因批次缺口丢失 | Package/Record/Coverage | synthetic D 较详细 | POST /v1/capture/packages 候选 | 合成状态 | 部分错误隔离 | 尚无实现测试 | 场景 A 和完整计数缺失 |
| 断点续采 | 同一有效权力短期恢复；其他走新 Attempt/recovery | checkpoint/Attempt/Package | 概念 | reconcile 候选 | 未冻结 | 晚到恢复缺失 | 无 | P0-04 |
| 数据归一化 | 失败不影响原始材料；版本可重算 | Artifact/Record/Interpretation/Observation | Interpretation Revision 后置 | processor v1 | worker 候选 | 第二版本 fail closed | 无 | P1-02 |
| 数据去重 | 网络 replay、对象重复、重观测分开 | Capture Identity/hash/Source Identity | 合成设计较强 | hash/replay 候选 | 合成详细 | conflict fail closed | 计划测试，尚无 | 无真实跨语言/DB 证明 |
| Topic/Corpus/Market Insight/Radar | 读取同一事实链；Coverage 限制解释资格 | Topic/Corpus/Claim/Signal/Intelligence | 草案 | UI/API 草案 | 未实现 | 原则较强 | 无 | 合理后置，不是当前 P0 |
| AI 分类/聚类/判断 | 输入范围、Coverage、模型版本、不确定性、证据关联 | Analysis Run/Claim revision 等 | 草案 | Agent 架构 | 未实现 | 不完整样本不得冒充完整 | 无 | 合理后置；首个 AI 接入前必须补测试 |

结论：目前只有“合成 Content Detail Package → Observation/Current → API/CLI explain”形成了较详细的 I/D 计划，但它没有 C/T，也不覆盖真实 producer。其余核心能力大多停在 I，不能被主线拆成无歧义的正式编码任务。

# 7. 数据模型审查结论

| 审查项 | 当前结论 | 证据等级 | 编码前要求 |
|---|---|---|---|
| 原始数据 | 原则要求保留，但新正式合同没有冻结完整页面/API Artifact | 缺少证据 | 明确 Artifact 内容、hash、存储、访问、期限、加密和处置 |
| 规范数据 | SCOPE 有 title/body synthetic payload 和 processor v1 | 设计/合同候选 | 不得冒充真实 producer；先用真实脱敏 fixture 校验 |
| 对象身份 | 平台 ID 优先、unresolved 不造对象的原则强 | 设计意图 | 用真实链接/ID/页面目标错配 fixture 证明 |
| 多时点快照 | Observation append-only、Current revision 和字段来源设计较完整 | 设计意图 | 用真实 PostgreSQL 测试迟到、同时间冲突、字段缺失和 pointer 原子性 |
| 覆盖缺口 | known set 与 maximum quota 有部分定义 | 合成合同候选 | 加入 source exhausted、execution interrupted 和真实 lane/cursor 镜头 |
| 数量语义 | 不完整 | 缺少证据 | 冻结完整词典和跨层对账；不得只用 attempted/emitted |
| 幂等 | Capture Identity + canonical/hash 设计较强 | 合成合同候选 | 独立跨语言 fixture、100 并发回传和真实 DB 证明 |
| 去重 | 网络 replay、对象身份和重观测在原则上分开 | 设计意图 | 定义 duplicate_count 属于哪一层及其与 persisted/Observation 的关系 |
| 事务 | Package ingress 边界较清楚；Record processing owner 不清楚 | 设计冲突 | 指定 application transaction owner 并做故障注入 |
| AI 推导数据 | 与事实分层原则很强 | 设计意图 | 首个 AI slice 冻结输入集合、Coverage、模型/规则版本和不可覆盖结果 |
| 数据重放 | SCOPE 明确后置第二 processor version | 尚未定义 | 正式原始数据入库前先定义 Interpretation Revision 或等价重处理身份 |
| 数据追溯 | SCOPE 目标是 Current → Observation → Record → Package | 合成设计 | 补到 Raw Artifact、producer build、station/account lens、Attempt 和 recovery receipt |

## 数量词典的最小建议

字段名称可以改变，但语义不能省略或互相替代。

| 语义 | 所属层 | 最小定义 |
|---|---|---|
| target_count | Work Order | 人在执行前冻结的目标或上限；必须带 basis 和 unit |
| discovered_count | producer discovery | 在冻结入口/镜头中发现的候选数 |
| visited_count | producer execution | 实际打开或调用的目标数 |
| requested_count | transport/API | 实际向平台或页面请求的单位数；不得与对象数混用 |
| returned_count | producer/package | producer 交付的 Record 数 |
| valid_count | validation/processing | 通过指定合同和身份资格的 Record 数 |
| persisted_count | ingress/storage | 实际持久化且取得 receipt 的 Record/Evidence 数 |
| duplicate_count | 明确层级 | 必须区分网络 replay、包内重复、对象已存在和相同 Observation |
| invalid_count | validation/processing | 因合同、身份或来源关系不合格的数量，并带原因 |
| remaining_count | 仅已知集合 | 只有冻结成员可计算；未知范围不得伪造数字 |

对 maximum quota + source exhausted，应该表达“上限 100、在冻结观察镜头内穷尽、发现/访问/返回 50、该镜头内剩余 0”，不能表达“平台总共只有 50”。对中断场景，应该表达剩余 known members 或 unknown，不能写 exhausted。

# 8. 任务状态机与插件协议结论

## 8.1 当前能确认的语义模型

~~~text
Objective / Collection Plan
  → bounded Work Order
    → 0..N Attempt
      → Capture Identity + lease epoch + Station/Account lens
      → local run/checkpoint/risk event
      → 0..1 terminal frozen Package
        → 0..N ingress Delivery
        → Record / Artifact / Coverage
          → processing work
          → Source Identity / Observation / Current
~~~

这个分责方向是合理的，也比一个 SUCCESS/FAILED 状态更可靠。但真实 lane 尚没有可执行的转换表。

## 8.2 必须补齐的状态/事件

- observable_scope_exhausted：已冻结镜头内确实无更多可访问范围；
- execution_interrupted：仍有已知或未知范围，但执行被崩溃、浏览器关闭或页面异常中断；
- risk_paused / risk_stopped：风控触发后停止新访问，已有原料仍进入冻结/恢复流程；
- authority_lost：失去执行权，与原料真实性分开；
- frozen_pending_delivery：包已经冻结但尚未取得服务端 accepted/recovery receipt；
- recovery_required / recovery_accepted / recovery_rejected：普通接入之外的迟到原料处置；
- processing_partial：Package 已接纳但部分 Record 无效或仍待处理；
- coverage_incomplete：任务或 Work Order 未满足，不等于已接纳 Record 失败。

名称不是强制，语义和转换证据必须存在。

## 8.3 规则结论

| 主题 | 当前结论 | 缺口 |
|---|---|---|
| 重试 | 同冻结 Package 同 hash 重传；新观察应新 Attempt | 真实插件与服务端集成未证明 |
| 补采 | 新 Attempt、新 Capture Identity、新 Package，不改旧包 | 如何只覆盖缺失范围需真实 lane/known target 规则 |
| 租约 | claim/renew/epoch/fence 方向明确 | authority 在采集、冻结、提交交错时的真实测试缺失 |
| 迟到数据 | 普通 ingress 失败关闭 | recovery/import 未定义，构成 P0 |
| 重复数据 | replay/conflict/Source Identity 原则明确 | duplicate_count 和真实 DB 证明缺失 |
| 风控中断 | 应停止、保存合格部分、记录 Coverage | 冷却/恢复窗口、账号镜头和包冻结触发未冻结 |
| 数据保存与任务状态 | 原则上分开；Package accepted 不等于 Work satisfied | 跨模块事务 owner 和状态对账缺失 |
| 插件成功与数量 | 不应相信单一 success | 缺 returned/valid/persisted 对账和失败关闭测试 |

# 9. 对抗场景结果

| 场景 | 当前设计是否有答案 | 正确预期 | 当前风险 | 等级 |
|---|---|---|---|---|
| 1. 目标 100，客观只能发现 50 且范围穷尽 | **没有可实施答案** | 保存 50；记录上限 100、冻结镜头内穷尽、实际各层数量和不代表平台总体 | v1 无 source_exhaustion，只能误写 unknown/risk/target reached | P0 |
| 2. 目标 100，取得 50 后崩溃，仍有范围 | 部分有 | 冻结并保存 50；状态为执行中断；保留 known/unknown 剩余；新 Attempt 补采 | 真实插件 checkpoint/恢复/回传路径未冻结 | P1 |
| 3. 50 条中 40 有效、5 重复、3 不完整、2 解析失败 | 部分有 | Package 原子保存原料；逐 Record 隔离；分别记录 returned/valid/duplicate/invalid/persisted | 完整数量词典和原始层缺失 | P1 |
| 4. 重试返回 30 重叠、20 新增 | 原则有，证明无 | 网络 replay 不重写；对象重复不复制身份；新观察按资格追加 | 没有真实聚合规则、约束和测试 | P1 |
| 5. 租约超时，旧插件迟到有效数据 | **没有安全闭环** | 普通权力路径拒绝，但冻结原料进入可审计 recovery/import；不改写旧 Attempt | 载荷只留本地，存在永久丢失 | P0 |
| 6. 两插件重复领取并回传 | 设计方向有 | 服务端唯一有效 lease/epoch；至多一个 accepted；另一方停止或 recovery | 无真实 PostgreSQL + 插件并发证明 | P1 |
| 7. 插件报告成功但数量不一致 | 没有完整答案 | 服务端按 Package、Record、validation、DB receipt 重算；不采信 success | 缺数量对账与不一致终态 | P1 |
| 8. 数据保存成功但状态更新失败 | 合成 Package 设计部分覆盖 | 需要原子关系或可重放派生；绝不能显示失败并重复写数据 | Record processing 跨模块事务 owner 未冻结 | P1 |
| 9. 标记完成但评论分页未访问 | 原则有，真实合同无 | Coverage 标记未穷尽/未知，Work 不 satisfied；已保存评论仍可限定用途 | 真实 cursor/hasMore 与 terminal 合同未冻结 | P1 |
| 10. 同作品二次采集，正文/指标/评论变化 | 部分设计有 | 新 Observation；Current 按版本规则更新；消失保持未知/不可访问观察，不删除历史 | SCOPE 只覆盖 title/body；指标/评论/删除后置 | P1 |
| 11. 归一化错误需重处理历史原料 | **当前切片没有** | 从不可变 Raw Artifact 产生新 Interpretation Revision，不改旧 Observation | 原始 Artifact 与重处理身份后置 | P1 |
| 12. AI 用不完整样本得出趋势 | 设计原则有 | 保存为受限候选或拒绝正式发布；展示 Coverage、反例、模型版本和不确定性 | 无代码/测试；但 AI 未获准开始 | P2，AI 开工前升为 P1 |
| 13. 风控触发，已有部分数据 | 部分有 | 停止新访问；冻结/保存合格部分；记录 risk stop 和剩余范围；等待重新授权 | 真实插件恢复与冷却参数未冻结 | P1 |
| 14. 作品删除、私密或账号注销 | 只有领域意图 | 保留历史；新观察记录不可访问及证据资格；不得从访问失败直接断言删除原因 | producer 信号、身份和处置规则无测试 | P1 |
| 15. 搜索显示 50 链接，去重后 32 作品 | 原则有，链路无 | discovered 50、identity-resolved unique 32；不得把 32 当 visited/returned；详情另行准入 | 数量层级和真实 discovery lane 未冻结 | P1 |

# 10. 第一条最小真实闭环

## 10.1 推荐 Walking Skeleton

**起点：** 人工授权在一个已知账号/工位和短时间窗内读取 3 个已确认公开的小红书内容详情链接。

**终点：** API/CLI 能解释 3 个目标中实际访问、返回、接纳、处理、持久化和剩余多少；每个采用字段可追到原始 Artifact、producer build、Attempt、Package、Record 和 Observation；重复回传不增加行；风险中断后可新 Attempt 补采。

**真实数据边界：**

- 只使用 3 个经用户批准的公开内容详情；
- 只保留最小必要 title/body 和受限原始 Artifact；
- 单一账号、单一工位、单一 lane；
- 明确开始/结束时间、访问者、用途、保留期和删除方式；
- 原始内容不进入 Git、普通日志、外部 Agent 或长期提示记录。

## 10.2 逐步输入与输出

| 步骤 | 输入 | 输出 | 验收证据 |
|---|---|---|---|
| 1. 创建任务 | 3 个冻结链接、权限、账号镜头、时间窗 | Work Order + target manifest | DB 中目标数 3、hash、unit、权限边界 |
| 2. 任务拆分 | 已知集合 | 1 个有界 content-detail Work Order 或 3 个明确 target | 每个 target ordinal/ID 可追踪 |
| 3. 插件领取 | station、account、capability | Attempt + Capture Identity + lease epoch | 唯一有效执行权；错误账号/能力被拒 |
| 4. 真实采集 | 前两个链接 | 2 个 Raw Artifact + 2 个 Record | 页面目标 ID 对账；原始 hash；实际 observed time |
| 5. 风险停止 | 第三个未访问，触发合成或受控停止条件 | terminal frozen Package，risk stopped，remaining known 1 | 插件不再访问；Package 内容不可变 |
| 6. 回传保存 | frozen Package | ingress receipt、Artifact/Record/Coverage、processing work | 同事务；故障点无半包 |
| 7. 规范化 | 2 个 Record + parser v1 | 2 个合格 Observation + Current | title/body 来源可追溯；失败 Record 不连坐 |
| 8. 状态与覆盖 | target 3、visited 2 | visited/returned/valid/persisted 2，remaining 1，risk stopped | DB/API/CLI 三者一致 |
| 9. 查询展示 | content ref | 当前值、来源、Coverage、limitations | 不宣称 3/3 或平台完整 |
| 10. 补采 | 第三个 target + 新授权 | 新 Attempt、新 Capture Identity、新 Package | 不修改第一次 Package；Work satisfaction 重算 |
| 11. 去重验证 | 重放同一 Package | 原 receipt | Package/Record/Evidence/Observation 行数不增加 |
| 12. 多时点验证 | 再次合法观察其中一条 | 新 Observation，必要时新 Current revision | 对象身份不复制；历史不覆盖 |

## 10.3 涉及模块

- contracts：真实 producer fixture 与运行时校验；
- evidence：Package/Artifact/Record/Coverage ingress；
- observation：Source Identity、Observation、Current；
- storage-postgres：pool、事务上下文、proof database helper；
- apps/api：有边界任务/回传/查询入口；
- apps/worker：Record processing application transaction；
- apps/cli：只通过 API 查询；
- 轻量插件 adapter：capability、claim/renew/reconcile、采集、冻结、submit/acknowledge。

## 10.4 暂不涉及

- 搜索、关键词监控、评论/回复、作者全量、媒体下载；
- Topic、Corpus、聚类、AI Agent、Signal、Market Insight、Radar；
- Web UI、多租户、正式趋势、生产部署、旧库退役；
- 自动扩采、切号、风控绕过或全账号发布。

## 10.5 失败和恢复

- 插件在冻结前崩溃：保存受限 checkpoint 和已经取得的原始材料摘要；reconcile 决定继续、冻结或 recovery；
- Package 已冻结但网络失败：同 hash 重传，不生成新 Attempt；
- authority 过期：普通 ingress 拒绝；走有审计的 recovery/import，不删除本地包；
- Package 接入事务失败：所有 accepted 业务行归零，重传同一包；
- 单 Record 解析失败：原料与 Package 保留，其他 Record 继续；失败原因可查；
- worker lease 丢失：新 epoch 接管，旧 epoch 不能写；
- 补采：只创建新 Attempt/Package，不修改第一次 Coverage。

# 11. 首轮强制验收测试

以下测试都必须使用真实 PostgreSQL 16；涉及 producer 的测试使用受控真实 fixture 或由真实 producer 捕获后脱敏冻结的 fixture。每个测试必须能单独运行，不能依赖前一个测试留下的状态。

## T-01 正常完整采集

- 前置条件：3 个 known targets，station/account/capability 合格，authority 有效。
- 输入：3 个合法真实 fixture Artifact/Record。
- 执行动作：claim → collect → freeze → submit → process → query。
- 数据库预期：1 Package、3 Record、3 Artifact、3 合格 Observation；Current 指向已发布 revision。
- 任务状态预期：Attempt accepted；Work satisfied；Coverage 3/3。
- 日志预期：只有 ID/hash/计数/阶段，不含 Cookie、token 或完整原文。
- 下游可见结果：target/visited/returned/valid/persisted 均为 3，provenance 完整。
- 失败判定：任一数量不一致、字段无来源、日志泄密或出现半包。

## T-02 客观数量不足

- 前置条件：maximum quota 100；冻结观察镜头在第 50 个对象后可证明穷尽。
- 输入：50 个合法对象和明确的终止证据。
- 执行动作：遍历到来源穷尽并提交。
- 数据库预期：50 条合格原料/记录；terminal 为 observable scope exhausted；没有伪造 50 个对象。
- 任务状态预期：执行完成已观察范围，但未达到配额；不标普通失败或 target reached。
- 日志预期：记录 lane、cursor/页面边界、终止依据和计数。
- 下游可见结果：可使用 50 条，但明确只代表冻结观察镜头，不代表平台总量。
- 失败判定：remaining 被写成 50 个对象、状态为 target reached/risk stopped，或 50 条被丢弃。

## T-03 执行中断

- 前置条件：目标 100；已取得 50；仍有已知或未知未访问范围。
- 输入：插件崩溃/浏览器关闭故障注入。
- 执行动作：恢复插件并 reconcile。
- 数据库预期：已冻结的 50 条可接纳；Coverage 保留未完成；新补采使用新 Attempt。
- 任务状态预期：execution interrupted 或 recovery required，不是 source exhausted。
- 日志预期：故障点、最后 checkpoint、Attempt/epoch 和剩余范围类型。
- 下游可见结果：50 条可限定使用；明确 incomplete。
- 失败判定：已取得数据丢失、旧 Package 被修改或重试重复插入。

## T-04 重复回传

- 前置条件：一个 Package 已 accepted。
- 输入：同 Capture Identity、同 canonical bytes/hash 重传 100 次。
- 执行动作：并发提交。
- 数据库预期：1 Package/Record/Artifact/work/Observation 集合；可追加最小 Delivery 记录。
- 任务状态预期：原 accepted receipt；重放不改变 Work satisfaction。
- 日志预期：replay 计数，不打印 payload。
- 下游可见结果：内容和 Coverage 不变。
- 失败判定：任一业务行重复或返回 conflict。

## T-05 延迟回传

- 前置条件：插件在 authority 有效期取得并冻结 Package，首次提交时已过期。
- 输入：真实有效的迟到冻结包。
- 执行动作：普通 submit 后执行 recovery/import。
- 数据库预期：普通路径无 accepted 副作用；recovery 保留原 Attempt/Capture/hash 和独立审批/receipt；重复 recovery 幂等。
- 任务状态预期：旧 Attempt 不被改写为普通成功；原料恢复结果可查。
- 日志预期：权力失效和恢复决策，不含原文。
- 下游可见结果：材料若通过恢复资格可用，同时显示 recovery provenance。
- 失败判定：数据永久留在临时浏览器、静默改挂新 Attempt 或重复 Evidence。

## T-06 并发回传

- 前置条件：同一 Work Order 被故障注入为两个竞争 claim。
- 输入：两个 station/epoch 并发提交。
- 执行动作：claim/submit 交错。
- 数据库预期：至多一个有效 epoch/accepted Package；旧 epoch 写入为零。
- 任务状态预期：胜者明确；另一方 lost authority/conflict/recovery decision。
- 日志预期：两个执行身份、epoch 和裁定。
- 下游可见结果：只有一条权威事实链。
- 失败判定：两个 accepted Package、last-write-wins 或身份混淆。

## T-07 部分字段错误

- 前置条件：同包 50 条，包含 40 合格、5 对象重复、3 字段不完整、2 解析失败。
- 输入：固定混合 fixture。
- 执行动作：Package 接入后逐 Record 处理。
- 数据库预期：原始 Package/Artifact/Record 全部按政策保存；合格 Observation 不被坏 Record 回滚；每类结果计数可对账。
- 任务状态预期：Package accepted，processing partial，Work/用途资格单独计算。
- 日志预期：每个错误引用 record ID 和类型，不打印原文。
- 下游可见结果：40 条合格结果及完整 returned/valid/duplicate/invalid/persisted 说明。
- 失败判定：整包回滚、坏记录生成空身份，或只显示 success 50。

## T-08 数据保存成功但状态更新失败

- 前置条件：在 Package、Record、Coverage、processing work 和状态关系之间设置故障点。
- 输入：合法 Package。
- 执行动作：逐个故障点注入并重试。
- 数据库预期：需要原子的部分全部回滚；非同事务派生可由权威 receipt 重放，不重复业务事实。
- 任务状态预期：不能显示 failed 后再重复写；也不能显示 complete 而无业务行。
- 日志预期：事务/receipt ID、故障阶段和可恢复动作。
- 下游可见结果：要么完整 accepted，要么明确 pending/retry，不见半包。
- 失败判定：数据与状态永久分叉或只能人工改库。

## T-09 同一对象多时点更新

- 前置条件：同一平台对象已有 Observation。
- 输入：第二次真实 fixture，正文变化、指标变化、部分评论不可见、新回复出现。
- 执行动作：新 Attempt/Package/processing。
- 数据库预期：对象身份唯一；追加新 Observation；旧事实不删除；Current 按固定 policy 发布新 revision。
- 任务状态预期：第二次 Attempt 独立完成。
- 日志预期：新旧 Observation 和 policy version。
- 下游可见结果：可查看变化及缺失/不可访问的证据边界。
- 失败判定：UPDATE 覆盖历史、把未看到旧评论写成删除事实，或重复 Source Identity。

## T-10 归一化重新处理

- 前置条件：Raw Artifact 已冻结；processor v1 有已知错误。
- 输入：processor v2 或 interpretation rule v2。
- 执行动作：对同一原始材料运行新版本。
- 数据库预期：原始 Artifact 不变；新增 Interpretation Revision/派生结果；v1 结果可审计。
- 任务状态预期：不是新世界 Observation，除非发生新平台观察。
- 日志预期：输入 artifact hash、旧/新 processor version、结果 ID。
- 下游可见结果：当前读取明确采用哪个解释版本。
- 失败判定：覆盖 v1、伪造新 observedAt 或无法回到原始材料。

## T-11 AI 使用不完整样本

- 前置条件：Coverage 明确不完整，50/100 且仍有未知范围。
- 输入：请求生成趋势/市场结论。
- 执行动作：运行候选分析和正式发布门。
- 数据库预期：若允许分析则保存输入集合、Coverage、模型/规则版本、不确定性和证据引用；正式趋势资格为未成立。
- 任务状态预期：分析运行可以成功，但 Claim/Intelligence 不自动正式。
- 日志预期：输入集合 ID、模型/规则版本、资格判定，不复制全部原文。
- 下游可见结果：只能看到受限候选和缺口，或明确拒绝形成趋势。
- 失败判定：把部分样本写成完整市场、覆盖旧 AI 结果或隐藏 Coverage。

# 12. 过度设计与可延后内容

## 必须保留

- Raw Artifact、Record、Observation、Current 和 AI 派生分层；
- 部分结果保存与 Coverage；
- Work Order、Attempt、Capture Identity、Package、Delivery 分责；
- canonical/hash、幂等、并发 fence；
- 多时点 Observation 和字段来源；
- 真实插件的账号/工位镜头、租约、checkpoint、recovery；
- 下游用途资格与样本完整性分开。

## 可以简化

- 第一条真实闭环只做 3 个已知内容详情，不做搜索、评论、作者全量和媒体；
- 一个 PostgreSQL 16，不引入消息中间件；
- API/worker 可以先是同一部署单元，但事务责任要清楚；
- CLI 只读 API，不建第二事实源；
- Current 首轮只处理 title/body。

## 可以延后

- Topic、Corpus、聚类、Signal、Market Insight、Radar；
- Web UI 和复杂运营中心；
- 多租户、企业级 IAM、正式趋势统计；
- 评论/回复、媒体字节下载、作者深档；
- 大规模调度、Kafka、Temporal、Redis、对象存储 SDK；
- 生产部署、RPO/RTO 完整体系。

## 建议删除或避免

- 把 synthetic.v1 直接命名为正式真实 Capture v1 的做法；
- 没有调用者的空模块、万能 repository、万能 workflow 或自由 JSON job；
- 用 success/failed/partial_success 一个字段承载任务、包、记录、处理和用途；
- 用 target - emitted 计算未知剩余；
- 让工作台、CLI 或 AI 缓存成为第二事实源。

## 需要真实数据后再决定

- 每个真实 lane 的字段集合和来源优先级；
- 搜索/API/页面之间的可比性；
- 评论分页与回复上限；
- 安全访问节奏和冷却时间；
- Raw Artifact 最终保留期限、加密和访问角色；
- API/worker 是否拆数据库角色；
- 百万级数据下是否需要额外分区、队列或存储。

# 13. 需要用户决定的问题

纯技术问题已在本报告给出推荐，不推回给用户。只有以下三项会改变产品边界、风险承受或上线顺序。

## DEC-IRR-01：SCOPE-001 是实验室 spike 还是正式产品基线

| 选项 | 收益 | 风险 |
|---|---|---|
| A. 可丢弃 synthetic spike | 可以快速验证 JCS、事务和 Rust/SQLx 技术 | 容易被误当正式 0001；需要隔离命名、目录和交付口径 |
| B. 正式产品基线 | 所有首批代码可延续到真实闭环 | 必须先完成最小真实 producer audit，开工时间稍后 |

**推荐：B。**

理由：当前最昂贵的风险不是 Rust/SQLx 能否工作，而是合成合同与真实平台字段、原料、终止和恢复不一致。先取得 3 个真实、受控、脱敏 fixture，成本远低于重做第一批 migration 和跨边界合同。

不决定的后果：现有文档继续允许两种相反实施。

## DEC-IRR-02：是否授权最小真实 XHS producer 审计

| 选项 | 收益 | 风险 |
|---|---|---|
| A. 授权 1 个账号/工位、3 个已知公开详情、短窗口 | 能冻结第一条真实合同和失败边界 | 涉及真实平台访问、账号风控和敏感原文，必须最小化与受控 |
| B. 暂不授权 | 没有平台访问风险 | 只能继续做非正式 spike，不能满足本次正式编码 GO 标准 |

**推荐：A，但仅限 3 个经批准公开链接，不含搜索、评论、媒体、批量、AI 或外部 Agent。**

不决定的后果：IRR-P0-03 无法关闭。

## DEC-IRR-03：真实审计原始 Artifact 的临时保留策略

| 选项 | 收益 | 风险 |
|---|---|---|
| A. 本机受限加密保存 7 天，验收后删除，必要时显式延期 | 能重放合同且暴露时间短 | 需要实现最小访问和删除记录 |
| B. 只保留脱敏字段，不留原始 Artifact | 隐私风险较低 | 无法验证字段来源、重放或修复归一化 |
| C. 长期保存原始 Artifact | 调试最方便 | 当前没有足够法律、访问、加密和处置依据 |

**推荐：A。** 这只是工程和风险控制建议，不代替法律意见。若材料涉及儿童、医疗或可反向识别内容，应进一步收缩样本和可见范围。

不决定的后果：真实 producer audit 无法满足原始事实和可重放要求。

# 14. 缺失证据与未验证假设

## 14.1 未找到的文件或实现

- database/migrations/ 下的业务 migration；
- apps/cli/src/main.rs 及 CLI crate；
- Capture Contract v1 正式文件；
- 新仓库 producer fixture；
- Raw Artifact schema；
- recovery/import 合同；
- 真实插件状态转换表；
- 真实插件认证/账号镜头合同；
- PostgreSQL 集成测试脚本的实际实现结果；
- API/worker 业务 route、handler 和处理器；
- AI 输入集合/派生结果 schema 和测试。

## 14.2 未定义的规则

- source exhausted 的可观察镜头和证明条件；
- returned、valid、persisted、duplicate、invalid 的唯一口径；
- 浏览器本地冻结包的加密、保留、删除和 recovery；
- processor 新版本如何重放而不伪造新 Observation；
- Record processing 跨域事务的唯一应用层 owner；
- 作品删除、私密、账号注销分别能由哪些 producer 证据确认；
- 真实插件错误账号、错误页面、错误 capability 和伪造回传的处理；
- 真实 lane 的 checkpoint 粒度、短期恢复窗口和冷却规则。

## 14.3 未验证技术假设

- 选定 Rust JCS 实现与真实 TypeScript producer 能字节一致；
- PostgreSQL 组合约束能完整表达 Attempt/Package/Target/Record 同属；
- API/worker 共用 runtime 角色仍能满足最小权限；
- 浏览器插件能可靠冻结大包、加密持久化并在 MV3 休眠后恢复；
- 真实 XHS 详情字段、时间、页面身份和原始 Artifact 可稳定取得；
- 当前本机 PostgreSQL 16 仍可运行并从空库重放未来 migration。

## 14.4 当前无法确认

- 平台当前真实页面结构、API 字段、排序和风控阈值；
- 账号间搜索/内容可见性是否可比；
- 评论分页是否可客观证明穷尽；
- 原始材料的最终合规保留期限；
- 生产备份/RPO/RTO；
- 第一阶段真实数据规模与是否需要额外基础设施。

# 15. 最终开工边界

## 15.1 编码前必须完成

按执行顺序：

1. **关闭事实冲突**：裁定 SCOPE-001 是 spike 还是真实正式基线，并同步所有相反文档和代码自述。
2. **取得最小真实 producer 证据**：若用户授权，完成 3 个公开详情的受控审计、脱敏 fixture、字段来源、时间、身份、终止、失败和处置记录。
3. **冻结 Capture/Coverage v1**：能分别表达客观来源穷尽、执行中断、风险停止、目标达成和权力丢失。
4. **冻结数量词典**：十类数量的单位、层级、计算、可加性和 DB/API/CLI 对账。
5. **冻结原始与重处理合同**：Raw Artifact、Record、Observation、Interpretation Revision、保留和血缘。
6. **冻结真实插件恢复合同**：reconcile、frozen package、late recovery/import、acknowledge、幂等和本地处置。
7. **指定事务与模块 owner**：Record processing application transaction 只能有一个编排责任。
8. **冻结最小插件状态/安全表**：每个事件的进入、退出、允许/禁止动作、DB/插件/日志结果。
9. **把第 11 节测试变成可执行规格**：至少先写会失败的合同、状态和 PostgreSQL 测试。
10. **恢复干净可复现基线**：完成或隔离 GOV-002，在明确 commit 上重跑 governance、bootstrap、cargo 和 PostgreSQL 验证。
11. **重新做开工门判断**：所有 P0 为 DISPROVED 或 RESOLVED，P1 有明确临时规则、负责人和验证方法后，才能给 CONDITIONAL GO/GO。

## 15.2 现在可以开始的任务

当前可以立即开始，但不产生正式业务 schema 或业务实现：

- 只读真实 producer 事实审计的准备和授权清单；
- 3 个已知详情 walking skeleton 的数据最小化、脱敏和处置方案；
- source exhaustion 与 interruption 的合同样例和攻击性测试规格；
- 数量语义词典和跨层对账表；
- recovery/import 时序、状态转换和安全威胁用例；
- Record processing 事务 owner 的技术裁定；
- 清理/提交或隔离现有 GOV-002 文档变更；
- 启动 Docker 后重新验证 PostgreSQL 16 环境，不创建正式业务表。

如果用户明确选择“可丢弃 synthetic spike”，可以单独实施实验，但必须满足：

- 不使用正式 0001 migration 名称；
- 不宣称 Capture v1 或正式产品基线；
- 不进入 production/mainline 依赖；
- 有明确删除或替换条件；
- 不因此跳过真实 producer audit。

## 15.3 现在禁止开始的任务

- 正式业务 migration 和长期表结构；
- 把 synthetic content-detail.synthetic.v1 发布为真实 Capture v1；
- 正式 Evidence/Observation ingress 和 Current 生产接口；
- 真实插件大改、批量采集、账号扩张或全量发布；
- 搜索、评论、作者深档、媒体的正式新合同；
- Topic、Corpus、AI Agent、聚类、趋势、Market Insight 或 Radar 实现；
- 生产部署、旧库迁移/退役、双写或历史数据回填；
- 任何把迟到有效数据只留在浏览器临时状态的真实运行；
- 任何未冻结账号镜头和认证边界的真实回传。

## 15.4 第一阶段完成标准

项目只有在以下证据同时存在时，才能从“设计阶段”进入“核心闭环已验证阶段”：

1. 一个干净、可复现的 commit 包含唯一正式合同；
2. 真实 producer 捕获并脱敏冻结的最小 fixture 可跨 TypeScript/Rust 验证；
3. 空 PostgreSQL 16 proof database 能重放 migration；
4. walking skeleton 真实经过 create → split → claim → collect → partial/full submit → raw save → normalize → coverage/status → query →补采；
5. 11 类强制测试通过，特别是 source exhausted、interrupted、late recovery 和 DB/status 故障；
6. 同 Package 重放、并发旧 epoch、对象重复和多时点 Observation 都有真实 DB 副作用证明；
7. Raw Artifact 可受控读取、重处理和到期处置，不进入 Git/普通日志；
8. API/CLI 显示 target、实际各层数量、Coverage、limitations、provenance 和 processing；
9. 插件/账号/工位/Attempt 可追踪，凭证与原文不泄漏；
10. 没有把完成一条真实内容详情闭环误报为搜索、评论、AI、情报或生产已上线。

# 16. 主线 Agent 复核协议

## 16.1 复核命令

在仓库根目录运行：

~~~bash
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main

rg -n "target_count|discovered_count|visited_count|requested_count|returned_count|valid_count|persisted_count|duplicate_count|invalid_count|remaining_count|source_exhaustion" . --glob "!target/**" --glob "!references/**"

rg --files database crates apps | sort

./scripts/check-project-governance.sh
./scripts/verify-bootstrap.sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
docker compose ps postgres
~~~

如果 Docker 未运行，只能标记 PostgreSQL 当前状态 UNVERIFIED；不得把历史 ENV-001 成功当作本轮实时数据库证明，也不得仅因 Docker 未运行就否定历史验收。

## 16.2 逐项复核输出格式

主线 Agent 应输出：

| Issue ID | 复核结果 | 当前最高优先级证据 | 原报告是否过期 | 是否改变 Gate |
|---|---|---|---|---|
| IRR-P0-01 | CONFIRMED / DISPROVED / STALE / DECISION_REQUIRED | 文件:行号、测试或 DB 副作用 | 是/否 | 是/否 |

四种结果的含义：

- CONFIRMED：当前证据仍支持问题；
- DISPROVED：存在更高优先级证据证明问题描述错误；
- STALE：问题曾成立，但当前 commit 已解决；
- DECISION_REQUIRED：事实明确，但需要用户选择产品边界或风险承受。

## 16.3 推翻 NO-GO 的条件

满足以下任一情况不能单独推翻本报告：

- 又一份设计文档说问题已经解决；
- 合成 fixture 通过；
- cargo test 通过但没有业务测试；
- HTTP 200、worker succeeded 或 UI toast；
- 历史 V2 代码曾经支持类似能力；
- Docker/PostgreSQL 可以启动；
- 用户说“可以编码”但现行合同仍互相冲突。

只有当 4 个 P0 都被当前最高优先级证据 DISPROVED 或 RESOLVED，并且 P1 对第一条真实闭环有明确临时规则、owner 和验证方法时，才能重新给出 CONDITIONAL GO 或 GO。

## 16.4 建议的主线最终答复

主线 Agent 核实后应明确回答：

1. 本报告哪些问题被确认、推翻或已过期；
2. 当前唯一事实基线是哪一个 commit；
3. 是否仍为 NO-GO；
4. 如果改为 CONDITIONAL GO，现在精确允许哪些文件、数据、账号和测试；
5. 哪个证据完成后才能扩大范围；
6. 是否需要更新 docs/current-state.md、SCOPE、database/README 和既有审查索引。

---

本报告没有修改业务代码、数据库、SCOPE、产品决定或既有 GOV-002 内容。它只记录本轮实施就绪审查的结论、证据、可证伪条件和主线复核方法。
