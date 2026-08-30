# TOPIC-WORKSPACE-REAL-001 · 实施卡

> 状态: 活跃计划
> 最后核对: 2026-08-31
> 适用范围: Issue #112 首个真实但明确暂定的 Topic 工作区垂直切片
> 事实来源: 用户本轮授权、Issue #112、Topic 领域语言、Work Resource Read、当前分支代码与验证
> 冲突时以谁为准: 用户最新确认、AGENTS.md、正式领域不变量、当前代码与真实运行证据
> Issue: #112
> 分支: `codex/topic-workspace-real-001`
> 基线: `3366a091cd466a6fa12f9d36ae12c31f0ccb87dc`

## 目标

交付 `/topics/{canonical_key}` 的首个真实垂直切片：暂定 Topic Definition、人工裁定 Classification Run、冻结 Material Pack、共享 Work Resource 组合和明确来源限制。

## 范围冻结

- additive `0027_topic_workspace.sql`；不改历史 migration，不回填共享库。
- `crates/intelligence` 的 Topic Workspace Interface；不实现 Claim/Trend/正式 Release。
- loopback POST/GET 与 L2 Topic 页面；页面不触发采集，不提供正式发布按钮。
- 以「任务启动困难」作为路由默认入口，但仓库不写入真实业务材料。
- 独立 PostgreSQL proof、Rust/API/UI source tests、LIDS 与浏览器检查。

## 非目标

- 插件、scheduler、采集/监控、历史材料补录、外部平台访问。
- 原始正文/评论复制、embedding、聚类、自动分类、统计趋势。
- A3/A4 Agent、正式知识写回、Claim、Action、部署、合并。

## TDD 记录

1. RED：integration test 先引用不存在的 Topic types/import/read seam，编译失败。
2. GREEN：实现 validation、append-only migration、事务导入与当前版本读取；2 个 unit test 通过。
3. PROOF RED：真实 API 组合首次返回 `409`，原因是 fixture 只有 content identity，没有可履行 Work Resource 的 observation/detail。
4. PROOF GREEN：fixture 增加 typed detail observation 后，4 个 intelligence PostgreSQL 用例 + 1 个 API 组合用例通过；proof container/volume cleanup verified。

## 停止条件

- 需要正式 Definition Release、Claim、趋势或市场事实时停止。
- 需要复制 Work Resource 字段、读取 raw Package 或展示敏感评论时停止。
- 需要修改插件、采集、scheduler、共享持久库或部署时停止。
- 与其它卡的共享 Shell/current-state 产生无法重放的冲突时停止并交给 integrator。

## 验收层

| 层 | 当前状态 | 退出条件 |
|---|---|---|
| 合同/实现 | VERIFIED（branch） | types、migration、API、page 完整 |
| 自动检查 | PARTIAL | workspace fmt/test、proof 已绿；focused clippy/治理检查待收口 |
| 浏览器 | VERIFIED | 1440×900、390×844 真实渲染无横向溢出；筛选/Inspector 联动通过 |
| 部署 | NOT STARTED | 不在本卡自动执行 |
| 业务验收 | NOT VERIFIED | Mog 明确验收 |
