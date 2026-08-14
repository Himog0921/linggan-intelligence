# 小红书平台数据探查报告

> 探查日期：2026-03-24
> 探查方式：Chrome DevTools Console + DOM 检查

---

## 一、数据源概览

小红书 Web 端数据主要来源于两个渠道：

| 数据源 | 访问方式 | 数据完整性 | 稳定性 |
|--------|---------|-----------|--------|
| `__INITIAL_STATE__` | `window.__INITIAL_STATE__` | 高（结构化 JSON） | 中（SPA 路由切换时更新） |
| DOM | `document.querySelector()` | 中（需解析） | 低（小红书频繁改版） |

**推荐优先使用 `__INITIAL_STATE__`**，DOM 作为兜底。

---

## 二、笔记详情页数据

### 2.1 数据路径

```
__INITIAL_STATE__.note.noteDetailMap[noteId].note
```

### 2.2 完整字段清单

| 字段 | 路径 | 类型 | 示例值 | 现有插件 | 价值 |
|------|------|------|--------|---------|------|
| `noteId` | `.noteId` | string | '67f7db87000000000b0163ff' | ✅ | 主键 |
| `title` | `.title` | string | '笔记标题' | ✅ | - |
| `desc` | `.desc` | string | '笔记正文...' | ✅ | - |
| `type` | `.type` | string | 'normal' / 'video' | ✅ | - |
| `time` | `.time` | string | '发布时间' | ✅ | - |
| `lastUpdateTime` | `.lastUpdateTime` | string | '修改时间' | ❌ | 判断是否编辑过 |
| `xsecToken` | `.xsecToken` | string | '安全token' | ❌ | API 请求鉴权 |
| **ipLocation** | `.ipLocation` | string | '天津' | ❌ | **内容地域分析** |
| `userId` | `.user.userId` | string | '5ac76309e8ac2b48b9ebafd9' | ✅ | - |
| `nickname` | `.user.nickname` | string | '麋鹿dear' | ✅ | - |
| `avatar` | `.user.avatar` | string | 'https://...' | ✅ | - |
| `userXsecToken` | `.user.xsecToken` | string | 'ABEY2rjZ...' | ❌ | 作者相关 API |
| `likedCount` | `.interactInfo.likedCount` | string | '245' | ✅ | - |
| `collectedCount` | `.interactInfo.collectedCount` | string | '42' | ✅ | - |
| `commentCount` | `.interactInfo.commentCount` | string | '7073' | ✅ | - |
| `shareCount` | `.interactInfo.shareCount` | string | '100' | ✅ | - |
| `liked` | `.interactInfo.liked` | boolean | false | ❌ | 当前用户是否点赞 |
| `collected` | `.interactInfo.collected` | boolean | false | ❌ | 当前用户是否收藏 |
| **followed** | `.interactInfo.followed` | boolean | false | ❌ | **当前用户是否关注作者** |
| `relation` | `.interactInfo.relation` | string | 'none' | ❌ | 与作者关系 |
| `imageList` | `.imageList[]` | array | [...] | ✅ | - |
| `imageList[].urlDefault` | | string | 'http://...' | ✅ | 默认图 |
| `imageList[].urlPre` | | string | 'http://...' | ✅ | 高清图 |
| `imageList[].width` | | number | 1200 | ❌ | 图片宽度 |
| `imageList[].height` | | number | 1600 | ❌ | 图片高度 |
| `imageList[].infoList` | | array | [{imageScene, url}] | ❌ | 多场景图 |
| `video` | `.video` | object | {...} | ✅ | 视频笔记才有 |
| `tagList` | `.tagList[]` | array | [{id, name, type}] | ✅ | - |
| `tagList[].id` | | string | '5ccd82c8000000000d029609' | ❌ | 话题 ID |
| `tagList[].name` | | string | '晒娃' | ✅ | - |
| `tagList[].type` | | string | 'topic' | ❌ | 标签类型 |
| **atUserList** | `.atUserList[]` | array | [{userId, nickname}] | ❌ | **@提及用户列表** |
| `shareInfo` | `.shareInfo` | object | {unShare: false} | ❌ | 分享限制 |
| `shareInfo.unShare` | | boolean | false | ❌ | 是否禁止分享 |

### 2.3 建议新增采集字段

**高优先级：**

| 字段 | 用途 |
|------|------|
| `ipLocation` | 内容地域分布分析、选题地域偏好 |
| `lastUpdateTime` | 判断笔记是否被编辑过（原创性分析） |
| `tagList[].id` | 话题 ID，用于话题聚合分析 |
| `atUserList` | @联动分析、KOL 合作识别 |

**中优先级：**

| 字段 | 用途 |
|------|------|
| `interactInfo.followed` | 当前登录账号是否关注了作者（账号运营分析） |
| `imageList[].width/height` | 图片尺寸分析 |
| `shareInfo.unShare` | 判断是否可转发（内容分发分析） |

---

## 三、博主主页数据

### 3.1 数据路径

```
__INITIAL_STATE__.user.userPageData._rawValue
```

> 注意：博主页数据被 Vue 响应式包装，需访问 `_rawValue` 或 `_value`

### 3.2 完整字段清单

| 字段 | 路径 | 类型 | 示例值 | 现有插件 | 价值 |
|------|------|------|--------|---------|------|
| `userId` | `.basicInfo.userId` | string | '5bfa9eb195485a000133c823' | ✅ | - |
| `redId` | `.basicInfo.redId` | string | '679474700' | ✅ | - |
| `nickname` | `.basicInfo.nickname` | string | 'Wamomo' | ✅ | - |
| `desc` | `.basicInfo.desc` | string | '教师生存笔记📝' | ✅ | - |
| `imageb` | `.basicInfo.imageb` | string | 'https://...' | ✅ | - |
| `images` | `.basicInfo.images` | string | 'https://...' | ❌ | 小头像 |
| **gender** | `.basicInfo.gender` | number | 1 (女) / 0 (男) | ❌ | **博主性别画像** |
| **ipLocation** | `.basicInfo.ipLocation` | string | '广东' | ❌ | **博主 IP 属地** |
| `follows` | `.interactions[type=follows].count` | string | '122' | ✅ | - |
| `fans` | `.interactions[type=fans].count` | string | '689' | ✅ | - |
| `interaction` | `.interactions[type=interaction].count` | string | '7873' | ✅ | - |
| `tags` | `.tags[]` | array | [{name, tagType, icon?}] | ✅ | - |
| `tags[].name` | | string | '安道尔' | ✅ | - |
| `tags[].tagType` | | string | 'location' / 'info' | ❌ | 标签类型 |
| `tags[].icon` | | string | 'http://...' | ❌ | 标签图标（性别等） |
| `extraInfo.fstatus` | `.extraInfo.fstatus` | string | 'none' | ❌ | 关注状态 |
| `extraInfo.blockType` | `.extraInfo.blockType` | string | 'DEFAULT' | ❌ | 封禁类型 |

### 3.3 建议新增采集字段

**高优先级：**

| 字段 | 用途 |
|------|------|
| `gender` | 博主性别画像（1=女, 0=男） |
| `ipLocation` | 博主 IP 属地（地域分析） |

**中优先级：**

| 字段 | 用途 |
|------|------|
| `extraInfo.fstatus` | 当前账号与博主的关系 |
| `extraInfo.blockType` | 账号状态（判断是否被封禁） |

---

## 四、评论区数据

### 4.1 数据路径

评论区数据主要通过 **DOM 解析** 获取（`__INITIAL_STATE__` 中评论数据结构复杂且不稳定）。

### 4.2 DOM 结构分析

```html
<div class="parent-comment">
  <div class="comment-item" id="comment-{commentId}">
    <div class="avatar">
      <a class="name" data-user-id="{userId}" data-xsec-token="{token}">
        <img class="avatar-item" src="{avatarUrl}">
      </a>
    </div>
    <div class="author-wrapper">
      <a class="name">{nickname}</a>
    </div>
    <div class="content">
      <span class="note-text">{commentText}</span>
    </div>
    <div class="info">
      <div class="date">
        <span>{time}</span>
        <span class="location">{ipLocation}</span>  <!-- IP 属地 -->
      </div>
      <div class="interactions">
        <div class="like">
          <span class="count">{likeCount}</span>
        </div>
      </div>
    </div>
  </div>
  <div class="reply-container">
    <!-- 子评论结构同上，class 增加 comment-item-sub -->
  </div>
</div>
```

### 4.3 完整字段清单

| 字段 | DOM 选择器 | 类型 | 现有插件 | 价值 |
|------|-----------|------|---------|------|
| `commentId` | `.comment-item[id]` | string | ✅ | - |
| `text` | `.note-text` | string | ✅ | - |
| `author` | `.author-wrapper .name` | string | ✅ | - |
| `profileUrl` | `.author-wrapper .name[href]` | string | ✅ | - |
| `likes` | `.like .count` | number | ✅ | - |
| `time` | `.date span:first-child` | string | ✅ | - |
| **ipLocation** | `.date .location` | string | ❌ | **评论者 IP 属地** |
| **avatarUrl** | `.avatar img.avatar-item[src]` | string | ❌ | **评论者头像** |
| `userId` | `.avatar a[data-user-id]` | string | ❌ | 评论者用户 ID |
| `xsecToken` | `.avatar a[data-xsec-token]` | string | ❌ | 评论者 token |

### 4.4 评论图片数据

#### DOM 结构

```html
<!-- 评论列表中的图片（缩略图） -->
<div class="comment-picture">
  <div class="img-box">
    <img class="inner" src="http://...!nc_n_webp_mw_1">
  </div>
</div>

<!-- 预览弹窗中的图片（高清图） -->
<div class="img-container fullscreen-glass">
  <img class="img-inner img-zoom-out" src="http://...!nd_whgt34_webp_wm_1">
</div>
```

#### 图片 URL 后缀规则

| 后缀 | 场景 | 画质 | 文件大小 |
|------|------|------|---------|
| `!nc_n_webp_mw_1` | 评论列表缩略图 | 低 | 小 |
| `!nd_whgt34_webp_wm_1` | 预览弹窗大图 | 高 | 中 |

#### 采集策略

1. **低清缩略图**：直接从 `.comment-picture img.inner` 获取
2. **高清图**：点击图片打开预览弹窗 → 从 `.img-zoom-out` 获取

### 4.5 建议新增采集字段

**高优先级：**

| 字段 | 用途 |
|------|------|
| `ipLocation` | 评论者地域分析、真实用户画像 |
| `avatarUrl` | 评论者头像（用户识别） |
| `imageUrl` | 评论图片 URL（高清） |
| `imageUrlThumb` | 评论图片 URL（缩略图） |

---

## 五、其他页面数据（待探查）

### 5.1 搜索结果页

#### 数据路径

```
__INITIAL_STATE__.search
```

#### 搜索关键词

```javascript
__INITIAL_STATE__.search.searchValue._rawValue  // "数学启蒙"
```

#### 搜索结果结构

每个搜索结果项包含：

| 字段 | 路径 | 类型 | 说明 |
|------|------|------|------|
| `id` | `.feeds[].id` | string | 笔记 ID |
| `modelType` | `.feeds[].modelType` | string | 'note' / 'rec_query' |
| `noteCard` | `.feeds[].noteCard` | object | 笔记卡片信息 |
| `noteCard.type` | | string | 'video' / 'normal' |
| `noteCard.displayTitle` | | string | 标题 |
| `noteCard.user` | | object | 作者信息 |
| `noteCard.interactInfo` | | object | 互动数据 |
| `noteCard.cover` | | object | 封面信息 |
| `noteCard.cornerTagInfo` | | array | 角标（发布时间等）|
| `xsecToken` | `.feeds[].xsecToken` | string | 安全 token |
| `index` | `.feeds[].index` | number | 排序位置 |

#### 广告/推荐识别

- `modelType === 'rec_query'` 表示推荐/相关搜索
- `cornerTagInfo` 包含 `publish_time` 等标签

---

### 5.2 首页 Feed

#### 数据路径

```
__INITIAL_STATE__.feed.feeds._rawValue
```

#### Feed 结构

每个 Feed 项包含：

| 字段 | 路径 | 类型 | 说明 |
|------|------|------|------|
| `id` | `.id` | string | 笔记 ID |
| `modelType` | `.modelType` | string | 'note' |
| `noteCard` | `.noteCard` | object | 笔记卡片信息（同搜索结果）|
| `xsecToken` | `.xsecToken` | string | 安全 token |
| `index` | `.index` | number | 排序位置 |
| `exposed` | `.exposed` | boolean | 是否曝光 |
| `trackId` | `.trackId` | string | 追踪 ID |

#### 采集策略

1. 从 `feed.feeds._rawValue` 获取推荐流笔记列表
2. 提取 `id` 字段作为笔记 ID
3. 进入笔记详情页采集完整数据

### 5.3 视频笔记

#### 数据路径

```
__INITIAL_STATE__.note.noteDetailMap[noteId].note.video
```

#### 完整字段清单

| 字段 | 路径 | 类型 | 示例值 | 现有插件 | 说明 |
|------|------|------|--------|---------|------|
| `videoId` | `.videoId` | number | 137842137475106320 | ✅ | 视频唯一 ID |
| `duration` | `.duration` | number | 110566 | ✅ | 时长（毫秒） |
| `md5` | `.video.md5` | string | '5bccb515ce18a...' | ✅ | MD5 校验值 |
| `stream` | `.media.stream` | object | {...} | ✅ | 多编码流 |
| `stream.av1` | `.media.stream.av1[]` | array | [{masterUrl, ...}] | ✅ | AV1 编码流（推荐） |
| `stream.h264` | `.media.stream.h264[]` | array | [{masterUrl, ...}] | ✅ | H.264 编码流 |
| `stream.h265` | `.media.stream.h265[]` | array | [{masterUrl, ...}] | ✅ | H.265 编码流 |
| `stream.h266` | `.media.stream.h266[]` | array | [{masterUrl, ...}] | ✅ | H.266 编码流（最高质量）|
| `capa` | `.capa` | object | {...} | ❌ | 封面信息 |
| `image.firstFrameFileid` | `.image.firstFrameFileid` | string | '110/0/01e9...' | ❌ | 首帧图片 ID |
| `image.thumbnailFileid` | `.image.thumbnailFileid` | string | 'frame/110/0/...' | ❌ | 缩略图 ID |

#### 采集策略

1. **优先使用 `stream.h266`**（最高质量）
2. **备选 `stream.h265`**（平衡质量与兼容性）
3. **兜底 `stream.h264`**（兼容性好）
4. **最后备选 `stream.av1`**（旧设备兼容）

**现有插件已支持**，按优先级依次尝试 h264/h265/av1 编码的视频流 URL。

---

## 六、数据采集技术路径

### 6.1 `__INITIAL_STATE__` 采集

```javascript
// 注入脚本到页面上下文
function getByInject(win, type) {
  return new Promise((resolve, reject) => {
    const channel = new MessageChannel();
    channel.port1.onmessage = (e) => {
      if (e.data.error) reject(new Error(e.data.error));
      else resolve(e.data.result);
    };
    win.postMessage({ type: `GET_${type.toUpperCase()}` }, '*', [channel.port2]);
  });
}

// 页面上下文监听
window.addEventListener('message', (e) => {
  if (e.data.type === 'GET_NOTEMAP') {
    const noteMap = window.__INITIAL_STATE__?.note?.noteDetailMap || {};
    e.ports[0].postMessage({ result: noteMap });
  }
});
```

### 6.2 DOM 采集

```javascript
// 评论 IP 属地
const location = el.querySelector('.date .location')?.textContent?.trim() || '';

// 评论者头像
const avatarUrl = el.querySelector('.avatar img.avatar-item')?.src || '';

// 评论者用户 ID
const userId = el.querySelector('.avatar a')?.dataset?.userId || '';
```

---

## 七、数据模型扩展建议

### 7.1 notes 表新增字段

```javascript
{
  // ... 现有字段
  ipLocation: '',        // 笔记 IP 属地
  lastUpdateTime: '',    // 最后修改时间
  atUserList: [],        // @提及用户 [{userId, nickname}]
  topicIds: [],          // 话题 ID 列表
  shareRestricted: false, // 是否禁止分享
  authorFollowed: false,  // 当前用户是否关注作者
}
```

### 7.2 comments 表新增字段

```javascript
{
  // ... 现有字段
  ipLocation: '',    // 评论者 IP 属地
  avatarUrl: '',     // 评论者头像
  authorId: '',      // 评论者用户 ID
}
```

### 7.3 authors 表新增字段

```javascript
{
  // ... 现有字段
  gender: 0,           // 性别 (0=未知, 1=女, 2=男)
  ipLocation: '',      // IP 属地
  accountStatus: '',   // 账号状态 (DEFAULT/blocked等)
  followedByMe: false, // 当前用户是否关注
}
```

---

## 八、风险与限制

### 8.1 数据稳定性风险

| 风险 | 影响 | 应对策略 |
|------|------|---------|
| 小红书 DOM 改版 | 选择器失效 | 优先用 `__INITIAL_STATE__` |
| `__INITIAL_STATE__` 结构变化 | 字段丢失 | 版本检测 + 兜底逻辑 |
| SPA 路由切换 | 数据未及时更新 | 监听路由变化，等待数据加载 |

### 8.2 采集限制

- **登录态依赖**：部分字段（如 `followed`、`relation`）需要登录才有数据
- **评论懒加载**：评论区需滚动才能加载更多
- **虚拟列表**：博主笔记列表使用虚拟滚动，DOM 只保留可视区域

---

## 九、后续行动

### Phase 7：数据字段扩展

1. **笔记采集增强**
   - [ ] 新增 `ipLocation` 字段
   - [ ] 新增 `lastUpdateTime` 字段
   - [ ] 新增 `atUserList` 字段
   - [ ] 新增 `topicIds` 字段

2. **评论采集增强**
   - [ ] 新增 `ipLocation` 字段
   - [ ] 新增 `avatarUrl` 字段
   - [ ] 新增 `authorId` 字段

3. **博主采集增强**
   - [ ] 新增 `gender` 字段
   - [ ] 新增 `ipLocation` 字段

4. **数据模型更新**
   - [ ] 更新 `DATA_MODEL.md`
   - [ ] 编写 Schema 迁移脚本（v4）
   - [ ] 更新 `SELECTORS.md`

---

## 附录：探查命令速查表

```javascript
// 笔记详情页
Object.keys(__INITIAL_STATE__.note.noteDetailMap)                    // 获取 noteId 列表
__INITIAL_STATE__.note.noteDetailMap[noteId].note                    // 完整笔记数据
__INITIAL_STATE__.note.noteDetailMap[noteId].note.interactInfo       // 互动数据
__INITIAL_STATE__.note.noteDetailMap[noteId].note.user               // 作者数据
__INITIAL_STATE__.note.noteDetailMap[noteId].note.tagList            // 标签列表
__INITIAL_STATE__.note.noteDetailMap[noteId].note.atUserList         // @用户列表
__INITIAL_STATE__.note.noteDetailMap[noteId].note.imageList[0]       // 图片详情

// 博主主页
Object.keys(__INITIAL_STATE__.user.userPageData._rawValue)           // 顶层字段
__INITIAL_STATE__.user.userPageData._rawValue.basicInfo              // 基本信息
__INITIAL_STATE__.user.userPageData._rawValue.interactions           // 关注/粉丝/互动
__INITIAL_STATE__.user.userPageData._rawValue.tags                   // 标签
__INITIAL_STATE__.user.userPageData._rawValue.extraInfo              // 额外信息

// 搜索结果页
Object.keys(__INITIAL_STATE__.search)                                    // 搜索模块字段
__INITIAL_STATE__.search.searchValue._rawValue                            // 搜索关键词
__INITIAL_STATE__.search.feeds._rawValue[0]                                // 第一条搜索结果
__INITIAL_STATE__.search.feeds._rawValue[1].noteCard                        // 第二条笔记卡片

// 首页 Feed
Object.keys(__INITIAL_STATE__.feed)                                    // Feed 模块字段
__INITIAL_STATE__.feed.feeds._rawValue[0]                                // 第一条推荐内容
__INITIAL_STATE__.feed.feeds._rawValue[0].noteCard                        // 推荐笔记卡片

__INITIAL_STATE__.feed.feeds._rawValue[0].noteCard.interactInfo          // 互动数据
__INITIAL_STATE__.feed.feeds._rawValue[0].noteCard.cover                        // 封面信息

// 评论区 DOM
document.querySelector('.parent-comment')                             // 父评论容器
document.querySelector('.comment-item')                               // 评论项
document.querySelector('.date .location')?.textContent                // IP 属地
document.querySelector('.avatar img.avatar-item')?.src                // 头像
document.querySelector('.comment-picture img')?.src               // 评论图片
document.querySelector('.img-zoom-out')?.src                              // 评论高清图片
```

---

## 十、探查记录

### 10.1 探查日期

**2026-03-24**（14:00-17:30）

### 10.2 探查页面

| 页面 | 状态 | 新发现字段数 |
|------|------|-------------|
| 笔记详情页 | ✅ 完成 | 6 |
| 博主主页 | ✅ 完成 | 2 |
| 评论区 | ✅ 完成 | 3 |
| 视频笔记 | ✅ 完成 | 0（已支持）|
| 搜索结果页 | ✅ 完成 | 0（复用结构）|
| 首页 Feed | ✅ 完成 | 0（复用结构）|

### 10.3 采集方式对比

| 方式 | 稳定性 | 速度 | 数据完整度 |
|------|--------|------|-----------|
| `__INITIAL_STATE__` | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| DOM 解析 | ⭐ | ⭐⭐ | ⭐⭐ |

**结论**：优先使用 `__INITIAL_STATE__`，DOM 仅作为兜底。

---

## 十一、对项目开发的启发意义

### 11.1 数据采集策略

1. **统一数据源**：`__INITIAL_STATE__` 是小红书 SPA 的核心数据源，结构稳定、字段完整
2. **Vue 响应式处理**：部分数据被 Vue 包装（`_rawValue` / `_value`），需正确提取
3. **复用结构**：搜索结果与首页 Feed 的 `noteCard` 结构与笔记详情页一致，可复用采集逻辑

### 11.2 新增字段优先级

**高优先级（建议 Phase 7 实现）：**

| 字段 | 模块 | 价值 | 实现难度 |
|------|------|------|---------|
| `ipLocation` | 笔记 | 内容地域分析 | 低 |
| `ipLocation` | 博主 | 博主画像 | 低 |
| `ipLocation` | 评论 | 评论者画像 | 低 |
| `gender` | 博主 | 性别画像 | 低 |
| `lastUpdateTime` | 笔记 | 编辑检测 | 低 |
| `atUserList` | 笔记 | @联动分析 | 低 |

**中优先级：**

| 字段 | 模块 | 价值 | 实现难度 |
|------|------|------|---------|
| `topicIds` | 笔记 | 话题聚合 | 低 |
| `avatarUrl` | 评论 | 用户识别 | 低 |
| `authorId` | 评论 | 用户关联 | 低 |
| `interactInfo.followed` | 笔记 | 账号运营 | 低 |

### 11.3 技术债务提醒

1. **图片质量规则**：评论图片的高清获取需要点击打开预览，增加了采集复杂度
2. **Vue 响应式**：博主页数据需要访问 `_rawValue`，现有 `injected/user.js` 需更新
3. **选择器稳定性**：评论区 DOM 选择器可能随小红书改版失效，建议增加 `__INITIAL_STATE__` 作为备选数据源

### 11.4 后续开发建议

1. **Phase 7：数据字段扩展**
   - 新增 6 个高优先级字段
   - 更新数据模型（Schema v4）
   - 更新 `SELECTORS.md`

2. **Phase 8：搜索页采集**
   - 支持搜索结果页批量发现
   - 支持首页 Feed 批量发现
   - 复用 `noteCollector` 逻辑

3. **技术优化**
   - 评论区采集改用 `__INITIAL_STATE__`（如果存在）
   - 增加数据源优先级：`__INITIAL_STATE__` > DOM
   - 增加字段版本检测机制

---

## 十二、本次工作总结

### 12.1 完成内容

| 任务 | 状态 | 说明 |
|------|------|------|
| 笔记详情页探查 | ✅ | 28 个字段清单 |
| 博主主页探查 | ✅ | 15 个字段清单 |
| 评论区探查 | ✅ | 10 个字段 + 图片质量规则 |
| 视频笔记探查 | ✅ | 完整视频流结构 |
| 搜索结果页探查 | ✅ | 搜索关键词 + 结果结构 |
| 首页 Feed 探查 | ✅ | 推荐流结构 |
| 数据探查报告编写 | ✅ | docs/DATA_EXPLORATION_REPORT.md |
| 报告文档清理 | ✅ | 删除重复内容，精简结构 |

### 12.2 关键成果

1. **新发现可采集字段**：11 个
   - 笔记：`ipLocation`、`lastUpdateTime`、`atUserList`、`topicIds`、`interactInfo.followed`、`shareInfo.unShare`
   - 博主：`gender`、`ipLocation`
   - 评论：`ipLocation`、`avatarUrl`、`authorId`

2. **技术发现**
   - 数据源优先级：`__INITIAL_STATE__` > DOM
   - 搜索结果与首页 Feed 结构一致，可复用采集逻辑
   - 评论图片高清获取需点击打开预览
   - 广告识别：`cornerTagInfo.type === 'ad'`

3. **产出文档**
   - `docs/DATA_EXPLORATION_REPORT.md`（591 行）

### 12.3 后续建议

| 优先级 | 任务 | 说明 |
|--------|------|------|
| 🔴 P0 | Phase 6 验收 | 发布前必须完成 |
| 🟡 P1 | Phase 7 数据扩展 | 实现今日探查的 6 个高优先级字段 |
| 🟢 P2 | 技术债务清理 | 代码质量优化 |
| 🟢 P3 | Phase 8 搜索页采集 | 功能扩展 |

### 12.4 抖音插件开发建议

如需开发抖音插件，建议复用小红书插件的架构：

1. **可复用部分**（约 70%）
   - Chrome MV3 扩展架构
   - Content Script + Background + Popup 结构
   - 批量采集状态机
   - 反检测策略（延迟、滚动模拟）
   - IndexedDB 数据层（Dexie）
   - UI 注入框架

2. **需重新探查**
   - 抖音的 `__INITIAL_STATE__` 或类似数据源
   - DOM 选择器
   - 视频流 URL 结构
   - 评论区分页机制

3. **开发步骤**
   - Phase 0：技术逆向分析（探查数据源）
   - Phase 1：单篇笔记采集
   - Phase 2：博主主页采集
   - Phase 3：批量采集
   - Phase 4：评论采集
   - Phase 5：UI 优化
   - Phase 6：验收测试

---

> 报告完成时间：2026-03-24 17:30
> 探查人员：Claude + 用户协作
