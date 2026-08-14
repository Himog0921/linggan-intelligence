# Phase 2 高危级审查报告（High Risk / P1）

**审查日期**: 2026-04-28
**审查范围**: 消息传递系统、第三方依赖、React 稳定性、批量任务控制
**审查文件**: shared/messaging.js, content/messageHandlers.js, background/index.js, popup/App.jsx, dashboard/App.jsx, workbench/runtime/taskPoller.js, shared/managedTaskController.js, platforms/xhs/selectorHealth.js, platforms/douyin/selectorHealth.js

---

## 2.1 消息传递系统健壮性

### [P1-1] sendToBackground 没有超时机制 — Promise 挂起风险

**位置**: `shared/messaging.js:19-32`
**代码**:
```js
export function sendToBackground(action, data = {}) {
  if (!isContextValid()) {
    return Promise.reject(new Error('Extension context invalidated'));
  }
  return new Promise((resolve, reject) => {
    chrome.runtime.sendMessage({ action, ...data }, (response) => {
      if (chrome.runtime.lastError) {
        reject(chrome.runtime.lastError);
      } else {
        resolve(response);
      }
    });
  });
}
```
**问题**: `sendToBackground` 没有 `timeoutMs` 参数。如果 background SW 被 kill、卡死或正在处理耗时操作，Promise 会永远挂起。popup 中的 `loadStats`、`loadAccounts` 等调用会因此无响应。

**修复建议**: 添加可选的 `timeoutMs` 参数，或使用 `AbortSignal.timeout()` 包装。

---

### [P1-2] getDocumentCookie handler 缺少权限校验

**位置**: `content/messageHandlers.js:789-791`
**代码**:
```js
getDocumentCookie: async () => {
  return { success: true, cookieString: document.cookie || '' };
},
```
**问题**: `getDocumentCookie` handler 没有调用 `ensurePluginAuthorized()`。虽然 content script 的 matches 限制了域名，但如果小红书/抖音页面被 XSS 攻击，恶意脚本可以通过 `chrome.runtime.sendMessage` 请求获取当前页面的完整 cookie。

**修复建议**: 添加 `ensurePluginAuthorized()` 校验，或限制该 handler 仅响应来自 background 的消息。

---

### [P1-3] background 消息路由缺少 sender 验证

**位置**: `background/index.js:675-688`
**代码**:
```js
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  const handler = bgHandlers[message.action];
  if (handler) {
    Promise.resolve(handler(message, sender)).then(...)
    return true;
  }
});
```
**问题**: 消息路由没有验证 sender 的 origin 或 tab 身份。虽然 Chrome 扩展默认只允许同扩展的消息，但如果 content script 被 XSS 控制，攻击者可以发送任意消息触发敏感操作（如 `CLEAR_PLUGIN_AUTHORIZATION`、`DELETE_NOTE` 等）。

**修复建议**: 对敏感操作（删除数据、清除授权、修改配置）验证 `sender.tab` 存在且来自可信域名。

---

### [P1-4] taskPoller tick 锁非原子 — 重复执行风险

**位置**: `workbench/runtime/taskPoller.js:982-986`
**代码**:
```js
async function tick() {
  if (state.ticking) {
    return { success: true, skipped: true, reason: 'tick_in_progress' };
  }
  state.ticking = true;
  try {
    ...
  } finally {
    state.ticking = false;
  }
}
```
**问题**: `state.ticking` 检查和设置不是原子的。虽然 JavaScript 是单线程的，但在 async/await 场景下，如果一个 alarm 触发时上一个 tick 的 `finally` 块尚未执行（可能由于事件循环调度），两个 tick 可能同时进入临界区。

**修复建议**: 使用 `Atomics` 或 Promise 链确保 tick 的串行执行。

---

### [P1-5] taskPoller state 直接修改 — 状态不一致风险

**位置**: `workbench/runtime/taskPoller.js:476-480`
**代码**:
```js
state.activeTask = {
  ...state.activeTask,
  ...nextPatch,
};
```
**问题**: `state.activeTask` 被多处直接修改。虽然 `updateActiveTask` 做了浅拷贝，但嵌套对象（如 `payload`）仍然是引用共享。如果外部代码修改了 `payload` 的属性，`state.activeTask` 会间接被修改。

**修复建议**: 对 `payload` 等嵌套对象进行深拷贝。

---

## 2.2 第三方页面依赖脆弱性

### [P1-6] 选择器验证日期已过期 — 隐性失效风险

**位置**: `platforms/xhs/selectorHealth.js:11`, `platforms/douyin/selectorHealth.js:10`
**代码**:
```js
const SELECTOR_VERIFIED_AT = '2026-04-20T00:00:00+08:00';
```
**问题**: 今天是 2026-04-28，选择器验证日期已过期 8 天。小红书/抖音的 DOM 结构随时可能改版，过期的验证日期意味着这些选择器可能已经不再有效，但代码仍然声称它们是"已验证"的。

**修复建议**: 
- 建立自动化的选择器健康检查告警（超过 7 天未验证标红）
- 在 CI 或定期任务中运行选择器探针

---

### [P1-7] 小红书评论选择器过于宽泛 — 误匹配风险

**位置**: `platforms/xhs/selectorHealth.js:15-18`
**代码**:
```js
const COMMENTS_CONTAINER_SELECTORS = [
  '.comments-container',
  '[class*="comments"]',
  '.parent-comment',
  '.comment-item',
];
```
**问题**: `[class*="comments"]` 这种模糊匹配在页面改版时可能匹配到错误的元素（如广告评论区、推荐评论模块），导致采集逻辑异常。

**修复建议**: 使用更精确的选择器，或增加多个验证信号来确认匹配的正确性。

---

## 2.3 React 组件层稳定性

### [P1-8] Dashboard 没有 Error Boundary — 白屏风险

**位置**: `dashboard/App.jsx`
**问题**: 整个 Dashboard 没有包裹 Error Boundary。`renderCell` 函数处理大量数据转换逻辑，如果传入异常数据（如 `item.images` 是字符串而非数组），`item.images.length` 会抛出 TypeError，导致整个 dashboard 白屏。

**修复建议**: 在 `App` 组件外层包裹 `<ErrorBoundary>`，在 `renderCell` 中对所有数据访问进行防御性编程。

---

### [P1-9] Dashboard 直接操作 DOM — 虚拟 DOM 不一致

**位置**: `dashboard/App.jsx:510`
**代码**:
```jsx
onError={(e) => { e.target.style.display = 'none'; e.target.parentElement.textContent = '-'; }}
```
**问题**: 在 React 中直接操作 DOM 是不推荐的。`e.target.style.display = 'none'` 和 `e.target.parentElement.textContent = '-'` 会修改真实 DOM，但 React 的虚拟 DOM 并不知道这些变化，可能导致后续渲染时状态不一致。

**修复建议**: 使用 React state 控制 img 的显示/隐藏，或在 `onError` 中调用 `setState`。

---

### [P1-10] Popup 使用全局变量传递模态框配置 — 反模式

**位置**: `popup/App.jsx:948`
**代码**:
```js
window._commentLimitOptions = options;
```
**问题**: 使用全局变量在组件之间传递状态是 React 反模式。这会导致：
- 内存泄漏（窗口关闭前不会释放）
- 并发问题（多个模态框同时打开时互相覆盖）
- 测试困难

**修复建议**: 使用 React Context 或 props 传递模态框配置。

---

### [P1-11] Popup message listener 闭包陈旧风险

**位置**: `popup/App.jsx:232-274`
**代码**:
```js
useEffect(() => {
  const listener = (message) => {
    if (message.action === MSG.PROGRESS) {
      setProgressVisible(true);
      showNotice(message.error.message, 'warning'); // 引用 effect 执行时的 showNotice
    }
  };
  chrome.runtime.onMessage.addListener(listener);
  return () => chrome.runtime.onMessage.removeListener(listener);
}, []);
```
**问题**: listener 在 effect 执行时捕获了当时的 `showNotice` 引用。虽然 `showNotice` 被 `useCallback` 包裹且依赖数组为空，但如果未来修改了 `showNotice` 的依赖，listener 会使用旧版本。

**修复建议**: 使用 ref 存储 `showNotice`，或在 listener 中直接调用 `setNotice` 而不是通过 `showNotice`。

---

## 2.4 批量任务控制逻辑

### [P1-12] managedTaskController pause/resume 竞态条件

**位置**: `shared/managedTaskController.js:15-25`
**代码**:
```js
pause() {
  if (!state.isRunning) return;
  state.isPaused = true;
},
resume() {
  if (!state.isRunning) return;
  state.isPaused = false;
  if (state.pauseResolve) {
    state.pauseResolve();
    state.pauseResolve = null;
  }
},
```
**问题**: `pause()` 和 `resume()` 没有锁保护。如果用户快速点击暂停/继续，`pauseResolve` 可能被错误地 resolve 或泄漏。更严重的是，如果 `resume()` 被连续调用两次，第二次调用时 `state.pauseResolve` 已经是 null，但 `state.isPaused` 被设为 false，这本身没有问题。但如果 `pause()` 在 `waitIfPaused()` 检查之后、`await` 之前被调用，可能有竞态。

**修复建议**: 使用队列或原子操作确保 pause/resume 的顺序一致性。

---

### [P1-13] managedTaskController stop 后状态清理不完整

**位置**: `shared/managedTaskController.js:27-31`
**代码**:
```js
stop() {
  if (!state.isRunning) return;
  state.isStopping = true;
  controller.resume();
},
```
**问题**: `stop()` 调用了 `resume()` 来解除暂停，但如果 `runTask` 中的代码在 `shouldStop()` 检查之后、`waitIfPaused()` 调用之前执行，任务可能在检查完 `shouldStop()` 后进入 `waitIfPaused()`，然后被 `resume()` 唤醒，但此时 `isStopping` 已经是 true，下一次循环会退出。这个竞态窗口很小，但在高频率暂停/停止操作下可能发生。

**修复建议**: 在 `waitIfPaused()` 返回后立即检查 `shouldStop()`。

---

### [P1-14] asyncDispatch fire-and-forget 无跟踪

**位置**: `content/messageHandlers.js:230-242`
**代码**:
```js
if (msg.asyncDispatch) {
  Promise.resolve()
    .then(() => runNoteCollection(remoteRun))
    .catch((error) => {
      console.error('[灵感爆爆爆] 远程单篇笔记采集失败:', error);
    });
  return {
    success: true,
    accepted: true,
    pending: true,
    collectionRunId: remoteRun?.collectionRunId || '',
  };
}
```
**问题**: asyncDispatch 模式下任务被 fire-and-forget 启动，没有任何跟踪机制。如果 content script 崩溃或页面刷新，background 只能通过 `pollActiveTask` 定期检查。但如果 `runNoteCollection` 内部抛出异常，`collectionRunStore` 可能没有被正确更新，任务将永远显示为 "pending"。

**修复建议**: 使用 `chrome.runtime.sendMessage` 在任务完成/失败时主动通知 background，或在 background 中添加任务心跳超时检测。

---

### [P1-15] taskPoller 与 content script 状态不一致

**位置**: `workbench/runtime/taskPoller.js:833-868`
**问题**: 当 `pollActiveTask` 中 `result?.success` 为 false 且是 recoverable 错误时，任务被标记为 paused/failed。但此时 content script 中的批量采集循环可能仍在执行。这导致后台状态和前台执行不一致：
- 后台认为任务已暂停
- 但 content script 仍在采集数据、写入 IndexedDB

**修复建议**: 在状态变更时，通过 `sendToTab` 向 content script 发送强制停止消息，确保前后台状态一致。

---

## Phase 2 审查统计

| 级别 | 数量 | 分类 |
|------|------|------|
| P1（高危）| 15 项 | 消息传递 5 项，第三方依赖 2 项，React 稳定性 4 项，批量任务 4 项 |
