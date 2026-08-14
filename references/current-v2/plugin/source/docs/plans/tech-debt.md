# 技术债务清单

> 按当前代码现实维护。旧数字、旧文件路径和已经完成的治理项不再继续保留在这里误导后续开发。

## 高优先级

| 编号 | 问题 | 说明 | 状态 |
|------|------|------|------|
| T1 | Popup / Dashboard / DouyinAdapter 过胖 | 当前 `popup.js` 约 1171 行、`dashboard.js` 约 1174 行、`src/platforms/douyin/index.js` 约 774 行，新的复杂度中心已经转移到交互与编排层 | 待处理 |
| T2 | 长任务语义仍未完全统一 | 单条评论、批量评论、评论图片区仍存在”阶段文案相近但停止/暂停语义不完全一致”的风险 | 待处理 |
| T3 | 实机验收闭环仍不完整 | 2026-05-24 已新增执行故障演练记录，覆盖 Service Worker 恢复、tab 关闭、页面刷新重注入、同账号互斥、辅助页清理和控制指令；剩余主要是真实登录账号安装态冒烟采集，不再把代码级执行链路验收混在这里 | 持续监控 |
| T4 | 飞轮同步支线只有止血，没有完整产品化 | 当前已修复构建错配，但后台无法直接读取页面 IndexedDB，同步路径仍主要依赖 Popup 从页面上下文发起 | 待处理 |
| T13 | 代码审查发现的安全与效率问题（2026-04-20） | 见下方「2026-04-20 代码审查发现」章节 | 部分已修复 |

## 中优先级

| 编号 | 问题 | 说明 | 状态 |
|------|------|------|------|
| T5 | 加载边界需要按“稳定优先级”治理，而不是一刀切异步化 | 2026-05-17 已确认 Chrome 内容脚本不适合继续拆运行时异步 chunk，`contentDataRuntime / douyinRuntime` 保持 `webpackMode:"eager"`；`content.js` 约 `581 KiB`，后续减包需采用内容脚本安全方案 | 持续监控 |
| T6 | 消息协议 envelope 未统一 | Workbench execution contract 边界已补 `eventId / attemptId / leaseId / eventSeq`、记录结构校验和 `check:contracts`；仓内非执行链路接口仍同时存在 `{ success, data }`、裸数组、裸对象等返回格式 | 持续监控 |
| T12 | 工作台协议适配层已落地，但仍需继续收口 | `src/workbench/*` 已建立，第一批远程任务也已接入；但 `background` 网关、消息 envelope 和长任务语义仍未完全统一，后续若继续直接把逻辑塞进胖文件，复杂度仍会快速回升 | 持续监控 |
| T7 | BaseBatchController 尚未抽象 | XHS / 抖音批量链路仍有重复生命周期控制逻辑 | 待处理 |
| T8 | `contentRouter` 抽象尚未完全落地 | `hostname -> platform -> init` 已收进 `contentRouter/contentPlatformRegistry`；2026-05-24 又补了 `platforms/registry.js` 与 XHS / Douyin adapter，页面能力判断已走统一接口。剩余主要是按钮动作和采集控制仍有平台分支，后续随瘦身继续收口 | 持续监控 |
| T9 | 文档仍需持续防漂移 | `ARCHITECTURE / TECH_STACK / BACKEND_STRUCTURE / active plans` 必须跟着每轮结构变化同步，否则很容易再次落后 | 持续监控 |

## 低优先级

| 编号 | 问题 | 说明 | 状态 |
|------|------|------|------|
| T10 | 评论过滤规则（<6字节） | 目前仍按 UTF-8 字节数过滤，是否要继续细化到平台特定策略尚未决定 | 待确认 |
| T11 | 批量采集逐条 upsert 的 N+1 风险 | 目前主链路可用，但后续若批量规模继续扩大，可能需要评估 `bulkPut` | 待观察 |

## 已完成但不再重复作为当前债务

- `src/content/index.js` 超大单点中心：已收口到约 261 行
- 抖音视频 ID 体系错位：已改为页面状态 + 当前视频上下文
- 抖音下载 blob-only 误判：已修复为更稳定的多级降级链路
- 原生阻塞弹窗：Popup / Dashboard / 抖音页内已清零
- `xhs.batchNotes` 第一篇过早关闭：已修复为”目标 note 数据连续稳定确认后再采集”，并已通过真实页面复测

---

## 2026-04-20 代码审查（/simplify）发现

### 已修复

| 编号 | 问题 | 影响文件 | 修复内容 |
|------|------|----------|----------|
| R1 | **XSS** — popup.js 账号列表 `innerHTML` 未转义用户输入 | `popup/popup.js:1268` | 对 `a.name`、`statusText`、`a.accountId` 调用 `escapeHtml()` |
| R2 | **内存泄漏** — `workbenchTaskRegistry` 只 `.set()` 无 `.delete()` | `background/index.js:175` | 任务完成后清理三个 Map（workbenchTaskRegistry / taskExecutionTabRegistry / navigatedTabs） |
| R3 | **内存泄漏** — `taskPoller.seenControlIds` Set 无限增长 | `workbench/runtime/taskPoller.js:399` | 任务终态时 `.clear()` |
| R4 | **内存泄漏** — 抖音 `__lgboom_dy_video_data` 无上限 | `platforms/douyin/index.js:479` | 添加 200 条上限 FIFO 淘汰 |
| R5 | **死代码** — `syncNoteToFlywheel` / `syncAllToFlywheel` / `getSyncHistory` 永远返回错误 | `sync/flywheelSync.js` | 删除三个未调用函数 |
| R6 | **死代码** — `SYNC_NOTE_TO_FLYWHEEL` / `SYNC_ALL_TO_FLYWHEEL` 常量无 handler | `shared/constants.js` | 删除两个无用常量 |
| R7 | **死代码** — `inferProgressStage` 三组不可达的重复条件分支 | `popup/popup.js:762-770` | 删除不可达分支 |
| R8 | **重复代码** — `normalizeServerUrl` 在 5 处各自定义 | `sync/flywheelSync.js` 等 | 提取到 `shared/utils.js` 并在 flywheelSync 中引用 |
| R9 | **重复代码** — `sendToTab` 三处重复实现（popup.js / popup/utils.js / background/index.js） | `shared/messaging.js` 等 | 提取统一 `sendToTab()` 到 `shared/messaging.js`；`background` 仅保留默认超时兼容包装 |
| R10 | **重复状态机** — `content/index.js` / `douyin/index.js` 各自维护 TaskController 逻辑 | `shared/managedTaskController.js` 等 | 提取共享 `createManagedTaskController()`；content 与 Douyin 批量统一复用 |
| R11 | **重复代码** — Workbench runtime 客户端仍各自维护 `normalizeServerUrl` | `workbench/runtime/taskLeaseClient.js` / `executionStationClient.js` | 改为统一引用 `shared/utils.js` 的 `normalizeServerUrl()` |
| R13 | **安全/配置** — `flywheelSync.js` API_TOKEN 硬编码为 `'dev-token'` | `sync/flywheelSync.js` | 改为从 `flywheelConfig.apiToken` 读取；未配置时不再默认注入固定 bearer token |
| R15 | **重复代码** — popup 批量设置弹层 cleanup / 重置逻辑重复 | `popup/popup.js` | 提取 `resetBatchSettingsOverlay()` 与共享评论深度读取 helper |
| R16 | **重复状态** — Background 里 task registry / execution tab registry 分离维护 | `background/index.js` | 改为统一使用 `workbenchTaskRegistry` + `get/set/clearWorkbenchTaskContext()` 收敛 tab / task 查询 |
| R17 | **状态漂移** — `300017` 切号后本地 poller 仍停留在 `dispatched` | `background/index.js` / `workbench/runtime/taskPoller.js` | 风控切号后会同步把 poller 内存态改成 `paused`，避免 45 秒后被误判为 dispatch startup timeout |
| R18 | **配额时机错误** — 替换账号在 resume 前就提前扣 usage | `background/index.js` / `workbench/runtime/taskPoller.js` | 切号后只记录 `pendingAccountUsageId`，等任务真正恢复并拿到 run 再消费 usage |
| R19 | **重试语义错误** — 启动超时文案写“自动释放重试”但状态打成 `failed` | `workbench/runtime/taskPoller.js` | 启动超时现在回到 `pending` 且清空本地 activeTask/lease，后续轮询可重新认领 |
| R20 | **数据风险** — Dashboard 桥接 nonce 暴露且未校验 iframe 来源 | `content/dashboardBridge.js` / `dashboard/utils.js` | iframe URL 不再携带 nonce，并要求消息来源必须是真实 dashboard iframe |
| R21 | **性能风险** — notes/authors 按 `collectionRunId` 查询全表扫描 | `db/index.js` / `db/noteStore.js` / `db/authorStore.js` | 新增 v13 索引并改为 indexed lookup；Dashboard 改为按批次读取本地记录 |
| R22 | **死代码** — React 迁移后仍保留旧 `src/popup/popup.js` | `src/popup/popup.js` / `tests/*` | 删除旧源文件，测试改为校验 React App 与 utils 入口 |

### 待继续收口

| 编号 | 问题 | 影响范围 | 建议 |
|------|------|----------|------|
| R12 | Content bundle 仍高于 webpack 默认建议阈值 | 构建产物 | 为避免 Chrome 内容脚本异步 chunk 加载风险，已回到 eager 运行时加载；`content.js` 约 581 KiB，后续若要继续降包，需要换成内容脚本安全的加载策略 |
| R14 | 状态字符串 repo 级统一仍未完成 | 30+ 处 | 热路径 UI/controller 已开始统一走 `TASK_STATE`，但仓内剩余字符串状态仍多，建议继续随大重构逐步收口 |
