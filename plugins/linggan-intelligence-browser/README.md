# Linggan Intelligence Browser

> 状态: 自动观察与固定材料深化 Producer
> 版本: `0.8.8`
> 适用范围: `OBSERVATION-RUNTIME-001`、`MEDIA-ACQUISITION-001` 与 `MATERIAL-DEEPENING-001`（GitHub Issue #103）
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
scheduler 由 Linggan 服务端常驻 worker 负责。插件以 MV3 alarm、安装/启动唤醒和单飞锁
自动签到、领取并执行已批准任务；Popup 的人工按钮不再是正常调度路径。

交付前，插件只信任 `GET /health` 在 `routes.localProducer` 中同时公布的
`taskCreation`、`attemptStart` 与 `submission` 三条本机路径；后台会按这一份 route bundle
依次创建 Task、开始 Attempt、提交 Package。只有完整运行时
`LINGGAN_BROWSER_PRODUCER_RUNTIME / PLUGIN_RUNTIME_002_SCHEMA_READY` 才会公布并允许使用
这一份完整 bundle。旧的 `LOCAL_TRUSTED_PRODUCER / LOCAL_003_SCHEMA_READY` 可以说明本机服务
可访问，但不是 Producer 交付就绪状态：它不会公布 bundle，也不会发送 Task、Attempt 或 Package。
health 缺少任一条、标识不成对或不在 ready 状态时，材料只会
保留为 retryable 本机 outbox 项，不显示为已接纳、已入库或已展示。

在尚未从 Linggan 读取统计时，插件和 Popup 只显示“未连接”或“未知”；绝不以 `0` 伪装
成没有笔记、评论或博主。

当前运行时的 Dexie 名称为 `LingganIntelligenceBrowserLocalStaging`，仅作**本机暂存/恢复层**，
不是 Linggan 的最终事实库，也不会复用旧 `LingganBoomDB`。从 0.7.0 起，已授权的
`discovery_search` / `profile_discovery` 回执中出现的封面会由服务端建立有界媒体取得工作，
插件在后台自动领取并进入独立字节通道；候选地址必须是受允许的平台 HTTPS 媒体地址，
页面回传也使用一次性请求
标识，不能把任意页面消息或本机地址送入下载/交付队列。0.8.0 在上游冻结明确作品集合后，
可自动执行 `content_detail → media_slots → comments → replies`；每个任务只请求一个能力，
评论最多 30 条、回复展开最多 2 层，服务端媒体和处理工作均最多尝试 3 次。
0.8.1 修复无人值守打开页面时的加载竞态：页面在监听器安装前已完成时，不再误等 20 秒后关窗，
已领取但尚未产生 Attempt 的同一任务可由同一安装按服务端幂等派发继续执行。
0.8.2 对齐内容工作台已经验证的 XHS 详情执行合同：服务端在派发时从最新已接纳发现材料读取
带 `xsec_token` 的来源链接，插件核对 HTTPS 平台域名、作品 ID 与 token 后，转换成
`/discovery/item/{id}` 执行链接；没有签名链接时任务保持等待，绝不回退到打不开的裸
`/explore/{id}`。插件升级产生新安装实例时，同一工位会承接前一安装尚在有效租约内的同一
Task，不重建任务、不重跑已完成步骤。
0.8.3 修复签名详情页的二次导航时序：XHS 可能先把 `/discovery/item/{id}` 标为加载完成，
再跳转到保留 token 的 `/explore/{id}`。插件现在要求 URL 与加载状态保持 1.5 秒稳定，且最终
页面的 content script 对同一 URL 回应后才发送采集动作；首次 `complete` 不再被误认作可执行
终态。整个等待仍受 20 秒上限约束，不会无限轮询。
0.8.4 继续收口真实详情读取：结构化详情已经取得标题、正文、作者或媒体时，互动字段在
有界等待后仍不完整不会再连坐整条材料；缺失的点赞、收藏、评论、分享保持 `null/UNKNOWN`，
不补成 0。页面采集器的失败回执也会原样返回后台，不再被统一包装为“已开始”。固定作品
深化还会由服务端在首个 `content_detail` 派发中附带同一 WorkOrder 已批准的同页读取范围；
插件只打开一次带签名详情页，沿用成熟单篇详情采集能力读取详情、媒体候选及有界评论树，
并把结果保存到独立的 MV3 可恢复缓存。后续 `media_slots`、`comments`、`replies` 仍须分别
领取自己的 TaskSpec，分别形成 Attempt、Package 和 Receipt，只是不再重复打开同一详情页。
缓存按 Lease 与作品隔离、受租约剩余时间约束；缺失或过期时继续走原有单 lane 页面执行，
不会用缓存扩大 WorkOrder 范围。
0.8.5 补齐内容工作台 `2.0.93` 已经真实验证过、迁入时遗漏的 XHS SSR 详情读取路径：
部分详情路由在页面水合后会删除全局 `__INITIAL_STATE__`，但原始页面脚本仍保留序列化的
`noteDetailMap`。插件现在只做有界 JSON 对象解析，绝不执行页面脚本文本；详情就绪判断、
完整度判断和正式单篇采集共用这一来源。旧详情容器缺失但 SSR 详情存在时，启动探针不再
误报 selector blocked。全局运行态和 DOM fallback 继续保留，数据库、TaskSpec、Attempt、
Package 与 WorkOrder 范围均未改变。
0.8.6 把标准详情评论窗口与单篇深采的 Coverage 分开：页面显示数等于本 Attempt
唯一采回数时可记为完整，短采保留部分材料且下次仍从评论入口重采；服务端按稳定评论身份
形成当前投影，不修改旧 Attempt。
0.8.7 修复真实页面首次接纳暴露的评论树边界：顶层评论的自指 `rootCommentId` 不再被误判为
回复，评论/回复出包前会把页面的多关系表示规范成服务端唯一关系合同；手工“公开自然结束”
任务使用 `maximumQuota: null` 和人工/时间/风险/自然结束条件表达无数值配额，不再把实际结果
倒填成事前目标。有限评论任务继续保留明确上限，scheduled TaskSpec 仍原样执行。
0.8.8 将详情附带评论、单篇深采和批量评论收口到同一克制执行策略：每轮只选择一次 API 分页、
一次回复展开或一次页面滚动，动作间至少冷却 1.2 秒，并在动作前后检查暂停/停止及等待页面状态。
进度分别保存本次唯一评论数、页面公开评论数和用户上限，不再产生 `130/53` 之类的伪分母；
人工暂停会把当下真实评论树作为 `PARTIAL / manual_pause` Package 交给 Linggan，继续执行仍沿当前
Attempt，重新执行仍从评论入口开始。媒体领取会在写入可靠队列前先安排一分钟恢复唤醒，并将
领取后的入队/执行失败绑定到精确工作代际回报，避免只留下 `lease_expired`。

实况图片仍是一个逻辑媒体卡槽，但静态图与动态图分别携带候选地址、取得工作和状态；
普通图片、封面、视频和实况图片都只把远程 URL 当来源观察，长期展示必须使用 Linggan
本地 Materialization。图片 OCR、视频抽帧 OCR、音频提取与 ASR 由本机 Rust worker 调用
Tesseract、FFmpeg 和本地 Whisper 处理，插件不执行语义分析。

本轮仍不使用 `cookies`、`downloads`、网络规则或通知权限；`alarms` 仅用于 MV3 后台唤醒、
自动签到、受控领取与媒体工作，不成为本地授权来源。平台 host permission 只服务已批准
TaskSpec 的页面执行和受限媒体候选取得。

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

发行包生成在 `releases/linggan-intelligence-browser-v0.8.8.zip`。打包器以
固定 ZIP 时间戳和稳定文件顺序生成；`releases/release-manifest.json` 记录已提交
ZIP 的 SHA-256。`npm run verify` 不会改写 release ZIP：它会以新的 `npm ci`、build
和临时 ZIP 重新打包，并要求该 SHA-256 与已提交 ZIP 完全一致，然后运行旧工作台
运行时隔离扫描。

## 未证明事项

本包可构建和可打包；合成回传测试可证明本机 Task/Attempt/Submission 的持久交付与
幂等 receipt。它**不证明**浏览器已加载、真实 XHS 或抖音页面可采、真实内容已入库、
OCR/ASR/研究链已运行。封面是否已本地化以 Evidence 返回的本地物化句柄为准；来源链接
只保留为观察事实，不能作为长期展示保证。
