# 小红书字段与页面结构调研

更新时间：2026-08-01
调研方式：Chrome DevTools Console + 实际页面验证

## 1. 目标

记录小红书页面的内部数据结构（`__INITIAL_STATE__`）和 DOM 选择器验证结论，供后续维护博主采集、笔记采集、评论采集等功能时快速检索。

核心原则：

1. 小红书页面使用 Vue 响应式系统，`__INITIAL_STATE__` 中的部分属性是 Vue ref 包装对象，实际数据藏在 `_rawValue` 里。
2. 博主采集依赖双路数据源：DOM 选择器 + `__INITIAL_STATE__` 注入脚本。两者互为兜底。
3. DOM 选择器有 30 天过期风险，`__INITIAL_STATE__` 结构也可能随版本更新变化。每次采集异常排查应先验证两条路径。

## 2. `__INITIAL_STATE__` 整体结构

在博主页（`/user/profile/{userId}`）验证，`__INITIAL_STATE__` 的顶层结构：

```
window.__INITIAL_STATE__
  └── user (对象)
        ├── loggedIn
        ├── activated
        ├── userInfo       ← Vue ref，实际数据在 ._rawValue
        ├── follow
        ├── userPageData   ← Vue ref，实际数据在 ._rawValue
        ├── activeTab
        ├── notes
        ├── isFetchingNotes
        ├── tabScrollTop
        ├── userFetchingStatus
        ├── userNoteFetchingStatus
        ├── bannedInfo
        ├── firstFetchNote
        ├── noteQueries
        ├── pageScrolled
        ├── activeSubTab
        └── isOwnBoard
```

### 2.1 关键发现：Vue ref 包装

`userInfo` 和 `userPageData` 都是 Vue ref 对象，表面结构为：

```json
{
  "dep": {},
  "__v_isRef": true,
  "__v_isShallow": false,
  "_rawValue": { /* 实际数据 */ },
  "_value": { /* 实际数据（响应式代理） */ }
}
```

**必须通过 `._rawValue` 才能拿到原始数据**，直接读取 `.interactions`、`.basicInfo` 等属性会得到 `undefined`。

验证日期：2026-04-18

### 2.2 `userPageData._rawValue` 结构

```json
{
  "interactions": [
    { "type": "follows", "name": "关注", "count": "134" },
    { "name": "粉丝", "count": "16272", "type": "fans" },
    { "type": "interaction", "name": "获赞与收藏", "count": "35183" }
  ],
  "tags": [ /* 标签对象数组 */ ],
  "tabPublic": {},
  "extraInfo": { /* blockType, fstatus 等 */ },
  "result": {},
  "basicInfo": {
    "imageb": "https://sns-avatar-qc.xhscdn.com/avatar/...",
    "nickname": "博名",
    "images": "https://sns-avatar-qc.xhscdn.com/avatar/...",
    "redId": "42020694788",
    "gender": 0,
    "ipLocation": "北京",
    "desc": "简介文本"
  }
}
```

### 2.3 `userInfo._rawValue` 结构

```json
{
  "guest": false,
  "red_id": "42020694788",
  "user_id": "...",
  "nickname": "博名",
  "desc": "简介文本",
  "gender": 0,
  "images": "https://sns-avatar-qc.xhscdn.com/avatar/...",
  "imageb": "https://sns-avatar-qc.xhscdn.com/avatar/...",
  "userId": "...",
  "redId": "42020694788"
}
```

注意：`userInfo` 里没有 `ipLocation`、`interactions`、`tags`，这些只在 `userPageData` 中。

## 3. 注入脚本拆包规则

注入脚本 `src/injected/user.js` 的职责是读取 `__INITIAL_STATE__` 并通过 `postMessage` 传给 content script。

**当前已知陷阱：** Vue ref 对象必须拆包。拆包规则：

| 属性 | 是否需要 `._rawValue` | 状态 |
|------|----------------------|------|
| `userInfo` | 需要 | 已拆包（`user.js` 第 8 行用了 `userInfo?._rawValue`） |
| `userPageData` | 需要 | **之前未拆包**，已在 2026-04-18 调研后修复 |

## 4. DOM 选择器验证记录

以下选择器在 2026-04-18 实际验证通过：

| 选择器 | 用途 | 验证结果 | 示例输出 |
|--------|------|---------|---------|
| `.user-name` | 博主名称 | 正常 | `孙爸养A娃（成长版）` |
| `.user-redId` | 小红书号（含前缀"小红书号："） | 正常 | `小红书号：42020694788` |
| `.user-IP` | IP 属地（含前缀"IP属地："） | 正常 | `IP属地：北京` |
| `.user-desc` | 个人简介 | 正常 | `混合型ADHD清华经管本科爸爸...` |
| `.user-image` | 头像（src 属性） | 正常 | `https://sns-avatar-qc.xhscdn.com/avatar/...` |

## 5. 博主采集数据优先级

`authorCollector.js` 的实际数据获取优先级：

| 字段 | 首选来源 | 兜底来源 | 备注 |
|------|---------|---------|------|
| 名称 | DOM `.user-name` | `basicInfo.nickname` / `userInfo.nickname` | DOM 优先 |
| 小红书号 | DOM `.user-redId` | `basicInfo.redId` / `userInfo.redId` | 需去掉前缀 |
| IP 属地 | `basicInfo.ipLocation` | DOM `.user-IP` | API 数据更可靠，不受 DOM 结构变化影响 |
| 简介 | DOM `.user-desc` | `basicInfo.desc` / `userInfo.desc` | DOM 优先 |
| 头像 | DOM `.user-image` src | `basicInfo.imageb` / `basicInfo.images` | DOM 优先 |
| 粉丝/关注/获赞 | `userPageData.interactions` | 无兜底 | DOM 无此数据，纯依赖 `__INITIAL_STATE__` |
| 标签 | `userPageData.tags` | 无兜底 | DOM 无可靠选择器 |
| 性别 | `basicInfo.gender` | 无 | 0=女, 1=男 |
| 关注关系 | `extraInfo.fstatus` | 无 | |

## 5.1 2026-06-01 搜索页笔记流回归验证

验证页面：`/search_result?keyword=课题分离&source=web_search_result_notes&type=51`

本次验证结论：

| 项目 | 当前结构 | 结论 |
|------|----------|------|
| 笔记流容器 | `.feeds-container` | 仍存在且可见，搜索页首屏 1 个容器 |
| 卡片外壳 | `.feeds-container section` / `section.note-item` | 仍存在；首屏 30 个 section 中 27 个是真实笔记，3 个是“相关搜索” |
| 真实笔记判定 | `section` 内存在 `a.cover`，且 href 能提取 noteId | 仍是更可靠的过滤条件，不能只按 `section.note-item` 计数 |
| 卡片链接 | `a.cover` | 搜索页 href 当前为 `/search_result/{noteId}?xsec_token=...`，打开后进入 `/explore/{noteId}` |
| 卡片标题 | `.title` 或限定在 section 内的 `.footer span` | `.title` 更干净；`.footer span` 全页匹配过宽，必须限定在单卡片内 |
| 卡片点赞 | `.like-wrapper .count` | 搜索页卡片内可用；详情页同名选择器会混入评论点赞，不能跨页面直接全局读 |
| 视频标识 | `.play-icon` | 搜索页可用，本次首屏识别到 3 个视频标识 |
| 详情容器 | `#noteContainer` / `.note-container` | 打开搜索页第一条笔记后可见 |
| 评论容器 | `.comments-container` | 详情页可见，本次样本首屏 10 组主评论、16 条评论项 |
| 详情数据 | `window.__INITIAL_STATE__.note.noteDetailMap[{noteId}]` | 命中当前笔记，含 `interactInfo`、`imageList`、`tagList`、`user` 等字段 |

滚动验证：

- 搜索页滚动 6 轮后，DOM 累计发现 57 个唯一笔记 ID。
- `window.__INITIAL_STATE__.search.feeds` 当前有 88 条，其中 `modelType=note` 为 80 条，`modelType=rec_query` 为 8 条。
- `search.feeds[].noteCard` 当前包含 `displayTitle / user / interactInfo / cover / imageList / type`，可作为后续增强“列表发现兜底”的候选数据源。

维护建议：

1. 当前代码继续以 `section + a.cover + noteId` 过滤真实笔记是正确的，可避开“相关搜索”混入。
2. 搜索页批量发现只依赖 DOM 会受可见区域和懒加载影响；后续若要提高稳定性，可增加 `__INITIAL_STATE__.search.feeds` 作为列表发现兜底。
3. 本次只刷新搜索页笔记流与详情页样本，不代表博主页、评论图片样本、子评论展开等旧选择器已全部重新验证。

## 5.2 2026-06-01 搜索页筛选面板调研与落地

验证页面：`/search_result?keyword=A娃的启动困难&source=web_search_result_notes`

筛选面板当前可见分组：

| 分组 | 可见选项 | 本轮插件支持 |
|------|----------|--------------|
| 排序依据 | 综合 / 最新 / 最多点赞 / 最多评论 / 最多收藏 | ✅ 支持 |
| 笔记类型 | 不限 / 视频 / 图文 | ✅ 支持 |
| 发布时间 | 不限 / 一天内 / 一周内 / 半年内 | ✅ 支持 |
| 搜索范围 | 不限 / 已看过 / 未看过 / 已关注 | 不支持，本轮不实现 |
| 位置距离 | 不限 / 同城 / 附近 | 不支持，本轮不实现 |

状态结构：

- 当前页面筛选状态可从 `window.__INITIAL_STATE__.search.filterParams` 读取。
- 本轮只读取并记录三类字段：`sort_type`、`filter_note_type`、`filter_note_time`。
- `filter_note_range` 和 `filter_pos_distance` 只作为页面可见信息记录在调研结论中，不进入插件配置。

实现口径：

1. 小红书搜索页批量采集弹窗新增三组筛选控件。
2. 用户选择明确筛选项后，插件先在搜索页打开筛选面板并点击对应选项。
3. 筛选点击确认后，插件等待笔记流刷新并连续稳定，再开始扫描笔记流。
4. 如果三组都选择“沿用当前”，插件不改动页面，只读取当前筛选快照并写入本轮采集配置。
5. 旧的“按点赞 Top N”本地排序在小红书搜索页不再作为主筛选入口；“最多点赞”改走小红书页面自身的排序依据。

外部 Chrome 验证补充：

- 小红书筛选选项存在重复渲染层，同一个选项会出现外层和内层两个可见节点；点击外层可能不触发页面筛选切换。
- 插件现在会优先点击实际选中样式会变化的内层选项，并在每组筛选后确认目标选项已 active。
- 如果页面未切换成功，批量任务会停在筛选阶段并提示“小红书筛选失败”，不再继续打开笔记造成采集卡住。
- 筛选成功不等于结果列表已经刷新。插件会读取 `.feeds-container` 前若干真实笔记卡片的 noteId、标题、点赞等摘要，等摘要变化且连续稳定后才进入扫描；如果页面迟迟不稳定，会停在筛选阶段并提示刷新后重试。

## 5.3 2026-07-01 搜索页接口与筛选状态复验

验证页面：`/search_result/?keyword=多动&source=web_search_result_notes&type=51`

本次复验结论：

| 项目 | 当前结构 | 结论 |
|------|----------|------|
| 搜索结果接口 | `https://so.xiaohongshu.com/api/sns/web/v2/search/notes` | 本轮捕获 7 次请求，均返回 200；每页约 22 条结果 |
| 结果条目 | `data.items[]` | 条目包含 `id / model_type / note_card / xsec_token` |
| 卡片主体 | `data.items[].note_card` | 包含 `type / display_title / user / interact_info / cover / image_list / corner_tag_info` |
| 表层指标 | `note_card.interact_info` | 包含 `liked_count / collected_count / comment_count / shared_count` |
| 作者信息 | `note_card.user` | 包含 `user_id / nickname / nick_name / avatar / xsec_token` |
| 封面/图片 | `note_card.image_list[].info_list[]` | 包含不同场景图片 URL，可支撑列表页封面兜底 |
| 发布时间 | `note_card.corner_tag_info[]` | `type=publish_time`，文本可能是 `1天前`、`06-14`、`2025-11-14` |
| 当前筛选状态 | `window.__INITIAL_STATE__.search.filterParams` | 当前为 Vue ref 包装对象，真实值在 `._rawValue` |

真实筛选状态样本：

```json
[
  { "type": "sort_type", "tags": ["collect_descending"] },
  { "type": "filter_note_type", "tags": ["不限"] },
  { "type": "filter_note_time", "tags": ["半年内"] },
  { "type": "filter_note_range", "tags": ["不限"] },
  { "type": "filter_pos_distance", "tags": ["不限"] }
]
```

维护结论：

1. `collect_descending` 是小红书当前“最多收藏”的真实状态值，插件需要把它映射回“最多收藏”。
2. `search.filterParams` 和博主页字段一样可能是 Vue ref 包装对象，读取前必须拆 `._rawValue`。
3. 搜索页列表扫描具备更强的接口数据源：它已经有标题、作者、点赞、收藏、评论数、发布时间、封面图和作品 ID。插件已接入“关键词表层巡查接口优先、页面卡片兜底”：`surfaceOnly + search` 任务优先使用 `search/notes`，完整批量采集仍保留原来的页面扫描，以保证后续能点击进入详情页。
4. 页面 DOM 卡片仍可作为兜底，但文本会混合标题、作者、发布时间和单个指标，不适合作为优先数据源。

## 5.4 2026-07-01 博主页列表接口复验

验证页面：`/user/profile/5ebe6d210000000001000afe?...`，页面标题：`孩悦 - 小红书`

本次复验结论：

| 项目 | 当前结构 | 结论 |
|------|----------|------|
| 博主页作品接口 | `https://edith.xiaohongshu.com/api/sns/web/v1/user_posted` | 本轮捕获 12 次请求，均返回 200；每页 30 条作品 |
| 请求参数 | `num / cursor / user_id / image_formats / xsec_token / xsec_source` | 可以按博主 ID 和游标翻页；请求依赖页面携带的 `xsec_token` |
| 结果数组 | `data.notes[]` | 条目包含 `display_title / user / interact_info / cover / note_id / xsec_token / type` |
| 表层指标 | `interact_info` | 本轮稳定看到 `liked_count / liked / sticky`；未在该接口样本中看到评论数、收藏数、转发数 |
| 作者信息 | `user` | 包含 `user_id / nickname / nick_name / avatar` |
| 封面 | `cover.info_list[]` | 每条通常有 2 个不同场景的封面 URL |
| 页面卡片 | `#userPostedFeeds section` | 仍可作为兜底，但虚拟列表会卸载不可见卡片，滚动扫描成本更高 |

真实样本形态：

```json
{
  "note_id": "684bc095000000002002a91c",
  "display_title": "汪汪队发声书，宝宝自主阅读的启蒙神器",
  "user": {
    "user_id": "5ebe6d210000000001000afe",
    "nickname": "孩悦"
  },
  "interact_info": {
    "liked_count": "36",
    "sticky": false
  },
  "cover": {
    "info_list": [{ "url": "..." }]
  },
  "xsec_token": "...",
  "type": "video"
}
```

维护结论：

1. 博主日常巡查的低成本主路径应优先使用 `user_posted`：打开博主页后直接读取页面已请求到的 30 条作品，不必为了前 10 条表层巡查持续滚动。
2. 博主页 `user_posted` 和搜索页 `search/notes` 的字段能力不同：博主页当前只稳定提供点赞数，搜索页能提供赞藏评转。后续任务能力描述应避免把“列表页表层指标”泛化成同一套字段。
3. 插件已接入“博主页表层巡查接口优先、页面卡片补齐”：`surfaceOnly + profile` 任务在已捕获作品数达到目标时直接使用 `user_posted`；未达到目标或接口被拒绝时继续滚动页面收集卡片，避免深度建档只拿首屏 / 首批作品就提前结束。

### 5.4.1 2026-07-02 博主页 author_links 直连接口拒绝样本

验证页面：`/user/profile/68736c50000000001b02302d?...xsec_source=pc_search`

用户控制台探查返回：

```json
{
  "userId": "68736c50000000001b02302d",
  "tokenPresent": true,
  "apiTries": [
    { "xsec_source": "pc_search", "status": 406, "count": 0 },
    { "xsec_source": "pc_user", "status": 406, "count": 0 },
    { "xsec_source": "pc_note", "status": 406, "count": 0 },
    { "xsec_source": "app_share", "status": 406, "count": 0 }
  ],
  "capturedCachePages": 0,
  "capturedCacheNoteCount": 0,
  "pageCardCount": 12
}
```

结论：

1. `user_posted` 不能被当成深度建档的唯一主路；同一个页面和账号下，直连请求可能全部返回 406。
2. 页面已经可见 12 个作品卡，说明“接口拒绝”不等于“无法发现作品链接”。
3. `author_links` 的完成条件必须是达到目标数量、页面确认到底、连续滚动无新增或触发平台风险；不能因为拿到首屏 / 首批非空结果就结束。
4. v2.0.70 起，插件会先使用已捕获清单；清单数量不够目标或接口被拒绝时继续滚动页面，并合并去重后再提交最终结果包。

### 5.4.2 2026-07-02 博主页 DOM 持续滚动探查补充

Mog 提供的 `xhs_profile_note_discovery_probe_report.md` 进一步证明：PC 博主页可以通过页面卡片持续滚动拿到大量历史作品链接。样本从首轮约 62 条增长到 179 条；另一组持续 Map 探查从 480 条持续增长到 690 条，且仍未到底。

本轮落实为 v2.0.71：

1. 深度建档 `author_links` 目标为 200 条时，滚动上限提高到 180 轮，并把连续无新增确认提高到 15 轮；中等目标也提高到 90 轮和 10 轮确认。
2. 博主页滚动改为长短步交替、分段停顿、到底回弹再确认，不再用固定步长机械下滑。
3. `discoverWithScroll()` 继续用持续 Map 记住滚动期间见过的全部作品，避免虚拟列表回顶后丢失底部卡片。
4. 结果包新增 `discoverySummary`，包含 `stopReason / rounds / maxRounds / canLoadMore / fieldQuality`，工作台可解释“达到目标、确认到底、连续无新增、达到安全滚动上限”四种结果。

## 5.5 2026-07-01 笔记详情页与评论接口复验

验证页面：`/explore/6a3bc045000000001702a7ff?...`，页面标题：`我用三年的奖励贴纸，亲手喂死了孩子的内 - 小红书`

本次复验结论：

| 项目 | 当前结构 | 结论 |
|------|----------|------|
| 详情页容器 | `.note-container` / `#noteContainer` | 页面详情主体稳定可见；弹层 `.note-detail-mask` 在直开详情页不出现 |
| 详情结构数据 | `window.__INITIAL_STATE__.note.noteDetailMap[noteId]` | 包含正文、标题、作者、标签、发布时间、图片、赞藏评转 |
| 正文与标题 | `title / desc` | 样本标题存在，正文长度 97 |
| 作者信息 | `user.userId / nickname / avatar / xsecToken` | 可直接支撑笔记作者归档 |
| 互动指标 | `interactInfo.likedCount / collectedCount / commentCount / shareCount` | 样本为赞 1059、藏 1315、评 287、转 1304 |
| 图片媒体 | `imageList[]` | 样本 9 张图，字段含 `urlDefault / url / infoList / stream / livePhoto` |
| 主评论接口 | `https://edith.xiaohongshu.com/api/sns/web/v2/comment/page` | 样本请求返回 10 条主评论，并带内联子评论 |
| 子评论接口 | `https://edith.xiaohongshu.com/api/sns/web/v2/comment/sub/page` | 样本捕获 8 次请求，均返回 200；每次约 5 条子评论 |
| 评论容器 | `.comments-el` / `.comments-container` | 页面显示共 287 条评论；本次已渲染 20 条主评论、74 条评论项、11 个展开入口 |

详情页页面状态样本：

```json
{
  "noteId": "6a3bc045000000001702a7ff",
  "title": "我用三年的奖励贴纸，亲手喂死了孩子的内",
  "descLength": 97,
  "type": "normal",
  "interactInfo": {
    "likedCount": "1059",
    "collectedCount": "1315",
    "commentCount": "287",
    "shareCount": "1304"
  },
  "imageCount": 9
}
```

评论接口真实请求形态：

```text
/api/sns/web/v2/comment/page?note_id=...&cursor=...&top_comment_id=&image_formats=jpg%2Cwebp%2Cavif&xsec_token=...
/api/sns/web/v2/comment/sub/page?note_id=...&root_comment_id=...&num=10&cursor=...&image_formats=jpg%2Cwebp%2Cavif&top_comment_id=&xsec_token=...
```

维护结论：

1. 详情页证实“进入一次详情页后，一次拿正文、媒体、赞藏评转和评论”是合理路径。正文/媒体/指标已经在 `noteDetailMap`，评论通过页面已捕获的评论接口和补页接口继续拿。
2. 插件现有 `xhs.batchNotes + includeComments` 已具备 `note_full` 的执行基础：先采集 `noteDetailMap`，再通过 `comment/page` 与 `comment/sub/page` 采集评论。
3. 评论补页请求应保留真实页面里的 `xsec_token`，否则在需要继续翻主评论或子评论时存在不稳定风险。本轮已让补页候选 URL 优先携带 `xsec_token / image_formats / top_comment_id`，再保留旧的简化请求作为兜底。
4. 后续工作台任务设计可以把 `detail_probe + comment_probe` 收敛为 `note_full`：默认评论上限建议保持 20 或 50；全量评论作为治理/回填参数单独放开。

### 5.5.1 2026-08-14 详情页水合后全局状态清理

真实页面 `/explore/6a56177e000000000f01e0ee` 已验证：页面标题、正文和媒体完成渲染后，`window.__INITIAL_STATE__` 为 `undefined`，但无 `src` 的 SSR 内嵌脚本仍保留 `window.__INITIAL_STATE__=...`，其中目标 `noteDetailMap` 是独立合法 JSON 对象。本轮只扫描平衡花括号并对该对象执行 `JSON.parse`，不执行整段页面脚本；现场 37,307 字节脚本准确解析出唯一目标 ID、标题和 `video` 类型。详情就绪判断、稳定性判断和笔记采集统一复用该事实源。

## 5.6 2026-07-03 详情页评论 DOM 低风险补采复验

基于 Mog 在真实详情页的评论采集调研，本轮补充以下结论：

| 项目 | 当前结构 | 结论 |
|------|----------|------|
| 主评论分组 | `.parent-comment` | 这是“一组主评论 + 回复”，不是单条评论本身 |
| 主评论节点 | `.parent-comment > .comment-item:not(.comment-item-sub)` | 用直接子节点判断主评论，避免把回复误当主评论 |
| 回复节点 | `.parent-comment .reply-container .comment-item.comment-item-sub` | 层级应以 `.comment-item-sub` 判断，不依赖文字缩进 |
| 尾部字段 | 可见文本中的时间、地区、点赞、回复按钮 | 小红书经常不写“IP属地”，地区会直接跟在时间后面 |
| 风控边界 | 高频滚动 + 自动展开所有回复 | 容易触发平台验证；默认详情评论采集不应全量展开回复 |

落地规则：

1. 如果页面公开评论数大于已采集评论数，且 `note_full` 目标还没达到 20 条，插件不能只因 API 已返回一部分评论就提前完成；需要继续从页面可见评论做低频补采。
2. 默认 `twoLevel` 采集只读取当前可见的主评论和已展开回复，不主动连续点击所有“展开更多回复”。只有显式 `allReplies` 才允许继续展开。
3. 评论正文必须和尾部字段分离：`4小时前 / 广东 / 309 / 回复` 分别进入时间、地区、点赞和按钮语义，不能混进正文。
4. “309 回复”这类可见尾巴通常表示 309 个赞 + 回复按钮，不应当被解析为 309 条回复；只有明确出现“3 条回复”才计为回复数。
5. 公开 27 条但只回 16 条时，任务应标记为评论不足或继续补采，不应作为完整评论结果交付。

## 5.7 2026-08-01 视频详情页媒体流分组实测与解析规则

本次由实际小红书视频详情页的 Chrome DevTools Console 探针验证。探针确认当前 URL 对应的条目命中
`window.__INITIAL_STATE__.note.noteDetailMap`，笔记类型为 `video`，页面内 `<video>` 已具备可播放的
`currentSrc`。为避免记录可失效的签名地址，下表只保留结构和已脱敏的字段事实。

| 项目 | 实测结果 | 维护结论 |
|------|----------|----------|
| 详情数据路径 | `note.video.media.stream` | 仍是视频流的事实来源，未迁移到另一条页面路径 |
| 外层分组 | `EF4 / EF5 / EF6 / EF7` | 外层键不是稳定的编解码协议，不能只读取 `h264 / h265 / h266 / av1` |
| 非空分组 | `EF4` 1 条、`EF5` 2 条；`EF6 / EF7` 为空 | 采集器必须遍历所有即时数组分组，而非假设某一个名字必然存在 |
| 每个流条目 | `masterUrl`、`backupUrls`、`avgBitrate`、`width`、`height`、`videoCodec` 等 | 内层 URL、码率和尺寸字段保持原有语义，可继续用于优先级排序和候选地址保留 |
| 页面可播放性 | 页面 `<video>` 已加载并有当前播放地址 | “页面能播放但采集结果没有视频地址”可归因于插件读取了过时的外层分组名 |

落地规则：`pickBestVideoStream()` 只把 `stream` 的即时数组值当作候选流集合，忽略非数组元数据；随后沿用
统一的 `masterUrl / backupUrls / url` 候选地址解析与“码率 + 分辨率”排序。不得在采集器里再直接读取
`stream.h264` 作为旁路或兜底。这样旧的命名和本次的 `EF*` 命名共用同一条解析规则，而不是形成双轨逻辑。

配套的 `scripts/probe-video-streams.js` 也采用相同规则：输出实际分组的数量，以及每条流的分组、主地址、
备用地址、码率和尺寸，不能再把固定编解码名当作探针前提。

验证日期：2026-08-01；验证方式：用户提供的真实页面 Console 探针（地址字段已脱敏）。

运行态验收补充（2026-08-01）：更新后的本地插件在真实视频笔记 `6a68a020000000000f02a43b` 上执行手动采集后，
Dashboard 显示“2 个（含视频）”且下载状态为“已完成”，确认本地采集与下载已经读取到视频流。该操作未执行
“同步到工作台”，因此不能据此声称工作台的 `MediaItem / ContentMediaUsage` 已生成；端到端验收必须在显式同步后
再查服务端记录。

## 6. 已知问题与修复记录

### 6.1 Vue ref 未拆包导致博主字段全丢（已定位）

- 现象：博主页采集后，粉丝/获赞/关注显示 0，头像/简介在某些页面丢失
- 根因：`user.js` 传递 `userPageData` 时未拆包 `._rawValue`，导致 `interactions` 和 `basicInfo` 全部读到 `undefined`
- 影响：P0 1.1 + P0 2.2（IP 属地被简介污染，因为 basicInfo 丢失后回退到 DOM，DOM 选择器在某些页面偏移）
- 修复：`user.js` 第 5 行改为 `userState.userPageData?._rawValue || userState.userPageData || {}`
- 发现日期：2026-04-18

### 6.2 IP 属地被简介内容污染

- 现象：IP 属地显示为"广东来加入我的原生家庭训练营"（实为简介文本）
- 根因链条：`basicInfo` 因 Vue ref 未拆包而为空 → `ipLocation` 清洗后为空 → Dashboard 渲染时回退读 `location`（来自 DOM `.user-IP`） → 某些页面 `.user-IP` 匹配到错误元素 → `location` 污染
- 与 6.1 同根因，修复 Vue ref 拆包后 `basicInfo.ipLocation` 恢复为首选数据源，问题消除

### 6.3 视频笔记下载失败

- 现象：小红书视频笔记采集后点击媒体下载，视频资源可能下载失败。
- 根因链条：视频文件大，不能稳定通过插件消息搬运成 ZIP；同时旧视频直链过期后，小红书链路没有刷新后重试，且 `h266` / 备用链接未完整进入候选队列。
- 修复：图片笔记继续打包 ZIP；视频笔记改为直接下载视频文件，失败时先刷新当前笔记媒体链接再重试；视频候选流遍历所有即时数组分组，并保留 `masterUrl / backupUrls / urlList` 等主、备用链接，不能把 `h266 / h265 / h264 / av1` 当作固定分组名。
- 发现日期：2026-05-09

## 7. 小红书页面维护最佳实践

1. **每次博主页采集异常排查**：先在 Console 运行探查脚本（见下方），确认 `__INITIAL_STATE__` 结构是否变化。
2. **DOM 选择器 30 天过期检查**：小红书前端更新频繁，超过 30 天应重新验证；2026-06-01 已回归搜索页笔记流和单条详情样本，博主页与评论图片样本仍需按各自日期单独复验。
3. **不要假设 `__INITIAL_STATE__` 的属性是原始对象**：Vue ref 包装可能出现在任何新加的属性上，读取时始终考虑 `._rawValue` 兜底。
4. **视频笔记下载不要走大文件 ZIP 搬运**：视频资源应直接下载；如果失败，先刷新笔记媒体链接并重试，再给出失败提示。

## 8. 快速探查脚本

博主页数据结构验证（在 `/user/profile/{userId}` 页面 Console 运行）：

```js
try { console.log(JSON.stringify({
  avatarDom: !!document.querySelector('.user-image'),
  avatarSrc: document.querySelector('.user-image')?.src?.substring(0, 80),
  descDom: !!document.querySelector('.user-desc'),
  descText: document.querySelector('.user-desc')?.textContent?.trim()?.substring(0, 40),
  ipDom: !!document.querySelector('.user-IP'),
  ipText: document.querySelector('.user-IP')?.textContent?.trim(),
  nameDom: !!document.querySelector('.user-name'),
  nameText: document.querySelector('.user-name')?.textContent?.trim(),
  redIdDom: !!document.querySelector('.user-redId'),
  redIdText: document.querySelector('.user-redId')?.textContent?.trim(),
  hasState: !!window.__INITIAL_STATE__,
  updRawKeys: Object.keys(window.__INITIAL_STATE__?.user?.userPageData?._rawValue || {}),
  updInteractions: window.__INITIAL_STATE__?.user?.userPageData?._rawValue?.interactions,
  updBasicInfo: window.__INITIAL_STATE__?.user?.userPageData?._rawValue?.basicInfo,
  uiRawKeys: Object.keys(window.__INITIAL_STATE__?.user?.userInfo?._rawValue || {}),
}, null, 2)) } catch(e) { console.log('ERROR:', e.message) }
```

用途：一次性确认 DOM 选择器是否正常、`__INITIAL_STATE__` 结构是否变化、Vue ref 拆包是否正确。
