# Linggan Intelligence Browser

> 状态: Draft migration baseline
> 版本: `0.3.0`
> 适用范围: `PLUGIN-REHOME-001`（GitHub Issue #41）
> 事实来源: 当前 package source、`MIGRATION-MAP.md`、构建与隔离检查输出
> 冲突时以谁为准: 用户最新确认、仓库 `AGENTS.md`、当前代码和实际运行证明

这是 Linggan Intelligence 自有的 Chrome Manifest V3 插件源码包。它从
`Himog0921/linggan-boom@8a00cc1`（审计版本 `2.0.91`）迁入完整浏览器端
源码、资产和操作界面；旧内容工作台不是运行依赖、数据中转或发布来源。

## 本轮实际迁入

- 原有 Popup、Dashboard、页面注入控制、主题、品牌资产、XHS/抖音采集器源码和浏览器本地暂存/恢复代码；
- 原有多平台页面识别、单条/批量/媒体动作的界面位置和操作反馈结构；
- 独立 npm build、发布 ZIP 和安装所需的 Manifest V3 source。

这保留的是用户熟悉的插件外形和成熟源码基线，不等于这些动作已经获得
Linggan 的接收合同或平台访问授权。

## 当前行为与明确边界

当前活跃 service worker 是 `src/linggan/background.js`。它只认识 Linggan
本机 `http://localhost:3000` 的 readiness 检查和界面状态；旧工作台的站点
授权、任务轮询、lease、工位调度、旧 endpoint、同步和 fallback 都不是当前
运行路径。

原有浏览器本地 Dexie 数据和恢复代码仅保留为**本机暂存/恢复层**，不是
Linggan 的最终事实库。尚未获得 Linggan adapter 合同的详情、评论、媒体、
批量、抖音和自动化动作仍保留在原界面位置，但会显示明确的“未接通”原因；
它们不会静默访问平台、下载媒体或写入 Linggan。

本轮刻意移除了不需要的 `cookies`、`downloads`、`alarms`、网络规则和通知
权限。平台内容脚本仍用于保持原注入界面与页面识别体验；可执行采集动作在
Linggan 接收合同接通前被明确阻断。

完整逐项清单见 [MIGRATION-MAP.md](MIGRATION-MAP.md)。

## Build 与可复现发行包

```bash
cd plugins/linggan-intelligence-browser
npm ci --ignore-scripts
npm run build
npm run package:release
npm run release:manifest
npm run release:verify
npm run release:reproducibility
npm run verify:linggan-isolation
```

发行包生成在 `releases/linggan-intelligence-browser-v0.3.0.zip`。打包器以
固定 ZIP 时间戳和稳定文件顺序生成；`releases/release-manifest.json` 记录已提交
ZIP 的 SHA-256。`npm run verify` 不会改写 release ZIP：它会以新的 `npm ci`、build
和临时 ZIP 重新打包，并要求该 SHA-256 与已提交 ZIP 完全一致，然后运行旧工作台
运行时隔离扫描。

## 未证明事项

本包可构建和可打包，**不证明**浏览器已加载、Linggan backend 已接通、XHS 或
抖音实际页面可采、真实内容已入库、封面/媒体已本地化，或 OCR/ASR/研究链已运行。
真实接入只能在对应的 Linggan backend 合同、权限、Coverage、媒体生命周期和
用户明确授权全部具备后另行验证。
