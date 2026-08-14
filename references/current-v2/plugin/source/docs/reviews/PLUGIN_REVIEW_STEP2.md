# 灵感爆爆爆插件审查报告 — Step 2 功能矩阵审查

> 审查日期：2026-05-07
> 审查阶段：Step 2 — 核心功能链路审查
> 审查人：Cindy
> 审查范围：5 条主链路（页面识别、批量采集、远程任务、数据回写、用户流程）

---

## 审查方法

逐链路阅读核心源码，评估：**完整性、准确性、异常恢复、可维护性**。每条链路给出「结论 + 发现 + 风险等级」。

---

## 链路一：页面识别与采集准确性

**核心文件**：`src/content/contentRouter.js`、`src/platforms/xhs/noteCollector.js`

### 1.1 页面识别

| 检查项 | 状态 | 说明 |
|---|---|---|
| 平台路由 | 通过 | `resolveContentPlatform(hostname)` 按域名区分小红书/抖音 |
| 页面类型检测 | 通过 | Popup 通过 URL 正则 + content script `GET_PAGE_CONTEXT` 双重确认 |
| 能力矩阵 | 通过 | `getPageCapabilities` 根据平台+页面类型动态决定可用操作 |

**发现 #FUNC-1** 🟢 低风险 — 页面类型检测分散

平台检测逻辑分布在多个位置：
- Popup：`detectPlatformByUrl(url)` + `getModeFromUrl(url, platform)`
- Content script：`contentRouter.resolvePlatform()`
- Background：`getActiveWorkbenchTaskForMessage`

虽然各位置检测方式一致（都是 hostname 正则），但分散在三个层级增加了维护成本。平台新增时需要改多处。

### 1.2 单篇笔记采集

**技术路径**：注入 `noteMap.js` → 读取 `window.__INITIAL_STATE__.note.noteDetailMap` → 字段映射 → 写入 IndexedDB

| 检查项 | 状态 | 说明 |
|---|---|---|
| 数据来源稳定性 | 警告 | 依赖小红书 `__INITIAL_STATE__`，平台改版会直接影响采集 |
| 数据完整性检查 | 通过 | `isCollectedNoteUsable` 检查标题/媒体/互动数，detail_probe 模式要求更严格 |
| 重试机制 | 通过 | `collectNote` 有 3 次重试，间隔 1.5s 递增 |
| 笔记 ID 匹配 | 通过 | `expectedNoteId` 精确匹配，防止拿到错误笔记 |
| 时间解析 | 优秀 | `parseXhsPublishedAt` 支持相对时间（"2小时前"）、中文日期、标准格式 |

**发现 #FUNC-2** 🟡 中风险 — `__INITIAL_STATE__` 依赖无兜底

如果小红书页面结构变化导致 `__INITIAL_STATE__` 不存在或字段路径变化，`collectNote` 会直接失败。虽然代码有 `noteMap` 为空检测，但没有 fallback 到 DOM 抓取。

**建议**：保留 DOM 抓取 fallback（代码历史中曾有 `domCollector.js`，可评估恢复）。

**发现 #FUNC-3** 🟢 低风险 — 视频流选择逻辑复杂

`pickBestVideoStream` 在 `shared/utils.js` 中处理多种视频格式（h264、hevc、av1 等），但没有看到此函数的实现。从调用端看逻辑依赖于 `note.video.media.stream` 结构，如果平台增加新编码格式，可能需要更新。

### 1.3 抖音视频采集

| 检查项 | 状态 | 说明 |
|---|---|---|
| 运行时加载 | 通过 | `loadDouyinRuntime()` 动态加载，与小红书代码隔离 |
| API 拦截 | 通过 | `douyinApiCapture.js` 拦截 `aweme/v1/web/aweme/detail/` 等 API |
| 数据归一化 | 通过 | `mapAweme` 将抖音数据结构映射到统一 schema |

---

## 链路二：批量采集完整性

**核心文件**：`src/platforms/xhs/batchController.js`、`src/platforms/xhs/noteCollector.js`

### 2.1 笔记列表发现

| 检查项 | 状态 | 说明 |
|---|---|---|
| 滚动加载策略 | 优秀 | `discoverWithScroll` 处理懒加载，博主页和搜索页使用不同参数 |
| 虚拟列表兼容 | 优秀 | 使用 `allNotes` Map 累积，不依赖回顶后的 DOM |
| 视觉排序 | 优秀 | 按 `_top` + `_left` 排序，解决瀑布流双列布局乱序 |
| 到底检测 | 通过 | 博主页使用 `bottomConfirmationRounds`（6 轮确认），搜索页 2 轮 |
| 探针回弹 | 通过 | `probeProfileBottom` 回弹再下拉触发加载 |

**发现 #FUNC-4** 🟡 中风险 — 滚动参数硬编码

`discoverWithScroll` 的关键参数在 `buildDiscoveryPlan` 中硬编码：
- 博主页目标轮数：`Math.min(Math.max(expectedCount, 28), 80)`
- 滚动步长比例：`0.55`（博主页）/ `0.68`（搜索页）
- 稳定延迟：`1300ms`（博主页）/ `900ms`（搜索页）

这些参数基于经验值，如果平台加载速度变化（如网络慢、服务器响应慢），可能导致发现不完整或过多等待。

**建议**：考虑将关键参数暴露为配置项，或根据页面实际加载速度自适应调整。

### 2.2 逐篇采集流程

```
滚动定位卡片 → 点击打开 → 等待页面响应（URL 变化或弹窗）
  → 等待 __INITIAL_STATE__ 就绪 → 等待数据稳定（连续 2 次完整）
  → 采集（最多 3 次重试） → 返回列表页
```

| 检查项 | 状态 | 说明 |
|---|---|---|
| 弹窗方式 | 通过 | 主流路径，模拟用户点击 |
| URL 导航 fallback | 通过 | 弹窗失败时导航到 `/explore/{noteId}` |
| 数据稳定性等待 | 通过 | `_waitForNoteDataStable` 要求连续 2 次完整数据 |
| 返回列表页 | 通过 | `history.back()` 超时后直接导航回原始 URL |

**发现 #FUNC-5** 🟠 高风险 — 批量采集失败处理可能丢失进度

`_captureLoop` 中如果某个笔记采集失败：
```javascript
this.failed.push({ noteId: noteInfo.noteId, error: '弹窗和导航方式均失败' });
```

失败笔记仅记录 ID 和错误文本，**不保存已部分采集的数据**。如果笔记详情页已经加载但 `collectNote` 失败（如数据不完整），该笔记完全丢失。

**建议**：在 fallback 路径中，即使 `collectNote` 失败，也保存从列表页已获取的表面数据（标题、封面、点赞数）。

### 2.3 风控与验证码

| 检查项 | 状态 | 说明 |
|---|---|---|
| 验证码监控 | 通过 | `watchCaptcha` 检测验证码弹窗，自动暂停 |
| 风控代码 300017 | 通过 | `isErrorCode300017` 检测，自动切换账号 |
| 风险页面暂停 | 通过 | `_pauseForRiskControl` 检测安全验证页 |

---

## 链路三：远程任务执行

**核心文件**：`src/workbench/runtime/taskPoller.js`、`src/workbench/runtime/taskLeaseClient.js`

### 3.1 任务生命周期

```
poll tick → claim lease → capability check → dispatch task
  → content script 执行 → 轮询结果 → task complete/failed
```

| 检查项 | 状态 | 说明 |
|---|---|---|
| 租约机制 | 优秀 | `claimCollectionTaskLease` + `renewCollectionTaskLease` 防止多工位冲突 |
| 启动超时 | 优秀 | 45 秒未启动 → 自动释放 + 2 分钟后重试 |
| 断线恢复 | 优秀 | 区分 monitor 任务（标记失败）和普通任务（标记暂停） |
| orphaned task | 通过 | 10 分钟无心跳自动释放 |
| 任务控制 | 优秀 | 支持 pause/resume/stop/delete，去重控制请求 |

**发现 #FUNC-6** 🟡 中风险 — 任务恢复逻辑对 monitor 任务过于严格

```javascript
if (hydrated.workbenchStatus === 'paused' && isMonitorTask(hydrated) && isRecoverableConnectionError(hydrated.errorMessage)) {
  await patchTask(hydrated.taskId, { status: 'failed', progress: 100, ... });
}
```

Monitor 任务在断线后直接被标记为失败，虽然会触发工作台的自动重试，但如果断线只是短暂的网络抖动（< 1 分钟），可能不必要地增加失败计数。

**建议**：评估是否增加短暂的 grace period（如 2 分钟），在 grace period 内恢复连接则不标记失败。

### 3.2 能力检查与派单

| 检查项 | 状态 | 说明 |
|---|---|---|
| 能力检查 | 通过 | `capabilityCheck` 验证工位是否具备执行任务的条件 |
| 账号选择 | 通过 | `selectAvailableAccount` 考虑 Cookie 有效性、配额、用途匹配 |
| 前置检查 | 通过 | `beforeDispatch` 检查授权、账号、页面状态 |

**发现 #FUNC-7** 🟢 低风险 — 能力检查失败原因未充分暴露给用户

当 `capabilityCheck` 返回 `accepted: false` 时，原因码（如 `ACCOUNT_PURPOSE_MISMATCH`）只通过事件上报到工作台，用户侧只看到"不接单"。

Popup 虽然能显示 `idleClaimSnapshot` 的原因，但信息有限。

---

## 链路四：数据回写可靠性

**核心文件**：`src/db/index.js`、`src/db/noteStore.js`、`src/workbench/runtime/taskDeltaReporter.js`、`src/sync/flywheelSync.js`

### 4.1 本地存储

| 检查项 | 状态 | 说明 |
|---|---|---|
| 数据库版本管理 | 优秀 | Dexie 从 v1 升级到 v12，每次升级有明确的 schema 变更和迁移逻辑 |
| 幂等性 | 通过 | `noteStore.upsert` 使用 `db.notes.put`，主键去重 |
| 批量操作 | 通过 | `bulkUpsert` 使用 Dexie 事务 |
| 数据规范化 | 通过 | `normalizeNoteRecord` 统一字段格式 |

**发现 #FUNC-8** 🟡 中风险 — 数据库升级 v12 有数据丢失风险

v12 升级时删除重复的 `idempotencyKey`：
```javascript
if (duplicates.length > 0) {
  await tx.table('workbenchOutbox').bulkDelete(duplicates);
}
```

虽然重复记录理论上应该被删除，但如果在升级过程中发生中断（如浏览器崩溃），可能导致部分重复记录残留，破坏 `&idempotencyKey` 唯一索引约束。

**建议**：v12 升级增加更 robust 的重复处理逻辑，或在应用层处理重复而非依赖数据库约束。

### 4.2 增量上报（Outbox 模式）

| 检查项 | 状态 | 说明 |
|---|---|---|
| Outbox 表结构 | 优秀 | `workbenchOutbox` 支持 `status`、`nextAttemptAt`、`idempotencyKey` |
| 复合索引 | 通过 | `[status+nextAttemptAt+createdAt]` 避免全表扫描 |
| 离线重试 | 通过 | `taskDeltaReporter` 封装了重试逻辑 |
| 幂等键 | 通过 | v12 增加 `&idempotencyKey` 唯一索引 |

**发现 #FUNC-9** 🟢 低风险 — Outbox 清理策略未看到

未在代码中看到 outbox 的定期清理逻辑。如果插件长期运行，outbox 可能累积大量已完成的记录，影响查询性能。

**建议**：增加已完成的 outbox 记录定期清理（如保留 7 天）。

### 4.3 工作台同步

| 检查项 | 状态 | 说明 |
|---|---|---|
| 配置缓存 | 通过 | 30 秒 TTL 缓存，storage 变化时立即失效 |
| 数据 Token | 通过 | `ensureFlywheelDataSession` 管理数据会话 |
| 超时处理 | 通过 | `AbortSignal.timeout` 设置请求超时 |

---

## 链路五：用户可见流程

**核心文件**：`src/popup/App.jsx`

### 5.1 Popup 交互

| 检查项 | 状态 | 说明 |
|---|---|---|
| 页面能力检测 | 优秀 | 根据 URL + content script 上下文动态显示可用操作 |
| 进度展示 | 通过 | 支持进度条、阶段标签、评论深度模式显示 |
| 批量控制 | 通过 | 暂停/恢复/停止，带确认对话框 |
| 错误提示 | 通过 | `toFriendlyError` 将技术错误转换为用户友好文案 |
| 防重复点击 | 通过 | `withBusyAction` + `busyActions` 状态管理 |

**发现 #FUNC-10** 🟡 中风险 — Popup 状态与 Content Script 可能不同步

Popup 的进度状态依赖 `chrome.runtime.onMessage` 接收 PROGRESS/COLLECT_DONE 消息。如果消息丢失（如 popup 关闭期间发送），重新打开 popup 后状态可能不正确。

具体表现：
- 批量采集进行中时关闭 popup
- 重新打开 popup，进度条显示 0/0 而非实际进度
- 但批量控制按钮可能正确显示（因为 `batchControlsVisible` 初始为 false）

**建议**：popup 打开时主动查询 content script 当前任务状态，而非仅依赖消息推送。

### 5.2 Dashboard

| 检查项 | 状态 | 说明 |
|---|---|---|
| 数据展示 | 未深入 | 本次审查未深入阅读 Dashboard 代码 |
| 主题切换 | 通过 | 支持 default 和 ac-ui 两种主题 |

### 5.3 授权与配置流程

| 检查项 | 状态 | 说明 |
|---|---|---|
| 授权码激活 | 通过 | `AUTHORIZE_PLUGIN_ACCESS` 流程 |
| 工位绑定 | 通过 | `REGISTER_EXECUTION_STATION` 流程 |
| 多环境支持 | 通过 | 支持生产环境 + localhost |

---

## 审查总结

### 风险等级分布

| 等级 | 数量 | 编号 |
|---|---|---|
| 🟠 高风险 | 1 | FUNC-5 |
| 🟡 中风险 | 5 | FUNC-2, FUNC-4, FUNC-6, FUNC-8, FUNC-10 |
| 🟢 低风险 | 4 | FUNC-1, FUNC-3, FUNC-7, FUNC-9 |

### 各链路健康度

| 链路 | 评分 | 说明 |
|---|---|---|
| 页面识别与采集准确性 | B+ | 时间解析优秀，但 `__INITIAL_STATE__` 依赖无兜底 |
| 批量采集完整性 | A- | 虚拟列表处理优秀，失败笔记可能丢失部分数据 |
| 远程任务执行 | A | 租约、超时、断线恢复机制完善 |
| 数据回写可靠性 | B+ | Outbox 模式成熟，但清理策略和升级风险需关注 |
| 用户可见流程 | B+ | 交互完整，但 popup 状态同步有瑕疵 |

### 优先修复建议

**P0（建议修复）**：
1. **FUNC-5**：批量采集失败时保存表面数据，避免完全丢失

**P1（建议修复）**：
2. **FUNC-2**：评估恢复 DOM 抓取 fallback
3. **FUNC-4**：将滚动发现参数暴露为配置或自适应
4. **FUNC-6**：Monitor 任务断线恢复增加 grace period
5. **FUNC-8**：数据库 v12 升级增加 robust 重复处理
6. **FUNC-10**：Popup 打开时主动查询任务状态

**P2（建议改进）**：
7. **FUNC-1**：统一页面类型检测到单一来源
8. **FUNC-3**：视频流选择逻辑文档化
9. **FUNC-7**：能力检查失败原因更充分暴露给用户
10. **FUNC-9**：增加 outbox 定期清理

---

## 功能层面核心结论

**这个插件能不能稳定承担内容工作台的采集执行端？**

**能，但有 3 个卡点：**

1. **平台改版风险**：核心采集依赖 `__INITIAL_STATE__`，小红书页面结构变化会导致大面积失效。当前无 DOM fallback。
2. **批量采集失败丢失数据**：单篇采集失败时，已获取的表面数据（封面、标题、点赞）不保存，用户可能感知"采了但找不到"。
3. **Popup 状态同步**：用户操作中关闭/重新打开 popup 可能看到错误状态，影响操作信心。

其余链路（远程任务、数据回写、风控处理）设计成熟，具备生产环境运行能力。

---

*本报告为 Step 2 功能矩阵审查结论。Step 3（浏览器验收测试）将在确认后执行。*
