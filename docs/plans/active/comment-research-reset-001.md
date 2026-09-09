# COMMENT-RESEARCH-RESET-001 · 开发期评论研究唯一内核重建

> 状态: 活跃计划
> 最后核对: 2026-09-09
> 适用范围: Issue #213；评论研究的唯一语义内核、研究运行、读取模型、四个研究视图和开发期派生数据重置
> 事实来源: Mog 2026-09-09 明确授权、[DEC-0003](../../decisions/0003-comment-research-single-semantic-kernel.md)、Issue #213 三路只读走查、`origin/main@45ffdf0`
> 冲突时以谁为准: 用户最新决定、`AGENTS.md`、真实代码/迁移/测试/运行证据

## 1. 用户结果

评论研究是把当前可读的评论证据转为可追溯的用户表达与变化，而不是把一套聚类算法的中间状态展示给用户。

- **概览**回答：当前研究覆盖了什么、用户正在表达哪些问题与需求。
- **用户原声**回答：具体原声是什么、来自哪篇作品、是否属于作品作者回复；不显示“等待语义向量准备”。
- **用户问题**回答：哪些不同说法已被研究为同一稳定问题，每个问题可回到原声证据。
- **变化观察**只回答：在两个可比时间窗口中，哪个已定义问题升温、降温、扩散或首次在本系统可用历史中出现；没有可靠变化时，明确说明不可比原因，绝不复用概览填版。

用户保存一次模型/研究策略与总预算后，符合该策略的“开始研究”直接调用；预检仍在服务端冻结范围、估算成本和排除项，但不再成为每次运行的重复人工授权。持续自动排程仍保持关闭，直到 Mog 单独开启。

## 2. 已确认决策与非目标

1. 当前处于开发期。旧研究派生数据可清空；不迁移旧 Problem、Atom、Group、Member、Vector、Candidate 或 Observation。
2. 原始 Evidence、采集包、作品/评论事实、作者归属事实、来源资格和调用账本不可删除或改写。
3. 旧 Task B 与 P4 均不保留为 fallback、双写或页面读取来源。
4. 新内核只持久四类 Atom：`problem`、`need`、`solution`、`experience`。`no_signal` 是 run item 终态，不是 Atom；`stance`、`story`、`quote`、`emotion` 不进入本交付的正式归并体系。
5. 向量模型与索引形态尚未选定。先以实际配置的 embedding adapter 和小样本精确 cosine 证明候选召回质量，再决定是否需要 ANN；不得把未验证 provider、pgvector、HNSW 或模型名称写成既成事实。
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

每个 Atom 包含 `kind`、受约束的 `proposition`、精确 `evidence span`、`basis`、来源/上下文引用、rule/model/input hash。程序在接纳前校验类型、长度、源身份、Unicode 区间与引用资格。无信号、输入不足、模型失败、字段拒绝必须是不同终态。

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
3. 清空/删除旧研究派生：Task B candidate/task/boundary/vector/member/problem 投影，P4 atom/vector/cluster/group/membership/lineage 及旧 result/observation 投影；绝不清空 RawComment、Evidence、作者归属事实、模型调用账本或来源资格。
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

实施进度（2026-09-09）：作者 attribution、ResearchDerivation、保存策略、冻结 Run、逐项 claim 与失败隔离已在 Issue #213 worktree 及 isolated PostgreSQL 证明；真实模型执行、调用/费用回执接入和新 worker 切换留待后续阶段，不能据此宣布 R1 全部完成。

### R2 · Atom、向量与归并

- 实现四类 Atom 接纳、embedding adapter、精确召回 benchmark、候选消歧、Problem/Membership。
- 验收：同义表达归并、近义但不同的问题不合并、`unknown` 角色不入分母、向量错维/NaN/部分响应逐项失败不污染其余 Atom、撤回后不再可读或统计。

实施进度（2026-09-09）：四类 Atom 的接纳边界已落地并由 isolated PostgreSQL 证明：模型只能提交受约束的类型、proposition、basis 与 frozen `research_text` Unicode span；程序根据 ResearchDerivation 的不可变 offsets 生成 source span，拒绝输出时不会写入 Atom 或结算 Item。该模块尚未接入真实模型/调用账本写入；embedding adapter、质量 benchmark、候选召回、模型消歧和 Problem/Membership 仍待实施。

### R3 · 冻结结果与变化

- 实现 ResultRevision、7d-vs-previous-7d、四类观察和 `not_comparable` 原因。
- 验收：规则或 membership revision 变化只影响相应结果；历史回填、采集扩大、无基线、未完成日均不能显示为需求增长；结果只在完整 revision 发布后可见。

### R4 · 读取模型与体验

- 以独立 endpoints/read models 重写四页和运行记录；不再在任意 tab 读取全量研究快照。
- 验收：变化页不渲染问题页/概览副本；技术原因不作为列表标签；当前 1,613 条开发样本与 100k 合成评论分别做 20 次独立测量，单 Tab 数据读取 P95 分别不超过 500ms 与 1s；无数据、未配置 embedding、部分失败、来源受限、不可比均如实表达。

### R5 · 开发库重置准备

- 执行 isolated PostgreSQL 全链路、API、worker crash/recovery、浏览器与治理证明；生成 exact-head deletion/runbook。
- 完成后仅报告“可供 Mog 授权重置”；不自行清空共享数据库、切 runtime、自动研究、调用真实模型、push、merge 或 deploy。

## 7. 直接影响文件（实施时精确收束）

- 新增：`crates/intelligence/src/comment_research_*`、对应 local API/read model、独立 PostgreSQL tests、终态 migration、此计划与必要 data/UI contracts。
- 删除/替代：`comment_problem_worker.rs`、`comment_problem_vectors.rs`、`comment_intelligence_problem_candidates.rs`、Task B 部分 relation/application、P4 compute/organization/adapter/persistence、旧 problem query/read/action/UI 代码与专属 tests。
- 共享接缝：`model_runner.rs`、`lib.rs`、local API 路由、`comment_cleaning.rs`、model invocation adapter、`scripts/local-runtime.sh`；每项改动进入同一 PR 并有替代调用。

## 8. 停止条件与未决项

- embedding provider、真实外发预算或敏感语料处理未获相应授权时，只完成本地/fixture lane；不伪造向量或模型质量。
- 若新问题身份需要自动 merge/split、正式 Topic 变更或不同用户体验路径，标记 `DECISION_REQUIRED`，不自行扩展。
- 若共享数据库含有超出“研究派生数据”的外键/消费者，停止 destructive reset，先报告精确依赖；不通过 cascade 猜删。
- 此计划尚不证明真实模型语义质量、共享数据库重置、运行切换、浏览器验收或 Mog 业务验收。
