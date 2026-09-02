# TOPIC-WORKSPACE-REAL-001 · 实施卡

> 状态: 活跃计划
> 运行状态: 已部署；待 Mog 业务验收
> 最后核对: 2026-09-02
> 适用范围: Issue #112 首个真实但明确暂定的 Topic 工作区垂直切片
> 事实来源: 用户本轮授权、Issue #112、Topic 领域语言、Work Resource Read、当前分支代码与验证
> 冲突时以谁为准: 用户最新确认、AGENTS.md、正式领域不变量、当前代码与真实运行证据
> Issue: #112
> 分支: `codex/topic-workspace-real-001`
> 基线: `main@fa80d8afa4c4134b568190a4a2565eb83f993a16`（PR #115 已合并）

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
- A3/A4 Agent、正式知识写回、Claim、Action。

## TDD 记录

1. RED：integration test 先引用不存在的 Topic types/import/read seam，编译失败。
2. GREEN：实现 validation、append-only migration、事务导入与当前版本读取；2 个 unit test 通过。
3. PROOF RED：真实 API 组合首次返回 `409`，原因是 fixture 只有 content identity，没有可履行 Work Resource 的 observation/detail。
4. PROOF GREEN：fixture 增加 typed detail observation 后，4 个 intelligence PostgreSQL 用例 + 1 个 API 组合用例通过；proof container/volume cleanup verified。
5. 当前主线整合：旧 Draft 的 Topic migration 编号与 main 的 unified-media 0027 冲突；实现迁移为 additive 0031，full-schema fixture 首次仍引用旧路径而编译失败，已同步为 0031 后重跑定向、完整 workspace 与统一 isolated proof。
6. 最终严格审查：发现 Topic API 把 `rejected`、`conflict`、`not_found` 一律序列化为 `unavailable`。以一个统一的 typed outcome 映射替代隐式默认值，覆盖全部失败类别；不以 HTTP 码近似业务状态。
7. 浏览器验收根因与授权修复：`390×844` 的 Topic 主工作区已无横向溢出，但共享一级导航在 `≤640px` 仍把五项保持为每项至少 `86px` 的横向条带；第五项「采集」落在 `344–430px` 而被裁断。Mog 已明确授权在 #112 内修复该实际可达性问题：Shell 在窄屏必须让所有一级入口可见、可触达、可键盘到达；未来入口超过一行时自动换行，不能再依赖隐蔽横滑。实现只修改 shared `shell.css`、必要的 source regression test 与本卡记录；不改变路由、状态真值、数据、权限、插件、runtime 或其它页面的业务逻辑。
8. 浏览器复验：隔离 current integration 在 `1440×900` 核对 Topic 读取和挑战材料筛选；请求 `390×844` 与 `430×932` 时五个一级项均完整可见且 `scrollWidth=clientWidth`，额外 `320×844` 确认自动换行。390px 真实点击「采集」抵达 `/collection/attention`，原生链接与 visible focus 保留；console/error 与外部资源均为零。共享 DB、`:3000` runtime、插件和外部平台均未触碰。
9. 最终审查写入一致性收口：数据库将 `canonical_key` 与 `idempotency_key` 设为全局唯一，原锁却含 `domain_key` 且只覆盖 canonical，两个不同 domain 的同 canonical 或两个不同 Topic 的同幂等键都可能绕过同一把锁并把业务冲突降级成 raw unique error。锁现在按稳定顺序同时取两类数据库真实 identity；隔离 PostgreSQL 并发回归分别要求一笔成功、另一笔为「Topic domain cannot change」或 idempotency conflict，最终都只形成一条 Topic。
10. 一次最终严格审查：以 `origin/main...current integration HEAD` 审查整包的结构、文件长度、模块归属、typed outcome、事务原子性、前端状态与回归边界。审查内发现并根治了第 9 条的两种唯一身份竞争，随后移除 `too_many_arguments` 豁免，用私有 `WorkspaceVersionRefs` 让同一不可变版本的 IDs 一起流动。复验后结论为 PASS：没有遗留高置信结构/边界/并发发现；既有 API 16 项 dead-code warning 仍是 main 已有警告，未写作本卡解决。
11. 用户授权部署：PR #115 合入 `main@fa80d8a` 后，建立同 commit 的独立 runtime snapshot；在原运行仍提供服务时锁定构建 API、巡检 worker 与媒体 worker。共享本地数据库的 migration ledger 已在事务中记录 `0031_topic_workspace` 的 checksum `cd7e6a33c5c9203d2fcddc88d246aaa16b2a9de4bebfda21123978315ee6297b`，三项 launchd 服务重启后 cwd 与 executable 均核验来自该 snapshot。`GET /topics` 返回 200；默认 API 对尚未导入的 `task-initiation-difficulty` 如实返回 `404 / outcome=not_found`，未写入 synthetic 或真实业务 Topic。

## 停止条件

- 需要正式 Definition Release、Claim、趋势或市场事实时停止。
- 需要复制 Work Resource 字段、读取 raw Package 或展示敏感评论时停止。
- 需要修改插件、采集、scheduler，或在本次已授权的 `0031` / loopback runtime 切换之外再改变共享持久库或部署时停止。
- 与其它卡的共享 Shell/current-state 产生无法重放的冲突时停止并交给 integrator。

## 验收层

| 层 | 当前状态 | 退出条件 |
|---|---|---|
| 合同/实现 | VERIFIED（current integration head） | types、0031 migration、API、page 与 shared Work Resource composition 完整 |
| 自动检查 | VERIFIED（current integration head） | format、shared Shell regression、Topic PostgreSQL、workspace、JS syntax、治理检查与统一 isolated proof 已通过 |
| 浏览器 | VERIFIED（isolated current integration） | 1440×900、材料筛选、390×844、430×932、320px 自动换行、入口点击/focus、console 与外部资源边界均已复验；外部 Chrome/完整辅助技术仍未证明 |
| 部署 | VERIFIED（本机 loopback） | `0031` ledger checksum、三项 launchd runtime cwd/executable、`:3000 /health` 与 `/topics` 已核验；不代表真实业务材料、外部平台或插件发布 |
| 业务验收 | NOT VERIFIED | Mog 明确验收 |
