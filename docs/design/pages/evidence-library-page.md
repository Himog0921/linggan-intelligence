# PAGE-EVIDENCE-001 · 多材料证据库

> 状态: 权威当前
> 最后核对: 2026-08-28
> 适用范围: `语料 → 证据库` 的产品任务、页面信息架构、技术呈现要求、状态与验收；运行时入口仍为 `http://localhost:3000/corpus/evidence`
> 事实来源: Mog 批准的五卡 Evidence Library 垂直交付、Issue #85、MEDIA-RECON-001、LIDS、UI execution contract、当前 Rust Discovery-only 页面/API 与现有 PostgreSQL 证明
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；本页规格不让静态原型冒充已接通运行时

本规格替代本文件 2026-08-25 的 Discovery-only 产品定义。旧定义仍准确描述当前运行时已经实现的窄能力，但不再代表证据库的目标产品职责。当前运行时与目标页面必须按下文分层表达，禁止把“产品已冻结”写成“多材料读模型或页面已经实现”。

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
- 五秒主动作：选择一篇作品，在右侧 Inspector 查看当前材料 lane、来源、Coverage、限制与允许的下一步。
- 成功标准：用户不查看数据库或插件日志，也能区分“没有请求”“没有观察到”“拿到一部分”“明确失败”“处理器未启用”“字节曾取得后清理”和“当前未知”。

### 1.3 页面非目标

本页不负责：

- 直接触发平台搜索、详情、评论或媒体采集；
- 在证据库内创建或批准 Work Order；
- 把一个含糊“补采”按钮连接到未知范围；
- 远程 CDN 媒体回退；
- 修改原始 Evidence、Package、Coverage 或来源时间；
- 把材料升级为 Observation、Topic、Claim、趋势或市场事实；
- 建立第二份正文、评论、媒体或作者真相；
- 暴露批量原始敏感评论、账号身份、Cookie、令牌或完整 Package payload；
- 把 checkpoint 当作语料或内容 Evidence；
- 在本页运行 OCR、ASR、抽帧、embedding 或 Agent 分析。

## 2. 当前运行事实与目标边界

| 层级 | 当前事实 | 页面必须怎样表达 |
|---|---|---|
| 产品与设计 | 本规格和静态原型已冻结多材料职责 | 可以作为后续后端/UI 实现合同，不得标为运行完成 |
| 当前运行时 | `/corpus/evidence` 主要读取已接纳 discovery，并有窄 `content_detail` 投影 | 继续标注 Discovery-only/窄详情边界，不得按原型显示未接通 lane |
| 评论/回复 | Package 可保存，当前多为 `retained_uninterpreted` | 不得把原始 Package 条数显示为可检索评论数量 |
| 作者资料 | 当前主要用于观察目标档案回填 | 不得假装已有统一作者材料投影 |
| 媒体槽位 | Slot/单 URI 有实现与合成 PostgreSQL 证明 | 槽位存在不等于字节已取得；多 URI、顺序、Live Photo 仍有限制 |
| 媒体字节 | 分块上传/Blob/本地路径有合成证明 | 仅本地受控副本可预览；真实平台字节未验证 |
| OCR/ASR 等派生 | Job/Event/Derivative 入口存在，provider 未启用 | `NOT_ENABLED`，不得显示 `PROCESSING` 或 `SUCCEEDED` |
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
| `EV-S03` | 查询与范围条 | 本地材料检索、时间视角、lane/状态/媒体/受限筛选 | Query scope / asOf / cursor | 触发平台采集或保存观察规则 |
| `EV-S04` | 结果上下文 | 结果数、匹配字段、排除/限制、读取水位 | Query receipt | 用总数证明平台总量或完整性 |
| `EV-S05` | 作品材料集合列表 | 比较作品身份、脱敏摘要、lane 状态、最近观察和主要限制 | Work-level material read model | 以 Package/Blob/Slot 作顶层行 |
| `EV-S06` | 当前作品 Inspector | 概览、评论/回复、媒体、派生、来源/血缘、限制 | Selected work envelope | 第二搜索器、AI 结论、无来源补值 |
| `EV-S07` | 反馈位置 | 查询无结果、读取失败、受限、部分、处理中、空态说明 | Read receipt / error envelope | Toast 替代必读错误或回执 |
| `EV-S08` | 窄屏控制 | 打开材料筛选、切换列表/Inspector、返回当前作品 | Local view state | 把三栏按比例缩小或隐藏关键状态 |

### 4.1 依赖地图与文件所有权

| 依赖 | owner | 本卡如何使用 | 本卡不得做 |
|---|---|---|---|
| `MEDIA-RECON-001` | 卡 1 / PR #84 | 消费材料、媒体、状态与来源语义 | 修改 identity、slot、bytes、derivative 或处置合同 |
| 多材料 read model/API | 卡 3 / Issue #86 | 冻结页面所需字段和查询语义 | 实现 Rust/SQL/API 或猜字段 |
| 运行时页面 | 卡 4 | 提供可直接实现的 PAGE/原型/验收输入 | 修改 runtime HTML/CSS/JS |
| 真实垂直证明 | 卡 5 | 提供必须验证的页面场景 | 运行真实平台、媒体或 OCR/ASR |
| Shell / LIDS Token | 共享 UI owner | 原型只消费现有规则 | 修改 `shell.rs`、`shell.css` 或全局 Token |

## 5. 信息架构

### 5.1 列表行的信息顺序

每个作品集合按下列顺序呈现：

1. 本地媒体预览，或准确的未取得/已清理/受限状态；
2. 平台、稳定作品引用、标题、作者、来源发布时间及精度；
3. 脱敏摘要或“尚无可展示摘要”；
4. 发现、详情、讨论、媒体、派生五组 lane；其中评论/回复和 OCR/ASR 仍可分别展开；
5. 最近观察时点、主要 Coverage/停止原因、限制；
6. 当前选择信号。

不得用一个“完整度 73%”代替 lane。列表不显示完整原始评论、URL、Cookie、完整 Package payload 或远程媒体地址。

### 5.2 Inspector 信息顺序

```text
当前作品 / 稳定 public ref
├─ 概览：标题、正文、作者、来源时间、逐字段来源/未知
├─ 评论与回复：脱敏片段、父子关系、各 lane Coverage/停止原因
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

保存视图、批量选择、发起研究和补采当前继续禁用，直到各自有独立产品/权限/回执合同。

## 6. 材料 lane 与数据来源

| Lane | 用户看到的材料 | 最小来源/字段 | 当前实现边界 |
|---|---|---|---|
| 发现 `discovery` | 从搜索面或作者页发现作品 | platform、content identity、入口、位置、observedAt、Coverage | 已有窄投影；不是详情 |
| 详情 `detail` | 标题、正文、作者、来源发布时间、互动快照 | 每字段 value/state/sourceRef/version | 当前仅窄投影，不是完整 Observation/Current |
| 评论 `comments` | 顶层评论脱敏片段、已知数量与覆盖 | comment refs、snippet、count state、Coverage、stop reason | 当前多为未类型化；无记录不得显示 0 |
| 回复 `replies` | 楼中楼父子关系与脱敏片段 | reply/root/parent refs、tree state、Coverage | 当前父子投影未实现 |
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
  display: title / creator / publishedAt + each value state
  preview: localAssetUrl / slotPurpose / bytesState / alt
  laneSummaries[]: lane / state / counts-with-value-state / stopReason / limitations / latestObservedAt
  summary: lastObservedAt / primaryLimitation / restrictionState / matchedFields
inspector:
  overview fields + sourceRefs
  commentThreads + commentsCoverage + repliesCoverage
  mediaSlots[] + origin generation + component/bytes/replica/derivative/disposition states
  derivatives[] + processorVersion + source location
  provenance: target/task/attempt/package/receipt/producer/station-account lens/coverage/checkpoint
  permissions + displayPolicy + limitations
```

计数必须带 value state。`null/UNKNOWN`、`0/KNOWN` 与 `N/A` 不得共用一个空值。所有列表和 Inspector 值均来自受控 read model，不允许 UI 自行聚合原始业务表或 Package JSON。

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

### 7.3 组合规则

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
| 保存视图/批量选择/研究/补采 | 当前无合同 | 无 | disabled + 开放条件 | 伪造成功 toast 或本地计数 |

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
| 查询条 | `Corpus Explorer` + LIDS Input/Quiet | 检索和筛选已接纳材料 | 平台搜索框、假保存视图 |
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
- `≤640px`：列表行不再保留横向缩略图列；预览进入标题下方，lane 使用两列网格；Inspector 作为同页后续区，提供“返回当前作品”锚点。
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
5. **读取成功但无匹配**：说明当前 query scope、排除/限制，不外推平台没有内容。
6. **读投影失败**：明确未读取材料、无远程/旧系统 fallback、建议重试本机读取。
7. **未选择**：Inspector 显示 `SELECTION_REQUIRED`，不填充伪造详情。
8. **受限材料**：脱敏片段可见，原文因 display policy 不可见。

原型中的数量、作者、标题、ID、时间和片段均为合成内容。它验证设计与状态，不证明 API、数据库、平台或媒体链。

## 13. 验收矩阵

| 层 | 场景 | 验收方法 | 完成信号 | 不能证明 |
|---|---|---|---|---|
| 产品任务 | 选择作品并判断各 lane | PAGE + 原型走查 | 3 秒看状态、5 秒进入 Inspector | 运行时可用 |
| 状态诚实 | partial/risk/cleaned/unknown/not requested/read error | 文案与 DOM 检查 | 没有总完成度、unknown→0、slot→bytes、ACK→complete | 数据真实 |
| 视觉 | 1280×800、1440×900、1536×960、1728×1117、1920×1080、2560×1440、390×844、430×932 | 实际浏览器 render + overflow/geometry 检查 | 无横向越界；重点与 LIDS 一致 | Mog 最终审美验收 |
| 可访问性 | keyboard/focus/landmark/Reduced Motion | DOM/键盘/媒体查询检查 | 控件可达、状态双通道、无 motion 依赖 | 辅助技术全量认证 |
| 技术呈现 | 字段/lane/provenance/sensitive boundary | 与 MEDIA-RECON 和卡 3 API 对照 | UI 无需从 Package/raw tables 猜测 | 后端已实现 |
| 真实后果 | 不适用，本卡无写动作 | diff 与网络检查 | 静态原型不读写真实服务 | 采集、下载、OCR/ASR |

### 13.1 本卡证明

- 多材料 Evidence Library 的唯一产品主对象、状态词典、表面地图、信息架构和消费字段已冻结；
- 静态合成原型能覆盖正常、部分、未知、未请求、风险停止、字节已清理、受限、空与读取失败；
- 页面沿用 LIDS 与既有 `Corpus Explorer + Split Evidence Inspector`，没有第二套视觉语言；
- 后续卡 3/4 可以按本规格实现 read model 与运行时 UI，而无需从视觉稿猜语义。

### 13.2 本卡不证明

- 多材料 API/read model、Rust/SQL、运行时 HTML/CSS 或交互已经实现；
- comments/replies/author/multi-origin/Live Photo 已完成类型化；
- 真实媒体字节、OCR、ASR、抽帧、embedding、清理或撤回传播；
- 真实平台、浏览器、账号、部署、性能、长期稳定性；
- Mog 已完成最终视觉/业务验收。
