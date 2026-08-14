# 灵感爆爆爆执行可靠性改造交接清单

> 交接日期：2026-05-24
> 交接对象：没有本项目历史背景的技术同事
> 项目根目录：`/Users/moglenny/proma/选题插件-打磨中/linggan-boom`
> 关联工作台目录：`/Users/moglenny/proma/内容工作台`
> 核心参考文件：`/Users/moglenny/Downloads/EXECUTION_CONTRACT_SPEC_v1.md`

## 1. 项目定位

「灵感爆爆爆」不是一个独立的小插件。它是「内容工作台」的浏览器执行端。

- 内容工作台负责：判断任务、组织调度、沉淀数据、分析结果。
- 插件负责：打开真实网页、执行采集、处理页面交互、把结果和状态回传给工作台。

因此，本次推进不能按“整理插件代码”理解，而要按“远程执行系统可靠性改造”理解。

最终目标：

> 任何一次采集任务，从被创建、被认领、被派发、被执行、失败、恢复、上传、结束，都必须有清晰状态、可恢复记录、可观测日志、可追踪错误、可验证结果。

## 2. 当前结论

本次讨论已经形成明确判断：

1. 应推进重大改造，但范围必须收住。
2. 只推进“执行可靠性核心重构”，不要做全仓库大翻修。
3. 可靠性问题优先于代码美化、组件拆分、工具函数去重。
4. 不要重写已经跑通的抖音内部控制器；先统一外部执行契约。
5. 不要全量 TypeScript 迁移；先在协议、任务事件、错误码、运行时边界做 JSDoc + schema 校验。

本次第一阶段不追求“代码变漂亮”，只追求：

- 任务不失踪
- 失败不沉默
- 重复不执行
- 状态可恢复
- 进度可解释
- 资源可清理
- 发布可验证

## 3. 背景问题拆分

原中期审查把很多问题都归为“架构问题”，但实际分两类。

### 3.1 交付可靠性问题，必须优先解决

这些问题会直接破坏插件和工作台之间的执行契约：

| 编号 | 问题 | 用户/工作台看到的症状 |
|---|---|---|
| P1 | Service Worker 状态丢失 | SW 重启后追不回任务、停不掉任务、关不掉辅助窗口 |
| P3 | `extractXhsProfileUserId` 多份实现不一致 | 任务正确派出，但执行端判断目标不匹配 |
| P4 | 异步任务失败后没有统一终态 | 工作台看到任务永远停在“执行中” |
| H6 | fire-and-forget 启动方式 | 页面没真正启动，工作台仍以为任务已接受 |
| H8 | heartbeat 与真实产出混在一起 | “有心跳无数据”难以判断 |
| H10 | 多 tab / 多任务无互斥 | 重复采集、数据污染、触发平台风控 |
| H19 | 监听器 / Observer 生命周期不清理 | SPA 页面跳转后监听器累积，状态污染 |

这些问题和调度中心之前看到的症状一一对应：

- 已派出但页面没启动
- 有心跳无数据
- 任务卡住反复重试
- 派出后被目标检查拒绝
- 永远执行中
- 工位忙碌但没有产出

### 3.2 可维护性技术债，后续渐进处理

这些是真的问题，但不能抢第一阶段资源：

- `normalizeText / normalizeObject / firstText` 等工具函数重复
- `background/index.js` 过大
- `Popup App.jsx` 和 `Dashboard App.jsx` 过大
- 小红书和抖音内部控制器风格不一致
- 架构文档、数据模型文档落后
- content 内联样式较多
- 部分协议消息缺文档

处理原则：

> 危险分歧先收敛，显眼重复后处理。
> 先统一执行契约，再拆代码结构。
> 先稳定交付，再追求架构整洁。

## 4. 本次改造总原则

以 `EXECUTION_CONTRACT_SPEC_v1.md` 为核心蓝图，先落地其中最关键的两类内容。

### 4.1 两个底座不变量

1. 状态可重建
   插件后台随时可能被 Chrome 杀掉，任何任务关键状态都不能只存在内存里。Service Worker 醒来后必须能恢复任务、tab、租约、未发送事件和执行锁。

2. 单执行互斥
   同一 `(平台, 账号)` 同一时间只能有一个重型采集任务执行。本地插件要加锁，服务端租约也要协同约束。

### 4.2 四段任务契约

远程任务必须显式经历：

```text
接受 / 派发 -> 运行确认 -> 进度增量 -> 终态
```

对应事件：

```text
task.claimed
task.running
task.progress
task.heartbeat
task.succeeded
task.failed
task.released
```

关键约束：

- `task.running` 只有在页面真的打开、目标真的确认后才能发。
- `task.heartbeat` 只表示“还活着”，不能表示“有产出”。
- `task.progress` 才表示真实推进。
- 任意失败路径必须落到 `task.failed` 或 `task.released`。
- 所有事件必须有 `eventId`，能幂等重发。
- 已作废的 `attemptId` 迟到事件必须被服务端忽略。

## 5. 第一阶段任务清单：执行可靠性止血

第一阶段是本次最重要交付。建议集中 1-2 周完成。

### T1. 建立执行事件信封

目标：插件向工作台回传的事件统一格式。

必须包含：

- `eventId`
- `taskId`
- `attemptId`
- `leaseId`
- `type`
- `occurredAt`
- `eventSeq`
- `stationId`
- `accountId`
- `payload`

插件侧重点文件：

- `src/workbench/protocol/schema.js`
- `src/workbench/runtime/taskDeltaReporter.js`
- `src/workbench/runtime/deltaOutbox.js`
- `src/db/workbenchOutboxStore.js`

工作台侧重点方向：

- collection task event 接收接口
- 事件唯一键
- 幂等写入
- 旧 attempt 事件忽略

验收：

- 同一事件重复上报不会产生重复状态变化。
- 网络断开后，事件能在恢复网络后补发。
- 旧 attempt 的迟到事件不会覆盖新 attempt 状态。

### T2. 拆分 heartbeat 与 progress

目标：让工作台能区分“任务活着”和“任务真的有产出”。

要求：

- heartbeat 只续租和证明活性。
- progress 只在有真实推进时发送。
- progress 要节流合并，避免抖音 heartbeat 风暴制造噪声。
- 工作台不能再把 heartbeat 当作最后产出时间。

插件侧重点文件：

- `src/workbench/runtime/heartbeat.js`
- `src/workbench/runtime/progressEvent.js`
- `src/workbench/runtime/taskPoller.js`
- `src/platforms/douyin/commentCollector.js`
- `src/platforms/douyin/batchController.js`
- `src/platforms/xhs/batchController.js`
- `src/platforms/xhs/batchCommentController.js`

验收：

- 任务运行 3 分钟只有 heartbeat、无 progress 时，工作台能标记“有心跳无数据”。
- 采集到记录时，工作台能看到 progress 推进。
- heartbeat 不再误导服务端判断“任务有产出”。

### T3. Service Worker 状态可重建

目标：Chrome 杀掉 SW 后，任务不能变成幽灵任务。

必须持久化：

- `taskId -> attemptId / leaseId / tabId / accountId`
- 当前 task context
- 由插件创建的辅助 tab/window
- 未确认上行事件
- 本地执行锁
- 最近运行阶段和最近进度

插件侧重点文件：

- `src/background/index.js`
- `src/workbench/runtime/taskPoller.js`
- `src/workbench/runtime/navigationOrchestrator.js`
- `src/db/collectionRunStore.js`
- `src/db/workbenchOutboxStore.js`

注意：

- 现在 `navigatedTabs` 已有 `chrome.storage.session` 兜底。
- `workbenchTaskRegistry` 仍主要是内存 Map，不能继续作为权威状态。
- 内存 Map 只能作为缓存，任何读取都要能从持久层恢复。

验收：

- 远程任务运行中手动 kill SW，唤醒后仍能恢复任务上下文。
- 任务仍能停止。
- 辅助 tab/window 仍能关闭。
- 未发出的事件能补发。
- 已作废 attempt 不再继续上报旧状态。

### T4. 显式运行确认

目标：消灭“已派出但页面没启动”。

规则：

- 认领成功后可以进入 `DISPATCHED`。
- 只有页面实际打开、content script 可用、目标身份确认后，插件才能发 `task.running`。
- 超过确认期限未收到 `task.running`，工作台回收任务并记录 `PAGE_NOT_STARTED`。

插件侧重点文件：

- `src/workbench/runtime/taskEnvelopeMapper.js`
- `src/workbench/runtime/capabilityCheck.js`
- `src/workbench/runtime/taskPoller.js`
- `src/background/index.js`
- `src/content/messageHandlers.js`

验收：

- 页面没有打开时，不进入 running。
- content script 不可用时，任务有明确失败/释放原因。
- 页面目标不一致时，失败原因是 `TARGET_MISMATCH`，不是泛化失败。

### T5. 所有异步任务必须有终态

目标：消灭永远 running / pending。

需要处理：

- `Promise.resolve().then(...).catch(console.error)` 路径
- `controller.start()` 后不等待、不接错误路径
- `.catch(() => {})` 中本该上报的错误
- 各平台任务的启动失败、采集中失败、上传失败、用户关闭 tab

插件侧重点文件：

- `src/content/messageHandlers.js`
- `src/content/douyinBatchMessageHandlers.js`
- `src/shared/managedTaskController.js`
- `src/workbench/runtime/errorMapper.js`
- `src/workbench/runtime/taskDeltaReporter.js`

验收：

- content script 主动抛错，工作台收到 `task.failed`。
- 用户关闭 tab，工作台收到 `task.released` 或明确失败。
- 页面安全验证出现，任务进入可解释失败/暂停。
- 不存在“任务一直执行中但没有任何终态”的路径。

### T6. 建立本地执行锁

目标：同一平台、同一账号不能并发执行重型采集。

锁粒度：

```text
platform + accountId
```

必要字段：

- `lockKey`
- `taskId`
- `attemptId`
- `tabId`
- `acquiredAt`
- `expiresAt`
- `reason`

规则：

- 远程任务拿不到锁：回 `task.released{reason: "account_busy"}`，服务端稍后重派。
- 手动任务拿不到锁：给用户明确提示“该账号已有任务执行中”。
- 锁必须有过期时间，SW 重启后能恢复或清理。

插件侧重点文件：

- `src/workbench/runtime/taskPoller.js`
- `src/workbench/runtime/executionStationRuntime.js`
- `src/background/index.js`
- `src/db/collectionRunStore.js`

工作台侧协同：

- 服务端租约也要按 `(platform, accountId)` 做防并发。
- 本地锁防同机并发，服务端租约防跨机并发。

验收：

- 两个 tab 同时启动同一账号采集，只有一个执行。
- 第二个任务被干净拒绝或延迟，不产生重复数据。
- 插件崩溃后锁不会永久占用。

### T7. 目标身份解析唯一化

目标：Background 和 Content 对同一 URL 得到同一个目标身份。

必须统一：

- 小红书 profile userId 解析
- 抖音 profile userId / secUid 解析
- 目标 URL 规范化
- `TARGET_MISMATCH` 错误构造

插件侧重点文件：

- `src/background/index.js`
- `src/platforms/xhs/batchController.js`
- `src/platforms/douyin/batchController.js`
- 建议新增 `src/shared/targetIdentity.js`

验收：

- 同一个小红书主页 URL，在 Background 与 XHS 执行层解析结果一致。
- 路由层认为可派发的任务，执行层不会因解析实现差异误拒绝。
- 所有目标不匹配都统一映射为 `TARGET_MISMATCH`。

### T8. 清理页面监听器和 Observer 生命周期

目标：SPA 页面跳转后，不累积旧监听器，不污染新任务。

范围：

- MutationObserver
- window/document 事件监听
- injected script bridge
- 抖音 API capture 监听
- dashboard iframe 通信监听
- 任务结束 / 页面切换 / stop / release 时的 cleanup

插件侧重点文件：

- `src/platforms/douyin/index.js`
- `src/platforms/douyin/uiInjector.js`
- `src/platforms/douyin/batchController.js`
- `src/content/dashboardBridge.js`
- `src/content/xhsPageController.js`

验收：

- 抖音 SPA 多次跳转后，不重复触发同一事件。
- 任务结束后再次启动，不会收到上一轮残留事件。
- stop/release 后观察器和监听器被清理。

## 6. 第二阶段任务清单：页面对抗与抽取健康度

第一阶段完成后推进。不要抢第一阶段资源。

### T9. Extractor 输出 schema 校验

目标：把“页面改版/抽取失败”和“本来没有数据”区分开。

范围：

- 小红书笔记
- 小红书评论
- 小红书作者
- 抖音视频
- 抖音评论
- 抖音作者

要求：

- 每类 extractor 都要有最低可用字段清单。
- 缺关键字段时返回 `EXTRACTION_FAILED` 或 `SELECTOR_BROKEN`。
- 不允许把空对象当成功结果。

建议落点：

- `src/workbench/protocol/validator.js`
- `src/db/recordNormalization.js`
- `src/platforms/xhs/*Collector.js`
- `src/platforms/douyin/*Collector.js`

验收：

- 页面改版导致关键字段缺失时，任务失败原因可解释。
- 工作台能按平台看到抽取健康度下降。

### T10. 抽取健康度遥测

目标：在用户报错前发现平台页面变化。

需要上报：

- parseAttemptCount
- parseFailureCount
- domFallbackUsed
- apiFallbackUsed
- selectorMissCount
- emptyResultReason
- platformBlocked
- securityChallengeVisible

落点：

- `src/workbench/runtime/taskRuntimeObservability.js`
- `src/workbench/runtime/errorMapper.js`
- 工作台监控快照 / 任务事件表

验收：

- 看到“空结果”时能判断是页面没数据、页面改版、账号问题还是平台限制。
- 工作台可按平台聚合抽取失败率。

### T11. 批量任务 checkpoint

目标：批量采集中页面刷新、SPA 跳转、SW 重启后，不完全丢进度。

范围：

- XHS 作者页 50 条建档
- XHS 搜索批量
- XHS 批量评论
- Douyin 作者页/搜索批量
- Douyin 评论图片下载

要求：

- 已发现列表落盘
- 已完成 item 落盘
- 当前 item / 当前页游标落盘
- 重启后能跳过已完成 item

落点：

- `src/db/collectionRunStore.js`
- `src/platforms/xhs/batchController.js`
- `src/platforms/xhs/batchCommentController.js`
- `src/platforms/douyin/batchController.js`

验收：

- 批量任务中途刷新页面，恢复后不会从零开始重复采集。
- 已采集内容不会重复上传。

## 7. 第三阶段任务清单：结构治理

第三阶段是可维护性改造，不能先于可靠性主线。

### T12. Background 拆分

优先顺序：

1. task registry / recovery
2. workbench handlers
3. cookie / download
4. accounts / sync
5. data 查询

注意：

- 不要一次性大搬家。
- 每次拆分必须行为不变。
- 拆分后构建产物仍要可运行。

### T13. 平台 Adapter 化

目标：外层统一接口，内层保留平台差异。

建议接口：

```javascript
{
  platform,
  detectPage(ctx),
  checkCapability(task, ctx),
  normalizeTarget(task),
  prepare(task, ctx),
  collect(task, ctx),
  pause(runId),
  resume(runId),
  stop(runId),
  cleanup(runId)
}
```

落点：

- `src/platforms/xhs/adapter.js`
- `src/platforms/douyin/adapter.js`
- `src/platforms/registry.js`

原则：

- 不重写抖音内部函数式实现。
- 不重写小红书内部类式实现。
- Task Runtime 只看统一 adapter 接口。

### T14. 协议边界 JSDoc + checkJs

先约束边界，不全量迁 TypeScript。

优先对象：

- task envelope
- execution event
- error code
- task state
- target identity
- extractor output schema

不优先对象：

- Popup UI
- Dashboard UI
- 平台采集器内部大段历史代码

### T15. 文档同步

必须同步：

- `docs/technical/DATA_MODEL.md`
- `docs/ARCHITECTURE.md`
- `docs/technical/MESSAGE_PROTOCOL.md`
- `docs/SELECTORS.md`
- `docs/plans/tech-debt.md`
- `progress.txt`

已知文档问题：

- 数据模型文档还停留在旧版本描述，真实 IndexedDB 已经到 v13。
- 架构文档对小红书评论采集路径描述落后，真实代码已经 API 优先、DOM 回退。
- 架构文档缺少若干新模块：运行态、导航编排、cookie 管理、任务清理、主题、content router 等。

## 8. 明确暂缓事项

以下事项不是不做，而是不能抢第一阶段资源：

1. 全量 TypeScript 迁移
2. 重写抖音批量控制器
3. 强行统一小红书和抖音内部实现模式
4. 大规模 Popup / Dashboard 拆分
5. 全量 normalize 工具函数去重
6. content CSS 大规模迁移到主题系统
7. 只为减少文件行数而拆 background

判断标准：

> 如果一个改动不能直接改善任务不失踪、失败不沉默、并发不乱跑、状态可恢复，就不要放进第一阶段。

## 9. 验收门槛

第一阶段完成时，必须逐条验收。

### 9.1 任务可靠性验收

- [x] 远程任务一定有终态：成功、失败、释放、取消或过期。
- [x] Content Script 抛错时，工作台收到 `task.failed`。
- [x] 页面未启动时，不会进入 running。
- [x] 页面目标不一致时，明确报 `TARGET_MISMATCH`。
- [x] tab 被用户关闭时，任务不永远 running。
- [x] 网络失败时，事件进入 outbox 并可重发。
- [x] 服务端能区分 heartbeat 与 progress。

### 9.2 SW 恢复验收

- [x] 任务运行中 kill Service Worker。
- [x] SW 唤醒后能恢复 task context。
- [x] 仍能停止任务。
- [x] 辅助 tab/window 能清理。
- [x] 未发送事件能补发。
- [x] 旧 attempt 事件不会污染新 attempt。

### 9.3 单执行互斥验收

- [x] 同一平台、同一账号、两个 tab 同时采集，只允许一个执行。
- [x] 第二个远程任务返回 `account_busy` 或等价 reason。
- [x] 第二个手动任务给用户明确提示。
- [x] 崩溃后锁不会永久占用。

### 9.4 页面适配验收

- [x] 抽取失败与本来无数据能区分。
- [x] 安全验证 / 风控能识别。
- [x] 页面改版导致关键字段缺失时，不写入伪成功数据。
- [x] SPA 跳转后监听器不累积。

验收记录：`docs/ACCEPTANCE_REPORT_2026-05-24_EXECUTION_FAULT_DRILL.md`。本轮自动故障演练覆盖 Service Worker 恢复、tab 关闭、页面刷新重注入、同账号互斥、辅助页清理和工作台控制指令；登录态平台手工点击采集不混入该记录，仍按发布候选包冒烟复验处理。

### 9.5 发布验收

插件仓库侧：

- [x] 相关 `node --test` 通过。
- [x] `npm run build` 通过。
- [x] `npm run release:verify` 通过。
- [x] zip 包内版本、manifest、构建产物与源码一致。

内容工作台侧：

- [x] 工作台相关测试通过。
- [x] `plugin-releases/linggan-boom-latest.zip` 已同步。
- [x] `plugin-releases/manifest.json` 的 `sha256 / size / updatedAt` 已同步。
- [x] Vercel deployment 为 `Ready`。
- [x] 正式域名已指向新部署。
- [x] 登录态下下载最新版插件能拿到新包。

验收记录：

- 插件全量 `node --test tests/*.test.mjs` 通过 `438` 个用例；`npm run test:douyin` 通过 `190` 个用例；故障演练通过 `74` 个用例。
- `npm run check:contracts`、`npm run build`、`npm run release:verify -- --version 2.0.19 --zip releases/linggan-boom-v2.0.19.zip` 通过。
- 工作台插件发布接口测试 `3` 个文件 / `19` 个用例通过；`npx prisma validate`、`npm run lint`、带本地占位密钥的 `npm run build` 通过。
- 正式域名 `https://lingganboom.fun` 指向部署 `dpl_5FjBcuDBn6bYab6xi4WF6hUpT41o`；登录态下载 ZIP 包内 `manifest.json` 为 `2.0.19`，并包含 `lgboom-install.json`。

注意：

- 未登录访问插件下载接口返回 `401` 是正常保护，不代表部署失败。
- GitHub 源码已推送不等于用户能下载到新插件。
- 插件源码、工作台托管下载包、生产站点部署必须分别确认。

## 10. 建议执行顺序

### 第 0 步：开工前核对

- [ ] 读 `EXECUTION_CONTRACT_SPEC_v1.md`。
- [ ] 读 `docs/reviews/MIDTERM_REVIEW_2026-05-23.md`。
- [ ] 读本交接清单。
- [ ] 查当前 git 工作区，避免覆盖他人改动。
- [ ] 确认当前线上插件版本和工作台下载包版本。

### 第 1 步：协议与状态账本

- [ ] 定义 execution event 信封。
- [ ] 给 outbox 事件补 `eventId / attemptId / leaseId / eventSeq`。
- [ ] 建立 task context 持久化。
- [ ] SW 唤醒时重水合。

### 第 2 步：显式运行与终态

- [ ] claim 后发 `task.claimed`。
- [ ] 页面确认后发 `task.running`。
- [ ] progress 与 heartbeat 分离。
- [ ] 所有失败路径发 `task.failed`。
- [ ] 用户/页面主动中断发 `task.released`。

### 第 3 步：互斥与目标统一

- [ ] 加 `(platform, accountId)` 本地锁。
- [ ] 服务端租约配合互斥。
- [ ] 统一 `extractProfileUserId`。
- [ ] 统一 target mismatch 错误。

### 第 4 步：恢复与清理

- [ ] kill SW 验收。
- [ ] tab 关闭验收。
- [ ] 辅助窗口清理。
- [ ] Observer / listener cleanup。

### 第 5 步：服务端闭环

- [ ] eventId 唯一去重。
- [ ] attemptId 过期事件忽略。
- [ ] DISPATCHED 确认超时回收。
- [ ] RUNNING 无 progress 超时回收。
- [ ] 错误分类和工位归因。

### 第 6 步：发布与文档

- [ ] 插件测试、构建、release verify。
- [ ] 工作台测试、构建、生产 Ready。
- [ ] 同步下载包和 manifest。
- [ ] 更新协议、架构、数据模型、进度文档。
- [ ] 输出稳定性报告。

## 11. 发版稳定性报告模板

每次发版给产品负责人一份简短报告，至少包含：

```text
版本：
源码 commit：
插件 zip：
工作台部署：

本次改动：
影响任务类型：
是否改数据库：
是否改消息协议：
是否改选择器：
新增错误码：

验收结果：
- kill SW：
- 关 tab：
- 页面错误：
- 账号失效：
- 重复启动：
- 网络失败：
- 正常任务：

已知风险：
回滚方式：
用户现在应该看到：
```

## 12. 给接手同事的工作原则

1. 先查真实代码和真实线上状态，不按旧文档猜。
2. 不要把服务端兜底当成日常路径；插件必须履行执行契约。
3. 所有任务都必须有终态。
4. 所有失败都必须可解释。
5. 所有关键状态都必须可重建。
6. 所有重型采集都必须有互斥。
7. 所有发布都必须同时确认源码、下载包、正式站。
8. 第一阶段不要做“看起来像架构”的大整理。

## 13. 完成定义

本项目阶段只有同时满足以下条件，才能算完成：

- 远程任务不会失踪。
- 失败不会沉默。
- heartbeat 与 progress 已分离。
- SW 重启后可恢复或可干净失败。
- 同一 `(平台, 账号)` 不会并发采集。
- target identity 只有一份共享实现。
- 工作台能按 `PAGE_NOT_STARTED / TARGET_MISMATCH / AUTH_REQUIRED / RISK_CONTROL / EXTRACTION_FAILED / NETWORK_ERROR / INTERNAL` 等原因解释失败。
- 插件与工作台测试均通过。
- 下载包和生产站点均已同步。
- 文档已更新。
- 产品负责人能看到一份清晰的稳定性报告。
