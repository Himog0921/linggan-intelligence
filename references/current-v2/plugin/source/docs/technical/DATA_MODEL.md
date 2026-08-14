# 数据模型

> 插件当前仍以本地 IndexedDB（Dexie）作为执行事实源。
> 同时，插件已支持通过 HTTP 与内容工作台交换任务和导入结果；但本文描述的“数据层”仍特指插件本地存储结构。
> 本文描述的是当前代码中的本地存储结构。它已经开始向 AI-ready 契约靠拢，但仍不完全等同于最终分析层模型。

## 数据库

- 数据库名：`LingganBoomDB`
- ORM：Dexie.js v4.3.0
- Schema 版本：v13（已引入远程任务映射、增量 outbox、账号表和按任务打包索引）

## 表结构

### notes

| 字段 | 类型 | 说明 |
|------|------|------|
| `noteId` | string (PK) | 笔记唯一 ID |
| `contentId` | string | 统一内容实体 ID |
| `platformContentId` | string | 平台原生内容 ID（如抖音 awemeId） |
| `platform` | string | 平台标识：`xhs` / `douyin` |
| `url` | string | 笔记链接 |
| `title` | string | 标题 |
| `content` | string | 正文 |
| `type` | string | normal / video |
| `cover` | string | 封面 URL |
| `coverProvenance` | string | 封面候选来源：`platform_explicit` / `first_observed_image`；仅证明采集来源，不代表已被 Canonical 接受 |
| `images` | string[] | 图片 URL 列表 |
| `video` | string | 视频流 URL |
| `likes` | number | 点赞数 |
| `collects` | number | 收藏数 |
| `comments` | number | 评论数 |
| `shares` | number | 分享数 |
| `keywords` | string[] | 标签列表 |
| `topicIds` | string[] | 话题 ID 列表 |
| `atUserList` | object[] | @用户列表（`[{userId,nickname}]`） |
| `authorId` | string | 作者 ID |
| `authorEntityId` | string | 统一作者实体 ID |
| `authorName` | string | 作者名 |
| `authorAvatar` | string | 作者头像 |
| `releaseDate` | string | 发布日期 |
| `publishedAt` | number | 内容发布时间时间戳 |
| `collectedAt` | number | 本次采集时间戳 |
| `lastUpdateTime` | string | 最后修改时间 |
| `ipLocation` | string | 笔记 IP 属地 |
| `shareRestricted` | boolean | 是否限制分享 |
| `authorFollowed` | boolean | 当前账号是否关注作者 |
| `createdAt` | number | 采集时间戳 |
| `syncStatus` | string | 同步状态 |
| `lastSyncAt` | number | 最后同步时间 |
| `mediaQuality` | string | 媒体质量等级 |
| `hashtags` | string[] | 话题列表（跨平台统一字段，部分历史数据为空） |
| `videoPlayUrl` | string | 视频播放直链（抖音/视频笔记常见） |
| `videoDownloadUrl` | string | 视频下载直链（可能会过期） |
| `videoStreams` | object[] | 视频候选流列表；小红书会保留 `h266 / h265 / h264 / av1` 主链与备用链接 |
| `imageCandidates` | string[][] | 按图片槽位保存的高清候选链接；同一槽位内是主、备地址，不是多张图片。仅作本地下载和原始证据，不得在跨端同步时展开为额外 `imageUrls`。 |
| `imageCandidateSlots` | `{ordinal:number\|null,candidates:string[]}[]` | 小红书 producer 从平台 `imageList` 的图片序位形成并跨运行边界持久保留；候选 URL 只属于该槽位，不决定槽位身份。legacy 记录没有已证明序位时必须保留 `ordinal=null`，V2 映射 fail closed，不得由下载队列位置补造。 |
| `livePhotoStreams` | object[] | 小红书 Live 图候选流列表，按图片序号保留主链与备用链接 |
| `mediaDownloadStatus` | string | 媒体下载状态 |
| `dataSource` | string | 数据来源，例如 `dom` / `render` / `api` |
| `triggerSource` | string | 触发来源，例如 `manual` / `native_share` / `batch_profile` |
| `shareText` | string | 分享文案原文（抖音） |
| `shareShortUrl` | string | 分享短链（抖音） |
| `shareCapturedAt` | number | 分享文案采集时间 |
| `collectorVersion` | string | 采集器版本标记 |
| `rawPayload` | string | 原始结构化载荷的截断序列化结果 |
| `rawDomText` | string | 原始 DOM 文本快照 |
| `rawShareText` | string | 原始分享文案快照 |
| `rawUrl` | string | 原始采集 URL |
| `rawSource` | string | 原始来源，如 `api` / `dom` / `render` / `share` |

### comments

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | number (PK, auto) | 自增主键 |
| `commentEntityId` | string | 统一评论实体 ID |
| `commentId` | string | 评论唯一标识 |
| `platform` | string | 平台标识：`xhs` / `douyin` |
| `contentId` | string | 所属内容统一 ID |
| `noteId` | string | 所属笔记 ID |
| `noteUrl` | string | 所属笔记链接 |
| `text` | string | 评论文字 |
| `author` | string | 评论作者 |
| `profileUrl` | string | 作者主页链接 |
| `location` | string | IP 属地 |
| `ipLocation` | string | IP 属地（新字段，保留 `location` 兼容旧导出） |
| `avatarUrl` | string | 评论者头像 URL |
| `authorId` | string | 评论者 userId |
| `likes` | number | 点赞数（v2 新增） |
| `parentCommentId` | string | 父评论 ID（子评论用） |
| `rootCommentId` | string | 评论树根评论 ID |
| `level` | number | 层级（1=主评论，2=子评论） |
| `replyToCommentId` | string | 当前评论直接回复的评论 ID |
| `replyToUserName` | string | 当前评论直接回复的用户名 |
| `time` | string | 评论时间 |
| `publishedAt` | number | 评论发布时间时间戳 |
| `collectedAt` | number | 评论采集时间戳 |
| `sortMode` | string | 评论排序方式 |
| `collectionRunId` | string | 所属采集任务 ID |
| `createdAt` | number | 采集时间戳 |
| `syncStatus` | string | 同步状态 |
| `collectorVersion` | string | 采集器版本标记 |
| `rawPayload` | string | 原始结构化载荷的截断序列化结果 |
| `rawDomText` | string | 原始 DOM 文本快照 |
| `rawShareText` | string | 原始分享文案快照 |
| `rawUrl` | string | 原始采集 URL |
| `rawSource` | string | 原始来源，如 `comment_api` / `comments.dom` |

### authors

| 字段 | 类型 | 说明 |
|------|------|------|
| `userId` | string (PK) | 用户 ID |
| `authorEntityId` | string | 统一作者实体 ID |
| `platformAuthorId` | string | 平台原生作者 ID |
| `platform` | string | 平台标识：`xhs` / `douyin` |
| `handle` | string | 平台账号标识（抖音号/小红书号） |
| `secUserId` | string | 抖音 secUid 等安全 ID |
| `redId` | string | 小红书号 |
| `name` | string | 博主名称 |
| `avatar` | string | 头像 URL |
| `location` | string | IP 属地 |
| `ipLocation` | string | IP 属地（结构化字段） |
| `gender` | number | 性别（0未知/1女/2男） |
| `accountStatus` | string | 账号状态（如 `DEFAULT`） |
| `followedByMe` | boolean | 当前账号是否关注该博主 |
| `description` | string | 个人简介 |
| `keywords` | string[] | 标签列表 |
| `follows` | number | 关注数 |
| `fans` | number | 粉丝数 |
| `interactions` | number | 获赞与收藏数 |
| `profileUrl` | string | 主页链接（v3 新增） |
| `collectedAt` | number | 作者采集时间戳 |
| `createdAt` | number | 采集时间戳 |
| `syncStatus` | string | 同步状态 |
| `lastSyncAt` | number | 最后同步时间 |
| `collectorVersion` | string | 采集器版本标记 |
| `rawPayload` | string | 原始结构化载荷的截断序列化结果 |
| `rawDomText` | string | 原始 DOM 文本快照 |
| `rawShareText` | string | 原始分享文案快照 |
| `rawUrl` | string | 原始采集 URL |
| `rawSource` | string | 原始来源，如 `profile-api` / `userPageData+dom` |

> 字段语义约定：
> - `handle` 是跨平台统一展示字段，表示“小红书号 / 抖音号 / 平台账号名”。
> - `redId` 仅保留为小红书兼容字段，后续 UI 与导出应逐步转向 `handle`。
> - `douyinId` 作为抖音平台特有兼容字段可继续存在于采集结果中，但分析层与导出层应优先读 `handle`。

### collectionRuns

| 字段 | 类型 | 说明 |
|------|------|------|
| `collectionRunId` | string (PK) | 采集任务唯一 ID |
| `externalTaskId` | string | 内容工作台侧任务 ID |
| `externalTaskType` | string | 内容工作台侧任务类型，如 `xhs.batchNotes` |
| `executorInstanceId` | string | 执行实例标识（预留给多实例/执行节点） |
| `protocolVersion` | string | workbench 协议版本 |
| `platform` | string | 平台标识 |
| `taskType` | string | `single_note` / `batch_notes` / `single_comments` / `batch_comments` 等 |
| `pageType` | string | 启动任务时的页面类型 |
| `triggerSource` | string | 触发来源 |
| `status` | string | `running / paused / stopped / done / failed` |
| `resultUploadStatus` | string | 结果上传状态，如 `local_only / pending_upload / packaged` |
| `lastHeartbeatAt` | number | 最近一次心跳时间戳 |
| `config` | object | 本次任务配置快照 |
| `meta` | object | 页面 URL 等额外上下文 |
| `processedCount` | number | 批量任务已处理数量 |
| `nextIndex` | number | 批量任务下一条目标序号 |
| `resumeCheckpoint` | object | 批量断点续跑快照，包含目标顺序、已处理数量和结果状态 |
| `latestSummary` | object | 最近一次可读进度摘要 |
| `error` | string | 失败原因摘要 |
| `startedAt` | number | 开始时间 |
| `finishedAt` | number | 结束时间 |
| `updatedAt` | number | 最近更新时间 |
| `createdAt` | number | 创建时间 |

### mediaAssets

| 字段 | 类型 | 说明 |
|------|------|------|
| `assetId` | string (PK) | 资产唯一 ID |
| `contentId` | string | 所属内容统一 ID |
| `collectionRunId` | string | 所属采集任务 ID |
| `assetType` | string | `image` / `video` / `comment_image` 等 |
| `role` | string | `cover` / `body` / `comment` / `avatar` 等 |
| `ordinal` | number | producer 从平台图片序位或媒体自身序位持久保留的同 subject/purpose/kind 稳定零基顺序；不得从下载队列位置、assetId、文件名或 URL 补造 |
| `candidateUrls` | string[] | 同一媒体候选的可观察地址；地址可过期，不参与稳定槽位身份 |
| `coverProvenance` | string | 仅 cover 使用：`platform_explicit` / `first_observed_image`；不得从 `role=cover` 反推 |
| `quality` | string | `origin` / `download` / `medium` / `thumb` 等 |
| `downloadStatus` | string | 下载状态 |
| `lastResolvedAt` | number | 最近一次刷新/解析时间 |
| `createdAt` | number | 创建时间 |

> V2 来源合同补充（`xhs.media-inventory/v2`）：terminal mapper 只把上述真实媒体事实映射为
> `subject + slotId + purpose + kind + ordinal + observedAddress + coverProvenance`。`slotId`
> 必须精确等于 `subjectKey:purpose:kind:ordinal`，URL 不参与身份；candidate 必须属于同一 CapturePackage
> 已发出的 note。comment media 与 author/avatar 当前没有可审计的 producer/Artifact 来源，因此 V2 明确
> 拒绝这些分支。producer 必须持久保留平台图片序位或媒体自身序位；不得从下载队列位置或 assetId
> 反推。该合同已接 XHS 终态 V2 Evidence caller，但插件不创建 CanonicalMediaSlot 或 Media Domain 关系。

### workbenchOutbox

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string (PK) | 本地队列行 ID |
| `taskId` | string | 内容工作台任务 ID |
| `pluginRunId` | string | 插件本地执行 ID |
| `idempotencyKey` | string (unique) | 幂等键，防止重复事件 / 记录重复写入 |
| `kind` | string | `event` / `record` |
| `status` | string | `pending / in_flight / failed / failed_terminal / acked` |
| `payload` | object | 待上传事件或记录 |
| `snapshot` | object | 可选任务快照 |
| `sequence` | number | 本地顺序号 |
| `attemptCount` | number | 上传尝试次数 |
| `nextAttemptAt` | number | 下次允许上传时间 |
| `errorMessage` | string | 最近一次失败原因 |
| `executionContext` | object | 冻结的执行身份（`attemptId / leaseToken / leaseEpoch / stationId / accountId / platform / pageFingerprint`）。2026-07-13 事故根因：此前 normalize 落库时丢弃该字段，严格校验在发 HTTP 前把行判成 `failed_terminal`。必须原样持久化（有 fake-indexeddb 回归测试保护） |
| `createdAt` | number | 创建时间 |
| `updatedAt` | number | 最近更新时间 |

状态语义（2026-07-13 分账，2026-07-15 熔断修正）：`pending / in_flight / failed`（可重试）计入「待发送」；`failed_terminal` 是死信——不再自动发送、不计入待发送，也不计入 `OUTBOX_BACKLOG_BLOCKED` 的数量阈值，只能经恢复导出 → `plugin_local_recovery` 找回。`countsByStatus()` 提供分桶计数与最老时间，`listDeadLetters()` 供导出。

性能语义（v15）：`countsByStatus()` 的最老时间通过 `[status+createdAt]` 复合索引逐状态取首行，禁止对 outbox 使用全量 `toArray()`；避免 Popup 诊断轮询和接单 tick 在大积压时反复反序列化 payload/snapshot。

### captureJournal

采集事实账本（2026-07-13 架构升级，报告 §9.2）：record 类采集内容在入 outbox 之前先落此表，与租约有效性解耦，append-only 不可变；即使出站行被判死信，事实仍在。修剪只清「已被服务端 ACK + 超 14 天」条目（background 每 6 小时）。

| 字段 | 类型 | 说明 |
|------|------|------|
| `entryId` | string (PK) | = outbox 行的 `idempotencyKey` |
| `taskId` | string | 内容工作台任务 ID |
| `pluginRunId` | string | 插件本地执行 ID |
| `kind` | string | 目前只journal `record` |
| `recordType` | string | `note / comment / author` 等 |
| `externalRecordId` | string | 平台侧记录 ID |
| `payload` | object | 采集内容原文 |
| `payloadHash` | string | canonical JSON 的 sha-256 hex |
| `capturedAt` | number | 采集时间 |
| `executionContext` | object | 冻结执行身份（同 outbox） |
| `pluginVersion` | string | 采集时插件版本 |
| `createdAt` | number | 写入时间 |

### accounts

| 字段 | 类型 | 说明 |
|------|------|------|
| `accountId` | string (PK) | 本地平台账号 ID |
| `name` | string | 账号展示名 |
| `status` | string | 当前账号可用状态 |
| `platform` | string | 平台标识 |
| `lastUsedAt` | number | 最近使用时间 |
| `createdAt` | number | 创建时间 |

## 数据约束

- 同一笔记以 `noteId` 去重（覆盖更新）
- 评论通过 `commentId + noteId` 去重（更新旧记录）
- 博主通过 `userId` 去重（覆盖更新）
- 批量任务通过 `collectionRunId` 去重
- 媒体资产通过 `assetId` 去重
- 增量 outbox 通过唯一 `idempotencyKey` 去重
- 批量任务通过 `resumeCheckpoint.targetIds + nextIndex` 恢复，不重复处理已完成目标

## Schema 迁移历史

| 版本 | 变更 |
|------|------|
| v1 | 初始 schema：notes + comments + authors |
| v2 | comments 表新增 `likes` 字段 |
| v3 | authors 表新增 `profileUrl` 字段 |
| v4 | 新增数据探查字段索引：`notes.ipLocation/lastUpdateTime/authorFollowed/shareRestricted`、`comments.ipLocation/authorId`、`authors.ipLocation/gender/accountStatus/followedByMe` |
| v5 | 多平台支持：`notes / comments / authors` 全部新增 `platform` 字段索引 |
| v6 | AI-ready 基线：新增 `contentId/platformContentId/authorEntityId`、评论树结构字段、`collectionRuns`、`mediaAssets` |
| v7 | `mediaAssets` 新增 `collectionRunId` 索引，支持按任务追溯评论图片区与后续内容媒体资产 |
| v8 | `collectionRuns` 新增 `externalTaskId / externalTaskType / executorInstanceId / protocolVersion / resultUploadStatus / lastHeartbeatAt`，支持内容工作台远程任务映射 |
| v9 | 新增 `workbenchOutbox`，支持工作台事件 / 记录增量上传、离线重试和幂等写入 |
| v10 | `workbenchOutbox` 新增 `[status+nextAttemptAt+createdAt]` 复合索引，避免待上传扫描退化 |
| v11 | 新增 `accounts`，支持平台账号状态和多账号执行管理 |
| v12 | `workbenchOutbox.idempotencyKey` 改为唯一索引，并在升级时清理重复 outbox 行 |
| v13 | `notes / authors` 新增 `collectionRunId` 索引，避免按任务打包结果时全表扫描 |
| v14 | 新增 `captureJournal` 采集事实账本（不可变、与租约解耦），2026-07-13 写回事故架构升级 |

## 当前模型的局限

当前结构已经足够支撑：

- 本地采集展示
- Dashboard 管理
- CSV / JSON 基础导出
- 单条内容和作者的人工查看

但它还不完全等于最终“AI-ready 数据契约”，仍有这些维护注意点：

- 并非所有历史数据都已持久化回填 `contentId / authorEntityId / publishedAt / handle / collectorVersion / raw*`
- 原始证据层已接入小红书内容/评论/作者与抖音视频/评论/作者，但历史记录尚未回填，评论图片区和更多媒体资产仍需继续补齐
- 跨平台字段语义仍在清理中，尤其是 `redId / handle / douyinId`
- 评论树字段虽然已进入 schema，且批量评论链路已开始写入，但单条评论、导出和分析层语义仍需再统一
- `collectionRuns / mediaAssets / workbenchOutbox / accounts` 已建表，并已接入批量任务、评论图片区、远程任务增量回写与账号执行锁链路
- 远程任务映射字段已经进入 `collectionRuns`，自动认领、租约、增量事件、记录回写和故障演练已有自动化覆盖；真实账号登录态下的完整页面点击采集仍需继续验收
- 现已提供“读时标准化 + 显式批量回填”双路径：
  - `src/db/recordNormalization.js`：所有 store 读写时对旧记录做运行时对齐
  - `src/db/legacyDataMaintenance.js`：需要时可一次性把历史记录持久化回填到新契约

后续目标模型请见：`AI_READY_DATA_CONTRACT_V1.md`

## chrome.storage.local 扩展存储

### platformCookies（双平台 Cookie 缓存）

| 字段 | 类型 | 说明 |
|------|------|------|
| `xhs.cookies` | object[] | 小红书 Cookie 数组，每项含 `name / value / domain / path / secure / httpOnly / sameSite / expirationDate?` |
| `xhs.cookieString` | string | 分号拼接的 `name=value` 字符串，供 HTTP 请求直接使用 |
| `xhs.count` | number | Cookie 条数 |
| `xhs.capturedAt` | string | ISO 8601 时间戳 |
| `douyin.*` | 同 xhs | 抖音平台同结构 |

通过 `chrome.storage.local.get('platformCookies')` 读取。
