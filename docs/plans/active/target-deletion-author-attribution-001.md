# TARGET-DELETION-AUTHOR-001 · 观察决定的安全撤回与作品作者归属

> 状态: 活跃计划
> 最后核对: 2026-09-09
> 适用范围: Issue #214 的观察目标撤回、作者归属 projection、Targets 行级操作与本机发布
> 事实来源: `collection_*` 外键、`linggan_material_*` append-only 投影、当前代码、隔离 PostgreSQL proof 与用户本轮明确授权
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实数据库副作用与 migration；Issue 仅记录交付过程

## 用户结果

人可以安全地停止或恢复一个目标的后续巡查；若要彻底撤回“我观察这个对象”的决定，先看到会删除的控制面与会保留的材料，再输入确认名执行。

删除只撤回控制面：申请、准入、工单、租约、任务、规则和调度/命令记录。它绝不把已接纳的作品、详情、评论、采集包或回执一起删掉。作品作者归属从采集事实推导，不再由观察目标存在与否决定。

## 范围与非目标

本包新增 `linggan_material_content_author` 只读 projection；用它显示删除前将保留的作者作品/详情数量，并覆盖详情作者与 `profile_discovery` 的有界回退。删除在跨行业样本或人工作品失效结论仍挂在目标上时必须整体拒绝。

本包不回填、不清理共享历史目标、不访问平台、不重载插件、不改变评论研究资格，也不把“作者回复”徽标识别当作已完成。评论研究 V1 的输入清洗、角色事实与语义路径属于独立交付包。

## 关键不变量

1. `Observation Target` 是观察控制面，不是作品作者关系的权威。
2. 所有删除 SQL 在一个事务中按外键从叶到根执行；任一失败即回滚。
3. 跨行业样本与人确认的失效结论都是已保留事实：不能为删除目标而删除它们。
4. `NULL` 或空显示名使用稳定 `identity_key` 作为确认名；空白输入永远不能通过确认。
5. 页面不能把“停止观察”“已删除”“删除受阻”混成同一种成功或失败反馈。

## 实施与验收

| 层级 | 验收 |
|---|---|
| 控制面删除 | 隔离 PostgreSQL 构造 command identity、receipt、scheduler decision、WorkOrder 与 Lease，证明删除不再被外键阻断且相关行全部消失。 |
| 保留事实 | 隔离 PostgreSQL 证明跨行业样本阻止删除；作品作者 attribution 在目标删除前后保持相同；详情优先、主页发现仅作 profile-discovery 回退。 |
| 状态与界面 | Rust/API renderer 测试覆盖确认名回退、删除回执、缺口投影与行级控制；页面只按真实 preview 渲染可删除/受阻。 |
| 发布 | 代码审查、`cargo fmt`、项目治理检查、隔离 PostgreSQL harness、PR 精确 head、main 复核、migration checksum 与本机 runtime health。 |

## 发布与回退

`0063_content_author_attribution.sql` 是 additive view。发布前先对本机持久库做可恢复备份，再通过 `local-runtime.sh migrate` 记录 checksum；不对共享库执行目标删除来测试。若需回退运行代码，保留 migration 和材料事实，禁用/回退 UI 入口即可；不得删除已应用的 migration 或它所指向的事实。
