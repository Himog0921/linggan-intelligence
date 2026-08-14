# DOM 选择器依赖清单

> **原则**：所有选择器必须经过实际页面验证，禁止凭经验猜测。每次修改后更新此文件。
> **过期规则**：验证日期超过 30 天的选择器标记为 ⚠️，需重新验证。

## 笔记采集 (noteCollector.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 笔记数据 | `window.__INITIAL_STATE__.note.noteDetailMap` | ✅ 已验证 | 2026-04-18 | 通过注入脚本 noteMap.js 读取。注意 detailMap 中可能含 `"undefined"` 幽灵 key，需按真实 noteId 过滤 |
| 笔记新增字段 | `note.ipLocation / note.lastUpdateTime / note.atUserList / note.tagList[].id / note.shareInfo.unShare / note.interactInfo.followed` | ✅ 已验证 | 2026-04-18 | 14 个字段全部正常。detailMap 含幽灵 key `"undefined"` 和 `""`，需用正则过滤真实 noteId |
| 笔记流容器（搜索页） | `.feeds-container` | ✅ 已验证 | 2026-06-01 | 关键词“课题分离”搜索页实测存在 1 个可见容器；首屏 30 个 `section.note-item`，其中 27 个为真实笔记、3 个为“相关搜索” |
| 笔记卡片（列表页） | `section`（在 `.feeds-container` 或 `#userPostedFeeds` 下） | ✅ 已验证 | 2026-06-01 | 搜索页 class 仍为 `note-item`；不要只按 section 数量计数，必须继续要求卡片内存在 `a.cover` 和真实 noteId |
| 卡片封面链接 | `a.cover` | ✅ 已验证 | 2026-06-01 | 搜索页 href 当前为 `/search_result/{noteId}?xsec_token=...`，打开后会进入 `/explore/{noteId}`；博主页 `/user/profile/{userId}/{noteId}?xsec_token=...` 仍需保留 |
| 卡片封面图 | `a.cover` 内 `img / picture img / source`，读取 `currentSrc / src / data-src / srcset`，兜底 `background-image` | ✅ 已验证 | 2026-06-01 | 搜索页首屏真实笔记均可从卡片图取到封面；详情数据没有图片时继续用卡片封面兜底 |
| 卡片标题 | `.title` 或 `.footer span` | ✅ 已验证 | 2026-06-01 | 搜索页 `.title` 更干净；`.footer span` 全页匹配 108 个元素，仍必须限定在单个 section 内读取 |
| 卡片点赞数 | `.like-wrapper .count` | ✅ 已验证 | 2026-06-01 | 搜索页首屏 27 个真实笔记均可读到点赞；详情页同名选择器会混入评论点赞，必须限定在卡片 section 内 |
| 视频标识 | `.play-icon` | ✅ 已验证 | 2026-06-01 | 搜索页首屏 3 个视频标识，选择器仍可用 |
| 搜索筛选入口 | 可见按钮文字 `筛选` / `已筛选` | ✅ 已验证 | 2026-06-01 | 搜索页右侧筛选面板入口仍可用；插件只操作“排序依据 / 笔记类型 / 发布时间”，不操作“搜索范围 / 位置距离”。面板选项存在重复渲染层，必须点击实际变为 active 的内层选项，并在点击后校验选中状态 |
| 搜索筛选后结果稳定 | `.feeds-container section` + `a.cover` 提取前若干真实笔记签名 | ✅ 已验证 | 2026-06-01 | 筛选点击生效后，插件会等待笔记流刷新且连续稳定，再开始扫描；稳定前不打开笔记，避免网络延迟时采到筛选前的旧列表 |
| 搜索筛选状态 | `window.__INITIAL_STATE__.search.filterParams` | ✅ 已验证 | 2026-06-01 | 当前筛选项按 `sort_type`、`filter_note_type`、`filter_note_time` 映射；采集任务会记录筛选快照，便于回溯本轮采集口径 |

## 评论采集 (commentCollector.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 评论容器 | `.comments-container` | ✅ 已验证 | 2026-04-18 | 含 `data-v-4a19279a` Vue 作用域属性 |
| 主评论组 | `.parent-comment` | ✅ 已验证 | 2026-04-18 | 10 组 |
| 主评论条目 | `.comment-item:not(.comment-item-sub)` | ✅ 已验证 | 2026-04-18 | 10 条 |
| 子评论条目 | `.comment-item.comment-item-sub` | ✅ 已验证 | 2026-04-18 | 7 条（已展开的子评论） |
| 评论作者 | `a.name` | ✅ 已验证 | 2026-04-18 | class 变为 `active router-link-exact-active name`，仍匹配 `a.name` |
| 评论文字 | `span:not([class])`（第一个非空、非时间格式） | ✅ 已验证 | 2026-04-18 | 在 .comment-inner-container 内查找。发现父级有 `span.note-text` 可作更稳定替代选择器 |
| 评论时间 | `span:not([class])`（匹配时间正则） | ✅ 已验证 | 2026-04-18 | 父级 class 为 `date` |
| 评论点赞数 | `.like-wrapper .count` 或 `.like .count` | ✅ 已验证 | 2026-04-18 | 46 个匹配。回退 `[class*="like"] .count` 同样有效 |
| 评论 IP 属地 | `.date .location` | ✅ 已验证 | 2026-04-18 | 17 个匹配 |
| 评论头像 | `.avatar img.avatar-item` 或 `.avatar img` | ✅ 已验证 | 2026-04-18 | 17 个 `.avatar-item`，19 个 `.avatar img`（含页面顶部用户头像 `user-image`） |
| 评论作者 ID | `.avatar a[data-user-id]` | ✅ 已验证 | 2026-04-18 | 17 个匹配 |
| 评论区图片 | `.comment-item img`（排除头像和表情） | ⚠️ 需改进 | 2026-04-18 | 22 个匹配全部是 `.avatar-item` 头像，未见评论附图样本 |
| 展开子评论按钮 | `div.show-more`（文字含"展开"） | ✅ 已验证 | 2026-04-18 | 7 个，文字格式 "展开 N 条回复" |
| 回复按钮（勿点） | `span.count`（文字含"回复"） | ✅ 已验证 | 2026-04-18 | 68 个匹配（混入了其他 `.count` 元素） |

## 博主采集 (authorCollector.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 博主信息容器 | ~~`.user-info`~~ 已移除 | ✅ 已验证不存在 | 2026-03-20 | 实际页面无此容器，改为直接从 document 查询 |
| 小红书号 | `SPAN.user-redId`（含前缀"小红书号："） | ✅ 已验证 | 2026-04-18 | 需去掉前缀 |
| 博主名称 | `DIV.user-name` | ✅ 已验证 | 2026-04-18 | |
| 头像 | `IMG.user-image` | ✅ 已验证 | 2026-04-18 | |
| IP属地 | `SPAN.user-IP`（含前缀"IP属地："） | ✅ 已验证 | 2026-04-18 | 需去掉前缀。但某些页面此选择器匹配到错误元素，`basicInfo.ipLocation`（来自 `__INITIAL_STATE__`）更可靠 |
| 个人简介 | `DIV.user-desc` | ✅ 已验证 | 2026-04-18 | |
| 标签 | ~~`.tag-item`~~ → `__INITIAL_STATE__` | ✅ 已验证 | 2026-04-18 | DOM 无可靠选择器，改用 `userPageData.tags`。实测返回 ["狮子座", "广东深圳", "撰稿人", "情感博主"] |
| 关注/粉丝/互动数 | ~~`.user-interactions .count`~~ → `__INITIAL_STATE__` | ✅ 已验证 | 2026-04-18 | DOM 只有 `SPAN.shows` 文字标签（3个），数值在 `userPageData._rawValue.interactions`（注意 Vue ref 拆包）。实测数据完整 |
| 性别/账号状态/关注关系 | `basicInfo.gender`、`basicInfo.ipLocation`、`extraInfo.blockType`、`extraInfo.fstatus` | ✅ 已验证 | 2026-04-18 | gender=1, ipLocation="广东", blockType="DEFAULT", fstatus="follows" |

### `__INITIAL_STATE__` Vue ref 拆包注意

`userPageData` 和 `userInfo` 都是 Vue ref 对象，实际数据在 `_rawValue` 属性中。注入脚本 `src/injected/user.js` 必须做 `._rawValue` 拆包，否则下游 `interactions`、`basicInfo` 等全部读到 `undefined`。

详见 `docs/technical/XHS_FIELD_SURVEY.md` 第 2-3 节。

## 批量采集 (batchController.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 笔记卡片定位 | 通过 noteId 在 DOM 中查找 `a.cover[href*="{noteId}"]` | ✅ 已验证 | 2026-04-18 | 博主页 href 新格式 `/user/profile/{userId}/{noteId}?xsec_token=...`，`extractNoteId` 兜底逻辑可用。建议增加显式正则 |
| 打开笔记 | 点击/打开搜索卡片 → 路由跳转到 `/explore/{noteId}` | ✅ 已验证 | 2026-06-01 | 搜索页 `/search_result/{noteId}` 链接实际打开为 `/explore/{noteId}` |
| 笔记数据就绪 | 监听 URL 变化 + 等待 `__INITIAL_STATE__` 中对应 noteId 数据 | ✅ 已验证 | 2026-06-01 | 详情页 `note.noteDetailMap[{noteId}]` 命中当前笔记，含标题、正文、互动数、图片、作者等字段 |
| 返回列表 | `history.back()` | ❓ 未验证 | - | 需确认 SPA 路由能正确返回 |
| 关闭弹窗按钮（旧方案，备用） | `.close-circle, .close-btn, [class*="close"]` | ❓ 未验证 | - | `[class*="close"]` 过于模糊，需收窄 |
| 关闭弹窗备用 | `chrome.debugger` 模拟 Esc | ✅ 已验证 | 2026-03-20 | |

## 页面识别 (pageDetector.js)

| 用途 | 识别方式 | 验证状态 | 验证日期 | 备注 |
|------|---------|---------|---------|------|
| 笔记详情页 | URL 含 `/explore/` 或 `/discovery/item/` | ✅ 已验证 | 2026-03-20 | |
| 搜索页 | URL 含 `/search_result` | ✅ 已验证 | 2026-03-20 | |
| 博主主页 | URL 含 `/user/profile/` | ✅ 已验证 | 2026-03-20 | |
| 发现页 | URL 是首页 `/` 或 `/explore` | ✅ 已验证 | 2026-03-20 | |

## 抖音视频采集 (src/platforms/douyin/videoCollector.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 激活视频元素 | `[data-e2e="feed-active-video"]` | ✅ 已验证 | 2026-03-27 | 弹层态下最重要的当前视频信号 |
| 当前视频 ID | `[data-e2e="video-info"][data-e2e-aweme-id]` | ✅ 已验证 | 2026-03-27 | 与 `modal_id / activeVid` 通常一致 |
| 视频标题 | `[data-e2e="video-desc"], [data-e2e="detail-video-info"]` | ✅ 已验证 | 2026-03-27 | 标题稳定性可用于校验当前上下文 |
| 视频信息文本 | `[data-e2e="video-info"]` | ✅ 已验证 | 2026-03-27 | 可见节点优先，不能直接拿第一个 |
| 博主昵称（纯昵称优先） | `[data-e2e="feed-video-nickname"]` | ✅ 已验证 | 2026-03-27 | 兜底 `[data-e2e="user-info"]` 会混勋章/统计信息 |
| 分享触发 | 原生分享按钮点击事件 | ✅ 已验证 | 2026-03-27 | 作为“当前视频确认动作” |
| 当前视频直链 | `/aweme/v1/web/aweme/detail/?aweme_id=<id>&aid=6383` | ✅ 已验证 | 2026-03-27 | detail API 比 DOM blob 更可信 |
| 主页预览态兜底 ID | URL 参数 `vid` | ✅ 已验证 | 2026-03-27 | 仅在无弹层、无激活视频 DOM 时可用 |
| 弹层态 URL ID | URL 参数 `modal_id` | ✅ 已验证 | 2026-03-27 | 弹层态主信号之一 |

## 抖音评论采集 (src/platforms/douyin/commentCollector.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 一级评论接口 | `/aweme/v1/web/comment/list/?item_id=<awemeId>&aweme_id=<awemeId>` | ✅ 已验证 | 2026-03-27 | 推荐主数据源 |
| 二级评论接口 | `/aweme/v1/web/comment/list/reply/?item_id=<awemeId>&aweme_id=<awemeId>&comment_id=<parentId>` | ✅ 已验证 | 2026-03-27 | 推荐回复数据源 |
| 评论主键 | `cid` | ✅ 已验证 | 2026-03-27 | 建议映射为 `commentId` |
| 父评论关系 | `root_comment_id / reply_id / reply_to_reply_id / level` | ✅ 已验证 | 2026-03-27 | 用于层级建模 |
| 评论者地域 | `ip_label` | ✅ 已验证 | 2026-03-27 | 比 DOM 拼接文本更可靠 |
| 评论图片区 | `image_list[]` | ✅ 已验证 | 2026-03-27 | 不要依赖 DOM 放大图 |
| 高清图片 URL | `image_list[].origin_url.url_list` | ✅ 已验证 | 2026-03-27 | 最高优先级 |
| 图片下载兜底 | `download_url → medium_url → crop_url → thumb_url` | ✅ 已验证 | 2026-03-27 | 按顺序回退 |

## 抖音批量采集 (src/platforms/douyin/batchController.js)

| 用途 | 选择器 / 方式 | 验证状态 | 验证日期 | 备注 |
|------|-------------|---------|---------|------|
| 博主主页识别 | URL 含 `/user/` | ✅ 已验证 | 2026-03-27 | 当前批量能力只服从博主主页场景 |
| 视频卡片入口 | 主页视频列表中的视频链接 / 卡片点击 | ✅ 已验证 | 2026-03-27 | 通过打开弹层进入当前视频上下文 |
| 批量任务前置动作 | 先打开弹层，再等当前视频稳定 | ✅ 已验证 | 2026-03-27 | 不能直接靠列表卡片猜当前视频 |
| 停止/暂停/继续 | Content 任务控制器 + Popup/页面按钮 | ✅ 已验证 | 2026-03-27 | 抖音已接入统一控制链路 |

### 抖音搜索页 DOM 差异（2026-04-18 验证）

**关键发现：综合搜索页与视频搜索页的 DOM 结构完全不同。**

综合搜索页（URL 无 `type=video` 参数）：
- `li a[href*="/video/"]` → 返回 0，搜索结果不使用 `<a>` 链接
- `a[href*="/video/"]` → 返回 0，页面上无任何视频链接
- 搜索结果区域无 `data-e2e` 属性
- DOM 发现策略（`batchDiscovery.js` 的 `discoverDouyinSearchTargetsFromDom`）**不可用**

视频搜索页（URL 含 `type=video`）：
- 视频结果使用 `<li><a href="/video/...">` 结构
- DOM 发现策略可用

结论：综合搜索页的批量采集必须走 API 发现（`aweme_general` 频道），不能依赖 DOM 发现。详见 `docs/technical/DOUYIN_FIELD_SURVEY.md` 第 10.1 节。

## 抖音页面识别 (src/platforms/douyin/pageDetector.js)

| 用途 | 识别方式 | 验证状态 | 验证日期 | 备注 |
|------|---------|---------|---------|------|
| 博主主页预览态 | URL 含 `/user/` 且仅有 `vid`，无激活视频 DOM | ✅ 已验证 | 2026-03-27 | 不能把 `vid` 当当前弹层视频 |
| 博主主页弹层态 | URL 含 `/user/` 且有 `modal_id`，同时存在 `[data-e2e="feed-active-video"]` | ✅ 已验证 | 2026-03-27 | 当前视频主业务场景 |
| 直接视频页 | URL 含 `/video/` | ✅ 已验证 | 2026-03-27 | 可优先信 URL 中的视频 ID |

---

## 维护规则

1. **新增选择器前**：先在真实页面上验证，再写入代码，同时更新此文件
2. **修改选择器后**：立即更新此文件的验证状态和日期
3. **定期检查**：每次大版本更新前，对所有 ⚠️ 和 ❓ 状态的选择器重新验证
4. **30 天过期**：验证日期超过 30 天的选择器需重新验证

## 探查脚本

| 脚本 | 用途 |
|------|------|
| `scripts/probe-batch-flow.js` | 验证卡片选择器和打开模式 |
| `scripts/probe-comment-media.js` | 映射评论图片 URL |
| `scripts/probe-video-streams.js` | 记录视频码率和分辨率 |
| `scripts/probe-douyin-video-fields.js` | 抖音视频页 URL/DOM/render/state 联合探针 |
| `scripts/probe-douyin-author-fields.js` | 抖音博主页字段与串号探针 |
| `scripts/probe-douyin-root-cause.js` | 抖音 blob-only / ID 错位根因探针 |
