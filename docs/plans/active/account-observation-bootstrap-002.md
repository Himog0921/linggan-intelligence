# ACCOUNT-OBSERVATION-BOOTSTRAP-002 · 首单软准入与账号观察启动

> 状态: 活跃计划
> 最后核对: 2026-09-10
> 适用范围: Issue #218；Collection Runtime、统一 capacity evaluator、dispatch、Browser Producer 的账号观察回执与配置诊断
> 事实来源: Mog 当前对话授权、`origin/main@9c84f3506a1ff3ee7a3357daba83130dd5ee438b`、共享 PostgreSQL 只读诊断、现行代码与自动化测试
> 冲突时以谁为准: Mog 当前确认、AGENTS.md、真实运行/数据库/代码、现行合同

> 实施状态: 源码、迁移、Browser Producer 合同与隔离验证已完成；待独立审核与 Mog 后续决定是否交付。共享 migration、3000 Runtime、Chrome、真实平台与业务验收均未触及。

## 用户结果

新安装的 Browser Producer 已完成服务端 check-in、但还没有账号观察或人工绑定时，不能再被 `account_unbound` 或“没有近期正向检查”拒绝首单。它可在工位、凭据、版本、能力、风险、并发和预算均合格时领取一项任务；任务本来要打开的 XHS 页面顺便形成账号观察。

账号观察不是周期巡检：不枚举标签、不打开/刷新/滚动页面、不调平台接口。只有服务端确认的 `cooling`、`needs_login`、`restricted`，或者已有人工绑定与新认证账号不一致，才停止本次任务并阻断后续领取。

## 已确认事实

- 现场两条 0.8.46 installation 已有新鲜 check-in 和有效服务端凭据，却各有 0 条账号观察、0 条活跃绑定；旧 installation 已取代，不能继承新 installation 身份。
- 运行时 `/health.routes.station.eligibilityReport` 在 schema 就绪后仍通告受控回执路由；本机缺少有效 `LINGGAN_ACCOUNT_DIGEST_KEY` 时，认证身份 observation 会返回 `identity_key_missing`，但明确登录墙、冷却或访问限制仍可被服务端接收并阻断，不制造额外平台访问量。
- 当前 `evaluate_capacity_in`、claimant revalidation 和 batch slot 仍把 `bound_account_ref.is_none()` 映射为 `account_unbound`；这与用户已确认的首单软准入语义冲突。
- 历史账号 identity digest 不能在不知道旧 HMAC 密钥时重算。该密钥恢复或受控重建属于独立运维/数据决定，不在本计划自动执行。

## 冻结语义

| 事实 | 新 Lease | 已领取任务 |
|---|---|---|
| 没有账号观察 / 没有人工绑定 | 允许首单 | 允许页面自然观察 |
| 账号观察传输失败或 DOM 无法判定 | 允许 | 不伪造负面事实，允许继续 |
| 明确冷却、登录墙或访问限制 | 阻断 | 停止并以明确 failure 回队 |
| 已绑定账号与新认证账号不同 | 阻断 | 停止并等待人工确认替换绑定 |
| 未绑定账号与新认证账号一致 | 允许 | 允许；人工确认只改变持续绑定，不是执行前置条件 |

没有已知账号的 installation 同时至多保有一个 live Lease；这是首单保护，不是额外账号租约或后台探测。

## 实施边界

1. 以 migration 将无身份但明确的负面页面观察保留为 installation 级 append-only 事实，使首单登录墙/受限状态可以阻断后续工作而不猜测账号身份。
2. 以一个 capacity candidate 投影统一普通派发、具体 claimant、frozen revalidation 和 batch-ready slots：绑定缺失不阻断；绑定存在时才检查身份变化；明确负面始终阻断；installation live Lease 保证首单串行。
3. 工作单与 Lease 可如实冻结 `account_ref = NULL` / `eligibility_ref = NULL`，表示本单在无账号事实时合法领取，不用伪造绑定或账号。
4. Producer/API receipt 区分 `bindingRequired` 与 `bindingMismatch`；前者是人工确认候选，后者才是任务页停止条件。
5. `/health` 和 Runtime 明示“账号身份摘要未配置”，不暴露密钥或摘要；插件回执保留该诊断。
6. `0065_account_observation_bootstrap` 以 append sequence 确定“最新 observation”，避免稳定服务端时间戳并列时由随机 UUID 决定账号状态。

## 非目标与停止条件

- 不生成、替换、输出或写入 `LINGGAN_ACCOUNT_DIGEST_KEY`，不重算历史 digest，不修改共享数据库记录。
- 不重载 Chrome、访问 XHS、领取真实任务、发布 ZIP、push、merge、切换 3000 或部署。
- 不改变账号绑定的人工审计责任；只移除“尚未绑定”作为首单前置阻断。
- 若需要把历史 identity 映射到新的密钥版本、自动迁移绑定、持久化新密钥，停止并由 Mog 决定恢复或 rekey 策略。

## 验收矩阵

| 层 | 必须证明 | 不作为本次证明 |
|---|---|---|
| Rust/domain | 未绑定、无观察可通过 capacity；明确负面/已有绑定变化仍关闭 | 真实账号状态 |
| PostgreSQL | anonymous negative observation 追加、无账号 lease 冻结、单 installation 首单串行、历史绑定不改写 | 共享数据库迁移 |
| API/UI | health 诚实公布 availability；Runtime 区分未观察与身份摘要缺失 | 真实 3000/浏览器视觉验收 |
| Browser Producer | route 缺失有明确无秘密 reason；unbound positive 不停止任务，mismatch/negative 会停止 | Chrome/XHS 运行 |
| 交付 | fmt、定向单元/插件、隔离 PostgreSQL、治理检查和独立代码审查 | push、PR merge、部署、业务验收 |

## 已完成的源码验证

- `cargo fmt --all`、Evidence 45 个单元测试、API 167 个普通单元测试与 Browser Producer 262 个测试通过。
- 完整 `scripts/test-local-001-discovery-postgres.sh` 通过，覆盖 Collection Control 17、Runtime 13、Dispatch sequence 17 和 API PostgreSQL 19 项；一次性 PostgreSQL 容器、数据库及卷均已清理。
- 直接回归覆盖：新工位无账号事实仍可得 capacity、匿名 `login_required` 可写为 `account_ref = NULL` 并阻断、无人工绑定的正向身份可冻结/revalidate、同安装 live Lease 返回 `station_busy`、相同时间粒度内后写 observation 以单调序列获胜。
