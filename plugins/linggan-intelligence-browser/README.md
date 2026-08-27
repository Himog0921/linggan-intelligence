# Linggan Intelligence Browser

> 状态: Draft target-driven XHS Producer adapter
> 版本: `0.4.8`
> 适用范围: `PLUGIN-XHS-ACTIVE-COLLECTION-001`（GitHub Issue #78）、`PLUGIN-XHS-ADAPTIVE-SCROLL-AND-DETAIL-RECEIPT-001`（GitHub Issue #80）及其既有 LOCAL_TRUSTED 接收边界
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

小红书的「按目标发现」会以任务请求数量为上限，继续滚动和去重：达到目标、确认页面
已无新增、无新增但未证明页面到底，或达到执行预算时，都会保留实际数、轮次和停止原因。
每轮仅记录可解释的页面加载事实（可见数量、增量数量、滚动位置/高度、到底状态与停止原因），
不记录笔记原文、链接或标识。搜索页会同时
带回当前可见筛选状态和下拉联想的页面事实；它不把自然到底说成平台全量，也不把实际数
反写成任务目标。

标准「采集当前笔记到 Linggan」是一次逻辑结果：详情、媒体槽位观察以及**最多 30 条**当前评论。
它不会打开媒体选择窗口，也不会下载媒体字节；界面逐条显示“笔记详情、媒体观察、评论与回复”
的本机接纳状态，其中任一 lane 尚在队列或未接纳时，不会把正文接纳误说为整个详情包已接纳。
「人工采集并下载媒体」是并列的明确人工动作：先保留同一详情结果，再打开原有选择窗口，按
人工选择下载本地媒体。单篇评论深采和批量评论是另一类明确执行任务：可设上限，
或选择“尽量采到公开自然结束”；它们不受标准详情 30 条窗口限制。批量评论逐篇保留
暂停/恢复 checkpoint；页面明确无评论是一篇成功的空结果，页面打不开、超时和用户停止
则各自保留原因，不连坐已取得的其它目标。

所有页面动作先将固定 `TaskSpec → Attempt → Submission` 写入 Linggan 专属的浏览器本地
outbox，再由后台以小批次向 Linggan loopback 发送。页面侧采集不等待网络回执；服务中断、
超时或 service worker 重启后，同一 `submissionId` 会继续重试，服务端回执幂等。这个
outbox 只保存待交付材料，不是 Evidence、也不代表平台采集已完成。每个 attempt 只能有
一个终态 package：相同 submission 只能 replay，新的 package 必须创建新的 attempt。
scheduler 明确为 `NOT_CONNECTED`。

交付前，插件只信任 `GET /health` 在 `routes.localProducer` 中同时公布的
`taskCreation`、`attemptStart` 与 `submission` 三条本机路径；后台会按这一份 route bundle
依次创建 Task、开始 Attempt、提交 Package。只有完整运行时
`LINGGAN_BROWSER_PRODUCER_RUNTIME / PLUGIN_RUNTIME_001_SCHEMA_READY` 才会公布并允许使用
这一份完整 bundle。旧的 `LOCAL_TRUSTED_PRODUCER / LOCAL_003_SCHEMA_READY` 可以说明本机服务
可访问，但不是 Producer 交付就绪状态：它不会公布 bundle，也不会发送 Task、Attempt 或 Package。
health 缺少任一条、标识不成对或不在 ready 状态时，材料只会
保留为 retryable 本机 outbox 项，不显示为已接纳、已入库或已展示。

在尚未从 Linggan 读取统计时，插件和 Popup 只显示“未连接”或“未知”；绝不以 `0` 伪装
成没有笔记、评论或博主。

当前运行时的 Dexie 名称为 `LingganIntelligenceBrowserLocalStaging`，仅作**本机暂存/恢复层**，
不是 Linggan 的最终事实库，也不会复用旧 `LingganBoomDB`。媒体原件只在用户单独请求时
进入独立字节通道；候选地址必须是受允许的平台 HTTPS 媒体地址，页面回传也使用一次性请求
标识，不能把任意页面消息或本机地址送入下载/交付队列。

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

发行包生成在 `releases/linggan-intelligence-browser-v0.4.8.zip`。打包器以
固定 ZIP 时间戳和稳定文件顺序生成；`releases/release-manifest.json` 记录已提交
ZIP 的 SHA-256。`npm run verify` 不会改写 release ZIP：它会以新的 `npm ci`、build
和临时 ZIP 重新打包，并要求该 SHA-256 与已提交 ZIP 完全一致，然后运行旧工作台
运行时隔离扫描。

## 未证明事项

本包可构建和可打包；合成回传测试可证明本机 Task/Attempt/Submission 的持久交付与
幂等 receipt。它**不证明**浏览器已加载、真实 XHS 或抖音页面可采、真实内容已入库、
封面/媒体已本地化，或 OCR/ASR/研究链已运行。真实接入只能在对应的 Linggan backend
合同、权限、Coverage、媒体生命周期和用户明确授权全部具备后另行验证。
