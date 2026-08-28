# 当前状态与事项队列

> 状态: 权威当前
> 最后核对: 2026-08-28
> 适用范围: 当前阶段、事项顺序、阻塞与下一步
> 事实来源: 本机实际检查、已确认项目边界和完成计划
> 冲突时以谁为准: 真实运行结果、ACCEPTED ADR 与用户最新确认

## 当前阶段

### MATERIAL-PROJECTION-001 / Issue #86（Draft stacked 实现）

基于 `MEDIA-RECON-001`，新的 accepted Package 已有作品级类型化材料投影：发现、详情、评论、回复、作者、媒体槽位、媒体字节状态及 OCR/ASR 生命周期共用一个 `items` 读取 envelope，旧 `cards` 暂时保留兼容。逐字段未知不补值；评论/回复保留稳定身份、根/父关系与各自 Coverage；作者资料按观察版本追加；媒体保留 Producer 顺序与未知展示顺序、多候选来源、generation、Live Photo partial、Blob/本地 Materialization、处理事件/派生和处置状态。普通 API 不返回远程候选 URI、storage key 或临时上传状态，`batch_checkpoint` 不生成材料或整体完成声明。

隔离 PostgreSQL 16 proof 已覆盖 producer admission、类型化表、首次同槽位并发 generation、作品级查询、loopback API、评论脱敏、媒体本地预览、`provider_not_enabled → NOT_ENABLED` 与 `BYTES_CLEANED` URL 抑制，并验证 proof database/container/volume 清理。该 Draft 未回填历史 Package，未访问真实平台、未取得真实媒体字节、未运行 OCR/ASR provider、未改 Evidence Library HTML/CSS，也未部署或完成用户验收。

### GOV-006 / Issue #82（决策治理收敛）

Mog 已明确取消“任何仓库写入都必须 Issue + Claim + 独立 worktree + Draft PR”的统一门禁。后续按风险分级：代码、数据库、运行合同、插件/release、部署/生产、真实外部动作、敏感数据/权限、不可逆处置和并行冲突继续走受保护交付；当前对话已明确授权的低风险文档勘误、索引/状态同步、进度记录和被取代卡片收口可以直接维护，但仍须核对工作区、检查 diff、运行适用治理检查并分层报告，不能借机扩成实现或自动 push/deploy。

ARC-001 已同步为“已回答问题不再重复提问”：Capture Control Contract 已由 PR #17 合入；媒体卡已从当前主线完成 V2 语义、PR #15 草案、产品规则、插件 `v0.5.0` 通道、Rust/PostgreSQL 实现和 Evidence Library 消费边界的校准，唯一当前入口为 [`architecture/media-lifecycle-contract.md`](architecture/media-lifecycle-contract.md)，决策卡现为 `resolved`。这只证明合同收口；真实媒体字节、OCR/ASR/抽帧/embedding、保留期清理、撤回传播、多材料读模型、页面实现和用户验收仍未证明。第一阶段运行时只剩当前实现缺口基线，首个用户可见范围只剩正式 SCOPE 冻结。Issue #10 和 #14 保持开放以承接当前交付；旧 Issue #11/#16/#18 与 PR #12/#13/#15/#19 已按“已吸收/已被取代”关闭。

### AUD-XHS-001 / Issue #74（受限真实页面探针执行中）

Mog 已于 2026-08-26 明确授权在已连接 Chrome 中执行最小小红书页面探针，用于回答当前搜索上下文/下拉联想、搜索列表、笔记详情（详情、媒体与当前顺序下最多 30 条评论）和作者页究竟有哪些可见字段、插件能否交付、何时停止。该授权严格限于匿名化的字段存在性、数量、页面状态、顺序和缺口记录：不向 Linggan 提交或持久化真实笔记、评论、作者资料、媒体或原始页面/API 输出；不读取 Cookie、账号秘密或隐藏账户资料；不下载媒体、不调用 OCR/ASR、不绕过验证码或安全限制。它是对 `DEV-03` 的受限预检，不表示 DEV-03、真实 Evidence 接入、插件发布、Canary、平台兼容、分析、部署或用户验收已经通过。唯一登记入口为 [`platforms/xiaohongshu/capture-capability-registry.md`](platforms/xiaohongshu/capture-capability-registry.md)，执行计划为 [`plans/active/aud-xhs-001-xiaohongshu-capture-probe-refresh.md`](plans/active/aud-xhs-001-xiaohongshu-capture-probe-refresh.md)。

首轮实测已确认搜索筛选、下拉结构、搜索卡片和作者页 DOM 的一部分当前事实，但同时暴露详情面漂移：当前详情使用 `.note-detail-mask / .note-container / .note-content / .comments-el`，本次页面未出现旧 `noteDetailMap`。Issue #76 已在源码中实现搜索页面事实回执、详情 DOM 回退和统一的“详情 + 媒体观察 + 最多 30 条评论”逻辑结果；其中 `30` 是标准详情窗口硬上限，空状态、部分结果和停止原因分开记录。Issue #78 `PLUGIN-XHS-ACTIVE-COLLECTION-001` 已获 Mog 授权，负责把搜索/主页发现、单篇评论深采和批量评论升级为目标驱动、主动加载、可暂停恢复的执行能力：`30` 不限制独立深采或批量评论，但每项任务必须保留目标、实际、停止原因和恢复状态。

Issue #80 `PLUGIN-XHS-ADAPTIVE-SCROLL-AND-DETAIL-RECEIPT-001` 已经由 PR #81 合并到 `main`：它把“采集当前笔记到 Linggan”与“人工采集并下载媒体”明确拆分，前者不得弹下载窗口，后者保留旧的人工选择和下载；详情回执按笔记详情、媒体观察、评论与回复三条 lane 如实显示。页面加载收敛为有限、可解释的步骤，`no_progress` 不伪装成页面结束；安全验证或访问受限会结束当前 Attempt。此前在一次明确确认下提交的最小详情样本，仅证明一条当前笔记记录进入 Linggan 当前投影；未下载媒体，未证明三条 lane 各自接纳。**真实新版本安装、不同筛选、非空评论/楼中楼、实际 lane 接纳、部署和用户验收仍为 `SOURCE_INCOMPLETE` / 未验证**；代码检查和合并不替代这些真实链路结论。

### PLUGIN-RUNTIME-001 / Issue #50（执行中）

用户已明确拒绝“按搜索、详情、评论、媒体逐张卡重新开放”的路线，改为将完整灵感爆爆爆浏览器插件一次性 retrofit 为 Linggan Intelligence 自有 Browser Producer Runtime。实现必须保留成熟 XHS/抖音页面采集、批量、恢复和原交互，只切断内容工作台运行时；Linggan 负责 TaskSpec、接入、Coverage、receipt、媒体真相与本地读取边界。媒体不再是后续附录：URL 只是 Observation，UI 只能使用 Linggan 本地 asset URL，OCR/ASR 为独立异步派生队列。2026-08-26 的同 Issue #50 / Draft PR #51 最终修订已形成可审查 head：合成“搜索前 20 条”可经 shared package、loopback ingress 到 Evidence Library；无类型接纳的 raw/quarantine 记录不显示为 Evidence 卡片；媒体保留独立上传/失败/派生队列血缘；Popup 已删除旧授权、工位、Cookie 和账号管理处理，并且暂停/继续/停止只在页面控制器实际确认状态转换后显示成功；无可控任务返回 `no_active_task` 且不修改可见进度。隔离且自动清理的 PostgreSQL proof 已通过，但未发生真实平台、账号、媒体、OCR/ASR 或用户验收。**但 v0.4.0 已被用户实际点击验证为工具栏 popup 空白，不能用于 REAL-CANARY-001；Issue #53 正在以 v0.4.2 只修复 popup 启动与诚实 fallback，未合并、未重新加载、未进行真实平台访问。** 本机持久运行库迁移仍受 `.env` 与 Docker 应用角色密码不一致阻塞，未自行运行要求明确授权的密码修复。

### LOCAL-001D / Issue #62（Draft PR 前实现）

REAL-CANARY #52 的聚合结果已确认：本地库存在已接纳的当前可见 discovery 记录，但其来源发布时间均未知；此前 Evidence Library 在 URL 未带窗口时仍隐式使用 `PUBLISHED:30D`，因此会把这些有效材料全部隐藏。Issue #62 只修复这一个读取/表达缺口：缺省 URL 改用显式 `latest_accepted_discovery` 视角，已知与 `PUBLISHED_AT UNKNOWN` 的已接纳卡片都可显示，且不以首次发现、观察、接收或重放时间替代发布时间；显式 7/30 天发布时间窗口继续严格排除未知值并报告排除数。该事项不改真实 Canary、插件、ingress、schema、媒体或分析；当前仍须完成隔离 proof、独立审查、合并后本机 release 和 Mog 验收，不能把代码改变说成真实链路已修复。

项目已完成 `DISC-001` 的产品定义、领域语言、不变量与责任边界基线；这不等于第一版产品形态、准确页面集合或完整前端/后端/数据库/插件/媒体/Agent 运行架构已经收口。`SCOPE-001` 的合成切片范围和实施授权已经确认；代码前语义冻结基线已经在 commit `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 推送到本地记录的 `origin/main`。语义冻结随后经过四轮独立只读攻击，历史结论均如实保留为当时的 G1–G4 FAIL/G5 PASS；第四轮新增的 payload owner、pre-routing audit union、动态 snapshot Oracle 与跨 Attempt Satisfaction 四项 P1 已完成最小收口。用户已明确要求停止重复复核循环并尽快进入代码阶段，因此 SCOPE-001 代码门现为 **CONTROLLED OPEN FOR TDD**。2026-08-21 用户进一步明确确认 F01 采用“可运行、可查询的合成主链优先”：F01 只继续到 synthetic Package → PostgreSQL atomic ingress → two Records → Observation/Current → loopback API → minimal CLI，完成后 hard stop。F01 已完成 Package hash、Record hash、duplicate-key、unsafe-integer、unpaired-surrogate 与 RFC 8785 JCS golden tracer，并已有仅含 F01 的手工静态 manifest Oracle；其余 canonicalization 负例仍未开始且不阻塞 F01 主链。`ARC-001` 已建立为并行的产品与系统架构决策图；其旧版“ARC 未收口前一律不得 Web/插件/真实 producer/媒体”的总禁令，已由 Mog 在 2026-08-24 的 `LOCAL-001` 取代其中关于**独立本地 Linggan**的部分：本地 Web、插件 Local 模式、local data presentation 与 preflight 后的受控 Canary 可分别申请和验证。SCOPE-001 仍保持 synthetic-only；F02–F10、生产、旧系统迁移、Agent crew、外部/线上 host、无界真实采集和无合同敏感材料处理仍未获授权。真实平台仍必须先通过 Local host、插件实际加载版本、localhost contract、隐私/日志、Coverage 与对应 Media/OCR/ASR 处理合同，不因 Local-001 或搜索前 20 条而自动开放。

[`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) 的 `product-shell`、`primary-daily-job` 与 `first-producer-canary` 已 resolved：Linggan Intelligence 是独立且完整的 Web 产品；每天先让 Mog 处理少量值得关注事项，理解来源、出现原因与不确定性，再决定暂不处理、继续观察、进入 Topic 深入或提出行动；首批 Canary 必须连续覆盖发现面、受控详情/媒体 acquisition 与异步 OCR/ASR，而不能让 discovery 冒充媒体完成。内容工作台只是历史原型和能力样本，不是运行时依赖；首页五个名称是产品职责，不是五个已授权运行实例。这里确认的是产品身份和日常用户任务，不表示 Web、P0 或 Agent 已获实现授权。ARC-001 当前已解锁但未回答的页面形态票是 `p0-surface-prototype`。

`SCOPE-001 / F01` 仍是当前进行中的合成事实内核实施；它不因本地页面存在而扩大。`LOCAL-001A` 已经独立审查、合并到 `main`：本机已有一个仅绑定 loopback 的 Rust host，根入口临时跳转到 `/corpus/evidence`，并存在一个诚实空态的 Evidence Library 页面及唯一运行时 Token 值源。它只证明本地入口和页面代码，不证明 Materials read model、真实数据、Evidence/Observation/Capture、数据库、插件/平台、媒体、OCR/ASR、Agent、部署或 Mog/业务验收。`LOCAL-001 / 001B-001C1` 已在 GitHub [Issue #34](https://github.com/Himog0921/linggan-intelligence/issues/34) 的独立 branch/worktree 与 Draft [PR #36](https://github.com/Himog0921/linggan-intelligence/pull/36) 实现受控 discovery 接纳/本地读投影，并在随机、一次性的隔离 PostgreSQL proof database/container/volume 中通过 admission/replay/append-only、已知发布时间的闭区间 WINDOW（未知与未来发布时间均不冒充最近）及 loopback ingress → storage → Evidence Library API/页面 E2E 验证；当前仍未合并，且这不等于真实插件或真实平台完成。001C-2/001C-3 仍未开始，不能从该 Draft 自动开启。Mog 已另行确认 `DESIGN-002`：以「任务启动困难」为例制作一个明确标为 `SYNTHETIC / NOT LIVE` 的 Topic Intelligence Reference Page，并把 [LIDS v2.0](design/lids/README.md) 纳入全项目唯一设计表达标准。LIDS 现在约束未来 UI 的 Token → Primitive → Component → Pattern → Page、L1/L2/L3、状态诚实与 Agent 工作方式，但成熟度仍为 Proposed；LOCAL-001A 的受限 loopback 页面与运行时 Token 源不表示已有通用组件、真实数据、L2/L3、3D/技术栈、权限或动作。DESIGN-002 仍只是 `p0-surface-prototype` 的一个可审查输入，不回答完整 P0，也不扩大 F01 或任何真实数据/权限/行动授权。`PLUGIN-REHOME-001` 的源码隔离与自有 release 基线已由 [PR #42](https://github.com/Himog0921/linggan-intelligence/pull/42) 合并到 `main`（merge commit `3ea210dd2bd426750a30c510e233564a9c35a6f9`）；其 Issue #41 的正式文档收口仍由独立事项处理。它只证明源码归属、build/release 与旧工作台运行切断，不证明真实采集、媒体、OCR/ASR 或页面数据互通。其后 `PLUGIN-RETROFIT-LOCAL-TRUSTED-001` 已由 [PR #46](https://github.com/Himog0921/linggan-intelligence/pull/46) 合并到 `main`（merge commit `123311c8dd98aa5c449b4ea508468a074a88db75`）：在测试期为**合成 manual Discovery**建立 `TaskSpec → Attempt → durable browser outbox → loopback receipt`，保留 partial Coverage、replay 与 attempt terminal conflict 的独立语义；`scheduler` 仍为 `NOT_CONNECTED`。它不证明真实浏览器/UI 触发、XHS/Douyin、账号/Cookie、详情/评论/媒体、OCR/ASR、Evidence Library 真实数据、研究/AI、部署或 Mog/业务验收。Issue #43 在本计划的 Draft 文档审查、合并后重新核验与分层记录完成前保持 OPEN，不能由 PR 合并自动关闭。`GOV-003` 是并行进行的低风险单 Issue/单 PR 治理工作单，不是第二个业务 SCOPE：GitHub [Issue #1](https://github.com/Himog0921/linggan-intelligence/issues/1) 与 draft [PR #2](https://github.com/Himog0921/linggan-intelligence/pull/2) 已建立，但 independent review 对旧 head `a7b63c365c7d3befccc54fd42586c48fe0f22542` 的结论为 **FAIL**，当前只允许在同一 branch/worktree 内修订并请求原 reviewer 复核；不得 merge、close 或把 GOV-003 记为完成。

全项目已正式按“开发阶段”跟踪；阶段路线、每阶段验收证据与需要 Mog 决定的关口统一见 [`development-stage-tracker.md`](development-stage-tracker.md)。本文继续只维护当前快照和唯一下一步，不复制长期路线。

已确认事实：

- GOV-001 已在 commit `73a6dd928a3be01d642efa9aa4c6c5e293c944cf` 推送至 `origin/main`。
- 代码前语义冻结基线 `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 之后的 6 个提交（截至 `64a2141`），以及 F01 ingress 实现 `a45cc61` 和证明记录 `d88fd87`，均已使用 GitHub noreply 身份推送至 `origin/main`。启动 GOV-003 前重新核实 root checkout 为 clean `main @ d88fd8715051aa15da9eeb6860770a03eaad67f6`；这只证明该时点 Git 同步状态，不证明 Issue → worktree → PR → independent review 协作链已跑通。GOV-003 只有在对应 PR 合并并重新核验后才进入当前完成记录。
- 当前机器已有 Git、Rust 和 Cargo。
- Docker PostgreSQL 16.14 已完成真实验证；日常是否正在运行以 `./scripts/dev-db.sh status` 为准。
- ENV-001 采用本机 Rust + Docker PostgreSQL 16，容器内 `psql` 与 `pg_restore` 已通过真实验证。
- ENV-001 验收时，开发库 `linggan_intelligence_dev` 有 0 张业务表，临时 proof 数据库已确认删除；日后需要声称“当前仍为 0”时必须在 Docker 可用状态下重新查询，不能把历史验收当实时状态。
- F01 已完成最小 PostgreSQL foundation 的真实 RED→GREEN：独立一次性 PostgreSQL 16 container 与 test-only volume 内的随机 proof database 从零应用 `0001_scope_001_capture_evidence.sql`，预置一个 Work、一个 Attempt 和两个冻结 target，接受最小 Delivery/Package/Record/target result/Coverage/processing work 关系，并拒绝跨父对象 Delivery；成功或失败均核验删除该 proof database、container 和 volume，未复用开发 PostgreSQL。该 foundation 结果本身不证明 ingress 原子回滚、运行角色权限、worker、Source/Content/Observation/Current、API/CLI 或完整 F01。
- F01 的 Package ingress 已完成真实 RED→GREEN：`crates/evidence` 的接入按判定顺序锁定并核对 Work/Attempt/Capture 同属、验证 epoch/固定合同/冻结 target/authority，然后在一个事务内写入 accepted delivery、Package、两条 Record、target result、Coverage 与两条 processing work 并冻结 Attempt 终态。5 项真实 PostgreSQL 16 测试（各自独立 schema）证明：accepted 行数逐表对齐 `manifest.json`、六个事务故障注入点零半写、同 hash replay 复用原 acceptedReceiptRef、不同 hash conflict 不覆盖既有 package_hash、绕过 Rust 的串父 SQL 仍被拒绝。该结果不证明 Record processing、Observation/Current、worker、API、CLI、F02–F10、运行角色权限负例或任何真实 producer。
- GOV-001 文件治理基线已通过验证并获用户确认，交付见 [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md)。
- GOV-002 已确认使用私有仓库 GitHub Issues、默认五类 triage 角色和单一领域上下文适配；工程技能读取现有 `docs/context/domain-language.md`、`docs/product/domain-invariants.md` 与 `docs/decisions/`，不建立第二套 `CONTEXT.md` 或 `docs/adr/`。
- DISC-001 已确认必须覆盖完整产品，而非只讨论情报核心：语料库、主题词表、主题地图、市场洞察、选题库和外部 Agent CLI 均已进入设计范围。
- DISC-001 已确认 Linggan Intelligence 的稳定核心定义是面向垂直领域的持续情报研究系统；语料、主题地图、市场洞察、内容增长和 Agent 调用是共同内核的应用出口，不能因其中一个应用深入而重新定义项目内核。
- DISC-001 已确认首个领域边界：当前一级 Domain 是 ADHD；ADHD 数学、家庭干预等是 Topic；专题研究和采集计划均发生在 ADHD 内。权威定义见 [`context/domain-language.md`](context/domain-language.md)。
- DISC-001 已确认第一阶段首要用户是产品负责人本人及其小团队，围绕 ADHD 持续进行市场研究、内容判断和选题决策；外部客户与多租户产品能力不进入首期。
- DISC-001 已确认两种产品发动方式：持续观察是默认日常主循环，主动研究由人的问题或经人确认的候选变化触发；两者使用同一资产底座。目标全量必须由实际 Coverage 证明。
- DISC-001 已确认五类长期积累视角：世界事实、语料与表达、领域知识、市场与情报认知、行动与学习。它们用于盘点长期价值，不等于五种平级真相、五张表或五个应用模块；Outcome 只校准未来，不改写过去事实或伪造因果。
- DISC-001 Gate 1 已完成：成功分为观察、资产、认知、决策与学习四层，不要求每次使用都产生 Intelligence 或 Outcome；任务完成、数据量、聚类、评分、报告或 AI 输出均不能单独证明成功。
- DISC-001 Gate 2 已确认六项子决定：受控渐进式采集责任链；有限资源下的统一采集准入与插件执行边界；持续观察中“比较性观察负责测量变化、探索与调查观察负责理解和发现”的解释纪律；主动研究的问题驱动有限闭环；以 Topic 为共同探索入口的多路径资产流转与市场理解边界；真相与派生责任边界。普通采集仍必须满足最低 Evidence 来源与验证底线。
- DISC-001 已建立 [`product/domain-invariants.md`](product/domain-invariants.md) 作为后续 Gate 和实现阶段的长期对抗性验收基线；它冻结系统不能越过的认识、历史、权限和因果边界，不预设物理模型。
- 中期对抗审查已核实：现役小红书 producer 会把相对时间按采集时刻和近似规则推算为 `publishedAt`，且归一化记录不总能保留实际被解析字段的来源；因此新项目必须把原始时间表达、来源、参照时间、解析版本和精度作为后续 Gate 3–5 的硬问题，不能把推算值当精确平台事实。
- 中期对抗审查已核实：Coverage 目前来自 `slotReports`、计数、cursor/`has_more`、终止和失败等多种 producer 事实，没有一个已证明的通用 Coverage 对象；来源事实与针对具体用途的 Coverage 评估必须分开设计。
- 中期对抗审查已确认：跨账号、跨时间的搜索结果可比性仍为 `SOURCE_INCOMPLETE`；它阻塞“近期趋势”承诺，但不阻塞只证明 Capture → Evidence → Observation 的基础切片。
- 中期对抗审查已修正：执行目标错、来源记录内部身份错、采集包身份/hash 冲突和媒体主体错不是同一种错误；Gate 4 必须按真实 producer/fixture 分别确认。
- DISC-001 Gate 2-6A 已在真相分层压力测试后确认：发生记录、Evidence、Source Object/Observation、Derived Analysis、Domain Definition、Claim、Decision/Action/Outcome Observation/Evaluation 分责；观察意图不复制对象事实；Corpus、Topic Map、页面和 CLI 是选择、视图或服务接口，不是线性终点或第二事实源。对抗基线持续按新确认边界增加案例，不用固定数量充当完成证明。
- DISC-001 `DEC-01` 已确认真实材料的最低隐私与传播边界：受限原始 Evidence 默认只在内部按明确用途保存，工作台默认脱敏，外部 Agent 不批量读取原文；获准删除、撤回、脱敏或访问限制必须传播到 Corpus、索引、embedding/feature、摘要、引用、缓存和 Agent 输出，历史判断标记证据资格变化并重新评估。具体保存期限、算法、角色和法律适用性仍留给 Gate 5–7 与专业审查。
- DISC-001 `DEC-02` 已确认第一阶段每日主任务：先从少量值得关注事项判断是否继续观察、进入 Topic 深入研究或提出行动；Topic 是点击后的核心工作区。每日展示不提升事项证据资格，没有合格事项与观察不完整必须分开表达，不能制造变化填满首页。
- DISC-001 `DEC-03` 已确认第一阶段外部 Agent 只能在明确调用者、目的、数据、时间、输出与资源委托内读取、分析、建议和申请。申请不是授权，授权不是执行成功；改变正式知识、长期观察、真实采集、敏感传播或现实行动必须由人确认，确认后仍经过各自正式责任链。
- DISC-001 `DEC-04` 已确认第一阶段不承诺尚未接通的深度评论、私信、咨询、销售等自动 Outcome 闭环。最低边界是 Decision/Action 可追溯、结果渠道可获得性可见，只记录来源明确的实际结果；未接入、未观察、窗口未结束或来源不合格保持未知。旧 schema 指标字段不证明真实结果链已成立。
- DISC-001 `DEC-05` 已确认第一阶段不承诺市场增长/下降等正式趋势，只展示有时间与实际捕获面边界的近期观察和保留替代解释的候选变化。正式趋势等待 Gate 4 真实可比性实验和 Gate 5 统计资格设计通过后按被证明范围开放；当前 `SOURCE_INCOMPLETE` 不阻塞基础观察切片。
- DISC-001 Gate 3 第一项已确认：搜索/作者页发现与逐篇详情/评论采集分责；发现面 Evidence、发现记录、来源身份解析、Source Object 和详情 Observation 不混用。API 返回与页面实际可见是不同来源陈述；发现 N 条不自动产生 N 次深采。真实字段、保留方式、顺序/终止、Coverage、批量和风险策略进入 Gate 4 producer 探查。
- DISC-001 Gate 3 第二项已确认：采集发生记录、Capture Package、Evidence 接入/接受、Record/Source Object 下游隔离、Observation 和 Derived Analysis 分责。包级接入原子且区分重放与 identity/hash 冲突；只有接纳后才暴露的单条对象归属问题进入下游隔离。无效交付留下安全失败与 Coverage 记录但不自动成为 Evidence 或永久原始档案；缺少稳定身份不创建 `Expression Object` 兜底；正常复观测、来源差异、解析更正、不可访问、来源删除和隐私处置不得混成一次覆盖更新。
- DISC-001 Gate 3 第三项已确认：Observation 只追加；迟到材料保留观察、接收和接纳时间而不倒退 Current；parser/结构化修正属于有血缘的解释修订，不制造新世界 Observation；API/页面来源差异未经审计不静默裁定。Current 是带来源、规则和理由的可重算读取责任，允许未知；执行、观察、接收、接纳、来源时间陈述和派生时间不得互相代替。最终字段、来源优先、持久关系和事务仍进入 Gate 4–5。
- DISC-001 Gate 3-4 已确认正式领域语言：Topic Identity 与 Definition Version 分责；Domain Definition Release 由明确发布决定形成；正式词表与实验分析词表隔离；Term/首选名称/Alias、Knowledge Relation、Classification Run/Human Adjudication 和 Map View 分责；定义发布与重分类/统计就绪分开；改名、边界修订、拆分、合并和导航移动分别治理，旧 Claim、分类与统计不自动继承。Claim 是有范围、时间和依据的重要可争论判断，支持、反例与治理历史不压成一个可覆盖分数。
- DISC-001 Gate 3-B 已确认研究、采集权力与行动反馈边界：持续观察目标、主动 Research Question、Collection Plan 和实际 Run 分责；Information Need 不等于插件命令或任务状态；Acquisition Request、人的 Acquisition Authorization、执行前 Admission 和 producer Work Order 分责，采集授权、材料使用许可和插件设备凭证不得互相借用。Decision 固定确切提案/计划版本；Action Proposal/Plan、Action Attempt/Occurrence、Expected Outcome、Outcome Observation 与 Evaluation 分责；研究可以诚实结束或暂时中止，不创建万能 Research、Proposal、Action 或 Workflow。
- DISC-001 Gate 3-C 已确认应用资产与发布视图边界：Corpus 是同一来源材料上的用途化选择；Signal 与注意力处理、Claim 资格分开；Market Insight/Intelligence Brief 固定当时判断表达但不成为第二 Claim；Content Idea、采用决定、成稿、外部发布行动和 Outcome 分责；临时 Agent 回答不自动成为正式情报。
- DISC-001 Gate 3 已于 2026-08-20 获用户整体确认通过。逐项确认、全部确认版本的全链对抗审查和两处跨模块歧义修正均已完成；Gate 3 只冻结领域含义、责任、不变量和后续证明边界，不等于 producer、PostgreSQL、业务代码、AI Agent 或插件升级已通过。
- 2026-08-20 用户正式确认扩展后的 `USER-DEC-02`，并认可 `USER-DEC-01`、`03`、`04`、`05`、`06`。七道 Gate 因此全部关闭；`DISC-001` 已归档为完成计划，正式 `SCOPE-001` 已建立。
- 2026-08-20 用户进一步批准按独立审查修订后的 `SCOPE-001` 开始实现。授权只覆盖计划明列的合成/脱敏 fixture、两份 migration、Rust 事实内核、loopback API、worker、minimal CLI 和验证脚本，不向后续切片外溢。
- 2026-08-20 用户进一步确认代码前语义冻结结论：SCOPE-001 采用 Closed World，独立状态不得压成总成功，unknown 不得被默认值吞掉，低层不得越权产生高层 Claim，任何验证声明必须携带未证明范围。四轮攻击与已知 P1 收口后，用户又明确终止重复文档复核并要求进入代码阶段；因此当前为受控 TDD 开工，不改变上述语义或真实范围硬停止线。
- 2026-08-21 用户逐题确认第一阶段首页情报面形态 `HOME-01`–`HOME-17`，记录在 [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md)，并据此收口 `DEC-G7-01`（首页部分）与 `DEC-G7-02`。该确认只固定产品形态与信息语义，**不是实现授权**：首页、常驻 Agent 编队、总编、通知推送、选题卡存储与校准回路全部未实现，也没有真实数据可驱动；当前在办事项仍只有 `SCOPE-001`，要实现其中任何一节必须另立 SCOPE 并经用户确认。

## 事项队列

| 顺序 | 编号 | 事项 | 状态 | 退出条件 |
|---|---|---|---|---|
| 1 | GOV-001 | 文件治理、索引、冲突与变更留痕 | 已完成 | 规则、索引、记录与自动检查随治理提交进入 `origin/main` |
| 2 | GOV-002 | Agent 工程技能、GitHub Issues 与领域文档适配 | 已完成 | 配置、标签、索引、完成计划和治理检查一致 |
| 3 | ENV-001 | Rust 与 PostgreSQL 16 开发环境 | 已完成 | 固定配置、真实数据库副作用、Rust 检查和用户确认均完成 |
| 4 | DISC-001 | 产品定义、领域语言、不变量与责任边界基线 | 已完成 | 七道设计关口与 `USER-DEC-01`–`06` 已确认；不代表完整产品形态或物理技术架构完成 |
| 5 | SCOPE-001 | synthetic fact-kernel technical tracer | 执行中（F01 主链） | 完成 Package → PostgreSQL atomic ingress → two Records → Observation/Current → loopback API → minimal CLI 后 hard stop；不是用户可见产品切片 |
| 6 | ARC-001 | 产品与系统架构收口决策图 | 进行中（决策） | `product-shell`、`primary-daily-job`、`first-producer-canary` 已解决；P0 页面形态与各项实施合同仍需收口，最后由 Mog 批准首个用户可见 SCOPE |
| 7 | GOV-003 | 外部 Agent Issue → worktree → PR 协作闭环 | 修订中（review FAIL） | 同一 PR 新 head 通过独立复核；integrator 合并后重新核验 main/正式文档、记录完成层并手工关闭 Issue；不是第二业务 SCOPE |
| 8 | DESIGN-002 | Topic Intelligence 合成 Reference Page + LIDS v2.0 | 执行中（设计参考/治理） | Issue #7 将 LIDS 作为全项目设计表达标准，并交付其 L2 合成参考页、规格、组件晋升门和检查；不接入真实数据、Web 产品、运行时主题/组件或真实动作 |
| 9 | LOCAL-001 | 独立本地产品、Evidence Library V7 与 Local Canary 路线 | 001A 与 001B-001C1 已合并；LOCAL-RUNTIME-001（Issue #38）在 Draft PR 验证中；001C-2–3 未开始 | #34 已接通受控 discovery Package → 本地读取代码；#38 只补持久本地 migration/readiness，不证明真实插件、平台、媒体或 OCR/ASR |

同一时间默认只允许一个事项处于“执行中”。状态流转为：`待讨论 → 需要决定 → 已确认 → 执行中 → 验证中 → 已完成`。来源不足使用 `SOURCE_INCOMPLETE`；必须由用户决定的边界使用 `DECISION_REQUIRED`；外部条件无法继续时使用 `BLOCKED`。

## 当前下一步

当前主线实施事项仍是 [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md)。它把已确认设计压缩为一条合成/脱敏技术 tracer：终态 Package 接入、逐 Record 处理、最小 Content 身份与 Observation、字段级 Current 来源，以及 API + minimal CLI 的解释结果；它不是用户可见产品切片。与主线并列、且由 Mog 单独授权的唯一受限现实页面工作是 `AUD-XHS-001 / Issue #74`；它只降低小红书字段与插件兼容性的未知，不接入真实材料，也不扩大 SCOPE-001。

语义冻结已经经过四次独立只读攻击。第四轮发现的 payload/ingress 分层、pre-routing audit union、动态 ref/time snapshot 和跨 Attempt Satisfaction 已分别用 processor owner、封闭数据库 union、固定 proof clock/ref 与 Work 1:1 Attempt 收口。按用户最新裁定不再进行第五轮文档复核。F01 已完成 contracts tracer、手工静态 manifest Oracle，以及 **TDD 步骤 3 的 Package ingress**：有效 Package 在随机隔离的 PostgreSQL 16 schema 中原子接入，行数逐表对齐 manifest 的 `fresh_seed` 与 `final` 阶段，六个事务故障注入点任一失败都零半写，同 hash replay 与不同 hash conflict 只新增一行 delivery 且不覆盖既有 Package，接入后 Observation/Current/Source 侧表仍不存在。三项主链保护中前两项（接入阶段不提前形成 Observation/Current、接入故障零半写）已有真实数据库证据；第三项（坏 Record 不撤销合格 Record）属于 Record processing，尚未实现。下一步是 TDD 步骤 4 的两条 Record 独立处理与 Observation/Current，需要先创建 `0002` migration；随后才是 loopback API 与只经 API 的 CLI。完成该链后停止实施扩张；其余 canonicalization、完整 envelope、资源上限和 F02–F10 等待 ARC-001 收口及后续明确排期，不从 F01 自动继续。

并行的产品决策下一步是推进 [`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) 的 `p0-surface-prototype`。001A 已完成其受限本地入口与诚实空态；#34 已合并，提供 discovery-only 接纳与本地读取投影。当前 Issue #38 只为这条已经合并的本地链补充持久数据库 migration、健康 readiness 与重启保留证明，仍不证明真实采集能力。`DESIGN-002` 的 Topic Reference Page 是 `p0-surface-prototype` 的一个可走查输入，而不是这张票的完整答案。首页职责到运行时的语义已澄清，但这不授权 P0、Agent、真实 producer、媒体或旧系统迁移实现；真实 producer 只能在相应 Local 子卡的 preflight 后申请。`ADV-AUDIT-001` 的独立复核最终处置见 [`audits/pre-implementation-architecture-audit-2026-08-21.md`](audits/pre-implementation-architecture-audit-2026-08-21.md)；它没有改动 F01 migration/合同，也不授权真实访问。

`SCOPE-001` 不包含真实 XHS 访问、真实原文、插件升级、Topic/Corpus/Signal/AI Agent、正式趋势、Web UI、生产部署或旧数据迁移。这个排除继续约束 SCOPE-001 本身；与之并列的 LOCAL-001 只为独立本地产品路线建立后续申请顺序，绝不把真实 producer 实验自动视为已获授权。任何真实搜索、详情、媒体 acquisition、OCR/ASR 或原文处理仍须取得对应的账号、对象、访问、存储、隐私、处理器和原件处置合同并通过 preflight。

已确认边界：

1. Rust 固定 `1.95.0`，使用本机工具链。
2. PostgreSQL 固定 Docker 官方 `16.14-bookworm`，不安装本机 PostgreSQL。
3. 开发库和 proof 数据库与旧项目隔离，禁止恢复旧 dump。
4. Node 固定 `24.13.0`，只用于历史 fixture 验证。
5. `.env` 自动生成本地随机密码，禁止进入 Git。

当前仓库的 Git 提交身份已确认为 `Himog0921 <188755262+Himog0921@users.noreply.github.com>`；它是仓库本地配置，不改变其他项目。

实施前准备矩阵见 [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md)，正式范围以当前 `SCOPE-001` 为准。GitHub noreply 身份和设计基线推送已经真实验证；后续提交仍需逐次核对实际 Git 状态。
