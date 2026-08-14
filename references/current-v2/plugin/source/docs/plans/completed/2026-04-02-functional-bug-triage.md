# 灵感爆爆爆功能 Bug 梳理（2026-04-02）

> 目标：把“技术债”与“用户会直接感知成坏掉”的功能问题分开，给接手期一个明确的修复顺序。

## 判断标准

- **功能 bug**：用户会直接看到页面判断错、按钮错、数据空白、状态误导、任务无法继续等问题。
- **技术债**：暂时不一定马上坏，但会持续增加功能 bug 发生概率。

## 当前优先级

## P0

### B1. Popup 会把部分非搜索场景误判成“搜索页”

- 状态：2026-04-02 已修复
- 证据：
  - `src/popup/popup.js` 中 `getModeFromUrl()` 对抖音默认返回 `search`
  - `src/popup/popup.js` 中 `getModeFromUrl()` 对小红书默认也返回 `search`
- 影响：
  - 抖音首页、异常页面、非稳定列表页，可能被 Popup 误标成“搜索结果页”
  - 随之出现错误提示文案、错误能力暗示，甚至让用户在不该发起批量任务的页面上点出批量动作
- 结论：
  - 这不是文案小问题，而是页面能力判断源头错误，应该优先修

### B2. XHS Popup 文案声称“评论图片区可用”，但真实按钮被隐藏

- 状态：2026-04-02 已修复
- 证据：
  - `src/popup/popup.js` 中 `renderPageContext()` 对小红书详情页写的是“适合执行单篇笔记、单篇评论和评论图片区下载”
  - 同文件 `applyPlatformUI()` 默认把 `btnCommentImages.style.display = 'none'`
  - 当前 `btnCommentImages` 点击处理只实现了抖音严格详情页链路
- 影响：
  - 用户在小红书详情页会看到“这里能下评论图片区”的提示，但 Popup 里根本没有对应入口
  - 这属于典型的“能力提示与真实能力不一致”
- 结论：
  - 要么补回 XHS Popup 评论图片区入口，要么把页面上下文提示改成真实能力

## P1

### B3. Dashboard 与页面桥接超时后会静默显示“空数据”

- 状态：2026-04-02 已修复
- 证据：
  - `src/dashboard/dashboard.js` 中 `sendToParent()` 超时后直接 `resolve(null)`
  - 同文件 `loadData()` 捕获异常后直接把 `allData = []`
  - 最终会渲染空表，而不是明确告诉用户“页面桥接超时/当前页面未响应”
- 影响：
  - 当内容脚本未就绪、页面桥接慢、或消息链短暂失败时，用户看到的是“数据没了”
  - 这种假空状态会误导用户误删数据、误以为采集失败或 IndexedDB 被清空
- 结论：
  - Dashboard 需要把“无数据”和“桥接失败/超时”分开显示

### B4. 批量任务入口与真实页面能力仍有错位风险

- 状态：2026-04-02 进一步缓解，仍待实机回归
- 证据：
  - `src/popup/popup.js` 现在会先向 content script 请求 `GET_PAGE_CONTEXT`
  - `src/content/contentDataRuntime.js` 已接入真实页面上下文解析，并把 `isStableSearchList` 与能力矩阵回传给 Popup
  - `src/popup/popup.js` 中抖音批量视频 / 批量评论只有在 `profile` 或 `search + isStableSearchList` 时才会放开
- 影响：
  - 相比此前纯 URL 猜测，首页、非稳定列表页、半详情页误点批量动作的概率已明显下降
  - 剩余风险主要在“浏览器实机环境下，页面结构变化导致稳定列表识别偏差”这一层
- 结论：
  - 代码层已经从“粗 URL 判定”推进到“内容脚本真实页面判定”
  - 下一步不再是继续猜规则，而是做 Chrome MCP / 实机回归，验证稳定列表识别是否覆盖真实页面变体

## P2

### B5. Background 批量任务启动失败时，浏览器 badge 可能残留

- 状态：2026-04-02 已修复
- 证据：
  - `src/background/index.js` 中 `START_BATCH_NOTES` / `START_BATCH_COMMENTS` 会先设置 badge
  - 然后才 `sendToTab(...)`
  - 若发送失败，当前分支没有回滚 badge 状态
- 影响：
  - 用户会看到扩展角标还在，但任务实际没启动成功
  - 这会进一步放大“任务状态和真实执行状态不一致”的问题
- 结论：
  - 这类问题优先级低于页面判定错误，但属于明显的状态一致性 bug

## 暂不归类为“当前功能 bug”的项

- `Popup / Dashboard / DouyinAdapter` 过胖：这是高风险结构债，但不是单一功能 bug
- `message envelope` 未统一：它会制造后续 bug，但当前更像 bug 温床
- `BaseBatchController` 尚未抽象：影响维护效率，不直接等同于用户眼前的故障

## 建议修复顺序

1. 先修 `Popup 页面判定`，把 `getModeFromUrl()` 从“默认 search”改成保守模式
2. 再修 `Popup 能力提示`，统一 XHS / Douyin 的真实入口与文案
3. 然后修 `Dashboard 空状态误导`，把“无数据”和“桥接失败”拆开
4. 最后补实机回归，验证 `GET_PAGE_CONTEXT` 在真实抖音搜索页 / 博主页上的稳定性

## 下一轮最值得直接动手的文件

- `src/popup/popup.js`
- `src/content/contentDataRuntime.js`
- `src/content/messageHandlers.js`
