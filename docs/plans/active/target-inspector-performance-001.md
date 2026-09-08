# TARGET-INSPECTOR-PERFORMANCE-001 · 观察目标检查器实施合同

> 状态: 活跃计划
> 最后核对: 2026-09-08
> 适用范围: Issue #158 的目标列表工具栏、creator 检查器与作品双视图 follow-up
> 事实来源: Mog 2026-09-08 确认、Issue #158 Claim、当前代码/隔离读模型、PAGE-COLLECTION-001 与 LIDS v7
> 基线: origin/main@e5ad0e3316760ae43bc0ab76e00877c7e0fd9fb8
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/回执、Issue #158 Claim；本计划不授权共享运行或外部平台动作

## 用户结果

观察目标页先帮助人找到对象和发起明确动作，而不是重复展示运行时抬头。六个目标均可勾选，选中后顶部批量编辑显示数量；操作列在桌面表格横向浏览时仍可见。creator 右侧检查器只回答四件事：对象是谁、系统正在做什么、已经取得多少、现在是否需要人工处理。作品页保留可查证的列表，并新增基于同一目标、同一 as-of 事实的「表现」视图。

## 事实与状态合同

- `TargetInspectorProjection` 在单个只读、repeatable-read 事务中固定 `as_of`，分开输出 archive、execution、patrol、coverage、required action。
- queued 不等于 running；只有有效 Lease 下已产生 Attempt 且未形成 Package 的任务才可写 running。
- 所有计数保留 `Known(0)` 与 `Unknown`。未知不得写成零，也不得进入散点或分布。
- creator lifecycle 继续消费 stable target-scoped Work、合格发布时间与当前 KNOWN 指标。`作品｜表现` 是该投影的日常入口；JSON API 不是 UI 的第二事实源。
- 当前没有可用于这些 creator works 的内容分类合同，因此不生成主题分布、内容结构或「监控价值」结论。评论只呈现已有覆盖，不从平台评论数推断已采评论数。

## 页面与交互

- 列表顶端删除「点击一行查看详情」说明；保留对象名链接与现有非交互单元格快捷打开。
- Targets breadcrumb band 不再重复巡检数、建档数、scheduler、patrol 和 timezone；筛选 tab 与表格是本页事实入口。
- 筛选为文字 tab + 3px signal underline；输入、select、批量与新建控件不小于 40px。只有当前选择、可用批量动作与唯一主动作使用新粗野主义强调。
- Drawer 一级保持 `概览｜作品｜巡查`；creator 的作品内为 `列表｜表现`。状态写入 URL：`dtab=works&wview=list|performance`，表现窗口/指标继续兼容既有 `life_*` 查询。
- 概览不重复作品表或生命周期图。系统可自动解决、已排队或正在执行时只说明，无人工按钮；只有明确需要人介入时给一个主动作。

## 分工与文件边界

- root 是本包 coordinator/integration owner；subagent 仅在同一 branch/worktree 的非重叠文件上执行。
- Evidence subagent：新增 target inspector read model、evidence exports 和专属测试。
- Drawer subagent：`target_drawer.rs/css`。
- QA subagent：专属 render/URL/静态合同测试，不修改生产文件。
- root：Collection toolbar/list/JS/CSS、route wiring、共享测试和文档。
- migration/schema、shared PostgreSQL、runtime-main、plugin/release、外部平台、部署、merge 与无关 comment-research 均禁止。

## 验收矩阵

1. 六目标选择：每个目标独立 checkbox；顶部按钮按选中数启用并显示数量；分组 select-all 不越组；勾选不打开 Drawer。
2. 状态：queued/running、Known zero/Unknown、blocked/automatic recovery 逐项有反例测试。
3. URL：list/performance、window/metric/selected work 刷新可恢复；旧 `life_*` 归一到 performance。
4. 视觉：1440 CSS px 下无页面级横溢出，操作列可见，控件 ≥40px，focus/reduced motion/图表可访问替代成立。
5. 事实：用 disposable PostgreSQL 跑三种不同稀疏度的 creator 数据；不把排除项、未知或来源不足补成数字。
6. 工程：focused tests、`cargo fmt --check`、适用 `cargo check/test`、项目治理检查、exact diff review。

## 完成边界

当前为源码实施。PR、review、merge、shared runtime replacement、deploy 与 Mog 业务验收分别为独立层；未经后续授权均为 NOT VERIFIED。
