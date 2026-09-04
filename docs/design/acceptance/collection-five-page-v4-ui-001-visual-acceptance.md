# ACC-COLLECTION-FIVE-PAGE-V4-UI-001 · 五页面 V4 桌面验收

> 状态: 一次性报告
> 最后核对: 2026-09-04
> 适用范围: `codex/collection-five-page-v4-ui`；原始基线 `84fd9498e18164011621ffabc81a2a421a52f7a1`，分支已并入 `origin/main@e5c5ec5f9bab62f25bea9798fd9abb127df03956`
> 事实来源: 当前分支代码与测试、一次性 PostgreSQL 16；首版隔离 `:3311` / 1440×900 浏览器证据只属于已被复审拒绝的首版 head
> 冲突时以谁为准: 用户最新确认、真实代码/数据库/运行结果、PAGE-COLLECTION-001、Package 1/2 合同与 LIDS

## 1. 验收边界

本报告验证五个真实 Collection route 对 V4 桌面信息架构的复刻，不验证参考 HTML 的模拟业务数字。参考文件 `/Users/moglenny/Downloads/linggan-collection-field-workspace-v4-ia-cn.html` 是 synthetic UI 对标，只提供结构、密度、几何和交互关系；全局壳层、事实、权限、状态与行动后果仍以 Intelligence 当前实现为准。

支持范围固定为 1440 CSS px 桌面全屏。1280、390、手机、小于 13 寸屏幕和 responsive parity 均不在本次验收范围。

## 2. 测试环境

| 项目 | 本次事实 |
|---|---|
| Git base | 原始 `origin/main@84fd9498e18164011621ffabc81a2a421a52f7a1`；当前 main `e5c5ec5f9bab62f25bea9798fd9abb127df03956` 已合并到交付分支 |
| 分支 | `codex/collection-five-page-v4-ui`；第二轮复审修正与 main 集成已完成，第三轮 exact-head 复审待执行；Mog 已授权 commit/push/merge 与本机 `:3000` 刷新 |
| PostgreSQL | 一次性 `postgres:16-alpine`；按当前顺序应用 33 个 migration |
| Fixture | synthetic：2 targets、2 scheduler decisions、3 tasks；不是共享库或真实平台数据 |
| HTTP | 隔离 loopback `127.0.0.1:3311`；共享 `:3000` 未改动 |
| 浏览器 | 首版：带 synthetic fixture 的 1440×900，已被复审拒绝；当前修正版：无数据库隔离 API + Chrome 152，显式 1440×900 终态/几何回归 |
| 外部系统 | 未操作共享数据库、launchd、Browser Producer、Chrome extension 或真实平台 |

## 3. 页面结果

| 页面 | 可见结构 | 数据与状态诚实性 | 浏览器结果 |
|---|---|---|---|
| 待处理 | 2 个可见范围说明的 Context KPI、恢复 ledger、右侧 Inspector | 只列可恢复 durable reason；owner/action 与选中行一致 | 1440 无数据库终态无溢出/无 loading/console clean；有数据选择由 source test 固定 |
| 观察目标 | 2 个可见范围说明的 Context KPI、筛选/新增、密集目录、宽幅抽屉 | UNKNOWN/0 分开；creator/keyword lifecycle 分开；不呈现语料或价值模块 | 1440 无数据库终态无溢出/无 loading/console clean；drawer 生命周期由 source/PostgreSQL test 固定 |
| 生产流 | 2 个可见范围说明的 Context KPI、stage ledger、dark decision instrument | stage 不是漏斗；恢复数不包含普通等待态；trace/review 不被 now 覆盖 | 1440 无数据库终态无溢出/无 loading/console clean；真实 projection/mode 由测试固定 |
| 采集任务 | 2 个可见范围说明的 Context KPI、Task ledger、右侧多 tab Inspector | Task/Attempt/Package/Receipt/frozen Work 分责；最新 Lease 的全部 Task 均有自己的 frozen tab；Inspector 状态样式随选择更新 | 1440 无数据库终态无溢出/无 loading/console clean；双 Task frozen SQL 与交互由 PostgreSQL/source test 固定 |
| 执行工位 | 2 个可见范围说明的 Context KPI、capacity 判定、资源关系、登记/认领区 | 使用服务端同一 capacity evaluator；不把存在插件版本冒充在线 | 1440 无数据库终态无溢出/无 loading/console clean；capacity 由 PostgreSQL test 固定 |

首轮带 synthetic fixture 的五页 1440 结果属于后来被复审拒绝的 head，不作为当前证明。当前修正版以无数据库隔离 `:3311` 强制 1440×900 复核五页终态：每页 `clientWidth=scrollWidth=1440`、恰有 2 个 `.v7-kpi` 且各自直接显示 scope/source、无 `.c-readout-strip`、唯一 `h1.v7-sr-only` 的 box 为 1×1 absolute、无“正在读取”，第二次逐页日志采集 warning/error/exception 均为 0。该回执证明当前 head 的桌面几何与失败终态，不冒充真实数据密度或有数据交互证明。

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
- `cargo test --workspace --all-targets --locked`：当前 main 集成后的 source tree 通过；API `111 passed / 19 ignored`，其余 workspace 目标全绿。
- `node --check apps/api/src/local_web/collection_workspace.js`：通过。
- `scripts/verify-ui-design-handbook.sh`：通过。
- 独立 PostgreSQL：完整 Work Resource/Material/Corpus/Topic/API 69 项、Collection dispatch 8 项，以及 Collection Control `8 + 11` 项通过；其中 69 项包含同一 Lease 双 Task frozen projection 的真实 SQL 回归。每套数据库、container、volume 均已清理。
- 隔离浏览器：当前 source tree 的无数据库 API `:3311` + Chrome 152 强制 1440×900，五页均无横向溢出、无 loading 残留、Context scope/source 可见、无第二 readout strip，应用页面 console warning/error/exception 为 0。首版带数据交互结果不替代当前 head；有数据行为由 exact PostgreSQL/source tests 证明。

## 6. 分层结论

| 层 | 结论 |
|---|---|
| 源码与合同文档 | VERIFIED IN INTEGRATED BRANCH；第三轮独立复审待执行 |
| Rust/JS/LIDS 自动检查 | VERIFIED on current source tree |
| 一次性 PostgreSQL SSR 组合 | VERIFIED；synthetic only；69 + 8 + 8 + 11 |
| 1440×900 浏览器视觉与终态 | VERIFIED on current source tree；无数据库终态/几何范围；有数据交互不由该回执声称 |
| commit / push / PR / merge | AUTHORIZED；执行证据待补 |
| 共享 PostgreSQL / `:3000` / launchd | PostgreSQL 未授权；仅本机 `:3000` API 刷新已获授权，执行证据待补 |
| Browser Producer / 真实平台 | NOT TOUCHED |
| Mog 业务验收 | PENDING |
