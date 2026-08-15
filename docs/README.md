# 文档总索引

> 状态: 权威当前
> 最后核对: 2026-08-15
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
| [`decisions/0001-greenfield-rust-clean-db.md`](decisions/0001-greenfield-rust-clean-db.md) | 权威当前 | 已接受的 Rust 与全新 PostgreSQL 决策 |
| [`product/PRD.md`](product/PRD.md) | 草案 | 产品方向和首个垂直切片建议 |
| [`architecture/target-architecture.md`](architecture/target-architecture.md) | 权威当前 | 已确认的目标架构边界 |
| [`architecture/current-v2-architecture.md`](architecture/current-v2-architecture.md) | 代码事实优先 | 现役 V2 固定点的参考架构 |
| [`architecture/rust-porting-map.md`](architecture/rust-porting-map.md) | 活跃计划 | TypeScript 参考能力到 Rust 模块的迁移路线 |
| [`pages/page-map.md`](pages/page-map.md) | 草案 | 页面与情报工作流的初步映射 |
| [`migration/action-plan.md`](migration/action-plan.md) | 活跃计划 | 分阶段重建路线与退出条件 |
| [`migration/transfer-checklist.md`](migration/transfer-checklist.md) | 活跃计划 | 跨机器搬迁和隔离检查 |
| [`reviews/bootstrap-review.md`](reviews/bootstrap-review.md) | 一次性报告 | Bootstrap 固定点审查，不代表当前实现状态 |
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
- 环境配置：读 `current-state.md`、`context/START-HERE.md`、`database/README.md`；待 ENV-001 启动后再读对应 runbook。
- 产品讨论：读 `product/PRD.md`、`context/discussion-decisions.md` 和相关 ACCEPTED ADR。
- 架构或实现：读目标架构、对应迁移地图、真实代码与测试；文档不得替代代码事实。
- 来源审计：先读 `references/README.md` 和 provenance，再只打开被当前审计明确引用的 fixture、源码或 handoff。

## 入库完成标准

新文件只有同时满足“位置正确、名称稳定、状态明确、进入本索引、冲突已处理、变更已登记、检查通过”才算正式入库。
