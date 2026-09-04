# ACC-COLLECTION-FIVE-PAGE-V4-UI-001 · 五页面 V4 桌面验收

> 状态: 一次性报告
> 最后核对: 2026-09-04
> 适用范围: `codex/collection-five-page-v4-ui` 相对 `origin/main@84fd9498e18164011621ffabc81a2a421a52f7a1` 的 Collection 五页面变更
> 事实来源: 当前分支代码与测试、一次性 PostgreSQL 16、隔离 `:3311`、1440×900 in-app browser
> 冲突时以谁为准: 用户最新确认、真实代码/数据库/运行结果、PAGE-COLLECTION-001、Package 1/2 合同与 LIDS

## 1. 验收边界

本报告验证五个真实 Collection route 对 V4 桌面信息架构的复刻，不验证参考 HTML 的模拟业务数字。参考文件 `/Users/moglenny/Downloads/linggan-collection-field-workspace-v4-ia-cn.html` 是 synthetic UI 对标，只提供结构、密度、几何和交互关系；全局壳层、事实、权限、状态与行动后果仍以 Intelligence 当前实现为准。

支持范围固定为 1440 CSS px 桌面全屏。1280、390、手机、小于 13 寸屏幕和 responsive parity 均不在本次验收范围。

## 2. 测试环境

| 项目 | 本次事实 |
|---|---|
| Git base | `origin/main@84fd9498e18164011621ffabc81a2a421a52f7a1` |
| 分支 | `codex/collection-five-page-v4-ui`；实现已完成，Mog 已授权 commit/push/merge 与本机 `:3000` 刷新，执行证据待补 |
| PostgreSQL | 一次性 `postgres:16-alpine`；按当前顺序应用 33 个 migration |
| Fixture | synthetic：2 targets、2 scheduler decisions、3 tasks；不是共享库或真实平台数据 |
| HTTP | 隔离 loopback `127.0.0.1:3311`；共享 `:3000` 未改动 |
| 浏览器 | Codex in-app browser，显式 1440×900 viewport |
| 外部系统 | 未操作共享数据库、launchd、Browser Producer、Chrome extension 或真实平台 |

## 3. 页面结果

| 页面 | 可见结构 | 数据与状态诚实性 | 浏览器结果 |
|---|---|---|---|
| 待处理 | 2 个 Context KPI、恢复 ledger、右侧 Inspector | 只列可恢复 durable reason；owner/action 与选中行一致 | 行选择实测更新 Inspector；无横向溢出 |
| 观察目标 | 2 个 Context KPI、筛选/新增、密集目录、宽幅抽屉 | UNKNOWN/0 分开；creator/keyword lifecycle 分开；不呈现语料或价值模块 | 列表和 lifecycle drawer 可见；无横向溢出 |
| 生产流 | 2 个 Context KPI、stage ledger、dark decision instrument | stage 不是漏斗；恢复数不包含普通等待态；trace/review 不被 now 覆盖 | 真实 projection 成功替换 now body slot；无横向溢出 |
| 采集任务 | 2 个 Context KPI、Task ledger、右侧多 tab Inspector | Task/Attempt/Package/Receipt/frozen Work 分责；Frozen Work 按当前 `task_id` 隔离 | 行选中、tab click/Arrow/Home/End 与 ARIA 合同成立 |
| 执行工位 | 2 个 Context KPI、capacity 判定、资源关系、登记/认领区 | 使用服务端同一 capacity evaluator；不把存在插件版本冒充在线 | `#runtime-register` 与三条 lane 判定可见；无横向溢出 |

首轮五页逐页测得 `document.documentElement.scrollWidth = clientWidth = 1440`，浏览器 console warning/error 为 0。exact-head 独立复审随后拒绝了首版的可见标题/五格读数；当前修正合同要求每页恰有 2 个 `.v7-kpi`、无 `.c-readout-strip`、`h1.v7-sr-only` 唯一。修正版的最终 1440 证据在 exact head 稳定后补记。

## 4. 关键取舍

- 不复制参考稿的第二套全局 header/context/rail；共享 shell 是 Intelligence 的唯一壳层 owner。
- 不复制参考稿的可见大标题与第二条五格读数；两者与 LIDS 页头收回规则冲突，Intelligence 标准优先。
- 不引入参考稿模拟数字、Evidence tab/卡片、代表证据、监控价值、机会评分或假健康分。
- 目标抽屉默认仍是 creator lifecycle。当前 synthetic fixture 没有合格作品，因此生命周期诚实显示“观察不足，暂时无法成图”；这证明空态，不证明有数据曲线的真实密度。
- Production flow 的 dark instrument 只显示持久 scheduler decision，不使用 timer 伪造实时事件。
- Runtime、规则 modal、URL 状态、Esc/focus return 和现有 LIDS token 继续复用，不创建第二套交互真相。

## 5. 自动验证

- `cargo fmt --all -- --check`：通过。
- `cargo check --workspace --all-targets --locked`：通过；仅保留既有 dead-code warning。
- `cargo test --workspace --all-targets --locked`：修正版通过；API `106 passed / 18 ignored`，其余 workspace 目标全绿。
- `node --check apps/api/src/local_web/collection_workspace.js`：通过。
- `scripts/verify-ui-design-handbook.sh`：通过。
- 独立 PostgreSQL：完整 Work Resource/Material/Corpus 68 项，以及 Collection Control `8 + 11` 项通过；数据库、container、volume 已清理。
- 隔离浏览器：首版 33 migrations + synthetic fixture 的五页 1440×900、Attention 行、Tasks tab、Targets lifecycle drawer、0 console warning/error 已成立；修正版最终 1440 回执待稳定 exact head 后补记。

## 6. 分层结论

| 层 | 结论 |
|---|---|
| 源码与合同文档 | VERIFIED in worktree |
| Rust/JS/LIDS 自动检查 | VERIFIED |
| 一次性 PostgreSQL SSR 组合 | VERIFIED；synthetic only |
| 1440×900 浏览器视觉与交互 | VERIFIED；本报告范围内 |
| commit / push / PR / merge | AUTHORIZED；执行证据待补 |
| 共享 PostgreSQL / `:3000` / launchd | PostgreSQL 未授权；仅本机 `:3000` API 刷新已获授权，执行证据待补 |
| Browser Producer / 真实平台 | NOT TOUCHED |
| Mog 业务验收 | PENDING |
