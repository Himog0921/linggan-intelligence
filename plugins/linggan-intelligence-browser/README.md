# Linggan Intelligence Browser

> 状态: Draft LOCAL_TRUSTED adapter
> 版本: `0.4.4`
> 适用范围: `PLUGIN-RETROFIT-LOCAL-TRUSTED-001`（GitHub Issue #43）
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

当前活跃 service worker 是 `src/linggan/background.js`。它只连接 Linggan 本机
`http://localhost:3000`；旧工作台的站点授权、任务轮询、lease、工位调度、旧
endpoint、同步和 fallback 都不是当前运行路径。

当前页面的「发现当前 20 条」是一个受限的手动 Discovery 入口：只读取此刻已经渲染在
当前页面的最多 20 张卡片，不滚动、不调用页面内部接口快照、不打开详情。它先将
固定 `TaskSpec → Attempt → Submission` 写入独立的浏览器本地 outbox，再由后台以小批次
向 Linggan loopback 发送。页面侧采集不等待网络回执；服务中断、超时或 service worker
重启后，同一 `submissionId` 会继续重试，服务端回执幂等。这个 outbox 只保存待交付材料，
不是 Evidence、也不代表平台采集已完成。每个 attempt 只能有一个终态 package：相同
submission 只能 replay，新的 package 必须创建新的 attempt。scheduler 明确为
`NOT_CONNECTED`。

在尚未从 Linggan 读取统计时，插件和 Popup 只显示“未连接”或“未知”；绝不以 `0` 伪装
成没有笔记、评论或博主。

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

发行包生成在 `releases/linggan-intelligence-browser-v0.4.4.zip`。打包器以
固定 ZIP 时间戳和稳定文件顺序生成；`releases/release-manifest.json` 记录已提交
ZIP 的 SHA-256。`npm run verify` 不会改写 release ZIP：它会以新的 `npm ci`、build
和临时 ZIP 重新打包，并要求该 SHA-256 与已提交 ZIP 完全一致，然后运行旧工作台
运行时隔离扫描。

## 未证明事项

本包可构建和可打包；合成回传测试可证明本机 Task/Attempt/Submission 的持久交付与
幂等 receipt。它**不证明**浏览器已加载、真实 XHS 或抖音页面可采、真实内容已入库、
封面/媒体已本地化，或 OCR/ASR/研究链已运行。真实接入只能在对应的 Linggan backend
合同、权限、Coverage、媒体生命周期和用户明确授权全部具备后另行验证。
