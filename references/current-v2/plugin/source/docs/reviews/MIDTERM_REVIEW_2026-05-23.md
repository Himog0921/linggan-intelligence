# 灵感爆爆爆 — 中期架构审查简报

> **文档用途**：供外部技术顾问审阅，讨论架构问题和改进方案
> **审查日期**：2026-05-23
> **项目阶段**：已发布 Chrome Web Store，当前版本 v2.0.19
> **注意**：本文档为自包含简报，无需阅读源代码即可参与讨论

---

## 1. 项目背景

### 1.1 它是什么

「灵感爆爆爆」是一个 Chrome 浏览器扩展插件，用于从小红书和抖音自动采集内容创作者的数据——包括笔记/视频、评论、博主信息、媒体文件等。它的定位不是独立产品，而是一个更大系统「内容工作台」（Web 端）的浏览器执行端。

**一句话概括**：内容工作台负责"判断、组织、沉淀"，插件负责"网页内采集、页面交互、结果回传"。

### 1.2 用户与场景

- **核心用户**：内容运营人员、自媒体从业者
- **典型场景**：
  - 打开小红书搜索页 → 搜索关键词 → 一键批量采集搜索结果（笔记+评论+博主信息）
  - 打开某博主主页 → 自动采集其全部作品、粉丝数、互动数据
  - 打开抖音视频页 → 采集视频数据 + 全部评论 + 评论图片
  - 采集结果自动推送到「内容工作台」进行后续分析和选题管理

### 1.3 技术栈

| 层 | 选择 | 说明 |
|---|---|---|
| 运行环境 | Chrome Extension Manifest V3 | 最新扩展标准 |
| 语言 | 纯 JavaScript（无 TypeScript） | ES2020+ |
| 构建 | Webpack 5 | 4 个独立入口 |
| 本地存储 | Dexie（IndexedDB 封装） | 6 张表，13 次版本迁移 |
| UI 框架 | React（Popup + Dashboard） | JSX 通过 Babel 编译 |
| 样式 | 纯 CSS | 无 CSS 框架 |
| 测试 | Node.js test runner | 95 个测试文件 |
| 代码规模 | ~37,250 行 JS + ~3,200 行 CSS | 90+ 源文件 |

---

## 2. 架构全景

### 2.1 整体结构

```
┌──────────────────────────────────────────────────────────────┐
│                    内容工作台（Web 端）                        │
│                  Next.js + PostgreSQL                         │
│           任务下发 / 数据沉淀 / AI 分析 / 团队协作              │
└──────────┬───────────────────────────────────┬───────────────┘
           │ HTTPS + Bearer Token              │ Web Push
           │ （任务轮询/认领/续租/回传）         │ （任务唤醒）
           ▼                                   │
┌──────────────────────────────────────────────┴───────────────┐
│              灵感爆爆爆插件（Chrome Extension）                │
│                                                              │
│  ┌─────────┐  ┌─────────────┐  ┌────────┐  ┌───────────┐    │
│  │ Popup   │  │ Background  │  │Content │  │ Dashboard │    │
│  │ 弹窗 UI │  │ Service     │  │Script  │  │ 数据看板  │    │
│  │ (React) │  │ Worker      │  │ 页内注入│  │ (React)   │    │
│  └────┬────┘  └──────┬──────┘  └───┬────┘  └─────┬─────┘    │
│       │              │              │              │          │
│       └──Chrome Msg──┘──Chrome Msg──┘──postMessage─┘          │
│                              │                               │
│              ┌───────────────┼───────────────┐                │
│              ▼               ▼               ▼                │
│         ┌────────┐    ┌──────────┐    ┌──────────┐            │
│         │ 小红书  │    │ IndexedDB│    │ 抖音     │            │
│         │ 采集器  │    │ 本地数据库│    │ 采集器   │            │
│         └────────┘    └──────────┘    └──────────┘            │
└──────────────────────────────────────────────────────────────┘
```

### 2.2 Chrome Extension 的 5 个运行上下文

Chrome 扩展有一个特殊的架构特点：**代码运行在 5 个完全隔离的上下文中**，它们之间只能通过消息通信。

| 上下文 | 比喻 | 职责 |
|--------|------|------|
| **Background Service Worker** | 后台管家 | 无界面，常驻后台。管理下载、消息路由、定时任务轮询、与工作台通信。MV3 中它会在空闲约 30 秒后被 Chrome 杀掉，需要靠闹钟唤醒。 |
| **Content Script** | 植入探针 | 注入到用户正在浏览的小红书/抖音页面中。可以读取页面 DOM，但不能直接访问页面的 JavaScript 变量。 |
| **Injected Script** | 深层间谍 | 注入到页面的"主世界"（main world），可以访问页面原生的 JavaScript 全局变量（如小红书的 `__INITIAL_STATE__`），但无法直接调用 Chrome API。 |
| **Popup** | 弹窗面板 | 点击扩展图标弹出的小窗口。显示平台检测、采集按钮、进度、统计、设置等。 |
| **Dashboard** | 数据面板 | 在页面中通过 iframe 打开的全屏数据管理界面。浏览/搜索/导出已采集数据。 |

**上下文间通信方式**：
- Popup ↔ Background ↔ Content Script：`chrome.runtime.sendMessage` / `chrome.tabs.sendMessage`
- Content Script ↔ Injected Script：`window.postMessage`
- Dashboard ↔ Content Script：`window.postMessage`（iframe 嵌套关系）

### 2.3 数据流：一次完整的批量采集

以"在小红书搜索页批量采集 50 篇笔记"为例：

```
用户点击 Popup 的「批量采集」按钮
    │
    ▼ chrome.runtime.sendMessage(START_BATCH_NOTES)
Background Service Worker 收到消息
    │
    ▼ chrome.tabs.sendMessage(转发到当前 tab 的 Content Script)
Content Script 收到消息，创建 BatchNoteController
    │
    ├── 1. 发现阶段：滚动页面，收集笔记卡片列表（DOM 选择器）
    │
    ├── 2. 排序阶段：按点赞数筛选 Top N
    │
    └── 3. 逐篇采集循环（50 篇）：
         │
         ├── 点击笔记卡片 → 页面弹出详情弹窗
         ├── 注入 Script 读取 __INITIAL_STATE__（页面全局数据）
         ├── Content Script 通过 postMessage 获取数据
         ├── 解析并标准化数据
         ├── 写入 IndexedDB（Dexie）
         ├── 按 Esc 关闭弹窗
         ├── 滚动到下一篇笔记
         │
         └── 循环完成 → 汇总结果 → 推送到工作台（如果已连接）
```

### 2.4 数据流：工作台远程任务

工作台可以远程下发采集任务到插件，流程更复杂：

```
工作台创建采集任务
    │
    ▼ 插件通过 30s 轮询（或 Web Push 唤醒）发现新任务
Background Service Worker 认领任务（claim + lease）
    │
    ▼ 能力检查：当前打开的 tab 是否能执行此任务？
    │
    ├── 能力不匹配：跳过，等待下次轮询
    │
    └── 能力匹配：
        │
        ▼ 派发到对应 tab 的 Content Script
Content Script 执行采集（与本地批量采集相同的执行器）
    │
    ├── 实时上报进度事件 → Background → 工作台
    ├── 采集完成 → 结果打包 → 增量上传到工作台
    │
    └── 失败 → 错误映射（8 类错误） → 重试/终止
```

### 2.5 本地数据模型

IndexedDB 中有 6 张表：

| 表 | 主键 | 存什么 | 当前索引数 |
|---|---|---|---|
| **notes** | noteId | 笔记/视频：标题、正文、互动数据、媒体信息 | 23 |
| **comments** | 自增 ID | 评论：文本、层级关系、点赞、图片 | 20 |
| **authors** | userId | 博主：粉丝数、简介、标签、IP 属地 | 19 |
| **collectionRuns** | collectionRunId | 任务执行记录：状态、进度、时间线、工作台绑定 | 13 |
| **mediaAssets** | assetId | 媒体文件：下载状态、质量、角色 | 8 |
| **workbenchOutbox** | 自增 ID | 离线上传队列：幂等去重、指数退避重试 | 9 |
| **accounts** | accountId | 采集账号管理：Cookie、配额、状态 | 6 |

### 2.6 双平台架构差异

小红书和抖音采用了不同的数据采集策略：

| 方面 | 小红书 | 抖音 |
|------|--------|------|
| 数据来源 | `__INITIAL_STATE__`（页面全局变量）为主，DOM 回退 | API 拦截 + render data + DOM 三源融合 |
| 批量控制 | 面向对象（`BatchNoteController` 类继承） | 函数式（独立导出函数 + 外部传入回调） |
| 页面导航 | 点击卡片 → 弹窗 → Esc 关闭 | URL 路由切换（SPA） |
| 评论采集 | DOM 滚动采集（已改为 API 优先 + DOM 回退） | API 调用 + 分页 |
| 下载策略 | chrome.downloads API | 页面上下文 fetch（绕过 CDN Referer 鉴权） |

---

## 3. 发现的问题

### 3.1 问题总览

| 严重度 | 数量 | 说明 |
|--------|------|------|
| 🔴 致命 | 7 | 可能导致运行时崩溃、数据丢失或功能失效 |
| 🟠 高风险 | 19 | 特定条件下影响稳定性或长期可维护性 |
| 🟡 中等 | 18 | 技术债积累，不影响当前功能但增加维护成本 |
| 🔵 低风险 | 5 | 风格问题或已确认的良好实践 |

### 3.2 🔴 致命问题详解

#### P1. Service Worker 全局状态无持久化恢复

**背景**：Chrome MV3 的 Service Worker 在空闲约 30 秒后会被终止。当有新的 chrome 事件（如闹钟、消息）时，Chrome 会重新启动它。但重新启动后，所有内存中的状态都会丢失。

**现状**：插件在 Background Service Worker 中维护了以下关键状态：

- `workbenchTaskRegistry`：当前工作台任务的上下文映射（哪个任务分配给了哪个 tab）
- `navigatedTabs`：为了执行任务而导航创建的浏览器 tab
- 各种轮询计时器和计数器

这些状态全部存在 JavaScript 变量（`Map` 对象）中。`navigatedTabs` 有部分缓解（会写入 `chrome.storage.session`），但 `workbenchTaskRegistry` 完全没有持久化。

**影响**：
- Service Worker 被杀后恢复，无法追踪正在执行的任务属于哪个 tab
- 无法停止正在执行的任务（因为找不到目标 tab）
- 为了执行任务而创建的辅助浏览器窗口永远不会被关闭
- 轮询策略重置（空轮询计数器归零，退避间隔失效）

**这个问题多久发生一次？**：取决于设备配置和浏览器内存压力。在低配置设备或浏览器标签页较多时，Service Worker 更频繁被杀。工作台远程任务场景下（依赖后台轮询），影响更大。

---

#### P2. 5 个工具函数在 4-19 个文件中重复定义

**现状**：以下函数在项目中各自独立实现了多份：

| 函数 | 逻辑 | 重复次数 |
|------|------|----------|
| `normalizeText` / `normalizeString` | `String(value\|\|'').trim()` | **19 个文件** |
| `normalizeObject` | 清理空值的对象标准化 | **10 个文件** |
| `firstText` | 从数组中取第一个非空文本 | **8 个文件** |
| `pickMediaUrlFromArray` | 从候选列表中选最优媒体 URL | **4 个文件** |
| `isRecoverableConnectionError` | 判断网络错误是否可重试 | **2 个文件** |

**为什么这是致命的**：如果其中一份实现的 bug 被修复了（比如处理了一个新的边缘情况），其他 18 份不会同步修复。这实际上是一个定时炸弹——随着代码演进，这些副本会逐渐分化，产生难以察觉的行为不一致。

---

#### P3. 两平台间 6 个函数逐字复制

**现状**：小红书和抖音两个平台模块中，以下函数完全相同（或 85% 相同）但各自独立维护：

1. `normalizeTargetIdentity` — 2 份，完全相同
2. `createTargetMismatchError` — 2 份，完全相同
3. `getXxxTargetProfileUrl` — 2 份，完全相同
4. `checkXxxAuthorMonitorTarget` — 2 份，85% 相同（65 行重复）
5. `extractXxxProfileUserId` — 2 份，结构完全一致，仅正则不同

更危险的是：同一个函数 `extractXhsProfileUserId` 在 `batchController.js` 和 `background/index.js` 中**有两个不同的实现**——前者匹配 3 种 URL 模式，后者只匹配 1 种。如果 Background 和 Content Script 对同一个 URL 解析出不同的作者 ID，会导致"任务被正确分发但被目标检查拒绝"。

---

#### P4. 异步任务执行失败时错误被静默吞掉

**现状**：当工作台远程下发采集任务时，Content Script 采用了"先接受、后异步执行"的模式：

```
收到远程采集请求
  → 立即返回 { success: true, pending: true }
  → 异步执行实际采集
  → 如果失败，只 console.error，不通知调用方
```

这意味着：
- 调用方（Background）收到 `success: true` 后认为任务已接受
- 后续异步执行如果失败，Background 永远不知道
- 工作台侧看到任务进入"执行中"后再也不会收到任何更新
- 用户看到 Popup 停留在"进行中"状态，永远不结束

**影响范围**：4 种远程任务类型（单篇笔记、单条评论、博主、评论图片下载）都有这个问题。

---

#### P5. 两个核心 UI 组件严重过大

| 组件 | 行数 | useState 数 | useCallback 数 | 职责数 |
|------|------|-------------|----------------|--------|
| Popup App.jsx | 1,279 行 | 45 个 | 25+ 个 | 8+ |
| Dashboard App.jsx | 1,144 行 | — | — | 6+ |

Popup 的 App.jsx 单组件承担了：平台检测、认证管理、Cookie 管理、批量任务控制、飞轮同步、主题切换、通知系统、确认弹窗等 8 个以上独立职责。45 个 state 声明占 50+ 行，25 个回调函数占 500+ 行。

**为什么这是致命的**：修改任何一个小功能都需要理解整个 1,279 行组件的上下文。状态之间的交叉影响不可预测（45 个 state 的排列组合），任何修改都可能引入回归。

---

#### P6. 数据模型文档严重过时

**现状**：文档记录的数据库版本是 v8，实际代码已经是 v13。5 个版本的迁移完全没有文档，包括：

- v9-v10：新增 `workbenchOutbox` 表（离线上传队列）及其复合索引
- v11：新增 `accounts` 表（采集账号管理）
- v12：幂等键唯一索引 + 去重升级
- v13：notes/authors 新增 `collectionRunId` 索引

**影响**：新开发者无法理解当前数据库结构的完整演变，也无法从文档推断出哪些字段是新增的、它们的含义是什么。

---

#### P7. 架构文档有 26 个源码文件未记录

ARCHITECTURE.md 缺少以下模块的描述：
- 工作台运行时 8 个新模块（导航编排、Cookie 管理、安装引导、任务清理等）
- 抖音平台 5 个新模块（安全挑战、搜索捕获、任务栏渲染状态等）
- 小红书选择器健康检查模块
- 主题系统（整个 `src/themes/` 目录）
- Content Script 路由器等 4 个新模块

此外，ARCHITECTURE.md 中小红书评论采集路径的描述仍然是"DOM 滚动采集"，但代码已经改为"API 优先 + DOM 回退"。

---

### 3.3 🟠 高风险问题摘要

| # | 问题 | 影响描述 |
|---|------|----------|
| H1 | background/index.js 承载 8 个独立职责域（2,294 行） | 定位和修改困难 |
| H2 | bgHandlers 巨石对象 831 行 | 单个消息处理映射对象比很多完整文件都大 |
| H3 | taskPoller 工厂函数体 885 行 | 单函数内含 15 个闭包，认知复杂度极高 |
| H4 | pollActiveTask 函数 277 行、15 个分支、6 层嵌套 | 几乎不可能安全修改 |
| H5 | messageHandlers 工厂函数 42 个参数 | 调用方极易遗漏参数 |
| H6 | managedTaskController.start() fire-and-forget | 任务启动后异常被静默吞掉 |
| H7 | `document.body.innerText` 在 250ms 循环中 5 次同步读取 | 批量采集时造成页面布局抖动 |
| H8 | 抖音 heartbeat 消息风暴 | 每条视频 3+ 次跨上下文消息 + IndexedDB 写入 |
| H9 | 两套批量控制器模式不一致 | 小红书用类继承，抖音用函数式回调，暂停/停止竞态行为不同 |
| H10 | 多 tab 无互斥 | 用户可在两个 tab 同时启动同一作者的采集，产生重复数据 + 触发平台风控 |
| H11 | XHS 内部两个 Batch 类共享 300+ 行导航方法 | 维护需同步两处 |
| H12 | content 组件 100+ 处内联 CSS 硬编码 | 主题切换无法覆盖页内注入的 UI |
| H13 | Cookie 处理 catch 块完全为空 | Cookie 静默失败时无法诊断 |
| H14 | 22 处 `.catch(() => {})` 静默吞错 | 出问题时调试困难 |
| H15 | SELECTORS.md 抖音选择器大量缺失 | 抖音博主/交互选择器无文档，改版时无法追溯 |
| H16 | 6 种消息类型未记录在协议文档 | 飞轮配置/账号管理/增量上报消息无协议文档 |
| H17 | loadIdleClaimSnapshot 在 popup/dashboard 逐字重复 | 同一函数维护两份 |
| H18 | Douyin batchController 两个主函数各 300+ 行 | 三层嵌套 try/catch，10 层缩进 |
| H19 | Douyin MutationObserver 和事件监听器无清理 | 页面 SPA 导航后监听器累积 |

---

### 3.4 已确认的良好实践

以下方面经过审查确认没有问题：

| 方面 | 确认结果 |
|------|----------|
| **平台隔离** | 小红书和抖音代码之间零直接引用，完全通过 shared/ 解耦 |
| **DB 层封装** | 所有 IndexedDB 操作通过统一的 db/ 模块，外部无绕过 |
| **Workbench 层边界** | workbench/runtime/ 中没有任何 DOM 操作，纯协议/调度/状态管理 |
| **批量异常隔离** | 单条笔记/视频采集失败不影响整个批次 |
| **评论批量写入** | 已正确使用 `bulkUpsert`，非逐条写入 |
| **离线韧性** | 增量发件箱支持幂等去重 + 指数退避重试 |
| **安全挑战处理** | 抖音有独立的安全挑战检测和恢复模块 |
| **任务租约机制** | 工作台任务有认领/续租/释放的完整生命周期管理 |

---

## 4. 根因分析

以上问题不是偶然出现的，它们有共同的根因：

### 栅根因 1：快速迭代 + 双平台并行，代码去重被持续推迟

小红书和抖音两个平台的采集逻辑最初是独立开发的（DOM 结构、API 设计、页面行为完全不同），导致大量相似但各自实现的代码。项目经历了 13 个数据库版本迁移，但每次都是"先让功能跑起来，文档以后再补"——而"以后"一直没有来。

### 根因 2：Chrome MV3 的架构约束没有被充分纳入设计

MV3 的 Service Worker 生命周期管理（会被杀掉 + 重启）是一个基本约束，但代码中大量关键状态仍然依赖内存。这不是一个 bug，而是一个架构层面的欠账——项目从 MV2 或更早期的代码演进到 MV3，但状态管理策略没有跟着更新。

### 根因 3：UI 层缺乏组件拆分意识

Popup 和 Dashboard 都是在一个 .jsx 文件中堆叠了所有功能。这与 React 的组件化理念相悖，但在"赶功能"的开发节奏下很常见。结果是每个文件都是一个"上帝组件"，承担了过多职责。

### 根因 4：两套控制器模式的历史包袱

小红书的批量控制器使用了面向对象的 `BaseBatchController` 类继承，而抖音的批量控制器使用了函数式 API。这是因为两个平台在不同时期开发，采用了不同的设计模式。现在两套并存，使得跨平台共享代码更加困难。

---

## 5. 解决方案建议

### 方案概览

```
Phase 0（止血）→ Phase 1（去重）→ Phase 2（拆分）→ Phase 3（统一）
     1-2 周           2-3 周          3-4 周          2-3 周
```

### Phase 0: 止血（1-2 周，最高优先级）

> 目标：消除可能导致运行时故障和数据丢失的问题

#### 0.1 Service Worker 状态持久化

**方案**：将 `workbenchTaskRegistry` 的关键映射持久化到 `chrome.storage.session`（MV3 专为 SW 状态设计的存储，生命周期与浏览器会话一致）。

```
写入时机：每次 workbenchTaskRegistry.set() 时，同步写入 session storage
恢复时机：SW 启动时（chrome.runtime.onStartup / onInstalled），从 session storage 重建 Map
清理时机：任务终态时同步清理 session storage
```

项目中已有先例：`navigatedTabs` 已经通过 `chrome.storage.session` 持久化。`workbenchTaskRegistry` 可以复用同一模式。

**评估**：改动集中在 background/index.js，约 50-80 行新增代码。不影响现有功能。

#### 0.2 asyncDispatch 错误上报

**方案**：为所有异步执行路径增加统一的失败上报机制。

```
现状：
  Promise.resolve().then(run).catch(console.error);  // 错误被吞掉
  return { success: true, pending: true };

改为：
  Promise.resolve().then(run)
    .catch(error => {
      taskDeltaReporter.enqueueEvent({
        type: 'execution_failed',
        error: errorMapper.map(error)
      });
      reportProgress({ error: error.message });  // 通知 popup
    });
  return { success: true, pending: true };
```

**评估**：4 个 handler 各增加约 5 行。通过 `taskDeltaReporter` 上报，与现有的增量上传链路复用。

#### 0.3 工具函数去重

**方案**：创建 `src/shared/normalize.js`，将 5 个重复函数统一到此文件，然后全局替换引用。

```javascript
// src/shared/normalize.js
export function normalizeText(value = '') { return String(value || '').trim(); }
export function normalizeObject(obj) { /* 统一实现 */ }
export function firstText(arr) { /* 统一实现 */ }
export function pickMediaUrlFromArray(candidates) { /* 统一实现 */ }
export function isRecoverableConnectionError(err) { /* 统一实现 */ }
```

**评估**：纯重构，不改变任何运行时行为。19 个文件需要修改 import 语句，但每个文件改动极小（删掉本地定义 + 加一行 import）。

---

### Phase 1: 去重（2-3 周）

> 目标：消除跨平台的重复代码，降低维护成本

#### 1.1 跨平台共享函数提取

**方案**：将 6 个跨平台重复函数提取到 `src/shared/batchUtils.js`，参数化平台差异部分。

```javascript
// src/shared/batchUtils.js
export function normalizeTargetIdentity(value) { /* 统一 */ }
export function createTargetMismatchError(label, expected, actual) { /* 统一 */ }
export function extractProfileUserId(url, patterns) { /* 参数化正则 */ }
export function checkAuthorMonitorTarget({ url, monitorMeta, patterns, modeCheck }) {
  // 通用逻辑，平台差异通过参数注入
}
```

**评估**：减少约 200 行重复代码。`checkAuthorMonitorTarget` 是最复杂的，需要将平台特定的正则和模式检查抽象为参数。

#### 1.2 同平台内 BatchController 导航方法上提

**方案**：将 XHS 的 `BatchNoteController` 和 `BatchCommentController` 共享的导航方法（`_scrollToAndFindNote`、`_waitForNoteLoad`、`_goBackToList`、`_closeNotePopup`）上提到 `BaseBatchController`。

**评估**：减少约 300 行重复代码。需要将相关方法从两个子类中移到基类，并确保它们不依赖子类特有的状态。

#### 1.3 extractXhsProfileUserId 统一

**方案**：将 `background/index.js` 中的 URL 解析函数统一为 `batchController.js` 中更完整的版本（3 种模式），通过 `shared/` 导出。

**评估**：约 10 行改动，但消除了一个潜在的任务路由不一致风险。

---

### Phase 2: 拆分（3-4 周）

> 目标：将巨型文件拆分为可维护的小模块

#### 2.1 background/index.js 拆分

**现状**：2,294 行，8 个职责域。

**方案**：

```
src/background/
  index.js              (约 200 行，入口：注册监听器 + 导入 handlers)
  urlUtils.js           (约 80 行，URL 解析/检测函数)
  taskRegistry.js       (约 100 行，workbenchTaskRegistry 持久化 CRUD)
  storageHelpers.js     (约 60 行，localStorage 读写)
  handlers/
    cookies.js          (约 100 行，GET_PLATFORM_COOKIES)
    download.js         (约 150 行，DOWNLOAD_MEDIA_FILE / FETCH_BINARY)
    workbench.js        (约 200 行，工作台相关 handler)
    batch.js            (约 150 行，批量采集 handler)
    data.js             (约 100 行，数据查询/导出 handler)
    accounts.js         (约 80 行，账号管理 handler)
    sync.js             (约 100 行，飞轮同步 handler)
```

**关键约束**：background 的打包不支持代码拆分（webpack 配置中 `splitChunks` 排除了 background），所以拆分只是源码组织层面的，不影响构建产物。

#### 2.2 popup/App.jsx 拆分

**现状**：1,279 行，45 个 useState。

**方案**：按职责提取自定义 Hook + 子组件。

```
src/popup/
  App.jsx               (约 200 行，布局 + 路由)
  hooks/
    usePlatformDetect.js (约 40 行)
    useBatchTask.js      (约 100 行)
    useAuth.js           (约 80 行)
    useFlywheelConfig.js (约 60 行)
    useAccounts.js       (约 60 行)
    useTheme.js          (约 30 行)
  sections/
    StatsSection.jsx     (约 60 行)
    ProgressSection.jsx  (约 80 行)
    ActionSection.jsx    (约 100 行)
    ToolSection.jsx      (约 60 行)
```

**评估**：这是改动量最大的拆分，但也是最必要的。每个 hook 管理自己的 state 子集，App.jsx 只做组合。

#### 2.3 taskPoller.js 拆分

**现状**：1,556 行，单函数体 885 行。

**方案**：将 `pollActiveTask`（277 行，15 个分支）按任务状态拆分为独立函数。

```javascript
// 当前：一个巨大的 switch/if-else 链
async pollActiveTask() { if status === 'running' ... else if === 'dispatched' ... }

// 目标：每个状态一个函数
async pollActiveTask() {
  const handlers = { running: pollRunning, dispatched: pollDispatched, ... };
  return handlers[activeTask.workbenchStatus]?.();
}
```

---

### Phase 3: 统一（2-3 周）

> 目标：统一两套控制器模式，建立一致的架构范式

#### 3.1 统一批量控制器模式

**方案选择**（需要讨论）：

**方案 A：让抖音也使用类继承**

```
优点：与小红书一致，共享基类的暂停/恢复/停止逻辑
缺点：需要重写抖音批量控制器的函数式 API，改动量大
```

**方案 B：统一为函数式 API**

```
优点：更灵活，抖音已验证可行
缺点：小红书需要重写，且类继承在当前场景下语义更清晰
```

**方案 C：保持两套，但统一接口层**

```
优点：改动量最小
缺点：仍然维护两套模式
```

**推荐**：方案 A，但分步执行——先将抖音的函数式 API 包装为类接口，再逐步迁移内部实现。

#### 3.2 文档同步

- DATA_MODEL.md 更新到 v13，补全 5 个版本的迁移说明
- ARCHITECTURE.md 补全 26 个缺失模块
- SELECTORS.md 补全抖音博主/交互选择器
- MESSAGE_PROTOCOL.md 补全 6 种缺失消息类型

---

## 6. 需要讨论的决策点

以下问题没有唯一正确答案，需要根据项目优先级和资源来决定：

### Q1. 是否引入 TypeScript？

**现状**：纯 JavaScript，类型安全完全靠运行时检查。

**支持引入**：
- 42 参数的工厂函数、19 个文件的重复函数定义，本质上都是缺乏编译期类型检查的结果
- 拆分重构时 TypeScript 能提供重构安全保障

**反对引入**：
- 37,000 行代码的渐进式迁移本身就是一个大工程
- Chrome Extension + MV3 + Dexie 的类型定义需要额外维护
- 当前项目只有一个非技术背景的产品负责人，TypeScript 不会直接提升他的体验

**替代方案**：JSDoc 类型注解，零迁移成本但能获得部分类型检查能力。

### Q2. 拆分重构的节奏怎么控制？

**选项 A：大重构**（集中 2-3 周一次性完成 Phase 0-2）
- 优点：一次到位
- 缺点：期间无法响应新需求和线上问题

**选项 B：渐进式**（每次只做一个小重构，穿插在日常需求中）
- 优点：风险可控，不影响日常迭代
- 缺点：整体周期长（可能 2-3 个月），期间新旧代码混存

**选项 C：仅做 Phase 0 止血，暂停后续重构**
- 优点：最小改动量
- 缺点：技术债持续累积

### Q3. 两套控制器模式的统一策略

如上 Phase 3 所述，是统一为类继承、函数式 API、还是保持两套只统一接口层？

### Q4. 性能优化的优先级

`document.body.innerText` 热路径（P-H7）和抖音 heartbeat 消息风暴（P-H8）是否需要在本次迭代中修复？还是等用户反馈实际卡顿再处理？

### Q5. content 组件的 CSS 管理策略

100+ 处内联 CSS 硬编码是一个长期维护问题。是：
- A. 迁移到现有的 AEDS 主题系统（但需要先确认 AEDS 能覆盖 content script 场景）
- B. 抽取为独立的 CSS 文件 + CSS 变量
- C. 保持现状，用全局搜索替换处理颜色变更

---

## 7. 附录：模块依赖关系

```
popup/              → shared/, background(消息)
dashboard/          → shared/, content(消息)
background/         → shared/, workbench/*, db/*, sync/
content/            → shared/, platforms/*, db/*, workbench/runtime/*
platforms/xhs/      → shared/, db/*, injected/noteMap.js + user.js
platforms/douyin/   → shared/, db/*, injected/douyinApiCapture.js
workbench/          → shared/, db/*, sync/
sync/               → shared/, db/*
db/                 → (无外部依赖，只依赖 Dexie)
shared/             → (无项目内依赖，纯工具层)
injected/           → (独立运行在页面主世界，不 import 项目代码)
```

**包大小现状**：

| 构建产物 | 大小 | 说明 |
|----------|------|------|
| content.js | ~581 KiB | 最大，包含所有平台逻辑和数据运行时 |
| background.js | ~100 KiB | 不含 React，不支持代码拆分 |
| popup.js | ~150 KiB | 含 React |
| vendor.js | ~130 KiB | React + ReactDOM（content 和 popup 共享） |
| dashboard.js | ~120 KiB | 含 React |

---

*本文档由代码审查自动生成，所有发现均基于代码实际状态。如需进一步了解某个问题的细节，可以安排技术评审会议深入讨论。*
