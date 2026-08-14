# Phase 4 低危级审查报告（Low Risk / P3）

**审查日期**: 2026-04-28
**审查范围**: 构建工程、文档一致性、可测试性、代码整洁度
**审查文件**: package.json, webpack.config.js, manifest.json, docs/technical/TECH_STACK.md, progress.txt, tests/*.test.mjs, src/**/*.js, src/**/*.jsx

---

## 4.1 构建工程

### [P3-1] 缺少 ESLint / Prettier 配置

**位置**: 项目根目录
**问题**: 项目中不存在 `.eslintrc`、`.prettierrc`、`eslint.config.js` 等任何代码风格配置文件。在 80+ 个源文件和 80+ 个测试文件、多人协作（或 AI 辅助）场景下，缺乏统一的格式约束会导致：
- 代码风格 drift（引号、分号、缩进不一致）
- 潜在 bug 无法被静态分析捕获（如未使用的变量、错误的 async/await 用法）

**修复建议**: 添加 ESLint（推荐 `eslint:recommended` + `plugin:react-hooks/recommended`）和 Prettier 配置。

---

### [P3-2] package.json 缺少测试运行脚本

**位置**: `package.json:6-19`
**代码**:
```json
"scripts": {
  "build": "webpack --mode production",
  "dev": "webpack --mode development --watch",
  ...
}
```
**问题**: `tests/` 目录下有 80 个 `.test.mjs` 文件，但 `package.json` 中没有 `test`、`test:unit`、`test:watch` 等任何脚本。这意味着测试无法通过 `npm test` 运行，测试文件可能是通过临时 `node` 命令或外部工具手动执行的。

**修复建议**: 配置测试运行器（Node.js 原生 test runner、`vitest` 或 `jest`）并添加 `npm test` 脚本。

---

### [P3-3] 生产构建包含 source map

**位置**: `webpack.config.js:75`
**代码**:
```js
devtool: 'cheap-module-source-map',
```
**问题**: 该配置在 `production` 模式下同样生效（webpack 配置中没有按 mode 条件切换）。source map 会随扩展一同打包上传到 Chrome Web Store，泄露完整源码结构，增加被逆向分析的风险。

**修复建议**: 使用条件配置：`devtool: process.env.NODE_ENV === 'production' ? false : 'cheap-module-source-map'`。

---

### [P3-4] manifest version 与 package.json version 不一致

**位置**: `manifest.json:4` vs `package.json:3`
**代码**:
```json
// manifest.json
"version": "2.0.0"
// package.json
"version": "1.0.0"
```
**问题**: 两个版本号长期 diverge，会导致发布流程混淆（脚本读取哪个版本？用户看到的版本以哪个为准？）。

**修复建议**: 统一版本源，例如让构建脚本从 `manifest.json` 同步到 `package.json`，或反之。

---

### [P3-5] 构建产物体积与文档记录不符

**位置**: `docs/technical/TECH_STACK.md:82-86` vs 实际 `dist/`
**文档记录**:
```
- content.js：约 483 KiB
- background.js：约 194 KiB
```
**实际构建** (2026-04-28):
```
content.js     570K
background.js  215K
```
**问题**: 文档中的构建快照已过时，content.js 比记录大 18%。体积增长如果来自依赖膨胀，需要关注 MV3 的内存限制。

**修复建议**: 更新文档中的构建快照，或设置 CI 监控 bundle size。

---

## 4.2 文档一致性

### [P3-6] TECH_STACK.md 声明的 Dexie schema 版本过期

**位置**: `docs/technical/TECH_STACK.md:13`
**代码**:
```md
- 当前本地 schema：Dexie `v7`
```
**问题**: `src/db/index.js` 已升级到 `v11`（新增了 `accounts` 表和 `workbenchOutbox` 复合索引）。文档声明的 v7 已过时，可能导致新开发者对数据模型产生错误认知。

**修复建议**: 更新为 `v11`，并在每次 schema 升级时同步此文档。

---

### [P3-7] TECH_STACK.md 依赖版本与 package.json 不符

**位置**: `docs/technical/TECH_STACK.md:17-28` vs `package.json:21-37`
**差异示例**:
| 包 | TECH_STACK.md | package.json |
|---|---|---|
| dexie | 4.3.0 | 4.0.1 |
| webpack | 5.105.4 | 5.90.0 |
| mini-css-extract-plugin | 2.10.1 | 2.8.0 |
| css-minimizer-webpack-plugin | 6.0.0 | 6.0.0（一致） |

**问题**: 文档版本高于实际安装版本。如果文档被当作“权威版本锁定”参考，可能造成依赖升级决策错误。

**修复建议**: 统一版本声明来源，或在 CI 中添加文档版本与 `package.json` 的自动校验。

---

### [P3-8] TECH_STACK.md 权限清单不完整

**位置**: `docs/technical/TECH_STACK.md:59-66`
**问题**: 文档列出的权限缺少 `tabs`、`cookies`、`alarms`，而这三个权限都在 `manifest.json:6-16` 中声明。文档作为安全审查的入口，缺失权限会造成审计盲区。

**修复建议**: 补全缺失权限，并建立 manifest ↔ 技术文档的双向校验机制。

---

### [P3-9] progress.txt 体积过大（285KB）

**位置**: `progress.txt`
**问题**: 该文件已达 285KB，接近阅读工具的 256KB 限制。随着项目持续迭代，这个文件会无限增长，最终失去"快速查看当前进度"的设计初衷。

**修复建议**: 按月份/轮次拆分历史记录（如 `progress-2026-04.txt`），`progress.txt` 只保留当前迭代。

---

## 4.3 可测试性

### [P3-10] 80+ 测试文件无运行基础设施

**位置**: `tests/*.test.mjs`（80 个文件）
**问题**: 项目有大量测试文件覆盖各模块（selector health、task poller、response envelope 等），但：
- 无 `npm test` 脚本
- 无测试运行器依赖（jest/vitest/node:test）
- 无 CI 集成自动跑测试

这意味着测试的"保鲜度"无法保证——代码已演进，测试可能已静默失效。

**修复建议**: 至少配置 Node.js 原生 `node --test` 运行器（支持 `.mjs`），将测试纳入提交前检查。

---

### [P3-11] 测试文件使用 .mjs 但源码使用 .js/.jsx

**位置**: `tests/*.test.mjs`
**问题**: 源码使用 CommonJS/ESM 混合的 Webpack + Babel 构建体系，而测试使用原生 ESM（`.mjs`）。这种差异意味着测试无法直接 `import` JSX 组件或需要 Babel 转换的源码，除非测试运行器也配置相同的转换管道。

**修复建议**: 明确测试策略——如果测试只测纯逻辑（不测 JSX），保持 `.mjs` 可行；如果需要测组件，需统一构建管道。

---

### [P3-12] 无代码覆盖率工具

**位置**: 项目根目录
**问题**: 没有配置 `c8`、`istanbul` 或任何覆盖率工具，无法量化测试覆盖范围，也难以识别未测试的临界路径。

**修复建议**: 添加覆盖率收集（`node --test` + `c8` 是最轻量的方案）。

---

## 4.4 代码整洁度

### [P3-13] 生产代码中存在 76 条 console 语句

**位置**: `src/**/*.js`, `src/**/*.jsx`
**统计**: `grep -rn "console\." src/ | wc -l` = 76
**示例**:
- `src/background/index.js:2024`: `console.log('[灵感爆爆爆] Background Service Worker 已启动');`
- `src/platforms/xhs/noteCollector.js:350`: `console.log('[灵感爆爆爆] note 原始数据:', JSON.stringify({...}));`
- `src/platforms/xhs/batchController.js:694`: `console.log(`[灵感爆爆爆] 采集成功: ...`);`

**问题**: 
- 用户浏览器控制台会被大量插件日志污染
- 某些 `console.log` 包含原始数据（如 `JSON.stringify({...})`），可能泄露敏感采集内容到控制台
- 在 MV3 Service Worker 中，`console.log` 虽然会输出到扩展的 background page，但过多的日志会影响调试体验

**修复建议**: 引入统一的日志封装（如 `src/shared/logger.js`），生产构建时通过 webpack DefinePlugin 或 terser `drop_console` 移除日志。

---

### [P3-14] jszip 依赖疑似未使用

**位置**: `package.json:34`
**代码**:
```json
"dependencies": {
  "dexie": "^4.0.1",
  "jszip": "^3.10.1",
  "react": "^19.2.5",
  "react-dom": "^19.2.5"
}
```
**问题**: `jszip` 被列为运行时依赖，但在核心源码中未找到 `import JSZip` 或 `import * as JSZip` 的引用。如果确实未使用，会增加 bundle 体积和攻击面。

**修复建议**: 验证是否在某个分支或动态导入中使用；如无使用，移除该依赖。

---

### [P3-15] 无 TODO/FIXME 内联标记

**位置**: `src/**/*.js`, `src/**/*.jsx`
**问题**: 整个源码中没有任何 `TODO`、`FIXME`、`HACK`、`XXX` 注释。这表面看是"代码很干净"，但实际上意味着：
- 技术债务没有 inline 标记，新开发者无法快速识别已知问题区域
- 可能说明代码审查过于严格地清除了所有标记，而非真正解决了底层问题

**修复建议**: 允许在代码中保留带日期和上下文的技术债务标记（如 `// TODO(2026-04-28): 移除 debugger 权限替代方案`），并配套跟踪到 `docs/plans/tech-debt.md`。

---

### [P3-16] manifest.json web_accessible_resources 仍包含通配符

**位置**: `manifest.json:76-77, 92-93`
**代码**:
```json
"resources": [
  "*.js",
  "*.css",
  ...
]
```
**问题**: 这是 P0-1 的遗留问题。虽然在 P0 审查中已标记为致命级，但在实际 manifest.json 中仍然存在。此处作为 P3 重新提及，是因为这是一个"配置级"修复，风险虽高但改动极小（仅需精确列举文件）。

**修复建议**: 精确列举所需文件，移除 `*.js` 和 `*.css` 通配符。

---

## Phase 4 审查统计

| 级别 | 数量 | 分类 |
|------|------|------|
| P3（低危）| 16 项 | 构建工程 5 项，文档一致性 4 项，可测试性 3 项，代码整洁度 4 项 |
