# 当前状态与事项队列

> 状态: 权威当前
> 最后核对: 2026-08-20
> 适用范围: 当前阶段、事项顺序、阻塞与下一步
> 事实来源: 本机实际检查、已确认项目边界和完成计划
> 冲突时以谁为准: 真实运行结果、ACCEPTED ADR 与用户最新确认

## 当前阶段

项目已完成 `DISC-001` 七道基础设计关口，`SCOPE-001` 的合成切片范围和实施授权已经确认；代码前设计基线已经在 commit `680eff35a1c5babae70eac642aee5afe22435e1f` 推送到 `origin/main`。随后终审发现，多个独立状态、unknown、Claim 层级和验证范围仍可能被后续 Agent 合理但错误地压缩。用户已确认先完成 G1–G5 语义冻结；当前只允许收口 SCOPE、fixture 正负 Oracle 和 Agent 执行合同，代码门仍关闭。真实平台访问、真实数据库生产接入、AI Agent 内核和插件升级仍未获准开始。

已确认事实：

- GOV-001 已在 commit `73a6dd928a3be01d642efa9aa4c6c5e293c944cf` 推送至 `origin/main`。
- 代码前设计基线已在 commit `680eff35a1c5babae70eac642aee5afe22435e1f` 使用 GitHub noreply 身份推送至 `origin/main`；本地与远端在推送后核对为 `0/0` 分歧。
- 当前机器已有 Git、Rust 和 Cargo。
- Docker PostgreSQL 16.14 已完成真实验证；日常是否正在运行以 `./scripts/dev-db.sh status` 为准。
- ENV-001 采用本机 Rust + Docker PostgreSQL 16，容器内 `psql` 与 `pg_restore` 已通过真实验证。
- ENV-001 验收时，开发库 `linggan_intelligence_dev` 有 0 张业务表，临时 proof 数据库已确认删除；日后需要声称“当前仍为 0”时必须在 Docker 可用状态下重新查询，不能把历史验收当实时状态。
- Rust workspace 仍是骨架；尚无业务 DDL、migration 或已证明的 Rust 业务链路。
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
- 2026-08-20 用户进一步确认代码前语义冻结结论：SCOPE-001 采用 Closed World，独立状态不得压成总成功，unknown 不得被默认值吞掉，低层不得越权产生高层 Claim，任何验证声明必须携带未证明范围。该确认没有撤销合成切片实施授权，但在 G1–G5 最终复核前保持代码门关闭。

## 事项队列

| 顺序 | 编号 | 事项 | 状态 | 退出条件 |
|---|---|---|---|---|
| 1 | GOV-001 | 文件治理、索引、冲突与变更留痕 | 已完成 | 规则、索引、记录与自动检查随治理提交进入 `origin/main` |
| 2 | GOV-002 | Agent 工程技能、GitHub Issues 与领域文档适配 | 已完成 | 配置、标签、索引、完成计划和治理检查一致 |
| 3 | ENV-001 | Rust 与 PostgreSQL 16 开发环境 | 已完成 | 固定配置、真实数据库副作用、Rust 检查和用户确认均完成 |
| 4 | DISC-001 | 从头梳理产品、功能、领域、采集、数据库和架构 | 已完成 | 七道设计关口与 `USER-DEC-01`–`06` 全部经用户确认 |
| 5 | SCOPE-001 | 合成/脱敏 Content Evidence 首个垂直切片 | 执行中（语义冻结） | G1–G5 全部通过并完成一次独立对抗复核，随后才进入 TDD |

同一时间默认只允许一个事项处于“执行中”。状态流转为：`待讨论 → 需要决定 → 已确认 → 执行中 → 验证中 → 已完成`。来源不足使用 `SOURCE_INCOMPLETE`；必须由用户决定的边界使用 `DECISION_REQUIRED`；外部条件无法继续时使用 `BLOCKED`。

## 当前下一步

当前唯一在办事项是 [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md)。它把已确认设计压缩为一条合成/脱敏链路：终态 Package 接入、逐 Record 处理、最小 Content 身份与 Observation、字段级 Current 来源，以及 API + minimal CLI 的解释结果。

早先针对 SCOPE 范围的独立对抗审查已经完成，原 1 个 P0、7 个 P1 和 5 个 P2 均已裁定并吸收，记录见 [`audits/scope-001-final-adversarial-audit-2026-08-20.md`](audits/scope-001-final-adversarial-audit-2026-08-20.md)。后续全项目实施就绪终审又发现了“范围已确认但执行语义仍可被 Agent 压缩”的新风险，因此当前不直接进入 TDD：先按 [`agents/scope-001-execution-contract.md`](agents/scope-001-execution-contract.md) 完成 G1–G5，并对冻结后的版本再做一次独立对抗复核。只有复核没有新增 P0/P1 语义歧义，才创建 fixture、migration、Rust 业务代码和 CLI。

`SCOPE-001` 不包含真实 XHS 访问、真实原文、插件升级、Topic/Corpus/Signal/AI Agent、正式趋势、Web UI、生产部署或旧数据迁移。`AUD-XHS-001` 继续后置；真实 producer 实验必须重新取得账号、对象、访问与原件处置授权。

已确认边界：

1. Rust 固定 `1.95.0`，使用本机工具链。
2. PostgreSQL 固定 Docker 官方 `16.14-bookworm`，不安装本机 PostgreSQL。
3. 开发库和 proof 数据库与旧项目隔离，禁止恢复旧 dump。
4. Node 固定 `24.13.0`，只用于历史 fixture 验证。
5. `.env` 自动生成本地随机密码，禁止进入 Git。

当前仓库的 Git 提交身份已确认为 `Himog0921 <188755262+Himog0921@users.noreply.github.com>`；它是仓库本地配置，不改变其他项目。

实施前准备矩阵见 [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md)，正式范围以当前 `SCOPE-001` 为准。GitHub noreply 身份和设计基线推送已经真实验证；后续提交仍需逐次核对实际 Git 状态。
