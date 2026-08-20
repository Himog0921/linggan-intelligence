# 目标架构

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: Linggan Intelligence 目标模块与数据边界
> 事实来源: ACCEPTED ADR 与已确认架构原则
> 冲突时以谁为准: ACCEPTED ADR；实现状态以真实代码和运行结果为准

本文保留 Rust、全新 PostgreSQL、Evidence First 等已接受硬边界；其余模块划分和处理顺序必须在 DISC-001 中与产品、采集和数据设计共同复核，当前不得直接作为实现蓝图。

Gate 6 的 Rust 模块接口、依赖方向、adapter seam、测试表面与文件规模门禁按渐进披露进入 [`module-architecture.md`](module-architecture.md)。该子文档仍是草案，不改变本文件的开工限制。

下图表示 Gate 2-6A 已确认的责任关系，不是数据库表、Rust 模块或每份材料必须走完的线性流水线。持续观察和主动研究都可以从已有资产开始，也允许在证据不足、无需行动或不再值得时停止。

## 结构

```text
观察意图 ──→ 计划 / 授权 / 采集准入
                    │
外部世界痕迹 ──→ 采集发生记录 ──→ Capture Package ──→ Evidence Ingress
                                                             │
                                                             ↓
                                                   Accepted Evidence
                                                             ↓
                                                   Source Object / Observation
                                                             │
当前 Domain Definition ──────────────────────────────────────┼─→ Derived Analysis ─→ Claim
                                                             │                       │
                                                             └───────────────────────┘
                                                                           ↓
                                                          Decision → Action
                                                                      ↓
                                                         Outcome Observation
                                                                      ↓
                                                                 Evaluation

Corpus 选择、Topic Map/页面和 CLI/Agent Interface 横跨读取或使用这些责任，
不成为下一层真相。
```

采用模块化单体：一个 Rust workspace、一个 PostgreSQL 主数据库、独立 API/worker 进程。对象存储只保存媒体二进制，数据库保存身份、来源、checksum 和状态。

## 领域层

### Discovery / Capture

Bootstrap 候选曾包括 Observation Target/Plan、Station、Run、CapturePackage 和 Coverage；Gate 3 已把前两者收敛为持续 Observation Objective 与版本化 Collection Plan，并与实际 Run 分开。Gate 4–5 继续区分发现执行、发现面 Evidence、每次发现记录、来源身份解析、稳定 Source Object/Identity、详情采集、来源时间陈述、Coverage 来源事实与用途评估；这些名称不预授权同名表或 crate。

搜索、作者页和其他低成本发现与逐篇详情/评论采集是不同责任。发现面 Evidence 可以证明接口返回或页面实际展示了什么，但不能替代对象详情 Observation；详情访问只能在 Evidence 复用、去重、身份解析、授权、资源和风险准入后按有界工作发生。

### Capture Package / Evidence Ingress

Capture Occurrence 保存有边界工作实际发生、结束、失败和 Coverage 影响；Capture Package 是 producer 的交付单元，可以包含头信息、零到多条来源 Record、Coverage/终态来源事实和合同需要的 Artifact。原始响应、页面快照、DOM 片段、媒体清单或上下文属于包内可选/按合同必需 Artifact，不预设所有 producer 都要经过一张独立 Raw Artifact 表。

Evidence Ingress 只决定交付是否满足来源权限、目标/身份、包一致性、运行时合同、canonical/checksum 和重放/冲突要求。包级接入是原子事务：不能只接纳半包却宣称整体成功。同一采集身份与相同内容只作幂等重放；同一身份却对应不同内容/hash 必须隔离且不得进入下游。包被接纳后，身份解析、对象归属和 Observation 形成按 Record / Source Object 隔离处理；接入后才发现的单条对象归属问题不能使其他合格记录无故丢失，也不能被整包成功掩盖。

未通过接入的交付不形成 Evidence，但必须留下足够安全的发生、失败、终态、Coverage 与影响记录。完整无效载荷是否进入受限隔离区、保留多久和怎样处置由 Gate 4–5 决定，不把“不可静默丢失”误写成“永久保存全部原始敏感数据”。

### Evidence

系统在特定时间、producer 和观察条件下接纳的来源材料。它证明系统收到了什么，不证明材料内容真实、完整、有代表性或适合任意判断；追加、版本化、不可被业务逻辑静默覆盖。

### Observation

基于被接纳 Evidence，对某个来源对象在一次观察中的版本化表达。它必须保留来源和观察条件，不能把采集缺失解释为现实不存在，也不能把模型分类伪装成来源自带属性。Observation 的身份不包含主动研究问题或持续观察目标；同一次观察可以通过使用关系被多个目的复用。

Observation 只追加。较晚观察形成新 Observation；迟到材料分别保留原观察时间和较晚接收/接纳时间，不按入库时间制造世界变化；同一 Evidence 的 parser/结构化修正进入有血缘的 Interpretation Revision，不制造一条现实并未发生的新 Observation。执行、观察、接收、接纳、来源时间陈述和派生时间必须保持不同责任。

### Source Discrepancy / Current Resolution

API、页面或其他合格来源在相近观察条件下对同一字段给出不同值时，各来源陈述并存，不按最大值、平均值或最后写入静默合并。Current Resolution 是按当前规则组织的可重算读取结果，不是 Evidence 或永久真相；每个当前值必须能回到 Observation/来源陈述、选择规则和理由，资格不足时允许未知或待裁定。

本阶段只冻结上述读取与历史责任，不选择整对象指针、字段级投影或混合存储，也不创建 Current 服务。来源优先资格由 Gate 4 的 producer 对照实验确认，持久关系、事务、重算与修订传播由 Gate 5 设计。

### Derived Analysis / Domain Definition / Claim

Derived Analysis 是有明确输入集合和方法版本的分类、聚类、统计与 Agent 分析；Domain Definition 是当前采用的 Topic、Term、Alias 与知识性关系；Claim 是可被支持、挑战和修订的具体主张。来源字段解析和格式归一化若只忠实表达 producer 可验证状态，属于 Evidence 接入/Observation 形成，不与 AI 语义解释混成通用 `Evidence Interpretation` 层。三者不能因为共用一个 Topic 名称而互相覆盖。

Gate 3-4 进一步冻结以下责任，但不预设同名表或独立服务：

- Topic Identity 负责长期引用，Topic Definition Version 负责某个时期的名称、定义和边界；拆分/合并通过血缘连接新旧身份，不自动迁移旧 Claim、分类或统计。
- Domain Definition Release 是由明确发布决定选择的可复现逻辑清单。当前正式版本不能由最后写入或最大版本自动推断；Gate 5 可以使用共享版本引用，不要求物理复制整套词表。
- 正式 Domain Definition Release 与 Experimental Analysis Vocabulary Snapshot 分开。Classification Run 固定不可变输入集合、词表快照、模型/规则/提示和运行版本；Human Adjudication 追加裁定和理由，不覆盖机器输出。
- Term 与其作为 Preferred Label/Alias 的版本化语境关系分开；Collection Seed、用户原话和正式词表关系不能互相替代。歧义 Term 不建立全局同义等价。
- Knowledge Relation、数据共现/相似/因果假设和 Map View 导航分开；正式层级关系在同一发布版本中不得自引用或成环，导航移动不改变定义或分类。
- 定义发布与重分类、统计和页面投影就绪分开。读取侧必须固定并暴露所用定义/分类版本；尚未重算时返回未就绪或明确旧版本，禁止静默混合。
- Claim 是值得长期追踪的重要原子判断，不是万能三元组或每个统计的物理容器。支持、反例、评估和治理历史追加保存，不能退化为一个可覆盖 confidence。

### Research / Acquisition / Decision / Action / Outcome

Gate 3-B 已冻结以下架构责任，但不预设同名表、crate、服务或统一状态机：

- Continuous Observation Objective、Research Question、Collection Plan 与实际 Run 分开；Information Need 只表达知识缺口，可以由既有材料、分析、人工材料或新采集满足，任务完成不自动代表信息足够。
- Acquisition Request、人的 Acquisition Authorization、执行前 Admission 与 producer Work Order 分开。采集授权、材料读取/传播许可和插件设备凭证不能互相替代；计划改版、排队或预留不能扩大旧授权。
- 多个申请可以共享一次必要平台访问，但各自目的、Coverage、新鲜度、使用资格、保留要求和 Need 满足评估继续独立。受控探索有单独预算、范围和停止条件，并与比较性观察隔离。
- Knowledge Change Proposal、Acquisition Request 与 Action Proposal 按后果分开；Decision 固定确切版本和当时依据，批准只允许下一责任继续，不证明执行或成功。
- Action Proposal/Plan、Action Attempt/Occurrence 与已证明的外部动作分开；Expected Outcome 固定行动前计划版本和观察窗口；Outcome Observation 保存结果事实；Evaluation 解释结果对原 Claim 的有限意义。它们不共享一个可覆盖的 `Outcome` 状态，也不自动回写正式知识。
- 研究的已回答、未获支持/受反证、证据不足、不值得继续、范围改变与暂时中止分别表达；中止后的恢复必须重新检查范围、时效、授权、资源和新增 Evidence。

## 关键边界

- Capture 成功不等于 Observation 成功。
- 搜索 API 返回与页面实际可见是不同来源陈述；在 Gate 4 真实对照前，不得把 API 返回数量写成页面可见数量，也不得把发现面 Evidence 写成完整笔记详情。
- 身份解析、语义相关性和详情采集优先级必须分开。稳定身份充分时可以仅建立或复用最小 Source Object；这不授权创建详情 Observation，也不自动进入 Corpus、Topic 或深采队列。
- 发现 N 个对象不能自动产生 N 次详情或评论访问。发现记录先进入 Evidence 复用、去重和采集准入；只有被准入的有限对象才形成有边界工作，其余对象的发现历史和停止理由仍可追溯。
- 不满足身份、来源或运行时合同的采集返回仍需保留足以说明执行、失败、Coverage 和影响范围的记录，但不得生成有效 Evidence 或 Observation；具体原始 payload 保留策略等待 Gate 4–5 的 producer 与隐私审计。
- Capture Package 的接入原子性与下游 Record/Object 隔离必须同时成立：整包不能部分写入后宣称接纳成功，单条对象解析失败也不能把其他合格记录一并作废。Gate 4 已确认 Work Order 0..N Attempt、Attempt 独立 Capture Identity、第一版 Attempt 0..1 终态 Package、网络 replay/执行 retry 分责和 checkpoint 仅属运行状态；最终表、字段、事务、队列与服务边界进入 Gate 5–6。
- 执行目标不匹配、来源记录内部身份不匹配、采集包身份/hash 冲突和媒体主体不匹配必须保留为不同失败族；它们对重试、隔离、Evidence 接受和 Coverage 的影响由 Gate 4 的真实合同决定。
- 缺少稳定来源身份的文本可以保留为未解析材料或受限 Corpus 片段候选，但不得创建 `Expression Object`、Comment、Source Object 或 Observation 兜底。
- 同 capture/hash 重放、同 capture/不同 hash 接入冲突、不同来源的值不一致、较晚时间的正常复观测和 parser 更正是五种不同语义；不得统一成 upsert 覆盖旧值。暂时不可访问也不自动等于来源已删除，隐私处置另走授权与传播规则。
- `publishedAt` 等时间只能按来源资格表达：平台绝对值、原始相对文本、producer 推算值、范围或未知不得被压成同一种精确时间。原始表达、来源、参照观察时间、解析版本、时区和精度必须可追踪。
- Observation、Interpretation Revision 与 Current Resolution 分责；迟到材料不能按接收时间推进世界状态，解析修正不能伪造新观察，Current 不能由最后写入、最大 `observedAt` 或无来源字段拼接决定。部分 Observation 的当前字段仍必须逐项保留来源，冲突或资格不足允许未知。
- Observation accepted 不等于现实对象 active。
- Source Object 不使用 `Discovered → Resolved → Observed → Changed → Deleted → Unavailable` 万能生命周期；发现、身份解析、观察、派生变化、来源删除、本次不可访问和隐私处置分别保留责任。
- 同一物理媒体可以承担多个业务 slot。
- Evidence、Observation 与产品 Current Projection 分离。
- Agent 判断不是事实；它必须带证据范围、反例或缺失、版本和允许结论边界。人工确认授予当前发布或使用资格，不制造现实真理。
- Corpus 可以持久保存用途、选择规则和成员历史但不复制 Evidence；检索运行、固定材料包与实际材料使用分开，当前用途资格不能由历史选择永久继承。Topic Map、市场洞察页面、情报简报和选题库是受控视图/工作能力；CLI/Agent Interface 是受控服务接口。它们不得分别维护第二套事实，彼此也不能被一个统一 Projection 概念抹平。
- Signal 保留有输入范围和 Analysis Run 版本的派生发现，Attention Handling 管理是否继续关注；二者都不复制 Claim，也不能因进入首页或重复出现而升级成正式趋势。Market Insight/Intelligence Brief 固定当时 Claim revision、分析、材料选择和正文，后续修订通过影响评估或后继版本表达。
- Content Idea 与 Domain Topic 分开；人工来源、机器建议、保存版本、采用 Decision、成稿、Publication/Action、材料实际使用和 Outcome 各自留痕。Agent Answer 只有在被保存、共享或用作 Decision 时才冻结复现链，输入血缘不替生成输出背书。
- 受限原始 Evidence 默认只在内部受控边界按明确用途保存；工作台读取脱敏视图，外部 Agent 默认只取得聚合结果、脱敏片段和受控引用。获准隐私处置必须通过来源血缘使 Corpus、索引、embedding/feature、摘要、引用、缓存和 Agent 输出失效或重建，并让历史判断进入来源资格重评估。
- 每日关注入口是读取候选事项、Coverage、反例/缺失和待决定后果的用户任务视图，不拥有新的事实或统一 `Signal` 身份。它可以把用户带入 Topic 深入研究，但不能通过排序或展示把候选升级成正式判断；无合格事项与观察不完整必须分开表达。
- 稳定来源对象与每次发现是不同事实：对象可以去重，Discovery Finding 的入口、时间、排名、查询/计划、观察身份和 Coverage 不能丢失。版本化分析还必须引用实际输入集合，不能只记录模型名称。
- Topic 的定义性/知识性关系与数据共现、市场关联和因果假设分开；后者默认属于 Derived Analysis 或 Claim。当前唯一一级 Domain 仍为 ADHD，不因某项研究或 Topic 自动创建新 Domain。
- 聚类标签、高置信分类或页面采用不能发布正式 Topic；语义变更先形成 Knowledge Change Proposal，由人的决策发布新 Domain Definition Release。纯展示修正保留编辑记录，不触发概念版本或全量重算。
- Outcome Observation 不直接修改 Topic、Claim 或观察计划；任何改变必须形成新的评价、判断或有记录的定义/策略修订。
- Gate 2 已确认：AI 只能提出证据缺口与候选获取方案；人批准授权范围；服务端在范围内编排有界工作；producer 只执行当前工作单元；Evidence 接入决定哪些运行结果可以作为事实接受。任何一层都不能凭任务完成补齐现实。
- Gate 2 已确认：有限工位、账号和平台访问时间是系统事实。所有获准的真实平台访问必须先经过统一采集准入，检查 Evidence 复用、等价需求/在途执行、有意复观测、所需 Coverage 与新鲜度、授权、时间价值、执行能力和风险资源；只有准入结果需要新访问时，才产生有界工作。
- 采集准入、执行协调、producer、Evidence 接入和需求满足判断必须分责。执行协调不能凭任务终态宣布 Evidence 成立；Evidence 接入也不能反向扩大采集目标。一次合格采集可以服务多个需求，但必须保留原始动机、发现路径和后续复用关系。
- 上述是产品与架构责任边界，不是最终模块图。编排组件名称、任务类型、恢复机制和物理部署必须等待 Gate 3–6 的语言、producer 合同、数据与架构设计后确定。

## Gate 2 已确认的控制边界

```text
产品 / AI / 人
  → 证据需求
  → 采集准入
      ├─ 复用已有 Evidence
      ├─ 合并等待在途工作
      ├─ 延后 / 过期 / 越权停止
      └─ 准入新访问
           → 有界执行计划
           → 执行协调（工位 / 账号 / 租约 / 恢复 / 重试）
           → Producer（插件 / 后续真实适配器）
           → Capture Occurrence + Capture Package
           → Evidence Ingress（包级原子接入与重放/冲突验证）
           → Accepted Evidence + Coverage Source Facts
           → 按 Record / Source Object 隔离解析与 Observation 形成
           → 需求满足判断
```

第一阶段仍采用 Rust 模块化单体和 PostgreSQL。这里的“统一控制边界”不等于创建单独微服务、万能工作流引擎或规则平台；根 `AGENTS.md` 中的 crate 名称也只是 Bootstrap 放置护栏。具体 Rust 模块归属在 Gate 6 根据前置合同决定。上层不依赖浏览器插件的内部实现，但 Evidence 必须记录真实 producer 类型、版本、能力、执行身份、计划版本和观察时间，抽象不能抹去来源。

平台生命周期与任务价值策略必须版本化。来源对象本身没有一个跨目的通用的 `stale` 状态；价值由观察目的、对象年龄、已有 Evidence、Coverage、延迟损失、成本和风险共同决定。市场分析使用明确可比边界的版本化样本，不把 Current 或全部历史直接当成统计真相。

## 比较性观察与探索/调查的解释边界

Gate 2 已确认：比较性观察是测量变化的“尺子”，探索与调查观察是理解和发现的“放大镜”。两者经过同一采集准入、producer 和 Evidence 接入，共享事实资产，但必须保留不同的观察上下文和允许结论范围。

- 影响解释的入口、范围、频率、排序、producer 能力或停止方式变化，必须形成新的比较条件；单轮执行不足只进入该轮 Coverage。
- 探索/调查可以在授权和有限预算内调整镜头，但产生的材料不能未经明确可比性处理直接加入趋势基线。
- 观察入口效率可以独立评估并形成计划调整建议；调整不能静默生效，也不能反向改写过去比较结论。
- 所有 Evidence 共享来源、执行、时间、实际结果、缺失/失败和运行时验证的最低合同；复杂判断在此基础上增加可比样本、反例、不确定性和方法版本。

这是逻辑和解释边界，不授权建立两套存储、两个微服务或两个物理队列。`Monitoring`、`Discovery`、`Investigation`、`Backfill` 等执行类别和物理模块归属分别留给 Gate 4 与 Gate 6。

## Gate 4 producer 探查硬门

在冻结搜索发现、详情/评论 Capture Package 合同或升级插件前，必须使用真实小红书页面完成有界、只读优先的 producer 探查。搜索侧记录实际关键词、页面、筛选、账号/工位、插件版本、起止时间和风险状态；分别对账 API 返回与页面可见卡片；逐字段确认 ID、链接、标题、作者、时间、指标、顺序/排名、游标/分页和来源；记录计划扫描、实际返回、实际可见、通过身份合同、重复/冲突、停止原因和失败。

探查还必须用 INV-19 证明两阶段边界：发现扫描完成后不会自动打开全部详情；系统只对明确准入的有限子集形成有界详情工作，并可分别记录成功、失败、暂停、恢复和风险中断。单次未触发验证码不证明“安全”，固定批量大小、间隔、账号容量和重试阈值只能由重复真实实验形成版本化策略。

详情/评论侧必须继续对账真实 producer 能提交的包头、Record、Artifact、Coverage 与终态，验证评论身份键、目标一致性、checksum/canonical、相同重放、不同 hash 冲突、媒体主体关系，以及“包级接入原子、下游单 Record/Object 失败隔离”。Attempt/Package 最小关系已经 Gate 4 审计和用户确认，不再由真实实验重新决定；真实实验仍需核实无效原始载荷隔离期限、指标来源链、评论/作者媒体、时间字段资格和各 lane 的实际能力。特别要对照 producer 执行时间、客户端观察时间、服务器接收/接纳时间、来源原始时间表达和 parser 结果，测量时钟偏差，并验证 API/页面对各字段是否存在稳定优先关系。不足项标记 `SOURCE_INCOMPLETE`，不得由旧 schema 补猜。

Gate 5 的详细候选数据模型单独维护在 [`data-architecture.md`](data-architecture.md)，避免目标架构同时成为数据库表设计和页面真相。

## 趋势资格边界（DEC-05 已确认）

第一阶段架构必须支持“描述近期所见”而不被迫生成趋势：

```text
有时间与捕获范围的 Observation
             ↓
近期观察（描述所见，不主张前后可比）
             ↓
候选变化（保留替代解释、反例和缺口）
             ↓
Gate 4 真实可比性实验 + Gate 5 统计资格
             ↓
正式趋势 Claim（仅在已证明范围内）
```

账号、工位、入口、排序、Coverage、内容年龄、时间精度、Topic 定义、分类/模型版本和实际分析输入都可能改变解释。架构不能用一个全局 `comparable`/`trend_ready` 布尔值抹平这些差异，也不能因为记录跨了两个日期就自动产生趋势。第一阶段不因此创建趋势服务、基线表、固定窗口、统一评分或 Agent；后续实现必须能够让同一批记录在不同 Claim 下拥有不同适用性。

## 主动研究的逻辑边界

Gate 2 已确认：主动研究先读取并组织已有资产，再识别会影响当前判断的证据缺口；只有采集准入确认需要新的真实平台访问时，才产生有界工作。研究分析能力可以修改假设和提出反例，但不能绕过准入直接驱动 producer，也不能凭研究结束自动发布正式知识、改变比较性观察或触发现实行动。

这是一条跨产品、分析、采集和资产治理的责任边界，不是物理模块图。本阶段不新增 `research`、`knowledge`、`analysis` 等微服务或固定权限层级；观察意图、问题、假设和候选状态进入 Gate 3，producer 上下文进入 Gate 4，判断适用性与分析版本进入 Gate 5，Rust 模块与部署边界进入 Gate 6。采集现实压力测试属于 Gate 4，不新增 `Gate 3-A`。

## 外部 Agent 委托边界（DEC-03 已确认）

外部 Agent 只通过与工作台一致的产品合同访问 Linggan，不直接进入 PostgreSQL、内部 producer 或插件队列。第一阶段的控制关系是：

```text
已知调用者 + 当前委托范围
        ↓
受控 Agent/CLI 接口
        ├─ 读取聚合结果、脱敏片段与受控引用
        ├─ 运行不改变正式世界的版本化分析
        └─ 提交候选建议或有边界申请
                         ↓
                   等待人的决定
                         ↓
              ┌──────────┼──────────┐
              ↓          ↓          ↓
        正式知识治理   采集准入链   Decision/Action
                           ↓          ↓
                  Producer/Evidence  Outcome/Evaluation
```

提交申请不产生资源副作用；批准只授予后续系统尝试的范围，不产生“采集成功”“知识成立”或“行动有效”的事实。每次调用必须能关联调用者、代表关系、目的、Domain、允许数据/动作、期限、资源和输出传播边界；同一 Agent 不能跨任务自动继承永久权限。任何获准工作仍使用原责任链的运行时验证、幂等、失败、Coverage 和结果回执，不建立一条 Agent 特权旁路。

这是产品与架构控制边界，不是权限表或协议设计。本阶段不确认 RBAC/ABAC、授权令牌结构、预算算法、CLI 命令、MCP、审批引擎或独立 Agent Gateway；这些分别等待 Gate 3–7 的领域、数据、运行和接口设计。

## 结果可获得性边界（DEC-04 已确认）

第一阶段不假设 Action 发生后系统天然拥有反馈数据。结果链至少保持以下责任分离：

```text
Decision → Action
              ↓
       结果渠道可获得性
       ├─ 系统可观察且来源合格
       ├─ 人工/外部陈述，可追踪提供者与依据
       └─ 未接入 / 未观察 / 窗口未结束 / 来源不合格
              ↓
       Outcome Observation（仅实际取得的结果）
              ↓
       Evaluation（结果意味着什么、不能证明什么）
```

“结果渠道可获得性”不是 Outcome 值，也不是 Action 状态。自动结果必须能关联真实 Action 与平台对象、producer、观察时间、运行时合同、缺失和 Coverage；人工/外部结果必须保留提供者、时间、依据与精度边界，默认只证明有人这样报告。未接入或未观察只能是未知，不能进入数值聚合、失败率或因果评价。

历史 `OperationPublication`、`ContentMetricSnapshot/Latest` 和 `leadCount` 等字段不构成新项目来源合同；V2 指标仍缺已证明的 metric Raw Record/同源链。公开指标、深度评论、私信、咨询、销售、访谈与产品行为分别需要 Gate 4–5 的真实 producer/输入来源和行动身份审计，不能从旧 schema 直接生成数据库模型。

这是长期学习闭环的最低架构约束，不要求首个切片实现完整反馈自动化，也不授权当前创建 Outcome 服务、统一渠道枚举、人工补录模块、归因引擎或指标同步 worker。

## Topic 探索与市场投影边界

Gate 2-5 已确认 Topic 详情是跨资产读取与下钻入口，不是新的事实拥有者。它组合已发布 Topic 知识、Corpus 材料、可复算市场投影、候选解释和 Evidence 引用；任何页面、CLI 或 Agent 都不得复制一套无法与这些来源同步的 Topic、计数、趋势或代表样本。

主题知识关系与市场观察关系必须分离：前者说明当前发布版本如何组织概念，后者说明特定观察面和时间窗口中看到了什么。树形导航、市场共现、语义相似和正式父子关系不能共用一个无来源的关系字段。当前只冻结该逻辑边界；投影合同、统计分母、双历史版本和查询组合方式分别留到 Gate 3–6。

## 领域不变量验收

后续领域、producer、数据、架构和实现设计必须持续通过 [`../product/domain-invariants.md`](../product/domain-invariants.md)。该基线覆盖无效返回、身份不确定、Candidate Topic、Topic 拆分、扩采假趋势、反例、Agent 越权、Outcome 伪因果、隐私处置、模型重分类、无效零值、Evidence 跨用途误用、时间伪精确、发现历史丢失、意图复制 Observation、关系偷换和结果直改知识。

这不授权现在建立统一 `Claim` 表、认识论引擎或审批微服务。Gate 6 只根据前置语义和真实 producer 合同选择最小模块边界；如果某个架构仍允许“任务没跑 → 计数为零 → Agent 输出现实不存在”，即使编译和数据库测试通过也视为架构失败。

## Rust 移植策略

TypeScript 现役实现不是逐行翻译对象。先固定跨仓 fixture、canonical bytes、hash、SQL 不变量和攻击用例，再在 Rust 中重建边界。每完成一段，以相同 fixture 对照旧实现和 Rust 输出。

## 数据库设计时序

在完成插件字段/时间来源审计、身份失败分类、发现事件来源、情报需求映射、Coverage 来源事实/用途评估、DEC-01 对应的保留/脱敏/访问/处置传播合同和媒体 producer 审计前，不创建业务 baseline。数据库从领域事实推导，不从页面列表倒推。

## Gate 6 开始前的硬门

最终系统架构只能在 Gate 3–5 收敛后开始。至少必须已有：稳定来源身份与发现语义、时间精度、Coverage、判断最小单位、分析输入版本、隐私传播、数据库概念关系和事务边界。AI Agent 还必须有工具权限、结构化输出、模型/提示/规则版本、评测、成本和停止合同；插件升级还必须有真实 producer fixture、四类身份失败、Work Order/lease/恢复和兼容性验收。缺少任一项时，只能保留为架构假设，不能进入实现。
