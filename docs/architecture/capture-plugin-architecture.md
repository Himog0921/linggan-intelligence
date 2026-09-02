# 采集控制层、有限工位与浏览器插件架构

> 状态: 代码事实优先
> 最后核对: 2026-09-02
> 适用范围: DISC-001 Gate 6 的采集准入、服务端调度、Work Order、Attempt/lease、浏览器 MV3 插件、离线恢复、终态 Package、部分结果、协议升级与可观测性
> 事实来源: 已确认 Gate 2–3 边界、当前 Rust/PostgreSQL/插件代码与测试、已部署 Browser Producer `0.8.31`、候选 `0.8.32` 发行包、工位与真实 Package/Receipt/Materialization 回执
> 冲突时以谁为准: 用户最新确认、真实 producer fixture、协议兼容测试、PostgreSQL 并发副作用与实际插件运行结果；旧插件字段和本文候选参数不自动成为现行合同

本文是 [`module-architecture.md`](module-architecture.md) 中 Capture 模块与浏览器插件接缝的渐进披露子文档。已落地部分以当前代码和真实回执为准；未落地候选仍为草案，本文本身不授权新的平台访问、数据迁移或产品范围。

## 一句话结论

> **Linggan 与插件之间需要一个服务端采集控制层：它先复用证据、合并需求、判断 Coverage 缺口、时间价值、有限工位、账号风险和授权，再产生有界 Work Order；MV3 插件只领取、续租、执行、暂停、交付和确认，不决定研究意义、任务优先级或下一步扩采。**

它不是普通“先入先出任务队列”，但第一阶段也不引入 Temporal、Camunda、Kafka 或微服务。一个 PostgreSQL 16 durable-work 实现足以承载当前规模，前提是状态、幂等、租约、部分结果和数据库围栏有真实测试。

## `0.8.31` 运行基线与 `0.8.32` 调度唤醒候选

- 服务端拥有工位、安装身份、认领、TaskSpec、Attempt、Package 接纳、Coverage、Observation 和分析；插件只是受控 Browser Producer。
- 当前已认领运行安装是 `0.8.31`；旧安装只保留为历史运行事实，不能依据旧 launchd 静态字段判断正在执行的版本。
- 一次真实标准详情任务已将详情、媒体槽位、顶层评论和回复分成四个不可变 Package 并全部接纳；评论窗口为 `30/30 DETAIL_WINDOW COMPLETE`，页面公开数为 233。
- 标准详情和评论深采共用同一评论采集内核；新深采 Attempt 从评论入口重新开始，不续上一 Attempt 的“第 201 条”。
- 媒体自动执行使用持久 outbox 与 `linggan-media-worker-v1` offscreen 通道；旧的人工媒体选择/下载窗口继续保留，两类使用者不互相取代。
- 插件不选择研究目标、不生成搜索词、不分析趋势/需求/爆文；它只带回当前页面/平台事实、媒体原件、Coverage 和失败回执。
- `0.8.30` 已补上一个严格位于 **Attempt 之前** 的恢复接缝：任务已领取但页面窗口/标签页/就绪检查失败时，插件仅上传受控的本机失败码与幂等 `failureRef`；服务端追加 dispatch-failure 审计并原子把同一个 frozen TaskSpec 退回 `pending`，一分钟后再由服务端派发。它不新建 Attempt、Package、Receipt、Evidence 或来源错误正文，也不生成第二份任务。未持有 live claim、安装已换代或 failure identity 冲突时，服务端拒绝重排而不是猜测恢复。
- `0.8.31` 将服务端给出的重试节奏写成 MV3 可恢复的周期 alarm；每次 tick 仍按服务端最新返回值替换 alarm。真实 Chrome reload 随后证明，仅在异步 patrol 结束时写 alarm 仍不足以跨越 install/startup handler 后的 worker 回收。
- `0.8.32` 候选因此在 install/startup handler 内先写一分钟可恢复 bootstrap alarm，再报到并领取；首次完成的 patrol 立即以服务端最近 cadence 覆盖 bootstrap。服务端 `nextPollAfterSeconds=0` 被明确作为合法“可立即检查后续”的返回保留到背景层，最终仍受 Chrome 的一分钟下限。它不将任何状态页面或安装重载变成自动认领或未授权平台访问。

当前未完全验收的组合不得被这条真实链扩大：搜索连续滚动/不同筛选端到端、作者页结构化统计、真实非空评论图片仍按能力登记册的 `PARTIAL / SOURCE_INCOMPLETE / NOT_OBSERVED` 管理。

## 先区分四种责任面

```text
产品/研究面
持续观察、主动问题、Candidate 补证、深度建档
        ↓ 产生 Information Need

采集控制面（服务端）
复用、去重、准入、预算、风险、分批、租约、恢复决策
        ↓ 产生有界 Work Order

平台执行面（MV3 插件）
领取、打开真实页面、读取来源、记录运行事实、提交终态交付
        ↓ 产生 Capture Package

证据接入面（服务端）
身份/hash/权限/合同/原子接入、Coverage 与下游处理
        ↓ Accepted Evidence / failure facts
```

四个面不共享一张万能 `task` 表，也不能相互越权：

- 研究需要不能直接成为插件命令；
- 人的采集授权不等于已经有工位和账号执行；
- Work Order 完成不等于目标世界被完整观察；
- 插件成功回调不等于 Evidence 已被数据库接受；
- Evidence 被接受不等于可以形成趋势、需求或市场机会；
- Agent 的补证建议只产生 Acquisition Request proposal，不直接排队。

## 采集控制面的深模块责任

控制层对外只暴露少量有业务含义的操作，内部隐藏排队与并发复杂性。

### 输入

- 已有 Information Need 或预先批准的持续观察计划；
- 明确调用者、用途、Domain、时间范围和允许资源；
- 所需对象/入口、字段族、Coverage 目标和可接受新鲜度；
- 当前 Evidence、正在运行的工作、近期失败和风险状态；
- 当前工位、账号、插件版本和 lane 能力；
- 可版本化的时间价值与风险政策。

### 输出

控制层只能给出下列结果之一：

```text
REUSED
已有材料足够，不访问平台

MERGED
已有等价工作正在执行，登记新的用途等待结果

ADMITTED
准入并生成一个或多个有界 Work Order

DEFERRED
有价值但当前资源/窗口不允许，保存原因和下次复评条件

DENIED
越权、风险、用途不成立或价值不足，不创建工作

EXPIRED
等待期间信息价值已经低于执行门槛

DECISION_REQUIRED
需要扩大预算、改变观察面或使用敏感资源
```

`DEFERRED` 不是隐藏失败，`DENIED` 也不删除原始需求。它们必须能回答为什么没有占用工位。

### 准入顺序

第一阶段使用可解释规则树，不建立万能 AI 分数：

1. 当前用途是否有效且在委托范围内；
2. 已有 Evidence 是否满足这次字段、时间和 Coverage 要求；
3. 是否已有等价 Work Order/Attempt，可以合并等待者；
4. 是否属于有意复观测，而非同一天无意义重复；
5. 对象/入口现在是否仍有时间价值；
6. 当前缺口是否能由插件的某个已证明 lane 获得；
7. 所需工位、账号、访问量和风险是否在授权预算内；
8. 延迟到预计执行时间后是否仍值得；
9. 通过后才拆成有界 Work Order。

相关性不能单独触发深采。探索驱动任务可以没有既有正式 Topic，但必须使用单独、受控的探索预算。

## 有限资源是正式系统事实

### Station 与 Account 分责

工位回答“哪台浏览器执行”，平台账号回答“以哪个观察身份看到”。两者均可能改变可见性、风险和可比性，不能只保存一个 `worker_id`。

逻辑上至少记录：

- Station Identity、在线/暂停/退役状态；
- 当前插件版本和支持的 contract/lane；
- Account Identity 的受限引用、平台、登录/风险/冷却状态；
- Station 与 Account 当前绑定关系及有效期；
- 最近租约、失败、验证码/登录提示和人工暂停；
- 允许的 lane、并发和访问预算；
- 不把 Cookie、Token 或完整账号凭据写入任务、日志或普通数据库列。

账号不是可无限替换的“代理池”。跨账号调度可能改变搜索观察镜头；在真实实验确认前，不得为了吞吐量任意切换账号后继续同一可比序列。

### 调度目标

调度不是追求“队列最终清空”，而是在有限资源下减少信息机会损失：

```text
是否应采
由明确缺口、用途、探索预算和权限决定

何时应采
由延迟信息损失、截止时间、依赖和风险决定

由谁来采
由 lane 能力、账号镜头、版本、健康和授权决定
```

第一版不固化全局 `priority_score`。规则结果应保留分项理由，例如 `time_sensitive`、`coverage_gap`、`baseline_due`、`risk_cooldown`，以便后续从真实执行校准，而不是让一个不可解释分数决定平台访问。

## 采集 lane 与渐进式任务图

复杂目标必须由服务端逐阶段展开，插件不接收“把这个作者完整建档”之类无限任务。

### 第一阶段 lane 候选

| lane | 只回答什么 | 典型输出 | 明确不做 |
|---|---|---|---|
| `search_discovery` | 某搜索入口这次展示了哪些候选 | 卡片/顺序/入口/页或 cursor/Coverage | 自动打开所有详情 |
| `author_profile` | 作者主页当前可见资料 | 身份候选、资料来源、观察时间 | 扫全部作品 |
| `author_index` | 当前作者页发现哪些作品链接 | 唯一链接、发现顺序、页级 Coverage | 打开详情和评论 |
| `content_detail` | 指定内容当前详情是什么 | 正文、来源时间、作者关系、指标、媒体候选 | 无限加载评论 |
| `comment_index` | 指定内容发现哪些一级评论 | 稳定身份候选、顺序、页级 Coverage | 自动展开全部回复 |
| `reply_index` | 指定根评论下发现哪些回复 | root 关系、回复、分页 Coverage | 跨根评论无限遍历 |
| `metric_reobserve` | 在明确时点复观测哪些指标 | 同一对象的新 Metric Observation | 重新采全部正文 |

lane 名称和字段仍须由 Gate 4 真实 producer 结果确认；这里冻结的是“发现—详情—评论—回复分段”，不是具体平台 selector 或接口。

### 深度建档示例

```text
Observation Objective: 跟踪作者 A
        ↓
Author Profile Work Order
        ↓ terminal package accepted
Author Index Work Order（有界页数/链接数）
        ↓ terminal package accepted
服务端去重、复用、评估价值
        ↓
Content Detail batches（每批固定上限）
        ↓
只对有明确缺口的内容产生 Comment Work Orders
        ↓
只对被选中的根评论产生 Reply Work Orders
```

每一阶段结束后，控制层根据新 Evidence 重新判断下一步。不能在第一步就预生成几千个不可撤回任务，也不能让插件根据页面内容自行扩大范围。

### 同一详情页的执行复用

`content_detail → media_slots → comments → replies` 是四个接纳与追溯边界，但不要求浏览器为同一作品重复打开四次页面。固定作品 WorkOrder 已经冻结上述范围时，服务端可在首个 `content_detail` 派发中返回一个短寿命的页面会话计划：作品身份、已批准 lane、评论上限、回复展开上限与缓存 TTL。

插件只在这个计划内调用成熟的单篇详情采集能力，一次读取详情、媒体候选和有界评论树，并把页面结果按 `Lease + 作品` 存入 MV3 可恢复缓存。它不得提前为尚未领取的 lane 提交 Package；后续任务仍逐个领取自己的不可变 TaskSpec，再从缓存形成对应的单能力 Package、Attempt 与 Receipt。缓存过期、身份不符、lane 未批准或数据不存在时，只能回退原有单 lane 执行，不能自行补 lane 或扩大评论范围。

因此这里复用的是昂贵的页面读取，不合并以下事实边界：

- WorkOrder 中的顺序 Step；
- 每个 Task 的能力和目标；
- Attempt 与执行权；
- Package、Coverage 和 Receipt；
- 媒体字节取得与后续 OCR/ASR。

## Work Order 的边界

Work Order 是 producer 可以理解的最小有界命令，不是产品研究目标。逻辑上必须包含：

- 唯一 Work Order identity 与 contract version；
- lane 和目标平台；
- 明确目标/入口及其稳定引用；
- 允许读取的字段/slot；
- 页数、对象数、回复根数、运行时长等硬上限；
- 已授权账号镜头或可接受账号集合；
- 风险和立即停止条件；
- Coverage 目标及单位；
- 允许的 checkpoint/resume 语义；
- 截止时间/执行前重新评估要求；
- 结果交付、隐私和原料政策；
- 不包含业务秘密、Cookie、Token 或自由文本“尽量多采”。

一个 Work Order 可有多次 Attempt。Retry 是否产生新 Attempt 取决于执行权和观察连续性，而不是代码异常类型：

- 短暂网络发送重放同一冻结 Package，不是新 Attempt；
- 在同一有效 lease、同一执行身份和允许的短暂停顿内恢复，可继续同一 Attempt；
- lease 丢失、换账号/工位、身份或观察条件改变、终态已形成后重做，必须新 Attempt；
- 新 Attempt 不得继承并改挂旧 Attempt 尚未接纳的记录或 checkpoint 成果；只能重新观察或通过明确 recovery-import 路径处理旧冻结材料。

“允许同一 Attempt 恢复多久、authority fence 以哪个时点为准”仍是 Gate 6 待确认技术决定，必须由并发实验而非文档直觉决定。

## Attempt、lease 与唯一执行权

### 状态责任

状态名可以在实现时调整，但语义至少需要区分：

```text
Work Order
ready / leased / waiting / satisfied / expired / cancelled

Attempt
started / active / paused-risk / terminal-submitted / accepted / rejected / lost-authority

Ingress Delivery
received / replayed / conflicted / rejected / accepted
```

不要用一个 `SUCCESS/FAILED` 同时表示任务、运行、交付和 Evidence。

### claim

服务端在一个短 PostgreSQL 事务内：

1. 选择当前 Station/Account/版本有资格执行的 Work Order；
2. 再次检查过期、暂停、预算、风险和依赖；
3. 建立新的 Attempt、Capture Identity 与 lease epoch；
4. 返回有界合同；
5. 同一 Work Order 不得并发产生两个有效执行权。

### renew

- renew 只延长当前执行权，不改变 Work Order、Attempt、Capture Identity、lane 或观察镜头；
- 风险暂停可以保留短期本地状态，但服务端可拒绝续租；
- 续租失败后插件必须停止新的平台访问；
- 失去执行权不代表已经观察到的材料自动无效，但提交能否接受必须经过原子 authority fence；
- 插件不能通过修改本地时钟延长权力。

### reconcile

插件启动或恢复时先向服务端报告本地未决 run/outbox 摘要，再获得逐项决定：

```text
CONTINUE_CURRENT_ATTEMPT
当前权力仍有效，可继续

SUBMIT_FROZEN_PACKAGE
不得再访问平台，只可重传已经冻结的终态包

EXPORT_FOR_RECOVERY
服务端不接受普通执行路径，转人工受控恢复

DISCARD_RUNTIME_STATE
没有可接纳原料，只清除运行辅助状态

STOP_AND_ESCALATE
身份冲突、风险或版本不兼容
```

禁止插件扫描“所有 pending 任务”自行认领；`reconcile → claim → renew → submit/ingest → acknowledge` 是唯一执行链。

## 一次 Attempt 与一个终态 Package

第一版维持：

```text
Work Order 1 ── N Attempt
Attempt    1 ── 0..1 terminal Capture Package
Package    1 ── N Record / Artifact / Coverage Fact
Attempt    1 ── 0..N Ingress Delivery Occurrence
```

这里需要区分“多个网络请求”和“多个业务包”：

- 大结果可以在传输层分片/流式上传；
- 同一冻结 Package 可以多次提交；
- 每次接收都留下 Delivery receipt；
- 但同一 Attempt 的终态业务内容只有一个 canonical manifest/hash；
- 同 Capture Identity + 同 hash 是 replay；同身份 + 不同 hash 是 conflict；
- terminal Package 形成后不可再追加第 51 条或修改 Coverage。

如果未来真实 payload 大到一个 terminal manifest 不可行，应先用实际体积、失败率和对象存储证明，再引入“manifest 指向多个不可变 chunk”；这仍是一个 Package，而不是把 Attempt 变成不断生长的事实容器。

## 部分结果：入库资格与目标完成分开

这是 2026-08-20 经用户确认的 `USER-DEC-02`。它不只回答“50 条能否保存”，而是同时分开保存、Evidence 接入、下游处理、用途适用和 Claim 推断：

> **目标是 100 个同单位对象、实际只观察并通过最低来源合同的 50 个时，这 50 个可以进入终态 Package 并分别接受为 Evidence；Attempt/Work Order 仍必须如实标记目标未完成，另外 50 个不能被推断为不存在。**

例：

```text
target:          100 note details
discovered:      100 note identities
attempted:        70 note details
emitted:          50 note records
failed:           20 note details
not_attempted:    30 note details
terminal_reason:  risk_control
```

只有单位相同、集合关系有来源时才允许这样的算术。`unknown` 不是用 `target - emitted` 计算出来的万能桶。搜索卡片、详情 Record、评论 Record 和页面访问次数不能放在同一行相减。

### 目标数量必须带目标语义

同样写“100”，业务含义可能完全不同：

| 目标语义 | 取得 50 条后允许表达什么 | 不能自动表达什么 |
|---|---|---|
| 已知对象集合 | 若 100 个稳定目标身份在执行前已冻结，可以区分已尝试、失败和已知未尝试成员 | 未访问对象不存在 |
| 最大配额 | 本轮最多希望取得 100 个，目前取得 50 个 | “还有 50 个现实对象未尝试”或完成 50% |
| 来源穷尽目标 | 当前观察到 50 个并按真实终止事实结束 | 除非 producer 证明来源穷尽，否则平台总量是 50 |
| 时间预算 | 允许的执行时间已经用完，已得到 50 个 | 内容观察范围完整 |
| 风险预算 | 风险停止条件已满足，已得到 50 个 | 业务目标完整或平台已耗尽 |
| 探针目标 | 50 个可能已经足以回答本次能力探查 | 这些材料可代表市场 |

第一版不要求把上表直接做成一个万能 `target_mode` 枚举，但 Work Order 合同必须能说明目标集合/上限、计数单位、停止条件以及哪些剩余成员是来源已知、哪些仍 unknown。只有已知集合才允许写具体 `not_attempted` 成员；配额差额不能伪造成已知缺失对象。

### Package 硬门必须保持最小

为了避免“一条平台字段异常导致其余 49 条合格材料连坐丢失”，Package 接入硬门只覆盖真正的包级责任：authority、Attempt/Capture Identity、目标/合同、canonical/hash、manifest/成员边界，以及每个成员最小 Record/Artifact envelope 是否可安全枚举和重放。标题、互动数、作者字段、评论关系等平台语义尽量作为原始 payload/Artifact 后续解析，不提升为整包硬门。

如果 Package 本身无法验证身份、权限、hash 或成员边界，它不能成为 Accepted Evidence；系统只保存最小安全失败记录，完整原始载荷是否短期隔离仍服从 producer、隐私和保留合同。项目不能承诺“所有任意字节永久保存”。

Package 一旦被接纳，其中可枚举的 Record/Artifact envelope 和原始引用成为不可静默覆盖的 Evidence。随后每个 Record 独立追加处理回执，例如解析成功、复用既有 Source Identity、身份 unresolved、来源关系冲突、隔离或可重试。处理结果不回写 Package 成员，也不要求在 Package 接入事务内全部完成。

需要特别避免三个名词误用：

- 网络 replay 是同一 Capture Identity/hash 的交付事实，不是 Record 处理状态；
- 多次发现同一现实对象应复用 Source Identity 并保留 Discovery/使用关系，不是丢弃“重复数据”；
- quarantine 是有原因、有访问和生命周期边界的下游处理结果，不是永久堆放任意原始内容的垃圾桶。

### 这 50 条可以证明什么

- producer 在记录的时间、账号/工位、lane 和观察条件下返回了这些材料；
- 其中通过身份、来源和合同的 Record 可以进入后续 Observation；
- 可以用于证明“捕获材料中存在这些表达/对象”；
- 在当前用途资格允许时，可以参与 Corpus、探索或具体对象研究。

### 这 50 条不能自动证明什么

- 平台只有 50 条；
- 未取得的 50 条不存在；
- 这 50 条代表完整搜索结果或总体用户；
- 任务已完整成功；
- 可以与另一轮 100/100 数据直接做市场增长比较；
- 采集失败不会产生选择偏差。

### 入库后的使用规则

不能给 Evidence 写一个永久 `usable = true/false`，也不能让 `Attempt = partial` 自动禁止或允许全部用途。每次使用按照当前问题重新判断：

| 使用层 | 部分结果可以做什么 | 仍需什么边界 |
|---|---|---|
| 来源/对象 | 查看被接纳原料；为身份、时间、来源和对象状态合格的成员形成 Observation | 不为未尝试/unknown 对象制造空 Observation、0 或不存在 |
| Corpus/检索 | 经用途、隐私和访问评估后选择原声、片段、场景或表达；保留 Evidence 引用 | 入库不等于自动进入 Corpus，更不等于永久可展示/外传 |
| 集合内描述/探索 | 明确说“本次实际合格输入中出现了什么”；提出 Candidate Term/Topic/Signal/Content Idea | 固定实际输入、去重口径、来源集中度、定义/模型版本和限制 |
| Claim/趋势/市场 | 只有满足具体 Claim 的 Coverage、可比性、时间、分母、来源分散、反例和权限要求后才能升级 | 不能从已入库直接继承总体、增长、需求或行动资格 |

一条高价值原声即使来自不完整批次，仍可以支持一个有来源的候选选题；但“内容启发价值高”“市场普遍性高”“需求正在增长”是三条不同主张。系统必须允许前者，不得偷偷继承后两者。

### “每次采集都有价值”的精确定义

项目接受用户提出的方向，但不把它误写成“任何返回字节都永久保存并可用于分析”。一次 Attempt 即使没有达到内容目标，仍可能产生三类不同价值：

1. **来源材料价值：** 实际取得的笔记、评论、作者、指标、媒体候选或页面/API 原料，按 Package/Evidence/Record 合同处理；
2. **Coverage 价值：** 实际尝试、成功、失败、未尝试、unknown、入口、时间和停止原因，限定后续解释；
3. **运行与 producer 价值：** 页面变化、登录/账号状态、插件解析失败、接口能力或风险停止，为插件和调度策略提供有来源的运行事实。

即使得到 0 条有效内容，后两类价值仍可能存在；但它们只能证明本次执行或 producer/平台表现，不能变成“现实没有内容”。完整原始字节是否保存、保存多久和谁可读取，仍受隐私、安全、成本和生命周期合同约束。

### 补采与跨批组合

后续补采形成新的 Attempt、Capture Identity、终态 Package 和 Observation，不修改第一次 Package。多个 Package 要共同参与分类、聚类、统计或 Agent 研究时，使用现有的 `Analysis Input Set` 或用途化 `Material Pack` 固定成员、版本、去重、时间、入口和 Coverage；不新增一个与它们重叠的 `Material Set` 真相。

两个批次的记录数不得直接相加成唯一对象数或完成度。相同 Source Identity 可能是重复发现，也可能是有意复观测；它们分别保留来源关系和 Observation 时间，再由具体分析定义计数单位。

数据库事务上的“Package 原子接入”仍表示：同一 terminal Package 的 manifest、Record、Artifact、Coverage、receipt 和 durable work 要么共同提交，要么共同回滚。它不要求平台目标 100% 完成，也不要求所有 Record 在下游解析成功。

## MV3 插件内部边界

插件继续使用 TypeScript 和 Manifest V3，因为它运行在浏览器平台，不属于 Rust 产品运行时。建议结构按职责切开，而不是形成巨型 `background.ts`：

```text
src/
  background/
    lifecycle/
    reconcile/
    lease/
    outbox/
    messaging/
  content/
    shared/
    xhs/
      discovery/
      author/
      content/
      comments/
      risk/
  protocol/
    versions/
    validation/
    canonicalization/
  runtime/
    run-store/
    checkpoint/
    package-builder/
  popup/
    station-status/
    recovery-actions/
  diagnostics/
```

这只是责任示意，不授权重构现役插件目录。

### Background service worker

只负责：

- 身份/版本协商；
- reconcile、claim、renew、submit、acknowledge；
- 创建和终止有界本地 run；
- 持久 outbox 与浏览器重启恢复；
- 在 content script、popup 与服务端之间校验消息；
- 不保存长期业务知识，不执行 Topic/AI 判断。

MV3 service worker 随时可能休眠，因此内存变量不能代表 lease、checkpoint、outbox 或终态。恢复所需的最小状态进入 IndexedDB/Chrome storage；敏感凭据使用现役授权机制的后续安全设计，不落普通日志。

### Content script / platform adapter

只负责：

- 确认当前页面/响应确实属于 Work Order 目标；
- 在 lane 合同范围内读取来源材料；
- 保留原始表示、字段来源、观察时点和风险事件；
- 返回结构化候选 Record/Artifact/Coverage fact；
- 达到上限、终止条件或风险时停止。

平台 adapter 不能自行：

- 新增搜索词、作者或目标；
- 把缺失值补成业务 0/false/`normal`；
- 合成稳定 Source Object identity 后假装为平台 ID；
- 把启发式停止称为 `source_exhausted`；
- 把 run 结束时间批量写成每条材料的真实观察时间；
- 因为 selector/API 变化而切换到未经合同批准的隐蔽抓取路径。

### Popup

Popup 是轻量执行与恢复入口，不是第二工作台。第一阶段只需要让操作者看到：

- 当前工位/账号是否有资格接单；
- 当前 Work Order 的人类可读摘要和硬上限；
- 正在运行、风险暂停、待提交、冲突或需要恢复；
- 停止、导出受控恢复包、重试提交等有限动作；
- 不在 Popup 中编辑研究问题、Topic、采集优先级或 Evidence。

## 本地恢复状态不是第二事实源

IndexedDB run、checkpoint、journal 和 outbox 的用途不同：

| 本地状态 | 用途 | 不能证明 |
|---|---|---|
| Run state | 浏览器重启后知道当前执行到哪 | 服务端执行权仍有效 |
| Checkpoint | 避免在同一合法 Attempt 内重复页面动作 | 观察集合完整或前后可比 |
| Journal | 诊断实际产生过哪些本地产物 | Evidence 已接受 |
| Outbox | 对冻结 Package 做可靠重传 | 可以继续修改 Package |
| Recovery export | 普通接入失败后的人工受控材料 | 自动归入新 Attempt |

服务端 receipt 和 PostgreSQL 副作用才是 Package 是否接受的权威。插件收到 `acknowledge` 后才可清理相应冻结 outbox；清理失败不会反向撤销服务端事实。

## 协议与版本演进

### Contract envelope

跨边界消息至少需要：

- protocol/contract version；
- Work Order、Attempt、Capture Identity、lease epoch；
- Station/Account 的受限引用；
- lane/profile/version；
- producer build 和 platform adapter version；
- observed/reference/started/finished/submitted times 及来源；
- canonical manifest/hash；
- Record/Artifact/Coverage/terminal sections；
- privacy handling label；
- runtime validation result。

字段清单仍须由真实 fixture 定稿。Rust 和 TypeScript 都必须做运行时验证，静态类型不能替代边界校验。

### 兼容策略

- 服务端公布允许的最小/最大协议与插件版本；
- 不兼容时明确拒绝领取或接入，不能静默降级字段；
- 新增可选字段不能改变旧字段语义；改变身份、时间、Coverage 或终态语义必须新 major contract；
- 每个支持版本都必须有固定 JSON fixture、canonical/hash 金样和正负向兼容测试；
- 插件升级采用服务端门槛与分阶段启用，不能只看 manifest 版本号；
- 源码、打包产物、工作台下载包和真实已加载浏览器版本分别验证。

## 错误与停止语义

错误至少按后果分组，不能归成一个 `failed`：

| 类别 | 示例 | 后果 |
|---|---|---|
| 目标/身份错误 | 打开错误 note/author、内部关系冲突 | 立即停止；相关 Record 不进入有效对象链 |
| 执行权错误 | lease 失效、epoch 过期、重复执行者 | 停止新访问；提交经 fence/recovery 判断 |
| 平台风险 | 验证码、登录失效、异常提示、限流 | 风险暂停；不自动切号或绕过 |
| 来源不完整 | 只取得部分页/评论/媒体 | 保存合格部分 + Coverage + 终止原因 |
| 合同错误 | 字段类型、必需身份、版本不合格 | 原料按政策隔离/失败记录；不生成有效 Observation |
| 重放 | 同 Capture Identity + 同 hash | 返回原 receipt，不重复 Evidence |
| 冲突 | 同 Capture Identity + 不同 hash | fail closed，人工调查，不选“最新” |
| 传输失败 | 超时、网络断开 | 重传同一冻结 Package，有限重试 |
| 解析器异常 | 单 Record 可预期拒绝或程序崩溃 | 接入后按 Record/Object 隔离；意外异常不能冒充处理完成 |
| 隐私阻断 | 授权撤回、处置生效 | 停止访问/提交/输出并触发传播 |

所有自动重试必须有次数、总时长、退避和最终 stop reason。没有静默换账号、换 lane、换 selector、扩大页数或降低合同要求的“降级成功”。

## 安全和隐私

- Manifest 权限按平台和功能最小化；新增 host/Chrome 权限需要单独审查与发布说明；
- content/background/popup 消息验证来源、类型、目标和 schema；不执行自由脚本消息；
- 生产包采用严格 CSP，不引入远程可执行代码；
- 服务端认证与插件设备授权分开，设备授权不代表材料用途授权；
- 日志不得保存 Cookie、Token、完整受限原文或可恢复的账号凭据；
- 原始 Artifact 是否保留、保留位置、期限和加密策略由 Gate 5 隐私参数决定；
- recovery export 视为高敏材料，必须有显式人工动作、加密/访问记录和到期处置；
- 外部 Agent 永远不通过插件通道获得平台凭据或直接发工作单。

本架构不提供规避平台限制、验证码或风控的技术。遇到风险的正确行为是停止、冷却、报告和重新授权。

## 可观测性

不依赖日志猜链路。每个阶段需要相关但不混淆的可查询事实：

```text
Need/Request/Auth/Admission
        ↓ why this visit exists
Work Order/Attempt/lease
        ↓ who had authority
Plugin run/checkpoint/risk event
        ↓ what execution did
Package/Delivery/Ingress Receipt
        ↓ what was submitted and accepted
Record processing/Observation/Current
        ↓ what became usable world observation
```

最小运营指标候选：

- 需求复用率、合并率、拒绝/延期/过期原因；
- 各 lane 排队时间、信息截止前完成率；
- Station/Account 有资格时长、风险暂停、版本不兼容；
- Attempt 数、lease 丢失、恢复、终态原因；
- Package replay/conflict/rejection/acceptance；
- 同单位 Coverage 分布与部分结果比例；
- 从 Package accepted 到 Observation/Current 的处理水位；
- 实际平台访问数，而不仅是任务数。

这些指标用于运维和校准，不自动证明市场 Coverage 或业务成功。

## 建议代码边界与无巨型文件门禁

首个实现切片若进入代码，Capture 逻辑模块内部建议按责任组织，而不是一个 `capture.rs`：

```text
capture/
  admission/
  work_orders/
  attempts/
  leasing/
  ingress/
  recovery/
  coverage/
  protocol/
  durable_work/
  observability/
```

每个目录以一个小公共 facade 隐藏私有状态机、SQL 和校验。API route、worker loop 和插件 adapter 只调用 facade；不能各自复制 lease、hash 或终态判断。

沿用 [`module-architecture.md`](module-architecture.md) 的候选门禁：生产文件 350 行提示、500 行硬门；函数 60/100 行；公共项 15/25 个；`main.rs/lib.rs` 只做组合。例外必须有有期限 ADR，而不是加一句 `allow`。

## 可证伪测试矩阵

### 纯合同与 deterministic 测试

1. TS 与 Rust 对同一 fixture 得到相同 canonical bytes/hash；
2. unknown/缺失不会被补成 0、false、`normal` 或 `source_exhausted`；
3. 相同 Capture Identity + 相同 hash 返回 replay；不同 hash 必须 conflict；
4. 目标、lane、身份、单位或版本不匹配时失败关闭；
5. 终态 Package 形成后再次添加 Record 必须失败；
6. 已知 100 个目标的部分结果 fixture 保留合格 50、失败 20、已知未尝试 30 和真实终止原因；最大配额 100、取得 50 的 fixture 则保持剩余来源范围 unknown；
7. 一个已接纳 Package 内的逐 Record 解析、身份复用、unresolved/隔离结果可以不同，不撤销其他成员，也不把对象复用记录成网络 replay；
8. checkpoint 不改变冻结观察成员、终态或服务端 authority；
9. Agent proposal 不能直接生成 Work Order。

### 真实 PostgreSQL 16 并发测试

1. 两个工位同时 claim 同一 Work Order，最多一个获得有效 Attempt；
2. renew 与取消/过期竞争时，只有一个 authority 结果；
3. authority 在验证和提交之间变化时，旧执行者不能写入 Evidence；
4. 同 Package 并发提交只产生一份 Evidence 和一个权威 receipt；
5. 冲突提交不选择最后写入；
6. Package 事务失败不留下半份 Record/Coverage/durable work；
7. Package 接受后单 Record 解析失败不撤销其他已接纳 Evidence；
8. 新 Attempt 不能改挂旧 Attempt 未接纳材料；
9. 隐私阻断与 submit 并发时，处置规则不会被旧授权绕过。

### 插件集成测试

1. service worker 休眠/重启后先 reconcile，不重复访问或越权继续；
2. 离线时冻结 Package 入 outbox，联网后重传同一 hash；
3. receipt 已成功但 acknowledge 丢失时，重传得到 replay 后安全清理；
4. lease 失效后 content script 停止新的平台访问；
5. 风险事件不会自动切号、绕过或继续滚动；
6. 搜索、作者索引、详情、评论、回复 lane 不互相偷做下游访问；
7. 版本不兼容能在接单前和接入时分别拒绝；
8. 打包产物、下载包和真实加载版本都能报告同一 build/contract identity。

### 真实 producer 门

`0.8.28` 已通过一个受控单工位标准详情真实门，但该结果仅对当次样本与 lane 成立。fixture 成功仍不能替代搜索跨筛选、作者页、账号镜头、真实非空评论图片或其他尚未观察组合的单独证明。

## 插件升级顺序（后续新升级仍适用）

插件升级不能先于服务端合同，推荐顺序：

1. 确认 Gate 4 七条 producer 约束和最小真实实验授权/冻结范围；
2. 确认本文中 partial-result、resume/fence、账号镜头和隐私参数；
3. 用固定 V2 fixture 建立现役行为兼容基线；
4. 先实现 Rust/TS protocol validator 与 deterministic contract tests；
5. 在服务端实现最小 claim/renew/submit/receipt 与 PostgreSQL 围栏；
6. 使用 synthetic/deidentified fixture 证明端到端数据库副作用；
7. 再改插件 adapter，并保留现役可回退发布物；
8. 小范围真实工位验证，不自动扩到全部账号；
9. 核对源码、构建包、下载包、已加载浏览器版本和服务端版本门；
10. 真实 Evidence/Observation/Coverage 与失败副作用共同通过后才宣布新链路可用。

## 已确认方向与后续真实插件决定

`USER-DEC-01/02/05` 已关闭来源纪律、部分结果和总体技术方向。当前 `0.8.28` 已在一个受控样本上证明标准详情/媒体/评论窗口链；恢复窗口、账号镜头、Artifact 处置和尚未验收 lane 仍需各自的 producer/隐私/现实授权证据，不能由该样本外推。

| 编号 | 决定 | 当前推荐 | 为什么不能静默决定 |
|---|---|---|---|
| `DEC-G6-CAP-01` | 50/100 合格部分是否进入 Evidence | **已确认：** 进入；目标完成与 Record 资格分开，具体用途再审查 applicability | 已由 `USER-DEC-02` 收口；SCOPE-001 负责用 F-02/F-09/F-10 和 PostgreSQL 副作用证明 |
| `DEC-G6-CAP-02` | 同一 Attempt 的恢复有效边界 | 仅同 lease/capture/镜头且未终态的短期恢复；其他新 Attempt | 影响去重、Coverage 和观察时间 |
| `DEC-G6-CAP-03` | authority fence 精确时点 | Package 接入事务内再次验证 epoch/状态/授权 | 影响旧执行者能否写入不可变 Evidence |
| `DEC-G6-CAP-04` | 账号镜头是否可替换 | 默认不可任意替换；明确非比较任务才按授权候选调度 | 搜索结果可能因账号不同而改变 |
| `DEC-G6-CAP-05` | 原始 Artifact/恢复包保留 | 真实实验前单独确认期限、位置、加密与删除传播 | 涉及敏感原文和磁盘治理 |
| `DEC-G6-CAP-06` | 第一版 lane 的最终合同 | 以真实 producer audit/fixture 为准，先保留分段原则 | 当前字段、分页和终止仍有 SOURCE_INCOMPLETE |

## Gate 6 Capture/Plugin 退出条件

1. 研究需求、授权、准入、Work Order、Attempt、Package、Evidence 与 Observation 不再混用；
2. 有限工位/账号、时间价值、Evidence 复用、等价需求合并和探索预算进入服务端控制层；
3. 插件没有任务优先级、研究意义、正式知识或自动扩采权；
4. 发现、作者索引、详情、评论、回复和指标复观测是有界 lane；
5. `reconcile → claim → renew → submit/ingest → acknowledge` 是唯一执行链；
6. 部分结果可以保留但不伪造目标完成、总体 Coverage 或不存在；
7. terminal Package、重放、冲突、传输分片、恢复和数据库围栏语义明确；
8. MV3 休眠、离线、风险暂停、版本不兼容和恢复都有失败关闭路径；
9. TS/Rust 合同、真实 PostgreSQL 并发、插件集成和真实 producer 分层验收；
10. 未引入第二工作台、第二任务真相、无限任务、静默降级、风控绕过、Temporal/Kafka 或巨型文件。
