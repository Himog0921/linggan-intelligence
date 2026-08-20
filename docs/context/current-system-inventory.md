# 当前系统资产盘点

> 状态: 代码事实优先
> 最后核对: 2026-08-20
> 适用范围: 现役来源固定点与可复用能力盘点
> 事实来源: 固定源码、fixture、测试与来源证明
> 冲突时以谁为准: 固定点真实源码、fixture 和可复现测试结果

固定来源盘点日期：2026-08-14（Asia/Shanghai）；本次源码与测试复核日期：2026-08-20。

## 现役来源

| 系统 | 固定点 | 用途 |
|---|---|---|
| 内容工作台 | `2dca6cb9b08cd217d851aef845ea462c19289ef7` | V2 内核、数据库合同和集成证明的参考来源 |
| 浏览器插件 | `60a896c1def5062dbb8098e05b030c5a0871203b` | v2.0.95 真实运行能力与采集事实来源 |

## 已验证、应保留的能力

插件侧：工位注册、授权、心跳、任务领取、租约、本地队列、重试、死信、幂等提交、终态报告、XHS 页面采集与生产合同。

服务端：CapturePackage、EvidenceIngressReceipt、RawSnapshot/RawRecord、运行时校验、canonical JSON/hash、受控 Evidence reader、访问审计、durable work、B2 evaluation、CanonicalObservation、B3 Content Projection、并发 fencing、原文链接和 observed media 合同。

### 固定 V2 小红书 producer 能力矩阵

下表只陈述固定源码、fixture 与本次可复现测试能够证明的范围。它不是 2026-08-20 真实小红书页面探查，不证明平台当前结构、账号间可比性、风控阈值、完整 Coverage 或新项目合同已经成立。

| 能力 | 固定源码与测试证据 | 已证明的边界 | 没有证明、必须后续处理 |
|---|---|---|---|
| 搜索/API 发现 | `src/injected/xhsApiCapture.js`、`src/platforms/xhs/noteCollector.js`、`src/workbench/runtime/monitorTask.js`；`xhs-note-discovery`、`monitor-author-surface-scan` 测试源码 | 能截获搜索结果与作者作品接口快照；搜索/作者表层记录可保留 note ID、链接、标题、作者、表层指标、封面候选、发现顺序和入口；发现结果可去重并带停止摘要 | 本次 2 个相关测试文件因归档缺少 `dexie` 未能加载；API 返回与页面实际可见、字段来源优先、平台排序、跨账号差异和真实终止仍为 `SOURCE_INCOMPLETE`。表层记录不是详情 Observation，发现 N 条不授权 N 次深采 |
| 作者资料与作品索引 | `src/platforms/xhs/authorCollector.js`、`noteCollector.js`；`workbench-author-remote-run`、作者表层扫描测试源码 | 作者采集能够从结构化页面数据和页面回退中取得平台身份、名称、简介、位置、粉丝/关注/互动及来源原始材料；作者作品索引可从 `user_posted` 快照/接口或页面发现并去重 | 昵称不能代替身份；字段精度和账号状态需要真实 producer 对账。旧 `author baseline` 仍可能继续进入批量详情采集，不能当成新项目已经实现“作者索引后再逐项准入” |
| 笔记详情 | `src/platforms/xhs/noteCollector.js`；`xhs-note-collector`、`workbench-xhs-batch-run-helper` 测试源码 | 详情路径检查目标 note ID，并能取得标题、正文、标签、类型、作者、可见互动、发布时间表达和图片/视频/实况候选；已知公开评论数为零与未知可在辅助逻辑中区分 | 两个相关测试文件本次因缺 `dexie` 未能加载。旧 IndexedDB upsert 不能移植为 Evidence/Observation 历史；详情字段来源、编辑检测、时间精度和“完整详情”必须重审 |
| 评论与回复 | `src/platforms/xhs/commentApi.js`、评论采集/展开代码；`xhs-comment-api`、`xhs-comment-collector-state`、`xhs-comment-expand` | 评论 API 测试证明主评论/回复分页解析、cursor/`hasMore`、主从关系、每根回复上限和 xsec token 传递；API 记录可以保留平台 comment ID | DOM 回退在缺少平台 ID 时会用“作者名 + 前 50 字正文”合成 `commentId`，而固定 source contract 只检查非空，因此合成身份仍能通过终态合同。默认最大主/子分页、DOM/API 回退和停止方式也不能证明全部评论已取得；此类记录只能保留为未解析材料，不能创建稳定 Comment Source Object 或有效 Observation |
| 指标 | 表层与详情 note、author 记录中的互动/粉丝字段；`xhs-collection-contracts.test.cjs` | V2 可以把采集时可见的数字嵌在 note/author payload 中；六个固定 XHS 合同明确不包含 `metric` RawRecord | 没有独立 Metric Observation、指标来源优先、生命周期时间点或行动 Outcome 同源合同。旧字段存在不能证明指标链路完成，也不能据此决定新项目“不需要指标模型” |
| 媒体 | `src/workbench/protocol/v2/xhs-source-contract.cjs`、`xhs-terminal-mapper.cjs`；source-contract、terminal-mapper、video-stream 测试 | 当前媒体作为 `media_inventory` Artifact，以 note subject、purpose、kind、稳定 slot、observed address 和封面来源表达；测试证明使用实际本轮媒体、拒绝跨主体/歧义封面/作者媒体/评论媒体 | 不存在 media RawRecord；没有证明文件真实下载、字节完整、对象存储、转码或评论/作者媒体 producer。Artifact 形态不能提前决定新项目 Media 是否拥有独立来源身份 |
| Coverage 与终态 | `src/workbench/runtime/xhsCaptureReport.js`；terminal-mapper 与 batch helper 测试 | 六类 profile、必需 slot、requested/discovered/emitted/deduplicated/failed 计数及七类已审计终态原因可以 fail closed；零结果不会自动推断 absent；缺 capture report 会拒绝映射 | 这是工作流级粗粒度 Coverage，不是逐页/cursor 完整性。成功报告只接受 observed/not_applicable；失败统一 invalid；comment batch helper 固定写 `target_reached`，可能高估真实来源终止，必须在 Gate 4 攻击 |
| 结果打包与 V2 映射 | `src/workbench/runtime/resultPackager.js`、`src/workbench/protocol/v2/xhs-terminal-mapper.cjs`；六合同、source、terminal 测试 | 打包器按 collectionRunId 汇总本地 note/comment/author/media 与 capture report；映射器固定服务端 capture/plan/target/job/lease/collector 身份、严格 record kind、canonical/hash、幂等键和终态；合同测试证明无 metric/media recordKind | 打包的是已落本地 store 的规范化记录，不是被截获的完整原始响应/页面本身。映射器用 run `finishedAt` 作为 record `observedAt`，不能证明真实观察时间；固定 V2 没有证明新项目的 Attempt/重试/checkpoint/Package 关系，该关系已在 Gate 4 经审计和用户确认后另行收口，不能从旧 mapper 自动推出 |
| 风控、暂停与恢复 | `src/platforms/xhs/batchController.js`、`batchCommentController.js`、批量启动/运行辅助测试 | 固定源码存在验证码/风控检测、暂停、恢复、停止、checkpoint、远程 run 绑定和 finalizer；启动失败测试证明不能伪装异步派发成功 | 没有真实账号流量实验，不能得出安全访问频率、批量、休息时长或可恢复成功率。`pause/resume` 存在不等于平台风险受控，也不等于恢复后 Evidence/Coverage 完整 |
| 时间语义 | API 快照 `capturedAt`、表层 `observedAt`、详情/评论时间解析与 terminal 映射 | 固定代码能保存部分原始 payload，并将多种时间表达解析成字段；相对时间 parser 有历史测试 | 搜索/表层常用本机当前时间，terminal 又使用 `finishedAt`；“N 天/月前”等解析可能丢失原始精度，客户端时钟资格没有闭环。不得将这些值直接用于 Current 或正式趋势 |

### Gate 4 开场：六类固定合同的证明资格

以下表格把“代码里存在合同”进一步拆成“合同究竟证明了什么”。它是 Gate 4 的第一份来源底稿，仍不是新项目合同。证明状态统一使用三档：

- **已证明：** 固定源码与已实际运行的断言共同支持，可以作为后续设计输入，但仍只代表固定版本。
- **源码暗示：** 实现路径存在，但本次相关测试未加载、缺少真实 producer 对账，或合同只验证了形状，不能升级为运行事实。
- **`SOURCE_INCOMPLETE`：** 必须通过受控 fixture 环境或最小真实 producer 实验取得新证据；未完成前只能保持未知或限制用途。

| 固定合同 | Record / slot / media 约束 | 当前可证明 | 明确不能推出 |
|---|---|---|---|
| `xhs.list-scan` | `note`；`note_list` required；`metadata_only` | 固定合同和 fixture 锁定该形状，允许零 Record，并把媒体候选作为 inventory Artifact 而非 media Record | 不能推出搜索页面完整、API 与页面一致、每条 Note 已取得详情，或零 Record 表示没有相关内容 |
| `xhs.note-detail` | `note, comment`；`note` required、`comments` conditional；`metadata_only` | 合同允许“详情有 Note、评论按计划决定是否观察”；Record 类型与 slot 可被严格核对 | 不能推出正文/指标/时间字段真实完整，也不能把 comments conditional 解释成已取得全部评论 |
| `xhs.note-full` | `note, comment`；`note` 与 `comments` required；`metadata_only` | 合同要求两个 slot 都有明确报告，缺报告会 fail closed | required 只证明 producer 必须交代该 slot，不证明分页穷尽、回复完整、页面未折叠或平台没有更多评论 |
| `xhs.comment-probe` | `comment`；`comments` required；media not required | 合同只允许稳定 `noteId + commentId` 的评论 Record，能够表达独立评论采集 | 不能推出评论者真实身份、评论代表性、全部回复、编辑/删除状态或评论媒体已审计 |
| `xhs.author-profile` | `author, note`；`author` required、`note_list` optional；`metadata_only` | 作者与表层 Note 可以在一次交付中分开记录；Author 最低身份必须满足 `authorId = platformAuthorId` | 不能用昵称建身份，不能推出作品索引完整，也不能把 optional note list 变成自动详情深采授权 |
| `xhs.author-links` | `note`；`note_links` required；media not required | 固定合同表达“先取得作者作品链接/表层 Note”这一低成本发现能力 | 不能推出这些链接都有效、全部属于目标作者、已打开详情或应该立即批量深采 |

跨六类合同共同成立的最低边界：

1. Note 只验证 `noteId` 与 `platformContentId` 非空且相等，并要求 `type` 为 `normal` 或 `video`；Author 只验证 `authorId` 与 `platformAuthorId` 非空且相等；Comment 只验证 `noteId + commentId` 非空。当前 source contract **没有逐字段证明**标题、正文、作者关系、发布时间、指标、评论文本或页面位置的来源正确性。
2. 六类合同都允许零 Record。零结果只有在 producer 报告、slot、Coverage 和观察资格成立时才能描述“本轮没有发出 Record”，绝不能转写成现实不存在。
3. `captureId`、`jobId`、`attemptId`、`leaseEpoch`、`executionPlanVersion`、预期目标、collector 版本、合同版本/hash、包 canonical/hash 和运行起止时间被固定 mapper 要求；这证明固定 V2 有严格交付头，不证明新项目应照搬字段或一个 Attempt 只能产生一个 Package。
4. 固定 mapper 把运行 `finishedAt` 同时用作包头和所有 Record 的 `observedAt`。因此其格式可以通过合同，语义却不足以证明每条 Record 的真实观察时刻；Gate 4 必须把“通过形状验证”与“来源时间有资格”分开。
5. slot、五个计数和终态必须由 producer 显式报告，映射器不会从 Record 数量猜终态；但 producer 自己仍可能错误报告，例如评论批处理固定写入 `target_reached`。所以“报告存在并通过格式验证”不等于“终止原因真实”。
6. 媒体 inventory 只接受 Note 主体、稳定 slot、用途、种类、顺序、观察到的地址和封面来源；它不证明文件下载、字节完整、长期可访问、对象存储、转码或作者/评论媒体已经支持。

由此得到 Gate 4 的第一个硬结论：**V2 固定合同可以作为失败关闭、身份最低线、交付完整性和来源缺口的攻击样本，不能直接晋级为新项目 producer 合同。** 下一步必须把每个 lane 的实际字段来源、观察时刻、终止依据和失败后果继续对到账户/页面/API/fixture；缺少新证据的地方保持 `SOURCE_INCOMPLETE`。

### Gate 4 第二项：Attempt、恢复、checkpoint 与 Package 的真实关系

本节只陈述固定 V2 源码、schema 与已存在测试能够支持的关系；它不把旧实现自动升级为新项目合同。

| 关系 | 固定 V2 当前可证明 | 不能据此推出 / 已发现缺口 |
|---|---|---|
| Execution Job → Task Attempt | 同一个服务端执行任务可以经历多次 Attempt；每次 `startJob` 创建新的 Attempt、`captureId`、lease token 和递增的 lease epoch。普通续租保持同一 Attempt；租约过期会把旧 Attempt 标为 `stale_result`，重新执行时创建新 Attempt | 不能把“同一任务恢复”解释成“同一 Attempt 永久延续”，也不能用任务 ID 代替 Attempt 或 Capture 身份 |
| Attempt → 插件本地 Collection Run | 插件能按外部任务 ID 找回未终止的本地 `CollectionRun`；任务轮询器会拒绝明显属于旧 Attempt 的持久上下文 | 另一个已测试分支会在 `pluginRunId` 相同时保留旧执行页和本地 run，同时接纳新的服务端 Attempt。因而当前 V2 **没有证明** Attempt 与 Collection Run 一一对应；旧 Attempt 的未接纳记录和 checkpoint 可能被新的 Attempt/capture 身份继续打包，这是来源漂移风险 |
| 传输重试与 Attempt 重试 | 严格 outbox 在产生记录时冻结 `attemptId`、lease token 与 lease epoch；网络重传复用同一条出站记录，服务端接受完全相同的幂等重放，不借用更新后的租约 | 这只证明传输重试可以保持原身份，不证明执行重试可以复用旧 Attempt 的未接纳材料。两类重试必须分开 |
| 增量消息与终态 Package | 非终态 envelope 携带 records 会被服务端拒绝为 `v2_terminal_submission_pending`；当前终态路径才组装并提交 V2 `captureSubmissionV2`。`resultPackager` 按 `collectionRunId` 汇总本地 store 中的 note/comment/author/media | 本地 outbox 的传输批次不是 Capture Package 分块；当前也没有证明部分 Evidence Package 流式接入。打包对象是规范化本地记录，不是每次平台原始响应 |
| checkpoint 与目标集合 | checkpoint 保存 `targetIds`、`nextIndex`、`processedCount` 和结果状态；恢复时优先保留仍能在本轮重新发现的旧目标顺序，再追加新目标，以减少部分重复访问 | 旧目标如果本轮未再发现会被移除，新发现目标会被加入；因此 checkpoint 没有冻结一次观察的不可变成员集合，也不能证明 Coverage、可比范围或“没有更多”。部分 checkpoint 写入失败会被调用方吞掉，重复访问仍可能发生 |
| 插件本地 capture journal | journal 以 `entryId` 追加并计算 payload hash；同 ID 再写不会覆盖已有行 | 同 ID 不同比特不会比较 hash 或产生冲突；调用链中的 journal 写入失败可以不阻断 outbox。它是本地恢复辅助，不是最终幂等、防冲突或每条事实已持久保存的证明，也不是 Accepted Evidence |
| Package 接入 | 服务端在单个 `SERIALIZABLE` 事务中写入 Package、RawSnapshot、全部 RawRecord、Artifact、receipt 和 verified package 的 durable work；数据库异常会整包回滚 | 包级接入原子不等于下游每条 Record 都成功，也不等于执行控制已同步完成 |
| 重放、身份冲突与执行控制 | 同 workspace、`captureId`、hash 的完全相同提交是重放，不重复写 Record；同 `captureId` 不同 hash 会保存 `capture_identity_conflict` 材料、返回冲突且不创建下游 work。Evidence 提交后，执行状态另用一次比较并更新；失败时明确返回 `evidence_committed_control_pending`，重放可继续完成控制推进 | 冲突材料不等于 Accepted Evidence。Evidence 已提交与任务状态完成是两个事实，任何页面或 Agent 都不能只看后者判断链路成功 |
| authority 与 Evidence 事务 | 接入前会严格验证 Attempt、lease、目标、合同、canonical 和 hash；下游 worker 已有事务 fence 模式 | 当前 Evidence Ingress 的 authority 检查发生在 Evidence 写事务之前，写事务本身没有使用同类 fence。源码与现有测试没有覆盖“验证通过后、提交前租约/控制状态刚好变化”的交错，标记 `SOURCE_INCOMPLETE`；新项目必须在同一事务或等价原子条件下失败关闭 |
| 下游 Record/Object 隔离 | verified Package 只创建一个 package 级 durable work；B2 会把预期的单条 `rejected` / `quarantined` 结果逐条保存并继续处理 | 全部 Record 仍在一个派生事务中处理；任一未预期异常会回滚整包派生并触发 package 级重试/死信。V2 只部分证明 Record 隔离，**没有完整证明**任意坏 Record 都不会阻塞其他合格 Record |

基于上述来源，Gate 4 给新项目的最小关系已于 2026-08-20 获用户确认，并进入共同语言和领域不变量。这里确认的是语义关系与实现硬约束，不是物理表、字段、队列或服务拆分：

1. 一个 Work Order 可以有多次 Execution Attempt；每次 Attempt 由服务端分配独立 Capture Identity。
2. 第一版采用一个 Attempt 最多提交一个终态 Capture Package；没有成功交卷时为零个。复杂任务通过有界 Work Order 拆分，不先引入可变多包协议。
3. 同 Attempt、同 Capture Identity、完全相同字节的网络重传是 replay，不产生新 Observation；相同身份、不同内容必须形成显式冲突并停止接纳。
4. 同 Attempt 内可以用 operational checkpoint 恢复，但 checkpoint 只是可变运行状态，不是 Evidence、Coverage 或观察成员清单。
5. 新 Attempt 必须获得新 Capture Identity；不得把旧 Attempt 尚未接纳的本地记录或 checkpoint 静默改挂到新身份。确需挽救时走独立 recovery/import 路径并保留原来源。
6. 已接纳 Evidence 可以按用途复用，不需要重新访问平台；这与复用旧本地未接纳材料是两件事。
7. 第一版不把 outbox 传输批次建模为 Package 分块。未来若真实成本要求部分交付，每个分块必须冻结成员并拥有独立服务端身份，不能反复改变同一 `captureId` 的 hash。
8. Package 接入继续保持全包原子；接纳后的来源身份解析和派生处理按 Record / Source Object 隔离。物理隔离方式和事务边界进入 Gate 5，不从 V2 的单一派生事务照搬。
9. authority 有效性与 Evidence 写入必须共享事务 fence 或等价原子条件；“先查有效、稍后写入”不能成为新项目的最终实现。

### Gate 4 第三项：逐 lane producer、字段与 Coverage 对账

本节继续向下核对固定 V2 的实际字段生产路径。它区分“平台/API/页面确实提供”“旧代码解析或默认”“终态合同只检查形状”三层，防止把规范化后的漂亮字段误当成已证明来源事实。全程没有访问真实小红书，也没有修改受保护的 `references/`。

| Lane | 固定 V2 实际路径与可保留经验 | 已发现的来源/语义漏洞 | 新项目当前处理 |
|---|---|---|---|
| 搜索发现 | 搜索 API 卡片和页面卡片都能发现 note ID、链接、标题、作者提示、封面候选、部分互动与顺序；页面发现与详情访问在代码上已有 `surfaceOnly` 分支，可作为“先发现、后准入”的参考 | 搜索 API 快照不保存 cursor、`hasMore`、排序、筛选、账号或工位身份；空结果响应直接不入快照；内存只留最近 12 份。API 与 DOM 被合并、去重后重编号，无法重建每个页面/入口的原始位置。无类型卡片会被表层 builder 默认成 `normal`；点赞、收藏、分享缺失可能变成 0。`noteStore` 按 note ID 覆盖，多个关键词/作者入口发现同一笔记时只剩最后一行，违反“对象去重但发现事件保留” | 搜索结果必须先成为独立 Discovery Capture 与 Discovery Event；原始页/响应、入口、筛选、顺序、分页边界、账号/工位和空结果资格需要单独合同。未知类型和未知指标保持 unknown，不能补成正常图文或零。发现不得自动授权详情深采 |
| 作者资料 | 作者平台身份从主页 URL 取得；结构化 `userPageData` 与页面字段混合提供名称、简介、位置、头像、粉丝/关注/互动等；作者作品接口快照比搜索快照多保存 user ID、cursor、`hasMore` | 字段没有逐项来源声明；结构化原料和 DOM 文本经 `createCollectorEvidence` 序列化后最多保留 6000 字，不是完整原始页面/API。缺失性别会变成 0，缺失关注状态会变成 false，缺失/真实为零的互动数也会合并。`authorStore` 按 user ID 覆盖历史。直接 profile API fallback 只请求一次；旧 author baseline 仍会从作品列表继续批量打开详情 | 作者身份、资料 Observation 与作者作品 Discovery 分开。未知资料不补默认值；作品索引终止必须保留 cursor/`hasMore`/页级 Coverage。新项目只保留“作者链接发现”能力，不继承旧 baseline 自动放大为详情访问 |
| 笔记详情 | 详情采集会校验实际 note ID 与目标一致，并要求互动字段存在及正文、媒体或作者至少一类可用；可取得正文、标签、作者关系、互动、发布时间表达和媒体候选 | 数据主要来自当前 `__INITIAL_STATE__.noteMap`/注入状态；所谓 raw payload 仍被截断到 6000 字。点赞/收藏/分享缺失可落为 0；只有公开评论数额外保留 known 标记。相对发布时间按插件本机时钟换算，月按 30 天、年按 365 天近似；解析实际使用的字段不一定被 `publishedAtText` 原样保留。结果继续覆盖同 note ID 的本地当前行 | 目标匹配与最低可用性检查可参考；详情原料、字段断言、解析版本与观察时间必须分层保存。缺失指标、近似时间和作者关系只能带限定进入 Observation；本地 current row 不得充当 Evidence 历史 |
| 评论与回复 | API 快照按 note/root comment 保存 endpoint、cursor、`hasMore`、comments、URL 和 capturedAt；API 映射能保留主从、回复对象、作者提示、文本和时间 | `readHasMore` 在来源字段缺失时返回 false，未知与明确无下一页合并；默认最多 6 个主评论页、每根最多 6 个回复页，并叠加总量、稳定无新增、页面末尾和展开次数等启发式停止。DOM 缺 ID 时合成“作者_前50字”，会碰撞、随编辑变化且可被固定合同接受。评论批处理无论取得多少都写 `target_reached`，并把“目标笔记数”作为 requested、“评论 Record 数”作为 emitted，计数单位混用；0 条也可被标成 comments observed | API 平台 ID 才能直接建立 Comment Source Object；合成 ID 只允许进入 unresolved material，不能满足身份合同。`hasMore` 必须三态；每页/每根回复 Coverage、停止依据和计数单位必须显式。零条只能陈述本次发出 0 条，不能证明无评论 |
| 指标 | 搜索卡片、详情与作者资料能保存某次可见互动数字 | 没有独立 Metric producer/Record；多数字段缺失会归零，字段来源与读取时刻未逐项固定。笔记发布后生命周期观察与页面指标来自不同运行，旧 current-row upsert 会覆盖前值 | 指标必须是带对象、指标名、原始表示、解析值、来源位置和观察时刻的时间性断言；缺失与零分开。是否建独立物理表留到 Gate 5，不由旧 payload 形状预判 |
| 媒体 | V2 terminal mapper 会把本轮 note 媒体候选转成稳定 subject/slot/purpose/kind/ordinal/observed address，并拒绝跨主体和歧义封面 | inventory 证明的只是“当时看到了这个地址”，不证明下载成功、字节完整、内容 hash、长期可访问或对象存储成功；固定合同只支持 note 主体，评论图片和作者头像会被拒绝。旧 media store 仍是按 `assetId` 的当前行，规范化还会给缺失 kind/role 写默认值 | 第一阶段先保留 observed media assertion 与下载后 blob/file proof 的分层；未下载地址不能冒充已保存媒体。评论/作者媒体继续 `SOURCE_INCOMPLETE`，不从旧下载工具反推 producer 已成立 |
| 风控与有限工位 | 固定插件有每次最多 50、随机间隔、分段降速、验证码检测、暂停/继续/停止和单篇超时；这些能减少一次任务失控 | 参数是静态经验值，没有真实账号实验支撑；没有跨工位全局访问预算、账号冷却/健康状态、任务机会成本、同对象并发合并或风险后自动降级策略。暂停后继续也不自动证明前后 Coverage 可拼接 | 风控是 Admission 与调度事实，不是插件局部延迟函数。新项目必须按平台、账号、工位和时间窗分配预算；风险事件结束当前 Attempt 或显式形成新的恢复边界，不得静默宣布完整成功 |
| 时间与终态 | producer 内部存在 API `capturedAt`、表层 `observedAt`、详情/评论 `collectedAt` 和发布时刻解析；terminal mapper 要求 run 起止时间 | 搜索表层常用当前本机时间，详情相对时间也依赖本机时钟，terminal mapper 又把 run `finishedAt` 复制给包和全部 Record 作为 `observedAt`。`captured_partial`、`api_partial`、`max_rounds_reached`、`stable_no_new` 等都会被粗映射为 `source_exhausted`，把启发式停止包装成来源耗尽 | 至少分开 published/discovered/observed/ingested 四种时间；每条记录使用真实观察时刻或明确 unknown，不用任务完成时刻回填。启发式停止只能报告自身原因，不能升级成平台已耗尽 |

#### Gate 4 第三项的 P0 结论

1. **最低身份合同不能只检查“非空”。** 必须同时证明身份来自哪个 producer 字段；合成 ID、昵称、正文 hash 或数组序号不得伪装成平台稳定身份。
2. **未知不能用业务默认值补齐。** `type=normal`、指标 `0`、`hasMore=false`、性别 `0`、未关注 `false` 只有在来源明确给出时才能成立；否则保持 unknown，并限制可用 Claim。
3. **发现对象与发现事件必须分别保存。** 同一 Note 可以只有一个 Source Object，但每次由不同关键词、作者、页面位置或研究路径发现都要保留，不能由 current-row upsert 覆盖。
4. **所有 Coverage 计数必须先固定单位。** 目标笔记数、发现卡片数、详情成功数、评论 Record 数、唯一评论数、页面数和回复根数不能塞进同一 requested/emitted 比较。
5. **启发式停止不等于来源耗尽。** 达到本轮上限、连续无新增、内存只剩最近页面、页面显示末尾、API 未给 `hasMore` 和真实平台无更多是不同事实。
6. **原始材料必须真实可重放。** 被截断的 JSON、拼接 DOM 文本和规范化 current row 可以辅助诊断，但不能替代完整 Capture Artifact；解析/默认/AI 结果必须与原料分层。
7. **真实工位访问只能由服务端控制平面准入。** 发现、详情、评论、回复和媒体是不同成本 lane；插件只能执行有界 Work Order，不能因为发现 N 个对象就自行展开 N 次深采。

以上七条目前是由固定源码缺口反推的 Gate 4 **设计约束候选**，尚未写入已确认领域不变量。Attempt/Package 最小关系已经单独确认；这七条 producer 约束仍需在 Gate 4 实验包与未执行冻结规则完成后由用户整体确认。

### Gate 4 尚缺的最小真实 producer 证据

源码审计已经足以否决直接照搬旧合同，但仍不能证明 2026-08-20 的真实平台行为。进入真实访问前应先形成有上限、可停止的实验单，至少覆盖：

1. 同一关键词、同一筛选下，一次搜索实际可见卡片、接口页、顺序、cursor/`hasMore`、空结果和页面终止分别能拿到什么；至少比较两个观察身份，但不得据此反推全平台代表性。
2. 一个脱敏作者主页的资料字段与作品索引逐项来源，验证 cursor/页数/终止以及“只拿链接、不打开详情”的低成本链路。
3. 少量不同年龄笔记的详情字段、发布时间原文、互动字段和媒体候选，对照首次发现与真实观察时刻。
4. 少量评论线程的主评论页、回复页、空评论、删除/不可见、缺 ID 和停止边界；必须确认平台稳定 ID 是否始终可得。
5. 同一小批笔记在预先确定的生命周期节点复观测，验证互动与新增评论的变化；“7 天”只作为待测策略，不作为硬编码事实。
6. 每个实验明确账号、工位、访问上限、最长时间、验证码/异常停止、原始材料保存与脱敏处置；未获授权前不执行。

### 本次历史测试复现边界

2026-08-20 运行 13 个定向测试文件，共发现 210 项：204 项通过，6 个测试文件在加载阶段因归档快照未包含 `dexie` 依赖而失败。通过部分覆盖六个 V2 合同与 fixture、canonical/hash、source contract、result packager、terminal mapper、评论 API 分页、视频流选择和远程批量启动失败语义；未加载文件覆盖作者表层扫描、笔记发现/详情、页面评论继续采集/展开和部分批量 run helper。

这 6 项不是业务断言失败，也不能被报告为通过。由于 `references/` 受来源 checksum 保护，本次没有在归档目录安装依赖或修改快照；对应能力继续按源码证据陈述，并在 Gate 4 通过受控依赖环境、脱敏 fixture 和真实小流量实验重新验证。

V2 B3 已通过实现和集成测试证明：历史 `ContentObservation` 与 `ContentCurrentProjection` 分离；较旧 `observedAt` 不能推进较新的 Current；相同 `observedAt` 对应不同 Observation 时 fail closed。这个证明只覆盖“按 observedAt 严格前进”的历史机制，不证明时间语义已经完整：V2 Evidence Ingress 仍直接采用包头/Record 的 `observedAt`，schema 预留的 `clientObservedAt`、`clockSkewSeconds`、`observedAtSource` 在当前接入写入中为空；Current 仍是整条 ContentObservation 指针，也没有证明 API/页面差异、迟到更强来源陈述和 parser 修订应如何裁定。因此新项目保留历史不可覆盖和防倒退原则，不照搬“最大 observedAt 决定全部 Current”的语义。

旧 Prisma 审计还显示历史产品存在 `ContentTopicTag` 与 `TopicPerformanceRollup`：后者保存 `sampleCount`、`viralCount`、`medianLikeCount`、`p90LikeCount`、`topExampleKeys`、`rollupVersion` 和计算时间。这只能证明旧系统设计过 Topic 表现汇总和分析版本字段；`topExampleKeys` 只是 JSON 内部引用，schema 本身没有证明标题、作者、原文、真实用户评论可读，也没有证明库存与趋势、观察可比性、市场验证或 Topic 下钻形成闭环。旧 Prisma schema 继续只作来源与依赖审计，不成为新数据库设计。

旧 Prisma schema 还定义过 `OperationPublication`，包含发布链接、平台内容 ID、浏览/点赞/收藏/评论、`leadCount` 和同步状态等字段；但在固定参考源码 `src/` 中未找到对应模型的实际调用链。更关键的是，V2 模型合同明确把 `ContentMetricSnapshot/Latest` 排除在第一条 XHS 切片之外，metric Raw Record 来源和同源关系仍为开放阻塞。因此当前只能确认“旧系统设计过发布与指标字段”，不能确认自己的内容发布、公开指标、深度评论、私信、咨询或销售已经形成可信 Outcome 链。

## 不能继承为“已完成”的能力

- Author、Comment、Metric 的完整新领域模型。
- Observation Plan 与多周期观察。
- Coverage/缺失事实闭环。
- 真实 observed media 的稳定端到端生产覆盖。
- Feature、Cluster、Signal、Intelligence Event、Outcome。
- Agent 推理审计与结果反馈闭环。
- 用户原声、内容叙事、Topic 长期历史、可比变化、可读代表样本与 Evidence 下钻组成的 Topic 探索闭环。
- 库内累计、时间窗口新增和可比观察面占比的可靠分离，以及爆款表现与需求/机会判断的可靠分离。
- 跨账号、工位、时间、搜索入口、Coverage、平台排序和分析版本均可解释的正式趋势链；当前搜索发现面仍只能作为单次观察来源，不能继承为趋势能力。

## 旧系统规模与迁移结论

- 旧工作台目录约 75GB，其中 `storage/local-blob` 约 68GB。
- 旧 PostgreSQL 数据库约 5.5GB；现有压缩 dump 约 658MB。
- 旧 schema 约 166 个模型，累计约 153 个 migration。
- 历史数据不能反向伪造 CapturePackage、采集上下文、coverage 或 producer 状态。

因此，新项目只保留源代码参考、合同、测试和数据清单。旧数据库与媒体另行只读归档。
