# PLUGIN-REHOME-001 · 旧插件迁入与运行边界映射

> 状态: 当前迁移与自动执行边界
> 最后核对: 2026-08-31
> 适用范围: `linggan-boom@8a00cc1` / `v2.0.91` 到 Linggan 自有浏览器包的第一条垂直迁入边界
> 事实来源: `plugins/linggan-intelligence-browser/` 当前 source、Manifest、Webpack active entries 和 isolation check
> 冲突时以谁为准: 当前可构建 source 和实际运行证明；旧插件只说明迁入来源，不是 Linggan 运行规格

## 迁入来源与所有权

| 项目 | 事实 |
|---|---|
| 来源 | `Himog0921/linggan-boom@8a00cc1`，tag `v2.0.91` |
| 新所有者 | 本仓库 `plugins/linggan-intelligence-browser/` 的独立 source、lockfile、build 和 release |
| 不允许的关系 | 不链接、导入、调用、发布或回退到旧内容工作台/旧插件仓库 |
| 活跃后台入口 | `src/linggan/background.js`，由 `webpack.config.cjs` 唯一指定 |

## 逐层处置

| 原有能力 | 本轮处置 | 用户现在看到什么 | 当前事实边界 |
|---|---|---|---|
| Popup / Dashboard | 已原样迁入其主要布局、主题、品牌资产、数据页和操作位置 | 熟悉的灵感爆爆爆操作界面，Linggan 本机状态区替代旧工作台连接区 | 界面可构建；没有真实数据证明 |
| 页面注入控制 | 已保留 XHS / 抖音页面识别、按钮位置和提示体验 | 原来的页面控制入口仍在 | 未接通动作显示 Linggan 未接通，不是采集成功 |
| XHS / 抖音 collector source | 完整保留在 `src/platforms/` 和关联 source | 后续可在同一源码包上适配 | 不能在本轮视为已授权、已接通或已验证 |
| 本机 Dexie / 恢复逻辑 | Linggan 专属 outbox 保存待交付 Submission 与媒体上传；service worker 重启后继续同一不可变标识 | 人工与 scheduled Package 均可恢复投递 | outbox 不是 Evidence / 数据库真相；调度事实仍在 Rust/PostgreSQL |
| 旧授权、轮询、lease、工位派发 | 旧 source 仅作历史保留；Webpack 不以旧 background 为入口 | 不再出现旧工作台授权成功或工位就绪承诺 | 当前 active service worker 不调用这些链路 |
| 旧数据同步 / fallback | 已从当前 Dashboard 动作中切断 | “提交到 Linggan（待接通）”只显示原因，不会提交 | 不存在对旧工作台的兼容提交 |
| cookies、媒体下载、定时、网络规则、通知权限 | 从 Manifest 移除 | 不会请求这些旧能力的浏览器权限 | 所有需要这些能力的未来接入必须重新评审 |
| 详情、评论、媒体、批量、抖音、自动化 | 代码与 UI 位置保留，运行动作均等待 adapter | 点击会得到具体未接通说明 | 未访问平台、未下载媒体、未写入 Linggan |

## Issue #78 / #80 的当前执行边界（覆盖上表中旧的 XHS 静态入口描述）

| XHS 能力 | 当前处置 | 不构成的承诺 |
|---|---|---|
| 搜索/博主页发现 | 以有限页面加载、稳定内容去重和目标数为执行边界；记录实际数、轮次、页面加载事实与停止原因 | 不调用旧 API snapshot，不宣称平台全量或调度已接通；不以随机化或规避机制替代页面结束判断 |
| 标准详情 | “采集当前笔记到 Linggan”交付详情、媒体槽位观察和最多 30 条评论；媒体槽位接纳后立即进入与 scheduled 相同的可靠字节队列 | 单一 lane 的接纳不代表全部接纳；该入口不打开人工媒体选择窗口，字节完成仍以 Materialization 回执为准 |
| 人工媒体下载 | “人工采集并下载媒体”保留原有媒体选择窗口和本地下载服务 | 只在人工显式操作后下载；不把人工下载结果写成 Linggan 自动接纳或媒体理解完成 |
| 单篇/批量评论 | 深采可明确设定上限或“公开自然结束”，批量逐篇 checkpoint、空态成功、失败隔离 | 不做内容分析、不绕过验证、不中断时不生成真实 Evidence |
| 本机恢复与媒体 | 使用独立 `LingganIntelligenceBrowserLocalStaging`；媒体仅允许受限 HTTPS 平台域名 | 不复用 `LingganBoomDB`，不允许任意页面消息/本机 URL 作为下载来源 |
| 发现封面自动物化（0.7.0） | 服务端从已接纳 discovery finding 建立 cover slot 与最多三代取得工作；插件自动领取、哈希、分块上传 | 不自动展开详情/评论/OCR/ASR；工作状态不是 Evidence，来源 URL 也不是长期资产 |
| 固定作品深化（0.8.0） | 上游把明确作品集合冻结到 WorkOrder；插件自动顺序执行详情、媒体槽位、评论与回复 | 不对所有发现结果无界 fan-out；普通巡检仍只发现变化 |
| 多样媒体卡槽（0.8.0） | 图片、封面、视频、实况图片进入同一媒体合同；实况 still/motion 分组件候选与有界取得工作 | URL 不是长期资产；一个组件成功不冒充整个实况卡槽完整 |
| 本机派生（0.8.0） | Rust worker 对已物化字节运行 thumbnail/OCR/audio/ASR/frame OCR，空文本写 KNOWN_EMPTY | 插件不承担 OCR/ASR；处理工作不冒充 Evidence 或来源事实 |
| 无人值守页面就绪（0.8.1） | 先订阅 tab 后读取当前状态，既承接未来 complete 事件，也接纳已经 complete 的页面 | 只修复页面就绪竞态；不改任务身份、租约、Attempt 或接纳规则 |
| XHS 签名详情定位（0.8.2） | 服务端从最新已接纳发现记录派发带 `xsec_token` 的短期来源链接；插件校验作品身份后转换为 `/discovery/item/{id}` | token 不进入 TaskSpec、作品身份或 Evidence；缺失时不执行，不回退裸 `/explore` |
| 同工位安装承接（0.8.2） | 新安装取代旧安装时承接其有效租约内尚未完成的同一 Task | 不重建 Task、不重跑完成步骤；已被取代安装不能再改变执行状态 |
| XHS 最终页面稳定（0.8.3） | 首次 `complete` 后继续观察 URL/加载变化；保持 1.5 秒稳定且最终 content script 回应同一 URL 才下发采集动作 | 总等待最多 20 秒；不新建 Task/Attempt，不把页面可响应冒充 Package 或 Receipt |
| XHS 部分详情与同页执行（0.8.4） | 结构化详情已取得内容/作者/媒体时允许互动字段部分未知；首个详情 Task 按服务端已批准范围一次读取详情、媒体候选与有界评论树，后续单能力 Task 复用持久缓存 | 不以未知补 0；不合并 Task/Attempt/Package/Receipt；缓存不授权新 lane、不跨 Lease，缺失或过期时回退原 lane 执行 |
| XHS SSR 详情读取（0.8.5） | 全局 `__INITIAL_STATE__` 被页面水合删除时，从原始页面脚本安全解析序列化 `noteDetailMap`，供详情就绪、完整度和正式采集共用 | 不执行页面脚本、不放宽作品身份校验、不把 SSR 内容冒充 API 或 DOM 观察；无 SSR 时仍走现有运行态/DOM 路径 |
| 评论回执与当前投影（0.8.6） | 标准 30 条窗口与深采分开；短采保留，重试从评论入口重新遍历；服务端按稳定评论身份形成当前投影 | 不跨 Attempt 累加数量冒充完整，不从旧评论游标续采，不修改历史 Package |
| 真实评论树接纳修复（0.8.7） | 顶层自指 root 与回复多关系字段在出包前规范到评论/回复唯一合同；自然结束深采使用无数值配额的有界停止条件 | 不从结果反推任务配额，不放宽服务端关系门闸，不改 scheduled TaskSpec |
| 克制评论执行与暂停交付（0.8.8） | API 分页、DOM 滚动、回复展开每轮三选一；最低冷却、页面稳定、暂停/停止检查共用；暂停提交真实 PARTIAL 评论树；进度区分已取得、页面公开数和请求上限 | 不伪装随机真人轨迹，不把 checkpoint 计数冒充评论材料，不跨 Attempt 累加完整性 |
| 媒体领取代际保护（0.8.8） | 领取后先安排恢复唤醒，再写可靠队列并执行；入队或执行异常按 workRef/claimGeneration 回报 | 不把来源 URL 当本地封面，不让人工媒体窗口消失，不把排队冒充物化完成 |
| 长评论首屏续采（0.8.9） | 不限任务以页面公开数作为当前集合目标；API 首屏不足时进入共用 DOM 补采器；新 Attempt 清空同页旧快照并从空游标重拉第一页 | 不把首屏 `no_progress` 冒充自然结束，不续用上一 Attempt 分页，不跨 Attempt 累加完成度 |
| 新媒体工作优先与超时（0.8.9） | 精确新领取代次优先于旧本地媒体行；候选请求 20 秒超时 | 不绕过三次服务端尝试上限，不删除人工媒体窗口，不把来源 URI 当物化结果 |
| 真实详情公开数续采（0.8.10） | 详情标题无“共 N 条”文案时，终态判定继续使用详情上下文观察到的公开评论数 | 不把 `18/508` 误判为无目标 `no_progress` |
| Attempt 公开数持久（0.8.11） | 采集任一轮观察到的公开数在本 Attempt 内保留，供终态续采判定与部分快照共用 | 不依赖虚拟列表终态时仍保留同一 DOM 容器 |
| 滚动位置进展（0.8.12） | `scrollTop` 能前进或 `scrollHeight` 增长即视为页面加载进展，继续克制下移到新评论或真实底部 | 不把“当前 280px 内没有新节点”误报为 no-progress |
| 单篇全回复参数贯通（0.8.13） | 人工设置的 `allReplies` 与回复上限从单篇任务控制器传入共享评论采集器 | 不把界面已选“全部展开”静默降级成默认两级模式 |
| 平台与插件发行收口（0.8.14） | 合并 Work Resource Read / Evidence Library 主线后，以真实 `time:number` 毫秒 epoch 回归固定 `xhs-detail-time-v2` | 不为已验证映射建立第二分支，不把字段探针写成内容 Package/Receipt |
| 首轮 API 失败转 DOM（0.8.15） | 新 Attempt 的首个 API 页未返回评论时，立即转交共享 DOM 采集器解析、去重、展开和滚动 | 不在 `allReplies` 模式中反复点击同一展开入口，不让页面已有评论时进度永久留在 0 |
| 执行与媒体终包（0.8.18） | 严格派发/页面回执、详情 lane 幂等、评论 stop/restart 互斥、批量 timeout drain、缓存回收、显式批量数量、回复按钮亚像素边界容错、全量深采达到或超过页面公开数即完成，以及统一 MediaResource/七类关系/唯一封面选择 | 不把页面读成接纳，不并发控制同页，不静默缩小目标，不让回复展开困在重复显露循环，不把 `596/594` 误判为失败，不让业务页面绕过本地媒体资源读取远程 URL |
| 详情头像与证据身份分栏（0.8.19） | 详情作者头像形成 `author.avatar` 媒体槽位并进入受控本地副本；Evidence 分开显示作品作者和监控目标，并连续列出全部媒体资源 | 不用监控目标回填作者，不直接展示远程头像，不把头像送入 OCR，不建立第二媒体模型 |
| 楼中楼展开无进展保护（0.8.20） | 区分“已点击”和“页面真的新增回复”；停滞控件在本次采集中只尝试一次，随后继续检查其他楼层并由既有无进展门闸收口 | 不把停滞点击记作采集进展，不从上次评论序号续采，不丢弃已取得的部分数据 |
| 媒体离屏执行与评论底部重触发（0.8.21） | 媒体工作先持久入队，再由扩展 offscreen 文档完成平台候选下载、哈希、续传、finalize 与精确代次失败回报；评论未达公开数且当前底部没有结束标记时，最多三次回拉后继续下滑以重触发平台加载 | 不以页面或 Service Worker 存活冒充执行保证，不改变服务端三次尝试权威，不移除人工媒体窗口；不无限上下抖动、不绕过冷却与风险门闸 |
| 详情媒体自动入队与 Live Photo 接纳修正（0.8.24） | 标准详情与 scheduled 媒体 lane 都在槽位持久提交后立即排入字节执行；同时观察到 still/motion 只证明候选存在，复合槽位保持 `PARTIAL` 直到独立字节链完成 | 不把候选观察冒充本地字节，不移除人工选择/下载窗口，不让一条 Live Photo 约束错误回滚同包其他图片、封面或头像 |
| 受信 CDN 协议规范化（0.8.25） | 仅将无凭据、无端口且命中平台主机白名单的 HTTP 媒体候选升级为 HTTPS，再进入同一离屏字节执行器 | 不允许任意 HTTP，不放宽第三方域名，不跳过重定向后的 HTTPS 与主机复验 |
| 标准详情媒体组件排空（0.8.26） | 普通媒体按 single、Live Photo 按 still/motion 直接进入本次持久队列；一次离屏唤醒串行处理最多 12 条 | 不把两种 Live Photo 字节合并，不依赖无关历史队列才能完成本次详情，不改人工媒体窗口 |
| 采集回执与离屏媒体通道隔离（0.8.27） | 页面采集继续通过 one-shot message 交给 Service Worker；媒体执行改用 `linggan-media-worker-v1` 命名 Port | 离屏文档不再注册通用 onMessage；单 lane 队列失败不阻断后续 lane，人工媒体窗口不变 |
| 评论图片与讨论回执终态收口（0.8.28） | 标准详情把评论图片作为 `comment.image` 送入统一媒体链；comments/replies 分别保留接纳事实 | 不建立第二媒体资产表，不把回复失败改写成评论失败，不移除人工媒体窗口 |
| 手动作者同步到观察目标（0.8.29） | Dashboard 将选中的已有作者资料重投递到同一 Linggan durable outbox；作者头像以 author-owned `media_slots` 进入已有受控媒体链 | 不重新访问平台，不直写目标表，不把进入本机队列说成服务器接纳、目标出现或头像已物化，不回退远程 CDN 头像 |
| 派发启动失败可审计回退（0.8.30） | 已领取但未形成 Attempt 的页面启动失败按受限失败码回报；Linggan 追加审计后将同一冻结 TaskSpec 恢复为 pending；待认领安装被本机认领后一分钟级重新询问 | 不把失败写成 Attempt、Package 或 Evidence，不存原始平台/浏览器报错，不创建替代任务，不放宽工位、授权或平台范围 |
| MV3 派单唤醒持久化（0.8.31） | 服务端节奏驱动的 `linggan-patrol` alarm 同时带有 delay 与 repeat；每次 tick 会用服务端最新 cadence 重新设定，`0` 映射到 Chrome 一分钟下限 | 不将状态查询变成平台动作，不自动认领新安装，不让插件自行设定服务端之外的授权、范围或长期轮询频率 |
| 生命周期 bootstrap 与零 cadence 合同（0.8.32） | install/startup 先写可恢复的一分钟 bootstrap alarm，首次可完成 patrol 再覆盖为服务端节奏；允许服务端派发的 `nextPollAfterSeconds=0` 流向 Chrome 的一分钟下限 | 不让 bootstrap 取代服务端长期 cadence，不把 `0` 当成未授权任务，不直连平台或跳过 `mayExecute=true` |
| 稳定页面身份就绪门（0.8.33） | URL 经安静期后，由同 URL 的内容脚本 `getPageContext` 回应证明可执行；不再等待可能被图片或长连接拖住的浏览器 `complete` | 不跳过重定向或页面身份核验，不把没有内容脚本的页面当作就绪，不改变失败审计或伪造采集交付 |
| 认领默认自动接活与工位名称回显（0.8.37） | 成功 claim 的 server-owned station 默认切为 `stationAccepting=true`；check-in 回显服务端 `display_name` 与接活状态，Popup 只读显示“自动接活 / 已暂停 / 待认领” | 不把名称作为安装身份/配对键，不在插件保存或编辑别名，不以自动接活绕过心跳、凭证、账号、风险、预算、并发或 TaskSpec 门禁；人暂停不得被替换安装覆盖 |
| 自动工位心跳与精确详情范围（0.8.38） | 每个 `linggan-patrol` alarm 先 check-in 再 claim；仅当服务端返回账号事实已过期/未绑定/登录失效，才向已打开 XHS 页请求同一被动导航观察并重试一次；固定材料可声明 detail-only | 不打开、刷新或导航 XHS 页面；不读 Cookie/存储，不把所查看博主当账号；不因“详情”自动采评论、回复或媒体 |
| 详情页派发回执身份（0.8.39） | 首个同页 `content_detail` 读取入本机可靠 outbox 后，回显原派发的 action、capability、taskId；后台保持严格比对后才继续交付 | 不因一个页面读取合并后续 lane；不将缺身份回执当作成功；不扩大 WorkOrder、页面访问或采集范围 |
| 持久账号绑定与任务页观察（0.8.46） | 账号 binding 不再依赖日历 expiry；严格嵌套 observation 合同将正向身份与显式负面事实分开。自然页面只读一次已渲染 DOM；已领取任务在其本来打开的页面复核后才采集。启动 selector probe 仅留诊断 | 不扫描标签、不为账号打开/刷新/滚动/切换页面、不调平台 API；缺失 DOM或本机回传失败不写/不阻断；负面仅能来自平台状态组件；服务端确认的负面或账号变化才中止当前任务；不把启动诊断误报为 selector blocked |

## 新旧运行路径对照

```text
旧路径（本轮禁止）
Popup / injected control
  -> old background
  -> old authorization / station / lease / polling
  -> 内容工作台 endpoint / sync / fallback

当前路径（0.8.46）
Popup / Dashboard / injected control
  -> Linggan adapter boundary
  -> scheduled 或 manual TaskSpec / Attempt / durable Submission outbox
  -> localhost receipt / retry / Evidence Material Projection

手动作者资料路径
Dashboard selected cached author
  -> same manual TaskSpec / Attempt / durable Submission outbox
  -> accepted author Profile -> stable observation target
  -> author.avatar media slot -> local Materialization -> local media URL

自动观察路径
Observation rule -> WorkOrder -> ordered single-capability steps
  -> first signed detail page read -> lease-scoped persistent page cache
  -> each claimed lane -> its own immutable Package -> Receipt
  -> durable media outbox -> offscreen bytes transfer -> local processors -> Evidence Library
```

## 发布前可验证项与未验证项

本卡的自动验证应证明：完整 source 与 UI entrypoints/assets 存在、可构建、可生成稳定
release ZIP，新的 `npm ci/build/package` 与已提交 ZIP SHA-256 一致，active content
bundle 不包含旧工作台 host/endpoint/运行时模块（lease、heartbeat、outbox、polling），
且 Manifest 不保留不应启用的旧权限。

自动验证不能证明：浏览器实际加载、真实 XHS/抖音页面兼容、账号可用、真实采集、媒体
保存、本地封面、OCR/ASR、Linggan 数据库接纳、Evidence Library 展示或研究结论。
