# 灵感爆爆爆接手稳定化推进计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让项目从“能力已成型但存在回退和漂移”推进到“构建健康、事实源一致、可继续做实机验收”的状态。

**Architecture:** 先修真实阻塞项，再修事实源，再恢复验收节奏。第一阶段不扩新功能，重点处理构建 warning、懒加载回退、文档漂移和接手时最容易误判的状态问题。

**Tech Stack:** Chrome MV3、Webpack 5、Dexie 4、原生 JS/CSS

---

### Task 1: 修复构建阻塞级 warning 与飞轮同步错配

**Files:**
- Modify: `src/background/index.js`
- Modify: `src/sync/flywheelSync.js`
- Modify: `src/popup/popup.js`
- Test: `package.json`

- [ ] **Step 1: 记录当前失败基线**

Run: `npm run build`
Expected: PASS with warnings, including `syncNoteToFlywheel` / `syncAllToFlywheel` / `testConnection` / `getFlywheelConfig` / `saveFlywheelConfig` missing exports

- [ ] **Step 2: 统一飞轮同步接口命名**

在 `src/sync/flywheelSync.js` 中补齐 Background 真实使用的导出，至少统一到以下接口：

```js
export async function syncNoteToFlywheel(noteId) {}
export async function syncAllToFlywheel() {}
export async function testConnection(serverUrl) {}
export async function getFlywheelConfig() {}
export async function saveFlywheelConfig(config) {}
```

要求：
- 不破坏现有 `syncToFlywheel / checkFlywheelConnection / getSyncHistory` 导出
- `testConnection` 与 Popup 当前直连 `/api/collect/status` 的语义保持一致
- `getFlywheelConfig / saveFlywheelConfig` 至少具备本地持久化能力

- [ ] **Step 3: 收口 Popup 与 Background 的配置读写路径**

若 `src/popup/popup.js` 仍存在绕过 Background 直接测连的逻辑，保留可工作的最短路径，但确保：

```js
const flywheelConfig = await sendToBackground(MSG.GET_FLYWHEEL_CONFIG);
await sendToBackground(MSG.SAVE_FLYWHEEL_CONFIG, { config: { serverUrl, enabled: true } });
```

与 Background / Sync 模块字段一致，不再出现“Popup 以为可用、Background 实际无实现”的状态。

- [ ] **Step 4: 重新构建验证**

Run: `npm run build`
Expected: PASS and the five missing-export warnings disappear

- [ ] **Step 5: 记录进度**

Update:
- `progress.txt`

说明本轮修复了什么、还剩什么 warning。

### Task 2: 恢复 Wave 3 的懒加载收益，找出首包回弹根因

**Files:**
- Modify: `src/content/index.js`
- Modify: `src/content/douyinRuntime.js`
- Modify: `src/content/contentDataRuntime.js`
- Modify: `src/content/commentImageTask.js`
- Modify: `src/platforms/douyin/commentCollector.js`
- Test: `webpack.config.js`

- [ ] **Step 1: 固化当前包体基线**

Run: `npm run build`
Expected: `content.js` about `413 KiB`, with performance warning

- [ ] **Step 2: 修掉显式回退的静态导入**

重点检查并恢复以下边界：

```js
// 不应在 content 首包中静态拉入整个抖音运行时
import * as douyinRuntime from './douyinRuntimeModule.js';

// 应优先通过懒加载入口消费
const douyinRuntime = await loadDouyinRuntime();
```

目标：
- `src/content/index.js` 不再静态打入抖音运行时主模块
- 数据运行时只在需要时初始化

- [ ] **Step 3: 处理 ZIP 依赖重新静态进入首包的问题**

检查：

```js
import JSZip from 'jszip';
```

在评论图片区链路中的使用方式，尽量改回按需加载，避免 `content.js` 首包重新吞入 ZIP 依赖。

- [ ] **Step 4: 重新构建并记录结果**

Run: `npm run build`
Expected: `content.js` 明显下降，至少确认是否低于 `300 KiB`，并写清是否已消除 warning

- [ ] **Step 5: 更新事实源**

Update:
- `docs/decisions/index.md`
- `progress.txt`

若确认存在“Wave 3 后续改动导致懒加载失效”的事实，必须补记决策或回退说明。

### Task 3: 刷新当前事实源文档，避免接手期误导

**Files:**
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/technical/TECH_STACK.md`
- Modify: `BACKEND_STRUCTURE.md`
- Modify: `docs/plans/tech-debt.md`
- Modify: `progress.txt`

- [ ] **Step 1: 按当前代码更新架构图**

在 `docs/ARCHITECTURE.md` 中修正这些事实：
- XHS 与 Douyin 已以 `src/platforms/*` 为主
- `content/index.js` 主要承担 bootstrap / 平台分发 / 消息接线
- 当前仍存在 `contentRouter` 抽象与手写平台分流并存

- [ ] **Step 2: 同步技术栈与权限现实**

在 `docs/technical/TECH_STACK.md` 中以当前 `package-lock.json` 与 `manifest.json` 为准，更新：
- schema/版本说明
- host permissions
- 真实入口

- [ ] **Step 3: 更新债务看板**

在 `docs/plans/tech-debt.md` 中移除已完成项的旧数字，把新债务写清：
- flywheel 同步支线易坏
- 懒加载回退导致首包反弹
- Popup / Dashboard / DouyinAdapter 过胖

- [ ] **Step 4: 记录当前已知未验收边界**

在 `progress.txt` 中明确写出：
- 单条评论体验
- 评论图片区下载
- 数据面板二次下载长时效

仍属于“代码已实现，实机验收未完全闭环”。

### Task 4: 重新建立 Phase 6 验收入口

**Files:**
- Modify: `docs/plans/active/phase6-acceptance.md`
- Modify: `docs/product/TEST_CHECKLIST.md`
- Test: `scripts/probe-douyin-collect.js`

- [ ] **Step 1: 刷新活跃验收清单**

把 `phase6-acceptance.md` 从旧阶段遗留清单，改成当前真实待验收项：
- 搜索页
- 博主页
- 详情/弹层页
- 暂停/继续/停止
- 评论图片区
- 二次下载

- [ ] **Step 2: 对齐产品级验收清单**

在 `docs/product/TEST_CHECKLIST.md` 中明确区分：
- 已稳定主链路
- 需回归主链路
- 长尾高风险链路

- [ ] **Step 3: 准备实机验证脚本入口**

确认以下脚本仍可作为采证入口：

```bash
npm run probe:douyin-collect
npm run probe:douyin-root-cause
```

Expected: 至少能输出可复制到 Console 的探针脚本内容

### Task 5: 开始第二阶段产品化收口

**Files:**
- Modify: `src/popup/popup.js`
- Modify: `src/platforms/douyin/index.js`
- Modify: `src/platforms/douyin/uiInjector.js`
- Modify: `docs/plans/active/2026-03-ui-ux-upgrade-plan.md`

- [ ] **Step 1: 统一页面能力提示**

让用户在搜索页 / 博主页 / 详情页明确知道“这里现在能做什么、不能做什么”。

- [ ] **Step 2: 统一评论任务语义**

把单条评论、批量评论、评论图片区三条链路的阶段语义收成：
- 准备中
- 扫描中
- 下载中
- 打包中
- 已暂停
- 已停止
- 已完成

- [ ] **Step 3: 更新 UI/UX 活跃计划**

同步本轮已完成项与后续剩余项，避免继续引用 2026-03-27 的半旧状态。
