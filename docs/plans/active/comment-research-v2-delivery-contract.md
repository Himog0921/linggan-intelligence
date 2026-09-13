# 评论研究 V2：从用户原声到可追溯问题观察

状态：`DRAFT — 已进入实现`
负责人：Codex
实现分支：`codex/comment-research-v2-foundation`
关联：#190、#167
更新：2026-09-13

## 1. 当前事实与本包边界

当前 Rust 仓库是 greenfield bootstrap，不包含旧工作台的评论研究运行代码、数据库 migration、API 路由、页面或真实数据。`references/` 是只读来源，不是运行依赖，也不构成已经完成的 Rust 能力。

因此本计划不把旧项目曾经出现过的“评论研究”“模型配置”“3000 页面”当成当前实现事实。它将已确认的产品决定转换为新项目的领域合同、可测试实现包和页面验收条件。

本包最终目标是让用户完成这一条工作流：

```text
看用户原声 → 理解语境 → 启动有限的自动研究 → 查看稳定用户问题 → 判断观察到的变化 → 必要时追溯运行过程
```

它不负责选题生产、内容制作、发布管理、自动业务决策或人工逐条标注。

## 2. 用户可见的信息架构

评论研究只保留五个视图；运行实现不得为内部 Agent、Skill、Fingerprint、Cluster 管理另加导航。

| 视图 | 用户进入时要回答的问题 | 主操作 | 不应该出现的内容 |
| --- | --- | --- | --- |
| 概览 | 当前用户讨论中有什么值得继续看？ | 打开问题、原声或变化 | 用孤立评论量宣称市场需求；没有证据的趋势摘要 |
| 用户原声 | 用户到底说了什么，发生在什么内容和回复链里？ | 搜索、筛选、分页、打开详情 | 手动凑批、技术任务状态、无意义的纯 emoji/@ 噪声 |
| 用户问题 | 大量表达背后反复出现哪些尚未解决的困惑、障碍、选择和需求？ | 查看定义、覆盖、代表原声与相邻问题 | 每次重跑就换 ID 的标签云；黑箱总分 |
| 变化观察 | 在可比较的范围内，哪些问题新增、增强、减弱或分化？ | 查看比较口径和证据 | 把系统采集时间伪装成用户发表趋势 |
| 运行记录 | 系统这次实际拿到什么、做了什么、为何失败？ | 查看批次、输入快照、输出校验、重试和失败原因 | 把技术日志放入日常研究主流程 |

「研究设置」和「开始研究」是页面级动作，不是第六个视图。

## 3. 最终用户流程

### 3.1 用户原声

原声浏览器是评论语料的基础。它必须支持全文检索、研究状态筛选、来源作品筛选、可控分页和详情抽屉。每行展示评论文本、来源作品、点赞、可用的评论发布时间或其精度、用户语言的研究状态；不直接展示内部 `RunItem`、`Atom` 或 fingerprint。

详情抽屉按如下顺序组织：当前评论、父/根评论链、所属作品的相关片段、互动与时间事实、最近一次研究结论、可展开的证据引用与运行入口。作者回复只作为理解语境的辅助材料，不进入用户问题的评论人数或覆盖分母。

### 3.2 开始研究

用户不选择单条评论，也不记忆哪些已经处理。点击后先读取一个**无副作用计划**：

```text
当前可读原声
├── 首次研究
├── 上轮未完成、可以继续的原声
├── 可有限重试的暂态失败
└── 当前研究规则下已完成的原声

本次自动选择
├── 处理上限与预计成本
├── 来源作品分布
├── 研究版本
└── 本次不会处理的完成/合同型失败原因
```

用户只可调整明确的范围与上限，然后确认启动。计划查询不得认领任务、创建 run、冻结输入、调用模型或写入数据库。

### 3.3 研究完成后

系统自动执行确定性清洗、输入组装、模型结构化理解、字段校验、语义组织和问题归并。用户直接查看问题及其证据；并不承担逐条审核、手动聚类或重命名的日常工作。

连续自动研究默认关闭。只有一个有限真实批次已证明输入、输出、校验和归并稳定后，才可另行设计并授权开启排程。

## 4. 三层对象与状态边界

### 4.1 Comment：事实层

`Comment` 是稳定的外部身份；`CommentObservation` 是不可变的每次观察事实。身份首期固定为：

```text
workspace + platform + note_source_id + comment_source_id
```

同一身份且研究正文 hash 不变是 replay；正文或已观察事实变化形成新的 Observation。点赞与观察时间是事实历史，不覆盖旧 Observation。原文永不由 AI 改写。

评论发表时间必须保存原始文本、解析值和精度。只有有可比解析时间时才可用于用户讨论变化；只有 `observed_at` 时只能说明系统观察变化。

### 4.2 Analysis：可替换的机器解释

`CommentDerivation` 保存清洗后的研究文本、清洗规则版本、输入上下文快照、研究合同版本和派生 fingerprint。`CommentAnalysis` 保存一次经校验的结构化解释。它们均不修改 Comment/Observation。

评论的用户层研究结果只允许：

- 未研究；
- 已研究；
- 无研究信号；
- 结果已过期；
- 研究未完成或失败。

执行状态独立存在于任务：待执行、排队、执行中、完成、失败、超时、中断、等待恢复；失败必须带可行动的原因代码。

### 4.3 Problem：长期情报对象

`Problem` 不是某条评论，也不是一轮临时聚类。它由多个有效的原子表达与证据关系组成，拥有稳定 ID、自动定义、首次/最近证据、成员关系和版本历史。后续允许出现关联、合并、分化和新问题，但不要求用户维护分类树。

每个问题必须可回溯：

```text
Problem → 成员表达 → Analysis → Derivation 输入/合同 → CommentObservation → 来源作品与回复语境
```

## 5. 清洗与真实上下文合同

### 5.1 Cleaning Contract V1

清洗仅生成派生研究文本；不对原始事实做覆盖。下列情况从评论研究语料中确定性剔除：

- 清理 `@用户名`、emoji、空白和无意义标点后没有自然语言；
- 纯 emoji；
- 纯 @用户名或 @用户名加纯 emoji；
- 纯无意义标点或异常空白。

有语义的短评论必须保留，例如「同问」「有效果吗」「我家也是」。`@作者 求具体方法` 只移除 mention，保留正文。URL、电话与联系人在模型输入前需受控脱敏；库内事实保存遵从 Evidence 安全合同。

仅凭纯文本无法可靠判断无分隔中文 `@` 后面哪些字符属于用户名。例如 `@作者求具体方法` 不能安全拆成 mention 与正文。没有采集端提供的 mention span 时，系统必须保守保留整句进入研究，不能为了过滤 mention 丢掉「求具体方法」等真实表达；获得 span 后才可做精确移除。

### 5.2 Context Pack V1

每一个被模型处理的评论必须生成可审计、尺寸受限的真实输入包：

```text
当前评论研究正文（唯一可作评论者证据的正文）
+ 必要父评论与根评论片段
+ 作品标题
+ 截断后的作品正文/OCR/ASR 相关片段
+ 已召回的候选问题定义（若启用）
```

上下文只能做指代消解与讨论理解。模型不能把作品正文、父评论或作者回复当成当前评论的证据；结构化输出的 evidence span 必须精确落在当前评论研究正文中。每个 Context Pack 都保存其材料版本、截断规则、hash 与实际发送的文本快照。

## 6. 去重、调度与失败规则

研究完成不等于存在任意历史 run。唯一去重边界是：

```text
comment derivation + current research fingerprint
```

fingerprint 由研究正文、必要上下文版本、清洗版本、研究合同版本、输出 schema 版本和实际影响语义的模型策略组成；它是实现细节，不成为用户导航对象。

| 最新有效状态 | 当前 fingerprint 是否可进入计划 | 规则 |
| --- | --- | --- |
| 无记录 / 新 fingerprint | 是 | 首次研究或规则语义改变后重新研究 |
| cancelled / interrupted | 是 | 上次未完成，不消耗重试配额 |
| provider timeout / connection / worker interruption | 是，受跨 run 的总次数限制 | 暂态失败，按 derivation + fingerprint 计数 |
| output invalid / schema invalid / evidence invalid / semantic contract invalid | 否 | 同一合同下自动重跑通常无意义；改版才重开 |
| succeeded / no_signal | 否 | 两者均为有效结论 |

选择器按研究价值和来源多样性做有界抽样，不能让单篇爆款作品占满一个批次。算法必须可解释本次候选来自哪些评论状态与作品分布；不能暗中改变用户已保存的上限或预算。

## 7. 市场与研究口径

一个问题至少同时呈现：独立评论数、来源作品数、可确认的创作者覆盖和时间/覆盖缺口。点赞只代表评论获得的社会反应，不能与评论频次合并为“需求分”。

趋势有三个不同含义，不能混用：

1. 新评论被系统采集：观察量变化；
2. 历史评论刚被模型分析：研究覆盖变化；
3. 在可比的用户发表时间窗口内讨论变化：用户讨论变化。

在真实 `published_at`、时间精度、研究规则和范围可比较前，变化观察必须显示为不可比或系统观察变化，不得标为市场升降。

高共鸣、高冲突、用户需求、解决方案、金句、故事和社区热词是可叠加的研究视角。只有在各自规则、证据与数据存在后才渲染；不得用预置文字或推测数据补空状态。

## 8. 实现顺序

### P1：来源字段审计与 Comment 事实合同

交付：插件/现役参考的字段审计、真实 producer fixture 入口、XHS Comment 运行时 validator、稳定身份、Comment/CommentObservation 的最小 schema、append-only/replay/current 约束、隔离 PostgreSQL 攻击性测试。

当前证据：`fixtures/xhs/comment-evidence-set-v1.json` 已提供一个去标识化的真实 `content_detail + comments` 来源形状对。它只解锁 package/record/跨包作品身份的 runtime validator；它不证明完整 collection profile，不能跳过缺口 fixture、PostgreSQL 约束或用户原声页面验收。

V0 进度：来源 validator、不可变 Evidence/CommentObservation、稳定身份、replay/current 规则与 PostgreSQL 攻击性证明已经落地；见 [`docs/proofs/comment-fact-storage-v0.md`](../../proofs/comment-fact-storage-v0.md)。它只覆盖配对 package 与原始正文变化，不代表 P2 原声浏览器、研究或问题页面已完成。

Context Storage V0 追加：`content_detail + comments + replies` 的三包 Source Contract、不可变 Evidence 锚点、作品/讨论上下文存储和 hash 严格匹配的只读 context detail 已有隔离 PostgreSQL proof；其 HTTP 与原声详情抽屉读取已完成独立证明，见 [`docs/proofs/comment-context-storage-v0.md`](../../proofs/comment-context-storage-v0.md) 与 [`docs/proofs/comment-voice-context-read-api-v0.md`](../../proofs/comment-voice-context-read-api-v0.md)。它已可与当前 comment-cleaning.v1 用户原声读取并用；仍未实现 Context Pack 截断、模型输入或研究结论。

停止条件：没有更多真实 producer fixture 时，只完成已有形状的 validator 和缺口记录；不伪造 Capture/Evidence/Comment 已实现。

### P2：评论语料与原声浏览器

交付：受控 Comment Current read DTO、原声查询/筛选/分页/详情、清洗派生与规则证据、作品与回复语境读取合同、空/受限/未知状态页面。页面遵从正式 LIDS/设计令牌；若新仓库尚未建立 LIDS，先建立统一页面壳与中文信息层级，不复制旧工作台皮肤。

### P3：研究计划、Context Pack 与可观测运行

交付：无副作用研究计划、fingerprint、跨 run 的可恢复调度、输入快照、模型/协议接缝、结构化输出与 evidence 校验、运行记录读模型。真实模型调用仅在用户明确授权后进行。

P3 前置已完成：只读的范围预览 V0 已在用户原声页提供当前语料总数、清洗等待/排除口径、来源轮换候选和来源分布；它没有持久化计划、冻结样本、创建 run、生成 fingerprint、组装 Context Pack 或调用模型。隔离 PostgreSQL 证明见 [`comment-research-plan-preview-v0.md`](../../proofs/comment-research-plan-preview-v0.md)。这不代表 P3 其余交付已完成。

### P4：问题组织与结果页

交付：原子语义、向量接缝、候选召回、稳定 Problem、问题详情与覆盖口径、结果版本和证据回溯。算法计算与 LLM 复核必须分开；不要求人工逐条标注。

### P5：可比较变化与自动化

交付：发表时间/精度的来源闭环、比较资格、变化规则、变化观察页、受控日批与关闭/开启状态。只有 P1–P4 的真实小批次证明稳定后才进入。

## 9. 每个包的强制验收

- 输入跨边界均做运行时验证；反例不能悄悄降级。
- 原始 Comment/Observation 不能被清洗或 AI 覆盖。
- P1 V0 已证明：同身份 replay、不同正文更新、来源 pair 追溯、跨作品相同 comment id、拒绝 history mutation，以及两个 client 对同一身份的本地接纳顺序推进。它没有证明 source time、迟到来源事件或模型结果的时间排序语义。
- 计划预览无数据库副作用、无模型调用；同一 fingerprint 成功或 no-signal 不会重新收费；cancelled 能恢复；暂态失败不会无限循环。
- Context Pack 实际包含允许的文本，而不仅是 ID/hash manifest；输出证据范围必须命中当前评论文本。
- 用户页面能区分「没有原声」「原声未研究」「没有已发布结果」「范围不可比」「无法访问」，不把 Unknown 显示为 0 或正常。
- 每项 AI 结果可展示模型/合同版本、输入快照、校验结果、原声证据、反例或缺口。
- 任何发布声明分别列出：分支/PR、自动测试、隔离 PostgreSQL、真实模型、共享迁移、运行时与用户验收；它们不能互相替代。

## 10. 非目标

- 不迁移旧工作台数据库、旧 Prisma schema 或历史 migration；
- 不创建“选题库”、生产流程、通用 Agent 平台或图数据库；
- 不把模拟样本称为真实用户研究；
- 不在本包中自动开启连续研究、采集、模型调用、共享数据库 migration 或 3000 部署；
- 不要求用户逐条审阅评论、命名簇或维护分类关系。

## P2 增补：Cleaning Derivation V1（已隔离证明）

`comment_derivation_v1` 是 Observation 锚定的不可变派生，不是新的 current、run 或 Agent 对象。其唯一边界为 `comment_observation_id + cleaning_contract`；当前 User Voices read 明确只读 `comment-cleaning.v1`，使未来合同可以并存而不污染当前清洗语料。每个 Created/Advanced Observation 在 admission transaction 中创建 V1 派生；迁移前 current Observation 只能通过有界、幂等、显式调用的本地物化方法补齐，GET 不会写入。

User Voices 默认显示 `available`，可切换 `ready` 与 `needs_context`，同时返回明确的 `awaiting_cleaning_total`。`dropped` / `anomaly` 保留原始 CommentObservation 事实但不显示为用户原声。表格文本是清洗后研究表达；详情按当前 Evidence locator 单独读取原始采集原声，且不在 DTO 暴露 reason codes、derivation ID 或 fingerprint。完整隔离证明见 [`comment-derivation-user-voices-v1.md`](../../proofs/comment-derivation-user-voices-v1.md)。
