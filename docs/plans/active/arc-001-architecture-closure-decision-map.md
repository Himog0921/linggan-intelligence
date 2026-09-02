# ARC-001 代码前产品与系统架构收口决策图

> 状态: 活跃计划
> 最后核对: 2026-08-28
> 适用范围: F01 之后、首个真实 producer 与首个用户可见产品切片之前的产品和系统架构收口
> 事实来源: 用户最新 course correction、当前开发跟踪表、产品/页面草案、采集与媒体历史经验、`ADV-AUDIT-001` 最终处置
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、ACCEPTED ADR、当前活跃 SCOPE 与可复现代码/数据库事实；本图不自动授权真实访问或实现

## 当前执行护栏

F01 可以继续且只可以继续到：

```text
synthetic Package
→ PostgreSQL atomic ingress
→ two independent Records
→ Observation / Current
→ loopback API
→ minimal CLI
```

完成即 hard stop。此处原有的“ARC-001 未收口前，不开始真实 producer、插件升级、媒体、Web”是一条历史性总禁令。Mog 于 2026-08-24 明确以 `LOCAL-001` 取代这条总禁令中针对**独立本地 Linggan 产品**的部分：可以分别规划和申请本地 Web、插件 Local 模式、local data presentation，以及经过 preflight 后的受控真实 Canary。它不解除 F02–F10、生产、旧系统迁移、Agent crew、外部/线上 host、无界真实采集、历史回填或敏感材料无合同处理的禁令；也不改变 SCOPE-001 的 synthetic-only 范围。真实 producer、媒体与异步 OCR/ASR 仍只能按 LOCAL-001 的独立子卡、明确合同和停止条件推进，不能因本段例外自动开始。当前 F01 内不改变产品含义的合同、migration、来源追踪和命名修正由工程团队负责，不建立 Mog 决策票。

审计衍生的 `ADV-M-01` 最高权威冲突规则仍为单独的 `DECISION_REQUIRED`；它不改变本图票序，也不授权低层实现或文档静默改写产品含义。

## Notes

Domain: Linggan Intelligence 第一阶段产品与系统架构收口

Consult: `decision-mapping`、`grilling`、`domain-modeling`、`prototype`、`product-manager`、`codebase-design`

Standing preferences:

- 一次只解决一个 ticket；
- Mog 只决定产品形态、真实资源、隐私、成本和对外后果；
- 技术选型、表结构、crate、状态管理和测试工具由工程团队负责；
- 整体逻辑架构面向全项目，物理架构只冻结第一阶段和已确认的扩展接缝；
- 不因现有首页草案丰富就默认五 Agent 编队、等距地形或七个导航已进入 P0；
- 历史能力只继承用户任务、业务价值、关键不变量和已验证交互认知；旧实现必须先经过 capability inheritance audit，不能因“保留能力”自动复制；
- ticket 产物只链接到本图，不把专题设计复制进本图。

状态只回答“这张决策卡是否仍需要新的决定”：

- `resolved` 表示产品/架构问题已有权威答案；不自动表示代码、真实链、部署或业务验收完成。
- `open` 必须说明尚缺的是产品选择、主线合同整合、实现基线还是用户验收，不能把已经回答的问题重新包装成开放研究。
- 一个旧 Issue/PR 可以因答案已被主线或更高权威文件吸收而关闭，即使其历史分支从未合并；关闭时必须说明吸收来源与未证明边界。

## product-shell: 第一阶段以什么产品形态存在？

Blocked by:
Status: resolved
Type: Grilling
Decision owner: Mog

### Question

Linggan 第一阶段以什么独立产品形态存在，旧内容工作台与它是什么关系，哪些历史能力应当继承而不复制旧实现？

### Answer

Mog 已确认：

1. **Linggan Intelligence 是独立且完整的 Web 产品。** 它不嵌入内容工作台，也不以与内容工作台长期分工或交接作为产品定义。这里确认的是产品身份，不是 Web 实现已经获得授权。
2. **内容工作台是 Linggan 的来时路。** 它是历史原型、最小 MVP、需求来源和经过实践的能力样本，不是 Linggan 的运行时依赖、下游执行系统或未来架构边界。
3. **已证明有价值的用户能力不能因重做而丢失。** 当前至少包括采集能力、任务调度模式、Media V2 的领域思想，以及市场洞察中的主题地图和相关成功页面、交互。
4. **保留能力不等于复制旧实现。** 保留的是用户任务、业务价值、关键不变量和经过实践验证的交互认知；本票不授权复制旧代码、旧数据库表、旧接口、旧页面层级或旧技术债。
5. 每项历史能力进入 Linggan 前都必须经过 **capability inheritance audit**：

   ```text
   旧能力解决什么用户问题
   → 哪些部分有真实成功证据
   → 旧实现有哪些限制或事故
   → Linggan 中更高维的新定义
   → 采用 / 重构 / 重新设计 / 退役
   → 新的验收标准
   ```

6. 与旧内容工作台的数据导入、迁移、兼容、导出或“情报弹药包”只是未来可选接缝，不定义 Linggan 的产品身份，本票不授权实现。
7. 本票只解决 `product-shell`。它不决定首页或页面数量，也不授权旧系统迁移。首个 producer 与媒体方向已在 2026-08-24 的 `LOCAL-001` 中由 Mog 明确：首批 Canary 纳入媒体；搜索发现面、详情/媒体取得和异步 OCR/ASR 必须连续但分卡推进。

## primary-daily-job: 用户每天首先要完成什么？

Blocked by: product-shell
Status: resolved
Type: Grilling
Decision owner: Mog

### Question

Mog 每天进入 Linggan 的 5–10 分钟内，系统必须帮助他完成的那一个核心判断或结果是什么？

### Answer

本票直接引用已确认的 `DEC-02 / USER-DEC-06`，不重新发明日常任务：

- Mog 每天先看到**少量值得关注事项**，理解为什么现在出现、系统实际观察到了什么、哪些解释或缺口仍不确定，然后决定：暂不处理、继续观察、进入 Topic 深入，或提出行动。
- Topic 是深入工作区，承载用户原声、内容叙事、长期历史、近期观察以及对应的 Coverage / Evidence；首页负责把人带到值得深入的地方，不替代 Topic 研究。
- “今天没有合格事项”与“今天的观察不完整”是两种不同结果，必须分别说明，不能都显示成空白或“暂无”。

该答案只确认用户每天首先完成的认知与决策任务。它不决定五个 Agent 运行实例、等距地形、卡片数量、通知、夜间调度或其他页面实现；这些仍由后续 `p0-surface-prototype` 与 `first-phase-runtime` 验证和收口。

## p0-surface-prototype: 第一版准确需要哪些可见界面？

Blocked by: primary-daily-job
Status: open
Type: Prototype
Decision owner: 产品团队制作，Mog 走查确认

### Question

为了完成已确认的第一条日常工作流，P0 应准确包含几个一级入口、几个详情工作区，以及哪些能力只应当是 Tab、抽屉或后台状态？

### Answer

Mog 已批准一个有界的 DESIGN-002 输入：以「任务启动困难」为例，制作 `SYNTHETIC / NOT LIVE` 的 Topic Intelligence Reference Page，用来走查 Topic 深入时的单一判断任务、LIDS v2.0 设计表达、状态诚实与组件晋升门。LIDS 统一约束未来 UI 的 Token → Primitive → Component → Pattern → Page，但仍为 Proposed 标准，不声明任何运行时页面/组件已经存在。该交付物见 [`design-002-topic-intelligence-reference-page.md`](design-002-topic-intelligence-reference-page.md) 及其引用规格。

这张卡已经不再处于“从零决定页面结构”的阶段。当前主线已有 Evidence Library、Collection Workspace 五个子面、共享中文壳层及对应页面规格和视觉验收；这些是已经落地的产品表面事实。旧 Draft PR #12 仍停留在早期「今日关注」候选，不能整体合并为当前答案。

本票保持 `open` 的唯一原因改为：尚未把当前真实页面、Topic 深入入口和“观察不完整”状态整理成一份统一的第一阶段界面验收说明，并由 Mog 完成页面级走查。后续不得再询问是否需要独立 Web、是否以中文为主或是否保留 Collection/Evidence 页面；这些已由当前主线回答。

Asset target: `docs/product/first-phase-interface-prototype.md`

验收必须包含三条真实脱敏样本走查：早晨判断、Topic 深入、采集不完整；同时明确 P0 不做清单。

## first-producer-canary: 首批真实 Canary 是否包含媒体？

Blocked by: primary-daily-job
Status: resolved by LOCAL-001
Type: Grilling
Decision owner: Mog

### Question

首批获得授权的真实 producer Canary，是只验证内容身份、标题、正文、作者、时间、有限评论和 Coverage，还是同时包含图片/视频下载与转录？

### Answer

Mog 已于 2026-08-24 确认：首批真实 Canary **包含媒体**，并且图片 OCR、视频口播转录/ASR 都是后续异步派生处理的必要能力。该确认不允许把它们压成一个插件大任务：

```text
搜索前 20 条 discovery
→ 受控详情与媒体 acquisition
→ 独立异步 OCR / ASR 派生处理
```

前一段只形成发现面与 Coverage，不代表详情、媒体字节或媒体 Canary 已完成。后两段分别等待媒体生命周期、隐私、存储、处理器、队列、撤回传播与最小样本合同；任何一段失败不能连坐安全取得的前一段材料。历史执行顺序、禁止项和验证阶梯保留在已归档的 [`LOCAL-001 计划`](../completed/local-001-local-product-evidence-library.md) `001C-1`–`001C-3`；后续能力必须由当前具体交付包重新授权，不能从该历史路线自动开启。

## capture-control-contract: 真实插件前必须冻结哪些调度责任？

Blocked by: first-producer-canary
Status: resolved
Type: Research
Decision owner: 工程架构团队；只有真实账号、风险或资源边界扩大时再交 Mog

### Question

结合旧内容工作台事故与新系统 Acquisition Admission，新 Linggan 在真实插件开工前最少必须冻结哪些 Demand、Work、Attempt、Lease、Reconcile、Package、ACK、Coverage 和恢复责任？

### Answer

[`capture-control-contract.md`](../../architecture/capture-control-contract.md) 已通过 PR #17 集成当前 `main`。它不复制旧 `CollectionTask`、pending fallback 或旧表结构，而是冻结：Research Intent / Evidence Need / Acquisition Authorization / Admission / Work Order / Attempt / Lease / Package / ACK / Evidence / Observation 的分责；known set 与 maximum quota 的 Coverage 语义；Discovery、详情、评论与媒体 lane；Evidence 复用、去重和有意复观测；有限工位/账号/预算/风险控制与执行前重估；partial success、replay、conflict、retry、recovery；以及“采集结果可用”与“Claim 有资格”之间的硬边界。

Mog 已选择含受限媒体的 Canary 方案 B。合同集成只证明控制责任已经冻结，不证明真实账号、媒体、OCR/转录、长期调度或用户验收；这些必须在对应实现和真实运行卡中分别证明。


Asset target: `docs/architecture/capture-control-contract.md`

产物必须包含“旧事故 → 新不变量 → 新责任 → 可证伪测试”映射，不复制旧 `CollectionTask`、pending fallback 或旧表结构。

## media-lifecycle-contract: Linggan 的媒体后继架构是什么？

Blocked by: first-producer-canary
Status: resolved
Type: Research
Decision owner: 工程架构团队；隐私、外部模型和保留策略交 Mog

### Question

怎样把内容工作台 V2 已验证的媒体身份/origin/slot/usage 语义、PR #15 已审查的生命周期草案、当前 Linggan Rust 媒体实现和最新产品运行规则，收敛成当前 `main` 的唯一媒体合同？

### Answer

产品和核心语义已经回答，不再从零设计：

- 直接继承 V2 的身份、来源、槽位、字节、副本、用途和异步事件分责；URL/路径不是媒体身份，同一物理媒体可服务多个业务槽位。
- Linggan 增加媒体观察、下载尝试、lane Coverage、`TaskSpec → Attempt → Package → Receipt` 血缘、分块上传/校验、派生版本和处置传播；不迁移旧表、旧运行时或旧串行 Outbox。
- [`collection-monitoring-rules.md`](../../product/collection-monitoring-rules.md) 已确认本机存储、图片长期保留、视频转录成功后 180 天、关键帧长期保留、OCR/ASR/抽帧/embedding 首批进入、资源队列分离、转录并发 1，以及外部 Agent 通过 CLI 而非直连数据库。
- PR #15 的历史合同草案已经形成并通过语义审查，但其分支与当前主线冲突且落后；当前 Rust 主线也已实现 Slot、Observation、Download Attempt、Blob、Materialization、Processing Job/Event 和派生入口。旧 PR 不应强行合并。

[`media-lifecycle-contract.md`](../../architecture/media-lifecycle-contract.md) 已从新鲜 `main` 完成校准并成为唯一当前入口。它已经：

- 把 V2 分成直接继承、语义继承/实现重写、Linggan 新增和明确废弃四类；
- 对齐插件 `v0.5.0` 的 discovery/profile/detail/comments/replies/author/media slots/media bytes/checkpoint 通道，以及 image/cover/video/live photo、多 candidate URI、展示顺序、用途和来源代次；
- 冻结 Blob/Replica、分块上传、派生版本、图片/视频/关键帧保留、撤回传播和资源队列边界；
- 对照当前 Rust/PostgreSQL 主线分别标明已实现、合成 PostgreSQL 证明、仅接口、未实现和真实链未验证；
- 给出 Evidence Library 后续多材料页面的最低消费合同，但不在本票实现页面或读模型。

因此产品和架构问题已经 resolved。真实媒体取得、OCR/ASR/抽帧/embedding 输出、保留期清理、撤回传播、多材料读模型、页面实现与用户验收仍由后续卡逐项证明；这些未证明项不能反向把本票重新包装成开放产品研究。

Asset target: `docs/architecture/media-lifecycle-contract.md`

首批 Canary 已确认含媒体；本票不再存在“纯文本即可绕开媒体”的路径。它继续作为任何 001C-2/001C-3 真实媒体 Lane 开工前的独立生命周期/隐私/派生血缘合同门。

## first-phase-runtime: 第一阶段怎样形成完整可运行系统？

Blocked by: p0-surface-prototype, capture-control-contract, media-lifecycle-contract
Status: open
Type: Research
Decision owner: 工程架构团队

### Question

用什么最小前端、后端、数据库、worker、插件接缝、AI 接缝和部署拓扑，才能实现已确认的 P0 工作流，同时不预建尚无真实调用者的未来模块？

### Answer

产品层的运行形态已经回答：Mac mini + Cloudflare、本机 PostgreSQL 与媒体存储、沿用内容工作台的调度经验但不迁移旧代码、调度决策与下载/OCR/ASR 重执行分离、不同资源使用独立队列、外部 Agent 经 CLI 访问。当前主线也已经存在 Rust API、PostgreSQL、worker 接缝、Browser Producer Runtime、Evidence Library 和 Collection Workspace。

本票保持 `open` 的原因不再是技术选型讨论，而是缺一份以当前代码为准的实现基线：逐项标明已实现/已由 PostgreSQL 合成链证明/仅有接口/尚未实现/尚未真实验证，并列出调度器、派发、媒体处理器、可观测性和部署的真实缺口。

Asset target: `docs/architecture/first-phase-implementation-baseline.md`

产物必须明确前端框架/路由/状态边界、后端模块/API-worker 分责、当前物理数据库模型、真实 producer/媒体/Agent 接缝，以及开发、测试、部署和可观测性的最小闭环；不得一次性物理实现概念地图中的全部实体。

## first-user-visible-scope: 首个真正产品切片是否获准开工？

Blocked by: first-phase-runtime
Status: open
Type: Grilling
Decision owner: Mog

### Question

基于页面原型、真实 Canary 边界和第一阶段技术基线，哪一条完整用户工作流被正式批准为 F01 之后的首个用户可见 SCOPE？

### Answer

第一阶段产品目标已经明确：先实现“可靠观察与可检索语料底座”，让观察目标按照明确规则形成任务，经工位执行并返回可追溯结果；成功、部分、失败、缺口、补采与最终语料可用性不能成为黑盒。现有一个账号、一个博主或一次入库只证明局部链路，不等于该阶段完成。

本票保持 `open` 只等待把这条目标写成一个有界、可验收的正式 SCOPE，并引用当前界面验收、媒体合同和实现基线；不再重新讨论第一阶段愿景或另造“研究工作台”。

Resolved 后才允许创建新的活跃 SCOPE、对应 GitHub Issues，并更新开发阶段跟踪和当前状态；F01 只在取得 SCOPE-001 约定的可复现技术证据后转为已完成 technical tracer。该证据必须包含真实 PostgreSQL 副作用，但不要求真实 producer、真实插件或真实平台数据。

## Next steps

`product-shell`、`primary-daily-job`、`first-producer-canary`、`capture-control-contract` 与 `media-lifecycle-contract` 已 resolved。剩余开放卡均已改写成具体缺口，不再重复产品提问。

ARC-001 当前下一张未解决的系统卡是：

```text
first-phase-runtime：以当前代码为准形成已实现 / 合成证明 / 仅接口 / 未实现 / 未真实验证的运行时缺口基线。
```

Mog 已另行批准五卡垂直路线；媒体合同之后的 Evidence Library 产品手册、材料接纳/读模型、运行时页面和真实垂直证明按各自任务卡推进，不由本图自动授权或宣称完成。`first-phase-runtime`、`first-user-visible-scope` 与 `p0-surface-prototype` 仍按各自具体缺口收口。旧 Issue #11/#16/#18 及 PR #12/#13/#15/#19 只作为历史输入或被取代材料处理；不得因本票 resolved 宣称真实媒体链、OCR/ASR、调度或业务验收完成。
