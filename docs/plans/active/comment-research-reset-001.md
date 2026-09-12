# COMMENT-RESEARCH-RESET-001 · 开发期评论研究唯一内核重建

> 状态: 活跃计划
> 最后核对: 2026-09-12
> 适用范围: Issue #213；评论研究的唯一语义内核、研究运行、读取模型、四个研究视图和开发期派生数据重置
> 事实来源: Mog 2026-09-09 明确授权、[DEC-0003](../../decisions/0003-comment-research-single-semantic-kernel.md)、Issue #213 三路只读走查、`origin/main@45ffdf0`
> 冲突时以谁为准: 用户最新决定、`AGENTS.md`、真实代码/迁移/测试/运行证据

## 1. 用户结果

评论研究是把当前可读的评论证据转为可追溯的用户表达与变化，而不是把一套聚类算法的中间状态展示给用户。

- **概览**回答：当前研究覆盖了什么、用户正在表达哪些问题与需求。
- **用户原声**回答：经过身份过滤的普通用户原声具体是什么、来自哪篇作品；不显示“等待语义向量准备”。作品作者回复和身份未知评论不进入这一页、问题或变化分母。
- **用户问题**回答：哪些不同说法已被研究为同一稳定问题，每个问题可回到原声证据。
- **变化观察**只回答：在两个可比时间窗口中，哪个已定义问题升温、降温、扩散或首次在本系统可用历史中出现；没有可靠变化时，明确说明不可比原因，绝不复用概览填版。

用户保存一次模型/研究策略与总预算后，在 qualified+enabled embedding 也已就绪时，符合该策略的“开始研究”直接调用；语义提取、向量候选和问题归并调用均写入同一 Run 的通用调用账本并受该总预算约束。预检仍在服务端冻结范围、估算成本和排除项，但不再成为每次运行的重复人工授权。研究模型也必须满足唯一的 V1 语义就绪判断：精确 config 所引用的 model/connection version 仍启用，且该 model/version 最新 `probe` 是成功、`ok`、`modelCallable` 与 `semanticQualified` 都为真。该判断用于保存默认模型配置、保存策略、启动 Run 和每次 generation reservation；reservation 会持有与 probe 写入/完成相同的版本化 PostgreSQL advisory lock 直到 adapter 返回，防止已写入的失败 probe 在资格判断与外发之间越过边界。任一处不满足都不发送评论。embedding 未就绪时，页面和服务端都拒绝创建 Run，避免产生必然无法归并和发布的无效调用。持续自动排程仍保持关闭，直到 Mog 单独开启。

## 2. 已确认决策与非目标

1. 当前处于开发期。旧研究派生数据可清空；不迁移旧 Problem、Atom、Group、Member、Vector、Candidate 或 Observation。
2. 原始 Evidence、采集包、作品/评论事实、作者归属事实、来源资格和调用账本不可删除或改写。
3. 旧 Task B 与 P4 均不保留为 fallback、双写或页面读取来源。
4. 新内核只持久四类 Atom：`problem`、`need`、`solution`、`experience`。`no_signal` 是 run item 终态，不是 Atom；`stance`、`story`、`quote`、`emotion` 不进入本交付的正式归并体系。
5. LOCAL-EMBEDDING-001 覆盖本条旧约束：V1 的唯一 embedding 是本机 `Tencent/WeMM-Embedding-2B` 固定 commit，Atom canonical_text 以 `document` 模式编码为 512 维并 L2 normalize，写入 pgvector 后只做 exact cosine Top-K；没有 ANN/HNSW、外部 embedding provider 或 Problem-definition embedding。Profile 绑定 model id/revision、编码模式、维度与 preprocessing 版本，候选召回不能跨 profile，也不能直接写 membership。
6. V1 不自动 merge/split 已发布 Problem；新的残余表达最多成为待定义候选，不能仅凭聚类取得永久身份。

## 3. 唯一数据链与最小数据合同

```text
RawComment（不可变证据）
  → ResearchDerivation（研究正文、角色、上下文、资格）
  → ResearchRun / ResearchRunItem（冻结范围、调用与终态）
  → SemanticAtom（提议、依据片段、规则版本）
  → AtomEmbedding（版本化向量）
  → Problem / AtomProblemMembership（稳定身份与归并依据）
  → ResultRevision（完成态冻结结果）
  → ProblemWindowStat / ChangeObservation（可比窗口派生）
```

### 3.1 Research Derivation

`ResearchDerivation` 以当前可读 comment source、source hash 与 cleaner/attribution policy 版本为键，至少保存：

- `research_text`：只用于研究的派生正文；原始 `body_text` 永不更新。
- `author_role`：`ordinary_user | content_author_reply | author_identity_unknown`。
- `attribution_basis`：作品作者身份来自何处、为何可比较或未知。
- 父评论、作品正文等上下文的可读引用和 hash，不复制其正文真相。
- `eligibility`：可研究、不可研究或暂不可确定，以及确切原因。

只有 `ordinary_user + eligible` 能进入新的研究 Run；`content_author_reply` 是可回看的作品作者语境，`author_identity_unknown` 保持未知且不进入问题/变化分母。

### 3.2 Atom 与模型职责

生成式模型只负责：从冻结的 `research_text` 与必要、可读的上下文中抽取结构化 Atom；在候选关系存在歧义时判断同一/不同/不确定；为获接纳的新 Problem 生成受限定义。它不计算趋势、不决定重要性、不直接创建正式 Topic，也不自行决定发布。

每个 Atom 包含 `kind`、受约束的 `proposition`、精确 `evidence span`、`basis`、来源/上下文引用、rule/model/input hash。程序在接纳前校验类型、长度、源身份、Unicode 区间与引用资格。无信号、输入不足、模型失败、字段/证据合同拒绝必须是不同终态。调用账本与 Run 读取只保存并呈现安全阶段/计数：JSON 不可解析、字段/范围合同拒绝和 Unicode span 无法映射回原评论相互区分，不保存原始评论或模型原文。

V1 的 `AtomProblemMembership(relation='same')` 只接受 `problem` 与 `need`：两者都可表达同一个待解决的用户问题。`solution`、`experience` 保持为 Atom/原声证据，不能被伪装成“与问题相同”；若以后需要关联它们，必须新增有独立语义和验收的关系，而不是扩大 `same`。

### 3.3 Problem 与 Membership

Embedding 产生同类型 Top-K 候选；确定性规则拒绝明显不同的类型/范围；只有剩余歧义才调用受限 LLM 归并判断。`AtomProblemMembership` 必须记录：`atom_ref`、`problem_ref`、`relation`、`basis`、definition/membership policy hash、决策模型版本、证据引用和创建时刻。

同一 Atom 同时只可有一个当前 Problem 归属。Problem 的名称可修订，但 `problem_ref` 不变；定义重大变化会生成新的 definition revision，并使受影响的统计重算，而不是悄悄改写历史窗口。

### 3.4 Result Revision 与变化

页面只读取已 `published` 的 `ResultRevision`；正在清洗、提取、向量化或归并的结果不会半成品混入页面。每个 revision 固定：范围、`as_of`、样本 manifest hash、research/attribution/embedding/membership policy hash、输入/失败/排除计数。

变化比较固定为 Asia/Shanghai 的两个完整自然周：`[as_of-14d, as_of-7d)` 对 `[as_of-7d, as_of)`；不使用当天未结束的部分日。统计同时计算去重评论占比和作品覆盖率。发布 `升温`、`降温`、`扩散`、`新出现` 前，必须满足冻结 membership basis、两期覆盖和范围可比性；否则只返回 `not_comparable` 与原因。它们是研究输入范围内的观察，不是“市场正在增长/下降”的断言。

`新出现` 的含义固定为“首次出现在本系统可用且可比的研究历史中”，不是现实世界第一次出现。

## 4. 模块边界与复用判断

| 模块 | 决定 | 原因 |
|---|---|---|
| 评论 Evidence 读取、当前资格、`as_of`、父评论/作品上下文 | 直接复用 | 是证据与权限接缝，不含旧语义判断。 |
| `comment-clean.v2` 与 Unicode 原文定位 | 直接复用 | 是原始文本到派生文本的可验证基础。 |
| 作品作者归属 | 小改复用 | 统一为一个 attribution projection，替代当前多条 author role 判断。 |
| 调用账本、预算、lease、局部恢复、受控外部计算 | 直接复用接口，重写研究专属调度 | 保留可审计副作用；杜绝 poison work 阻断新任务。 |
| 旧 Task B candidate/relation/vector | 删除 | 旧候选与 member 语义不再参与新系统。 |
| P4 V4/V5 Atom adapter、HDBSCAN/Leiden、旧 group/lineage | 删除 | 新 Atom 合同和归并内核不兼容；不迁移其结果。 |
| `linggan_ci_source`、全 Tab 超级查询、旧变化观察 | 删除并重写 | 混合身份、资格、算法、页面和运行状态，造成 16 秒加载与页面重复。 |
| local-only API 壳、no-store/host guard、isolated PostgreSQL harness | 复用 | 与旧语义无关。 |

新内核提供少量深接口：`prepare_run`、`advance_run`、`publish_revision`、`read_overview`、`read_voices`、`read_problems`、`read_changes`、`read_run`。页面不得跨越这些接口读取算法内部表。

## 5. 开发期破坏性重置

本交付先在独立数据库完成 new-schema migration 与证明，不碰共享库。若 Mog 后续授权共享开发库重置，顺序固定为：

1. stop 接受新的旧研究任务，并等待旧研究 worker 无在途 lease/invocation；不停止采集和 Evidence 写入。
2. 部署已验证的新代码和新 schema，但尚不启动真实模型调用。
3. 清空/删除旧研究派生：Task B candidate/task/boundary/vector/member/problem 投影，P4 atom/vector/cluster/group/membership/lineage、旧 result/observation 投影，以及早期开发分支写入的 V1 policy/run/derivation/Atom/vector/Problem/result 行；绝不清空 RawComment、Evidence、作者归属事实、模型调用账本或来源资格。
4. 建立新的 empty 研究内核；旧分析结果不作为新 Atom 输入。
5. 由用户明确启动一轮新研究后，才从新 Run 生成首个 ResultRevision。

历史 migration 文件不修改、不删除；迁移历史是仓库证据。新 migration 必须以终态 schema 明确删除旧派生结构、创建新的单一结构，不能让运行时同时维护旧/新表。

## 6. 实施阶段与验收

### R0 · 合同、删除图和 test oracle

- 冻结新表、DTO、错误分类、状态词典、API surface、删除顺序及旧模块清单。
- 建立脱敏固定 fixture：普通用户、作者回复、身份未知、父评论、短但有语义、无信号、相似/不同表达、撤回来源。
- 验收：不再有新代码依赖 `linggan_ci_problem_member`、`linggan_ci_problem_candidate`、`linggan_ci_semantic_group` 作为新路径输入。

### R1 · 正确研究输入与运行

- 实现单一作者 attribution / research derivation；模型策略保存后直接运行；将 recoverable、incompatible、unrecoverable、model_failed 分开。
- 验收：`作者` 徽标不进入 `research_text`；一条旧不兼容恢复记录不阻断一条新研究；取消或 prepare 失败没有模型调用；调用、费用与 source hash 可追溯。

实施进度（2026-09-12，隔离候选）：已把研究模型 V1 语义就绪判断收束为唯一事务内 predicate，并接入默认模型配置、策略保存、Run 启动和 generation reservation。该候选还将 `semantic_json_unparseable`、`semantic_contract_rejected` 与 `semantic_evidence_offset_unmappable` 写入安全的 item/code 汇总；模型设置 projection 的外层 alias 已修正并有隔离 PostgreSQL 回归。它尚未合并、切换 runtime 或对真实评论/模型生效，真实模型语义质量仍须在用户主动启动首轮后单独验收。

### R2 · Atom、向量与归并

- 实现四类 Atom 接纳、embedding adapter、精确召回 benchmark、候选消歧、Problem/Membership。
- 验收：同义表达归并、近义但不同的问题不合并、`unknown` 角色不入分母、向量错维/NaN/部分响应逐项失败不污染其余 Atom、撤回后不再可读或统计。

实施进度（2026-09-10）：四类 Atom 的接纳边界、版本化 embedding、exact cosine Top-K、候选消歧与稳定 Problem/Membership 已接入 V1 worker 和通用调用账本。模型只能提交受约束的 Atom 类型、proposition、basis 与 frozen `research_text` Unicode span；程序根据 ResearchDerivation 的不可变 offsets 生成 source span，拒绝输出时不会写入 Atom 或结算 Item。只有 `problem`/`need` 能以 `same` 进入稳定 Problem；Problem 只能通过显式 `new_problem` 或 `same_problem` 决定写入，一个 Atom同时只有一个 current membership，definition revision、policy hash、basis、decision evidence 与 invocation 均留痕。错维/NaN/零范数拒绝，candidate recall 最多返回十个仍有可读成员的定义候选，绝不直接写 membership。真实供应商语义质量与 gold-set benchmark 不由 synthetic infrastructure proof 代替，仍需在用户主动运行后单独验收。

### R3 · 冻结结果与变化

- 实现 ResultRevision、7d-vs-previous-7d、四类观察和 `not_comparable` 原因。
- 验收：规则或 membership revision 变化只影响相应结果；历史回填、采集扩大、无基线、未完成日均不能显示为需求增长；结果只在完整 revision 发布后可见。

实施进度（2026-09-10）：ResultRevision 的严格发布门和 Asia/Shanghai 完整 7d 对前一完整 7d 的计算已由 isolated PostgreSQL 证明，并由 V1 worker 作为最后一步发布。它要求 Run `completed`、所有 `problem`/`need` Atom 有 current membership、且每个 frozen source 仍可读；不满足即不发布。每个 Problem 记录评论占比和作品覆盖率，覆盖不足写 `not_comparable`。0066 约束同一 Problem 的每种 published signal 最多一条，保留 `newly_observed`、`rising/falling` 和 `spreading` 的并存证据；published revision 只读且重复发布幂等。真实历史回填、规则/definition revision 变更后的重算和真实结果质量仍需单独验收。

### R4 · 读取模型与体验

- 以独立 endpoints/read models 重写四页和运行记录；不再在任意 tab 读取全量研究快照。
- 验收：变化页不渲染问题页/概览副本；技术原因不作为列表标签；当前 1,613 条开发样本与 100k 合成评论分别做 20 次独立测量，单 Tab 数据读取 P95 分别不超过 500ms 与 1s；无数据、未配置 embedding、部分失败、来源受限、不可比均如实表达。

实施进度（2026-09-10）：V1 的五个 read model/API 与唯一页面已经替换旧入口：`overview`、`voices`、`problems`、`changes` 与 `runs`。每个请求在 repeatable-read 只读事务中选择一个 readable published ResultRevision；指定 revision 不可读时返回明确 unavailable，schema 缺失时不伪造空研究结果。概览只返回当前问题与覆盖，原声返回 raw evidence / research text / author role / research outcome 而没有 embedding 状态，问题返回冻结的 Definition 与双窗口事实，变化只返回 published Observation 和 `notComparable` 原因。隔离 PostgreSQL 已证明同一冻结 revision 的五个投影互不混用。真实浏览器交互与性能的上线验收进入 R5，不用源码替代。

### R5 · 开发库重置、运行切换与上线验证

- 执行 isolated PostgreSQL 全链路、API、worker crash/recovery、浏览器与治理证明；生成 exact-head deletion/runbook。
- Mog 已于 2026-09-10 授权本事项完成开发库旧研究派生数据的清除、source merge、localhost runtime 切换和上线测试；不授权自动启动持续排程或因部署而发起真实评论模型调用。
- 切换前必须完成 exact terminal migration、隔离 PostgreSQL、API/worker/browser proof，并逐项核对 RawComment、作者归属、来源资格、模型调用账本和通用模型配置仍在。

实施进度（2026-09-10）：0069 终态 migration 已在完整历史 migration fixture 上通过，并明确删除旧表、视图、触发器、函数与 identity index（含 `linggan_ci_source_revision`），以及任何早期 V1 开发派生行；没有使用 `CASCADE`。0070 使 derivation identity 覆盖作者归属和回复上下文：冻结 Run 读取其原始可读输入，新 Run 才读取当前 head；输入变化使整个已发布结果撤出可读投影。20 项 isolated PostgreSQL V1 proof、2 项 API proof、Pi adapter SDK proof、Rust compilation 和 API route/page guard 已完成。PR #217/#219 已进入 main；共享开发库已应用 0064–0070，`runtime-main` 同步旧的 `638b6cd` 并通过 loopback health，浏览器已实际看到 V1 的五视图空态。部署前后未创建真实模型调用。qualified+enabled embedding 的页面/服务端双重运行门已有 isolated proof，待本 guarded revision 合并、runtime 切换以及页面和 POST 回执现场核验后才能记为当前运行时事实；当前没有 embedding provider，故首轮真实运行、向量/语义质量、tab 性能测量与 Mog 业务验收仍待下一张明确的模型配置/首轮运行任务。

## 7. 直接影响文件（实施时精确收束）

- 新增：`crates/intelligence/src/comment_research_*`、对应 local API/read model、独立 PostgreSQL tests、终态 migration、此计划与必要 data/UI contracts。
- 删除/替代：`comment_problem_worker.rs`、`comment_problem_vectors.rs`、`comment_intelligence_problem_candidates.rs`、Task B 部分 relation/application、P4 compute/organization/adapter/persistence、旧 problem query/read/action/UI 代码与专属 tests。
- 共享接缝：`model_runner.rs`、`lib.rs`、local API 路由、`comment_cleaning.rs`、model invocation adapter、`scripts/local-runtime.sh`；每项改动进入同一 PR 并有替代调用。

## 8. 停止条件与未决项

- embedding provider、真实外发预算或敏感语料处理未获相应授权时，只完成本地/fixture lane；不伪造向量或模型质量。
- 若新问题身份需要自动 merge/split、正式 Topic 变更或不同用户体验路径，标记 `DECISION_REQUIRED`，不自行扩展。
- 若共享数据库含有超出“研究派生数据”的外键/消费者，停止 destructive reset，先报告精确依赖；不通过 cascade 猜删。
- 此计划尚不证明真实模型语义质量、共享数据库重置、运行切换、浏览器验收或 Mog 业务验收。
