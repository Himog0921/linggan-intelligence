# Linggan Intelligence Browser

> 状态: 自动观察与固定材料深化 Producer
> 版本: `0.8.39`
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

### 执行工位：认领即自动接活，名称以 Intelligence 为准

工位的可读名称只有服务端 `execution_station.display_name` 一份真相。人在 Collection Runtime
注册或更名（注册提示为“本机 Chrome”）后，插件通过 check-in 取得该名称和 `stationAccepting`，
并在 Popup 原样回显；插件不保存/编辑本地别名，也不把名称当作安装身份或配对键。

未被 claim 的工位只是待认领资源；一个安装成功 claim 后，服务端写入审计并默认开启“自动接活”。
这不等于立即采集：服务端仍逐一核对最新心跳、凭证、最低版本、能力、账号绑定/资格、风险、
并发 Lease 和 station 预算，且必须确有合格排队任务，才会派 Lease。人通过 Runtime 显式暂停
后，`person_disabled` 是持久覆盖；心跳、重装或替换安装都不能自行恢复，只有人显式恢复才会重开。

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
0.8.9 根据 0.8.8 的真实长评论验收继续修正：不限数量任务会使用页面公开评论数作为当前
集合目标，API 首屏不足时必须进入同一 DOM 补采器，而不是在首屏 `no_progress`。
每个新 Attempt 还会先清除同标签页内旧评论页和楼中楼快照，再从空游标重新请求第一页；不把上一
Attempt 的缓存当成本次重采结果。新领取的
媒体工作代次会优先于旧浏览器本地媒体行执行，平台候选下载最长等待 20 秒，避免旧请求悬挂
并把新封面工作拖过 Lease。0.8.8 已真实证明 `15/492` 人工暂停会形成可接纳的
`PARTIAL / manual_pause` 评论与回复 Package；完整 492 条与封面物化仍以 0.8.9 重载后复验为准。
0.8.10 修正 0.8.9 真实复验暴露的最后一处公开数传递断点：当详情页标题不含
“共 N 条评论”文案时，API collector 的终态续采判定直接使用当前详情上下文已观察的公开数，
不再将已正确显示的 `18/508` 误判为无目标 `no_progress`。
0.8.11 把这个页面公开数从“某一轮页面节点上临时可读”收口为 Attempt 内持久观察值：
任一轮进度观察到 514 后，即使虚拟列表在终态切换了容器，续采决策仍使用 514，不再退回空目标。
0.8.12 按真实 `.note-scroller` 探针修正滚动停止语义：首批 18 个节点占用约 3000px，前 560px 下移不会
立即产生新节点。只要 `scrollTop` 仍能前进或 `scrollHeight` 继续增长，就不累计 no-progress；只有到底且
无新节点才进入有界停止计数。
0.8.13 修复真实单篇深采暴露的深度参数断点：设置弹窗已经选择“全部展开”时，单篇任务控制器会把
`commentDepthMode=allReplies` 原样交给共享评论采集器；不再只把该值保存在任务状态、却让执行器按默认
两级模式运行。有限/不限配额、动作冷却、暂停交付和新 Attempt 从头重采语义均不改变。
0.8.14 将已进入主线的 Work Resource Read 平台读取、Evidence Library 视图收口与
0.8.13 评论执行修复固定为同一发行快照；新增真实 XHS 详情 `time:number` 毫秒 epoch
回归样本，确认现有 `xhs-detail-time-v2` 映射无需另行降级或分支适配。
0.8.15 修复 0.8.14 真实全回复 Attempt 暴露的 DOM 切换死循环：当本轮首个
页面 API 请求未返回评论时，立即交给共享 DOM 采集器解析当前已可见评论，再按
1.2 秒最低冷却展开回复与滚动；不再在评论节点已存在时反复点击同一展开入口并停在 0。
0.8.18 是本轮终包：服务端派发与页面回执均在控制页面前 fail-closed；详情同页缓存的
每条 lane 使用稳定幂等键，停止中的评论采集器必须完成当前原子动作后才可启动下一轮，
批量单篇超时也必须排空旧采集器后再导航。批量目标可输入 1–50，越界明确拒绝而不静默缩小。
媒体序号按 cover/image/video 各自计数，服务端新增七类统一媒体关系和一份
`linggan.media-resource.v1` 读模型；Evidence 只消费受控本地句柄，封面按平台封面、首张正文图、
视频 poster 的唯一顺序选择，3:4 仅作为卡片展示策略。人工媒体下载窗口继续保留。
0.8.20 为“全部楼层回复”增加无进展保护：展开控件点击后如果回复数没有增长且同一控件仍存在，
本次采集会跳过这个停滞控件并继续检查后续楼层，避免深采在一个按钮上无限循环。0.8.19 补齐的
详情作者头像真实媒体链继续保留：`content_detail` 已取得的 `authorId + authorAvatar`
会形成独立 `author.avatar` 槽位，继续走原有候选、下载、Blob 与本地 Materialization，
不把远程头像 URL 直接交给业务页面。Evidence Library 将作品作者与监控目标拆成两个事实区，
作者区显示受控本地头像；媒体 Inspector 同时列出头像、封面、正文图、视频与派生资源。
0.8.21 把无人值守媒体字节执行从随时可能被 Chrome 回收的 MV3 Service Worker 异步尾部，
迁到插件自己的 offscreen 执行页。服务端领取仍先落入同一 IndexedDB outbox，候选域名、20 秒
超时、SHA-256、分块续传、精确工作代次失败回报和最多三次服务端重试全部保留；人工媒体选择与
下载窗口不变。页面关闭或后台 worker 休眠不再把已领取工作静默留成 `lease_expired`。同版还补上
评论无限加载的底部重触发：若已到当前底部、没有结束标记且取得数仍小于页面公开数，采集器最多
三次执行“回拉一屏以内、再按原节奏下滑”的状态驱动动作；每次仍受 1.2 秒最低冷却、暂停/停止和
风险门闸约束，取得新评论后重新计数，不靠无限抖动伪装进展。

0.8.22 修复详情 SSR 先于媒体水合的真实页面结构：正文和互动数仍优先使用结构化记录；当结构化
图片、作者平台身份或作者头像缺失时，才从当前详情容器补齐，并对轮播复制节点按媒体 URL 去重。
媒体选择器限定在详情媒体容器，作者选择器限定在详情作者区，不会把推荐流或评论作者误认成作品
媒体。补齐结果仍只形成 `media_slots`，字节取得继续交给 0.8.21 引入的 offscreen 执行器。

0.8.23 只把非空字符串 URL 视为已水合媒体，SSR 占位对象不会再阻止当前详情 DOM 补齐。0.8.24
让标准详情媒体 lane 与 scheduled 媒体任务共用同一个可靠字节队列：先持久提交媒体槽位，再由
offscreen 执行器下载、哈希、续传和 finalize。0.8.25 允许无凭据、无端口且命中平台白名单的
HTTP CDN 候选在网络请求前升级为 HTTPS；任意 HTTP 与第三方主机仍拒绝，重定向目标仍复验。
0.8.26 让标准详情直接按字节组件入队：普通媒体为 `single`，Live Photo 为 `still` 与 `motion`；
一次离屏唤醒串行处理最多 12 条有界任务，不再让同一详情的剩余媒体依赖全局历史积压。
0.8.27 把离屏媒体执行从采集回执使用的 one-shot `runtime.onMessage` 总线移到命名 Port。
页面提交详情、媒体槽位、评论与回复时，唯一接收方继续是 MV3 Service Worker；Service Worker 只在
需要处理媒体字节时建立 `linggan-media-worker-v1` Port。标准详情各 lane 独立形成失败回执，单一
队列异常不再让后续 lane 静默不执行；浏览器可靠队列的冷启动接纳窗口统一为 15 秒。

0.8.28 完成终态评审发现的两个合同缺口：标准详情的评论图片与作者头像、封面、正文图、视频
共用同一媒体槽位、字节、物化与 Work Resource 读取链，并保留 `comment.image` 的评论主体；评论包
已经可靠入队后，即使回复包单独失败，也只把回复 lane 标为未接纳，不再改写评论 lane 的成功事实。
常规详情读模型有界返回最多 64 个媒体槽位和 256 个派生资源，覆盖作品媒体、头像与 30 条评论窗口。
旧人工媒体选择/下载窗口继续独立保留。

0.8.29 为 Dashboard 缓存中的已选博主补上明确的“同步到观察目标”动作：它只把已有的作者
资料重新放入同一份 Linggan TaskSpec、Attempt、不可变 Package 与 durable outbox，不重新访问平台，
也不直接写目标表。服务端接纳作者 Profile 后按平台稳定作者 ID 创建或字段级补全观察目标；若作者
资料已观察到头像，则另以作者自身的 `author.avatar` 槽位进入同一受控媒体取得、Blob 与本地
Materialization 链。Dashboard 只报告“已进入本机交付队列”，目标出现和头像本地可读仍分别以服务端
Receipt 和媒体物化事实为准。

0.8.30 收口了真实本机验收暴露的派发恢复边界：插件已经领取、但无法打开页面、等待最终
content runtime 或取得一致的页面回执时，会向 Linggan 本机回报一个受限的执行启动失败码。
服务端追加该失败审计后，把**同一份冻结 TaskSpec**恢复为待领取；它不是 Attempt、Capture
Package 或 Evidence，也不记录平台/浏览器原始错误文本。未归位安装被本机工位页认领后，
服务端改为一分钟级重新询问，避免此前 15 分钟退避让“已认领但尚未领活”看起来像断联。

0.8.31 收口了这个重新询问在真实 Chrome MV3 生命周期中的最后一段：服务端给出的下一次
领取节奏不再只创建一次性 alarm，而是创建可在 service worker 被回收后继续唤醒的周期 alarm；
每次轮询仍会用服务端最新节奏替换该 alarm。服务端的 `0`（刚派出任务，立即再问）保留为
Chrome 所允许的最短一分钟，不会被错误退避成五分钟。它不改变工位认领、授权、TaskSpec 或
页面访问门闸；`mayExecute=true` 仍是唯一允许接触平台的条件。

0.8.32 补上真实重载验证揭示的两个更早边界：`onInstalled` / `onStartup` 会先持久化一份
一分钟 bootstrap alarm，再异步报到和领取，避免报到完成后 MV3 worker 被回收时没有任何
后续唤醒；第一次可完成的 patrol 仍会立刻以服务端 cadence 覆盖该 bootstrap。服务端合法
返回的 `nextPollAfterSeconds=0` 也会保留到背景调度层，再由 Chrome 的一分钟下限执行，
而不是把一份已允许的派发判成无效合同。

0.8.33 收口真实作者页派单验收暴露的就绪门：执行端不再要求小红书顶层文档的所有长尾
资源先达到 Chrome `complete`。它仍等待 URL 安静期，并以 `getPageContext` 回应的 URL 与
Chrome 当前 URL 精确一致为唯一页面身份证明；只有这一证明成立，才会把已领取的任务交给
内容采集器。重定向、内容脚本未注入或身份不一致仍在有界超时后走既有 append-only
dispatch-failure 回退，绝不伪造 Attempt、Package、Receipt 或 Evidence。

实况图片仍是一个逻辑媒体卡槽，但静态图与动态图分别携带候选地址、取得工作和状态；
普通图片、封面、视频和实况图片都只把远程 URL 当来源观察，长期展示必须使用 Linggan
本地 Materialization。图片 OCR、视频抽帧 OCR、音频提取与 ASR 由本机 Rust worker 调用
Tesseract、FFmpeg 和本地 Whisper 处理，插件不执行语义分析。

本轮仍不使用 `cookies`、`downloads`、网络规则或通知权限；`alarms` 仅用于 MV3 后台唤醒、
自动签到、受控领取与媒体工作，不成为本地授权来源。平台 host permission 只服务已批准
TaskSpec 的页面执行和受限媒体候选取得。

0.8.34 增加安装凭据的 pending→activate 两阶段交付、账号资格最小信号回报与服务端准入
判定；媒体取得和失败回报也必须携带当前安装凭据。凭据原文只在本机存储，服务端只保留
摘要，账号原始身份只在回报请求内短暂存在并以 keyed digest 入库。

0.8.37 将认领后的 station acceptance 从旧的默认关闭改为默认自动接活，并把人工暂停作为
不可被重装/心跳覆盖的安全状态；check-in 同时回显服务端确认的工位名称和接活状态。它不把
Popup 名称变成身份，也不绕过账号、凭证、风险、配额、并发或 TaskSpec 门禁。

0.8.38 让每个 MV3 `linggan-patrol` alarm 在 claim 前先刷新本机安装心跳；工位已认领且未被
人工暂停时不再因 service worker 空闲被误判失联。只有服务端明确回答账号资格已过期、未绑定或
登录失效时，插件才会向一张**已经打开**的小红书页面请求同一份被动全局导航观察，并对同一 claim
重试一次；不会打开、刷新或导航页面，不读取 Cookie/存储，也不把被查看的博主当成执行账号。固定
作品深化现可冻结为 `detail-only`（评论数为 0），因此不会把“补齐详情”悄然扩大为评论、回复或媒体采集。

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

0.8.17 修复真实小红书页面中回复按钮因亚像素边界被反复判为不可见、导致深采停滞的问题；
0.8.18 将真实终验发现的 `596/594` 收口为完成：全量深采取得数达到或超过页面公开数即可
完成，详情附带评论窗口仍保持精确上限。

0.8.39 修正固定材料详情同页读取的派发回执：首个 `content_detail` Task 在页面读取完成并进入
本机 outbox 后，会回显原派发的 `action`、`capability` 与 `taskId`。后台继续以这三项精确
核验，不会把缺失身份的页面“成功”当成可交付结果；这不改变 WorkOrder 范围、不新增页面访问、
也不改变后续评论、回复或媒体 lane 的独立 claim/Package/Receipt。

发行包生成在 `releases/linggan-intelligence-browser-v0.8.39.zip`。打包器以
固定 ZIP 时间戳和稳定文件顺序生成；`releases/release-manifest.json` 记录已提交
ZIP 的 SHA-256。`npm run verify` 不会改写 release ZIP：它会以新的 `npm ci`、build
和临时 ZIP 重新打包，并要求该 SHA-256 与已提交 ZIP 完全一致，然后运行旧工作台
运行时隔离扫描。

## 未证明事项

本包可构建和可打包；合成回传测试可证明本机 Task/Attempt/Submission 的持久交付与
幂等 receipt。它**不证明**浏览器已加载、真实 XHS 或抖音页面可采、真实内容已入库、
OCR/ASR/研究链已运行。封面是否已本地化以 Evidence 返回的本地物化句柄为准；来源链接
只保留为观察事实，不能作为长期展示保证。
