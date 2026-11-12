# DECISIONS

记录设计决定、与现有实现的冲突及取舍。未获得负责人裁定的冲突不写入“已决定”，只链接到 `QUESTIONS.md`。

本文件中 D-001 至 D-020 为负责人于 2026-11-12 作出的裁定，**与实施手册具有同等约束力**。手册 v1.1 已同步修订；两者如仍有出入，以本文件为准，并在 QUESTIONS.md 提出。

> 状态: 权威当前
> 最后核对: 2026-11-12
> 适用范围: 机会卡（Opportunity Card）实施的设计裁定与取舍，D-001 至 D-020。
> 事实来源: 负责人在 2026-11-12 对 Q-01 至 Q-10 的答复，以及仓库代码、迁移与合同。
> 冲突时以谁为准: 负责人最新裁定；本文件与实施手册冲突时以本文件为准，并回到 QUESTIONS.md。

---

## D-000 2026-11-12 T0.1 勘察基线

- 决定：以 `main@ad19a4d7` 的实际代码和实际迁移为事实来源；`docs/opportunity/RECON.md` 未记录的现有符号视为不存在。
- 理由：手册 §3.1/§3.2 要求先勘察后编码，且代码/合同优先于历史文档。
- 影响：后续任务引用现有表、列、函数、路由、配置项时，必须先能在 RECON 中找到。

---

## 一、基线与工程约定

### D-001 先同步基线，再继续（RECON 补充）

- 事实：RECON 记录工作树 `main...origin/main [behind 53]`。RECON 描述的是 53 个提交之前的代码。
- 决定：
  1. 执行 `git fetch` 与 `git pull --ff-only`，**不得**改动、提交或丢弃他人未提交的 `docs/README.md` 与未跟踪文件。无法快进时停止并询问。
  2. 对 `ad19a4d7..HEAD` 做差异勘察，范围是 `database/migrations/`、`crates/intelligence/`、`crates/evidence/`、`apps/api/src/local_web/`、`apps/web/src/app/`、`docs/data-contracts/`、`env.example`。
  3. RECON 中受影响的条目逐条更新，并在 RECON 顶部记录新基线提交号和迁移链头。
- 归属：任务卡 T0.1b（见手册 v1.1 §7）。

### D-002 既有失败的基线

- 事实：`creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work` 失败（503 ≠ 200）；`cargo fmt --check` 在既有测试支持文件上失败。
- 决定：
  1. 两者作为“既有失败”记入 PROGRESS.md 的基线段，**不在机会卡任务中修复**。
  2. 每个 PR 的验证输出必须证明：失败集合与基线相同，没有新增失败。
  3. `cargo fmt --check` 只要求新增和修改的文件通过，用 `cargo fmt -- --check <文件列表>` 或等价方式证明。
  4. 同步到新基线（D-001）后，重新记录一次既有失败。

### D-003 前端依赖

- 决定：涉及前端的任务开始前，在 `apps/web` 用现有 lockfile 执行 `npm ci`，并记录输出摘要。之后按 RECON 记录的 `npm run verify` 验证。**不得**修改 lockfile 中与本任务无关的条目。

### D-004 命名、路由与接口约定（覆盖手册中的写法）

| 手册 v1.0 写法 | 实际采用 |
| --- | --- |
| schema `opportunity` | `public` schema，表名前缀 `linggan_opportunity_`，例如 `linggan_opportunity_problem` |
| 路由 `/api/...` | `/api/local/opportunity/...`，在 `apps/api/src/local_web/opportunity*.rs` 中实现 `routes()` 并在 `local_web.rs` 挂载 |
| 响应字段 snake_case | 接口字段一律 camelCase（serde `rename_all = "camelCase"`）；请求体同时加 `deny_unknown_fields`；**枚举值保持 snake_case**，例如 `"suspected_gap"` |
| 错误格式 | 读接口用现有 `local_read_json_error`；写接口复用现有本地写入 guard 与对应错误 helper |
| 分页 | `cursor` + `limit`（默认 50，范围 1–100），响应 `page { limit, hasMore, nextCursor, asOf }` |
| 业务模块位置 | `crates/intelligence/src/opportunity.rs` 与 `crates/intelligence/src/opportunity/` |
| 前端页面 | `apps/web/src/pages/opportunity/`；新路径同时登记到 `apps/web/src/app/pages.ts` 与 `apps/api/src/local_web/web_app.rs` 的 `REACT_PAGES` |
| 日志 | 沿用 `println!` / `eprintln!`；R9 的检查方式改为 grep 新增的打印语句 |

字段名换算规则：去掉下划线后驼峰，例如 `as_of` → `asOf`，`rate_per_1k` → `ratePer1k`，`ci_per_1k` → `ciPer1k`，`mentioners_total` → `mentionersTotal`。

### D-005 迁移只能向前

- 事实：仓库没有 down migration；迁移由 `scripts/local-runtime.sh` 的 ledger 机制顺序执行，并校验 checksum。
- 决定（覆盖手册 §4、§10 中“迁移可回滚”的要求）：
  1. 遵循仓库约定，只写前向迁移。编号接在同步后的链头之后，并按现有方式登记到 `scripts/local-runtime.sh`。
  2. 机会卡迁移**只创建** `linggan_opportunity_*` 对象（表、视图、函数、触发器）。**不得 `ALTER` 任何现有表。** 确有需要时，单独提出并等待裁定。
  3. “回滚”的含义：关闭特性开关；如需清除，再写一个前向迁移删除 `linggan_opportunity_*` 对象。
  4. 迁移文件一旦应用就不得修改（ledger 会因 checksum 不同拒绝继续）。修正一律用新迁移。
  5. 隔离测试仿照现有 `scripts/test-*-postgres.sh`，新增 `scripts/test-opportunity-postgres.sh`。

### D-006 特性开关（答复 Q-02，选 A）

- 环境变量 `LINGGAN_OPPORTUNITY_CARD`。**只有取值严格等于 `enabled` 时开启**；缺省、空值和其他任何值都视为关闭。
- 在 API 和 worker 启动时读取一次，存入各自的状态结构，不在请求中途重复读取。
- 关闭时：
  - 所有 `/api/local/opportunity/*` 路由返回 HTTP 404，响应体为 `local_read_json_error` 格式，`code = "opportunity_card_disabled"`；
  - 机会相关的 worker 任务不启动；
  - 前端导航不显示“机会”入口。页面路由仍需登记（以满足 `pageRegistry.test.ts`），直接访问时显示“未启用”；
  - 现有页面和接口的行为与现在完全一致（R14）。
- 前端通过一个只读接口 `GET /api/local/opportunity/status` 获知开关状态。该路由是唯一在关闭时也返回 200 的机会接口，响应 `{ "enabled": false }`。
- `env.example` 增加一行注释说明，默认不开启。

---

## 二、时间、身份与口径

### D-007 首次采集时间（答复 Q-09，选 A）

- 手册中 `observed_at` 的含义是“首次采集时间”。在机会卡领域，它**统一改名为 `first_observed_at`**，以免与现有 `linggan_material_comment.observed_at`（单次观察时间）混淆。
- **不修改** `linggan_material_comment`。`first_observed_at` 由视图或函数推导：
  1. 优先：`linggan_material_comment_first_admission.first_package_ref` → `linggan_runtime_capture_package.accepted_at`；
  2. 没有 first_admission 记录时：取该稳定评论身份所有观察中最早的 `observed_at::timestamptz`，同时标记 `first_observed_backfilled = true`；
  3. `observed_at` 文本无法解析为时间时，该评论不进入任何指标，计入排除原因 `observed_at_unparseable`（见 D-012），**不得静默丢弃**。
- T0.2 的验收改为：统计三种来源（first_admission / 最早观察回填 / 无法解析）各有多少条评论，写入 PROGRESS.md。
- 手册 R12 中的 `observed_at <= as_of` 一律读作 `first_observed_at <= as_of`。

### D-008 统计周期的时间基准（答复 Q-01）

- 事实：评论没有发布时间；作品有 `linggan_material_content_detail.published_at`，并有现成的精确度约束（`0026_work_resource_read.sql:17-25`）。
- 决定：
  1. 新增口径参数 `period_basis`。v1 取值为 `work_published_cohort`：**每条评论归入其所属作品发布时间所在的周**。
  2. 作品发布时间取截至 `as_of` 的最近一次详情观察中非空的 `published_at`。没有合格发布时间的作品，其全部评论不进入周期指标（k 和 n 同时排除），计入排除原因 `period_anchor_missing`。
  3. 口径含义随之调整：提及率表示“**该周发布的作品下的评论中**，提及该问题的比例”。界面文案改为“提及率（每千条评论，按作品发布周）”。
  4. **不采用**方案 B（从正文推断评论时间），也**不**用采集时间代替发布时间。
  5. 同时进行补充勘察（T0.1b 第 3 项）：采集侧原始 package 或合同 fixture 中，评论对象是否带有平台给出的评论时间。如果有，另立任务把它以只追加的方式存入 `linggan_opportunity_comment_published`（新表，不改 0016）。待覆盖率达到 90% 以上，再新增口径版本 v2，把 `period_basis` 切到 `comment_published` 并全量重算。在此之前不得混用两种基准。
- 已知局限（写入 D-016 的页面说明）：越近期发布的作品，积累评论的时间越短，所以当周 n 偏小。`min_n_period` 门槛会拦住样本不足的周，比例本身受影响较小，但不能完全消除。

### D-009 领域分母（答复 Q-04，选 A）

- 领域内的作品集合 = `linggan_material_domain_usage` 中 `domain_ref = 目标领域`、`use_kind = 'domain_research'`、`role = 'primary'` 的作品。
- **分子 k 和分母 n 必须使用同一作品集合**。不在集合内的作品，其评论上的提及不计入 k（保持 k ⊆ n）。
- `reference` 和 `creator_context` 不进入机会卡的任何分母或分子。
- 补充勘察（T0.1b 第 4 项）：`linggan_material_domain_usage` 是否有可用于 `as_of` 判断的时间列。有则按 `as_of` 过滤，没有就记录为已知局限。

### D-010 作者归属（答复 Q-05，选 A）

- 作品作者统一用 `linggan_material_content_author_at(content_public_ref, as_of)`，不使用非 `as_of` 的视图。
- 作者为 NULL 的作品，其全部评论不进入周期指标和作者集中度（k 和 n 同时排除），计入排除原因 `work_author_unknown`。
- 排除比例超过 `max_author_unknown_share`（v1 = 0.2）时，卡片 flags 加 `author_unknown_share_high`。
- “作者自评”判定：评论的 `author_external_id` 与作品作者 `author_external_id` 相等，记为 `creator_self`，从 k 和 n 中同时排除。评论者 ID 为 NULL 时无法判定，按非作者处理，保留在 n 中（见 D-013）。

### D-011 稳定评论身份（答复 Q-06，选 A）

- 评论身份 = `(content_ref uuid, comment_external_id text)`，其中 `content_ref` 即 `content_public_ref`。
- 手册中所有 `comment_id` 都指这一对值。`comment_mention` 的主键改为 `(content_ref, comment_external_id, problem_id, taxonomy_version)`。
- 可另存 `source_material_ref uuid`，记录是哪一次观察带来的这条提及，只用于溯源，不参与去重。
- 接口中评论引用的格式：`{ "contentRef": "<uuid>", "commentExternalId": "<text>" }`。

### D-012 排除必须计数

- 新建函数 `linggan_opportunity_eligibility_exclusions(domain_ref, as_of)`，按原因返回被排除的评论数，原因闭集为：`period_anchor_missing`、`work_author_unknown`、`creator_self`、`restricted`、`observed_at_unparseable`、`not_in_domain_primary`。
- 每次指标计算都记录这组计数（写入运行记录或日志），卡片 flags 按 D-008、D-010 的阈值生成。
- 这条落实手册 §3.4“静默跳过数据行”的禁令。

### D-013 评论者哈希（答复 Q-08，选 A）

- 密钥：独立于 `account_digest_key`。文件放在 `${LINGGAN_SUPPORT_DIR}/opportunity/commenter-digest.key`（`LINGGAN_SUPPORT_DIR` 缺省为 `$HOME/.linggan-support`，与 `model_secrets.rs` 的约定一致），至少 32 字节。读取方式参照 `crates/intelligence/src/model_secrets.rs`，如有可复用的读取函数就复用。
- 密钥缺失或不足 32 字节时，机会卡的写入和计算任务拒绝启动，报明确错误。读接口仍可返回已有读模型。
- 算法：复用 `crates/evidence/src/collection_control.rs:1432-1454` 的 HMAC-SHA256 实现。若它不是 `pub`，允许把可见性改为 `pub`，**不得改变其行为**，现有 RFC 4231 测试必须继续通过。这一改动要在 PR 中单独说明。**不得**复制一份加密代码。
- 存储：`commenter_hash TEXT`（十六进制）加 `commenter_digest_version TEXT`，v1 为 `commenter-hmac-sha256-v1`。
- 评论者 ID 为 NULL：`commenter_hash = NULL`，`excluded_reason = 'commenter_unknown'`。该评论保留在 n 中，不计入 k，与现有合同“未知作者不能作为独立支持”一致（`docs/data-contracts/comment-study-http-001.md:43`）。
- 不按领域加盐。轮换方式：换密钥 → 新的 digest version → 全量重算。同一次计算中不得混用两个版本。

### D-014 用户问题来源（答复 Q-07，选 A，附确认步骤）

- 以 0177 的 `linggan_comment_problem_definition` / `linggan_comment_problem_membership` 为**唯一活动来源**。0064 的旧表不读、不导入。
- `linggan_opportunity_problem.source_ref` = `problem_ref`（稳定问题 id）。新增列 `source_definition_ref`，记录当前定义。同步时如果发现同一 `problem_ref` 的 `definition_ref` 变了，`def_version` 加 1。
- 评论关联链：`membership.request_ref` → `linggan_comment_research_request.source_ref`（= `linggan_material_comment.material_ref`）→ 该观察的 `(content_public_ref, comment_external_id)`。
- 字段类型跟随现有类型：`domain_ref`、`problem_ref` 等用现有的 uuid 或 text 类型，**不使用手册 DDL 中的 BIGINT**。
- 层级：v1 导入的问题一律 `level = 1`、`parent_id = NULL`。三级层级的整理不在 v1 范围内。
- 开工前确认（T0.1b 第 5、6 项）：
  1. 用 `rg` 确认当前代码不再写入 0064 的 problem 相关表。如果仍有写入，停止并询问。
  2. 确认 0177 是否存在问题合并、取代或撤销的语义（字段、状态、工作流）。有的话，映射到 `problem_lineage`，并把映射规则写入本文件；没有的话，v1 中 `problem_lineage` 只建表、不自动写入。

### D-015 向量来源（答复 Q-10，选 A）

- T2.2 关闭向量来源，只用评论来源和搜索来源。理由：检索调用未确认，现有 embedding 也不覆盖作品理解摘要。
- 不新建 embedding 任务。以后如需启用，另行立项。

---

## 三、测量局限与 AI 输入

### D-016 评论抽样上限是已知的测量局限

- 事实：采集能力登记表显示，每次评论采集的目标是 `min(公开评论数, 30)`（`docs/platforms/xiaohongshu/capture-capability-registry.md:68-69`）。所以 n 是“我们采到的评论”，而且受平台排序影响。
- 决定：
  1. 不做统计修正，但必须如实说明。首页固定说明改为：“提及人数反映讨论热度，不等于问题对用户的重要性；评论只代表愿意留言的人，且每篇作品只采集平台排序靠前的部分评论。”
  2. 机会卡的“依据与局限”区块固定显示上述局限和 D-008 的时间基准说明。
  3. 补充勘察（T0.1b 第 7 项）：真实数据中每篇作品已采评论数的分布，以及多次采集是否会补到新评论。结果写入 RECON，作为解读 n 的依据。

### D-017 作品回答判定的输入（预先裁定，供阶段 2 使用）

- 引文校验（手册 §6.6.3 第 5 步）只能针对**作品原始文本**：截至 `as_of` 的 `linggan_material_content_detail.title` 与 `body_text`。整篇理解结果可以作为辅助上下文提供给模型，但引文必须能在原始文本中找到。
- 如果使用整篇理解，只用 `scope = 'note'` 的结果，不用 `note_with_comments`，避免把评论内容当成作品的回答。
- 图片识别文字和口播转写：目前 RECON 中没有可用字段，v1 不使用。补充勘察（T0.1b 第 8 项）后再决定。
- 新的判定任务需要注册 `linggan_ai_task`、batch 的 task/kind 约束，以及 egress 的 material_class（RECON §4.1 末尾）。这一步在 T2.1 开始时提交方案，**由负责人单独批准**后再实施。

### D-018 时间文本解析

- 现有 `observed_at` 是 text。所有 `::timestamptz` 转换集中在 D-007 的视图或函数里完成，失败按 D-012 计数，不得在其他地方各自转换。

### D-019 点赞未知

- 作品和评论的点赞只在 state 为 `KNOWN` 时取值，其余一律为 `null`，不得写成 0（与 `comment-like.v1` 一致）。
- 代表原声按点赞排序时，`null` 排在最后，再按 `first_observed_at` 升序、评论身份升序。
- “领域近 90 天作品点赞中位数”只用 `KNOWN` 值计算，并同时记录参与计算的作品数。这个数为 0 时，供给状态不能进入第 4 步，停在 `insufficient_coverage`，并加 flag `engagement_reference_missing`。

### D-020 时区与周期计算放在 PostgreSQL（答复 Q-03，选 B）

- 不新增 Rust 日期库。周期函数用 SQL 实现，放在机会卡迁移中：
  - `linggan_opportunity_period_start(ts timestamptz, tz text) RETURNS date`，实现为 `date_trunc('week', ts AT TIME ZONE tz)::date`（PostgreSQL 的 week 从周一开始）；
  - `linggan_opportunity_period_bounds(p date, tz text) RETURNS TABLE(start_utc timestamptz, end_utc timestamptz)`。
- `tz` 只能由调用方从 `metric_definition.params.period_tz` 传入，函数内不写死时区。参数加载器只接受 `week_start = "monday"`，其他值拒绝加载。
- 手册 §6.4.3 的四条标准答案用 `scripts/test-opportunity-postgres.sh` 在隔离数据库中验证。
- Rust 统计模块（手册 §6.4.1）保持纯函数，不涉及日期。

---

## 待裁定

无。Q-01 至 Q-10 均已答复，见上文。新问题继续写入 `QUESTIONS.md`。
