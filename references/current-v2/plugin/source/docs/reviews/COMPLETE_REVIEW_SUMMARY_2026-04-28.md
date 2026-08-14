# 灵感爆爆爆 — 完整代码审查总结报告

**审查日期**: 2026-04-28
**审查范围**: 全项目源码（src/、tests/、manifest.json、构建配置、文档体系）
**审查方法**: 四阶段分层审查（Fatal P0 → High P1 → Medium P2 → Low P3）
**审查产出**: 4 份阶段报告 + 1 份汇总

---

## 一、审查统计总览

| 阶段 | 级别 | 数量 | 主要类别 |
|------|------|------|----------|
| Phase 1 | P0（致命）| **11 项** | MV3 合规性 4 项，数据丢失 5 项，XSS 安全 2 项 |
| Phase 2 | P1（高危）| **15 项** | 消息传递 5 项，第三方依赖 2 项，React 稳定性 4 项，批量任务 4 项 |
| Phase 3 | P2（中危）| **21 项** | 代码质量 6 项，数据模型 6 项，错误处理 5 项，CSS/样式 4 项 |
| Phase 4 | P3（低危）| **16 项** | 构建工程 5 项，文档一致性 4 项，可测试性 3 项，代码整洁度 4 项 |
| **合计** | — | **63 项** | — |

---

## 二、P0 / P1 核心风险速览（需立即关注）

### 2.1 安全与合规（P0-1 ~ P0-4, P1-2 ~ P1-3）

| 编号 | 问题 | 文件位置 | 风险 |
|------|------|----------|------|
| P0-1 | `web_accessible_resources` 通配符暴露全部 JS/CSS | `manifest.json:76-77` | 反检测策略完全泄露，可被平台方探测 |
| P0-2 | `debugger` 权限存在 CWS 审核被拒风险 | `manifest.json:13` | 扩展可能被拒上架或后续更新被卡 |
| P0-10 | `innerHTML` 模式属结构性 XSS 风险 | `popup/popup.js` 等 4 处 | 未来维护者遗漏 `escapeHtml` 即引入漏洞 |
| P1-2 | `getDocumentCookie` 缺少权限校验 | `content/messageHandlers.js:789` | XSS 场景下恶意脚本可窃取完整 cookie |
| P1-3 | background 消息路由无 sender 验证 | `background/index.js:675` | 被控 content script 可触发敏感操作 |

### 2.2 数据丢失与一致性（P0-5 ~ P0-9, P1-12 ~ P1-15）

| 编号 | 问题 | 文件位置 | 风险 |
|------|------|----------|------|
| P0-5 | `syncToFlywheel` 部分失败仍返回 success | `sync/flywheelSync.js:205` | 用户看到"同步成功"但部分数据已丢失 |
| P0-6 | `cachedConfig` 无 TTL | `sync/flywheelSync.js:11` | 配置修改后 SW 仍使用旧值同步到错误服务器 |
| P0-7 | `workbenchOutbox.enqueue` 竞态 | `db/workbenchOutboxStore.js:53` | 相同 idempotencyKey 可能重复写入 |
| P0-8 | `legacyDataMaintenance` 全表加载 | `db/legacyDataMaintenance.js:20` | 数万条记录时 MV3 OOM 崩溃 |
| P1-14 | `asyncDispatch` fire-and-forget 无跟踪 | `content/messageHandlers.js:230` | 任务异常后永远显示 pending |
| P1-15 | taskPoller 与 content script 状态不一致 | `workbench/runtime/taskPoller.js:833` | 后台认为已暂停，前台仍在采集 |

### 2.3 稳定性与用户体验（P0-3, P1-1, P1-4, P1-8, P1-9）

| 编号 | 问题 | 文件位置 | 风险 |
|------|------|----------|------|
| P0-3 | SW 顶层执行长异步任务 | `background/index.js:2016` | SW 超 30 秒被 kill，任务状态丢失 |
| P0-4 | `BLOCK_MEDIA` 规则全局副作用 | `background/index.js:883` | 所有 tab 图片被永久拦截 |
| P1-1 | `sendToBackground` 无超时 | `shared/messaging.js:19` | Promise 永远挂起，UI 无响应 |
| P1-4 | `taskPoller` tick 锁非原子 | `workbench/runtime/taskPoller.js:982` | alarm 密集触发时重复执行 |
| P1-8 | Dashboard 无 Error Boundary | `dashboard/App.jsx` | 一个数据异常导致全页白屏 |
| P1-9 | React 中直接操作 DOM | `dashboard/App.jsx:510` | 虚拟 DOM 与实际 DOM 不一致 |

---

## 三、P2 / P3 重点改进建议

### 3.1 数据层（P2-7 ~ P2-12）

**IndexedDB schema 管理**: 当前 v1→v11 的迁移历史中有大量废弃索引未清理，建议：
1. 在 `docs/technical/DATA_MODEL.md` 中维护一份"活跃索引清单"
2. 新版本中通过 `.upgrade()` 显式删除已知废弃索引
3. `recordNormalization.js` 停止使用 `...record` 展开，改用白名单构建对象

**事务一致性**: `collectionRunStore.put`、`mediaAssetStore.bulkUpsert`、`accountStore.resetIfNewDay` 都缺少显式事务包裹，建议统一为 Dexie `db.transaction('rw', ...)` 模式。

### 3.2 工程化（P3-1 ~ P3-5, P3-10 ~ P3-12）

**测试基础设施**: 80 个 `.test.mjs` 文件是值得肯定的资产，但当前处于"无法自动运行"状态。建议：
1. 添加 `npm test` 脚本（Node.js 原生 `node --test` 即可支持 `.mjs`）
2. 配置 `c8` 覆盖率
3. 在关键模块（`taskPoller`、`flywheelSync`、`recordNormalization`）达到 70%+ 覆盖后再扩展新功能

**构建优化**:
1. 生产构建移除 source map（`devtool: false`）
2. 统一 manifest.json 与 package.json 版本号
3. 添加 ESLint 至少捕获 `no-unused-vars` 和 `no-console`（生产构建时）

### 3.3 文档同步（P3-6 ~ P3-9）

根据 CLAUDE.md 的"文档同步硬门禁"要求，以下文档与代码存在明显偏差：

| 文档 | 偏差内容 |
|------|----------|
| `TECH_STACK.md` | schema 版本声明为 v7（实际 v11） |
| `TECH_STACK.md` | dexie 版本 4.3.0（实际 4.0.1） |
| `TECH_STACK.md` | 构建体积记录（content.js 483K → 实际 570K） |
| `TECH_STACK.md` | 权限清单缺失 `tabs`、`cookies`、`alarms` |
| `progress.txt` | 285KB 已超阅读工具限制，建议按月拆分 |

---

## 四、修复优先级矩阵

### 🔴 本周必须修复（P0 + 部分 P1）

1. **P0-1** `manifest.json` 移除 `*.js` / `*.css` 通配符
2. **P0-5** `flywheelSync.js` 部分失败返回 `success: false`
3. **P0-6** `cachedConfig` 添加 TTL 或变更监听
4. **P0-7** `workbenchOutboxStore.js` 添加唯一索引或事务保护
5. **P0-8** `legacyDataMaintenance.js` 分页处理
6. **P0-10** `innerHTML` 全面替换为 `createElement` + `textContent`
7. **P1-1** `sendToBackground` 添加超时机制
8. **P1-8** Dashboard 添加 Error Boundary
9. **P1-2** `getDocumentCookie` 添加 `ensurePluginAuthorized()`

### 🟡 下周建议修复（剩余 P1 + 关键 P2）

10. **P1-3** background 消息路由添加 sender 验证
11. **P1-4** `taskPoller` tick 锁原子化
12. **P1-9** React 直接 DOM 操作改为 state 驱动
13. **P1-14** `asyncDispatch` 添加完成/失败回调通知
14. **P2-2** `normalizeServerUrl` 默认 `https://`
15. **P2-7** IndexedDB 废弃索引清理
16. **P2-11** `accountStore.getAll()` N+1 写入优化

### 🟢 按月排期（P2 + P3）

17. **P3-1** 添加 ESLint + Prettier
18. **P3-2** 配置测试运行器
19. **P3-3** 生产构建移除 source map
20. **P2-18** content script 样式从 inline JS 迁移到 CSS class
21. **P3-13** 统一日志封装，生产环境移除 `console.*`
22. **P3-9** `progress.txt` 按月拆分

---

## 五、正向发现（值得保持的做法）

1. **测试覆盖意愿**: 80 个测试文件体现了对质量的投资，只需补齐运行基础设施即可发挥价值
2. **XSS 防护意识**: `escapeHtml()` 实现正确（覆盖 `& < > " '`），`dangerouslySetInnerHTML` 目前仅用于可信的 `icon()` 输出
3. **selector 健康检查机制**: `selectorHealth.js` 的预检、过期检测、告警去重设计是成熟的工程实践，只需保持验证日期更新
4. **主题隔离**: `themeManager.js` 支持两套主题切换，popup/dashboard 都有主题感知
5. **错误分类**: `feedback.js` 中的 `FEEDBACK_META` 提供了统一的用户-facing 错误语义
6. **无 debugger 语句**: 源码中没有任何遗留的 `debugger;` 断点
7. **无 TODO/FIXME 堆积**: 虽然没有 inline 标记，但也说明没有明显的"已知未完成"功能被搁置

---

## 六、审查文件索引

| 报告 | 路径 |
|------|------|
| Phase 1 致命级审查 | `docs/reviews/PHASE1_FATAL_REVIEW_2026-04-28.md` |
| Phase 2 高危级审查 | `docs/reviews/PHASE2_HIGH_RISK_REVIEW_2026-04-28.md` |
| Phase 3 中危级审查 | `docs/reviews/PHASE3_MEDIUM_RISK_REVIEW_2026-04-28.md` |
| Phase 4 低危级审查 | `docs/reviews/PHASE4_LOW_RISK_REVIEW_2026-04-28.md` |
| **本汇总报告** | `docs/reviews/COMPLETE_REVIEW_SUMMARY_2026-04-28.md` |

---

*审查完成时间*: 2026-04-28
*下次建议审查周期*: 2-3 周后（重点验证 P0/P1 修复情况 + selector 验证日期刷新）
