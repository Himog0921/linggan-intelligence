# Phase 1 致命级审查报告（Fatal / P0）

**审查日期**: 2026-04-28
**审查范围**: MV3 合规性、数据丢失风险、XSS 安全
**审查文件**: manifest.json, background/index.js, db/*, sync/flywheelSync.js, popup/popup.js, platforms/xhs/antiDetect.js, platforms/xhs/batchController.js

---

## 1.1 Chrome MV3 合规性

### [P0-1] web_accessible_resources 过度暴露 — 信息泄露风险

**位置**: `manifest.json:76-77, 92-93`
**代码**:
```json
"resources": [
  "*.js",
  "*.css",
  ...
]
```
**问题**: 将 `*.js` 和 `*.css` 作为 web_accessible_resources 暴露给小红书/抖音域名，意味着目标页面可以通过 `chrome-extension://<id>/xxx.js` 直接访问插件的所有 JS/CSS 文件。攻击者或平台方可以：
- 分析反检测策略（`antiDetect.js` 的逻辑完全暴露）
- 探测插件版本和功能边界
- 发现可利用的漏洞

**修复建议**: 仅暴露特定必需文件，移除 `*.js` 和 `*.css` 通配符。

---

### [P0-2] debugger 权限 — CWS 审核高风险

**位置**: `manifest.json:13`
**代码**:
```json
"permissions": [..., "debugger", ...]
```
**问题**: `debugger` 是 Chrome Web Store 审核的敏感权限。虽然在 `DISPATCH_ESC` handler（background/index.js:908-933）中用于模拟 Esc 键关闭弹窗，但 CWS 可能会：
- 要求详细说明使用场景
- 拒绝上架或延迟审核
- 在后续更新中要求移除

**修复建议**: 评估是否可以通过 `chrome.scripting.executeScript` 注入脚本触发 `keydown` 事件来替代 debugger API。

---

### [P0-3] Service Worker 顶层直接执行长异步任务 — 生命周期风险

**位置**: `background/index.js:2016-2018`
**代码**:
```js
chrome.alarms?.create(WORKBENCH_TASK_POLL_ALARM, { periodInMinutes: INITIAL_WORKBENCH_TASK_POLL_MINUTES });
chrome.alarms?.create(WORKBENCH_STATION_HEARTBEAT_ALARM, { periodInMinutes: 1 });
void runWorkbenchTaskPollTick();
void runExecutionStationHeartbeatTick();
```
**问题**: SW 文件顶层直接执行网络密集型任务。`runWorkbenchTaskPollTick` 内部涉及 fetch 任务列表、claim lease、renew lease 等多个网络调用，如果总耗时超过 Chrome SW 的 30 秒生命周期限制，SW 会被 kill，导致：
- 任务状态丢失
- 心跳中断
- 工单认领失败

**修复建议**: 顶层仅注册 alarm，不执行异步操作。所有异步逻辑应在 alarm handler 内执行（已部分实现，但顶层调用应移除）。

---

### [P0-4] BLOCK_MEDIA 规则全局副作用 — 异常残留风险

**位置**: `background/index.js:883-897`
**代码**:
```js
condition: {
  urlFilter: '*',
  resourceTypes: ['image', 'media'],
}
```
**问题**: `urlFilter: '*'` 会在所有 tab 上阻止图片和媒体加载，不限于采集目标 tab。如果 content script 因页面刷新/崩溃导致 `UNBLOCK_MEDIA` 消息未发送，规则将永久残留，用户所有页面的图片都无法加载。

**修复建议**: 
1. 在条件中添加更精确的 URL 过滤（限定小红书/抖音 CDN 域名）
2. 或在 SW 启动时（已有）和 alarm tick 时定期清理残留规则
3. 使用 `tabId` 条件限定（declarativeNetRequest 支持按 tab 过滤）

---

## 1.2 数据丢失风险

### [P0-5] syncToFlywheel 部分失败仍返回 success — 数据静默丢失

**位置**: `sync/flywheelSync.js:205-217`
**代码**:
```js
if (batchErrors.length > 0 && successfulBatchCount === 0) {
  return { success: false, error: batchErrors.join('; ') };
}
return { success: true, imported, skipped, details };
```
**问题**: 当部分 batch 失败时（`successfulBatchCount > 0` 但 `batchErrors.length > 0`），函数仍返回 `success: true`。调用方（popup 同步按钮）会显示"同步成功"，但实际上部分数据未上传且未被重试。

**修复建议**: 
- 任何 `batchErrors.length > 0` 都应返回 `success: false`
- 或将失败的数据保留在本地队列中供后续自动重试

---

### [P0-6] flywheelSync cachedConfig 无 TTL — 配置不一致

**位置**: `sync/flywheelSync.js:11-27`
**代码**:
```js
let cachedConfig = null;
async function readFlywheelStorage() {
  if (cachedConfig) return cachedConfig;
  ...
}
```
**问题**: 模块级缓存 `cachedConfig` 一旦设置永不刷新。如果用户在 popup 中修改了 serverUrl 或 apiToken，background SW 中的缓存仍返回旧值，导致同步到错误的服务器或使用错误的 token。

**修复建议**: 
- 添加 TTL（如 30 秒）
- 或在 `writeFlywheelStorage` 时通过 `chrome.storage.onChanged` 广播通知刷新

---

### [P0-7] workbenchOutboxStore.enqueue 竞态条件 — 重复写入

**位置**: `db/workbenchOutboxStore.js:53-61`
**代码**:
```js
const existing = await getByKey(row.idempotencyKey);
if (existing) return existing;
await db.workbenchOutbox.put(row);
```
**问题**: `getByKey` 和 `put` 之间没有事务保护。多个 alarm tick 交错执行时，相同的 idempotencyKey 可能重复写入。

**修复建议**: 使用 `db.transaction('rw', db.workbenchOutbox, async () => { ... })` 包裹，或在 `idempotencyKey` 上添加唯一索引。

---

### [P0-8] legacyDataMaintenance.js 全表加载 — OOM 风险

**位置**: `db/legacyDataMaintenance.js:20-26`
**代码**:
```js
const [notes, comments, authors, mediaAssets] = await Promise.all([
  db.notes.toArray(),
  db.comments.toArray(),
  db.authors.toArray(),
  db.mediaAssets.toArray(),
]);
```
**问题**: `toArray()` 将整个表加载到内存。数万条记录时，MV3 有限内存环境下可能 OOM，导致 SW 崩溃和数据丢失。

**修复建议**: 使用分页游标（`db.notes.toCollection().each()` 或 `offset/limit` 分批次）逐批处理。

---

### [P0-9] noteStore/authorStore bulkUpsert 缺少显式事务

**位置**: `db/noteStore.js:9-10`, `db/authorStore.js:13-14`
**代码**:
```js
async bulkUpsert(notes) {
  await db.notes.bulkPut((notes || []).map(normalizeNoteRecord));
}
```
**问题**: 与 `commentStore.bulkUpsert` 的显式事务包裹不同，这里直接调用 `bulkPut`。如果 `normalizeNoteRecord` 抛出异常（如传入数据包含不可序列化对象），已处理的部分记录状态不确定。

**修复建议**: 添加 `db.transaction('rw', db.notes, async () => { ... })` 包裹。

---

## 1.3 XSS 安全

### [P0-10] innerHTML 模式属高风险代码债

**位置**: `popup/popup.js:737, 1249, 1253`, `platforms/xhs/antiDetect.js:95`, `platforms/xhs/batchController.js:958`
**代码示例**:
```js
// popup.js:737
tagsEl.innerHTML = tags.map((tag) => `<span class="context-tag">${escapeHtml(tag)}</span>`).join('');

// popup.js:1253
accountListEl.innerHTML = accounts.map((a) => {
  return `<div...>${escapeHtml(a.name)}...</div>`;
}).join('');
```
**问题**: 虽然当前都调用了 `escapeHtml()`（实现正确，覆盖了 `& < > " '`），但 `innerHTML` 仍是不安全的模式。未来若有开发者忘记调用 `escapeHtml`，或传入的变量包含未转义的 HTML，将直接引入 XSS。

**修复建议**: 使用 `document.createElement` + `textContent` 替代 `innerHTML`。

---

### [P0-11] dangerouslySetInnerHTML 依赖 icon() 内部约束

**位置**: `Notice.jsx:13`, `Toast.jsx:67`, `ButtonGroup.jsx:414/470`, `dashboard/App.jsx:60/653/674/769`
**代码**:
```jsx
dangerouslySetInnerHTML={{ __html: icon(meta.icon, { size: 16 }) }}
```
**问题**: 目前 `icon()` 输入是硬编码字符串键，输出是可信 SVG，所以是安全的。但如果未来有人将用户输入（如评论内容）传入 `icon()`，会立即变成 XSS。

**修复建议**: 在 `icon()` 函数中添加输出 sanitize（如 DOMPurify 轻量版），或对 `dangerouslySetInnerHTML` 的使用添加显式注释警告。

---

## Phase 1 审查统计

| 级别 | 数量 | 分类 |
|------|------|------|
| P0（致命）| 11 项 | MV3 合规性 4 项，数据丢失 5 项，XSS 2 项 |
