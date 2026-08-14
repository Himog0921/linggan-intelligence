# 修复总结 2026-03-26

## 修复的问题

### 问题1：采集顺序正常 ✅
- 状态：已验证正常，无需修复

### 问题2：第二条视频开始下载失败（blob 错误）
**根因**：下载函数中 `routerMapped` 变量未定义，导致切换视频后无法获取最新的 `_ROUTER_DATA`

**修复**：
- 文件：`src/platforms/douyin/videoCollector.js`
- 位置：第 996-1003 行
- 改动：在下载函数中添加强制刷新 `_ROUTER_DATA` 的逻辑

```javascript
// 修复前：routerMapped 未定义
const merged = {
  ...(renderMapped || {}),
  ...apiData,
  ...(routerMapped || {}),  // ❌ routerMapped 是 undefined
};

// 修复后：强制刷新 _ROUTER_DATA
const routerVideo = getRouterVideoData(document);
const routerMapped = routerVideo ? mapRouterVideoToCache(routerVideo) : null;

const merged = {
  ...(renderMapped || {}),
  ...apiData,
  ...(routerMapped || {}),  // ✅ routerMapped 有最新数据
};
```

### 问题3：数据面板媒体预览和下载失败
**根因**：同问题2，`videoPlayUrl` 字段为空

**修复**：同问题2的修复，确保 `routerMapped.videoPlayUrl` 被正确合并到 `merged` 对象中

### 问题4：话题标签混在标题/正文中
**根因**：未提取 `#话题` 标签到独立字段

**修复**：

#### 抖音平台
- 文件：`src/platforms/douyin/videoCollector.js`
- 新增函数：`extractHashtags()` - 提取话题标签
- 修改函数：`sanitizeVideoTitle()` - 移除话题标签
- 修改映射函数：
  - `mapRenderVideoToCache()` - 添加 `hashtags` 字段
  - `mapRouterVideoToCache()` - 添加 `hashtags` 字段
  - `mapAwemeDetailToApiData()` - 添加 `hashtags` 字段

#### 小红书平台
- 文件：`src/platforms/xhs/noteCollector.js`
- 新增函数：
  - `extractHashtags()` - 提取话题标签
  - `removeHashtags()` - 移除话题标签
- 修改数据映射：
  - `title` - 移除话题标签
  - `content` - 移除话题标签
  - 新增 `hashtags` 字段 - 合并标题和内容中的所有话题（去重）

## 数据结构变化

### 抖音视频数据
```javascript
{
  desc: "视频标题（已移除 #话题）",
  hashtags: ["人生七年", "纪录片"],  // 新增字段
  videoPlayUrl: "https://...",
  videoDownloadUrl: "https://...",
  // ... 其他字段
}
```

### 小红书笔记数据
```javascript
{
  title: "笔记标题（已移除 #话题）",
  content: "笔记内容（已移除 #话题）",
  hashtags: ["穿搭", "OOTD"],  // 新增字段
  // ... 其他字段
}
```

## 验收步骤

1. **博主页批量下载**：
   - 打开博主主页 → 点击视频1 → 下载 ✅
   - 关闭弹窗 → 点击视频2 → 下载 ✅
   - 重复3-5次，确认每次都能正常下载

2. **数据面板媒体预览**：
   - 采集3-5个视频
   - 打开数据面板 → 点击"媒体预览" → 应该能看到视频预览
   - 点击"下载" → 应该能正常下载

3. **话题标签提取**：
   - 采集包含 `#话题` 的视频/笔记
   - 检查数据：
     - `title` / `desc` / `content` 中不应包含 `#话题`
     - `hashtags` 字段应包含提取的话题（数组格式）

## 技术债更新

- T11（ID 体系不一致）：✅ 已完成
- T12（IP 属地污染）：✅ 已完成
- T13（下载链路 blob-only）：✅ 已完成
- T14（话题标签混入）：✅ 新增并完成
