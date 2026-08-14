# 技术栈

> 本文档锁定当前项目技术栈与版本，避免"随机依赖"。

## 1. 运行环境

- 浏览器：Chrome（Manifest V3 扩展）
- 目标站点：小红书、抖音，以及对应媒体 CDN 与本地飞轮调试地址
- 语言：JavaScript（ES Modules）+ JSX
- 前端框架：React 19（createRoot / Hooks）
- 样式：原生 CSS
- 数据存储：IndexedDB（Dexie）
- 当前本地 schema：Dexie `v13`

## 2. 核心版本（锁定）

| 包 | 版本 |
|----|------|
| `dexie` | `4.3.0` |
| `webpack` | `5.105.4` |
| `webpack-cli` | `5.1.4` |
| `copy-webpack-plugin` | `12.0.2` |
| `css-loader` | `7.1.4` |
| `mini-css-extract-plugin` | `2.10.1` |
| `css-minimizer-webpack-plugin` | `6.0.0` |
| `react` / `react-dom` | `19.2.5` |
| `@babel/preset-react` | — |
| `babel-loader` | — |

说明：版本以当前 `npm ls --depth=0` / `package-lock.json` 为准。

## 3. 插件架构入口

| 入口 | 文件 |
|------|------|
| Content Script | `src/content/index.js` |
| Background Service Worker | `src/background/index.js` |
| Popup | `src/popup/index.jsx` |
| Dashboard | `src/dashboard/index.jsx` |

说明：
- `src/popup/popup.html` 与 `src/dashboard/dashboard.html` 只是 UI 容器壳文件。
- 运行时加载的 `popup.js` 与 `dashboard.js` 都是由各自的 `index.jsx` 构建生成，不再对应旧的原生 DOM 源文件。
- Webpack 配置文件为 `webpack.config.cjs`；项目源码按 ES Module 运行。

## 4. Chrome API 使用清单

- `chrome.runtime.sendMessage`
- `chrome.runtime.onMessage`
- `chrome.tabs.sendMessage`
- `chrome.tabs.query`
- `chrome.action.setBadgeText`
- `chrome.action.setBadgeBackgroundColor`
- `chrome.storage.local`
- `chrome.declarativeNetRequest.updateDynamicRules`
- `chrome.downloads.download`

## 5. 权限清单（manifest）

> 事实源：`manifest.json`。本项目**不使用** `chrome.debugger`（关闭弹窗/派发 Esc 走 `chrome.scripting.executeScript`）。

permissions：

- `activeTab`
- `tabs`
- `storage`
- `cookies`
- `downloads`
- `alarms`
- `scripting`
- `declarativeNetRequest`
- `declarativeNetRequestWithHostAccess`
- `notifications`

host_permissions：

- `http://localhost/*`
- `https://lingganboom.fun/*`
- `https://xiaohongshu.com/*`
- `https://*.xiaohongshu.com/*`
- `https://www.xiaohongshu.com/*`
- `https://ci.xiaohongshu.com/*`
- `https://*.xhscdn.com/*`
- `https://www.douyin.com/*`
- `https://*.byteimg.com/*`
- `https://*.douyinpic.com/*`
- `https://*.douyinstatic.com/*`
- `https://*.amemv.com/*`
- `https://*.douyinvod.com/*`
- `https://*.bytevcloudcdn.com/*`

## 6. 当前构建快照（2026-06-25，v2.0.52 + 死代码清理后）

- `vendor.js`（React 共享 chunk）：约 `185 KiB`
- `content.js`：约 `651 KiB`
- `background.js`：约 `305 KiB`
- `popup.js`：约 `75 KiB`
- `dashboard.js`：约 `52 KiB`
- 内容脚本异步 chunk：无

备注：
- React 运行时通过 `splitChunks` 提取为 `vendor.js`，由 popup / dashboard / content 共享
- content script 注入 UI（按钮组、任务控制条、Toast、Dialog）已全部迁移为 React 组件
- 抖音运行时、内容数据运行时保持 eager 动态加载，避免 Chrome 内容脚本运行空间导致异步 chunk 加载失败
