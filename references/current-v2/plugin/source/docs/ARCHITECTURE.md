# linggan-boom — 系统架构描述

> 本文件是架构的机器可读描述，供 AI agent 审查、优化、迭代。
> 不含任何视觉坐标信息。需要生成架构图时，从本文件渲染。

---

## 1. 项目概览

| 属性 | 值 |
|---|---|
| 名称 | 灵感爆爆爆 (linggan-boom) |
| 类型 | Chrome Extension (Manifest V3) |
| 版本 | 2.0.52 |
| 目标平台 | 小红书 (xhs) + 抖音 (douyin) |
| 定位 | 多平台内容灵感采集工具箱 |
| 上游系统 | 内容工作台 (Content Workbench) |

---

## 2. 技术栈

| 层 | 技术 | 用途 |
|---|---|---|
| 构建 | Webpack 5 | 多入口打包（content / background / popup / dashboard） |
| 存储 | Dexie (IndexedDB) | 本地数据持久化，13 次版本迁移 |
| 打包下载 | JSZip | 评论图片批量打包下载（按需动态加载） |
| 样式 | 纯 CSS | content.css, popup.css, dashboard.css |
| 语言 | JavaScript (ES2020+) | 无 TypeScript |
| 测试 | Node.js test runner (.mjs) | 110 个测试文件 |
| 脚本 | 探查脚本 (scripts/probe-*.js) | 一次性 DOM/API 结构调研 |

---

## 3. Extension 架构 (Chrome MV3)

### 3.1 Extension 上下文

| 上下文 | 入口文件 | 说明 |
|---|---|---|
| **Background Service Worker** | src/background/index.js | 常驻后台：下载管理、媒体屏蔽、消息路由、工作台任务轮询 |
| **Content Script** | src/content/index.js | 注入 xiaohongshu.com / douyin.com 页面：采集数据、注入 UI、平台路由 |
| **Popup** | src/popup/popup.html + src/popup/index.jsx (构建产物为 popup.js) | 扩展弹窗：统计、采集触发、同步配置 |
| **Dashboard** | src/dashboard/dashboard.html + src/dashboard/index.jsx (构建产物为 dashboard.js) | 数据看板 iframe：浏览/搜索/导出已采集数据 |
| **Injected Scripts** | src/injected/*.js | 注入页面主世界 (main world)：访问 `__INITIAL_STATE__`、拦截 fetch/XHR |

### 3.2 上下文间通信

```
Popup ──sendToBackground──→ Background ──sendToTab──→ Content Script
  ↑                                                          │
  └─────────────── reportProgress ←──────────────────────────┘

Dashboard ──postMessage──→ Content Script (parent) ──sendToBackground──→ Background

Content Script ←──postMessage──→ Injected Script (main world)
```

### 3.3 权限

```
activeTab, tabs, storage, cookies, downloads, alarms, scripting,
declarativeNetRequest, declarativeNetRequestWithHostAccess, notifications
```

> 本项目**不使用** `chrome.debugger`（关闭弹窗/派发 Esc 走 `chrome.scripting.executeScript`）。

Host permissions: `xiaohongshu.com`、`*.xiaohongshu.com`、`www.douyin.com`、`lingganboom.fun`、`localhost` 及媒体 CDN 域名（完整清单见 `manifest.json` / `docs/technical/TECH_STACK.md` §5）

### 3.4 加载边界

| 模块 | 加载方式 | 说明 |
|---|---|---|
| content.js / douyinRuntimeModule.js / contentDataRuntime.js | 静态打包 | 动态 import 在 content script 场景下会丢失 chunk，已回退为静态 |
| JSZip | 按需动态加载 | 仅评论图片打包时加载 |
| Injected scripts | 运行时注入 | 通过 `getByInject()` 创建 `<script>` 标签注入 |

---

## 4. 消息协议

### 4.1 消息类型 (MSG 常量 — src/shared/constants.js)

| 分类 | 消息 | 说明 |
|---|---|---|
| **单条采集** | COLLECT_SINGLE_NOTE, COLLECT_SINGLE_COMMENT, COLLECT_AUTHOR | 采集当前页面内容 |
| **批量控制** | START/STOP/PAUSE/RESUME_BATCH_NOTES, *_BATCH_COMMENTS | 批量采集生命周期 |
| **后台操作** | BLOCK_MEDIA, UNBLOCK_MEDIA, DOWNLOAD_MEDIA_FILE, FETCH_BINARY_AS_DATA_URL | 媒体管理与下载 |
| **数据操作** | GET_STATS, EXPORT_CSV, EXPORT_JSON, DELETE_*, CLEAR_ALL_* | 本地数据管理 |
| **工作台** | WORKBENCH_CAPABILITY_CHECK, WORKBENCH_DISPATCH_TASK, WORKBENCH_SYNC 等 | 与内容工作台集成 |
| **状态** | PROGRESS, COLLECT_DONE, ERROR | 任务状态通知 |

### 4.2 任务状态机

```
idle → running ⇄ paused → stopping → done / error
```

### 4.3 批量采集配置

| 参数 | 说明 |
|---|---|
| COLLECT_MODE | search, profile, favorite |
| COMMENT_DEPTH_MODE | twoLevel, allReplies |
| BATCH_CONFIG | 采集数量、排序方式、滚动间隔、时间常量 |

---

## 5. 平台采集层

### 5.1 小红书 (XHS) — src/platforms/xhs/

| 模块 | 文件 | 职责 |
|---|---|---|
| 入口 | index.js | 导出所有 XHS 功能 |
| **笔记采集** | noteCollector.js | 从 `__INITIAL_STATE__` 和 DOM 采集单条笔记；列表页笔记发现与滚动加载 |
| **评论采集** | commentCollector.js | DOM 滚动采集评论，支持分页、回复展开、验证码检测 |
| **博主采集** | authorCollector.js | 从 `__INITIAL_STATE__.user` 和 DOM 采集博主信息 |
| **批量控制** | batchController.js | BatchNoteController / BatchCommentController，支持暂停/恢复/停止 |
| **评论批量** | batchCommentController.js | 多笔记评论批量采集协调 |
| 共享状态 | batchShared.js | 批量任务的共享状态与工具函数 |
| 页面检测 | pageDetector.js | NOTE_DETAIL / SEARCH / PROFILE 页面类型识别 |
| UI 注入 | uiInjector.js | 注入操作按钮和对话框 |
| **反检测** | antiDetect.js | 模拟人类滚动、验证码监控、频率限制 |

#### XHS 数据源

| 数据 | 来源 | 说明 |
|---|---|---|
| 笔记详情 | `__INITIAL_STATE__.note.noteDetailMap` | 全量笔记数据 |
| 博主信息 | `__INITIAL_STATE__.user.userPageData` | 博主资料 |
| 评论 | DOM 解析 + 自动滚动 | **依赖页面 DOM 结构，改版即失效** |
| 列表发现 | DOM 解析 + 滚动加载 | 搜索页/个人页笔记发现 |

### 5.2 抖音 (Douyin) — src/platforms/douyin/

| 模块 | 文件 | 职责 |
|---|---|---|
| 入口 | index.js | SPA 路由处理 + 原生分享捕获 |
| **视频采集** | videoCollector.js | 多源视频数据采集（DOM + API + render data） |
| API 数据 | videoApiData.js | API 响应映射、缓存、fallback API 调用 |
| 视频上下文 | videoContext.js | 识别当前活跃视频 |
| DOM 解析 | videoDom.js | DOM 中提取视频元数据 |
| **视频下载** | videoDownload.js | 页面上下文 fetch 绕过 CDN 鉴权 |
| **评论采集** | commentCollector.js | API 采集评论 + 评论图片批量下载 + ZIP 打包 |
| 评论 API | commentApi.js | 评论 API 封装，chunked JSON 解析 |
| 评论媒体 | commentMedia.js | 评论图片资产提取 |
| **博主采集** | authorCollector.js | render data + API + DOM 多源采集 |
| 批量控制 | batchController.js | 个人页/搜索页批量视频和评论采集 |
| 批量发现 | batchDiscovery.js | 目标发现 + Top-N 排序 |
| 页面检测 | pageDetector.js | VIDEO_DETAIL / NOTE_DETAIL / SEARCH / PROFILE |
| UI 注入 | uiInjector.js | 平台专属 UI 注入 |

#### Douyin 视频上下文原则

抖音必须遵守"当前视频上下文"原则，原因：
1. 博主页视频播放是弹层 + SPA 复用 DOM
2. URL、DOM、API 返回会错峰更新
3. vid / modal_id / 可见标题 / 激活视频元素并不总是同时正确

约束：
- 先判断页面状态（主页预览态 / 博主页弹层态 / 直接视频页）
- 弹层态下，当前视频由 modal_id + active video + visible video-info 共同确认
- vid 在弹层态只做兜底，不允许覆盖当前激活视频
- 采集、下载、评论、数据面板都消费同一份 VideoContext

#### Douyin 数据源（三源融合）

| 数据 | 来源 | 说明 |
|---|---|---|
| 视频详情 | API 拦截 + render data + DOM | 三源互为 fallback |
| API 拦截 | injected/douyinApiCapture.js | fetch/XHR 拦截 → `window.__lgboom_dy_video_data` |
| 评论 | API 调用 + 分页 | 不走 DOM |
| 博主 | render data + API | 多源采集 |

### 5.3 Injected Scripts (Main World) — src/injected/

| 文件 | 注入目标 | 职责 |
|---|---|---|
| noteMap.js | XHS 页面 | 从 `__INITIAL_STATE__` 提取笔记数据 |
| user.js | XHS 页面 | 从 `__INITIAL_STATE__` 提取用户数据 |
| douyinApiCapture.js | Douyin 页面 | 拦截 fetch/XHR，缓存视频 API 响应；提供页面上下文下载能力 |

---

## 6. Content Script 生态

| 文件 | 职责 |
|---|---|
| **index.js** | 入口：平台检测 (xhs vs douyin)、适配器加载、消息路由、受控任务创建 |
| contentRouter.js / contentPlatformRegistry.js | 域名 → 平台入口分发 |
| contentDataRuntime.js | 延迟加载数据采集函数、Dashboard bridge、媒体下载服务 |
| dashboardBridge.js | Dashboard iframe 通信桥接 |
| messageHandlers.js / messageHandlers/\* | Popup/Background 消息分发门面；数据、采集、工作台控制、媒体下载按职责拆分 |
| remoteControlRegistry.js | 远程任务暂停 / 继续 / 停止控制登记，供采集与工作台控制共享 |
| xhsPageController.js | XHS 页面控制器 + 批量任务管理 |
| douyinRuntime.js | Douyin 运行时加载入口 |
| douyinRuntimeModule.js | Douyin 适配器 + 单条/批量/评论能力 |
| douyinBatchMessageHandlers.js | Douyin 批量操作消息处理 |
| noteMediaDownload.js | 笔记媒体下载 |
| mediaDownloadUtils.js | 媒体下载工具（高清候选 URL、重试） |
| commentTaskController.js | 评论采集任务控制器 |
| commentImageTask.js | 评论图片下载任务 |
| uiInjector.js | 页面 UI 注入（浮动控制面板等） |

---

## 7. 数据层 (IndexedDB)

### 7.1 数据库: LingganBoomDB (Dexie, 13 次版本迁移)

| 表 | 主键 | 关键索引 | 说明 |
|---|---|---|---|
| **notes** | noteId | platform, contentId, collectionRunId, authorId | 笔记/视频，含互动指标、媒体状态、AI 就绪字段 |
| **comments** | auto-inc | commentEntityId, noteId, platform, collectionRunId | 层级评论（parent/root），支持嵌套 |
| **authors** | authorEntityId | platform, handle, redId, douyinId | 博主资料，跨平台统一 ID |
| **collectionRuns** | collectionRunId | externalTaskId, executorInstanceId, status | 任务执行记录，心跳追踪，工作台绑定 |
| **mediaAssets** | auto-inc | collectionRunId, platformContentId, downloadStatus | 媒体文件追踪（下载状态、质量、角色） |
| **workbenchOutbox** | auto-inc | status, collectionRunId, idempotencyKey | 离线同步队列，幂等性保证 |

### 7.2 Store 模块

| Store | 文件 | 核心操作 |
|---|---|---|
| noteStore | noteStore.js | upsert, bulkUpsert, search, filterByType |
| commentStore | commentStore.js | upsert, getByNoteId, 去重 (commentId + noteId + platform) |
| authorStore | authorStore.js | bulkUpsert, search (多字段), 统一 handle |
| mediaAssetStore | mediaAssetStore.js | 下载状态追踪，质量级别管理 |
| collectionRunStore | collectionRunStore.js | 生命周期管理，外部任务绑定，心跳更新 |
| workbenchOutboxStore | workbenchOutboxStore.js | 入队/确认/重试/终态，指数退避 (1s→2s→5s→15s→60s) |

### 7.3 数据标准化 — recordNormalization.js

```
原始数据 → normalizeNoteRecord / CommentRecord / AuthorRecord → 标准化入库
```

- 平台推断：从 URL/ID 自动识别 xhs/douyin
- ID 统一：`xhs_` / `dy_` 前缀的 contentId
- 时间戳归一化
- 原始数据保留 (rawData 字段)

### 7.4 遗留数据维护 — legacyDataMaintenance.js

- 检测需要更新的旧记录
- 批量回填标准化字段
- 无损版本迁移

---

## 8. 工作台集成

### 8.1 集成架构

```
内容工作台 (Next.js)
  POST /api/plugin-authorizations/activate      → 激活插件授权
  POST /api/execution-tasks/manual-import    ← 插件手动同步：原始证据、内容关联和媒体账本登记
  POST /api/execution-stations/sync           → 工位轻量同步、信箱检查、对账与接单
  POST /api/collection-tasks/[id]/lease       ← 执行工位续租
  POST /api/collection-tasks/[id]/ingest      ← 增量上传
  POST /api/execution-stations/register       → 配对绑定执行工位
  GET  /api/collection-tasks/[id]/control-requests → 远程控制
  PATCH /api/collection-tasks/[id]            ← 状态更新
        ↕ HTTPS + Bearer Token (PLUGIN_API_TOKEN)
linggan-boom 插件
  pluginAuthorizationClient (授权码激活、设备资格、本地授权状态)
  taskPoller (对账恢复、租约认领、续租和提交；不再扫旧任务列表)
  executionStationClient (工位配对、身份、心跳)
  taskLeaseClient (本地租约持久化、续租)
  deltaOutbox (增量上传 + 离线重试；笔记记录入队前规范化平台媒体源字段)
  heartbeat (3s 心跳)
```

**关键事实**：

- 远程任务入口在 Background，不在 Popup。真正执行和落库发生在 Content Script 侧。
- 插件现在有两层身份：授权码决定“谁能用”，配对码决定“这台已授权浏览器绑定到哪个工位”。

### 8.2 同步模块 — src/sync/flywheelSync.js

| 函数 | 方向 | 说明 |
|---|---|---|
| syncToFlywheel() | 出 | 批量推送笔记+评论到工作台 |
| ingestCollectionTaskDelta() | 出 | 增量上传事件和记录 |
| fetchCollectionTaskControlRequests() | 入 | 获取远程控制指令 |
| patchCollectionTask() | 出 | 更新任务状态 |

> 执行工位不再通过 `GET /api/collection-tasks` 扫描待执行/运行中任务；恢复和接单统一走 `reconcile -> claim -> renew -> submit` 链路。

### 8.2.1 执行工位运行时 — src/workbench/runtime/

| 文件 | 职责 |
|---|---|
| pluginAuthorization.js | 激活授权码、持久化设备授权状态、统一授权门禁 |
| executionStationClient.js | 配对注册、保存工位身份、发送心跳 |
| executionStationRuntime.js | 监控工位能力清单、平台账号健康汇报 |
| taskLeaseClient.js | 任务租约认领、续租、本地持久化 |
| monitorTask.js | 监控任务策略翻译，生成 `monitorMeta`，把表层卡片转成雷达观察记录 |
| taskPoller.js | 已配对时走租约认领；未配对时不认领工作台远程任务 |

### 8.3 工作台协议 — src/workbench/protocol/

| 文件 | 职责 |
|---|---|
| **schema.js** | 协议版本 v1 定义，消息类型枚举，支持的任务类型列表 |
| **validator.js** | 信封验证（协议版本、任务类型、页面类型、载荷结构） |
| **deltaEnvelope.js** | 幂等键生成、事件/记录信封构建、批量上传封装 |

#### 支持的任务类型

| 任务类型 | 平台 | 说明 |
|---|---|---|
| xhs.batchNotes | XHS | 批量采集笔记 |
| xhs.batchComments | XHS | 批量采集评论 |
| xhs.collectAuthor | XHS | 采集博主信息 |
| xhs.authorNoteLinks | XHS | 发现博主主页历史笔记链接，供工作台拆分后续详情补采 |
| douyin.batchNotes | Douyin | 批量采集视频 |
| douyin.batchComments | Douyin | 批量采集评论 |
| douyin.collectAuthor | Douyin | 采集博主信息 |
| douyin.singleComments | Douyin | 单视频评论采集 |
| douyin.commentImageDownload | Douyin | 评论图片下载 |

### 8.4 工作台运行时 — src/workbench/runtime/

| 模块 | 文件 | 职责 |
|---|---|---|
| **任务轮询** | taskPoller.js | 30s 轮询 → 认领 → 能力检查 → 分发 → 状态追踪 → 过期恢复 (2h 超时) |
| **执行器身份** | executorIdentity.js | 持久化 UUID (plugin_* 前缀)，任务归属 |
| **能力检查** | capabilityCheck.js | 验证当前页面能否执行指定任务 |
| **能力报告** | capabilityReportBuilder.js | 自描述：平台、页面类型、能力枚举、就绪状态、推荐下一步 |
| **任务映射** | taskEnvelopeMapper.js | 协议信封 → 内部指令翻译 |
| **记录结构校验** | protocol/recordPayloadValidator.js | 校验 `note/comment/author/media` 最小可用结构，缺关键字段时输出健康告警 |
| **监控策略** | monitorTask.js | `author_baseline / author_patrol / keyword_patrol / detail_probe` → `surfaceOnly / monitorMode / monitorMeta` |
| **增量上报** | taskDeltaReporter.js | 包装 deltaOutbox，上报工作台事件/记录 |
| **增量发件箱** | deltaOutbox.js | 按任务批量、幂等去重、指数退避、确认/终态处理 |
| **进度事件** | progressEvent.js | 阶段推断 (CONTEXT_CHECK→DISCOVERING→COLLECTING→DOWNLOADING→PERSISTING→FINALIZING) |
| **错误映射** | errorMapper.js | 8 类：CONTEXT / AUTH / NETWORK / PLATFORM_BLOCK / STORAGE / DOWNLOAD / USER_CANCEL / INTERNAL |
| **结果打包** | resultPackager.js | 任务完成后汇总所有记录、构建结果摘要 |
| **结果摘要** | resultSummaryBuilder.js | 笔记/评论/博主/媒体数量统计 |
| **心跳** | heartbeat.js | 3s 周期心跳，防过期回收 (2h 超时) |
| **控制映射** | taskControlMapper.js | 工作台控制指令 → 内部操作 |
| **XHS 批量辅助** | xhsBatchRunHelper.js | XHS 批量任务的远程 Run 创建和进度更新 |

#### 监控路由

| 监控策略 | 插件路线 | 结果 |
|---|---|---|
| `author_baseline` | `collectAuthor` + 表层作品卡片 | 作者快照 `author_profile`，作品卡片 `author_surface` |
| `author_patrol` | `collectAuthor` + 少量表层作品卡片 | 同上，数量由工作台 `scanLimit` 控制 |
| `keyword_patrol` | `batchNotes` 表层模式 | 搜索结果卡片 `keyword_surface` |
| `detail_probe` | `batchNotes` 详情模式 | 详情记录 `detail_probe` |

原则：监控默认做轻量“看见”，不自动深采评论。只有 `detail_probe` 或用户手动深采才打开候选内容页补全详情。

### 8.5 任务执行流程

```
工作台 → claimTaskLease() → taskPoller.tick()
  → capabilityCheck() → dispatchTask()
  → collectionRunStore.createRun() → 平台采集器执行
  → 进度事件 → taskDeltaReporter.enqueueEvent()
  → deltaOutbox.flush() → ingestCollectionTaskDelta()
  → 工作台确认 → markAcked()
```

### 8.6 控制流

```
工作台 → fetchControlRequests() → taskControlMapper
  → 暂停/恢复/停止指令 → 执行状态变更 → enqueueEvent() → 确认
```

### 8.7 离线韧性

```
网络失败 → workbenchOutboxStore.enqueue() → markRetry() (指数退避)
→ 定期 flush → 成功 → markAcked() / 终态失败 → markTerminal()
```

---

## 9. UI 组件

### 9.1 Popup — src/popup/

| 区域 | 功能 |
|---|---|
| 平台检测 | 显示当前平台和页面类型 |
| 统计 | 笔记/评论/博主数量 |
| 操作卡片 | 单条采集、批量采集按钮 |
| 进度区 | 实时进度条 + 暂停/恢复/停止 |
| 工具区 | Dashboard、导出、维护 |
| 飞轮同步 | 工作台连接测试、配置、推送 |
| 设置浮层 | 批量采集参数配置 |

### 9.2 Dashboard — src/dashboard/

| 区域 | 功能 |
|---|---|
| 导航标签 | 笔记 / 评论 / 博主 |
| 工具栏 | 搜索、筛选、排序、导出 |
| 数据表格 | 动态列（按平台和数据类型） |
| 批量选择 | 多选 + 批量操作（导出/删除/同步） |
| 媒体预览 | 图片/视频预览弹窗 |

### 9.3 Content UI — src/shared/taskUi.js

| 组件 | 功能 |
|---|---|
| TaskBarShell | 浮动任务控制面板 |
| ProgressDisplay | 阶段徽章 + 进度条 + 状态文字 |
| ToastNotifications | 浮动通知 (info/success/warning/error) |

---

## 10. 共享工具

| 文件 | 关键函数/常量 | 用途 |
|---|---|---|
| constants.js | MSG, TASK_STATE, COLLECT_MODE, BATCH_CONFIG | 消息协议与配置常量 |
| messaging.js | sendToBackground, sendToTab, reportProgress, reportDone | 消息传递封装 |
| utils.js | parseCount, extractNoteId, csvEscape, generateCsv, downloadFile, getHighQualityImageCandidates | 通用工具函数 |
| collectorMetadata.js | 版本戳记、原始数据保留、证据收集 | 数据溯源 |
| taskUi.js | TaskBarShell, ProgressDisplay, Toast | 页面内 UI 组件 |

---

## 11. 构建产物

Webpack 5 多入口打包到 dist/：

| 入口 | 产物 | 说明 |
|---|---|---|
| content | content.js + content.css | 注入页面的脚本和样式 |
| background | background.js | Service Worker |
| popup | popup.js + popup.css + popup.html | 扩展弹窗 |
| dashboard | dashboard.js + dashboard.css + dashboard.html | 数据看板 |
| copy | injected/*.js, assets/*.png, manifest.json | 原样复制 |

---

## 12. 测试

110 个测试文件 (tests/*.test.mjs)：

| 覆盖域 | 测试文件 |
|---|---|
| **工作台协议** | capability-check, control-sync, delta-outbox, executor-identity, heartbeat, progress-error, record-delta-message, result-packager, task-poller |
| **工作台远程执行** | author-remote-run, douyin-remote-long-tail |
| **XHS 平台** | batch-record-delta, batch-shared, note-collector |
| **Dashboard** | dashboard-bridge-sync, dashboard-react-entry, dashboard-url-resolution |
| **抖音批量控制台** | douyin-batch-ui-routing |
| **批量辅助** | xhs-batch-run-helper |

---

## 13. 探查脚本

| 脚本 | 用途 |
|---|---|
| probe-batch-flow.js | 验证卡片选择器和打开模式 |
| probe-comment-media.js | 映射评论图片 URL |
| probe-video-streams.js | 记录视频码率和分辨率 |
| probe-douyin-collect.js | 抖音视频定位/IP/下载候选一键采证 |
| probe-douyin-video-fields.js | 抖音视频页字段重建探针 |
| probe-douyin-author-fields.js | 抖音博主页字段重建探针（防串号） |
| probe-douyin-root-cause.js | 抖音当前视频错位根因探针 |

---

## 14. 已知问题与优化空间

> 供 AI agent 审查时关注。

- [ ] 纯 JavaScript 为主；协议 / 运行态 / Adapter 边界已接入 `npm run check:contracts`，但 UI 和采集器内部尚未纳入
- [ ] XHS 评论采集走 DOM 解析，依赖页面 DOM 结构，小红书改版即失效（SELECTORS.md 应持续更新）
- [ ] Douyin 数据三源融合逻辑复杂，fallback 路径多，测试覆盖是否充分
- [ ] IndexedDB 单线程写入，大批量采集时可能卡顿
- [ ] 工作台轮询间隔 30s 固定，无自适应调整
- [ ] 错误映射 8 类但缺少平台限流 (rate limit) 专门分类
- [ ] 遗留数据维护 (legacyDataMaintenance.js) 随版本增加会持续膨胀
- [ ] antiDetect.js 反检测策略较基础（随机延迟），未覆盖指纹检测
- [ ] popup.js 和 dashboard.js 中存在与 background 重复的业务逻辑
- [ ] mediaDownloadUtils.js 中 getHighQualityImageCandidates() 硬编码了 URL 模板
- [ ] 平台路由和页面能力判断已走 contentRouter + PlatformAdapter；按钮动作和采集控制仍有平台分支，后续随瘦身继续收口

---

## 15. 变更日志

| 日期 | 变更 |
|---|---|
| 2026-04-14 | 基于代码库全量分析重写：补充工作台集成、运行时模块、数据层细节、已知问题 |
| 2026-04-02 | 初始版本：模块职责、消息协议、加载边界、外部依赖验证流程 |
