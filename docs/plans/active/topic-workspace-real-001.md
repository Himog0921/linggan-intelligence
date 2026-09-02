# TOPIC-WORKSPACE-REAL-001 · 实施卡

> 状态: 活跃计划
> 最后核对: 2026-09-02
> 适用范围: Issue #112 首个真实但明确暂定的 Topic 工作区垂直切片
> 事实来源: 用户本轮授权、Issue #112、Topic 领域语言、Work Resource Read、当前分支代码与验证
> 冲突时以谁为准: 用户最新确认、AGENTS.md、正式领域不变量、当前代码与真实运行证据
> Issue: #112
> 分支: `codex/topic-workspace-real-001`
> 基线: `44c9ab68dee43feaff1e69b15b046ccff0a1810e`

## 目标

交付 `/topics/{canonical_key}` 的首个真实垂直切片：暂定 Topic Definition、人工裁定 Classification Run、冻结 Material Pack、共享 Work Resource 组合和明确来源限制。

## 范围冻结

- additive `0031_topic_workspace.sql`；不改历史 migration，不回填共享库。当前 main 的 `0027` 已属于 unified media，因此旧 Draft 编号不得复用。
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
5. 当前主线整合：旧 Draft 的 Topic migration 编号与 main 的 unified-media 0027 冲突；实现迁移为 additive 0031，full-schema fixture 首次仍引用旧路径而编译失败，已同步为 0031 后重跑定向、完整 workspace 与统一 isolated proof。
6. 最终严格审查：发现 Topic API 把 `rejected`、`conflict`、`not_found` 一律序列化为 `unavailable`。以一个统一的 typed outcome 映射替代隐式默认值，覆盖全部失败类别；不以 HTTP 码近似业务状态。

## 停止条件

- 需要正式 Definition Release、Claim、趋势或市场事实时停止。
- 需要复制 Work Resource 字段、读取 raw Package 或展示敏感评论时停止。
- 需要修改插件、采集、scheduler、共享持久库或部署时停止。
- 与其它卡的共享 Shell/current-state 产生无法重放的冲突时停止并交给 integrator。

## 验收层

| 层 | 当前状态 | 退出条件 |
|---|---|---|
| 合同/实现 | VERIFIED（current integration head） | types、0031 migration、API、page 与 shared Work Resource composition 完整 |
| 自动检查 | VERIFIED（current integration head） | format、targeted API/domain、Topic PostgreSQL、workspace 与统一 isolated proof 已通过；治理检查待终包收口 |
| 浏览器 | NOT VERIFIED（current integration head） | 旧 Draft 浏览器记录保留为历史；现行 runtime 尚无 Topic schema/route，最终只做一次有界前端验收 |
| 部署 | NOT STARTED | 不在本卡自动执行；shared migration/runtime 需另行授权 |
| 业务验收 | NOT VERIFIED | Mog 明确验收 |
