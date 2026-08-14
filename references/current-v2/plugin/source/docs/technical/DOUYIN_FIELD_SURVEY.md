# 抖音字段与页面结构调研

更新时间：2026-03-27
调研方式：Chrome MCP + 页面 Console + 实际用户操作回放

## 1. 目标

这份文档不再记录“猜测型修复”，只记录已经在真实页面里验证过的结论，用于后续维护抖音采集、下载、评论和批量任务。

核心原则：

1. 先识别页面状态，再决定该信哪些字段。
2. 先识别“当前视频上下文”，再执行采集、下载、评论或分享链路。
3. `URL / DOM / API / 页面复用时序` 必须一起看，不能只看单个字段。

## 2. 用户真实操作路径

当前抖音能力必须服从这条真实业务路径：

1. 进入博主主页
2. 点开某一条视频
3. 在视频弹层里上下滑动切换视频
4. 对当前视频执行采集、下载、分享、评论采集等动作

这意味着抖音 Web 端不是传统的“换页式详情页”，而是一个会复用 DOM、错峰更新 URL 和数据的 SPA。

## 3. 页面状态分型

### 3.1 博主主页预览态

典型 URL：

```text
https://www.douyin.com/user/<secUid>?from_tab_name=main&vid=<awemeId>
```

特征：

- 没有激活视频 DOM
- 找不到 `[data-e2e="feed-active-video"]`
- 找不到可用的 `[data-e2e="video-info"]`
- `desc` 也可能为空

结论：

- 这个状态下，`vid` 只能作为“页面预览线索”
- 不能直接把它当成“当前弹层视频”

### 3.2 博主主页视频弹层态

典型 URL：

```text
https://www.douyin.com/user/<secUid>?from_tab_name=main&modal_id=<id>&vid=<id-or-stale>
```

特征：

- 存在 `[data-e2e="feed-active-video"]`
- 存在可见的 `[data-e2e="video-info"]`
- 评论区、分享按钮、标题都围绕弹层内当前视频更新

关键结论：

- 当前视频应优先按弹层上下文识别
- 这个场景里，`vid` 经常是旧值或页面级值
- 不能把 `vid` 当作当前视频唯一主键

### 3.3 直接视频页

典型 URL：

```text
https://www.douyin.com/video/<awemeId>
```

结论：

- 这类页面可以优先信 URL 中的视频 ID
- 但当前项目的主要业务场景不是这类页面，而是“博主主页弹层”

## 4. 当前视频识别规则

## 4.1 已验证可靠的信号

在“博主主页视频弹层态”里，当前视频识别优先信这组信号：

1. URL 里的 `modal_id`
2. `[data-e2e="feed-active-video"]` 的 `data-e2e-vid`
3. 可见 `[data-e2e="video-info"]` 的 `data-e2e-aweme-id`

在实测样本中，这三者通常一致。

## 4.2 已验证不可靠的信号

- URL 里的 `vid`
- 旧的 `RENDER_DATA.app.videoDetail`
- 已缓存但标题对不上的旧视频数据
- 通过类名 `.video_xxx` 反推出的历史卡片 ID

结论：

- `vid` 只能作为“无弹层时的预览兜底”
- 一旦弹层存在，`vid` 必须降级

## 4.3 最佳实践

后续维护必须遵守：

1. 先判断当前是不是弹层态。
2. 若是弹层态，只从“当前激活视频元素”解析主键。
3. 只有在没有激活视频 DOM 时，才允许回退 `vid`。
4. 采集、下载、评论、面板展示必须共享同一份 `resolvedVideoId`。

## 5. 分享动作结论

已验证：

- 用户点击抖音原生分享按钮，是一个稳定的“当前视频确认动作”
- 这个动作比“仅靠滑动自动猜当前视频”更可靠

结论：

- 单条视频采集适合以“分享动作”作为强触发
- 后续如果页面再次改版，优先保住“分享触发采集”这条链路

## 6. 评论接口调研结论

### 6.1 主评论接口

已验证可用：

```text
/aweme/v1/web/comment/list/?device_platform=webapp&aid=6383&channel=channel_pc_web&item_id=<awemeId>&aweme_id=<awemeId>&cursor=0&count=<n>
```

用途：

- 获取当前视频一级评论

### 6.2 回复接口

已验证可用：

```text
/aweme/v1/web/comment/list/reply/?device_platform=webapp&aid=6383&channel=channel_pc_web&item_id=<awemeId>&aweme_id=<awemeId>&comment_id=<parentCommentId>&cursor=0&count=<n>
```

用途：

- 获取某条一级评论下的二级评论

### 6.3 评论层级字段

已验证有分析价值的字段包括：

- `cid`
- `text`
- `create_time`
- `digg_count`
- `ip_label`
- `level`
- `reply_comment_total`
- `root_comment_id`
- `reply_id`
- `reply_to_reply_id`
- `user.uid`
- `user.sec_uid`
- `user.nickname`

维护建议：

- 评论采集不要只存扁平文本
- 至少保留 `commentId / parentCommentId / rootCommentId / level / replyToCommentId`

## 7. 评论图片区字段

已在真实评论接口中拿到 `image_list`，字段结构如下：

- `origin_url`
- `download_url`
- `medium_url`
- `crop_url`
- `thumb_url`

高清图优先级建议：

1. `origin_url.url_list`
2. `download_url.url_list`
3. `medium_url.url_list`
4. `crop_url.url_list`
5. `thumb_url.url_list`

结论：

- 抖音评论图片区高清下载不应靠 DOM 放大图兜底
- 应优先直接消费评论接口里的图片字段

## 8. 抖音页面维护最佳实践

### 8.1 不要再做的事

- 不要把 `vid` 直接当成弹层当前视频 ID
- 不要让采集、下载、评论各自独立猜当前视频
- 不要在“数据未稳定”时立刻开始下载或批量任务

### 8.2 应该坚持的做法

1. 先判定页面状态：主页预览态 / 弹层态 / 直接视频页
2. 先解析统一 `VideoContext`
3. 再等当前视频稳定
4. 最后才做采集、下载、评论、批量操作

### 8.3 稳定性判定建议

一个视频上下文至少应同时满足：

- 激活视频元素存在
- 当前标题可读
- 当前 `resolvedVideoId` 可读
- 当前上下文在短时间内连续两次一致

## 9. 现阶段固定排障顺序

从 2026-03-27 起，抖音异常排障顺序固定为：

1. 先确认页面状态
2. 再确认当前视频 ID 是否正确
3. 再确认 detail API 是否有直链
4. 最后才判断下载执行层

不要再反过来“先改下载器，再猜页面是谁”。

## 10. 已知问题与代码层根因

### 10.1 综合搜索页批量采集失效

- 现象：综合搜索页发起批量任务失败，视频搜索页正常
- 根因（2026-04-18 实机验证确认）：
  1. 综合搜索页（`/search/{keyword}`，无 `type=video` 参数）的视频搜索结果**不使用 `<a href="/video/...">` 链接渲染**
  2. DOM 发现策略 `document.querySelectorAll('li a[href*="/video/"]')` 在综合搜索页返回 **0**
  3. `document.querySelectorAll('a[href*="/video/"]')`（不限 `<li>`）同样返回 **0**
  4. 综合搜索页的 `data-e2e` 属性只有导航栏组件（searchbar-input、button 等），搜索结果区域无任何 `data-e2e` 属性
  5. `detectDouyinSearchBatchContext()` 中 `hasResultCards >= 5` 的判断始终为 false → `stableSearchList = false` → 搜索页被判定为"不可批量"
  6. **即使 `stableSearchList = true`**，DOM 发现返回空数组（`[]`），代码不会把空数组当作失败去触发 API 兜底（`batchDiscovery.js:466-472` 中 `domTargets.length > 0` 为 false 时才走 API，但空数组也是"成功返回"，不会进入 catch）
- 修复方向：综合搜索页应直接走 API 发现（`aweme_general` 频道），跳过 DOM 发现。或者在 DOM 发现返回空时主动降级到 API
- 风险类型：外部依赖风险（抖音搜索页 DOM 结构随时可变）
- 验证 URL：`/search/%E5%92%96%E5%95%A1%E5%A5%BD%E5%96%9D`（搜索词"咖啡好喝"）

#### 10.1.1 综合搜索页 vs 视频搜索页 DOM 对比

| 特征 | 视频搜索页（`type=video`） | 综合搜索页（默认/无 type） |
|------|--------------------------|-------------------------|
| 视频链接 `<a href="/video/...">` | 存在 | 不存在（返回 0） |
| `<li>` 结构 | 搜索结果在 `<li>` 内 | 不适用 |
| `data-e2e` 搜索结果属性 | 有 | 无（仅导航栏有） |
| `[data-e2e="search-result"]` | 待验证 | 不存在 |
| API 频道参数 | `aweme_video` | `aweme_general` |
| DOM 发现可行 | 是 | 否 |
| API 发现可行 | 是（`aweme_video`） | 待验证（`aweme_general`） |

#### 10.1.2 综合搜索页实机探查记录（2026-04-18）

探查 URL：`/search/%E5%92%96%E5%95%A1%E5%A5%BD%E5%96%9D`

```
document.querySelectorAll('li a[href*="/video/"]').length  → 0
document.querySelectorAll('a[href*="/video/"]').length     → 0
document.querySelectorAll('a[href*="/note/"]').length      → 0
document.querySelector('[data-e2e="searchbar-input"]')?.value → null
document.querySelector('[data-e2e="search-result"]')       → null
data-e2e attributes: searchbar-input, searchbar-button, something-button, im-entry, live-avatar
douyin.com links: 仅导航链接（jingxuan, aisearch, follow 等）
```

### 10.2 评论图片下载抓错来源

- 现象：下载的不是评论区图片，文件格式异常无法打开
- 根因分析（2026-04-18 代码审查）：
  1. `commentMedia.js:6-18` 的 `extractImageCandidatesFromComment` 只读取 `comment.image_list`，数据来源正确
  2. `fetchImageBlob()`（`commentMedia.js:56-97`）在 `cors` 模式失败后用 `no-cors` 重试，`no-cors` 返回 opaque blob
  3. 代码不校验 blob 的 MIME type 就直接写入 ZIP，如果 URL 返回的是 HTML 错误页或重定向页，生成的文件不是有效图片
  4. 部分抖音图片 URL 需要特定 referer/cookie，`fetch` 未携带完整凭证
- 修复方向：下载后检查 blob.type，非图片 MIME 直接跳过

### 10.3 单条评论采集有跳失

- 现象：部分评论未采集到，存在遗漏
- 根因分析（2026-04-18 代码审查，三层叠加）：
  1. **Cursor 翻页死循环风险**：`getCommentCursor()`（`commentApi.js:80-93`）如果 API 返回与当前相同的 cursor，会反复请求同一页
  2. **只抓两级评论**：`commentCollector.js:158-205` 的 reply 循环只取 level 1 → level 2，level 3+ 子回复全部丢失
  3. **空 ID 评论被静默丢弃**：`commentCollector.js:141-142` 如果 API 返回的评论缺少 `cid` 字段，`!parentRecord.commentId` 判定为真，整条评论被跳过
- 修复方向：加 cursor 重复检测防死循环；对空 ID 生成 fallback；reply 层级递归展开

## 11. 搜索页探查脚本

综合搜索页 DOM 结构验证（在搜索结果「综合」tab 页面 Console 运行）：

```js
try { console.log(JSON.stringify({
  url: location.href,
  liVideoCount: document.querySelectorAll('li a[href*="/video/"]').length,
  liNoteCount: document.querySelectorAll('li a[href*="/note/"]').length,
  allVideoLinks: Array.from(document.querySelectorAll('a[href*="/video/"]')).length,
  allNoteLinks: Array.from(document.querySelectorAll('a[href*="/note/"]')).length,
  searchInput: document.querySelector('[data-e2e="searchbar-input"]')?.value,
  resultCardSample: Array.from(document.querySelectorAll('a[href*="/video/"]')).slice(0,3).map(a => ({
    href: a.href?.substring(0,60),
    parentTag: a.parentElement?.tagName,
    grandparentTag: a.parentElement?.parentElement?.tagName,
    text: a.textContent?.trim()?.substring(0,60),
  })),
}, null, 2)) } catch(e) { console.log('ERROR:', e.message) }
```

## 12. 当前结论摘要

最重要的结论：

1. 抖音博主页弹层场景下，当前视频应该认 `modal_id = activeVid = data-e2e-aweme-id`。
2. `vid` 在这个场景里经常不是当前视频，最多只能做兜底线索。
3. 评论和回复都已有稳定接口，适合直接结构化采集。
4. 评论图片区高清下载应该优先吃接口里的 `origin_url`，不要只靠 DOM 图预览。
5. 评论图片下载需校验 MIME type，`no-cors` 模式的 opaque blob 不可信。
6. 评论采集只抓两级，三级及以上子回复会丢失。
7. 综合搜索页的 DOM 结构可能与视频搜索页不同，批量发现策略需要分页适配。
