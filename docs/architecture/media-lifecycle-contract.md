# Linggan 媒体生命周期与材料投影合同

> 状态: 权威当前
> 最后核对: 2026-08-28
> 适用范围: ARC-001 `media-lifecycle-contract`；Browser Producer 媒体观察、字节取得、派生处理、保留/处置，以及 Evidence Library 后续读取投影
> 事实来源: Mog 已确认的产品运行规则、当前 `main` 的插件 `v0.5.0` / Rust / PostgreSQL 实现、内容工作台 V2 固定参考、历史 PR #15 语义审查
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实运行结果、当前代码/数据库/测试、ACCEPTED 决定；历史 V2 和 PR #15 只提供继承证据

## 0. 结论与边界

本合同是当前 `main` 唯一的媒体生命周期架构入口。它不从零发明媒体系统，而是把已经存在的四类事实收敛为一个后继合同：

1. 内容工作台 V2 已经验证的 `identity / origin / slot / blob / replica / usage / outbox` 分责；
2. 历史 PR #15 已通过语义审查的候选、下载、字节、副本、派生、用途和处置边界；
3. Mog 已确认的本机存储、保留期限、首批派生和资源队列规则；
4. 当前 Linggan 插件与 Rust 主线已经存在的 Package、媒体槽位、分块上传、内容寻址和处理任务事实。

合同解决的是“各责任是什么、怎样连接、页面最少可以说什么”。`resolved` 只表示产品与架构问题已经有权威答案，不表示下列事项已经发生：

- 真实平台媒体字节取得；
- 真实图片 OCR、视频 ASR、抽帧或 embedding；
- 保留期清理和撤回传播；
- Evidence Library 多材料页面实现；
- 生产部署、规模化稳定性或用户验收。

本卡不修改 Rust、SQL、API、Web、插件或运行数据，也不授权真实平台访问、回填、处理器调用、部署或清理。

## 1. 不可压缩的责任链

媒体不是一个 URL 字段，也不是一条线性 `pending → complete` 状态机。最小事实链为：

```text
来源主体（作品；未来可扩展到有合同的评论或作者）
  └─ MediaSlot（媒体槽位：在主体中的用途与顺序）
       └─ MediaOriginObservation（媒体来源观察：本次看见的候选地址与代次）
            └─ MediaDownloadAttempt（媒体下载尝试）
                 └─ MediaBlob（经校验的内容寻址字节）
                      └─ MediaReplica / Materialization（可读取副本）
                           └─ ProcessingJob + Event（派生处理运行）
                                └─ MediaDerivative（OCR / ASR / 关键帧 / 缩略图 / embedding 等）

MediaSlot / MediaBlob / MediaDerivative
  └─ MaterialUsage（页面、Corpus、分析、引用或 Agent 的具体使用）

任一受影响对象
  └─ Disposition（保留期清理、授权撤回、删除或访问限制及传播回执）
```

后层成立不能改写前层，前层成立也不能自动授权后层：

- 看见候选地址，不等于已经下载；
- 下载返回字节，不等于字节已经校验；
- Blob 存在，不等于当前有可读取副本；
- 副本可读，不等于 OCR/ASR 已完成；
- 派生成功，不等于普通页面或外部 Agent 有权读取；
- 页面不再显示，不等于原件和所有派生已完成处置。

## 2. V2 继承矩阵

| 分类 | V2 能力或经验 | Linggan 当前决定 |
|---|---|---|
| **直接继承** | 媒体逻辑身份、来源、槽位、字节、存储副本、业务用途和异步事件分责 | 保留这些责任；一个物理字节可服务多个槽位，一个槽位的多个来源观察不能因去重消失 |
| **直接继承** | `MediaBlob` 以完整性身份描述字节，`MediaReplica` 描述字节在哪里 | 内容 hash 管字节身份；路径、交付 URL 和 CDN 地址都不成为媒体身份 |
| **直接继承** | `MediaOrigin` 保存稳定来源线索、可变 fetch URL 与 generation | 来源地址按观察追加；下载工作必须固定所针对的来源观察/代次，旧任务不能覆盖新观察 |
| **直接继承** | `ContentMediaUsage` 等关系把媒体资产和具体用途分开 | 封面、正文、发布、Topic、Corpus、页面或 Agent 使用均是显式关系，不是 Blob 永久属性 |
| **直接继承** | Outbox / processing event 把原件登记与异步处理分开 | OCR、ASR、抽帧和 embedding 使用独立任务/事件，不阻塞文本 Package 或调度事务 |
| **语义继承、实现重写** | Prisma `MediaItem / MediaOrigin / MediaMaterialization / ContentMediaUsage` | 使用 Rust + 全新 PostgreSQL 逐步实现，不迁移旧表、旧 ID、旧 migration 或旧服务运行时 |
| **语义继承、实现重写** | V2 CanonicalMediaSlot 与真实 PostgreSQL 对抗证明 | 继承同主体、槽位、顺序、append-only、并发/回滚/错主体 fail-closed；使用 Linggan 的 Package 与当前数据库边界重写 |
| **语义继承、实现重写** | 同地址的封面与正文槽位可共享物理媒体 | 槽位/用途仍分别存在，校验后的 Blob 可复用；不得因字节去重删除任一来源或用途血缘 |
| **Linggan 新增** | `TaskSpec → Attempt → CapturePackage → Receipt` | 所有媒体观察和取得必须回到受控执行与不可变 Package；checkpoint 只作恢复，不是 Evidence |
| **Linggan 新增** | lane Coverage 与 partial success | 详情、评论、媒体槽位、媒体字节和派生分别报告范围与停止原因；任一失败不连坐已安全取得的其他材料 |
| **Linggan 新增** | Browser Producer 分块上传与本地内容寻址 | 上传 session 是可变交付状态，最终 Blob/来源/回执是不可静默覆盖的事实 |
| **Linggan 新增** | 多候选地址、复合媒体、派生版本和处置传播 | `candidateUris`、Live Photo 组件、模型版本、保留期清理与撤回传播进入明确合同 |
| **明确废弃** | 旧 `MediaAsset` 万能对象、URL/路径身份、旧页面 fallback | 不建立兼容双读、字段 fallback 或旧表依赖；Evidence Library 不使用远程 CDN 代替本地副本 |
| **明确废弃** | 调度事务内执行下载/转录，下载/OCR/ASR 共用串行 Outbox | 调度只做轻量决定和入队；不同资源独立队列，转录跨进程并发默认 1 |
| **明确废弃** | 空数组、任务结束或候选缺失推导 `absent` | 未观察、未知、不可访问、来源明确无候选和已清理分别表达；没有合格来源时绝不写“无媒体” |

继承的是经过验证的责任和不变量，不是 V2 的物理 schema。`references/` 仍为只读历史证据区。

## 3. 身份、槽位、顺序和来源代次

### 3.1 三种身份不得互相替代

1. **槽位身份**：某个来源主体中的一个稳定展示/用途位置。
2. **来源媒体身份**：仅当平台明确提供稳定标识且其作用域经过合同验证时成立；没有时保持未知。
3. **字节身份**：完整取得并校验后的内容 hash；相同 hash 只证明字节相同，不证明主体、槽位或用途相同。

URL、文件名、数组对象引用、本机临时路径和页面标题均不属于上述稳定身份。

### 3.2 MediaSlot（媒体槽位）

槽位最小语义是：

```text
platform
+ subject_kind
+ subject_external_id
+ purpose
+ display_ordinal
```

- `subject_kind` 首批为 `content`。评论图片和作者头像只有在各自来源合同、主体身份和 Coverage 完整后才可新增，不能借现有作品槽位兜底。
- `purpose` 首批至少区分 `cover / body_image / video / live_photo`。技术字段可以继续使用现有 role，但必须能够无损映射到这些产品含义。
- `display_ordinal` 表达作品内稳定展示顺序，从 1 开始；它不是“每种 role 内的第几个”，也不是 collector 临时数组拼接后的序号。
- 同一个资源同时作为封面和正文第一图时，保留两个不同槽位/用途；二者以后可以指向同一来源身份或 Blob。
- 来源明确的独立裁切封面与正文首图必须保持不同来源/Blob，不能因为视觉相似或 URL 接近合并。

当前插件 `packageMediaSlots` 按 `images → cover → video → livePhotoStreams` 拼接后生成全局 ordinal，Rust 又以 `(platform, content, role, ordinal)` 唯一化。它已足以证明基础 Slot lane，但尚不足以证明平台展示顺序和复合媒体关系；后续 Producer 合同适配必须显式提交来源展示顺序，不得让服务端猜测。

### 3.3 多个 `candidateUris`

插件已经能为一个候选产生多个地址，但当前 Rust 主要持久化 `externalUri`。目标合同规定：

- 同一次来源观察中，每个候选 URI 都是可追溯的地址断言；它们属于同一个槽位和同一个来源观察组，不是多个媒体槽位。
- `primary` 只表示 producer 当次建议的优先尝试地址，不表示永久权威地址。
- 每个地址至少保留顺序、来源字段/位置、观察时间和可选有效期；地址文本相同的再次观察也不能抹除新的观察发生。
- 下载尝试绑定精确的地址断言与来源代次。地址过期后可对同槽位的新观察发起新 Attempt，不修改旧下载历史。
- 服务端不得只保留第一个 URI 后宣称已经保存完整来源清单。

### 3.4 来源代次

一次 Package 对同一槽位形成一个不可变来源观察组；组内可以有多个候选地址。后续时间再次观察到地址、平台媒体标识、类型提示、展示顺序或组件关系变化时，追加新观察组/代次。

generation 是来源解析和下载 fencing，不是 Blob 版本：

- 旧 generation 的下载只能写回旧来源观察；
- 同一 Blob 可以被不同 generation 取得；
- 新 generation 不覆盖旧来源，也不能让历史页面看起来过去已经看到新地址；
- 无法证明同一来源媒体身份时，只声明“同槽位的新观察”，不制造全局 media ID。

### 3.5 图片、封面、视频与 Live Photo

| 类型 | 槽位语义 | 最小组件 | 关键限制 |
|---|---|---|---|
| 图片 | 作品正文中的展示位置 | 单个 image bytes | 顺序来自合格来源；不能按 URL 排序 |
| 封面 | 作品封面用途 | 单个 image bytes | 可与正文图片共享 Blob，但封面用途独立；必须保留 `platform_explicit / first_observed_image / unknown` 等来源依据 |
| 视频 | 作品的视频展示位置 | video bytes；派生可有 audio、ASR、关键帧 | 视频封面、视频原件和关键帧不是同一个对象 |
| Live Photo | 一个复合展示位置 | 至少 `still_image` 与 `motion_stream` 两个组件及同一 pair/bundle 关系 | 只收到一个 `live_photo` URL 不能宣称复合媒体完整；任一组件失败时保持 partial |

复合媒体不会要求所有组件在同一事务中下载成功。槽位、组件清单、各组件下载/Blob 和整体 Coverage 分别记录；成功取得静态组件时不能因运动组件失败而丢弃，也不能把它显示为完整 Live Photo。

### 3.6 `observed / absent / unavailable / unknown`

- `observed` 只能由本次合格来源明确列出的槽位或组件形成。
- `absent` 需要来源合同明确证明某个**预期、封闭槽位**在本次观察中不存在；空数组、页面结束和计数为 0 都不够。
- `unavailable` 需要来源明确说明对象存在但本次因访问、权限、格式或平台原因不可取得，并保留原因与观察条件。
- 其余情况保持 `unknown / not observed`，不能为了让页面有确定状态而补成 `absent`。

当前 V2 和 Linggan Producer 都没有完成 `absent/unavailable` 的真实来源闭环，因此当前生产性类型化接纳只能安全形成 `observed` 或保留未知；合同定义状态含义，不授权服务端推断。

## 4. 下载、字节、副本与派生

### 4.1 MediaDownloadAttempt

每次下载尝试至少固定：Work/Attempt、工位/producer、槽位、来源观察/代次、实际尝试 URI、开始/结束时间、预算、终态和失败原因。

终态必须区分：`acquired`、地址过期、网络失败、MIME 不符、大小限制、风险控制、人工停止、访问受限和未知。重试产生新的下载尝试；失败不能改写槽位或文本 Package。

### 4.2 MediaBlob 与 Replica

只有完整字节经过 hash、长度与 MIME/格式校验后才能形成 Blob。Blob 负责“字节是什么”，Replica/Materialization 负责“字节在哪里以及是否可读取”。

- 同 hash、同校验元数据重放可复用 Blob；同 hash 但长度或 MIME 冲突必须 fail-closed。
- 同 Blob 可有多个副本和多个槽位血缘。
- 本地展示只允许 Linggan 自有、经过读取校验的 `/api/local/media/<sha256>` 或后继受控句柄；外部 CDN、浏览器临时路径和旧工作台 URL 不得回退展示。
- 上传 session、chunk offset 和临时 storage key 是可变交付控制状态，不是 Evidence、Blob 或 UI 来源。
- 当前实现把 `storage_key` 放在 Blob、用 Materialization 记录本地 path，尚未形成完整多 Replica 状态/读取复核模型；合同保留 V2 的 Blob/Replica 分责作为后续实现要求。

### 4.3 ProcessingJob、Derivative 与队列

首批派生范围已经确定：图片 OCR、视频 ASR、关键帧/抽帧、embedding；缩略图和音频抽取可以作为这些能力的必要中间产物。

每个处理任务和派生结果必须固定：

- 精确输入 Blob/Replica/槽位或媒体组件；
- processor kind、模型/规则版本、输入范围、参数/提示版本和执行时间；
- pending/running/succeeded/failed/cancelled/invalidated 的 append-only 事件；
- 输出 hash、类型、位置/时间范围和访问级别；
- 与原件处置、模型升级和重新处理的失效/替代关系。

OCR/ASR 文本不是原始 Evidence 的替代物。它属于有血缘的 Material Transformation（材料转换）；Evidence Library 搜索或引用时必须能够回到具体 Blob、处理版本以及图片区域、视频时间段或帧位置。

调度与执行规则：

- 调度决策不执行下载、OCR 或 ASR，只生成有界工作；
- 下载、OCR、ASR/音频、抽帧和 embedding 按资源类型独立队列/并发；
- 转录并发必须跨进程，默认 1，执行超时 15 分钟，槽位锁最大 6 小时；
- 存活判断绑定实际工作进程，不能只看父服务；
- 本地模型默认离线；`BUSY` 只表示背压，不消耗执行重试次数；
- 当前 Rust 只创建 `provider_not_enabled` 的 pending 事件，不能显示为处理运行或成功。

## 5. 保留、清理、撤回与失效传播

### 5.1 已确认保留规则

| 对象 | 当前产品规则 |
|---|---|
| 图片原件 | 长期保存；长期不是永久，由磁盘水位和后续获准处置约束 |
| 视频原件 | 从**成功转录之日**起保留 180 天 |
| 尚未成功转录的视频 | 不进入 180 天倒计时；继续等待、重试或经明确决定放弃 |
| 明确放弃的视频 | 可以按获准处置删除字节，但必须保留最小记录与原因 |
| 关键帧 | 按图片规则长期保存 |
| OCR / ASR / embedding 等派生 | 保留版本和血缘；读取资格随原件处置/用途规则重新判断 |

删除的是受控字节或读取资格，不是历史发生：

- `从未取得`、`取得后按保留策略清理`、`读取损坏/不可用`、`获准撤回` 必须是不同状态；
- 视频清理后，合格转录和关键帧可以继续存在，但必须保留来源 Blob 身份、清理状态与当前访问限制；
- 图片“长期”必须有磁盘水位告警兜底，建议阈值 80%；超水位不授权自动删除任意原件，实际清理次序必须由后续运行策略明确；
- 来源页面消失或一次不可访问不是隐私撤回，也不自动触发本地删除。

### 5.2 获准处置的传播

隐私撤回、获准删除、脱敏或访问限制必须先阻断新的原件/派生读取，再向所有登记消费者传播：

```text
Replica / local asset
→ thumbnail / keyframe / audio / OCR / ASR
→ full-text index / embedding / feature
→ Corpus selection / Material Pack / citation
→ cache / page projection / Agent output
```

每个消费者回写完成、失败、重试或人工例外。一个文件删除、一个 worker 完成或前端隐藏均不等于传播完成。历史 Claim/Brief 不静默删除；它们保留不含敏感原文的“来源资格变化”说明并进入重新评估。

## 6. Producer Package 到材料与 Evidence Library 的映射

Package 被接纳只证明 producer 交卷通过运行时最低合同，不自动把其中每条 Record 变成 Evidence/Observation。类型化接纳、材料读取、用途资格和 Claim 资格分别判断。

| Package kind | 原始责任 | 类型化材料/事实 | Evidence Library 最低消费 | 当前主线状态 |
|---|---|---|---|---|
| `discovery_search` | 搜索面实际发现 | Discovery Finding、入口、位置、观察时间与 Coverage | 作品聚合的发现入口；可检索标题/作者候选，但明确仅发现面 | 已类型化进入 discovery 读取投影；PostgreSQL 合成证明存在，真实完整性有限 |
| `profile_discovery` | 博主页作品发现 | 作者入口下的 Discovery Finding 与 Coverage | 合并到同一作品聚合，同时保留“由哪个作者页发现” | 已类型化进入 discovery 读取投影；不证明详情 |
| `content_detail` | 作品详情来源材料 | 作品正文/标题/作者/来源时间/互动快照的版本化材料 | 作为作品聚合主体，逐字段保留来源/未知；不得把缺字段清零 | 已进入 `accepted_for_library_content` 的窄投影；不是完整 Observation/Current |
| `comments` | 顶层评论材料与本 lane Coverage | 稳定评论身份、父作品、原文/时间/作者来源和停止原因 | 评论数量、片段、覆盖、失败/未取尽；无记录不能显示 0 | Package 可保存，但当前多为 `retained_uninterpreted`；材料接纳/读取未实现 |
| `replies` | 楼中楼回复与独立 Coverage | 回复身份、父评论/根评论关系、层级、材料与停止原因 | 评论树、回复覆盖和“展开最多 2 次/自然结束”等边界 | Package 可保存，但当前多为 `retained_uninterpreted`；父子投影未实现 |
| `author_profile` | 作者资料观察 | 作者身份与资料字段的版本化来源观察 | 作品 Inspector 的作者上下文或独立作者入口；粉丝等未知不能写 0 | 已用于观察目标档案回填；尚无统一 Evidence Library 材料投影 |
| `media_slots` | 作品当次媒体清单与来源候选 | Slot、来源观察组、candidate URIs、顺序/用途/组件与 Coverage | 槽位总数、类型、顺序、来源代次和取得状态 | Slot/单一 external URI 已实现并有合成 PostgreSQL proof；多 URI、明确顺序、Live Photo 组件待适配 |
| `media_bytes` | 独立媒体字节 lane 的交付/回执语义 | Download Attempt、Upload Session、Blob、Replica/Materialization | 只显示本地受控副本、完整性和清理状态 | package kind/接口存在；真实字节走独立分块 API，真实平台 bytes 未验证；完整 Replica 模型不足 |
| OCR / ASR / 抽帧 / embedding | 服务端派生处理，不是 producer 原始内容 Package | ProcessingJob/Event、Derivative、Source Span/Material Transformation | 可检索文字或关键帧，带 processor 版本与来源位置 | Job/Event/Derivative DDL 与 pending 入口存在；provider、真实输出、embedding/关键帧完整模型未实现 |
| `batch_checkpoint` | 一次独立 checkpoint Attempt 提交时冻结的进度回执 | 当次进度、暂停/恢复位置和任务状态；浏览器可变 `resumeCheckpoint` 仍属于 execution control | **不成为语料卡或正文材料**；只在来源/执行核验区显示 | Package kind 已实现；当前每次提交会创建独立 TaskSpec/Attempt，服务端没有“同一 Attempt 可变 checkpoint”模型；不能作为 Evidence、Coverage 完成或成员清单 |

评论图片、作者头像和其他新媒体来源不得借作品 `media_slots` 进入。它们需要各自稳定主体、slot、Coverage 和用途合同后再扩展。

## 7. Evidence Library 最低消费合同

本节只冻结后续页面和读模型必须获得的信息，不修改现有 UI。

### 7.1 页面主对象

列表/卡片的主对象是：

> **一个稳定来源作品在当前 Linggan 中可核验的材料集合。**

它不是 Capture Package、Blob、媒体槽位、采集任务或“发现卡片”。同一作品可以聚合多次发现、详情、评论、回复、作者上下文、媒体槽位/字节和派生材料，但每条采用值必须保留自己的 Package/Evidence/Observation 来源；聚合视图不能制造一次不存在的“完整快照”。

### 7.2 列表最低信息

- 平台与稳定作品身份；
- 标题、作者、来源发布时间及其 KNOWN/UNKNOWN 精度；
- 发现、详情、评论/回复、媒体槽位、媒体字节、OCR/ASR 的分 lane 状态；
- 当前可用的本地媒体预览（仅 Linggan 本地受控句柄）；
- 最近观察时间与主要 Coverage/停止原因；
- 受限、处置中或字节已清理的明确提示。

禁止用一个“完整度百分比”压缩全部 lane。

### 7.3 Inspector 最低信息

```text
概览：正文、作者、来源时间、逐字段来源/未知
评论与回复：父子树、样本片段、各 lane Coverage/停止原因
媒体：槽位顺序、用途、来源代次、组件、字节/副本/派生/清理状态
来源与血缘：观察目标、Task、Attempt、Package、Receipt、producer/工位/账号镜头
限制：未请求、未观察、失败、风险停止、用途/隐私限制与真实链未验证项
```

原始受限材料默认只在受控 Inspector 中按最小必要展示；普通列表使用脱敏片段。外部 Agent 继续通过后续 CLI 获取聚合结果、脱敏片段和受控引用，不直连数据库或 Blob。

### 7.4 状态词典

各 lane 至少支持以下彼此独立的状态：

| 状态 | 含义 |
|---|---|
| `NOT_REQUESTED` | 当前没有获准请求，不是失败 |
| `QUEUED` | 已形成有界工作，尚未开始 |
| `NOT_OBSERVED` | 目标在范围内但该 lane 尚未形成合格观察 |
| `OBSERVED` | 已看见来源材料/槽位，未必取得字节 |
| `PARTIAL` | 部分材料合格，另有失败、未尝试或未知 |
| `ACQUIRED` | 原始材料或字节已经取得；不自动等于可检索/可展示 |
| `PROCESSING` | 派生处理实际运行中；pending/provider disabled 不得冒充此状态 |
| `NOT_ENABLED` | 当前处理器/读取能力没有启用；任务入口或 pending 记录存在不等于执行失败 |
| `SEARCHABLE` | 合格材料/派生已进入当前检索投影 |
| `FAILED` | 有明确执行失败和原因；不能用来表示未请求 |
| `RISK_CONTROL` | 风险/访问限制终止当前 Attempt，不自动重试或换账号绕过；读模型唯一映射现行协议 `stoppedReason=risk_control` |
| `BYTES_CLEANED` | 曾取得的字节按获准策略清理，记录与允许保留的派生仍可追溯 |
| `WITHDRAWN_OR_RESTRICTED` | 有有效处置决定，新的读取已阻断并传播中/完成 |
| `UNKNOWN` | 当前合同或来源不能确定；绝不替换成 0、无媒体或完成 |

UI 不得：

- 把 Slot 存在显示成字节已取得；
- 把正文接纳显示成作品全部材料已接纳；
- 把无评论记录显示成评论为 0；
- 把 OCR 未请求或 provider 未启用显示成 OCR 失败；
- 把 `BYTES_CLEANED` 显示成从未取得；
- 用外部 CDN 做本地封面 fallback；
- 把 checkpoint 当作语料或内容 Evidence。

## 8. 当前实现与证明矩阵（以 `95c1207` 主线为基线）

状态分类：

- `IMPLEMENTED`：当前代码/migration 有相应行为；
- `PG_SYNTHETIC_PROVED`：提交的隔离 PostgreSQL 合成测试覆盖相应不变量；
- `INTERFACE_ONLY`：合同、表或入口存在，但没有实际 processor/caller/完整行为；
- `NOT_IMPLEMENTED`：当前主线没有该能力；
- `REAL_CHAIN_NOT_VERIFIED`：没有真实平台/媒体/处理器/用户链证明。

| 能力 | 当前分类 | 当前证据与限制 |
|---|---|---|
| 九类 Producer Package 的闭集解析、Task/Attempt/Package/Receipt、replay/conflict | `IMPLEMENTED` + `PG_SYNTHETIC_PROVED` | Rust 合同和隔离 PostgreSQL tests 已覆盖合成链；不证明真实浏览器所有通道 |
| discovery/profile/detail 的窄 Evidence Library 投影 | `IMPLEMENTED` + `PG_SYNTHETIC_PROVED` | 当前读投影只消费类型化 discovery/content；页面语言仍以 discovery 为主 |
| comments/replies 类型化材料与树 | `NOT_IMPLEMENTED` | 当前多为 `retained_uninterpreted`，无统一读取投影 |
| author_profile 目标档案回填 | `IMPLEMENTED` | 只证明目标 enrichment；不是统一作者 Evidence/语料投影 |
| MediaSlot、单 URI Observation、DownloadAttempt | `IMPLEMENTED` + `PG_SYNTHETIC_PROVED` | 已证明槽位、失败不连坐文字；未证明真实平台顺序/完整媒体清单 |
| 多 `candidateUris` 来源观察 | `NOT_IMPLEMENTED` | 插件产生数组，Rust 当前只取 `externalUri` |
| cover/image/video 的槽位适配 | `INTERFACE_ONLY` | role 可保存，但明确展示顺序、封面来源依据和生产正向样本未完整证明 |
| Live Photo still/motion 配对 | `NOT_IMPLEMENTED` | 插件有 `live_photo` 候选；当前没有组件/bundle 合同和正向真实样本 |
| 分块上传、offset fencing、finalize 恢复 | `IMPLEMENTED` + `PG_SYNTHETIC_PROVED` | session 是运行状态；证明为合成字节/数据库链 |
| SHA-256 Blob 去重、同 Blob 多 Slot/Materialization、本地 asset path | `IMPLEMENTED` + `PG_SYNTHETIC_PROVED` | 当前 Blob/Replica 物理分责仍不完整；真实字节未验证 |
| 本地文件临时写、hash 校验、原子 rename 和读取路由 | `IMPLEMENTED` | 当前 API 有实现与合成测试；未证明真实平台大文件、性能和长期可靠性 |
| ProcessingJob/Event 与 Derivative DDL | `INTERFACE_ONLY` + 部分 `PG_SYNTHETIC_PROVED` | Blob admission 会建 `provider_not_enabled` pending 事件；没有成功输出 |
| OCR / ASR provider | `NOT_IMPLEMENTED` + `REAL_CHAIN_NOT_VERIFIED` | 不能显示 PROCESSING/SUCCEEDED |
| 抽帧/关键帧与 embedding | `NOT_IMPLEMENTED` | 当前值域不构成完整实现 |
| 下载/OCR/ASR 独立 worker 队列、跨进程转录限流 | `NOT_IMPLEMENTED` | 产品与架构规则已冻结，等待第一阶段运行时卡实施 |
| 图片/视频保留期清理、磁盘水位告警 | `NOT_IMPLEMENTED` | 产品规则已冻结；无清理 worker/回执证明 |
| 撤回/删除/访问限制及全消费者传播 | `NOT_IMPLEMENTED` | PR #15 的语义被继承；当前没有运行链 |
| 多材料 Evidence Library 聚合、筛选和 Inspector | `NOT_IMPLEMENTED` | 本合同只冻结消费合同，后续页面/读模型卡实现 |

这里的 `PG_SYNTHETIC_PROVED` 指提交到仓库的隔离 PostgreSQL 测试与进度记录，不表示本卡重新执行了真实数据库、真实媒体或平台链。任何后续完成声明必须对 exact head 重新运行相称证明。

## 9. 对抗性验收目录

| 编号 | 场景 | 必须成立 |
|---|---|---|
| MLC-01 | 一篇 9 图作品 | 9 个正文槽位按来源顺序稳定；封面用途独立，不因 URL 去重丢槽位 |
| MLC-02 | 封面就是正文第一图 | 两个用途可指向同一 Blob；来源与用途血缘均保留 |
| MLC-03 | 封面是独立裁切图 | 不与正文首图合并；页面可以分别说明用途和本地副本状态 |
| MLC-04 | 一个槽位返回 3 个 CDN 地址 | 一个槽位/来源观察组包含 3 个候选断言；下载绑定精确 URI，不能只保留首个后声称完整 |
| MLC-05 | 同槽位后来出现新地址，旧下载晚到 | 新代次追加；旧尝试只能写回旧来源，不覆盖新观察 |
| MLC-06 | 两个槽位下载到同一 hash | 一个 Blob 可复用；两个槽位、来源和用途都存在 |
| MLC-07 | 视频封面、视频原件和关键帧 | 三者责任分开；清理原视频不删除合格关键帧或伪造其来源 |
| MLC-08 | Live Photo 只取得 still | still 可作为部分材料保留；整体明确 PARTIAL，不能显示完整实况照片 |
| MLC-09 | 10 个槽位只下载 3 个，2 个失败，5 个未尝试 | 3 个合格 Blob 继续进入受限链；Coverage 分开表达失败/未尝试，不制造“其余不存在” |
| MLC-10 | 详情成功、评论失败、媒体未请求 | 作品详情可见；评论 FAILED、媒体 NOT_REQUESTED，不能压成一个任务失败/成功 |
| MLC-11 | Blob 存在但 provider 未启用 | 页面显示已取得/待处理或 NOT_ENABLED，不显示 OCR/ASR 处理中或成功 |
| MLC-12 | 同输入以新模型重处理 | 新 Job/Derivative 版本追加；旧结果与历史使用不被覆盖 |
| MLC-13 | 视频转录持续失败 180 天 | 原件不因采集日到期被自动删除；只有成功转录或明确放弃后进入对应处置 |
| MLC-14 | 视频已成功转录并满 180 天 | 删除字节、保留清理记录、转录/关键帧血缘和当前读取限制 |
| MLC-15 | 页面暂时不可访问与隐私撤回同时出现 | 前者只影响来源观察/Coverage；后者阻断读取并传播，两者不能共用 deleted 状态 |
| MLC-16 | 原件撤回时已有 OCR、ASR、embedding、缓存和 Agent 引用 | 所有消费者各有传播回执；只删原文件不能宣称完成 |
| MLC-17 | `batch_checkpoint` 带总数和当前进度 | 只用于执行恢复；不生成 Evidence 卡、不证明成员清单或来源耗尽 |
| MLC-18 | 旧 CDN 可显示但本地副本不存在 | Evidence Library 显示媒体未取得/副本不可用，不远程回退 |

## 10. 后续卡的实现门

本合同 resolved 后，后续卡仍需各自证明：

1. **证据库产品/页面卡**：把本合同第 6–7 节转成完整产品手册、状态与可审查原型；不得为视觉需要新增数据语义。
2. **材料接纳与读模型卡**：实现 comments/replies/author/multi-origin/live-photo/派生状态的类型化接纳和 API；不得回填或猜测历史 raw Package。
3. **Evidence Library UI 卡**：只消费真实 API 状态；不得远程媒体 fallback、把 unknown 写 0 或把 Package ACK 写成材料完整。
4. **真实垂直证明卡**：按“合成 → 已有真实材料只读 → 单篇详情/槽位 → 单个媒体字节 → OCR/ASR”的阶梯逐级授权和验证。

任何真实媒体行动仍必须携带有界对象、账号/工位镜头、最大范围、允许字节/处理、停止条件、存储/处置和风险控制。遇到安全验证或访问限制时结束当前 Attempt 为相应风险终态，不绕过、不换账号、不在原 Attempt 中继续。

## 验证与未证明边界

- 历史 PR #15 的责任链、部分成功、来源代次、处置传播和 Agent 限制已吸收；其旧分支未整体移植，也不再作为当前合同入口。
- 内容工作台 V2 的模型与真实 PostgreSQL 证明只支持本合同的继承判断；它不证明 Linggan 当前实现或真实链。
- 当前插件 Manifest 与 release manifest 为 `v0.5.0`；插件 README 的 `0.4.8` 状态头属于文档漂移，不改变代码/发布包版本事实，本卡不越权修改插件文档。
- 当前真实页面探针只证明有限字段和图片槽位样本；没有视频/Live Photo 正向样本，不能据此推断平台没有这些媒体。
- 本卡未运行真实平台、浏览器、媒体字节、OCR/ASR、清理、处置、部署或用户验收，也未修改运行时代码或数据。
