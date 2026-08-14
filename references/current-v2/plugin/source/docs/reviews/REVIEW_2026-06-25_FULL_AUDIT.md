# 灵感爆爆爆插件 — 全面审查报告

> 审查日期：2026-06-25
> 审查范围：架构、设计、代码可靠性、死代码、功能问题、文档与代码一致性
> 审查方式：全量读 `src/`、`docs/`、`manifest.json`、`webpack.config.cjs`、`package.json`，用代码事实校准文档
> 当前版本：`2.0.52`（manifest 与 package.json 一致）
> 规模：约 170 个源文件、4 万行代码、110 个测试文件、13 个数据库版本

---

## 0. 一句话结论

这是一个**完成度远超"插件脚本"水平、已经具备企业级执行端雏形**的项目。代码、测试、文档三条线都维护得很扎实，最大问题不是"写得烂"，而是**几份导航文档（TECH_STACK / ARCHITECTURE）已经滞后于代码**，以及**少量 React 迁移后没清理干净的死代码**。把这两类问题收口后，项目就具备"可交付给另一个 agent 1:1 复刻"的条件。

---

## 1. 总体评价（给非技术读者）

| 维度 | 评分 | 说明 |
|---|---|---|
| 架构合理性 | 优 | 分层清晰：Background / Content / 平台适配器（xhs、douyin）/ 数据层 / 工作台协议层 / UI。平台差异通过 `platforms/registry.js` 收口，不是 if-else 满天飞 |
| 代码可靠性 | 良→优 | 关键路径都有测试（110 个测试文件），协议层有 `check:contracts` 静态校验，回写队列有幂等键 + 复合索引 + 过期回收 |
| 文档完整度 | 优（产品/协议层）/ 滞后（导航层）| PRD、APP_FLOW、MESSAGE_PROTOCOL、DATA_MODEL、SELECTORS 与代码高度一致；但 TECH_STACK、ARCHITECTURE 还停留在 v7/2.0.0 时代 |
| 死代码 | 少量但明确 | 1 个整文件 + 16 个导出符号从未被引用，都是 React 迁移和早期飞轮同步遗留 |
| 功能完整度 | 高 | 双平台单条/批量/评论/博主/媒体/工作台远程任务全部落地，仅"真实账号长时效实机闭环"未签收 |

---

## 2. 架构与设计亮点（值得保留）

1. **执行端定位清晰**：插件不试图做主系统，所有"判断/沉淀/Topic"都归内容工作台。AGENTS.md 把这个分工写成铁律。
2. **平台适配器收口**：`platforms/registry.js` + `xhs/adapter.js` + `douyin/adapter.js` 把"页面能力自检"统一成一个接口 `checkCapability()`，远程任务派单前必走这道门。
3. **任务账本 + 租约 + 心跳**：远程任务有完整的 `claimed → running → paused/stopped/completed/failed` 状态机，配 3 秒心跳、2 小时租约超时、断点续跑 checkpoint。
4. **回写 outbox 三重防护**：`workbenchOutbox` 表有 `&idempotencyKey` 唯一索引（v12）+ `[status+nextAttemptAt+createdAt]` 复合索引（v10，避免全表扫描）+ `in_flight` 5 分钟过期回收（P0-1 已修）。
5. **Dashboard 桥接一次性 nonce**：iframe 不再在 URL 暴露 nonce，且校验消息来源必须是真实 dashboard iframe（R20 已修）。
6. **采集器证据层**：每条记录都带 `collectorVersion / rawPayload / rawDomText / rawUrl / rawSource`，方便事后追溯采错。
7. **选择器验证文化**：`SELECTORS.md` 每条都带验证状态和日期，30 天过期规则，配套 7 个 `probe-*.js` 探查脚本。

---

## 3. 发现的问题清单

### 3.1 文档与代码不一致（高优 — 会直接误导复刻 agent）

| 编号 | 文档 | 写的 | 实际 | 影响 |
|---|---|---|---|---|
| D1 | `docs/technical/TECH_STACK.md` §1 | "当前本地 schema：Dexie `v7`" | `db/index.js` 已到 **v13** | 复刻 agent 会建错数据库 |
| D2 | `docs/technical/TECH_STACK.md` §5 | 权限含 `debugger`；缺 `cookies/tabs/alarms/notifications/declarativeNetRequestWithHostAccess` | manifest 实际**无 `debugger`**；`chrome.debugger` 在 src/ 零引用；DISPATCH_ESC 已改用 `chrome.scripting.executeScript` 派发键盘事件 | 复刻 agent 会申请错权限，且会以为有 debugger 代码 |
| D3 | `docs/technical/TECH_STACK.md` §5 | host_permissions 写 `https://www.xiaohongshu.com/*`、`http://localhost:*/*` | manifest 实际是 `https://xiaohongshu.com/*` + `https://*.xiaohongshu.com/*` + `https://www.xiaohongshu.com/*` + `https://lingganboom.fun/*` + `http://localhost/*`（无端口限定）| 复刻 agent 会漏配 host |
| D4 | `docs/ARCHITECTURE.md` §1 | 版本 `2.0.0` | `2.0.52` | 误导 |
| D5 | `docs/ARCHITECTURE.md` §12 | "32 个测试文件" | **110 个** | 严重低估测试覆盖 |
| D6 | `docs/ARCHITECTURE.md` §3.3 / §14 | 又把 `debugger` 列进权限 | 同 D2 | 同 D2 |
| D7 | `docs/ARCHITECTURE.md` §6 | 列出 `commentCollectTask.js` 为活跃模块 | 该文件**零代码引用**，是死文件（见 3.2）| 复刻 agent 会复活死代码 |
| D8 | `docs/ARCHITECTURE.md` §10 | 把 `sendToContent` 列为共享工具 | 该函数导出后**零引用**（见 3.2）| 同上 |
| D9 | `docs/technical/TECH_STACK.md` §6 | 构建快照写 "2026-05-17"，content.js 581 KiB | 当前 dist/content.js **667 KiB**；background 247→312 KiB | 体量信息过期 |

**建议**：本轮直接修 D1/D2/D3/D7/D8（影响复刻正确性），D4/D5/D6/D9 一并更新。

### 3.2 死代码（中优 — 不影响运行，但污染复刻）

> **2026-06-25 后续：本节列出的死代码已全部清理**（1 整文件 + 16 个零引用导出符号 + 2 个连带未使用 import/const），详见 `progress.txt` 2026-06-25 第二条。`npm run check:contracts` / `npm run build` 通过，528 测试 527 过（唯一失败 pre-existing，与本次无关）。

用"导出符号在仓库内被引用次数 = 1（只有定义本身）"精确扫描，确认以下 16 个符号 + 1 个整文件完全无引用：

| 类型 | 路径 / 符号 | 说明 |
|---|---|---|
| **整文件死** | `src/content/commentCollectTask.js`（`createCommentCollectTaskController`）| 全仓库零 `import`，只在 ARCHITECTURE.md 被提到。评论任务控制实际走 `commentTaskController.js` + `shared/managedTaskController.js` |
| **模块整死** | `src/shared/taskUi.js` 中 7 个函数：`applyFloatingToastStyle / applyTaskbarShellStyle / buildTaskbarMarkup / cardStyle / dialogStyle / inputStyle / updateTaskbarView` | React 迁移前的原生 DOM 渲染函数，UI 已全部改走 `src/content/components/*.jsx` |
| **单个死函数** | `sendToContent` (`src/shared/messaging.js`) | 被 `sendToTab`/`sendToBackground` 取代 |
| **单个死函数** | `checkFlywheelConnection` (`src/sync/flywheelSync.js`) | 早期飞轮连接测试遗留，连接测试已走 `TEST_FLYWHEEL_CONNECTION` 消息 |
| **单个死函数** | `isAcUiTheme / onThemeChange / parseBatchLikes / normalizeMonitorTaskStrategy / isContentEnvelopeAction / isSupportedRemoteTaskType` | 散落在 themes/utils/workbench，均为重构后的残留导出 |

**建议**：直接删除 `commentCollectTask.js` 整文件 + `shared/taskUi.js` 内 7 个函数 + 上述单函数导出。删前搜一遍 tests/ 是否有契约测试引用（已确认无）。

### 3.3 可靠性隐患（中优 — 不阻塞，但应排期）

| 编号 | 位置 | 问题 | 建议 |
|---|---|---|---|
| R1 | `src/background/index.js` 末尾 + `onStartup` + `onInstalled` + 顶层 | `WORKBENCH_TASK_POLL_ALARM` 在 **4 个地方**重复 `chrome.alarms.create`（顶层、onStartup、onInstalled 各一次，加上 daily-quota-reset）。MV3 service worker 重启会再跑一次顶层，虽然 Chrome 会去重，但维护成本高 | 收口为一个 `registerAlarms()` 函数，三处调用 |
| R2 | `manifest.json` | `notifications` 权限已申请，但 `src/` 内 `chrome.notifications` 调用要确认是否真在用（本轮未深入）| 若未用则删权限；若用则补文档 |
| R3 | `src/` 全局 | **92 处 console.log/warn/error** 进了生产 bundle（content.js 667 KiB）| 加 webpack `terser-plugin drop_console`，或 babel 插件按环境剔除 |
| R4 | `docs/plans/tech-debt.md` T4 | 飞轮同步支线只有止血；后台无法直接读页面 IndexedDB，同步仍依赖 Popup 发起 | 排期产品化或明确退役 |
| R5 | `manifest.json` host_permissions | 同时声明 `https://xiaohongshu.com/*` 和 `https://*.xiaohongshu.com/*` 和 `https://www.xiaohongshu.com/*`，后两条是第一条的子集 | 可精简，但不影响功能 |
| R6 | `webpack.config.cjs` | `devtool: 'cheap-module-source-map'` 在生产模式仍启用，source map 进 bundle | 生产构建改 `false` 或 `source-map` 单独文件 |

### 3.4 功能问题与已知空缺（信息性 — 非本轮新增）

这些项目自己的 `progress.txt`、`PLUGIN_REVIEW_REPORT.md`、`tech-debt.md` 已经如实记录，不是新发现，列在这里供复刻 agent 知情：

- 抖音远程任务**无账号池自动切换**（小红书有）：抖音远程任务依赖当前浏览器登录态，无法像小红书一样自动换号/控配额/进冷却（P1-2）。
- 监控任务的"页面连接中断"曾被映射为 `failed`，污染失败率（P1-3），需确认是否已对齐工作台状态口径。
- 注入脚本 `injected/xhsApiCapture.js`、`douyinApiCapture.js` 覆盖全局 `fetch`/`XMLHttpRequest`（P1-4），已有 `__lgboom_*_installed` 防重，但仍需关注平台检测风险。
- 工作台自动接单主链路**仍需一轮真实账号实机闭环**（pending → dispatched → running → completed/failed）。
- `content.js` 667 KiB 偏大（T5/R12），内容脚本不能拆异步 chunk，已是当前最佳折中。
- 结果 envelope 未统一为 `{ success, data, error }`（T6），消费方仍按 action 容错解析。

---

## 4. 复刻可行性评估

**结论：完全可复刻，但必须用"代码事实"而不是"导航文档"作为复刻依据。**

| 复刻资产 | 状态 |
|---|---|
| 产品功能清单 | ✅ `docs/product/PRD.md` + `APP_FLOW.md` 已够 |
| 消息协议 | ✅ `docs/technical/MESSAGE_PROTOCOL.md` 与 `src/shared/constants.js` 一致 |
| 数据库 schema | ✅ `docs/technical/DATA_MODEL.md` 与 `src/db/index.js` v13 一致 |
| 选择器 | ✅ `docs/SELECTORS.md` 带验证日期 |
| 工作台协议 | ✅ `docs/technical/MESSAGE_PROTOCOL.md` §2.7-2.8 + `PLUGIN_AUTHORIZATION_PROTOCOL.md` |
| 构建配置 | ✅ `webpack.config.cjs` + `package.json` |
| 导航文档 | ⚠️ TECH_STACK / ARCHITECTURE 滞后，**不能直接喂给复刻 agent** |

因此本轮**新增一份自包含的复刻蓝图**：`docs/REPLICATION_BLUEPRINT.md`。
它把另一个 agent 复刻所需的全部契约（MSG 全量、schema 全量、workbench 协议全量、错误码、监控策略、magic number、构建命令、铁律）集中到一份文档，并明确标注"事实源 = 代码"，避免 agent 凭印象幻觉。

---

## 5. 本轮建议执行顺序

1. **修滞后文档**（D1/D2/D3/D7/D8 + D4/D5/D6/D9）— 防止复刻 agent 第一步就走偏。
2. **删死代码**（`commentCollectTask.js` 整文件 + `taskUi.js` 7 函数 + 5 个单函数导出）— 减少复刻噪音。
3. **完成 `docs/REPLICATION_BLUEPRINT.md`** — 这是用户本轮的核心交付物。
4. （可选）排期 R1/R3/R6 等可靠性优化。

---

## 6. 本轮已同步的文件

| 文件 | 动作 |
|---|---|
| `docs/reviews/REVIEW_2026-06-25_FULL_AUDIT.md` | 新增（本文件）|
| `docs/REPLICATION_BLUEPRINT.md` | 新增（复刻蓝图）|
