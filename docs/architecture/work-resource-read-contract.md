# WORK-RESOURCE-READ-001 · Intelligence 共享作品资源读取合同

> 状态: 权威当前
> 最后核对: 2026-09-04
> 适用范围: Intelligence 中需要展示作品封面、标题、作者、发布时间、互动、材料状态与来源血缘的页面
> 事实来源: Issue #110 / #128 / #148、PR #132 / #151 review remediation、Media V2、当前 Material Projection、Browser Producer `0.8.28`、additive migrations `0026_work_resource_read.sql` / `0027_unified_media_resource.sql` / `0029_author_avatar_media.sql` / `0030_comment_image_media.sql`
> 冲突时以谁为准: 用户最新确认、不可变 Capture Package、类型化材料事实、Media V2、当前代码与数据库约束

## 1. 唯一公共入口与唯一 Current 裁定 owner

Intelligence 页面只能消费共享 `Work Resource Read` Interface：

- 列表：`GET /api/local/work-resources`
- 单品：`GET /api/local/work-resources/{publicRef}`
- 授权评论通道：`GET /api/local/work-resources/{publicRef}/comments`
- Rust Interface：`read_work_resources`、`read_work_resource`、`work_resource_schema_is_ready`

这些公共入口与详情 Inspector 共同建立在 crate-private typed `WorkResourceCurrent` batch
projection 上。它由 `material_query_sql` 中同一份 CTE/列合同逐字段裁定 title、作者、qualified
`published_at` 与四项 engagement Current，并使用固定 tie-break；列表、单品、详情 Inspector 和
creator lifecycle 都消费该裁定结果，不得各自复制 `ORDER BY`/fallback。batch seam 在调用方的同一
repeatable-read/read-only transaction 与同一 `as_of` 中读取，避免 HTTP/N+1 和不同时间切片。

Creator lifecycle 可以先用 target relationship 与详情 stable author ID 选择有界候选 Work ref，
但候选查询只决定范围；页面需要展示的标题、作者、发布时间与互动 Current 必须来自上述共享 owner。
生命周期模块只派生窗口排除、分位、中位、composite 与 scan receipt。

Issue #133 为已选中的单一 XHS Work Resource 增加一个**受限 command adjunct**，它不改变上述
read interface 的事实 owner，也不授权任何泛化采集或页面私有事实拼接：

- `POST /api/local/work-resources/{publicRef}/reobserve`：只有既有 target-linked active
  deep-archive authorization 才会记录新的 admission，并且只能沿 Work Order → Lease →
  server-issued Task 发起详情/评论/回复（评论最多 30）的无媒体复观测；
- `GET /api/local/work-resources/{publicRef}/reobserve/{leaseRef}`：只读取该精确 lease 的
  Task/Attempt/Package/Receipt 状态；`ACCEPTED` 仍不是全 lane Coverage 完整。

该 command 只调用 Linggan domain/acquisition chain；页面不得自己创建 Task、从作者/标题/URL/显示名
猜测目标或授权，也不得把 command response 当成新的 Work Resource 事实。完成后必须重新读取本节的
单品 GET，才可展示已接纳 Package 形成的当前/历史事实。

Evidence Library 是首个消费者，不是接口 owner。后续选题、创作者、观察、研究或其他页面不得另写 SQL、另读 Package JSON、另建封面/作者/时间拼接规则，亦不得以页面私有 endpoint 形成第二份事实。

`GET /api/local/collection/targets/{targetRef}/lifecycle` 是窄化的派生读面，不是第二个 Work facts
API：它只公开 target/window/as-of、Work public ref、规则版本、coverage/排除/scan receipt、分位与
滚动中位；不公开 title、author、published-at 或 engagement Current。Collection 服务端渲染可以在
进程内消费共享 typed projection，以显示图表所需的最小摘要，但这不把内部 projection 晋升为另一份
公共事实合同。

内部的 typed Material Projection、Media V2 slot/origin/blob/materialization、评论与作者版本化读取可以继续拆模块；它们对页面只通过这个小 Interface 暴露。显式 `/api/local/evidence-library/legacy` 仅为旧发现卡兼容读取，不是共享资源入口，不得成为新页面 fallback。

## 2. 一个作品资源的最小合同

```text
WorkResource
├─ identity: platform / contentExternalId / publicRef
├─ display
│  ├─ title + state
│  ├─ creatorDisplayName + state
│  ├─ publishedAt + publishedAtState
│  ├─ publishedAtSourceText / sourceField / sourceKind / precision
│  ├─ publishedAtReferenceObservedAt / parserVersion
│  └─ engagement + per-field state
├─ collectionContext
│  ├─ relationshipState
│  ├─ targetRef / targetKind / targetDisplayName + state
│  ├─ authorIdentityMatchState
│  └─ workOrderRef
├─ media: linggan.media-resource.v1
│  ├─ avatar / cover / images / video
│  ├─ ocr / transcript / commentImages
│  └─ per-resource relationship, state, local handle and intrinsic facts
├─ preview: compatibility projection only; business pages must not consume it
├─ laneSummaries[] / summary / matchedFields
└─ detail inspector: field sources, Target/WorkOrder/Task/Attempt/Package/Receipt, media and limitations
   └─ XHS reobservation operation: exact authorization/admission/lease/task status only; no media request
```

列表与详情必须使用同一字段语义。三种布局只改变排版，不改变查询、字段资格、状态或血缘。

## 3. 作者与监控目标不是同一字段

来自创作者主页监控的作品可以证明“这条作品是在该监控目标表面被观察到”，但只有作品详情中的平台作者 ID 与目标稳定 ID 一致，才能证明“该监控目标就是作品作者”。因此：

| 情况 | `relationshipState` | `authorIdentityMatchState` | UI |
|---|---|---|---|
| 创作者主页发现，详情尚无作者 ID | `OBSERVED_ON_TARGET_SURFACE` | `NOT_VERIFIED` | 显示“监控目标：木可可同学”；作品作者仍是“当前未知” |
| 详情作者 ID 与目标稳定 ID 相等 | `OBSERVED_ON_TARGET_SURFACE` | `MATCHED` | 分别显示作者和监控目标，并写明平台 ID 已证明一致 |
| 详情作者 ID 不同 | `OBSERVED_ON_TARGET_SURFACE` | `MISMATCH` | 保留两者并突出不一致，禁止覆盖作者 |
| 关键词发现 | `DISCOVERED_FOR_TARGET` | `NOT_APPLICABLE` | 显示关键词目标，不把关键词当作者 |

血缘优先使用 `Task → LeaseTask → Lease → WorkOrder → Target`；旧的手动 profile discovery 没有 lease 关系时，只允许用 TaskSpec 的稳定 `authorExternalId` 与同平台 creator target 精确匹配。显示名只作 UI 文案，不参与身份匹配。

## 4. 发布时间资格

Browser Producer 的详情解析先返回一个有来源资格的时间断言：

- `publishedAt`：归一化毫秒值；
- `publishedAtText`：来源原值文本；
- `publishedAtSourceField`：实际命中的 payload 字段，如 `publishTime`；
- `publishedAtSourceKind`：`platform_epoch | visible_text | unknown`；
- `publishedAtPrecision`：`millisecond | second | minute | day | relative | unknown`；
- `publishedAtReferenceObservedAt`：相对文本的观察参照时点；
- `publishedAtParserVersion`：当前为 `xhs-detail-time-v2`。

服务端只有在 `sourceKind=platform_epoch`、精度为秒/毫秒、字段名命中已支持的详情字段清单，且 parser version 精确为 `xhs-detail-time-v2` 时，才把值写入 `published_at`。可见的“3 小时前”“4 月 17 日”仍保留为 `SOURCE_TEXT_ONLY`；即使插件为了当前页面体验算出了一个毫秒值，服务端也不得把该派生值晋升成历史精确发布时间。

2026-08-31 的一条用户授权签名详情探针已完成：作品身份精确匹配，SSR `noteDetailMap` 顶层字段为 `time`，JavaScript 类型为 `number`，值为 13 位毫秒 epoch `1787747429000`，对应 `2026-08-26T12:30:29Z` / 北京时间 `2026-08-26 20:30:29 +08:00`；页面可见文本为“4天前 广东”。该结果精确命中现有 `time + platform_epoch + millisecond + xhs-detail-time-v2` 合同，不需要改动 Producer 映射。探针未保留签名 token、正文、作者资料、评论或媒体。

迁移只新增列，不回写、覆盖或重新解释旧 Package。旧行没有精确来源时继续是 `UNKNOWN` 或 `SOURCE_TEXT_ONLY`，不能用 `observedAt/acceptedAt` 代替发布时间。

## 5. Media V2 与统一媒体资源

媒体事实继续由 Media V2 拥有：slot → source observation/generation → candidate assertion → download attempt → blob → materialization/derivative/disposition。`0027_unified_media_resource.sql` 没有新建第二套资产表，只在既有链上增加一份 append-only 关系词汇：`author.avatar`、`content.cover`、`content.image`、`content.video`、`content.ocr`、`content.transcript`、`comment.image`。

Work Resource Read 对页面只返回一份 `linggan.media-resource.v1`：

- `cover` 只按 `explicit_cover → first_body_image → video_poster → none` 选择；`selectedBy` 与 `fallbackUsed` 必须可见；
- `images`、`video.items`、`ocr.resources`、`transcript.resources` 保留各自关系和序号；
- `intrinsicDimensions` / `durationMs` 是 Blob 事实，3:4 只是 Evidence Cover 布局策略；
- 只有 `INLINE_SAFE` 的同源 `/api/local/media/` 或 `/api/local/derivative/` 句柄可内联；原始 CDN URI 不进入 DTO；
- 没有被生产链观察或物化的 avatar、comment image、OCR、transcript 保持 `NOT_OBSERVED`，不造空成功。

`preview` 暂时保留为兼容投影，现有业务页面已禁止读取它。Collection Target 也不再直接渲染 `identity_facts.avatar` 的远程地址。`0.8.28` 真实标准详情已证明 Work Resource 返回并在 Evidence UI 展示本地封面和本地作者头像；作品作者与监控目标继续是两个独立字段，未证明的目标保持 `NOT_VERIFIED`。

`0030` 只扩展现有媒体 purpose 约束，没有建立第二套资产。评论图片以评论为主体、作品为 Work 读取上下文，经同一 Slot → Candidate → Download → Blob → Materialization 链返回 `comment.image + subjectExternalId`。代码与隔离 PostgreSQL 非空正样本已通过；当前真实样本未观察到非空评论图片，因此真实层仍是 `NOT_OBSERVED`。

## 6. 页面与验证边界

- `layout=research|table|cover` 是本地展示状态；`view=` 仍是材料状态筛选，两者不得复用。
- 切换布局不重新请求 API，不修改数据，不改变当前选择与 Inspector。
- `Research` 展示完整 lane 带；`Table` 用于横向比较；`Cover` 用于视觉扫描。
- 真实签名详情字段探针已在当次明确授权下一次性完成，只读发布时间字段名/类型/值；未保存正文、作者资料、评论或媒体，不得把本次授权延伸为第二条平台请求。
- 当前已有一次 `0.8.28` 真实 XHS 标准详情、真实浏览器、本机 runtime 与 Evidence UI 验收，但只对当次作品、详情 30 条窗口与已观察媒体成立。自动测试/隔离 PostgreSQL 仍不能替代其他样本、非空评论图片、全量深采或长期业务验收。
