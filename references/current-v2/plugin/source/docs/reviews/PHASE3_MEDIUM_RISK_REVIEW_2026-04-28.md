# Phase 3 中危级审查报告（Medium Risk / P2）

**审查日期**: 2026-04-28
**审查范围**: 代码质量、数据模型一致性、错误处理完整性、CSS/样式层
**审查文件**: content/index.js, shared/utils.js, platforms/douyin/index.js, content/contentDataRuntime.js, db/*, themes/themeManager.js, content.css, popup/popup.css, dashboard/dashboard.css, shared/selectorHealth.js, content/components/Toast.jsx

---

## 3.1 代码质量与可维护性

### [P2-1] content/index.js 消息监听早期返回未关闭通道

**位置**: `content/index.js:188-189`
**代码**:
```js
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!isContextValid()) return; // 扩展已重载，忽略消息
```
**问题**: 当 `isContextValid()` 返回 false 时，直接 `return` 没有调用 `sendResponse`。由于 listener 返回了 `true`（第 212 行），Chrome 会保持消息通道开放等待异步响应，但这里永远不会响应，导致 popup/background 的 Promise 挂起直到超时。

**修复建议**: `return sendResponse({ success: false, error: 'Extension context invalidated' });`

---

### [P2-2] normalizeServerUrl 默认使用不安全协议

**位置**: `shared/utils.js:463-468`
**代码**:
```js
export function normalizeServerUrl(serverUrl = '', fallback = '') {
  const raw = String(serverUrl || fallback || '').trim();
  return raw
    .replace(/\/+$/, '')
    .replace(/^(?!https?:\/\/)/, 'http://');
}
```
**问题**: 当用户输入 `example.com` 时，函数前缀为 `http://` 而非 `https://`。在 2026 年的网络环境中，默认使用 HTTP 是不安全的，可能导致 token 等敏感信息明文传输。

**修复建议**: 将默认协议改为 `https://`。

---

### [P2-3] safeUrl 硬编码小红书域名，不支持抖音

**位置**: `shared/utils.js:156-162`
**代码**:
```js
export function safeUrl(url) {
  if (!url) return '';
  if (url.startsWith('http')) return url;
  if (url.startsWith('//')) return 'https:' + url;
  if (url.startsWith('/')) return 'https://www.xiaohongshu.com' + url;
  return url;
}
}
```
**问题**: 相对路径 `/` 被硬编码指向 `xiaohongshu.com`。抖音页面（`douyin.com`）的相对 URL 会被错误地解析到小红书域名。

**修复建议**: 接受 `baseOrigin` 参数，或根据当前 `location.hostname` 动态判断。

---

### [P2-4] DouyinAdapter._sendBackgroundAction 重复实现消息发送逻辑

**位置**: `platforms/douyin/index.js:142-161`
**代码**: 自定义 Promise 包装 `chrome.runtime.sendMessage`，处理 `lastError` 和 response error。
**问题**: 该逻辑与 `shared/messaging.js` 中的 `sendToBackground` 几乎完全一致，但缺少 `isContextValid()` 前置检查。重复代码增加维护成本，且两个实现可能逐渐 diverge。

**修复建议**: 复用 `shared/messaging.js` 中的 `sendToBackground`。

---

### [P2-5] contentDataRuntime factory 创建 dashboardBridge 但未注册监听器

**位置**: `content/contentDataRuntime.js:45-51`
**代码**:
```js
const dashboardBridge = createDashboardBridge({
  MSG,
  noteStore,
  commentStore,
  authorStore,
  downloadNoteMediaFromRecord,
});
```
**问题**: `createDashboardBridge` 返回的对象包含 `registerDashboardBridge()` 方法，但 `contentDataRuntime.js` 中没有调用它。监听器注册被延迟到 `content/index.js:95-103` 单独处理，造成工厂职责不完整，调用方需要知道内部细节才能正确完成初始化。

**修复建议**: 在 `createContentDataRuntime` 内部调用 `dashboardBridge.registerDashboardBridge()`，或在返回对象中暴露需由调用方执行的初始化步骤文档。

---

### [P2-6] initThemeManager 静默失败

**位置**: `content/index.js:35`
**代码**:
```js
initThemeManager().catch(() => {});
```
**问题**: 主题管理器初始化失败被完全吞掉。如果 `chrome.storage.local.get` 抛出异常（如 MV3 存储配额已满），后续所有依赖 `getCurrentTheme()` 的组件会使用错误的默认主题，且没有任何日志或提示。

**修复建议**: 至少记录 `console.warn`。

---

## 3.2 数据模型一致性

### [P2-7] IndexedDB 迁移从未清理废弃索引

**位置**: `db/index.js`（v1→v11）
**问题**: Dexie 的 `.stores()` 在升级时**只添加新索引，从不删除旧索引**。v1 到 v11 累积了大量历史索引（如 `syncStatus` 在早期版本添加但可能已 unused），导致：
- 数据库文件体积膨胀
- 写入性能下降（每个索引都需要维护）
- 新开发者看到 schema 时无法区分活跃索引和死索引

**修复建议**: 在版本迁移中使用 `db.version(N).upgrade(tx => { ... })` 显式清理已知废弃索引，或在注释中标记索引活跃状态。

---

### [P2-8] normalizeNoteRecord 先展开后覆盖，存在字段污染风险

**位置**: `db/recordNormalization.js:97-141`
**代码**:
```js
export function normalizeNoteRecord(record = {}) {
  const platform = inferPlatform(record);
  // ...
  return {
    ...record,        // 先展开原始对象的所有字段
    platform,         // 再覆盖特定字段
    platformContentId,
    // ...
  };
}
```
**问题**: 如果上游采集代码传入了一个包含未知/废弃字段的对象（如早期版本的 `releaseDate` 现已改为 `publishedAt`），这些字段会被保留在记录中并写入数据库，导致数据模型逐渐腐化。

**修复建议**: 使用显式字段白名单构建返回对象，而非先 `...record`。

---

### [P2-9] collectionRunStore.upsert 缺少显式事务

**位置**: `db/collectionRunStore.js:64-65`
**代码**:
```js
async upsert(run) {
  await db.collectionRuns.put(normalizeRunRecord(run, { preserveCreatedAt: true }));
},
```
**问题**: 与 P0-9 类似，`put` 直接调用未包裹在事务中。如果 `normalizeRunRecord` 抛出异常或存储层发生竞态，数据一致性无法保证。

**修复建议**: 添加 `db.transaction('rw', db.collectionRuns, async () => { ... })` 包裹。

---

### [P2-10] mediaAssetStore 批量操作无事务保护

**位置**: `db/mediaAssetStore.js:5-11`
**代码**:
```js
async upsert(asset) {
  await db.mediaAssets.put(normalizeMediaAssetRecord(asset));
},
async bulkUpsert(assets) {
  await db.mediaAssets.bulkPut((assets || []).map(normalizeMediaAssetRecord));
},
```
**问题**: `upsert` 和 `bulkUpsert` 都没有事务包裹。`normalizeMediaAssetRecord` 中涉及复杂的字段推导（如 `contentId` 的推断逻辑），如果中间抛出异常，部分数据可能已写入。

**修复建议**: 统一添加事务包裹，与 `commentStore.bulkUpsert` 的做法保持一致。

---

### [P2-11] accountStore.getAll 产生 N+1 写入

**位置**: `db/accountStore.js:69-76`
**代码**:
```js
async getAll() {
  const accounts = await db.accounts.toArray();
  const results = [];
  for (const account of accounts) {
    results.push(await resetIfNewDay(account)); // 每次可能触发一次 put
  }
  return results;
},
```
**问题**: `resetIfNewDay` 在日期切换时会对每个账号执行 `db.accounts.put(updated)`。如果用户有 100 个账号，一次 `getAll()` 会产生 100 次独立写入。在 MV3 的 Service Worker 中，这种写放大可能影响性能和生命周期。

**修复建议**: 使用批量更新（`bulkPut`）或事务包裹整个循环。

---

### [P2-12] resetIfNewDay 读写无事务隔离

**位置**: `db/accountStore.js:34-42`
**代码**:
```js
async function resetIfNewDay(account) {
  const today = todayStr();
  if (account.lastResetDate !== today) {
    const updated = { ...account, dailyQuotaUsed: 0, lastResetDate: today };
    await db.accounts.put(updated);
    return updated;
  }
  return account;
}
```
**问题**: 读取账号状态和写入更新之间没有事务保护。两个并发的 alarm tick 可能同时判定需要重置，导致二次写入（虽然结果相同，但存在竞态窗口）。

**修复建议**: 使用 `db.transaction('rw', db.accounts, async () => { ... })` 包裹读写逻辑。

---

## 3.3 错误处理完整性

### [P2-13] 消息监听器 catch 块引用可能未定义的对象

**位置**: `content/index.js:206-211`
**代码**:
```js
.catch((err) => {
  sendResponse(normalizeRuntimeMessageResponse(message.action, {
    success: false,
    error: err.message,
  }));
});
```
**问题**: 如果 `loadContentDataRuntime()` 本身失败（第 190 行），catch 块试图访问 `message.action`。虽然 `message` 在闭包中通常存在，但如果监听器实现未来被重构为异步提取消息，这里可能出现 `message is undefined` 的二次错误。

**修复建议**: 在 catch 块顶部添加防御性检查：`const action = message?.action || 'unknown';`

---

### [P2-14] _ensurePluginAuthorized 双重通知风险

**位置**: `platforms/douyin/index.js:51-58`
**代码**:
```js
async _ensurePluginAuthorized() {
  try {
    return await assertActivePluginAuthorization();
  } catch (error) {
    showDouyinToast(String(error?.userMessage || error?.message || '...'), 'warning');
    throw error;  // 重新抛出
  }
},
```
**问题**: 错误被捕获后显示 toast，然后重新抛出。调用方（如 `handleButtonClick`）通常也会捕获并再次显示 toast，导致用户看到两个相同的错误提示。

**修复建议**: 要么不重新抛出（让本层作为最终处理），要么在调用方检测已处理的错误避免重复提示。

---

### [P2-15] 安全验证检测读取整个页面 innerText

**位置**: `platforms/douyin/securityChallenge.js:85-91`
**代码**:
```js
const text = String(
  root?.body?.innerText
  || root?.documentElement?.innerText
  || root?.body?.textContent
  || ''
).trim();
```
**问题**: `innerText` 会强制浏览器重新计算整个文档的 layout 和 text rendering。在抖音这种重度 DOM 页面（可能有数万个节点）上，每次检测都读取 `innerText` 会造成明显的性能抖动，尤其在批量任务的高频检查循环中。

**修复建议**: 使用 `textContent` 替代 `innerText`（不需要 layout 计算），或限制检查范围到特定容器节点。

---

### [P2-16] dashboardBridge SYNC_TO_WORKBENCH 的兜底响应不够健壮

**位置**: `content/dashboardBridge.js:68-82`
**代码**:
```js
[MSG.SYNC_TO_WORKBENCH]: async (data) => {
  try {
    const result = await chrome.runtime.sendMessage({ ... });
    return result || { success: false, error: 'No response from background' };
  } catch (err) {
    return { success: false, error: err.message };
  }
},
```
**问题**: `result` 如果是 `{ success: false, error: '...' }` 这样的对象，`||` 不会触发，这是正确的。但如果 `result` 是一个 Error 实例（某些情况下 `sendMessage` 的回调可能收到异常对象），`result || fallback` 会返回 Error 对象，破坏下游的类型预期。

**修复建议**: 显式检查 `result?.success` 是否为布尔值，而非依赖 truthy/falsy。

---

### [P2-17] getImageSize fallback 无超时保护

**位置**: `shared/utils.js:296-315`
**代码**:
```js
return new Promise((resolve) => {
  const img = new Image();
  img.onload = () => { ... resolve({ width, height }); };
  img.onerror = () => { ... resolve({ width: 0, height: 0 }); };
  img.src = objectUrl;
});
```
**问题**: `new Image()` 的加载没有设置超时。如果网络层出现奇怪状态（既不 onload 也不 onerror），Promise 将永远挂起。在 `downloadMediaFile` 的循环中，这可能导致整个下载流程卡住。

**修复建议**: 添加 `setTimeout(() => resolve({ width: 0, height: 0 }), 5000)` 兜底。

---

## 3.4 CSS / 样式层

### [P2-18] content.css 过于单薄，大量样式通过 JS 内联注入

**位置**: `content.css`（仅 12 行）
**问题**: 几乎所有 content script UI 的样式（iframe overlay、toast、按钮等）都通过 `Object.assign(element.style, {...})` 在 JS 中硬编码。这导致：
- 样式无法通过 CSS 主题切换覆盖
- 调试困难（开发者工具中分散在各处）
- 重复代码（dashboardBridge、uiInjector、Toast.jsx 都有各自的 inline style 对象）

**修复建议**: 将稳定的样式抽取到 `content.css` 的 class 中，JS 只负责动态值（如位置、显示状态）。

---

### [P2-19] themeManager 重复注册 storage 监听器

**位置**: `themes/themeManager.js:17-31`
**代码**:
```js
export async function initThemeManager() {
  const stored = await chrome.storage.local.get(THEME_KEY);
  cachedTheme = stored[THEME_KEY] || 'default';

  chrome.storage.onChanged.addListener((changes, area) => {
    ...
  });
  return cachedTheme;
}
```
**问题**: 如果 `initThemeManager()` 被意外调用多次（例如 content script 热重载、SPA 导航后重新初始化），会注册多个 `onChanged` 监听器，导致同一个 storage 变化触发多次回调。虽然当前代码只在 `content/index.js:35` 调用一次，但这是一个隐性的泄漏风险。

**修复建议**: 使用标志位确保监听器只注册一次，或在注册前移除旧监听器。

---

### [P2-20] popup.css 使用 `:has()` 选择器，兼容性有限

**位置**: `popup/popup.css:1035-1037`
**代码**:
```css
.depth-radio-option:has(input:checked) {
  background: var(--brand);
}
```
**问题**: `:has()` 选择器需要 Chrome 105+。虽然当前 Chrome 版本已支持，但部分企业环境或延迟更新的用户可能运行旧版 Chrome，导致该样式不生效，用户无法 visually 区分选中的 radio option。

**修复建议**: 添加 JS 驱动的 `.active` / `.selected` class 作为兜底方案。

---

### [P2-21] dashboard.css 使用 webkit-only 文本截断

**位置**: `dashboard/dashboard.css:511-517`
**代码**:
```css
.cell-clamp-3 {
  display: -webkit-box;
  overflow: hidden;
  text-overflow: ellipsis;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
```
**问题**: `-webkit-line-clamp` 是 webkit 私有实现，虽然在 Chrome 中有效，但属于非标准 CSS。未来如果 dashboard 需要迁移到其他渲染环境（如 Electron、Firefox），这段样式会失效。

**修复建议**: 该问题影响较小，因为项目目标仅为 Chrome 扩展。建议添加注释说明此限制即可。

---

## Phase 3 审查统计

| 级别 | 数量 | 分类 |
|------|------|------|
| P2（中危）| 21 项 | 代码质量 6 项，数据模型 6 项，错误处理 5 项，CSS/样式 4 项 |
