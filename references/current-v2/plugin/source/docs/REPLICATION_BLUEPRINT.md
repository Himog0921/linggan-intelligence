# 灵感爆爆爆插件 — 1:1 复刻蓝图

> 本文件是**自包含**的复刻执行手册。把它单独交给一个 AI agent，它应该能重建一个功能等价、契约兼容的 Chrome MV3 插件。
> **事实源永远是代码**。本蓝图所有常量、字段、协议都来自 `linggan-boom` 仓库 v2.0.52 的真实源码，不是描述性总结。
> 关联审查报告：[reviews/REVIEW_2026-06-25_FULL_AUDIT.md](reviews/REVIEW_2026-06-25_FULL_AUDIT.md)
> 最后校准日期：2026-06-25

---

## §0. 给执行 agent 的硬铁律（先读，再动手）

大模型 agent 写代码最大的失败模式是**幻觉**：凭印象编一个看起来合理的字段名/选择器/API 路径，跑不通后再让用户调试。本蓝图用以下规则把它锁死。

### R1. 凡是依赖外部系统的字段，必须先验证再写

外部系统 = 小红书页面 DOM、抖音页面 DOM、`window.__INITIAL_STATE__`、抖音/小红书接口字段、内容工作台 HTTP 接口。

- 写之前必须问自己：**"我写的这个选择器/字段名/接口路径，是本蓝图里**带验证日期**写明的，还是我猜的？"**
- 如果是猜的，**禁止写代码**，先回到 §8 查证；§8 没有的，必须用真实页面/接口调研一次，调研脚本一次拿齐所有信息。
- 调研产出要回写到本蓝图的 §8 和 `docs/SELECTORS.md`，标注验证日期。

### R2. 凡是本蓝图里"带值的常量"，必须照抄，不能改名

`MSG`、`TASK_STATE`、`REMOTE_TASK_TYPE`、`REMOTE_ERROR_CODE`、`WORKBENCH_TASK_EVENT_TYPE`、Dexie schema 字段名、`BATCH_CONFIG` 数值——这些是**跨进程契约**，改一个字符就跟内容工作台/旧数据/测试对不上。复刻时字段名、值、大小写必须 1:1。

### R3. 模块职责不能合并、不能拆散

§7 给出每个文件的职责。复刻时可以重新组织代码细节，但不能把"Background 做的事"挪到 Content，不能把"协议层"和"运行时层"混进同一个文件。架构边界是产品语义，不是代码风格。

### R4. 每个外部动作必须有可读错误 + 忙碌态 + 确认态

- 危险动作（停止批量、删除账号、删除单条、批量删除、清空数据）必须二次确认。
- 主按钮点击后必须立即禁用 + 显示"执行中/获取中/导出中"。
- 错误必须翻译成非技术语言，禁止直接抛 `chrome.runtime.lastError.message` 给用户。

### R5. 复刻完成的判定标准（不是"能跑"，是"契约对齐"）

一个复刻版本算成功，必须同时满足：

1. `manifest.json` 的 permissions / host_permissions / content_scripts.matches / web_accessible_resources 与 §3.4 一致。
2. MSG 常量、Dexie schema、workbench 协议常量与本蓝图 §4/§5/§6 逐字一致。
3. 通过本蓝图 §10 的验收清单（含真实浏览器实机项）。
4. 与内容工作台联调时，任务能走完 `pending → dispatched → running → completed/failed` 且记录正确 ingest。

---

## §1. 产品定位

### 1.1 它是什么

**灵感爆爆爆** 是一个 Chrome MV3 扩展，在统一产品"内容工作台"中担任**浏览器执行端**。

- 内容工作台（独立仓库，Next.js）：主系统，负责任务下发、判断、沉淀、Topic 管理
- 本插件：执行端，负责在真实登录态下完成网页内采集、页面交互、结果回传

**它不是独立产品**。任何把它写成"带后端的全栈采集系统"的复刻都偏离了定位。

### 1.2 它做什么（插件负责）

- 网页内采集动作（小红书 + 抖音）
- 页面交互与上下文识别
- 批量任务执行（可暂停/继续/停止）
- 原始结果落本地 IndexedDB
- 向内容工作台回传进度、状态、记录增量
- 本地 Dashboard 查看/搜索/导出/二次下载

### 1.3 它不做什么（边界外）

- Topic 生命周期管理
- 洞察结果承载
- 人工评估与采纳决策
- 复盘沉淀与团队知识管理

这些归内容工作台。复刻时**不要**在本插件里实现它们。

### 1.4 双平台当前能力（v2.0.52）

| 平台 | 能力 | 状态 |
|---|---|---|
| 小红书 | 单篇笔记采集 | ✅ |
| 小红书 | 批量笔记（搜索页/博主页/收藏页，5/10/20/50）| ✅ |
| 小红书 | 单篇评论 + 子评论 | ✅ |
| 小红书 | 批量评论 | ✅ |
| 小红书 | 博主采集 | ✅ |
| 小红书 | 评论图片区下载 | ✅ |
| 小红书 | 媒体下载（封面/图片/Live/视频分项）| ✅ |
| 抖音 | 单条视频采集（弹层 + 原生分享触发）| ✅ |
| 抖音 | 单条视频下载 | ✅ |
| 抖音 | 单条博主采集 | ✅ |
| 抖音 | 单条评论（一级 + 二级）| ✅ |
| 抖音 | 批量视频（博主页/搜索页，API 驱动）| ✅ |
| 抖音 | 批量评论 | ✅ |
| 抖音 | 评论图片区下载 | ✅（待真实长时效回归）|
| 抖音 | 数据面板二次下载（旧直链失效刷新）| ✅ |
| 工作台 | Dashboard 勾选同步 notes/comments/authors | ✅ |
| 工作台 | 自动接单（pending 轮询 → lease → heartbeat → ingest）| ✅（待真实账号实机闭环）|
| 工作台 | 远程任务暂停/继续/停止控制 | ✅ |
| 工作台 | Web Push 唤醒 | ✅ |

---

## §2. 协作铁律（与现有项目一致）

来源：`CLAUDE.md`、`AGENTS.md`。复刻时保留这些约束。

1. **黑盒目标先调研**：依赖外部系统的功能，禁止凭经验猜选择器/字段名/接口路径。
2. **不转嫁技术债**：探查、调试、证据收集是 agent 的职责，不甩给用户。
3. **方案阶段识别外部依赖风险**：涉及 DOM/API/平台行为的方案，必须先写验证计划。
4. **实现前假设检查**：每个外部依赖项写代码前自问"已验证还是猜测"。
5. **非技术沟通**：对用户用结果导向语言，技术细节放括号。
6. **文档同步硬门禁**：代码改动 + 测试 + 用户反馈闭环后，必须同步更新 `progress.txt` 和对应权威文档，否则不算完成。

---

## §3. 技术栈与构建配置

### 3.1 运行环境（锁定）

| 项 | 值 |
|---|---|
| 浏览器 | Chrome（Manifest V3）|
| 目标站点 | `xiaohongshu.com`、`www.douyin.com`、对应媒体 CDN、内容工作台域名 |
| 语言 | JavaScript（ES Modules，`"type": "module"`）+ JSX |
| 前端框架 | React 19（`createRoot` + Hooks）|
| 样式 | 原生 CSS（无 Tailwind / CSS-in-JS）|
| 本地存储 | IndexedDB，经 Dexie.js v4 封装 |
| 数据库 schema | **v13**（见 §5）|
| 打包 | Webpack 5，4 入口 + 1 vendor chunk |
| 压缩下载 | JSZip（仅评论图片打包时动态加载）|
| 测试 | Node.js 内置 test runner（`node --test tests/*.test.mjs`）|

### 3.2 依赖版本（来自 `package.json`，以 lock 为准）

dependencies：

| 包 | 版本 |
|---|---|
| `dexie` | `^4.0.1` |
| `jszip` | `^3.10.1` |
| `react` | `^19.2.5` |
| `react-dom` | `^19.2.5` |

devDependencies：

| 包 | 版本 |
|---|---|
| `@babel/core` | `^7.29.0` |
| `@babel/preset-react` | `^7.28.5` |
| `babel-loader` | `^10.1.1` |
| `copy-webpack-plugin` | `^12.0.2` |
| `css-loader` | `^7.1.4` |
| `css-minimizer-webpack-plugin` | `^6.0.0` |
| `mini-css-extract-plugin` | `^2.8.0` |
| `typescript` | `^6.0.3`（仅用于 `check:contracts` 做协议层 checkJs，不编译）|
| `webpack` | `^5.90.0` |
| `webpack-cli` | `^5.1.4` |

### 3.3 npm scripts（来自 `package.json`）

| 命令 | 作用 |
|---|---|
| `npm run build` | `webpack --config webpack.config.cjs --mode production` |
| `npm run dev` | webpack watch 开发模式 |
| `npm run release:patch` / `minor` / `major` | 调 `scripts/version.sh` 改版本号 + 打 release zip |
| `npm run release:verify` | `node scripts/verify-release-package.mjs` 校验 release zip 与 dist 一致 |
| `npm run check:contracts` | `tsc -p jsconfig.contracts.json --noEmit`，对协议层 13 个文件做 checkJs 静态校验，并运行 XHS Collection/source contract 与 terminal mapper 测试 |
| `npm run test:douyin` | 跑指定子集测试（193+ 用例）|

> **注意**：项目**没有** `npm test` 全量脚本（tech-debt T2）。全量测试用 `node --test tests/*.test.mjs`。

### 3.4 manifest.json（必须 1:1 照抄）

```json
{
  "manifest_version": 3,
  "name": "灵感爆爆爆",
  "version": "2.0.52",
  "description": "多平台内容灵感采集工具箱（小红书 · 抖音）",
  "permissions": [
    "activeTab", "tabs", "storage", "cookies", "downloads",
    "alarms", "scripting",
    "declarativeNetRequest", "declarativeNetRequestWithHostAccess",
    "notifications"
  ],
  "host_permissions": [
    "http://localhost/*",
    "https://lingganboom.fun/*",
    "https://xiaohongshu.com/*",
    "https://*.xiaohongshu.com/*",
    "https://www.xiaohongshu.com/*",
    "https://ci.xiaohongshu.com/*",
    "https://*.xhscdn.com/*",
    "https://www.douyin.com/*",
    "https://*.byteimg.com/*",
    "https://*.douyinpic.com/*",
    "https://*.douyinstatic.com/*",
    "https://*.amemv.com/*",
    "https://*.douyinvod.com/*",
    "https://*.bytevcloudcdn.com/*"
  ],
  "background": { "service_worker": "background.js" },
  "content_scripts": [
    {
      "matches": ["https://xiaohongshu.com/*", "https://*.xiaohongshu.com/*"],
      "js": ["vendor.js", "content.js"],
      "css": ["content.css"],
      "run_at": "document_end"
    },
    {
      "matches": ["https://www.douyin.com/*"],
      "js": ["vendor.js", "content.js"],
      "css": ["content.css"],
      "run_at": "document_end"
    }
  ],
  "action": {
    "default_popup": "popup.html",
    "default_icon": { "16": "icon-16.png", "48": "icon-48.png", "128": "icon-128.png" }
  },
  "icons": { "16": "icon-16.png", "48": "icon-48.png", "128": "icon-128.png" },
  "web_accessible_resources": [
    {
      "resources": [
        "dashboard.html", "dashboard.js", "dashboard.css", "vendor.js", "content.css",
        "lgboom-logo.svg", "lgboom-banner.svg", "lgboom-inject-logo.svg",
        "injected/noteMap.js", "injected/user.js", "injected/xhsApiCapture.js"
      ],
      "matches": ["https://xiaohongshu.com/*", "https://*.xiaohongshu.com/*"]
    },
    {
      "resources": [
        "dashboard.html", "dashboard.js", "dashboard.css", "vendor.js", "content.css",
        "lgboom-logo.svg", "lgboom-banner.svg", "lgboom-inject-logo.svg",
        "injected/douyinApiCapture.js"
      ],
      "matches": ["https://www.douyin.com/*"]
    }
  ]
}
```

**关键事实**（旧文档写错过）：

- **没有 `debugger` 权限**。`chrome.debugger` 在源码中零引用。关闭弹窗/派发 Esc 走 `chrome.scripting.executeScript` 注入键盘事件（见 `MSG.DISPATCH_ESC` 实现）。
- `cookies` 真在用（`chrome.cookies.getAll/set/remove`，账号管理 + 远程任务账号注入）。
- `notifications` 已申请权限（评估使用面，见审查报告 R2）。
- host_permissions 注意 `http://localhost/*` 没有端口限定；`lingganboom.fun` 是工作台正式站。

### 3.5 webpack 入口与产物

| 入口 | 源 | 产物 |
|---|---|---|
| `content` | `src/content/index.js` | `content.js` + `content.css` |
| `background` | `src/background/index.js` | `background.js`（不进 vendor chunk）|
| `popup` | `src/popup/index.jsx` | `popup.js` |
| `dashboard` | `src/dashboard/index.jsx` | `dashboard.js` |
| `vendor`（splitChunks 自动）| react/react-dom/scheduler | `vendor.js`，被 content/popup/dashboard 共享，**background 不共享** |

CopyPlugin 原样复制：`manifest.json`、`popup.html`、`dashboard.html`、`popup.css`、`dashboard.css`、`src/themes/ac-ui/popup.css`、`src/injected/`、`src/assets/`（忽略 `.gitkeep`）。

**加载边界铁律**：

- Content script **不能拆异步 chunk**。Chrome 内容脚本运行空间下异步 chunk 加载会失败。`contentDataRuntime`、`douyinRuntime` 必须用 `webpackMode: "eager"` 静态加载。当前 `content.js` ≈ 667 KiB，是已知妥协（tech-debt T5/R12）。
- JSZip 仅评论图片打包时动态加载。
- `devtool: 'cheap-module-source-map'`（生产也开，复刻时建议改 `source-map` 独立文件 + 生产 drop_console）。

### 3.6 标准命令序列（复刻完成后的验证）

```bash
npm install
npm run check:contracts   # 协议层 checkJs 必须通过
npm run build             # 产出 dist/
node --test tests/*.test.mjs   # 全量测试
```

加载 `dist/` 到 `chrome://extensions`（开发者模式）做实机验收。

---

## §4. 内部消息协议（MSG 常量全表）

> 事实源：`src/shared/constants.js`。值是字符串，跨进程传输时按值匹配，**禁止改名**。

### 4.1 MSG（消息 action）

```js
// Content Script 操作
COLLECT_SINGLE_NOTE           = 'collectSingleNote'
COLLECT_SINGLE_COMMENT        = 'collectSingleComment'
DOWNLOAD_CURRENT_COMMENT_IMAGES = 'downloadCurrentCommentImages'
COLLECT_AUTHOR                = 'collectAuthor'
DISCOVER_AUTHOR_NOTE_LINKS    = 'discoverAuthorNoteLinks'

// 批量操作
START_BATCH_NOTES             = 'startBatchNotes'
STOP_BATCH_NOTES              = 'stopBatchNotes'
PAUSE_BATCH_NOTES             = 'pauseBatchNotes'
RESUME_BATCH_NOTES            = 'resumeBatchNotes'
START_BATCH_COMMENTS          = 'startBatchComments'
STOP_BATCH_COMMENTS           = 'stopBatchComments'
PAUSE_BATCH_COMMENTS          = 'pauseBatchComments'
RESUME_BATCH_COMMENTS         = 'resumeBatchComments'

// Background 操作
BLOCK_MEDIA                   = 'blockMedia'
UNBLOCK_MEDIA                 = 'unblockMedia'
DISPATCH_ESC                  = 'dispatchEscapeViaDebugger'   // 名字保留历史，实际走 scripting
DOWNLOAD_MEDIA_FILE           = 'downloadMediaFile'
FETCH_BINARY_AS_DATA_URL      = 'fetchBinaryAsDataUrl'
RELEASE_EXECUTION_ACCOUNT_LOCK = 'releaseExecutionAccountLock'

// 数据操作
GET_STATS                     = 'getStats'
EXPORT_CSV                    = 'exportCsv'
EXPORT_JSON                   = 'exportJson'
RUN_DATA_MAINTENANCE          = 'runDataMaintenance'
TOGGLE_DASHBOARD              = 'toggleDashboard'
GET_ALL_NOTES                 = 'getAllNotes'
GET_ALL_COMMENTS              = 'getAllComments'
GET_ALL_AUTHORS               = 'getAllAuthors'
GET_PAGE_CONTEXT              = 'getPageContext'
DELETE_NOTE                   = 'deleteNote'
DELETE_COMMENT                = 'deleteComment'
DELETE_AUTHOR                 = 'deleteAuthor'
CLEAR_ALL_NOTES               = 'clearAllNotes'
CLEAR_ALL_COMMENTS            = 'clearAllComments'
CLEAR_ALL_AUTHORS             = 'clearAllAuthors'
DOWNLOAD_NOTE_MEDIA           = 'downloadNoteMedia'

// 飞轮同步
TEST_FLYWHEEL_CONNECTION      = 'testFlywheelConnection'
GET_FLYWHEEL_CONFIG           = 'getFlywheelConfig'
SAVE_FLYWHEEL_CONFIG          = 'saveFlywheelConfig'

// 工作台接入
WORKBENCH_CAPABILITY_CHECK    = 'workbenchCapabilityCheck'
WORKBENCH_DISPATCH_TASK       = 'workbenchDispatchTask'
WORKBENCH_TASK_CONTROL        = 'workbenchTaskControl'
WORKBENCH_GET_RESULT_PACKAGE  = 'workbenchGetResultPackage'
WORKBENCH_LOCAL_CONTROL_EVENT = 'workbenchLocalControlEvent'
WORKBENCH_RECORD_DELTA        = 'workbenchRecordDelta'
WORKBENCH_DELTA_FLUSH         = 'workbenchDeltaFlush'
SYNC_TO_WORKBENCH             = 'syncToWorkbench'
AUTHORIZE_PLUGIN_ACCESS       = 'authorizePluginAccess'
REQUEST_PLUGIN_AUTHORIZATION  = 'requestPluginAuthorization'
CLAIM_PLUGIN_AUTHORIZATION_REQUEST = 'claimPluginAuthorizationRequest'
CLEAR_PLUGIN_AUTHORIZATION    = 'clearPluginAuthorization'
GET_EXECUTION_STATION_STATUS  = 'getExecutionStationStatus'
REGISTER_EXECUTION_STATION    = 'registerExecutionStation'
SEND_EXECUTION_STATION_HEARTBEAT = 'sendExecutionStationHeartbeat'

// 账号管理
GET_ACCOUNTS                  = 'getAccounts'
ADD_ACCOUNT                   = 'addAccount'
REMOVE_ACCOUNT                = 'removeAccount'
UPDATE_ACCOUNT                = 'updateAccount'

// Cookie 管理
GET_PLATFORM_COOKIES          = 'getPlatformCookies'
GET_STORED_PLATFORM_COOKIES   = 'getStoredPlatformCookies'

// 进度与状态
PROGRESS                      = 'progress'
COLLECT_DONE                  = 'collectDone'
ERROR                         = 'error'
```

### 4.2 传输路径

| 路径 | 方式 |
|---|---|
| Popup → Background | `chrome.runtime.sendMessage` |
| Background → Content | `chrome.tabs.sendMessage`（封装在 `sendToTab`，支持断线自动重注入）|
| Content → Background | `chrome.runtime.sendMessage` |
| Content ↔ Injected Script（主世界）| `window.postMessage` / `CustomEvent` |
| Content ↔ Dashboard（iframe 子框架）| `postMessage`，带一次性 `nonce` |
| Background ↔ 内容工作台 | HTTP（轮询/PATCH/ingest）|
| 内容工作台 → Background | Chrome Web Push（仅唤醒，不直接派任务）|

**Dashboard nonce 规则**：Content 以 iframe 打开 Dashboard 时生成随机 nonce，写入 `chrome.storage.session` **并** 放入 `dashboard.html?nonce=...`。Dashboard 优先读 URL nonce 发 postMessage，避免首次加载 storage 未同步被拒。iframe URL 不暴露 nonce 给外层页面（R20 修复）。Content 校验 `event.source` 必须是真实 dashboard iframe。

### 4.3 消息封装当前状态（重要）

> tech-debt T6：仓内 envelope **未统一**。

- 工作台执行链路：`{ success, data, error }` + `eventId / attemptId / leaseId / eventSeq`
- 其他链路：同时存在 `{ success, data }`、裸数组、裸对象等返回格式
- 消费方必须按具体 action 容错解析

复刻时**不要**强行统一所有 action 的 envelope——会破坏与现有内容工作台的契约。统一是长期目标，不是复刻目标。

### 4.4 任务状态机

```
idle → running ⇄ paused → stopping → done / error
```

`TASK_STATE`：

```js
IDLE='idle'  RUNNING='running'  PAUSED='paused'
STOPPING='stopping'  DONE='done'  ERROR='error'
```

适用范围：小红书批量笔记、小红书批量评论、抖音批量视频、抖音批量评论、评论图片区下载、远程派单后的本地 `collectionRun`。

### 4.5 其他共享常量

```js
COLLECT_MODE = { SEARCH:'search', PROFILE:'profile', DETAIL:'detail', FAVORITE:'favorite' }
COMMENT_DEPTH_MODE = { TWO_LEVEL:'twoLevel', ALL_REPLIES:'allReplies' }
PAGE_TYPE = { NOTE_DETAIL:'noteDetail', SEARCH:'search', PROFILE:'profile', EXPLORE:'explore', UNKNOWN:'unknown' }

BATCH_CONFIG = {
  maxPerSession: 50,
  intervalMin: 1200, intervalMax: 2800,
  iframeTimeout: 15000,
  scrollStepMin: 100, scrollStepMax: 300,
  scrollIntervalMin: 200, scrollIntervalMax: 500,
  maxScrollRetries: 10,
  maxSubComments: 200,
}
```

---

## §5. 本地数据层（Dexie schema v13 全表）

> 事实源：`src/db/index.js`。数据库名 `LingganBoomDB`。**字段名必须 1:1**。

### 5.1 表与索引（v13）

```
notes: noteId, contentId, platformContentId, platform, collectionRunId, url, title, type,
       authorId, authorEntityId, authorName, likes, collects, comments, releaseDate,
       publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus,
       dataSource, triggerSource, shareShortUrl, createdAt, syncStatus

comments: ++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text,
          author, authorId, profileUrl, location, ipLocation, likes, parentCommentId,
          rootCommentId, level, replyToCommentId, replyToUserName, publishedAt,
          collectedAt, sortMode, collectionRunId, createdAt, syncStatus

authors: userId, authorEntityId, platformAuthorId, platform, collectionRunId, handle,
         secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation,
         gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus

collectionRuns: collectionRunId, externalTaskId, externalTaskType, executorInstanceId,
                protocolVersion, platform, taskType, pageType, triggerSource, status,
                resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt

mediaAssets: assetId, contentId, collectionRunId, assetType, role, quality,
             downloadStatus, lastResolvedAt, createdAt

workbenchOutbox: id, taskId, pluginRunId, &idempotencyKey, kind, status, nextAttemptAt,
                 createdAt, [status+nextAttemptAt+createdAt]

accounts: accountId, name, status, platform, lastUsedAt, createdAt
```

索引语法说明：逗号分隔的是索引字段，第一个是主键；`++id` = 自增主键；`&idempotencyKey` = 唯一索引；`[a+b+c]` = 复合索引。

> 字段语义、类型、完整业务含义见 `docs/technical/DATA_MODEL.md`（已与代码一致）。完整字段说明表很长，复刻时必须照那份文档实现每个字段的写入逻辑。

### 5.2 关键 schema 演进点（迁移历史）

不能跳过的版本演进（每版都要实现 `db.version(N).stores(...)`，Dexie 自动升级）：

| 版本 | 关键变更 |
|---|---|
| v5 | 全表加 `platform` 索引；存量 undefined 视为 'xhs' |
| v6 | AI-ready 基线：加 `contentId/platformContentId/authorEntityId`、评论树字段、`collectionRuns`、`mediaAssets` |
| v8 | `collectionRuns` 加远程任务映射（`externalTaskId/executorInstanceId/...`）|
| v9 | 新增 `workbenchOutbox` |
| v10 | `workbenchOutbox` 加 `[status+nextAttemptAt+createdAt]` 复合索引（**避免 listPending 全表扫描**，P0 性能修复）|
| v11 | 新增 `accounts`（账号池）|
| v12 | `workbenchOutbox.idempotencyKey` 改唯一索引 + 升级时清重复行（**消除 enqueue 竞态**）|
| v13 | `notes/authors` 加 `collectionRunId` 索引（**避免按任务打包结果时全表扫描**，R21 修复）|

### 5.3 chrome.storage.local 键

| 键 | 用途 |
|---|---|
| `platformCookies` | 双平台 Cookie 缓存，结构 `{ xhs: {cookies, cookieString, count, capturedAt}, douyin: {...} }` |
| `workbenchPluginAuthorization` | 授权状态：`deviceId/authorizationId/authorizationToken/status/teamName/memberName/seatName/expiresAt` |
| `workbenchExecutionStation` | 工位绑定：`stationId/stationToken/stationKey/displayName/role` |
| `workbenchActiveTaskLease` | 本地任务租约快照 |
| flywheel 配置 | serverUrl / apiToken / enabled 等 |

### 5.4 数据约束（去重规则）

- 笔记按 `noteId` 去重（覆盖更新，保留原 `createdAt`）
- 评论按 `commentId + noteId + platform` 去重
- 博主按 `userId` 去重
- 批量任务按 `collectionRunId` 去重
- 媒体资产按 `assetId` 去重
- outbox 按唯一 `idempotencyKey` 去重
- 批量任务恢复通过 `resumeCheckpoint.targetIds + nextIndex`，不重复处理已完成目标

---

## §6. 工作台协议层（与内容工作台契约）

> 事实源：`src/workbench/protocol/schema.js` + `docs/technical/MESSAGE_PROTOCOL.md` §2.7-2.8 + `PLUGIN_AUTHORIZATION_PROTOCOL.md`。

### 6.1 协议版本与消息类型

```js
WORKBENCH_PROTOCOL_VERSION = 'v1'

WORKBENCH_MESSAGE_TYPE = {
  EXECUTOR_HELLO: 'executor.hello',
  CAPABILITY_CHECK: 'capability.check',
  CAPABILITY_REPORT: 'capability.report',
  TASK_ENVELOPE: 'task.envelope',
  TASK_ACCEPTED: 'task.accepted',
  TASK_REJECTED: 'task.rejected',
  TASK_PROGRESS: 'task.progress',
  TASK_RESULT: 'task.result',
  TASK_FAILED: 'task.failed',
  TASK_CONTROL: 'task.control',
  TASK_RECORD: 'task.record',
}
```

### 6.2 支持的远程任务类型（必须全部实现）

```js
REMOTE_TASK_TYPE = {
  XHS_BATCH_NOTES:           'xhs.batchNotes',
  XHS_BATCH_COMMENTS:        'xhs.batchComments',
  XHS_COLLECT_AUTHOR:        'xhs.collectAuthor',
  XHS_AUTHOR_NOTE_LINKS:     'xhs.authorNoteLinks',
  DOUYIN_BATCH_NOTES:        'douyin.batchNotes',
  DOUYIN_BATCH_COMMENTS:     'douyin.batchComments',
  DOUYIN_COLLECT_AUTHOR:     'douyin.collectAuthor',
  DOUYIN_SINGLE_COMMENTS:    'douyin.singleComments',
  DOUYIN_COMMENT_IMAGE_DOWNLOAD: 'douyin.commentImageDownload',
}
```

每个任务类型在 `SUPPORTED_REMOTE_TASKS` 里定义：`platform` / `targetPageTypes` / `dispatchTarget`（background 或 content）/ `startAction`（映射到哪个 MSG）/ `controlActions`（pause/resume/stop/delete 各映射到什么）/ `capabilityKey`。完整映射见 `src/workbench/protocol/schema.js`，复刻时照抄。

**关键映射示例**：

- `xhs.batchNotes` → `dispatchTarget: background`，`startAction: START_BATCH_NOTES`，pause/resume/stop/delete 都映射到对应 `*_BATCH_NOTES`
- `xhs.collectAuthor` → `dispatchTarget: content`，控制走 `WORKBENCH_TASK_CONTROL`
- `delete` 控制动作统一映射为本地 `stop`（软删除由工作台负责）

### 6.3 监控策略与记录模式

```js
MONITOR_TASK_STRATEGY = {
  AUTHOR_BASELINE: 'author_baseline',
  AUTHOR_PATROL:   'author_patrol',
  KEYWORD_PATROL:  'keyword_patrol',
  DETAIL_PROBE:    'detail_probe',
  DEEP_COLLECT:    'deep_collect',
}

MONITOR_RECORD_MODE = {
  AUTHOR_PROFILE:   'author_profile',
  AUTHOR_SURFACE:   'author_surface',
  KEYWORD_SURFACE:  'keyword_surface',
  DETAIL_PROBE:     'detail_probe',
}
```

策略 → 执行方式：

| 策略 | 插件执行 | 记录模式 |
|---|---|---|
| `author_baseline` | 作者快照 + 主页当前顺位前 50 篇（**顺位优先，不走点赞 Top N**）| `author_profile` + `author_surface` |
| `author_patrol` | 作者快照 + 少量主页表层卡片 | `author_profile` + `author_surface` |
| `keyword_patrol` | 搜索页表层结果卡片 | `keyword_surface` |
| `detail_probe` | 打开指定候选补全正文/评论数/发布时间 | `detail_probe` |
| `deep_collect` | 保留为人工深采入口 | `detail_probe` |

**监控记录必须保留四字段**（工作台用来把 `CollectionTaskRecord` 转 `MonitorObservation/RadarSignal`）：`monitorMode / monitorId / taskStrategy / monitorMeta`。普通手动采集**不**携带这四字段。

### 6.4 任务事件类型（全表，按顺序）

```
task.claimed          task.page_opened       task.execution_started
task.first_record_seen  task.page_open_failed  task.login_required
task.platform_restricted  task.started         task.running
task.heartbeat        task.progress          task.partial_result
task.control_requested  task.control_applied  task.control_failed
task.paused           task.resumed           task.stopping
task.stopped          task.completed         task.succeeded
task.released         task.failed            task.deleted
task.capability_mismatch
```

### 6.5 记录类型与最小结构校验

```
WORKBENCH_RECORD_TYPE = { NOTE:'note', COMMENT:'comment', AUTHOR:'author', MEDIA:'media' }
```

最小结构（不满足的不进 outbox，任务转失败 + `observability` 上报）：

| 类型 | 最小要求 |
|---|---|
| `note` | 有稳定内容 ID 或 URL，且有可见正文/标题/媒体 |
| `comment` | 有评论 ID、父级内容 ID、评论文本 |
| `author` | 有稳定作者 ID 或主页 URL |
| `media` | 有资产 ID、URL 或本地路径 |

### 6.6 错误码全表（REMOTE_ERROR_CODE）

```js
UNSUPPORTED_TASK_TYPE       PAGE_TYPE_MISMATCH       PAGE_PERMISSION_DENIED
PLATFORM_MISMATCH          PAGE_TARGET_MISMATCH     ERROR_PAGE
PAGE_CONTEXT_UNAVAILABLE   PLATFORM_SECURITY_CHALLENGE  PLATFORM_BLOCKED
SEARCH_LIST_UNSTABLE       LOGIN_REQUIRED           LOGIN_EXPIRED
CONTENT_NOT_FOUND          HEARTBEAT_ONLY_STALL     EXECUTOR_BUSY
STORAGE_WRITE_FAILED       DOWNLOAD_FAILED          TASK_STOPPED_BY_USER
UNEXPECTED_INTERNAL_ERROR
```

错误分类（`REMOTE_ERROR_CATEGORY`）：`context / auth / network / platform_block / rate_limit / storage / download / user_cancel / internal`。

**失败归类规则**（影响是否重试）：

- **明确不可执行 → `failed`，不重排队**：`content_not_found`、`error_page`、`page_permission_denied`
- **可恢复执行环境问题 → `released`，稍后重试**：例如 `page_context_unavailable`
- **能力不匹配 → `task.capability_mismatch`**：payload 至少含 `taskType/reasonCode/reasonMessage/status`，有页面报告时附带 `reportUrl/reportMode/reportPageType/readinessReady/readinessReasonCode/readinessReasonMessage/capabilityTaskTypes`

### 6.7 控制动作

```js
REMOTE_TASK_CONTROL_ACTION = { PAUSE:'pause', RESUME:'resume', STOP:'stop', DELETE:'delete' }
```

`delete` 对插件执行端 = 本地 `stop`。插件应用控制后通过 ingest 写 `task.control_applied`，失败写 `task.control_failed`。插件本地控制按钮也通过 `WORKBENCH_LOCAL_CONTROL_EVENT` 写回同一任务事件流。

### 6.8 delta envelope（增量上传载荷）

```json
{
  "protocolVersion": "v1",
  "taskId": "task_123",
  "pluginRunId": "run_123",
  "executorInstanceId": "plugin_profile_uuid",
  "attemptId": "attempt_123",
  "leaseToken": "lease_token",
  "leaseEpoch": 3,
  "pageFingerprint": { "platform": "xhs", "url": "..." },
  "cursor": "local-outbox-seq-42",
  "events": [],
  "records": [],
  "snapshot": {}
}
```

**幂等键规则**（必须照抄）：

```
event:  {taskId}:{pluginRunId}:event:{eventType}:{controlRequestId || sequence}
record: {taskId}:{pluginRunId}:record:{recordType}:{externalRecordId || sequence}
```

### 6.9 工作台 HTTP 端点

| 方法 | 路径 | 用途 |
|---|---|---|
| POST | `/api/plugin-authorizations/activate` | 激活授权码（不需要 Bearer）|
| POST | `/api/execution-stations/register` | 配对绑定执行工位 |
| POST | `/api/execution-stations/sync` | 工位轻量同步、信箱检查、对账、接单 |
| POST | `/api/collection-tasks/:taskId/lease` | 任务租约认领/续租 |
| POST | `/api/collection-tasks/:taskId/ingest` | 增量上传事件/记录 |
| GET  | `/api/collection-tasks/:taskId/control-requests` | 拉取远程控制指令 |
| GET  | `/api/collection-tasks/:taskId/control-requests?executorInstanceId=<id>&after=<cursor>` | 带游标拉取 |
| PATCH| `/api/collection-tasks/:taskId` | 状态更新 |
| POST | `/api/execution-tasks/manual-import` | 手动同步笔记、评论、博主；建立只导入不派单的可追踪任务，并在同一媒体账本路径登记封面、图片和视频来源 |
| POST | `/api/plugin-data-workspace` | 绑定当前登录使用者账号（普通同步前必走）|
| GET  | `/api/push/vapid-public-key` | Web Push 公钥 |
| POST | `/api/execution-stations/push-subscription` | 注册 push 订阅 |

所有请求（除 activate）携带 `Authorization: Bearer <authorizationToken>`，普通同步还额外带 `X-Plugin-Data-Token`。

**工位同步响应示例**：

```json
{
  "mode": "mailbox_idle",
  "heartbeat": { "success": true },
  "mailbox": { "version": 108, "pendingCount": 0 },
  "reconcile": { "action": "idle", "serverLease": null },
  "claim": null,
  "nextSyncAfterMs": 60000
}
```

- `mode=mailbox_idle` / `mode=full_sync`：旧响应兼容解析口径；当前 V1.1 服务端优先返回 `mailboxVersions`、`reservations[]`、`operationResults` 与 `nextSync`
- 插件不再发送 `claimMode`、`mailboxVersion`、`localLease` 等旧 body 字段；任务领取通过 `capacity → reservations[] → start_job` 完成
- 运行中续租通过 `/api/execution-stations/sync` 的 `progress_update` operation 完成，不再调用旧 lease 口

**Web Push**：

- 推送消息类型：`collection_task_available` / `collection_task_control`
- 收到推送**只**把接单检查提前执行，**不**绕过 `/sync` reservation、`start_job`、control、ingest 安全链路
- VAPID 未配置/不支持/订阅过期/发送失败 → 回退低频对账

### 6.10 授权两层身份（铁律）

| 层 | 作用 | 谁生成 |
|---|---|---|
| **授权码** `authorizationCode` | 决定谁有资格用插件 | 内容工作台 → 设置 → 插件授权 |
| **配对码** `pairingCode` | 决定已授权浏览器绑定到哪个工位 | 内容工作台 → 工位管理 |

门禁规则：未授权时，Popup/页内按钮/Background 接单**全部拒绝**启动任务。普通同步的数据归属于**当前登录的使用者账号**，不是授权码创建者；未绑定使用者时禁止同步。

### 6.11 数据地基出站字段（笔记入 outbox 前补齐）

笔记记录进 outbox 前，必须补齐这些字段供工作台做低粉爆文/聚类/Claude 打标：

```
standardContentCode  standardAuthorCode  keywords
authorFans  authorFansCollectedAt
mediaUnderstanding
sourceRun（摘要）  dataFoundation（摘要）
```

### 6.12 小红书附带评论规则

`xhs.batchNotes` 且 payload 带 `includeComments=true` / `collectComments=true` 时：

- 必须采当前笔记正文/指标/公开评论数后，继续采 **20 条以内**评论
- **不能**改走只采正文的单篇路径
- 公开评论数 < 20 时，预期评论数 = 公开评论数
- 公开评论数 = 0 时，回传 `publicCommentCount: 0` 且不标失败
- 公开评论数 > 0 但没带回评论正文 → 带 `error: "comments_empty_after_request"`
- 实际 < 预期 → 带 `error: "comments_under_expected"` + `expectedCommentCount`

---

## §7. 模块职责地图

> 复刻时每个文件的职责不能合并/拆散。文件名可以调整，职责边界不能。

### 7.1 Background（`src/background/`）

| 文件 | 职责 |
|---|---|
| `index.js`（约 2656 行）| Service Worker：消息路由总中枢、下载管理、媒体屏蔽（declarativeNetRequest）、工作台任务轮询/接单/派单/控制、风控切号（300017）、alarms 注册、Web Push 处理、cookie/账号管理、插件授权门禁 |
| `downloadService.js` | 媒体下载：候选 URL 顺序重试、抖音 CDN 单候选优化、文件名清洗、headers 构建、超时/完成等待 |
| `messageSecurity.js` | 消息来源安全校验 |

**alarms**（3 个）：

| alarm 名 | 周期 | 用途 |
|---|---|---|
| `WORKBENCH_TASK_POLL_ALARM` | `INITIAL_WORKBENCH_TASK_POLL_MINUTES` | 任务接单轮询 |
| `WORKBENCH_STATION_HEARTBEAT_ALARM` | 1 分钟 | 工位心跳 |
| `daily-quota-reset` | 60 分钟 | 检查日期变化，清零账号日配额 |

**已知问题**（R1）：alarms 在顶层/onStartup/onInstalled 重复 create，复刻时建议收口为单个 `registerAlarms()` 函数。

### 7.2 Content（`src/content/`）

| 文件 | 职责 |
|---|---|
| `index.js`（约 261 行）| 入口：平台路由、消息监听注册、Dashboard bridge、批量消息 handler 装配 |
| `contentRouter.js` | hostname → platform → init 分发 |
| `contentPlatformRegistry.js` | hostname 到平台的映射表 |
| `xhsPageController.js` | XHS 页面控制器 + 批量任务管理 |
| `douyinRuntime.js` / `douyinRuntimeModule.js` | 抖音运行时（**eager** 动态加载，不能拆 chunk）|
| `contentDataRuntime.js` / `contentDataRuntimeLoader.js` | 内容数据运行时（**eager**）|
| `messageListener.js` | `chrome.runtime.onMessage` 监听创建 |
| `messageHandlers.js` | 兼容门面（实际逻辑已拆分）|
| `messageHandlers/{data,collection,workbench,media}Handlers.js` | 按职责拆分的消息处理 |
| `dashboardBridge.js` | Dashboard iframe 通信桥 + nonce 校验 |
| `remoteControlRegistry.js` | 远程任务暂停/继续/停止控制登记 |
| `commentTaskController.js` | 评论采集任务控制器 |
| `commentImageTask.js` | 评论图片下载任务 |
| `noteMediaDownload.js` / `mediaDownloadUtils.js` | 笔记媒体下载 |
| `platformHostMatcher.js` | 平台 host 匹配 |
| `douyinBatchMessageHandlers.js` | 抖音批量消息处理 |

**死文件，复刻时不要实现**：`commentCollectTask.js`（零代码引用，是早期遗留；评论任务控制实际走 `commentTaskController.js` + `shared/managedTaskController.js`）。

### 7.3 平台层 - 小红书（`src/platforms/xhs/`）

| 文件 | 职责 |
|---|---|
| `index.js` | 统一导出 |
| `adapter.js` | 平台适配器：`detectPage` / `normalizeTarget` / `checkCapability`（返回 8 个能力布尔位）|
| `noteCollector.js` | 单篇笔记采集（`__INITIAL_STATE__` + noteMap.js）；列表发现 + 滚动加载（视觉排序）|
| `commentCollector.js` | 评论采集：API 捕获优先 + DOM 兜底；评论图片下载 |
| `commentApi.js` | 评论 API 桥接；`mapXhsCommentRecord` 评论记录映射 |
| `authorCollector.js` | 博主采集（`__INITIAL_STATE__.user` + DOM）|
| `batchController.js` | BatchNoteController / BatchCommentController |
| `batchCommentController.js` | 多笔记评论批量协调 |
| `batchShared.js` | 批量任务共享状态 |
| `pageDetector.js` | 页面类型识别（NOTE_DETAIL/SEARCH/PROFILE/EXPLORE）|
| `searchFilters.js` | 搜索筛选操作（排序/类型/时间）|
| `uiInjector.js` | UI 注入：浮动按钮、对话框、任务控制条、Toast |
| `antiDetect.js` | 反检测：随机延迟、拟人滚动、验证码监控、分级节流 |
| `selectorHealth.js` | 选择器健康检查 |

### 7.4 平台层 - 抖音（`src/platforms/douyin/`）

| 文件 | 职责 |
|---|---|
| `index.js`（约 774 行）| 抖音适配器主实现：SPA 路由监听、MutationObserver、原生分享捕获、API bridge 绑定 |
| `adapter.js` | 平台适配器（含安全验证检测）|
| `videoCollector.js` | 视频采集（三源融合：DOM + render data + API）|
| `videoApiData.js` | API 响应映射/缓存/fallback |
| `videoContext.js` | 当前活跃视频上下文识别 |
| `videoDom.js` | DOM 提取视频元数据 |
| `videoDownload.js` | 页面上下文 fetch 绕过 CDN 鉴权 |
| `commentCollector.js` | 评论 API 采集 + 评论图片批量下载 + ZIP |
| `commentApi.js` | 评论 API 封装，chunked JSON 解析 |
| `commentMedia.js` | 评论图片资产提取 |
| `commentTaskSupport.js` | 评论任务支持 |
| `authorCollector.js` | 博主采集（render data + API + DOM）|
| `batchController.js` | 个人页/搜索页批量视频和评论 |
| `batchDiscovery.js` | 目标发现 + Top-N 排序 |
| `searchCapture.js` | 搜索页 API 捕获 |
| `securityChallenge.js` | 安全验证检测 |
| `fetchWithTimeout.js` | 带超时 fetch |
| `taskbarRenderState.js` | 任务条渲染状态 |
| `selectorHealth.js` | 选择器健康检查 |
| `pageDetector.js` | 页面识别（VIDEO_DETAIL/NOTE_DETAIL/SEARCH/PROFILE）|
| `uiInjector.js` | 抖音专属 UI 注入 |
| `videoApiData.js` | API 数据 |

**抖音铁律 - 当前视频上下文**：

抖音博主页是弹层 + SPA 复用 DOM，URL/DOM/API 会错峰更新。必须遵守：

1. 先判页面状态：主页预览态 / 博主页弹层态 / 直接视频页
2. 弹层态下，当前视频由 `modal_id` + `[data-e2e="feed-active-video"]` + 可见 `[data-e2e="video-info"]` 共同确认
3. URL 参数 `vid` 在弹层态只做兜底，**不允许**覆盖激活视频
4. 采集、下载、评论、数据面板都消费同一份 VideoContext

### 7.5 注入脚本（`src/injected/`，主世界执行）

| 文件 | 注入目标 | 职责 |
|---|---|---|
| `noteMap.js` | XHS | 从 `window.__INITIAL_STATE__.note.noteDetailMap` 提取笔记数据，过滤幽灵 key（`/^[a-f0-9]{24}$/i`），postMessage `{type:'noteMap', data}` 回 content |
| `user.js` | XHS | 从 `__INITIAL_STATE__.user.userPageData` 提取博主数据，**必须做 `._rawValue` 拆包**（Vue ref）|
| `xhsApiCapture.js` | XHS | 拦截 fetch/XHR，捕获评论 API（`/api/sns/web/v2/comment/page`、`/comment/sub/page`）；提供 snapshot 查询 + 页面侧 fetch 通道 |
| `douyinApiCapture.js` | Douyin | 拦截 fetch/XHR，缓存视频 API 响应到 `window.__lgboom_dy_video_data`（200 条 FIFO）|

**注入脚本铁律**：

- 必须 `if (window.__lgboom_*_installed) return; window.__lgboom_*_installed = true;` 防重安装
- 必须保留原始 fetch/XHR 引用再 hook
- hook 失败要静默（不能影响平台页面）

### 7.6 数据层（`src/db/`）

| 文件 | 职责 |
|---|---|
| `index.js` | Dexie 实例 + 13 版 schema 定义 |
| `noteStore.js` / `commentStore.js` / `authorStore.js` | 三类数据的 CRUD + 查询 |
| `collectionRunStore.js` | 任务执行记录生命周期 + 外部任务绑定 + 心跳更新 |
| `collectionRunStatus.js` | 任务状态语义 |
| `mediaAssetStore.js` | 媒体资产 + 下载状态 |
| `workbenchOutboxStore.js` | outbox 入队/确认/重试/终态，`in_flight` 5 分钟过期回收，指数退避（1s→2s→5s→15s→60s）|
| `accountStore.js` | 平台账号池 + 日配额 |
| `recordNormalization.js` | 读时标准化（旧记录运行时对齐）|
| `legacyDataMaintenance.js` | 显式批量回填（一次性）|

### 7.7 工作台协议层（`src/workbench/protocol/`）

| 文件 | 职责 |
|---|---|
| `schema.js` | 所有协议常量（§6 全部来源）|
| `validator.js` | 信封校验（协议版本/任务类型/页面类型/载荷结构）|
| `deltaEnvelope.js` | 幂等键生成、事件/记录信封构建、批量上传封装 |
| `recordPayloadValidator.js` | 记录最小结构校验 |
| `responseEnvelope.js` | 响应封装 |

### 7.8 工作台运行时（`src/workbench/runtime/`，36 个文件）

核心模块（完整清单见 `docs/ARCHITECTURE.md` §8.4）：

| 模块 | 职责 |
|---|---|
| `taskPoller.js` | 对账恢复 → 租约认领 → 续租 → 提交；空轮询按服务端 `nextPollAfterMs` + 5-15s 随机错峰 |
| `pluginAuthorization.js` | 授权码激活 + 设备授权持久化 + 统一授权门禁 |
| `executionStationClient.js` / `executionStationRuntime.js` | 工位配对、心跳、能力上报 |
| `taskLeaseClient.js` | 租约认领/续租/本地持久化 |
| `capabilityCheck.js` / `capabilityReportBuilder.js` | 页面能力自检 + 自描述报告 |
| `taskEnvelopeMapper.js` | 协议信封 → 内部指令翻译 |
| `taskControlMapper.js` | 控制指令 → 内部操作 |
| `monitorTask.js` | 监控策略翻译 + `monitorMeta` 生成 |
| `taskDeltaReporter.js` + `deltaOutbox.js` | 增量上报 + outbox flush |
| `progressEvent.js` | 阶段推断（CONTEXT_CHECK→DISCOVERING→COLLECTING→DOWNLOADING→PERSISTING→FINALIZING）|
| `errorMapper.js` | 错误 → 协议错误码映射 |
| `resultPackager.js` + `resultSummaryBuilder.js` | 结果打包 + 数量统计 |
| `heartbeat.js` | 3 秒心跳 |
| `executorIdentity.js` | 持久化 UUID（`plugin_*` 前缀）|
| `cookieManager.js` | 远程任务账号 Cookie 注入（仅 xhs 完整，抖音待补）|
| `executionAccountLock.js` / `manualExecutionLock.js` | 同账号互斥锁 |
| `navigationOrchestrator.js` | 执行页导航编排（避免劫持用户前台页）|
| `taskExecutionCleanup.js` | 任务页回收（只关插件自己开的页，不关用户页）|
| `pluginInstallBootstrap.js` | 安装/启动 bootstrap |
| `workbenchPushSubscription.js` | Web Push 订阅注册 |
| `xhsBatchRunHelper.js` / `douyinBatchRunHelper.js` | 批量任务远程 run 辅助 |
| `dataFoundationPayload.js` | 笔记数据地基出站字段补齐 |
| `batchResume.js` | 批量断点续跑 checkpoint |
| `taskRuntimeObservability.js` | 运行可观测性（schema 校验计数/失败字段/reasonCode）|

### 7.9 Popup（`src/popup/`，React 19）

| 文件 | 职责 |
|---|---|
| `index.jsx` | React createRoot 入口 |
| `App.jsx`（约 1171 行）| Popup 主应用：3 tab（采集/数据/配置）、平台检测、统计、批量设置、Cookie/账号、授权、工位 |
| `utils.js` | 工具函数 |
| `popup.html` / `popup.css` | 容器壳 + 样式 |
| `components/` | AddAccountModal / BatchSettingsModal / ConfirmModal / ActionButtons / CookieAccountSection / FlywheelSection / Notice / PageContextInfo / ProgressSection / StatsSection / TabNav |

**3 个 tab**：

```js
TABS = [
  { id: 'tab-collect', label: '采集' },
  { id: 'tab-data',    label: '数据' },
  { id: 'tab-config',  label: '配置' },
]
```

**工作台地址快捷入口**：

```js
CONTENT_WORKBENCH_PROD_URL  = 'https://lingganboom.fun'
CONTENT_WORKBENCH_LOCAL_URL = 'http://localhost:3000'
```

### 7.10 Dashboard（`src/dashboard/`，React 19，iframe 形态）

| 文件 | 职责 |
|---|---|
| `index.jsx` | React 入口 |
| `App.jsx`（约 1174 行）| 主应用：notes/comments/authors 三 tab、搜索/筛选/排序、勾选批量、媒体预览、二次下载、同步 |
| `utils.js` | 工具 |
| `components/ErrorBoundary.jsx` | 错误边界 |

**关键常量**：

```js
DASHBOARD_LOAD_CHUNK_SIZE = 200    // 分批读取本地记录
PAGE_SIZE_OPTIONS = [50, 200, 500]
```

### 7.11 共享层（`src/shared/`）

| 文件 | 职责 |
|---|---|
| `constants.js` | MSG / TASK_STATE / COLLECT_MODE / COMMENT_DEPTH_MODE / PAGE_TYPE / BATCH_CONFIG |
| `messaging.js` | `sendToBackground` / `sendToTab`（含断线重注入）/ `reportProgress` / `reportTaskError` / `reportWorkbenchRecord` / `reportDone` / `isContextValid` |
| `utils.js` | `parseCount` / `extractNoteId` / `csvEscape` / `generateCsv` / `downloadFile` / `toHighQualityImageUrl` / `pickBestVideoStream` / `getHighQualityImageCandidates` / `normalizeServerUrl` 等 |
| `responseEnvelope.js` | 响应封装 |
| `baseBatchController.js` | 批量控制器基类 |
| `managedTaskController.js` | 共享任务控制器（content + douyin 复用）|
| `collectorMetadata.js` | 采集器版本戳 + 原始证据（`COLLECTOR_VERSION = '2026-03-27-wave5-v1'`）|
| `targetIdentity.js` | 目标身份识别 |
| `feedback.js` | 反馈语义（标题/正文/图标/颜色）|
| `icons.js` | 图标 |
| `selectorHealth.js` | 选择器健康 |
| `brandAssets.js` | 品牌资源 URL |
| `taskUi.js` | **死代码集中地**（7 个函数零引用，复刻时不要实现）|

### 7.12 同步（`src/sync/`）

| 文件 | 职责 |
|---|---|
| `flywheelSync.js`（约 663 行）| 工作台同步：`syncToFlywheel` / `ingestCollectionTaskDelta` / `fetchCollectionTaskControlRequests` / `patchCollectionTask` / 封面上传。`checkFlywheelConnection` 是死函数，不要实现 |

### 7.13 主题（`src/themes/`）

| 文件 | 职责 |
|---|---|
| `themeManager.js` | 主题管理（content/popup/dashboard 共享）|
| `ac-ui/tokens.js` + `ac-ui/popup.css` | AC-UI 主题 token + popup 样式 |

---

## §8. 外部依赖清单（高风险 — 必须重新验证）

> 这些是**会随平台改版而失效**的契约。本节给出当前验证状态（来自 `docs/SELECTORS.md` 和 `docs/technical/*_FIELD_SURVEY.md`），但**复刻上线前必须重新实机验证**，不能照抄。

### 8.1 小红书页面契约

| 项 | 值 | 验证日期 |
|---|---|---|
| 笔记结构化数据 | `window.__INITIAL_STATE__.note.noteDetailMap` | 2026-04-18 |
| 博主结构化数据 | `window.__INITIAL_STATE__.user.userPageData`（**Vue ref，需 `._rawValue` 拆包**）| 2026-04-18 |
| 搜索筛选状态 | `window.__INITIAL_STATE__.search.filterParams`（`sort_type/filter_note_type/filter_note_time`）| 2026-06-01 |
| 笔记流容器（搜索页）| `.feeds-container` | 2026-06-01 |
| 笔记流容器（博主页）| `#userPostedFeeds` | 2026-06-01 |
| 笔记卡片 | `section`（容器下）| 2026-06-01 |
| 卡片封面链接 | `a.cover`（href 格式：搜索页 `/search_result/{noteId}?xsec_token=...`；博主页 `/user/profile/{userId}/{noteId}?xsec_token=...`）| 2026-06-01 |
| 卡片标题 | `.title`（搜索页更干净）或 `.footer span`（限定在 section 内）| 2026-06-01 |
| 卡片点赞 | `.like-wrapper .count`（限定在 section 内）| 2026-06-01 |
| 视频标识 | `.play-icon` | 2026-06-01 |
| 评论容器 | `.comments-container`（含 `data-v-4a19279a`）| 2026-04-18 |
| 主评论 | `.parent-comment` / `.comment-item:not(.comment-item-sub)` | 2026-04-18 |
| 子评论 | `.comment-item.comment-item-sub` | 2026-04-18 |
| 评论 API 端点 | `/api/sns/web/v2/comment/page` / `/comment/sub/page` | 2026-04-18 |
| 博主号 | `SPAN.user-redId`（含前缀"小红书号："）| 2026-04-18 |
| 博主名 | `DIV.user-name` | 2026-04-18 |

> 完整选择器表（含每个的"匹配数"和"备注"）见 `docs/SELECTORS.md`。

**详情页 URL 形态**：

- `/explore/{noteId}` 或 `/discovery/item/{noteId}`
- 搜索页 `/search_result/{noteId}` 打开后实际进入 `/explore/{noteId}`

**详情页 key 过滤**：`noteDetailMap` 含幽灵 key `"undefined"` 和 `""`，必须用正则 `/^[a-f0-9]{24}$/i` 过滤真实 noteId。

### 8.2 抖音页面契约

| 项 | 值 | 验证日期 |
|---|---|---|
| 激活视频 | `[data-e2e="feed-active-video"]` | 2026-03-27 |
| 当前视频 ID | `[data-e2e="video-info"][data-e2e-aweme-id]` | 2026-03-27 |
| 视频标题 | `[data-e2e="video-desc"]`, `[data-e2e="detail-video-info"]` | 2026-03-27 |
| 博主昵称 | `[data-e2e="feed-video-nickname"]` | 2026-03-27 |
| 视频 detail API | `/aweme/v1/web/aweme/detail/?aweme_id=<id>&aid=6383` | 2026-03-27 |
| 一级评论 API | `/aweme/v1/web/comment/list/?item_id=<id>&aweme_id=<id>` | 2026-03-27 |
| 二级评论 API | `/aweme/v1/web/comment/list/reply/?item_id=<id>&aweme_id=<id>&comment_id=<parentId>` | 2026-03-27 |
| 评论主键 | `cid` | 2026-03-27 |
| 评论层级 | `root_comment_id / reply_id / reply_to_reply_id / level` | 2026-03-27 |
| 评论地域 | `ip_label` | 2026-03-27 |
| 评论图片 | `image_list[].origin_url.url_list`（高清优先）；兜底 `download_url → medium_url → crop_url → thumb_url` | 2026-03-27 |
| 主页预览态 URL | 含 `/user/` 且仅有 `vid` 参数 | 2026-03-27 |
| 弹层态 URL | 含 `/user/` 且有 `modal_id` | 2026-03-27 |
| 直接视频页 | 含 `/video/` | 2026-03-27 |

**搜索页 DOM 差异**（关键）：

- 综合搜索页（无 `type=video`）：DOM 发现**不可用**，必须走 API 发现（`aweme_general` 频道）
- 视频搜索页（`type=video`）：`<li><a href="/video/...">` 结构，DOM 发现可用

### 8.3 内容工作台契约

见 §6.9 HTTP 端点表。**鉴权边界仍未完成统一收口**（`MESSAGE_PROTOCOL.md` §6 标注），接入生产前必须重新核对。

### 8.4 注入脚本通信协议

**小红书 noteMap.js**：

```js
// content → injected：通过创建 <script> 标签注入
// injected → content：window.postMessage({ type: 'noteMap', data }, '*')
// data 是 { [noteId]: note } map，已过滤幽灵 key
```

**小红书 xhsApiCapture.js**（双向 bridge）：

```js
// 来源标识：
BRIDGE_SOURCE = 'lgboom-xhs-api-capture'    // injected → content
REQUEST_SOURCE = 'lgboom-xhs-content'       // content → injected

// 请求类型：
SNAPSHOT_REQUEST_TYPE  = '__lgboom_xhs_comment_api_request__'
SNAPSHOT_RESPONSE_TYPE = '__lgboom_xhs_comment_api_response__'
PAGE_FETCH_REQUEST_TYPE  = '__lgboom_xhs_page_fetch_request__'
PAGE_FETCH_RESPONSE_TYPE = '__lgboom_xhs_page_fetch_response__'

// 拦截的 API：
API_PATTERNS = ['/api/sns/web/v2/comment/page', '/api/sns/web/v2/comment/sub/page']

// 缓存：
window.__lgboom_xhs_comment_pages      // { [noteId]: snapshot[] }，最多 30 条
window.__lgboom_xhs_sub_comment_pages  // { [noteId::rootCommentId]: snapshot[] }
```

**抖音 douyinApiCapture.js**：

```js
// 缓存：
window.__lgboom_dy_video_data  // 视频数据数组，200 条 FIFO 上限
```

---

## §9. Magic number 全表

### 9.1 采集节奏（`BATCH_CONFIG`）

| 参数 | 值 | 说明 |
|---|---|---|
| `maxPerSession` | 50 | 单次批量上限 |
| `intervalMin` / `intervalMax` | 1200 / 2800 ms | 操作间隔随机区间 |
| `iframeTimeout` | 15000 ms | iframe 加载超时 |
| `scrollStepMin` / `scrollStepMax` | 100 / 300 | 滚动步长 |
| `scrollIntervalMin` / `scrollIntervalMax` | 200 / 500 ms | 滚动间隔 |
| `maxScrollRetries` | 10 | 最大滚动重试 |
| `maxSubComments` | 200 | 子评论上限 |

### 9.2 XHS 发现计划（`buildDiscoveryPlan`）

| 场景 | stepRatio | settleDelay | stableNoNewLimit | bottomConfirmationRounds |
|---|---|---|---|---|
| 搜索页 | 0.68 | 900 ms | 2 | 0 |
| 博主页（`#userPostedFeeds`）| 0.55 | 1300 ms | 4 | 6 |

博主页 `profileTargetRounds`：`min(max(expectedCount, 28), 80)`，缺省 28。

### 9.3 工作台运行时

| 参数 | 值 | 说明 |
|---|---|---|
| 心跳周期 | 3 秒 | `heartbeat.js` |
| 租约超时 | 2 小时 | 过期回收 |
| 页面启动保护窗口 | 45 秒 | 接单后页面没跑起来 → 标失败释放 |
| `MIN_CHROME_ALARM_INTERVAL_MS` | 30000 ms（30s）| `taskPollSchedule.js`，Chrome alarm 最小周期 |
| 任务轮询初始周期 | `INITIAL_WORKBENCH_TASK_POLL_MINUTES` | alarm 周期 |
| 工位心跳 alarm | 1 分钟 | |
| 空闲接单等待 | 服务端 `nextPollAfterMs`（约 2 分钟）+ 5-15s 随机错峰 | |
| 能力不匹配/账号冷却等待 | 服务端给定（约 2-5 分钟）| |
| `sendToBackground` 默认超时 | 15000 ms | `messaging.js` |
| `sendToTab` 能力检查超时 | 4000 ms | background |
| `WORKBENCH_DISPATCH_TASK` 派单超时 | 10000 ms（asyncDispatch 12000 ms）| |
| 媒体下载单候选超时 | 180000 ms（3 分钟）| `downloadService.js` |

### 9.4 Outbox 退避

指数退避序列：**1s → 2s → 5s → 15s → 60s**（`workbenchOutboxStore.js`）。

`in_flight` 过期：**5 分钟**（P0-1 修复），过期后自动回可重试队列。

### 9.5 抖音缓存上限

`window.__lgboom_dy_video_data`：**200 条 FIFO**（R4 修复，防内存泄漏）。

### 9.6 Dashboard 分批

`DASHBOARD_LOAD_CHUNK_SIZE = 200`，分页选项 `[50, 200, 500]`。

---

## §10. 验收清单（复刻完成判定）

### 10.1 静态契约验收（机器可查）

- [ ] manifest.json 与 §3.4 逐字一致
- [ ] `src/shared/constants.js` 的 MSG 全部 41+ 常量与 §4.1 一致
- [ ] `src/db/index.js` 的 v13 schema 与 §5.1 一致
- [ ] `src/workbench/protocol/schema.js` 的协议常量与 §6 一致
- [ ] `npm run check:contracts` 通过
- [ ] `npm run build` 通过
- [ ] `node --test tests/*.test.mjs` 全部通过（参考基线：110 个测试文件，450+ 用例）

### 10.2 实机功能验收（真实浏览器）

按 `docs/product/TEST_CHECKLIST.md` 全量跑。关键项：

- [ ] 小红书笔记详情页：采笔记 → 媒体弹窗分项下载（封面/图片/Live/视频）
- [ ] 小红书搜索页/博主页：批量笔记 5/10/20/50，含 Top-N、筛选、暂停/继续/停止
- [ ] 小红书评论采集：含子评论，API 捕获 + DOM 兜底
- [ ] 抖音弹层页：采视频 / 下载视频 / 采评论 / 评论图片区下载
- [ ] 抖音博主页/搜索页：批量视频 + 批量评论（API 驱动）
- [ ] 抖音 native 分享按钮触发当前视频采集
- [ ] Dashboard：搜索/筛选/排序/勾选/批量导出/批量删除/媒体预览/二次下载/同步
- [ ] Popup：Cookie 获取（按平台隔离）、账号列表、删除二次确认
- [ ] 授权流程：授权码激活 → 工位配对 → 心跳 → 接单
- [ ] 远程任务完整闭环：`pending → dispatched → running → completed/failed`，记录正确 ingest

### 10.3 安全验收

- [ ] Dashboard 伪造 `lgboom-dashboard` 消息（不带 nonce / 错 nonce）→ 插件拒绝处理
- [ ] 页面脚本伪造 source → 拒绝
- [ ] 断网/重启后 outbox `in_flight` 5 分钟后自动重试，数据不丢
- [ ] 危险动作（停止/删除/清空）全部二次确认
- [ ] 主按钮忙碌态：禁用 + 显示"执行中"

### 10.4 文档同步验收

每轮实现闭环后必须同步（`CLAUDE.md` 文档同步硬门禁）：

| 改动类型 | 必须更新 |
|---|---|
| 任意代码改动 | `progress.txt` |
| 用户可见行为变化 | `docs/product/PRD.md` + `APP_FLOW.md` + `TEST_CHECKLIST.md` |
| 字段/协议/选择器变化 | `docs/technical/DATA_MODEL.md` + `MESSAGE_PROTOCOL.md` + `SELECTORS.md` + `AI_READY_DATA_CONTRACT_V1.md` |
| 架构/策略转向 | `docs/decisions/index.md` |

---

## §11. 已知空缺（复刻时如实告知用户，不要遮掩）

| 编号 | 空缺 | 影响 |
|---|---|---|
| G1 | 抖音远程任务无账号池自动切换（P1-2）| 抖音远程任务依赖当前浏览器登录态，无法像小红书一样自动换号 |
| G2 | 工作台自动接单主链路未签真实账号实机闭环 | 代码 + 单测已覆盖，但 pending→completed 全链路在工作台 UI 里推进未签收 |
| G3 | 监控任务"页面连接中断"失败口径（P1-3）| 需与工作台状态口径对齐 |
| G4 | `content.js` 667 KiB 偏大 | 内容脚本不能拆异步 chunk，已是当前折中 |
| G5 | envelope 未统一为 `{success,data,error}`（T6）| 消费方按 action 容错 |
| G6 | 工作台 HTTP 鉴权边界未完成统一收口 | 接入生产前必须重新核对 |
| G7 | `notifications` 权限申请了但使用面待确认 | 评估是否真在用 |

---

## §12. 复刻工作流建议（给执行 agent）

按这个顺序做，能把幻觉风险降到最低：

1. **先搭骨架**：manifest.json + 4 个 webpack 入口 + 空 MSG 常量 + Dexie v13 schema + 空 React popup/dashboard。`npm run build` 能产出可加载的空壳。
2. **再实现共享层**：`constants.js` 全量、`messaging.js`、`utils.js`。这一层不依赖外部系统，可以照抄契约。
3. **小红书单篇采集**：这是最基础闭环，先把 noteMap.js 注入 + noteCollector.js + noteStore 跑通。**选择器必须先在真实页面验证**。
4. **Dashboard + 本地数据查看**：让采集结果可见，建立反馈闭环。
5. **小红书批量**：BatchNoteController + 滚动发现 + 视觉排序。
6. **评论 + 媒体下载**：API 捕获 + DOM 兜底 + 媒体分项。
7. **抖音单条 + 三源融合**：VideoContext 铁律是关键。
8. **抖音批量（API 驱动）**。
9. **工作台协议层**：协议常量 + validator + deltaEnvelope。先跟内容工作台做"假任务"联调。
10. **工作台运行时**：taskPoller + lease + heartbeat + outbox。
11. **授权两层身份**：授权码 + 配对码 + 门禁。
12. **Web Push**。
13. **实机验收**：跑 §10 全量清单。

**每个外部依赖步骤（3/5/6/7/8）动手前，必须先用 §8 的探查脚本在真实页面验证一遍选择器和字段**。验证产出回写 `docs/SELECTORS.md` + 本蓝图 §8。

---

## §13. 与现有文档的导航

本蓝图是**自包含复刻入口**，但不重复已有权威文档的全部细节。深入某主题时去对应文档：

| 需要 | 去哪 |
|---|---|
| 字段完整业务语义 | `docs/technical/DATA_MODEL.md` |
| 消息 payload 细节 | `docs/technical/MESSAGE_PROTOCOL.md` |
| 选择器匹配数和备注 | `docs/SELECTORS.md` |
| 小红书字段调研 | `docs/technical/XHS_FIELD_SURVEY.md` |
| 抖音字段调研 | `docs/technical/DOUYIN_FIELD_SURVEY.md` |
| 授权协议完整 | `docs/technical/PLUGIN_AUTHORIZATION_PROTOCOL.md` |
| AI 数据契约 | `docs/technical/AI_READY_DATA_CONTRACT_V1.md` |
| 用户操作流 | `docs/product/APP_FLOW.md` |
| 功能验收清单 | `docs/product/TEST_CHECKLIST.md` |
| 架构决策历史 | `docs/decisions/index.md` |
| 真实时间线 | `progress.txt` |
| 探查脚本 | `scripts/probe-*.js`（7 个）|

**警告**：以下文档**已滞后**，复刻时不要直接当事实源（审查报告 §3.1）：

- `docs/technical/TECH_STACK.md`（写 v7/ debugger 权限，实际 v13/ 无 debugger）
- `docs/ARCHITECTURE.md`（写 2.0.0 / 32 测试，实际 2.0.52 / 110 测试；列了死文件 commentCollectTask.js）

以本蓝图 §3/§5/§7 为准。
