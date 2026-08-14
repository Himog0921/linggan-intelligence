# T11-T13 技术债修复总结

**修复日期**: 2026-03-26
**修复人**: Proma Agent
**影响范围**: 抖音视频采集和下载功能

---

## 问题根因

通过探针测试发现：**博主页弹窗**（`/user/xxx?modal_id=xxx`）与**视频详情页**（`/video/xxx`）的 DOM 结构不同：

- **博主页弹窗**：作者信息区域不渲染，DOM 选择器全部失效
- **视频详情页**：DOM 完整，但 ID 解析优先级混乱

这导致：
- T11: 批量采集时 ID 解析错误，采到错误的视频
- T12: IP 属地从 DOM 解析时混入昵称片段
- T13: 下载链路因 ID 不一致导致 blob-only 误判

---

## 解决方案

**核心策略**：改用 `window._ROUTER_DATA` 作为主要数据源，不依赖 DOM 渲染。

### 新增函数

1. **`getRouterData()`**: 从 `<script>` 标签提取 `window._ROUTER_DATA`
2. **`getRouterVideoData()`**: 提取 `loaderData['video/:id'].videoInfoRes.item`
3. **`mapRouterVideoToCache()`**: 将 `_ROUTER_DATA` 映射为统一格式

### 修改内容

**T11 修复 - ID 解析优先级调整**：
```javascript
// 旧优先级（7个候选来源，优先级混乱）
awemeId || renderId || playingId || activeId || vidFromQuery || urlId || titleMatchedId || initialId

// 新优先级（_ROUTER_DATA 优先）
routerAwemeId || modalId || vidFromQuery || urlId || awemeId || renderId || ...
```

**T12 修复 - IP 属地提取**：
```javascript
// 旧方式：从 DOM 解析（易污染）
const ipLocation = parseLocationFromInfoText(infoEl.textContent);

// 新方式：从 _ROUTER_DATA 提取（干净）
const ipLocation = normalizeIpLocation(routerVideo.author?.ip_location);
```

**T13 修复 - 下载链路**：
```javascript
// 旧方式：ID 不一致导致链路错位
const merged = { ...renderMapped, ...apiData };

// 新方式：统一 ID 后合并所有来源
const merged = { ...renderMapped, ...routerMapped, ...apiData };
```

---

## 修改文件

1. `/src/platforms/douyin/videoCollector.js`
   - 新增 `getRouterData()`, `getRouterVideoData()`, `mapRouterVideoToCache()`
   - 修改 `resolveCurrentVideoId()` 优先级
   - 修改 `collectDouyinVideo()` 和 `downloadDouyinVideo()` 数据合并逻辑

2. `/docs/plans/tech-debt.md`
   - 标记 T11/T12/T13 为已完成

---

## 验收标准

### T11 验收
- [ ] 博主页弹窗批量采集，每个视频的 ID 正确
- [ ] 视频详情页单篇采集，ID 正确
- [ ] Console 无 "无法定位当前正在播放的视频" 错误

### T12 验收
- [ ] 博主页弹窗采集，IP 属地不含昵称片段
- [ ] 视频详情页采集，IP 属地正确
- [ ] 无 IP 属地时返回空字符串，不误判

### T13 验收
- [ ] 博主页弹窗下载，不报 "仅捕获到 blob" 错误
- [ ] 视频详情页下载，直链正确
- [ ] 下载的视频与当前播放视频一致

---

## 测试建议

1. **博主页弹窗场景**：
   - 打开博主主页 → 点击视频（弹窗播放）→ 批量采集 5 个视频
   - 检查每个视频的 `noteId`、`authorName`、`ipLocation` 是否正确

2. **视频详情页场景**：
   - 右键视频 → "在新标签页中打开" → 单篇采集
   - 检查数据完整性

3. **下载场景**：
   - 博主页弹窗 → 点击下载按钮 → 检查下载的视频是否正确
   - 视频详情页 → 点击下载按钮 → 检查下载的视频是否正确

---

## 后续优化

1. **性能优化**：`_ROUTER_DATA` 解析只需执行一次，可缓存结果
2. **兼容性**：如果抖音改版移除 `_ROUTER_DATA`，需回退到 API 方案
3. **监控**：添加日志记录 ID 解析来源，便于排查问题
