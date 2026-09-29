# COLLECTION-RUNTIME-STATE-20260929 · 采集运行状态纠偏

> 状态: 权威当前
> 最后核对: 2026-09-29
> 适用范围: COLLECTION-RUNTIME-STATE-20260929 的执行工位状态纠偏候选
> 事实来源: Mog 当日要求恢复采集并修复 bug；本机运行、数据库、插件 0.8.57 与 `origin/main@94e32cb4` 实测
> 冲突时以谁为准: 用户最新决定、真实调度与派发合同、当前代码及 LIDS 数据/语言规则

> 实施状态: 候选分支，待 PR 集成与页面验收。

## 用户结果与范围

`/collection/runtime` 必须把全局 recovery 阶段写成「采集暂停」，三条通道写「暂不派发」，原因用一句中文说明。工位在线与容量读数仍可展示，但不能替代是否准许新任务派发；即使控制投影查询失败，已知的全局 recovery 闸门仍要显示。过期仍标记 `queued` 的历史工单不计入当前排队数；`expires_at IS NULL` 的无期限工单仍计入。正常 governance 阶段继续按既有能力、工位、账号与风险判定显示。

本变更属于状态/语义纠偏，不增页面、动作、权限、采集范围或新视觉组件。与 RUNTIME-STATION-V7-2-001 历史表达记录的冲突仅在全局阶段判定：旧记录当时沿用通道容量；本清单按真实派发闸门修正这一处，不重定义其它状态。

## 表面地图与状态词典

| 表面 | recovery | governance | 未读到 |
|---|---|---|---|
| 执行工位首页判断与三条通道 | 采集暂停；暂不派发；恢复阶段 | 沿用当前通道容量判断 | recovery 仍显示暂停；governance 可退回容量概览 |
| 最近派发回答 | 采集暂停及一句原因 | 沿用当前回答 | 保留未知码的审计解释 |
| 当前排队读数 | 只计未过期 queued | 同左 | 显示「—」 |

`recovery` 是全局阻断；工位容量可用不等于新任务可派发。`queued` 的持久历史与「当前排队」读数分开，过期历史不删除。`ACCEPTED` 回执仅证明包准入，材料处置另行读取。

## 依赖与验收

- 真源：`collection_governance_enabled`、派发闸门 `collection_upgrade_recovery_only`、`read_capacity` 与 `collection_work_order.expires_at`。页面仅投影上述事实。
- 自动验证：恢复阶段即使工位有容量也阻断三通道；控制投影缺失但容量可读时不得误报可接活；governance 时可用工位仍可接活；隔离 PostgreSQL 中过期 queued 历史保留但当前排队数归零，无期限 queued 仍计入。
- 运行验证：本机 main 在 2026-09-29 16:35 左右已有 WorkOrder→Lease→Attempt→Package→Receipt；代码候选需在集成发布后再验 `/collection/runtime` 的 recovery 与 governance 页面行为。当前治理阶段的实跑不证明未来切回 recovery 的页面已发布。
- 人工验收：Mog 核对页面词义和实际采集结果。PR、自动测试和 HTTP 200 均不代替此项。
