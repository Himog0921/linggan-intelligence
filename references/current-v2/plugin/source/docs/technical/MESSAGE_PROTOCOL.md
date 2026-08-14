# 消息协议

> 扩展内部各层之间的消息通信契约。  
> 当前消息常量事实源：`src/shared/constants.js`

## 1. 传输路径

| 路径 | 方式 |
|------|------|
| Popup → Background | `chrome.runtime.sendMessage` |
| Background → Content | `chrome.tabs.sendMessage` |
| Content → Background | `chrome.runtime.sendMessage` |
| Content ↔ Injected Script | `window.postMessage` / `CustomEvent` / `postMessage` |
| Content ↔ Dashboard | iframe `postMessage` |
| Background ↔ 内容工作台 | HTTP JSON 轮询 / V1.1 sync / ingest | 执行工位同步、任务 reservation、续租进度、控制请求拉取、事件/记录增量写入 |
| 内容工作台 → Background | Chrome Web Push | 有新任务或任务控制变化时叫醒插件，插件随后走原有 HTTP 接单链路 |

> 说明：抖音链路除了 Content Script 自己的逻辑，还包含页面桥接、页面侧 fetch 和 API capture，不应再假设所有能力都只靠 Content 直接完成。
> Dashboard 由 Content 以 iframe 打开时，会生成一次性 `nonce`，同时写入 `chrome.storage.session` 并放入 `dashboard.html?nonce=...`。Dashboard 读取数据时优先使用 URL 中的 `nonce` 发起 `postMessage`，避免首次加载时 storage 尚未同步而被外层拒收。

## 2. 命令消息

### 2.1 单条采集

| Action | 发送方 → 接收方 | Payload | 说明 |
|--------|----------------|---------|------|
| `COLLECT_SINGLE_NOTE` | Popup → Content | `{}` | 小红书采当前笔记；抖音采当前视频 |
| `COLLECT_SINGLE_COMMENT` | Popup → Content | `{ maxTotal?, maxSubComments?, sortMode?, triggerSource? }` | 小红书采当前笔记评论；抖音采当前视频评论 |
| `DOWNLOAD_CURRENT_COMMENT_IMAGES` | Popup → Content | `{ maxTotal?, maxSubComments? }` | 当前视频评论图片区下载（当前仅抖音 Popup 使用） |
| `COLLECT_AUTHOR` | Popup → Content | `{}` | 采当前博主 |
| `DISCOVER_AUTHOR_NOTE_LINKS` | Background → Content | `{ maxLinks?, maxScrolls?, profileUrl?, authorArchiveJobId? }` | 小红书博主主页历史笔记链接发现，只回传作品卡片链接，不打开详情 |

### 2.2 批量内容

| Action | 发送方 → 接收方 | Payload |
|--------|----------------|---------|
| `START_BATCH_NOTES` | Popup → Background → Content | `{ tabId, mode, count, topByLikes }` |
| `PAUSE_BATCH_NOTES` | Popup → Background → Content | `{ tabId }` |
| `RESUME_BATCH_NOTES` | Popup → Background → Content | `{ tabId }` |
| `STOP_BATCH_NOTES` | Popup → Background → Content | `{ tabId }` |

### 2.3 批量评论

| Action | 发送方 → 接收方 | Payload |
|--------|----------------|---------|
| `START_BATCH_COMMENTS` | Popup → Background → Content | `{ tabId, mode, count, commentLimit }` |
| `PAUSE_BATCH_COMMENTS` | Popup → Background → Content | `{ tabId }` |
| `RESUME_BATCH_COMMENTS` | Popup → Background → Content | `{ tabId }` |
| `STOP_BATCH_COMMENTS` | Popup → Background → Content | `{ tabId }` |

> `commentLimit = 0` 代表未设置上限，即“尽量采全量”。当前抖音批量评论已实际使用该字段。

### 2.4 数据与面板

| Action | 发送方 → 接收方 | Payload | 说明 |
|--------|----------------|---------|------|
| `TOGGLE_DASHBOARD` | Popup → Content | `{}` | 打开 / 关闭 Dashboard |
| `GET_STATS` | Popup / Dashboard → Content | `{}` | 读取统计数据 |
| `GET_ALL_NOTES / GET_ALL_COMMENTS / GET_ALL_AUTHORS` | Dashboard → Content | `{ source: "lgboom-dashboard", nonce }` | 读取本地数据；`nonce` 必须和本次 iframe 打开的值一致 |
| `DELETE_NOTE / DELETE_COMMENT / DELETE_AUTHOR` | Dashboard → Content | `{ noteId / id / userId }` | 删除单条 |
| `CLEAR_ALL_NOTES / CLEAR_ALL_COMMENTS / CLEAR_ALL_AUTHORS` | Dashboard → Content | `{}` | 清空某类数据 |
| `EXPORT_CSV / EXPORT_JSON` | Popup / Dashboard → Content | `{}` | 导出数据 |
| `EXPORT_OUTBOX_RECOVERY` | Popup → Background | `{}` | 只读导出恢复包（captureJournal + 死信 outbox 行，plugin-local-recovery/v1），供工作台 `POST /api/execution-tasks/recovery-import` 显式恢复导入；不改任何行状态 |
| `DOWNLOAD_NOTE_MEDIA` | Dashboard → Content | `{ noteId }` | 下载内容媒体 |

### 2.5 Background 专属

| Action | 发送方 → 接收方 | Payload | 说明 |
|--------|----------------|---------|------|
| `DOWNLOAD_MEDIA_FILE` | Content → Background | `{ candidates, filename, saveAs?, conflictAction?, headers? }` | 下载图片、视频、评论图等媒体 |
| `BLOCK_MEDIA / UNBLOCK_MEDIA` | Content → Background | `{}` | 批量采集时临时屏蔽媒体资源 |
| `DISPATCH_ESC` | Content → Background | `{ tabId }` | 通过 debugger 派发 Esc 等动作 |

### 2.6 工作台协同

| Action | 发送方 → 接收方 | Payload | 说明 |
|--------|----------------|---------|------|
| `SYNC_TO_WORKBENCH` | Dashboard / Content → Background | `{ notes?, comments?, authors? }` | 将选中的本地记录同步到内容工作台；Dashboard 会按大批量评论同步等待，并展示工作台返回的评论接收、入库、跳过、待处理和待重试结果；博主结果以实际创建的监控来源数为准 |
| `WORKBENCH_CAPABILITY_CHECK` | Background → Content | `{ tabId?, task }` | 对远程任务做页面能力检查 |
| `WORKBENCH_DISPATCH_TASK` | Background → Content | `{ tabId?, task }` | 将工作台任务协议映射到内部动作并派单；成功回包必须保留本次执行页 `tabId` |
| `WORKBENCH_TASK_CONTROL` | Background → Content | `{ tabId?, taskControl?, command? }` | 对已接单任务执行暂停 / 继续 / 停止；统一归口到 Content runtime 的工作台处理器 |
| `WORKBENCH_GET_RESULT_PACKAGE` | Background → Content | `{ tabId?, externalTaskId?, collectionRunId? }` | 从本次执行页的 `collectionRuns` 打包结果并回传给工作台 |
| `WORKBENCH_LOCAL_CONTROL_EVENT` | Content → Background | `{ externalTaskId?, collectionRunId?, taskType?, controlAction, status, message?, occurredAt? }` | 插件本地暂停 / 继续 / 停止同步回工作台事件流 |
| `WORKBENCH_DELTA_FLUSH` | 内部 / 调试 → Background | `{}` | 触发工作台增量 outbox 立即 flush |
| `AUTHORIZE_PLUGIN_ACCESS` | Popup → Background | `{ serverUrl, authorizationCode, browserLabel? }` | 使用内容工作台设置里生成的授权码连接当前浏览器；Background 会同时带上本浏览器稳定工位身份，工作台返回授权和工位 |
| `CLEAR_PLUGIN_AUTHORIZATION` | Popup → Background | `{}` | 清除当前浏览器的插件授权，并解除当前工位绑定 |
| `GET_EXECUTION_STATION_STATUS` | Popup → Background | `{}` | 查看当前浏览器是否已授权、当前工位是否已准备好，以及可上报的平台账号状态 |
| `REGISTER_EXECUTION_STATION` | Popup → Background | `{ serverUrl, pairingCode, browserLabel? }` | 用内容工作台生成的配对码绑定当前浏览器工位 |
| `SEND_EXECUTION_STATION_HEARTBEAT` | Popup / alarm → Background | `{}` | 主动发送一次执行工位心跳 |

> 当前实现中，`WORKBENCH_*` 是插件内部桥接动作；所有暂停 / 继续 / 停止都进入 runtime 的工作台处理器，再按任务登记控制执行页。Background 在授权连接时自动拿到工位身份，再通过执行工位协议和内容工作台对账，按服务端 `nextPollAfterMs` 安排下一次接单检查。空闲时心跳只更新工位在线状态，不会绕过已安排的接单等待；已有活跃任务时仍会继续短周期续约、取控制指令和回写进度。Web Push 只负责把下一次接单检查提前，不负责直接派发任务。
>
> 工作台远程任务的最终结果包必须从执行页读取，不能用 Background 本地库伪造。派单成功后，轮询器要持久保存执行页 `tabId`，后续 `WORKBENCH_GET_RESULT_PACKAGE` 必须优先带上这个 `tabId`。如果任务已经进入 running，但超过保护窗口仍找不到页面侧结果包，应把任务标记为“结果包没有交回工作台”的失败，而不是继续只发心跳。
>
> 2.0.95 候选中，`WORKBENCH_GET_RESULT_PACKAGE` / `TASK_RESULT` 负责从真实执行页取得终态。XHS 终态由 workflow 持久化 `captureReport`，Background mapper 生成唯一 `CaptureSubmissionV2`，outbox 直达 `/api/v2/evidence/execution`。`/sync commit_raw_snapshot` 是 2.0.91 及更早版本的历史路径，不得用于 2.0.95 XHS 新数据。
> 笔记记录进入 outbox 前会补齐数据地基出站字段，包括 `standardContentCode`、`standardAuthorCode`、`keywords`、`authorFans`、`authorFansCollectedAt`、`mediaUnderstanding`、`sourceRun` 和 `dataFoundation` 摘要，供内容工作台做低粉爆文、爆款聚类和 Claude Agent 打标。

### 2.7 工作台 HTTP 协议补充

控制请求路径：

```text
Workbench UI → Workbench API control request → Plugin Background poller
```

插件拉取：

```text
GET /api/collection-tasks/:taskId/control-requests?executorInstanceId=<id>&after=<cursor>
```

执行工位与任务租约：

```text
POST /api/plugin-authorizations/activate
POST /api/execution-stations/register
POST /api/execution-stations/sync
```

工位同步响应补充：

```json
{
  "mailboxVersions": {
    "station": 108,
    "xhs.monitor_patrol": 5
  },
  "operationResults": {},
  "reservations": [],
  "controlCommands": [],
  "nextSync": { "afterMs": 60000, "reason": "idle" }
}
```

说明：
- 插件只处理 V1.1 响应字段：`mailboxVersions`、`reservations[]`、`operationResults`、`controlCommands[]` 与 `nextSync`。
- 插件从 v2.0.55 起 body 只发送 V1.1 字段；任务领取通过 `capacity → reservations[] → start_job` 完成。
- 任务运行中的续租与进度上报通过 `/api/execution-stations/sync` 的 `progress_update` operation 完成。
- `/sync` 只承接 reservation、续租/进度和控制邮箱；2.0.95 XHS 终态不再发送 `commit_raw_snapshot`。producer 已证明的空结果通过 V2 Evidence 提交；无法证明的终态走非 Evidence 失败控制路径并释放 lease。

### V1.1 /sync 协议（v2.0.55+）

v2.0.55 起，`/api/execution-stations/sync` 使用纯 V1.1 派单/续租协议，对应内容工作台手册第 5.5 节。插件授权身份走 `Authorization: Bearer <authorizationToken>` 请求头，body 只发送 V1.1 字段。

| 字段 | 类型 | 说明 |
|------|------|------|
| `protocolVersion` | string="3" | V1.1 协议版本，缺失或低于 "3" 会被服务端 401 拒绝 |
| `pluginVersion` | string | 插件版本号，低于服务端最低要求会被 426 拒绝（VERSION_REJECTED） |
| `stationSessionId` | string | 单次插件安装周期内的会话标识，持久化在 chrome.storage.local，换授权/换工位时清理 |
| `mailboxCursors` | `{station: number, [platform.lane]: number}` | 客户端已看到的 mailbox 版本号；服务端对比后短路 idle 返回 |
| `capacity` | `{[platform.lane]: {remainingWorkSeconds, targetWorkSeconds, maxReservedTasks}}` | 按 V1.1 真实车道上报容量，例如 `xhs.monitor_patrol`、`douyin.governance`；服务端按此发放 reservation |
| `activeLeases[]` | `Array<{jobId, leaseToken, leaseEpoch, lane?, progress?, stage?, lastProgressAt?}>` | 工位当前持有的租约数组 |
| `operations[]` | array | 2.0.95 XHS 当前使用 `start_job`、`progress_update` 与 `release_job`；`commit_raw_snapshot` 仅为旧版历史操作，不得承接新终态。`start_job` 在 reservation 携带账号时必须回传同一个 `platformAccountId`；`account_risk_control / control_ack` 待后续迁移 |
| `accountReports[]` | `Array<{platform, platformAccountId?, healthStatus, cooldownUntil?}>` | 基础账号健康上报，由本地平台账号状态转换 |

V1.1 响应 body 结构：

```json
{
  "serverTime": "2026-06-27T00:00:00.000Z",
  "mailboxVersions": {
    "station": 108,
    "xhs.monitor_patrol": 5,
    "douyin.governance": 7
  },
  "operationResults": {
    "start_job_123": {
      "status": "accepted",
      "attemptId": "attempt_123",
      "leaseToken": "lease_123",
      "leaseEpoch": 4,
      "leaseExpiresAt": "2026-06-27T00:10:00.000Z"
    }
  },
  "reservations": [
    {
      "jobId": "job_123",
      "reserveToken": "reserve_123",
      "reservationEpoch": 3,
      "lane": "xhs.monitor_patrol",
      "platformAccountId": "runtime:xhs",
      "taskSpec": {
        "id": "job_123",
        "platform": "xhs",
        "lane": "monitor_patrol",
        "taskType": "xhs.batchNotes",
        "source": "monitor",
        "taskStrategy": "author_patrol",
        "target": "https://www.xiaohongshu.com/user/profile/...",
        "platformAccountId": "runtime:xhs"
      }
    }
  ],
  "controlCommands": [],
  "nextSync": { "afterMs": 240000, "reason": "idle" }
}
```

说明：
- 插件只解析 V1.1 字段：`mailboxVersions` 对象和 `nextSync` 对象。
- 插件收到 `reservations[]` 后不会直接执行，而是立刻再发一次 `/sync`，用 `start_job` operation 携带 `jobId / reserveToken / reservationEpoch` 确认开始；服务端接受后返回正式 `leaseToken`。
- reservation 如果携带 `platformAccountId`，插件必须把该值写入本地任务对象，并在 `start_job` operation 中回传同一个值。插件执行时优先使用服务端绑定账号；`runtime:<platform>` 表示使用当前浏览器登录会话，不再做本地 Cookie 注入。
- 插件运行中续租发 `progress_update` operation，携带 `jobId / leaseToken / leaseEpoch / stage / progress`。
- `nextSync.reason` 前缀为 `mailbox` 或 `claim` 时，插件立即触发任务轮询。
- 426 错误：pluginVersion 低于服务端最低要求时返回，reasonCode=`VERSION_REJECTED`，用户需升级插件。
- 401 错误：protocolVersion 缺失/低于 "3" 时返回，reasonCode=`PROTOCOL_VERSION_REJECTED`。

Web Push 唤醒：

```text
GET  /api/push/vapid-public-key
POST /api/execution-stations/push-subscription
```

推送消息类型：

```text
collection_task_available
collection_task_control
```

说明：
- 插件注册 push 订阅时仍要携带执行工位身份和插件授权，工作台会校验 station token 与授权归属。
- `collection_task_available` / `collection_task_control` 只用于叫醒 Background；收到后插件立即运行既有任务检查，真正的任务内容仍从 `/sync` 的 `reservations[]` 与 `start_job` 结果读取。
- 如果工作台未配置 VAPID、浏览器不支持 push、订阅过期或 push 发送失败，插件继续按低频对账节奏检查任务。

授权与自动工位说明：

- `authorizationCode` 由内容工作台“设置 → 插件授权”生成，决定谁可以使用插件
- `stationKey` 是插件本地为当前浏览器生成的稳定身份，授权连接时会一并发给工作台，用来自动创建或复用工位
- `pairingCode` 用于配对码人工绑定场景，不是日常连接流程
- 插件完成授权后，后续工作台请求统一携带 `Authorization: Bearer <authorizationToken>`

控制动作：

```text
pause | resume | stop | delete
```

说明：
- `delete` 对插件执行端映射为本地 `stop`；软删除事实由内容工作台负责。
- 插件应用控制后通过 ingest 写入 `task.control_applied`，失败则写入 `task.control_failed`。
- 插件本地控制按钮仍保留，通过 `WORKBENCH_LOCAL_CONTROL_EVENT` 写回同一条任务事件流。

2.0.95 XHS 终态写入路径：

```text
Content/platform runtime → Background final result package
workflow persisted captureReport → runtime mapper → Background outbox
Background → POST /api/v2/evidence/execution → EvidenceIngress
```

插件写入：

```text
POST /api/v2/evidence/execution
body = CaptureSubmissionV2
```

delta envelope：

```json
{
  "protocolVersion": "v1",
  "taskId": "task_123",
  "pluginRunId": "run_123",
  "executorInstanceId": "plugin_profile_uuid",
  "attemptId": "attempt_123",
  "leaseToken": "lease_token",
  "leaseEpoch": 3,
  "pageFingerprint": {
    "platform": "xhs",
    "url": "https://www.xiaohongshu.com/user/profile/..."
  },
  "cursor": "local-outbox-seq-42",
  "events": [],
  "records": [],
  "snapshot": {}
}
```

终态规则：
- XHS `snapshot.status` 为终态时必须携带已经持久化并校验通过的 `captureSubmissionV2`；缺包禁止请求 Evidence route。
- `records: []` 只有在 producer 明确持久化 `observed + emitted=0` 时才是合法 Evidence；不能从空数组推断 `absent/unavailable/observed`。
- 已分类失败可持久化 `invalid` slots 与封闭 reason 后提交失败 Evidence；无法证明的 mapping failure 不进 outbox、不写 V1 Raw，通过唯一失败控制路径释放 lease，且不能报告 success。
- 页面运行期只允许写进度/心跳；六个新架构 profile 必须等最终结果包聚合完成后一次性提交 V2 Evidence，禁止双写、fallback 或重新接回 `commit_raw_snapshot`。

事件类型：

```text
task.claimed
task.page_opened
task.execution_started
task.first_record_seen
task.page_open_failed
task.login_required
task.platform_restricted
task.started
task.running
task.heartbeat
task.progress
task.partial_result
task.control_requested
task.control_applied
task.control_failed
task.paused
task.resumed
task.stopping
task.stopped
task.completed
task.succeeded
task.released
task.failed
task.deleted
task.capability_mismatch
```

能力拒收：

`task.capability_mismatch` 表示插件已经完成页面自检，但当前页面不能执行本任务。payload 至少包含 `taskType`、`reasonCode`、`reasonMessage`、`status`；如果插件拿到了页面报告，还会附带 `reportUrl`、`reportMode`、`reportPageType`、`readinessReady`、`readinessReasonCode`、`readinessReasonMessage`、`capabilityTaskTypes`，用于排查“页面已删除 / 页面未就绪 / 页面类型不对 / 任务类型缺失”的具体差异。

明确不可执行的页面错误会终止任务，不再重新排队：`content_not_found`、`error_page`、`page_permission_denied` 对应任务状态 `failed`。可恢复的执行环境问题仍保持释放语义：例如 `page_context_unavailable` 继续通过 `task.released` 回到稍后重试。

运行指标：

`task.progress`、`task.completed`、`task.succeeded`、`task.failed`、`task.released` 等事件的 `payload.observability` 会携带插件侧运行摘要，用于工作台统一日志与告警排查。该字段只放计数和阶段信息，不放目标链接、正文或完整页面数据。

```json
{
  "durationMs": 12500,
  "taskType": "xhs.batchNotes",
  "taskStrategy": "author_baseline",
  "status": "failed",
  "stage": "collecting",
  "parseAttemptCount": 20,
  "parseFailureCount": 2,
  "parseFailureRate": 0.1,
  "schemaValidationAttemptCount": 20,
  "schemaValidationFailureCount": 1,
  "schemaValidationFailureRate": 0.05,
  "recordSchemaFailed": true,
  "recordType": "note",
  "invalidRecordField": "payload",
  "itemAttemptCount": 50,
  "itemFailureCount": 3,
  "reasonCode": "page_data_not_ready",
  "report": true
}
```

记录类型：

```text
note | comment | author | media
```

记录 payload 最小校验：

| 类型 | 最小要求 |
|------|----------|
| `note` | 有稳定内容 ID 或 URL，且有可见正文、标题或媒体 |
| `comment` | 有评论 ID、父级内容 ID、评论文本 |
| `author` | 有稳定作者 ID 或主页 URL |
| `media` | 有资产 ID、URL 或本地路径 |

不满足最小结构的记录不会进入 outbox；插件会把任务转为失败，并通过 `observability` 上报 `recordSchemaFailed / invalidRecordField / reasonCode`。

小红书详情笔记的附带评论采集：

- 内容工作台下发 `xhs.batchNotes` 且 payload 携带 `includeComments=true` / `collectComments=true` 时，插件必须在采集当前笔记正文、指标和公开评论数后，继续采集当前作品 30 条以内评论；不能改走只采正文的单篇路径。
- 笔记公开评论数不足 20 条时，预期评论数等于公开评论数；公开评论数明确为 0 时，附带评论结果应回传 `publicCommentCount: 0` 且不标失败。
- 如果公开评论数大于 0，但实际没有带回评论正文，附带评论结果必须带 `error: "comments_empty_after_request"`；如果实际评论数少于预期，应带 `error: "comments_under_expected"` 和 `expectedCommentCount`。
- 评论记录仍受最小结构校验约束：缺评论正文、评论 ID 或父级内容 ID 的记录不进入 outbox。

幂等键规则：

```text
event:  {taskId}:{pluginRunId}:event:{eventType}:{controlRequestId || sequence}
record: {taskId}:{pluginRunId}:record:{recordType}:{externalRecordId || sequence}
```

### 2.8 监控任务补充

内容工作台下发监控任务时，`task.envelope` 可以携带：

```json
{
  "taskStrategy": "author_baseline",
  "payload": {
    "monitorId": "monitor_123",
    "taskStrategy": "author_baseline",
    "scanLimit": 50,
    "detailProbeLimit": 10,
    "keyword": "数学思维",
    "accountPurpose": "author_monitor"
  }
}
```

插件上报的 V1.1 接单能力：

```text
xhs.list_scan
xhs.author_links
xhs.note_full
xhs.comment_scan
xhs.author_profile
douyin.list_scan
douyin.note_full
douyin.comment_scan
douyin.author_profile
```

插件内部映射：

| 能力 | 插件执行方式 | 记录模式 |
|---|---|---|
| `xhs.list_scan` | 博主页 / 搜索页表层巡查，不进入每篇详情 | `author_surface` / `keyword_surface` |
| `xhs.author_links` | 深度建档第一阶段，在作者主页持续发现作品链接，不进入详情 | `author_links` |
| `xhs.note_full` | 打开小红书笔记详情页，一次带回正文、媒体、指标和 30 条以内评论 | `note_full` |
| `xhs.comment_scan` | 纯评论采集；`xhs.note_full` 可覆盖 | `comment` |
| `xhs.author_profile` | 读取作者主页资料 | `author_profile` |
| `douyin.*` | 抖音侧同类能力 | 对应平台记录模式 |

监控记录必须在 payload 中保留：

```text
monitorMode
monitorId
taskStrategy
monitorMeta
```

这四个字段是内容工作台把 `CollectionTaskRecord` 转成 `MonitorObservation / RadarSignal` 的桥。普通手动采集不应携带这些字段。

## 3. 平台差异说明

- 小红书批量内容/评论主要依赖页面扫描、DOM 交互与列表跳转。
- 抖音批量视频已经转为“博主页作品列表 API 驱动”。
- 抖音批量评论已经转为“作品列表 + 评论/回复接口 + 页面桥接辅助通道”。
- 抖音单条视频和评论链路存在页面桥接与页面侧 fetch，不应再假设所有能力都只靠 Content Script `fetch` 完成。
- 工作台远程任务不会直接调用 `MSG.*`；外部任务先进入 `src/workbench/protocol/*`，再由 mapper 转成内部动作。

## 4. 状态消息

| Action | Payload | 说明 |
|--------|---------|------|
| `PROGRESS` | `{ current, total, status?, taskType, taskState, phase, message?, platform?, heartbeat? }` | 任务进度更新 |
| `COLLECT_DONE` | `{ type, count, taskType?, taskState?, phase?, platform? }` | 采集完成通知 |
| `ERROR` | `{ message, taskType?, platform? }` | 错误通知（并非所有路径都统一发送） |

> 当前代码中的返回结构尚未完全统一到 `{ success, data, error }`，消费方仍需按具体 action 容错解析。

## 5. 任务状态机

```text
IDLE → RUNNING → PAUSED → RUNNING（循环）
                → STOPPING → DONE / ERROR
```

适用范围：
- 小红书批量笔记
- 小红书批量评论
- 抖音批量视频
- 抖音批量评论
- 评论图片区下载
- 内容工作台远程派单后的本地 `collectionRun`

### 2.9 Cookie 管理

| Action | 发送方 → 接收方 | Payload | 说明 |
|--------|----------------|---------|------|
| `GET_PLATFORM_COOKIES` | Popup → Background | `{}` | 提取小红书和抖音 Cookie，自动持久化到 `chrome.storage.local`，返回各平台 cookie 详情 |
| `GET_STORED_PLATFORM_COOKIES` | Popup → Background | `{}` | 读取已保存的 Cookie 状态（不重新提取） |
| `getDocumentCookie` | Background → Content | `{}` | Content script 返回 `document.cookie`，作为 cookies API 不可读时的页面侧读取通道 |

## 6. 错误处理规则

- 用户可读错误优先，避免直接暴露底层技术术语
- 单条失败不阻断整批任务
- 停止任务时必须恢复媒体规则并清理页面状态
- 当前技术债：结果 envelope 仍未统一，后续应逐步收口为 `{ success, data, error }`
- 当前产品化空缺：工作台 HTTP 轮询与 patch 的鉴权边界仍未完成统一收口，接入生产环境前必须重新核对

## 7. XHS V2 CollectionContract（B1-B-03-R1）

V2 的六个小红书采集合同和脱敏 `CaptureSubmissionV2` fixtures 位于
`src/workbench/protocol/v2/xhs-contracts.cjs`。该文件是合同定义与 fixture 的插件侧来源真值；内容工作台会镜像合同并在跨仓校验中实际运行其 EvidenceIngress validator。V2 首期将媒体作为 `media_inventory` artifact，不再把 V1 `media` 作为 RawRecord。

## 8. XHS 终态到 CaptureSubmissionV2 运行边界（B1-B-12）

`src/workbench/protocol/v2/xhs-terminal-mapper.cjs` 已由 `taskPoller` 的 XHS 终态路径调用。真实 workflow 在 collection run 中持久化 `captureReport`，`resultPackager` 原样携带，mapper 只做运行时校验和转录，终态 outbox 只提交 V2 Evidence route。

- `captureId` 与 `executionPlanVersion` 必须来自服务端 reservation；插件不得自行生成或接受旁路覆盖。
- `captureTerminal`、`slotReports`、`captureCounters` 必须由各自采集源显式持久化；0 条结果可以是 producer 明确证明的 `observed + emitted=0`。无法证明的终态不得形成 Evidence，而走唯一非 Evidence 失败控制路径终结任务和释放 lease，且不写 V1 Raw。
- `targetKey` 来自服务端执行计划，`collectorVersion` 来自运行中的插件版本；缺失均拒绝。
- 当前 run 的媒体记录只编码为该包的 `media_inventory` Artifact；固定 fixture 媒体只用于合同 hash fixture，不能进入运行映射。
- 当前路径没有 V1/V2 双写或 fallback；本仓候选尚未发布、部署或升级九工位。

## 9. XHS V2 来源合同（B3-CONTRACT-SRC-001 / B3-MEDIA-SRC-001）

`src/workbench/protocol/v2/xhs-source-contract.cjs` 是插件侧来源真值，内容工作台持有独立镜像并在跨仓检查中比较 canonical JSON。

- `xhs.record-payload/v2`：note 必须同时提供相等的 `noteId/platformContentId`，author 必须同时提供相等的 `authorId/platformAuthorId`；note 类型唯一读取 `type`，只允许 `normal/video`，旧别名与未知类型拒绝。
- `xhs.media-inventory/v2`：候选只接受同批 emitted record 中存在的 note subject；comment media 与 author/avatar 在真实 producer/Artifact 来源审计完成前均拒绝。producer 必须从平台明示的图片序位或媒体自身序位持久保留非负 ordinal，不得从下载队列位置、assetId、文件名或 URL 补造。稳定 `slotId` 必须精确等于 `subjectKey:purpose:kind:ordinal`；候选还必须包含 observedAddress 与独立 cover provenance，未知键、跨 subject、重复/不匹配 slot 和无证明封面均拒绝。
- 六个 XHS CollectionContract 已升级为 v2，并把上述两份来源合同的 schemaVersion/canonical hash 纳入自身 definition/hash；旧 v1 header 不再代表当前来源合同。
- terminal mapper 与六份真实 `resultPackager` fixture 都经过同一严格来源 validator；运行 caller 只走 V2 终态 Evidence，缺失 producer 事实 fail-closed，不双写、不 fallback；本仓候选尚未部署或升级工位。

## 10. XHS 手动同步到 V2 Evidence

`syncManualRecordsToWorkbench` 与 `syncToFlywheel` 只向现役 `POST /api/execution-tasks/manual-import` 发送 `ingressKind=manual_import` 的 `CaptureSubmissionV2`。发送前必须以运行中插件版本、严格 XHS identity/type 和真实观测时间构建并校验完整 CapturePackage；旧 `{ source, result.records, metadata }` body 已废止，禁止 fallback。

- note、comment、author 分别绑定 `xhs.list-scan`、`xhs.comment-probe`、`xhs.author-profile`，一次用户操作可按合同产生多份独立 Evidence package，但每一份都只能包含该合同允许的 record kind。
- 只有服务端实际返回 `committed` 或 `replayed` 才算该包完成；未知响应或任一 HTTP 失败不得报告整次成功。确定性 capture identity 允许重试时复用已提交观察。
- 手动 note 中的媒体地址只作为原始 Evidence payload 保留；没有 producer Artifact 证明时不得把 URL 推断成媒体已登记、已入队或已下载。
- Douyin 手动同步未进入本次 XHS 合同，必须在联网前 fail-closed，不能误发到 XHS V2 route，也不能回退旧 body。
