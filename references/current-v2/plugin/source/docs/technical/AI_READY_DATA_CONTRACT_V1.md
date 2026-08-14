# AI Ready 数据契约 v1

> 目标：让小红书与抖音采集到的数据，不只是“能展示”，而是能稳定地进入 AI 大模型分析链路。
> 本文是后续数据结构重构、批量采集、评论采集、二次下载能力的统一设计基线。

## 1. 设计目标

AI 可用的数据契约，必须同时满足 4 件事：

1. 能明确知道“这条数据是谁、来自哪里、何时采到”
2. 能保留平台原始结构，方便后续重算
3. 能跨平台统一字段语义，方便模型直接比较
4. 能表达关系结构，尤其是评论树与作者-内容关系

## 2. 数据分层

后续所有采集结果，建议按 3 层理解：

### 2.1 Raw 原始层

保留平台原始证据，便于追溯与重算：

- `rawPayload`
- `rawDomText`
- `rawShareText`
- `rawUrl`
- `collectorVersion`

### 2.2 Canonical 标准层

对小红书与抖音做统一命名：

- `platform`
- `contentId`
- `platformContentId`
- `authorEntityId`
- `platformAuthorId`
- `publishedAt`
- `collectedAt`
- `updatedAt`

### 2.3 Derived 分析层

为后续 AI 分析直接提供特征：

- `contentType`
- `normalizedGeo`
- `engagementRate`
- `commentDepth`
- `language`
- `riskFlags`

### 2.4 Data Foundation 出站层

同步到内容工作台前，插件会把已有采集结果整理成数据地基字段。该层不做业务判断，只保证工作台能稳定识别、归并和分析：

- `standardContentCode`：内容唯一编码，格式 `cw-content:global:<platform>:<contentType>:<platformContentId>`
- `standardAuthorCode`：作者唯一编码，格式 `cw-author:global:<platform>:<platformAuthorId>`
- `contentType`：统一为 `video` / `image_text` / `note` 等工作台可识别类型
- `keywords`：由平台关键词、话题、搜索词合并去重
- `authorFans`：作者粉丝数快照
- `authorFansCollectedAt`：粉丝数采集时间
- `mediaUnderstanding`：视频转写、画面摘要、OCR 文本等素材理解结果
- `sourceRun`：本次来源运行记录，包含 `source`、`taskId`、`recordId`
- `dataFoundation`：上述字段的摘要，包含 schema 版本和证据字段可用性

这些字段会进入内容工作台的“今日低粉爆文”“爆款聚类”“证据完整性”和 Claude Agent 打标输入。

## 3. 核心表建议

## 3.1 contents

一条内容就是一条笔记或视频。

| 字段 | 说明 |
|------|------|
| `contentId` | 全局主键，建议格式：`{platform}_{platformContentId}` |
| `platform` | `xhs` / `douyin` |
| `platformContentId` | 平台原始内容 ID |
| `standardContentCode` | 内容工作台数据地基编码 |
| `url` | 当前内容链接 |
| `canonicalUrl` | 去噪后的标准链接 |
| `contentType` | `image_post` / `video_post` / `mixed_post` |
| `title` | 清洗后的标题 |
| `bodyText` | 清洗后的正文 |
| `hashtags` | 话题列表 |
| `mentions` | @对象列表 |
| `keywords` | 平台原始关键词 |
| `sourceRun` | 本次回传来源运行记录 |
| `dataFoundation` | 数据地基出站摘要 |
| `topicIds` | 平台原始话题 ID |
| `authorEntityId` | 关联到 authors 表 |
| `authorName` | 作者名快照 |
| `coverUrl` | 采集到的平台原始封面来源；同步时作为唯一媒体账本的 `source_cover` 登记依据，不替换为展示地址 |
| `imageUrls` | 有序的实际图片来源列表；每个位置只代表一张图片，工作台按顺序登记 `source_image` 关系。同一图片的主、备地址只能选一个确定来源，备用地址仅保留在 `rawData.imageCandidates`，不得扩展为另一张图片。 |
| `videoUrl` | 已选定的视频原始来源；视频内容必须有该字段，工作台登记为 `source_video`，缺失即为可见的采集契约异常 |
| `videoStreams` | 仅允许留在 `rawData` 的本地采集候选流，用于本地下载和排障；不得作为跨端笔记的顶层业务字段，不替代 `videoUrl` |
| `livePhotoStreams` | 小红书 Live 图备选流列表，用于采集后或数据面板按 Live 类型下载 |
| `stats.likes` | 点赞数 |
| `stats.comments` | 评论数 |
| `stats.collects` | 收藏数 |
| `stats.shares` | 分享数 |
| `stats.plays` | 播放数 |
| `location.ipLocation` | 内容展示 IP 属地 |
| `location.genderTaggedLocation` | 类似“广东男”这种原始值，单独保存 |
| `publishedAt` | 标准发布时间戳 |
| `publishedAtText` | 原始展示时间 |
| `collectedAt` | 采集时间戳 |
| `updatedAt` | 最近更新时间 |
| `dataSource` | `dom` / `api` / `render` / `share` |
| `triggerSource` | `manual` / `native_share` / `batch_profile` |
| `collectionRunId` | 归属的采集任务 |
| `mediaDownloadStatus` | `无媒体` / `待下载` / `下载中` / `已完成` / `部分失败` / `失败` |
| `rawRef` | 指向 raw 证据的引用 |

## 3.2 comments

评论表必须支持分析评论树，而不只是平铺文本。

| 字段 | 说明 |
|------|------|
| `commentEntityId` | 全局主键，建议：`{platform}_{contentId}_{commentId}` |
| `platform` | 平台标识 |
| `commentId` | 平台原始评论 ID |
| `contentId` | 关联 contents |
| `contentTitle` | 当前内容标题快照 |
| `contentUrl` | 当前内容链接 |
| `authorEntityId` | 评论作者统一 ID |
| `authorName` | 评论作者名称 |
| `text` | 评论文本 |
| `likeCount` | 评论点赞数 |
| `ipLocation` | 评论 IP 属地 |
| `publishedAt` | 评论发布时间戳 |
| `publishedAtText` | 原始时间文案 |
| `parentCommentId` | 直接父评论 ID |
| `rootCommentId` | 所属主评论 ID |
| `level` | `1` 主评论 / `2` 二级评论 |
| `replyToCommentId` | 回复目标评论 ID |
| `replyToUserName` | 回复目标用户名 |
| `positionIndex` | 在当前抓取结果中的顺序 |
| `sortMode` | `hot` / `latest` / `unknown` |
| `collectedAt` | 采集时间 |
| `collectionRunId` | 所属批量任务 |
| `rawRef` | 原始证据引用 |

## 3.3 authors

作者表既要满足展示，也要满足后续画像分析。

| 字段 | 说明 |
|------|------|
| `authorEntityId` | 全局主键，建议：`{platform}_{platformAuthorId}` |
| `platform` | 平台标识 |
| `platformAuthorId` | 平台作者原始 ID |
| `profileUrl` | 作者主页链接 |
| `handle` | 平台账号名，如小红书号 / 抖音号 |
| `secUserId` | 抖音 sec_user_id（如适用） |
| `name` | 作者昵称 |
| `badge` | 认证/身份标记 |
| `ipLocation` | IP 属地 |
| `gender` | 标准化性别 |
| `stats.fans` | 粉丝数 |
| `stats.follows` | 关注数 |
| `stats.interactions` | 总互动数 / 总获赞 |
| `stats.postCount` | 发帖数 |
| `followedByMe` | 当前账号是否关注 |
| `accountStatus` | 账号状态 |
| `collectedAt` | 采集时间 |
| `rawRef` | 原始证据引用 |

字段语义补充约定：

- `handle` 是跨平台统一字段，后续 UI、导出、分析默认都应优先消费它。
- `redId` 仅作为小红书历史兼容字段保留，不再作为跨平台展示主字段。
- `douyinId` 仅作为抖音历史兼容字段保留，不再作为跨平台展示主字段。

## 3.4 collection_runs

这张表非常关键，用来回答“这批数据是怎么采来的”。

| 字段 | 说明 |
|------|------|
| `collectionRunId` | 主键 |
| `platform` | 平台 |
| `taskType` | `single_video` / `single_comments` / `batch_videos` / `batch_comments` / `author` |
| `pageType` | `profile` / `detail` / `modal` / `search` |
| `triggerSource` | `manual` / `native_share` / `batch_profile` |
| `targetCount` | 目标采集数 |
| `commentLimitPerContent` | 每条内容评论上限 |
| `commentSortMode` | 评论排序模式 |
| `status` | `running` / `paused` / `done` / `partial_failed` / `failed` |
| `successCount` | 成功数 |
| `failedCount` | 失败数 |
| `startedAt` | 开始时间 |
| `finishedAt` | 结束时间 |
| `failureItems` | 失败明细 |

## 3.5 media_assets

用于“批量采集后，数据面板还能二次下载”。

| 字段 | 说明 |
|------|------|
| `assetId` | 主键 |
| `contentId` | 关联 contents |
| `assetType` | `cover` / `image` / `video` / `comment_image` |
| `role` | `primary` / `fallback` |
| `url` | 当前可用链接 |
| `candidateUrls` | 候选链接列表 |
| `quality` | `HD` / `SD` / `unknown` |
| `downloadStatus` | 下载状态 |
| `lastResolvedAt` | 最近一次重新解析直链时间 |

## 3.6 XHS V2 暗态来源合同

`xhs.record-payload/v2` 是进入内容工作台 V2 Derived 前的来源门禁，不替代本文件的 V1
展示/导出字段：

- note 必须同时携带非空 `noteId` 与 `platformContentId`，且二者相等；`type` 是内容类型的唯一来源，只允许 `normal` / `video`，不得从别名、URL 或媒体字段猜测。
- author 必须同时携带非空 `authorId` 与 `platformAuthorId`，且二者相等。
- comment 必须携带非空 `noteId` 与 `commentId`。
- 任一身份不完整或不相等，必须在 ContractEvaluation accepted 之前拒绝，不能推进后续 Current 指针。

`xhs.media-inventory/v2` 只描述已观察媒体候选，固定字段为
`subject / slotId / purpose / kind / ordinal / observedAddress / coverProvenance`：

- `slotId` 只由稳定 subject、purpose、kind、ordinal 决定；会过期的 URL 不参与身份。
- subject 当前只允许同包已发出的 note；comment media 与 author/avatar 没有审计完成的 producer/Artifact 来源，必须拒绝。
- `ordinal` 必须由媒体 producer 从平台明示的图片序位或媒体自身序位持久保留且非负；不得从下载队列位置、assetId、文件名或 URL 补造。
- cover 来源必须显式区分 `platform_explicit` 与 `first_observed_image`；`role=cover` 本身不是平台明确封面的证明。
- 媒体 subject 必须能在同一包的 note 记录中找到；`slotId` 必须精确等于 `subjectKey:purpose:kind:ordinal`，重复/不匹配 slot、未知字段或歧义 cover 均拒绝。

两份合同已用于 XHS 终态 runtime mapper 与 V2 Evidence outbox；workflow 持久化来源报告，mapper 只校验/转录。
插件仍不直接写 Canonical、Projection 或 Media Domain。

## 4. 当前版本与目标版本的差距

### 4.1 已具备

- `notes / comments / authors` 三张主表
- 平台区分字段 `platform`
- 小红书笔记的 `topicIds / atUserList / hashtags`
- 抖音视频的 `shareShortUrl / triggerSource / dataSource`
- 基础评论父子关系 `parentCommentId`

### 4.2 尚未完全具备

- 历史记录的 AI-ready 字段持久化回填仍未全量执行
- `collection_runs` 已落地，但成功/失败明细和更细粒度任务阶段仍有深化空间
- `media_assets` 已落地，但仍在从评论图片区扩展到更多内容媒体
- 统一作者主键语义已在新数据与导出层开始收口，旧记录和部分 UI 仍有兼容负担
- 原始证据层已接入核心采集器，但更多平台媒体与历史记录尚未完全补齐
- 标准化发布时间戳已进入主链路，但旧记录与部分评论/作者场景仍需继续清洗

## 5. 与当前业务需求的直接关系

本轮讨论里确认的功能，都依赖这份契约：

- 抖音单条评论采集
- 抖音批量评论采集（前 5 / 10 / 20 条视频）
- 评论上限由用户填写，不填表示全部
- 包含二级评论
- 必须知道每条评论属于哪条视频
- 必须保留父评论与子评论层级
- 抖音评论区图片高清下载
- 抖音批量视频采集后，数据面板要支持二次下载视频

## 6. 实施优先级

### P0

- `contents` 与 `comments` 的统一主键与关系字段
- `collection_runs`
- 评论层级字段
- `media_assets` 的最小实现（至少覆盖视频与评论图片）

### P1

- 原始证据层
- 更完整的时间标准化
- 跨平台作者实体清洗
- 历史记录批量回填与一致性校验

### P2

- 派生分析字段自动计算
- AI 分析专用导出模板
