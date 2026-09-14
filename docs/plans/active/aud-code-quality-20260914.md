# AUD-CODE-QUALITY-20260914 · 完成性、事实一致性与质量门修复

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #279 的 embedding/run-item 所有权恢复、关键词历史详情续办、材料 retirement 读取、Targets 问题入口、无效新建字段与检查器信号
> 事实来源: 用户确认的审查分类、Issue #279 Claim、当前 Rust/SQL/migration/测试与 PAGE-COLLECTION-001
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库合同与可复现 PostgreSQL 证明；本计划不授权生产迁移、部署、插件或真实采集

## 1. 目标与封闭范围

本包修复的不是“更多状态”，而是让已存在事实仍有唯一、可恢复的后续路径：

```text
Embedding: pending/retryable -> running(lease, attempt) -> succeeded | retryable | failed
Run item: claim(attempt) -> 只允许同一 attempt 写 invocation / atom / terminal state
Keyword: discovered material + detail incomplete -> 可建立 detail work，不依赖 target 是否仍在 baseline
Material: target-scoped retirement -> 目标档案读取不再把它作为待补齐材料
Target UI: execution 与 archive problems 并列，主动作一致地指向问题
```

包含：A1、A2（删除无效排序输入）、A3、A4、B1、B3、C1。

明确不做：B2、B4–B7、C2、C3；不修改真实数据，不运行 shared migration，不刷新 runtime，不重载插件，不访问外部平台，不合并。

## 2. UI Change Manifest（A2 + A4）

### 读取回执

| 来源 | 已核对 | 用途 |
|---|---:|---|
| AGENTS.md、docs/README.md、current-state.md | 是 | Issue/Claim/worktree、文档与交付门 |
| UI execution contract | 是 | 状态/交互变更需表面、状态、依赖、验收矩阵 |
| PAGE-COLLECTION-001 | 是 | 主动作仍是“处理异常/补采缺口”，不得把 running 压成唯一状态 |
| target-inspector manifest | 是 | Targets drawer 为 L1 Collection Control 的受限 L2 Split Inspector |
| LIDS README / data truth / language | 是 | 不增 Token/CMP/视觉层；已知事实不可被互斥状态吞没 |
| 当前 target drawer、inspector、intake 与 tests | 是 | 确认新建关键词排序字段不进入 TargetIdentity，且两处 archive action precedence 不一致 |

### 分类、表面与状态

- 分类：状态语义 + 交互（最高风险：状态语义）。
- Pattern：L1 Collection Control；Targets Drawer 为既有 L2 Split Inspector；不新增视觉、Token、Primitive、CMP、Scene 或 Motion。
- 表面：`/collection/targets` 新建目标表单、目标行主动作、Inspector 主动作。
- 状态词典：`running` 表示当前执行；`quarantined`/`blocked_details` 表示仍须处理的档案事实；两者可同时存在。`NoAction` 只在没有可处理 archive problem 时成立。
- 依赖：Target inspector 的 coverage projection 是两处 UI 共享事实；keyword ranking 是 monitoring rule 的口径，非 target 创建身份字段。
- 停止条件：若需新增用户可见状态、权限或 action，或 retirement 的全局读取语义与当前数据合同冲突，则停止相关项并记录 `DECISION_REQUIRED`。

### 验收矩阵

| 层级 | 自动证明 | 不证明 |
|---|---|---|
| 任务可用 | Rust unit / isolated PostgreSQL：运行中且有 archive problem 时两个入口给出同一问题动作；新建表单不再提交无效 ranking | 真人页面验收 |
| 状态诚实 | 覆盖读数仍独立于 execution；Known zero 与 Unknown 不变 | 实时 worker 状态 |
| 视觉一致 | HTML/JS seam 断言不新增 Token/CMP、删除排序控件 | 1440px 浏览器视觉走查 |
| 真实后果 | branch SQL / tests only | production write、采集、部署与业务验收 |

## 3. 实施与证明

1. 添加 additive migration：embedding lease、attempt、retry schedule；把 claim、success/failure 与 expiry recovery 全部以 attempt 条件围住。
2. 为 comment-research RunItem 的 invocation、input、Atom 接纳和终态写入补齐 attempt fencing；陈旧 owner 只能收到 claim-lost。
3. 把 keyword detail 待办从 baseline lifecycle 解耦，保留 baseline coverage 判据本身。
4. retirement 保持 target-scoped：在目标档案读取根上应用约束，不在无 target 的 Work Resource/Cursor 读取中扩大成人工全局删除；检查器核对双键读取。
5. 用覆盖事实决定 Targets 的 archive-problem 主动作；删除无效 Target intake ranking UI/字段。
6. 修正 INV-1 注释计数和 INV-2 跨表误报；对 rust-boundaries 保留实际 baseline，不宣称全绿。

交付前：migration fixture、定向 Rust/isolated PostgreSQL、`check-invariants.sh`、`check-project-governance.sh`、diff review。需要提交前按 AGENTS 指定独立 review；随后建立 PR，不合并。

## 4. Rust boundary 基线（不伪装为通过）

2026-09-14 在本事项分支实际运行 `./scripts/check-rust-boundaries.sh`：**53 errors / 25 warnings**。这些是仓库既有的大文件、`apps` 业务 SQL 与公开项规模债；本事项没有关闭、降级或“忽略”该门，也不把 `check-invariants.sh` 的通过写成全仓边界健康。该输出是后续独立 `RUST-BOUNDARIES` 工作包的明确基线；本包新增的 execution-recovery 场景留在既有 comment-research PostgreSQL fixture，不新建第二条执行路径。
