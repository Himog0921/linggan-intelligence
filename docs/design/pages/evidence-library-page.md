# PAGE-EVIDENCE-001 · 多材料证据库

> 状态: 权威当前
> 最后核对: 2026-09-24
> 适用范围: `语料 → 证据库` 的产品任务、页面信息架构、技术呈现要求、状态与验收；运行时入口仍为 `http://localhost:3000/corpus/evidence`
> 事实来源: Mog 批准的五卡 Evidence Library 垂直交付、Issue #85/#86/#90、MEDIA-RECON-001、MATERIAL-PROJECTION-001、LIDS、UI execution contract 与当前 Rust/HTML/CSS/JS
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；本页规格不让静态原型冒充已接通运行时

本页所有读取均显式属于一个正式 Domain。所有 Domain 共用同一 Work Resource 和 Material Inspector；primary/reference 不再决定另一套 API 或材料身份。

从全局导航进入语料而尚未明确领域时，页面先呈现现有 LIDS 语料壳层上的「选择研究领域」弹窗；点击领域链接后才进入对应证据库。关闭弹窗只返回未选领域空态，不自动代选，也不读取材料。弹窗显示 active/paused 的真实状态，paused 历史可读；未知统计保持未知。页面内面包屑可换领域，不能保留旧领域检索或作品选择。领域列表读失败与列表确实为空必须区分。

从 Collection 的“全部领域”作品行进入时，先选领域，但需保留该作品的稳定引用；选定领域后只读它在该领域内的材料。若误选未收录该作品的领域，Inspector 明确提示该领域未收录并引导切换，不把 `material_not_found` 显示成普通空库或已读详情。

本规格替代本文件 2026-08-25 的 Discovery-only 产品定义。Issue #90 已把本规格的可由现行合同承担的部分落到运行页；静态参考仍只证明设计场景，运行页只证明当前 Material Projection 可以诚实返回的字段和状态。禁止把“页面已接通”写成“真实平台、媒体、处理器或业务验收已完成”。

## 1. 产品结论

### 1.1 页面主对象

证据库列表的主对象固定为：

> **一个稳定来源作品在当前 Linggan 中可核验的材料集合。**

同一作品可以聚合多次发现、详情、评论、回复、作者上下文、媒体槽位、媒体字节与派生材料，但每个展示值仍保留自己的来源、版本、Coverage 与限制。聚合视图不制造一次从未发生的“完整快照”。

主对象明确不是：Capture Package、Media Blob、媒体槽位、采集任务或 Attempt、单条评论、“发现卡片”、Corpus Selection、AI 摘要或洞察。Package、槽位、任务和派生均在右侧核验区成为该作品集合的血缘或组成部分，不提升为顶层卡片。

### 1.2 用户、情境与成功标准

- 首期用户：Mog 本人及小团队。
- 使用情境：桌面工作台中快速检索一篇来源作品，判断系统实际拿到了哪些材料、缺了什么、为什么缺，以及是否值得进入受控核验。
- 用户任务：以作品为单位检索、比较、阅读脱敏材料，并沿 `Target → Task → Attempt → Package → Receipt → Coverage` 追溯来源。
- 三秒答案：**这篇作品目前有哪些可核验材料，哪些仍未请求、未观察、部分、失败、风险停止、尚未启用或已清理。**
- 五秒主动作：选择一篇作品，在右侧 Inspector 查看当前材料 lane、来源、Coverage、限制与允许的下一步；对具备既有目标关联和有效深度归档授权的小红书作品，才可请求一次有界详情复观测。
- 成功标准：用户不查看数据库或插件日志，也能区分“没有请求”“没有观察到”“拿到一部分”“明确失败”“处理器未启用”“字节曾取得后清理”和“当前未知”。

### 1.3 页面非目标

本页不负责：

- 直接触发平台搜索、泛化详情、评论或媒体采集；唯一例外是本规格第 8 节所定义、已有关联授权的小红书作品详情复观测；
- 在证据库内创建或批准 Work Order；
- 把一个含糊“补采”按钮连接到未知范围；
- 远程 CDN 媒体回退；
- 修改原始 Evidence、Package、Coverage 或来源时间；
- 把材料升级为 Observation、Topic、Claim、趋势或市场事实；
- 建立第二份正文、评论、媒体或作者真相；
- 暴露批量原始敏感评论、账号身份、Cookie、令牌或完整 Package payload；
- 把 checkpoint 当作语料或内容 Evidence；
- 在本页运行 OCR、ASR、抽帧、embedding 或 Agent 分析。

该唯一例外不创建人工 `Task`，不猜测监控目标，不请求媒体/OCR/ASR，不改变既有资产；它只能沿
`已关联 Target/Authorization → Work Order → Lease → server-issued Task → Attempt → Package → Receipt`
发起，并且 `Receipt` 之外的状态不得写成成功。

## 2. 当前运行事实与目标边界

| 层级 | 当前事实 | 页面必须怎样表达 |
|---|---|---|
| 产品与设计 | 本规格和静态原型已冻结多材料职责 | 继续约束运行页，但原型内容不得冒充运行数据 |
| 当前交付分支 | `/corpus/evidence` 只消费共享 `/api/local/work-resources`，列表与 Inspector 共用 Work Resource Read Interface | Evidence Library 不是接口 owner；后续页面不得另写 SQL/API/字段推断；不混读 legacy cards |
| 作者与采集来源 | 2026-09-04 产品决策：创作者监控的入口即该创作者主页，从该入口取回的作品视为其所发 | 只显示一个“作者”字段：详情 `creatorDisplayName` 优先，其次 creator 目标显示名（`MISMATCH` 时不回退），皆无则“当前未知”；关键词目标永不填补作者；采集来源移入 Inspector「来源与溯源」 |
| 发布时间 | detail collector 交付原始字段、值类型、精度、参照时点和 parser version；只有平台 epoch 晋升为精确时间 | `KNOWN / SOURCE_TEXT_ONLY / UNKNOWN` 分开；禁止用 observed/accepted 时间代替发布 |
| 评论/回复 | 类型化 lane、Coverage 与本机授权评论研究通道已接入详情；普通列表不返回原文 | 原文只在授权详情按页读取，匿名上下文不暴露平台用户标识 |
| 作者资料 | 详情可返回版本化作者上下文；稳定作者 ID 与头像同时存在时，头像进入统一媒体链 | 页面不显示 `authorExternalId`，未知字段不补值；只内联受控本地头像，不回退远程地址 |
| 媒体槽位 | Slot、来源代次、候选断言、Live Photo 组件及有界回执已进入 Material Projection | 槽位存在不等于字节已取得；回执截断但无通道 URL 时显示 `SOURCE_INCOMPLETE` |
| 媒体字节 | Blob/Materialization/处置与受控本地 asset handle 已进入详情 | 仅 `INLINE_SAFE` 且同源受控句柄可内联；真实平台字节未验证 |
| OCR/ASR 等派生 | Job/Event/Derivative 生命周期进入详情，provider 当前未由本卡启用 | 原样显示 `QUEUED/PROCESSING/NOT_ENABLED/FAILED/ACQUIRED/UNKNOWN`，不由 UI 推断 |
| 小红书详情复观测 | 仅可复用既有 target-linked active deep-archive authorization；标准范围为详情、评论、回复，评论窗口最多 30 条 | Inspector 只显示实际 Work Order/Lease/Task/Attempt/Package/Receipt；不把工作请求说成已读取，也不新建媒体、OCR 或 ASR 工作 |
| 静态原型 | 全部内容均为合成场景 | 首屏和每个材料区持续显示“合成参考 / 非运行数据” |

## 3. 设计方向锁

Issue #85 沿用已确认项目方向，不重新向用户提出视觉选择。

1. **谁使用、在什么情境**：Mog/小团队在 Linggan 桌面 App Shell 内执行高密度证据核验；不是营销站、客户门户或移动优先消费产品。
2. **美学方向**：`LIDS-SYS-001` 的“纯白台面上的精密情报基础设施”；编辑式信息设计为主、仪器式读数为辅、新粗野主义只作少量重音。
3. **记忆点**：每个来源作品拥有一条可横向扫描的“材料 lane 带”，用户不靠总百分比即可看见发现、详情、讨论、媒体和派生的真实分层。
4. **硬约束**：中文主表达；现有 LIDS Token；桌面工作台优先；键盘可达；Reduced Motion；敏感材料默认脱敏；不修改 global Token、runtime HTML/CSS、后端或插件。
5. **签名交互**：选择作品集合后，同一工作面更新右侧 Inspector；选中行只使用获准的 `translate(-2px,-2px)` 与硬阴影重音，Inspector 只以 opacity/transform 进入，Reduced Motion 下立即切换。

### 3.1 视觉与内容 thesis

- **Visual thesis**：高密度但克制的白色证据台面，以冷黑结构、橙红选中信号和琥珀/红/灰状态点，让材料差异像实验记录一样可扫描。
- **Content plan**：方位与查询 → 作品集合列表和 lane 状态 → 当前作品材料 → 来源血缘与限制；没有营销 Hero。
- **Interaction thesis**：行选择同步 Inspector；lane 筛选只改变本地合成视图；受限原文用 inline restricted state，不用 Modal 或 Tooltip 隐藏必读边界。
- **CSS 策略**：原生 CSS only；静态原型直接消费 `apps/api/src/local_web/lids_tokens.css`，不声明 `--lgi-*`，不新增第二套 Token。
- **半径系统**：只使用 `none / xs / sm / md = 0 / 2 / 4 / 8px`；按钮和选择项以 0–2px 为主，不使用 Pill。

## 4. 页面强度、Pattern 与表面地图

- LIDS 强度：L1 `Corpus Explorer`，嵌入受限 L2 `Split Evidence Inspector`。
- 唯一主 Pattern：`Corpus Explorer`。
- 页面唯一视觉核心：中央“来源作品材料集合 + lane 带”。
- 右侧 Inspector 是当前作品的核验区，不是第二个首页，不复制搜索、筛选或导航。

| 表面 ID | 表面 | 页面责任 | 主要数据 | 不负责 |
|---|---|---|---|---|
| `EV-S01` | 共享页头与上下文行 | 产品方位、当前职责、关键数量/读取状态 | Shell + read envelope | 页面内再重复标题、显示假实时状态 |
| `EV-S02` | 语料二级 rail | 当前位于证据库；其他未接通入口保持禁用 | Route capability | 承担材料筛选或对象状态 |
| `EV-S03` | 查询与视图工作台 | 本地材料检索、时间视角、lane/状态/媒体/受限筛选；`SYSTEM VIEWS / 系统视图` 承载真实 `view` 预设；`MY VIEWS / 我的视图` 只显示保存能力未接通的诚实空态 | Query scope / asOf / cursor | 触发平台采集；把状态预设做成第三栏导航；伪造已保存视图或可保存能力 |
| `EV-S04` | 结果上下文 | 结果数、匹配字段、排除/限制、读取水位 | Query receipt | 用总数证明平台总量或完整性 |
| `EV-S05` | 作品材料集合列表 | 以研读/表格/封面三种排版比较同一批作品身份、作者/目标、时间、lane 和限制 | Work Resource Read | 以 Package/Blob/Slot 作顶层行；布局各走一套数据逻辑 |
| `EV-S06` | 当前作品 Inspector | 概览、评论/回复、媒体、派生、来源/血缘、限制 | Selected work envelope | 第二搜索器、AI 结论、无来源补值 |
| `EV-S07` | 反馈位置 | 查询无结果、读取失败、受限、部分、处理中、空态说明 | Read receipt / error envelope | Toast 替代必读错误或回执 |
| `EV-S08` | 窄屏控制 | 打开材料筛选、切换列表/Inspector、返回当前作品 | Local view state | 把三栏按比例缩小或隐藏关键状态 |

### 4.1 依赖地图与文件所有权

| 依赖 | owner | 本卡如何使用 | 本卡不得做 |
|---|---|---|---|
| `MEDIA-RECON-001` | 卡 1 / PR #84 | 消费材料、媒体、状态与来源语义 | 修改 identity、slot、bytes、derivative 或处置合同 |
| 多材料 read model/API | 卡 3 / Issue #86 | 冻结页面所需字段和查询语义 | 实现 Rust/SQL/API 或猜字段 |
| 运行时页面 | 卡 4 / Issue #90 | 当前 HTML/CSS/JS 只消费卡 3 Material Projection，并提供有界 Inspector | 修改卡 3 字段、SQL、媒体资格或接纳语义 |
| 真实垂直证明 | 卡 5 | 提供必须验证的页面场景 | 运行真实平台、媒体或 OCR/ASR |
| Shell / LIDS Token | 共享 UI owner | 原型只消费现有规则 | 修改 `shell.rs`、`shell.css` 或全局 Token |

## 5. 信息架构

### 5.1 列表行的信息顺序

每个作品集合按下列顺序呈现：

1. 本地媒体预览，或准确的未取得/已清理/受限状态；
2. 平台、稳定作品引用、标题；作者使用单一事实区并可显示受控本地头像，随后是来源发布时间及精度；采集来源只在 Inspector「来源与溯源」区表达；
3. 脱敏摘要或“尚无可展示摘要”；
4. 发现、详情、讨论、媒体、派生五组 lane；其中评论/回复和 OCR/ASR 仍可分别展开；
5. 最近观察时点、主要 Coverage/停止原因、限制；
6. 当前选择信号。

不得用一个“完整度 73%”代替 lane。列表不显示完整原始评论、URL、Cookie、完整 Package payload 或远程媒体地址。

小红书作品的封面预览在研读与封面排版中统一使用平台原始竖版语义 `3:4`；图片保持 `object-fit: cover`。该比例只决定预览容器，不改变受控本地媒体资格，也不允许在本地副本缺失时回退到远程 CDN 地址。

### 5.2 Inspector 信息顺序

```text
当前作品 / 稳定 public ref
├─ 概览：标题、正文、作者（含受控本地头像）、来源与溯源（单行来源监控）、来源时间、逐字段当前事实/来源、互动当前值/前值/变化
├─ 概览内受限动作：仅 XHS + target-linked active deep-archive authorization 才可“立即复观测”；显示租约与逐 lane 真实状态
├─ 评论与回复：脱敏片段、父子关系、各 lane Coverage/停止原因及每个 Package/Receipt 独立的 Coverage 历史
├─ 媒体：槽位顺序、用途、来源代次、组件、字节/副本/清理状态
├─ 派生：OCR/ASR/关键帧/embedding 状态、processor 版本、来源位置
├─ 来源与血缘：目标、Task、Attempt、Package、Receipt、producer/工位/账号镜头
└─ 限制与访问：用途、敏感级别、未请求/未观察/风险停止/真实链未验证
```

Inspector 默认停在“概览”，但页面不得只在隐藏 Tab 中提供限制；最严重限制在 Inspector 顶部和列表行都可见。评论原文、外部 URL 和账号镜头不进入首屏。

### 5.3 搜索、筛选与排序职责

| 控件 | 获准语义 | 必需回执 | 禁止退化 |
|---|---|---|---|
| 文本搜索 | 检索获准字段；首批至少标题、作者，后续可含已进入检索投影的脱敏正文/OCR/ASR | `matchedFields`、query scope、asOf、结果数 | 平台搜索、未接纳 Package/raw payload 搜索 |
| 时间视角 | 缺省为最新已接纳视角；显式 7/30 天只按来源可验证发布时间 | time view、unknown published exclusion | 用观察/接收时间替代发布时间 |
| Lane | discovery/detail/comments/replies/author/media/derivative | 当前 lane 与结果数 | 总“完整/不完整”筛选 |
| Lane 状态 | 本规格状态词典闭集 | 状态来源与数量 | 把未请求混入失败 |
| 媒体类型 | image/cover/video/live photo；只依据合格槽位 | 来源与槽位 Coverage | 从扩展名或 URL 猜类型 |
| 访问/处置 | restricted/withdrawn/bytes cleaned 等 | 当前 display policy | 让筛选绕过权限 |
| 排序 | 默认最近观察降序；后续可明确切换来源发布时间 | sort key、asOf | 把“最近观察”写成“最新发布” |

排版使用独立 URL 参数 `layout=research|table|cover`；状态筛选继续使用 `view`。`view` 是查询预设，必须与其他筛选一起放在主结果上方的 `SYSTEM VIEWS / 系统视图` 横向工作台，不作为左侧导航或独立状态面板；结果数、读取回执和当前选择分别由工作台底栏、inline receipt、选中行/Inspector 承担，不重复做统计栏。`MY VIEWS / 我的视图` 可以保留参考稿的结构位置，但在保存合同缺失时只能显示不可交互的“暂无已保存视图 / SAVED VIEWS NOT CONNECTED”，不得展示假视图或假保存动作。切换排版只重排已读取的同一 Work Resource 集合，不重新请求、不改字段资格、不改变当前选择。保存视图、批量选择、发起研究和泛化“补采”继续禁用，直到各自有独立产品/权限/回执合同。唯一已接通的例外是当前选中作品的受限 `立即复观测`：它只面对小红书、稳定 public ref 和已存在的 target-linked active deep-archive authorization，不能从作者、标题、URL 或采集来源名称推断授权；请求与状态均来自受控 API，不能显示本地伪造成功。

### 5.4 Domain 范围与材料用途

列表、单品、Inspector、评论通道、cursor 与筛选必须显式绑定当前 Domain。所有 Domain 使用共享 Work Resource 接口；不传 Domain、Domain 不存在或 cursor 属于其他 Domain 时明确失败，不回落 ADHD，也不读取其他 Domain。

| 用途 | 允许表达 | 默认研究行为 | 必须保持 |
|---|---|---|---|
| `primary` | 当前 Domain 的默认研究材料 | 默认进入该 Domain 的自动研究与统计 | 展示材料真实 lane、来源和限制 |
| `reference` | 当前 Domain 的对照材料 | 默认排除；用户可在 Comment Study 等明确动作中选入 | 所有基础材料可完整读取；持续显示参照标签；不自动支持本领域结论 |

同一 Content 在多个 Domain 可被读取，但仍只有一个 canonical Work Resource。每个 Domain 的材料列表必须来自明确、可追溯的材料用途；不能从 Target 当前关系或旧单值字段推导历史归属。paused Domain 的历史材料和既有结果继续可读并标注暂停，新的研究写入被拒绝。

## 6. 材料 lane 与数据来源

| Lane | 用户看到的材料 | 最小来源/字段 | 当前实现边界 |
|---|---|---|---|
| 发现 `discovery` | 从搜索面或作者页发现作品 | platform、content identity、入口、位置、observedAt、Coverage | 已有窄投影；不是详情 |
| 详情 `detail` | 标题、正文、作者、来源发布时间、互动快照 | 每字段 value/state/sourceRef/version；逐字段最新合格当前事实；每指标最新合格 current/previous/delta | 当前事实与时间线分开；时间线有界，不得决定 current；不回填或覆写历史 Package |
| 评论 `comments` | 顶层评论脱敏片段、已知数量与覆盖 | comment refs、snippet、count state、Coverage、stop reason | 当前投影只展示获准脱敏材料；每个 Package/Receipt 的 Coverage history 独立，缺失不得显示 0 |
| 回复 `replies` | 楼中楼父子关系与脱敏片段 | reply/root/parent refs、tree state、Coverage | 只展示已验证父子关系；每个 Package/Receipt 的 Coverage history 独立，不从评论历史推断 |
| 作者 `author` | 该作品的作者上下文 | author ref、字段状态、observedAt、sourceRef | 目标档案回填不等于统一材料投影 |
| 媒体槽位 `media_slots` | 类型、用途、顺序、来源代次、组件 | slot key、purpose、display ordinal、generation、component state | 单 URI/顺序/Live Photo 适配有限 |
| 媒体字节 `media_bytes` | 是否取得、校验、本地副本、清理 | download attempt、blob hash ref、replica/materialization、disposition | 只显示本地受控句柄；真实平台 bytes 未验证 |
| OCR `ocr` | 图片文字的脱敏可检索片段 | derivative ref、processor version、source region | provider 未启用；不能显示 processing/success |
| ASR `asr` | 视频转录的脱敏可检索片段 | derivative ref、processor version、time range | provider 未启用；不能显示 processing/success |
| 执行 checkpoint | 本次执行进度与恢复位置 | checkpoint receipt、Attempt、stopped reason | 只在血缘区显示，不成为 lane/语料卡/Evidence |

### 6.1 页面所需最小 read envelope

字段名由卡 3 的版本化 API 决定，但语义责任必须完整：

```text
query: text / timeView / lane / laneState / mediaKind / restriction / sort / cursor
read: asOf / scope / versions / cursor / resultCount / excludedCounts / limitations
item:
  identity: platform / contentExternalId / stablePublicRef
  display: title / creator / publishedAt + source field/kind/precision/reference/parser + each value state
  collectionContext: target / relationshipState / authorIdentityMatchState / workOrderRef
  media.avatar: relationship / state / purpose / localAssetUrl / blob delivery state
  preview: localAssetUrl / slotPurpose / bytesState / alt
  laneSummaries[]: lane / state / counts-with-value-state / stopReason / limitations / latestObservedAt
  summary: lastObservedAt / primaryLimitation / restrictionState / matchedFields
inspector:
  overview fields + sourceRefs
  detailCurrent: field-wise current title/body/creator/publishedAt + source
  engagementCurrent: latest known per metric + previous + delta
  engagementTimeline[]: packageRef / sourceLane / observedAt / recordedAt / qualified metric states
  commentThreads + commentsCoverage + repliesCoverage
  commentsCoverageHistory[] + repliesCoverageHistory[]: one entry per Package/Receipt, never cross-attempt aggregate
  mediaSlots[]:
    slotRef / purpose / displayOrdinal / currentCoverage
    currentOriginGroup:
      originGroupRef / packageRef / generation / observedAt
      declaredBundle: bundleRef / bundleKind
      components[]:
        componentRef / componentKind / state
        candidateAssertions[]:
          candidateRef / order / primary / sourceField / observedAt / expiresAtState / expiresAt
        downloadAttempts[]:
          downloadAttemptRef / candidateRef / originGroupRef / generation / state / terminal / failureReason / startedAt / endedAt
    originHistory[]: originGroupRef / packageRef / generation / observedAt / historyState
    bytes / replica / derivative / disposition states
  derivatives[] + processorVersion + source location
  provenance: target/task/attempt/package/receipt/producer/station-account lens/coverage/checkpoint
  permissions + displayPolicy + limitations
action:
  reobservation: XHS only + canonical supported/eligible/reason from target-linked active deep-archive authorization
  request: URL exists only when eligible; POST rechecks and atomically writes Work Order → Lease → server-issued Task
  merge: only same target + authorization + complete frozen material policy; detail/comments/replies only; commentLimit=30
  media: NOT_REQUESTED / EXISTING_ASSETS_REUSED; no new media bytes, OCR, or ASR work
  status: requestRef / workOrderRef / leaseRef / task → attempt → package → receipt
```

计数必须带 value state。`null/UNKNOWN`、`0/KNOWN` 与 `N/A` 不得共用一个空值。所有列表和 Inspector 值均来自受控 read model，不允许 UI 自行聚合原始业务表或 Package JSON。普通页面只读取脱敏 `candidateRef` 和上述地址断言元数据，不读取或显示原始 URI。`primary` 只表示 Producer 对本次来源观察的建议，不是永久权威；`expiresAtState=KNOWN` 时必须同时提供 `expiresAt`，未知时为 `UNKNOWN/null`。一次 Package 对同一 slot 只形成一个槽位级来源观察组；declared bundle、still/motion 组件和组件内 candidate assertions 都挂在该父级下。下载尝试必须同时绑定该来源观察组/generation 与精确 `candidateRef`，并交付开始/结束时间、terminal 和失败时的 `failureReason`。跨代来源只能进入明确标识的 `originHistory[]`，不得与本次 Package 的当前来源组摊平混排。

## 7. 状态词典

### 7.1 Lane 状态闭集

| 状态 | 中文主表达 | 用户应理解 | 禁止暗示 | 视觉角色 |
|---|---|---|---|---|
| `NOT_REQUESTED` | 尚未请求 | 当前没有获准工作 | 失败、平台没有、以后一定会请求 | Unknown |
| `QUEUED` | 已排队 | 有界工作已形成，尚未开始 | 已观察、处理中或已取得 | Info |
| `NOT_OBSERVED` | 尚未形成观察 | lane 在范围内但没有合格观察 | 数量为 0、来源不存在 | Unknown |
| `OBSERVED` | 已观察 | 看见来源材料/槽位 | 字节已取得、可检索、覆盖完整 | Info |
| `PARTIAL` | 部分取得 | 已有合格材料，同时有缺口/失败/未尝试/未知 | 整体失败或完整代表性 | Warning |
| `ACQUIRED` | 已取得 | 原始材料或字节已安全取得 | 已进入搜索、已完成派生 | Success |
| `PROCESSING` | 处理中 | 处理器实际运行 | 仅有 pending/job row/provider disabled | Info/operation |
| `NOT_ENABLED` | 处理器未启用 | 当前能力不存在或 provider disabled | 执行失败、正在处理 | Unknown |
| `SEARCHABLE` | 可检索 | 合格材料/派生已进入当前检索投影 | 对任意用途可公开、可代表总体 | Success |
| `FAILED` | 执行失败 | 有明确失败和原因 | 未请求、未知、访问受限 | Danger |
| `RISK_CONTROL` | 风险控制停止 | 当前 Attempt 因风险/访问限制结束 | 可自动重试、换账号或绕过 | Danger |
| `BYTES_CLEANED` | 字节已按策略清理 | 曾经取得，当前字节不再可读；允许派生/记录可追溯 | 从未取得、来源不存在 | Warning |
| `WITHDRAWN_OR_RESTRICTED` | 已撤回或限制读取 | 有有效处置决定，读取被阻断/传播中或完成 | 普通失败、历史从未存在 | Danger |
| `UNKNOWN` | 当前未知 | 合同或来源不能确定 | 0、正常、失败、完整 | Unknown |
| `SOURCE_TEXT_ONLY` | 仅有来源时间文本 | 来源提供了可展示文本，但不足以证明精确时间点 | 精确发布时间、可用于严格时间窗 | Warning |

### 7.2 页面级状态

| 状态 ID | 触发来源 | 用户理解 | 页面处理 |
|---|---|---|---|
| `SYNTHETIC_REFERENCE` | 本卡静态原型 | 仅验证设计，不是系统数据 | 首屏固定标识；每个原声/计数区域再次标识 |
| `SOURCE_INCOMPLETE` | 来源/读模型没有资格回答 | 不是空库或平台没有内容 | 说明缺什么，禁用依赖动作 |
| `READ_PROJECTION_UNAVAILABLE` | 受控读取失败 | 没有读取任何材料 | inline error + incident ref；不远程回退 |
| `LOCAL_QUERY_INVALID` | 查询合同不接受 | 未执行读取或平台搜索 | 保留输入，说明可修正字段 |
| `NO_MATCHING_MATERIAL` | 读模型成功、当前查询无匹配 | 只对当前范围为零 | 显示 scope/Coverage/排除，不能外推现实不存在 |
| `SELECTION_REQUIRED` | 结果存在但未选择作品 | Inspector 尚无对象 | 引导选择一行；不显示伪造详情 |
| `ACCESS_RESTRICTED` | displayPolicy 不允许当前材料 | 材料可能存在但当前不展示 | 显示限制与用途，不泄漏原文 |

### 7.3 复观测操作状态

| 状态 | 用户应理解 | 不得写成 |
|---|---|---|
| `QUEUED` | 有界工作已形成，尚未由工位领取 | 已读取、已观察或已接纳 |
| `CLAIMED` | 工位已领取，但尚未形成 Attempt | 执行完成 |
| `RUNNING` | Attempt 已形成，尚未交付回执 | 成功、完整或当前事实已刷新 |
| `ACCEPTED` | 对应 lane 的 Package 已有接纳回执 | 全 lane 完整、Coverage 完整或平台事实代表性 |
| `EXPIRED_WITHOUT_RECEIPT` | Lease 到期而该 lane 未见接纳回执 | 失败已可自动重试、平台没有内容 |
| `COMPLETED_WITHOUT_RECEIPT` | Task 已结束而未见接纳回执 | 成功或已经观察 |
| `INPUT_BLOCKED` | 这次复观测缺执行地址，在打开页面前就被停下，租约随之结束 | 已排队、稍后会自动重试、试过没成功 |

`INPUT_BLOCKED` 是迁移 `0097` 新增的 `execution_state`：这类成员没有 Attempt、没有 Package，也不会沿这张租约继续走，只可能被**新的**输入或新的资格行重新派发。所以它是终态，界面不得显示为「已排队」，也不得让页面继续对它轮询等待。

`ACCEPTED` 是回执事实，Coverage 仍按 lane 和每个 Package/Receipt 独立显示；部分 lane 接纳不能遮蔽另一个 lane 的未回执或部分 Coverage。

本表是复观测状态的**闭集**声明。当前读取模型还会输出 `PAGE_UNAVAILABLE`（生产方确认页面不在）与 `DETAIL_READ_BLOCKED`（读过但未读成）两个值，本表与运行时词表都尚未给它们各自的文案，它们在界面上会落到兜底的 `来源未完整表达`。这是本页的已知缺口，不是上面两个状态的正确表达；补齐与否由产品决定（2026-09-21 记录，未扩展本次交付）。

### 7.4 组合规则

- 作品没有全局 `complete / failed` 状态；只显示 lane 组合和最高优先限制。
- `PARTIAL + VALID` 是一等状态；列表保留已取得材料，不进入“全部失败”。
- Package ACK 只说明交卷接纳，不产生作品完整状态。
- Slot `OBSERVED` 与 bytes `NOT_REQUESTED` 可以同时成立。
- detail `SEARCHABLE`、comments `FAILED`、media `NOT_REQUESTED` 可以同时成立。
- bytes `BYTES_CLEANED` 与 ASR `SEARCHABLE` 可以同时成立，且必须保留清理说明。
- risk control 只由现行 `stoppedReason=risk_control` 映射为 `RISK_CONTROL`，不派生自动重试。

## 8. 交互、反馈与回执

| 用户动作 | 前置条件 | 请求/回执 | 页面反馈 | 不得宣称 |
|---|---|---|---|---|
| 搜索/筛选/排序 | read model 可用 | Query receipt：normalized query、scope、asOf、cursor、结果/排除/限制 | 结果区 inline 更新；保留可恢复 URL 状态 | 平台搜索、采集已启动 |
| 选择作品 | 当前结果包含 stable public ref | 只读 Inspector request/response | 选中态 + Inspector 更新；移动端进入详情并可返回 | 创建 Selection、修改 Evidence |
| 展开材料片段 | displayPolicy 允许 | Material fragment read receipt | 展示脱敏片段、来源 ref、访问级别 | 自动获得完整原文权限 |
| 查看来源/血缘 | 有受控 refs | Provenance read response | 展示 refs、Coverage、限制和 checkpoint execution receipt | checkpoint 是 Evidence、ACK 是完整 |
| 查看本地媒体 | 可读本地副本且用途允许 | Linggan local asset handle | 固定尺寸预览、alt、bytes/replica 状态 | 使用外部 CDN fallback |
| 立即复观测 | 当前选中 XHS 作品有稳定 public ref，且可解析为既有 target-linked active deep-archive authorization | `POST /api/local/work-resources/{publicRef}/reobserve` 创建真实 Work Order/Lease；`GET …/reobserve/{leaseRef}` 读取实际 task/attempt/package/receipt | 显示固定详情/评论/回复范围、30 条评论窗口、无媒体策略和真实操作状态；无授权返回明确拒绝 | 已读取平台、泛化补采、作者/标题/URL/目标名回退、媒体/OCR/ASR 已请求 |
| 保存视图/批量选择/研究/泛化补采 | 当前无合同 | 无 | disabled + 开放条件 | 伪造成功 toast 或本地计数 |

查询、选择和 Tab 是可恢复 URL/本地视图状态，但不能被写成服务端业务动作。必读失败、限制和部分状态使用结果区或 Inspector inline feedback，不使用 Toast/Tooltip 作为唯一载体。

## 9. 敏感材料最小展示合同

1. 普通列表只显示脱敏标题/摘要、作者显示名的获准形式、稳定 public ref 和状态；不显示完整评论、回复、来源 URL 或账号镜头。
2. ADHD、儿童、家庭、医疗和可反向识别片段默认脱敏；短片段也必须有用途、访问级别与来源 ref。
3. Inspector 只有在 `displayPolicy` 允许时显示受限原文；默认仍为脱敏片段。受限状态必须可理解，但不能泄漏被限制内容。
4. 外部 Agent 权限与本页无关；页面不得因 Agent 可访问聚合结果，就为人类普通列表扩大原文。
5. 获准撤回/限制后，页面、搜索和 Inspector 停止返回失效原文；历史条目显示“来源资格已变化”，而非无声消失。
6. 静态原型只使用人工合成中文材料，不包含真实 XHS 标题、作者、URL、评论、媒体或 ID。

## 10. 已批准的页面组合

| 区域 | 来源 | 使用 | 禁止替代 |
|---|---|---|---|
| 共享壳层 | `LIDS-PAT-001` shell ownership | 页头、上下文、216px rail | 在页面 CSS 重新定义 runtime shell |
| 查询与视图工作台 | `Corpus Explorer` + LIDS Input/Quiet | 检索和筛选已接纳材料；以系统视图/我的视图区分平台预设与用户保存能力；后者未接通时显示禁用空态 | 平台搜索框、假保存视图、独立状态侧栏 |
| 作品连续列表 | `Corpus Explorer` | 高密度比较主对象 | 卡片瀑布流、KPI 卡阵列 |
| 材料 lane 带 | `LIDS-PRI-001` 五轴分责 + MEDIA-RECON | 分开显示每 lane 状态 | 万能 StatusTag、完整度百分比 |
| Inspector | `Split Evidence Inspector` | 核验当前作品和血缘 | 第二个首页/筛选器/AI summary |
| 内联限制/错误 | LIDS feedback | 明确影响与下一步 | 原生 alert、toast-only、技术堆栈 |

本卡不晋升正式 CMP。材料 lane 带、作品行和 Inspector section 都是 page-local 候选；只有第二个独立页面证明同一责任后才可走 CMP promotion。

## 11. 响应式、可访问性与动效

### 11.1 桌面

- `≥1500px`：216px rail + flexible results + 440px Inspector。
- `1181–1499px`：216px rail + flexible results + 420px Inspector。
- `901–1180px`：216px rail + flexible results + 380px Inspector；lane 带允许分组换行，但不丢状态。
- 桌面内容工作面固定在视口剩余高度；列表和 Inspector 各自内部滚动，不让 document 无界延长。

### 11.2 窄屏

- `≤900px`：共享页头按内容增高；rail、查询、结果、Inspector 顺序折叠；document 原生滚动。
- `≤640px`：列表行不再保留横向缩略图列；预览进入标题下方，lane 使用两列网格；选择作品后滚动到 Inspector，Inspector 提供“返回当前作品所在列表”，形成列表 ↔ Inspector 闭环。
- 390×844 和 430×932 必须无页面级横向滚动；技术键允许换行，不截断必读中文。

### 11.3 可访问性和 Motion

- 每个交互命中区至少 40×40px；可见 Focus Ring；不用 `tabindex>0`。
- 结果使用语义列表/文章；lane 状态既有中文文字也有图形位置，不只靠颜色。
- 技术键紧邻中文主语义，原始材料不翻译。
- 选中行和按钮只使用 transform/box-shadow 的获准反馈；Inspector 内容只使用 opacity/transform。
- `prefers-reduced-motion: reduce` 下关闭位移和过渡，状态信息不消失。

## 12. 完整原型场景

静态原型 [`evidence-library-multi-material-reference.html`](evidence-library-multi-material-reference.html) 至少包含：

1. **部分但可用**：detail 可检索；comments partial；replies failed；media slots observed；bytes not requested；OCR/ASR not enabled。
2. **风险停止**：详情/已有材料保留；当前 Attempt 为 `RISK_CONTROL`；不显示自动重试。
3. **字节已清理**：视频 bytes 为 `BYTES_CLEANED`；ASR 仍可检索；来源 Blob 和清理状态可追溯。
4. **未知与未请求**：作者/发布时间 unknown；媒体 not requested；无评论记录不显示 0。
5. **处理中**：只有处理器实际运行才显示 `PROCESSING`；原始字节与本地副本状态、Job、处理版本和开始时间可核验；当前不得提前显示 `SEARCHABLE`。该场景是合成目标状态，不表示当前 provider 已启用。
6. **多候选来源与 Live Photo 部分取得**：本次 Package 对同一 Live Photo slot 形成一个槽位级来源观察组/generation，再由 declared Bundle 关联独立的 `still_image` 与 `motion_stream` 组件。每个组件可保留多个脱敏地址断言，并逐条展示 `candidateRef/order/primary/sourceField/observedAt/expiresAtState/expiresAt`；`primary` 只是本次 Producer 建议。下载尝试绑定精确 `candidateRef`，并展示 startedAt、endedAt、terminal 及失败时的 failureReason。示例分别显示 still 取得、motion 失败、整体 `PARTIAL`。候选顺序不推断组件，不暴露原始 URL，也不把历史代次伪装成本次 Package 的并列来源组。
7. **读取成功但无匹配**：说明当前 query scope、排除/限制，不外推平台没有内容；Inspector 与选择状态同时清空。
8. **读投影失败**：明确未读取材料、无远程/旧系统 fallback、建议重试本机读取；Inspector、结果与选择状态互斥，不残留上一作品。
9. **筛选回执**：全部、部分取得、风险停止、字节已清理和处理中均真实改变可见作品、结果数量、当前选择与 read receipt；不能使用无行为的 enabled 按钮。
10. **未选择**：Inspector 显示 `SELECTION_REQUIRED`，不填充伪造详情。
11. **受限材料**：脱敏片段可见，原文因 display policy 不可见。

原型中的数量、作者、标题、ID、时间和片段均为合成内容。它验证设计与状态，不证明 API、数据库、平台或媒体链。

## 13. 验收矩阵

| 层 | 场景 | 验收方法 | 完成信号 | 不能证明 |
|---|---|---|---|---|
| 产品任务 | 选择作品并判断各 lane | PAGE + 原型走查 | 3 秒看状态、5 秒进入 Inspector | 运行时可用 |
| 状态诚实 | partial/risk/cleaned/processing/unknown/not requested/read error/Live Photo partial | 文案与 DOM 检查 | 没有总完成度、unknown→0、slot→bytes、ACK→complete；processing 不提前成功 | 数据真实 |
| 视觉 | 1280×800、1440×900、1536×960、1728×1117、1920×1080、2560×1440、390×844、430×932 | 实际浏览器 render + overflow/geometry 检查 | 无横向越界；重点与 LIDS 一致 | Mog 最终审美验收 |
| 可访问性 | keyboard/focus/landmark/Reduced Motion | DOM/键盘/媒体查询检查 | 控件可达、状态双通道、无 motion 依赖 | 辅助技术全量认证 |
| 技术呈现 | 字段/lane/provenance/sensitive boundary | 与 MEDIA-RECON 和卡 3 API 对照 | UI 无需从 Package/raw tables 猜测 | 后端已实现 |
| 真实后果 | 不适用，本卡无写动作 | diff 与网络检查 | 静态原型不读写真实服务 | 采集、下载、OCR/ASR |

### 13.1 当前页面实现证明

- 多材料 Evidence Library 的唯一产品主对象、状态词典、表面地图、信息架构和消费字段已冻结；
- 静态合成原型能覆盖正常、部分、未知、未请求、风险停止、处理中、字节已清理、多候选来源、Live Photo 组件部分取得、受限、空与读取失败；
- 页面沿用 LIDS 与既有 `Corpus Explorer + Split Evidence Inspector`，没有第二套视觉语言；
- 卡 3 已提供列表、`detailUrl`、详情、评论研究通道、媒体/派生/来源回执与受控本地 asset handle；卡 4 运行页默认只消费这些入口；
- 列表选择、Inspector Tab、评论通道继续读取、作品列表继续读取与 375px 顺序流均有运行代码和聚焦合同测试。
- Issue #110 的运行 UI 收口已在隔离只读实例以 13 个本机作品集合完成 1440×900 / 375×812 in-app Browser 检查：重复宣言和独立状态侧栏退出首屏，参考稿的 `SYSTEM VIEWS / MY VIEWS` 双区策略恢复，其中系统视图绑定五项真实 `view` 查询、我的视图明确未接通；表格标题/正文/状态分别提升到 14px、11–12px、10px，页面级无横向溢出；这仍不等于部署或 Mog 最终验收。

### 13.2 当前页面实现不证明

- 历史 Package 已回填、所有现有作品都有完整 lane，或受控本机数据库含足够材料覆盖全部设计场景；
- 媒体/派生/来源通道的下一页路由已经提供；当前回执截断但无 URL 时只显示 `SOURCE_INCOMPLETE`；
- 真实媒体字节、OCR、ASR、抽帧、embedding、清理或撤回传播；
- 真实平台、浏览器、账号、部署、性能、长期稳定性；
- Mog 已完成最终视觉/业务验收。
