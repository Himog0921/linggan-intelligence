# 文档总索引

> 状态: 权威当前
> 最后核对: 2026-08-20
> 适用范围: 全仓库文档导航、权威状态与渐进式披露
> 事实来源: 当前 Git 文件树、`AGENTS.md` 与文档状态头
> 冲突时以谁为准: `AGENTS.md` 和真实代码、合同、测试、运行结果

本页是项目的“总书目”。Agent 不应先通读所有材料，而应从这里逐层进入与当前事项直接相关的文件。

## 固定阅读顺序

1. 根目录 [`AGENTS.md`](../AGENTS.md)：最高约束、事实优先级和不可违反规则。
2. [`current-state.md`](current-state.md)：当前阶段、唯一在办事项、阻塞和下一步。
3. 本页对应分类中的权威文档。
4. 只有当前任务明确需要来源核对时，才进入 [`references/README.md`](../references/README.md)。

## 当前权威与状态

| 文档 | 状态 | 用途 |
|---|---|---|
| [`current-state.md`](current-state.md) | 权威当前 | 当前阶段、事项队列和决策缺口 |
| [`governance/file-placement-standard.md`](governance/file-placement-standard.md) | 权威当前 | 文件分类、命名、放置、归档与生成物规则 |
| [`governance/agent-collaboration.md`](governance/agent-collaboration.md) | 权威当前 | Agent 读取、任务、冲突、更新和验收流程 |
| [`governance/generated-artifacts-registry.md`](governance/generated-artifacts-registry.md) | 权威当前 | 所有生成型文件的固定位置与入库许可 |
| [`context/START-HERE.md`](context/START-HERE.md) | 权威当前 | 新机器和新 Agent 的项目背景入口 |
| [`context/current-system-inventory.md`](context/current-system-inventory.md) | 代码事实优先 | 固定来源资产及已证实能力盘点 |
| [`context/discussion-decisions.md`](context/discussion-decisions.md) | 权威当前 | 已确认结论和待决定事项摘要 |
| [`context/domain-language.md`](context/domain-language.md) | 权威当前 | DISC-001 已确认的产品、领域与采集责任共同语言；不预设数据库对象 |
| [`decisions/0001-greenfield-rust-clean-db.md`](decisions/0001-greenfield-rust-clean-db.md) | 权威当前 | 已接受的 Rust 与全新 PostgreSQL 决策 |
| [`product/PRD.md`](product/PRD.md) | 草案 | DISC-001 的产品输入，不是已接受实现合同 |
| [`product/domain-invariants.md`](product/domain-invariants.md) | 权威当前 | 跨 Gate 的领域不变量、判断资格与持续扩展的对抗性验收案例 |
| [`architecture/target-architecture.md`](architecture/target-architecture.md) | 草案 | DISC-001 已确认硬边界之上的总体架构建议；具体实现按 SCOPE 渐进冻结 |
| [`architecture/module-architecture.md`](architecture/module-architecture.md) | 草案 | Gate 6 Rust 模块、接口、依赖、adapter、测试表面与无巨型文件门禁；不授权创建 crate |
| [`architecture/agent-architecture.md`](architecture/agent-architecture.md) | 草案 | Gate 6 Agent Invocation、工具权限、预算/停止、恢复、结构化输出、隐私与评测；不选择模型或授权真实原文 |
| [`architecture/capture-plugin-architecture.md`](architecture/capture-plugin-architecture.md) | 草案 | Gate 6 服务端采集控制层、有限工位/账号、Work Order/Attempt/lease、部分结果、MV3 插件与协议升级 |
| [`architecture/runtime-operations-architecture.md`](architecture/runtime-operations-architecture.md) | 草案 | Gate 6 API/worker、Durable Work、scheduler、重试/接管、可观测性、数据库角色、部署与恢复 |
| [`architecture/data-architecture.md`](architecture/data-architecture.md) | 草案 | Gate 5 数据分类、身份、版本、Current、隐私、统计资格与 PostgreSQL 概念模型；不是 DDL |
| [`architecture/data-relations.md`](architecture/data-relations.md) | 草案 | `data-architecture.md` 的渐进披露子文档；收敛候选基数、外键责任、类型化关系与并发约束，不是最终表清单 |
| [`architecture/data-consistency.md`](architecture/data-consistency.md) | 草案 | 数据架构第三层；定义事务、重放、并发、隐私传播和 PostgreSQL 16 可证伪验收，不是 SQL 或 migration |
| [`architecture/current-v2-architecture.md`](architecture/current-v2-architecture.md) | 代码事实优先 | 现役 V2 固定点的参考架构 |
| [`architecture/rust-porting-map.md`](architecture/rust-porting-map.md) | 草案 | Rust 移植候选参考；只有正式 SCOPE 内明确列出的部分可以实施 |
| [`pages/page-map.md`](pages/page-map.md) | 草案 | 页面与情报工作流的初步映射 |
| [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md) | 草案 | Gate 7 工作台、Topic/Corpus/研究/行动/运行中心、API 与外部 Agent CLI 入口架构 |
| [`migration/action-plan.md`](migration/action-plan.md) | 活跃计划 | 分阶段重建路线与退出条件 |
| [`migration/transfer-checklist.md`](migration/transfer-checklist.md) | 活跃计划 | 跨机器搬迁和隔离检查 |
| [`reviews/bootstrap-review.md`](reviews/bootstrap-review.md) | 一次性报告 | Bootstrap 固定点审查，不代表当前实现状态 |
| [`audits/gate-3-domain-model-audit-2026-08-20.md`](audits/gate-3-domain-model-audit-2026-08-20.md) | 一次性报告 | 已获用户整体确认的 Gate 3 设计快照与全链压力测试；不替代权威共同语言，也不证明 Gate 4–7 已通过 |
| [`audits/gate-5-data-architecture-audit-2026-08-20.md`](audits/gate-5-data-architecture-audit-2026-08-20.md) | 一次性报告 | Gate 5 平台无关概念数据模型的第一轮对抗审查、已修漂移、条件通过项和待确认决定；不等于数据库实现获批 |
| [`audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md`](audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md) | 一次性报告 | Gate 4–7 在统一决定与 SCOPE-001 前的采集、数据、架构、页面/CLI 跨文档审计 |
| [`audits/gate-7-synthetic-task-walkthrough-2026-08-20.md`](audits/gate-7-synthetic-task-walkthrough-2026-08-20.md) | 一次性报告 | 用完全合成的“任务启动困难”负面场景走查今日关注、Topic、50/100 运行状态与外部 Agent CLI |
| [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md) | 一次性报告 | 不启动 SCOPE/代码的前提下预演首个 Content Evidence 切片的 fixture、数据库副作用、模块/文件预算和完成标准 |
| [`audits/scope-001-final-adversarial-audit-2026-08-20.md`](audits/scope-001-final-adversarial-audit-2026-08-20.md) | 一次性报告 | 正式 SCOPE-001 的独立代码前审查；P0/P1/P2、修订结果和最终实施确认边界 |
| [`runbooks/development-environment.md`](runbooks/development-environment.md) | 权威当前 | Rust 与 Docker PostgreSQL 16 的统一运行入口 |
| [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md) | 活跃计划 | 当前唯一事项：独立审查已吸收、实施已获用户批准；代码前基线推送后进入 TDD |
| [`plans/completed/disc-001-project-foundation-design.md`](plans/completed/disc-001-project-foundation-design.md) | 已完成计划 | 七道项目基础设计关口与 `USER-DEC-01`–`06` 的完成记录 |
| [`plans/completed/env-001-development-environment.md`](plans/completed/env-001-development-environment.md) | 已完成计划 | ENV-001 的范围、执行和验收记录 |
| [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md) | 已完成计划 | GOV-001 的范围、交付和验收记录 |
| [`progress/README.md`](progress/README.md) | 权威当前 | 变更记录规则和月份索引 |
| [`progress/2026-08.md`](progress/2026-08.md) | 权威当前 | 2026-08 的重要变更记录 |

## 仓库外层资料入口

| 文档 | 状态 | 用途 |
|---|---|---|
| [`database/README.md`](../database/README.md) | 权威当前 | 新数据库交付与安全边界 |
| [`database/legacy-archive-manifest.md`](../database/legacy-archive-manifest.md) | 历史归档 | 旧数据库和媒体的只读归档清单 |
| [`references/README.md`](../references/README.md) | 历史归档 | 历史证据区阅读边界 |
| [`references/current-v2/SOURCE-PROVENANCE.md`](../references/current-v2/SOURCE-PROVENANCE.md) | 历史归档 | 固定源码快照来源证明 |

## 按任务渐进读取

- 项目管理或新增文件：先读 `governance/`，再读当前事项文档。
- 环境配置：读 `current-state.md`、[`runbooks/development-environment.md`](runbooks/development-environment.md) 和 `database/README.md`。
- 产品讨论：读 `product/PRD.md`、`context/discussion-decisions.md` 和相关 ACCEPTED ADR。
- 数据与 PostgreSQL 设计：先读 `architecture/data-architecture.md`；只有需要关系/基数时再读 `architecture/data-relations.md`，需要事务/并发/验收时再读 `architecture/data-consistency.md`，并同时遵守 `database/README.md` 和领域不变量；未确认草案不得直接生成 DDL。
- 架构或实现：读目标架构、对应迁移地图、真实代码与测试；文档不得替代代码事实。
- 来源审计：先读 `references/README.md` 和 provenance，再只打开被当前审计明确引用的 fixture、源码或 handoff。

## 入库完成标准

新文件只有同时满足“位置正确、名称稳定、状态明确、进入本索引、冲突已处理、变更已登记、检查通过”才算正式入库。
