# DECISIONS

记录设计决定、与现有实现的冲突及取舍。未获得负责人裁定的冲突不写入“已决定”，只链接到 `QUESTIONS.md`。

本文件中 D-001 至 D-034 为负责人于 2026-11-12 作出的裁定；D-035 至 D-048 记录后续接手裁定。**本文件与实施手册具有同等约束力**。两者如仍有出入，以本文件为准，并在 QUESTIONS.md 提出。

---

> 状态: 权威当前
> 最后核对: 2026-11-13
> 适用范围: 机会卡（Opportunity Card）实施的设计裁定与取舍，D-001 至 D-048；未裁定冲突见文末。
> 事实来源: 负责人对 Q-01 至 Q-28 的答复，以及仓库代码、迁移与合同。
> 冲突时以谁为准: 负责人最新裁定；本文件与实施手册冲突时以本文件为准，并回到 QUESTIONS.md。

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
  5. ~~补充勘察评论时间并计划 v2~~ 已由 D-021 取代：评论平台时间的覆盖率只有 0.14%，v2 暂不启用。
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

- 新建函数 `linggan_opportunity_eligibility_exclusions(domain_ref, as_of, tz, period_basis)`，按原因返回被排除的评论数，原因闭集为：`period_anchor_missing`、`comment_time_unknown`、`work_author_unknown`、`creator_self`、`restricted`、`observed_at_unparseable`、`not_in_domain_primary`。
- 每次指标计算都记录这组计数（写入运行记录或日志），卡片 flags 按 D-008、D-010 的阈值生成。
- 这条落实手册 §3.4“静默跳过数据行”的禁令。

### D-013 评论者哈希（答复 Q-08，选 A）

- 密钥：独立于 `account_digest_key`，至少 32 字节。**存放路径和读取方式已由 D-022 更正**（原写的 `$HOME/.linggan-support` 有误）。
- 密钥缺失或不足 32 字节时，机会卡的写入和计算任务拒绝启动，报明确错误。读接口仍可返回已有读模型。
- 算法：复用 `crates/evidence/src/collection_control.rs:1432-1454` 的 HMAC-SHA256 实现。若它不是 `pub`，允许把可见性改为 `pub`，**不得改变其行为**，现有 RFC 4231 测试必须继续通过。这一改动要在 PR 中单独说明。**不得**复制一份加密代码。
- 存储：`commenter_hash TEXT`（十六进制）加 `commenter_digest_version TEXT`，v1 为 `commenter-hmac-sha256-v1`。
- 评论者 ID 为 NULL：`commenter_hash = NULL`，`excluded_reason = 'commenter_unknown'`。该评论保留在 n 中，不计入 k，与现有合同“未知作者不能作为独立支持”一致（`docs/data-contracts/comment-study-http-001.md:43`）。
- 不按领域加盐。轮换方式：换密钥 → 新的 digest version → 全量重算。同一次计算中不得混用两个版本。
- v1 不识别或标记营销号；本阶段只对评论者身份缺失设置 `commenter_unknown`。`marketing` 保留为手册中的未来排除类型，不据此新增分类算法。

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
- 图片识别文字和口播转写：~~v1 不使用~~ 已由 D-026 更新，存储已确认存在。
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

## 四、T0.1b 之后的裁定（2026-11-12）

### D-021 评论平台时间（答复 Q-11）

> **2026-11-12 修订：本条第 1、3 款已由 D-028 取代。** 评论时间不再是“暂不启用”，而是由阶段 C 主动补齐，并以它为目标口径。第 2、4 款继续有效。

- 事实（RECON §7.3）：带精确 epoch 的评论只有 45/31809（0.14%）；`publishedAtText` 都有值，但多数是日精度的展示日期（常缺年份）或相对于采集时间的“N 天前”。
- 决定：
  1. v1 继续以作品发布周为统计基准（D-008）。`period_basis = comment_published` 暂不启用。覆盖率数字写入“已知局限”。
  2. **不采用方案 C**（解析展示文本）。原因：精度混杂、年份缺失、相对时间依赖采集时刻；而能拿来核对解析结果的精确样本只有 45 条，无法验证解析是否正确。以后如果要做，必须作为单独的验证研究：先积累足够的精确样本（至少 200 条），逐行标注精度，与精确值比对，并且不得与精确时间混用。
  3. **方案 B 移交采集侧**，不属于本项目，也不阻塞本项目：建议采集侧把平台 `create_time` 以毫秒 epoch 按只追加的方式写入。
  4. **数据完整性风险，需要报告给采集侧负责人**：`plugins/linggan-intelligence-browser/src/db/recordNormalization.js:162` 写的是 `record.publishedAt || record.createdAt`。如果 `createdAt` 存在，采集时间就会被当成发布时间写入。机会卡任何时候都不得读取评论的 `publishedAt` 字段。

### D-022 支持目录与密钥路径（答复 Q-12，选 A，并收敛为一处解析）

- 更正：D-013 中的缺省路径是我根据 T0.1 版 RECON §4.5 写的，那一条本身有误。T0.1b 已核对代码，实际缺省是 `$HOME/Library/Application Support/Linggan Intelligence`。
- 决定：
  1. 密钥文件为 `<support_dir>/opportunity/commenter-digest.key`。其中 `<support_dir>` 必须由**与 `model_secrets.rs` 相同的那一段解析逻辑**得出：`LINGGAN_SUPPORT_DIR` 优先，否则用代码中的缺省值。
  2. 实现方式：把 `crates/intelligence/src/model_secrets.rs:39-43` 的支持目录解析，以及私有文件的权限校验，提取为 `pub` 函数，行为不变（加一个测试断言提取前后结果相同），然后由 `model_secrets` 和机会卡共同调用。**不得再写第二份路径解析或缺省值。**
  3. `media_worker_support`、`paddle_ocr.py` 等其他地方的重复解析不在本项目范围内，不要改动。可以在 PROGRESS.md 的遗留项中记录“支持目录解析存在多处副本”。
  4. 文档、错误提示中不写死具体路径，提示语应包含实际解析出的路径。

### D-023 领域归属的时点判断

- 事实（RECON §7.4）：`linggan_material_domain_usage.created_at` 是入库时间。旧数据迁移回填的行，其 `created_at` 是迁移执行时间（`0103_domain_membership_and_usage.sql:163-169`）。
- 决定：
  1. 普通行按 `created_at <= as_of` 过滤，同一作品多行时按 `content_public_ref` 去重。
  2. 迁移回填行视为“自始有效”，即不受 `as_of` 限制。执行者先在 RECON 中记录回填行 `basis_kind` 的确切取值（以 0103:163-169 为准），再按该值判断。
  3. 这样做在回测中会引入未来信息（回填的归属当时未必成立）。T4.2 的回测报告必须写明有多少作品依赖回填行。线上实时计算（`as_of` 接近当前）不受影响。

### D-024 测试参数与生产参数分开

> **2026-11-12 修订：第 2 款已由 D-029 取代。** 生产参数恢复为与测试参数集相同的数值。第 1、3、4 款继续有效。

- 事实（RECON §7.7）：每篇作品已采评论数 P50 = 14，P90 = 30；全库共 29017 条评论、1735 篇作品。手册 §6.3 的阈值是在看到数据之前定的。
- 决定：
  1. **测试参数集**：保持手册 §6.3 的数值，永久固定。§6.4.4、§6.5、§8.2 的标准答案一律在测试参数集下计算，不得修改。
  2. **生产参数 v1**：在测试参数集基础上只改两项：`min_n_period = 100`，`creator_min_n = 30`。理由：Wilson 下界已经会惩罚小样本，最小样本门槛只用来挡住极端噪声。按每篇作品 P50 为 14 条，`creator_min_n = 50` 会把大多数作者排除在排序之外。
  3. T0.3 的 seed 迁移写入生产参数 v1。测试参数集放在测试代码里，不写入生产库。
  4. T0.2 完成后，执行者用只读查询出一份分布报告：
     - 每个领域最近 12 周每周的有效评论数 n（P25、P50、P75）；
     - 作者 90 天窗口内的有效评论数分布；
     - 在生产参数 v1 下，各证据等级的问题数，以及充足与样本不足的作者数。

     报告交负责人。如果多数领域的周 n 中位数低于 100，停止并报告（可能需要改成更长的统计周期，届时另行裁定）。

### D-025 回复与一级评论

- 决定：`linggan_material_comment` 中的一级评论和回复同等计入（作者本人的回复按 `creator_self` 排除）。T0.2 中如果能区分两者，在运行记录中报告回复所占比例；不能区分就记录“无法区分”。

### D-026 作品回答判定的输入（更新 D-017）

- 事实（RECON §7.8）：图片识别文字和口播转写已有存储，整篇材料快照 `linggan_note_material_fragment` 含 `title`、`cover_headline`、`body`、`tags`、`ocr`、`transcript` 字段（`0130_note_material_snapshot.sql:27`）。
- 决定（阶段 2 生效）：
  1. 判定的原始文本优先使用整篇材料快照的 `title`、`body`、`ocr`、`transcript` 字段。引文校验针对这四个字段拼接后的文本（每段之间加一个换行）。
  2. T2.1 开始时，执行者先确认三点：该快照能否按 `as_of` 选取；快照是否已排除受限来源；它是否就是整篇理解所用的输入。三点都成立才采用；否则退回 D-017 的 `title` + `body_text`，并在 DECISIONS 中记录原因。
  3. D-017 的其余规定不变：只用 `scope = 'note'`；新 AI 任务的注册方案须单独批准。

### D-027 计数以稳定评论身份为准

- 事实（RECON §7.2、§7.7）：first_admission 覆盖全部 29017 个稳定身份；`observed_at` 全部可解析；观察行（31771）与包记录（31809）相差 38 条。
- 决定：
  1. 机会卡的候选评论集合就是 first_admission 中的稳定身份。观察行和包记录的条数都不作为计数依据，所以这 38 条差异不影响任何指标，v1 不再追查。
  2. `observed_at_unparseable` 在当前库为 0，但检查和计数仍要保留（D-012），以防将来的数据出现问题。
  3. 0177 中一个 `problem_ref` 只对应一条定义，`def_version` 在 v1 中恒为 1。问题同步只会新增问题，不会更新。`problem_lineage` 只建表、不写入（D-014）。

## 五、数据供给是本项目的一部分（2026-11-12）

负责人决定：评论时间和评论数量都由我方采集能力保证，属于本项目的交付范围，而不是要去适应的外部限制。为此新增**阶段 C（评论时间与采集协议）**，与阶段 0、阶段 1 并行推进（任务卡见手册 v1.3 §7）。

### D-028 评论发布时间是目标口径

- 目标：机会卡的统计周期最终按**评论的平台发布时间**切分（`period_basis = comment_published`）。作品发布周（`work_published_cohort`）是过渡口径。
- 采集侧改造（阶段 C，任务 C1–C3）：
  1. 插件把平台的评论时间（`create_time` 等，见 `plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:93-114`）以毫秒 epoch 写入记录。
  2. **删除回退**：`recordNormalization.js:162` 不再用 `record.createdAt` 顶替发布时间。取不到时写“未知”（`null`），**不得写 0，也不得写采集时间**。
  3. 改造后的包必须能和改造前的包区分开，例如带上合同版本或协议版本字段。具体字段由 C1 勘察确定，不得假设。
  4. 新建只追加的表 `linggan_material_comment_platform_time`，以 `(content_public_ref, comment_external_id)` 为键，记录 `published_at`、`precision`、`source_package_ref`、协议版本。数据由投影器从包中提取，同一身份以第一条有效值为准；多个包给出的值不一致时计数并报告。**不 `ALTER` 现有评论表。**
- 历史数据：
  - **只能通过重新采集来补**（C4）。重新采集仍在作品可访问、且现有采集控制（频率、风险冷却）允许的范围内进行。
  - 展示文本（“3 天前”“11-02”）**不用作发布时间**，只在 C3 中用来交叉核对 epoch 是否正确：同一条评论，epoch 换算成上海时间的日期，应当与绝对日期型展示文本一致。
- 切换规则：
  1. 切换前，`comment_published` 口径以**影子模式**同步计算并保存，不展示，用来和作品发布周口径对比。
  2. 当某个领域最近 `series_periods + baseline_periods`（12）周内的有效评论中，有平台时间的比例达到 90% 以上时，该领域可以切换。切换时新增口径版本，全量重算。
  3. 因为统计窗口只看最近 12 周，改造上线后即使不补历史数据，大约 12 周也能自然达到切换条件；重新采集可以提前。
  4. 切换后，没有平台时间的评论计入排除原因 `comment_time_unknown`，**不混用**作品发布周口径。
- 验收数据（C3）：平台时间覆盖率；`published_at <= first_observed_at` 的违例数（必须为 0）；与展示日期的一致率（目标 99% 以上，不一致的逐条列出）。
- **2026-11-12 修订：第 2 项“历史数据”中的“展示文本不用作发布时间”已由 D-034 取代。** 展示日期与相对时间在明确的精度与推算规则下可以作为平台时间；epoch 仍然优先。D-028 其余条款继续有效。

### D-029 采集量目标与生产阈值

- 负责人确认：每个领域每天约 100 条评论的采集量可以做到。据此：
  1. **采集目标**：每个领域每周不少于 700 条有效评论。这是运营与采集侧的服务目标，机会卡负责如实显示是否达标。
  2. **生产参数 v1 恢复为与测试参数集相同**：`min_n_period = 500`，`creator_min_n = 50`，取代 D-024 第 2 款。
  3. 新增 flag `collection_below_target`：某领域当周有效评论数低于 700 时出现。新增参数 `collection_target_per_period = 700`。
  4. 采集不足时，页面显示“样本不足”，**不降低门槛**。“样本不足”本身就是给采集侧的工作信号。
- D-024 第 4 款的分布报告照常交付，用来核对采集目标的实际达成情况。

### D-030 采集协议必须有版本

- 理由：采集量扩大后，最大的风险是**采集方式变了，被误读为讨论变了**。例如从“热门排序前 30 条”改为“按时间排序全量”，提及率会因样本构成变化而突变，与用户需求无关。
- 决定：
  1. 每个评论采集包记录采集协议：排序方式（热门 / 最新）、单次条数上限、触发来源（定时、关键词、补采）。字段由 C1 勘察后确定；没有现成字段就通过合同变更新增，**不得用插件版本号代替**。
  2. 指标计算按协议版本分组。同一统计窗口内出现协议变化时，卡片 flag `collection_protocol_changed`，趋势图在变化点做标记。
  3. 协议变化后，基线只用新协议的周期重新积累；不足 `baseline_periods` 时，显示 `no_baseline`，不做显著性判断。
  4. 建议（需 C1 确认平台能力）：在热门排序之外，增加按时间排序的采集。按时间排序的样本受“谁更受欢迎”的影响更小，更接近讨论的真实分布。机会卡统计使用哪一种协议，由负责人在 C1 的勘察结果出来后决定，**决定之后保持不变**。

### D-031 数据再多也不改变的两条

- 提及人数不等于重要性：`not_calibrated` 照常显示，直到导入问卷（T4.3）。
- 评论只代表留言者：不得用评论数据推算用户占比。
- 这两条写在页面的“依据与局限”中，不因采集量增加而删除。

### D-032 评论排序协议固定为平台默认顺序（答复 Q-13，选 A）

- 事实（RECON §8.1）：小红书顶层与子评论请求都不带排序参数；记录里的 `sortMode` 固定写 `'unknown'`，只是“未记录”，不是平台事实。能力登记只证明当前默认顺序可见，平台是否支持时间排序未验证。
- 决定：
  1. 机会卡统计固定使用**平台默认顺序**，`collection_protocol.sort_mode = "platform_default"`。不尝试按时间倒序，不实现双协议分组；该决定在 C 阶段及之后保持不变。
  2. 采集包新增 `collectionProtocol`：`version`、`sortMode`、`singleRunLimit`、`triggerSource`。它是 D-030 要求的协议版本，也是区分改造前后的标识；`triggerSource` 取值为 `manual`、`scheduled`、`incremental`、`unknown`。
  3. 默认顺序下，单次窗口不保证“最新 N 条”。补新评论依靠重复采集；重新采集使用 C2 之后的协议。单次上限按实际请求记录，全量遍历时写 `null`。
  4. C2 的实现必须让新包携带该字段，并让旧包（无该字段）继续被接收。协议变化时按 D-030 第 2、3 款处理：指标按协议版本分组，出现变化时 flag `collection_protocol_changed`。

### D-033 暂停期数据不作为周期口径的停止条件（答复 Q-14）

- 事实：2026-11-12 的只读分布报告显示最近 12 个有数据周期的周中位偏低；负责人说明前端采集已手动暂停一个多月，数据库中的数据周期反映暂停状态，不是项目设计或采集能力的结论。
- 决定：
  1. D-024 第 4 款的分布报告继续保留为基线观察，但暂停期数据不触发“停止并改周期”的裁定。
  2. `period = week`、`period_basis = work_published_cohort` 保持 v1 口径不变；不改 4 周窗口。
  3. 采集恢复后重新运行同一只读报告；届时的分布才作为周期口径与采集目标的判断依据。
  4. 实现按手册设计继续推进，不以当前库的数据量阻塞 T0.4 及之后任务。
- 影响：Q-14 已答复；PROGRESS 中 T0.2 的停止状态更新为“继续”。

### D-034 展示日期与相对时间作为日精度平台时间（答复 Q-15）

- 事实：派发会话拿不到评论 API 的毫秒 epoch，但 2026-11-12 的只读核查显示 32,219 条评论/回复全部带有 `publishedAtText`。格式为 `MM-DD`（18,103）、`YYYY-MM-DD`（6,661）、`N天前`（3,614）、`昨天`（2,080）、`N小时前`（1,518）、`N分钟前`（183）、`刚刚`（14）、ISO 文本（45，且这 45 条另有 epoch）。年份规则经数据验证：小红书只对非当年打印 `YYYY-MM-DD`；当年只打印 `MM-DD`，且不存在晚于采集日的月日。
- 决定：
  1. **接受日精度**。`linggan_material_comment_platform_time.precision` 允许 `'millisecond'` 与 `'day'`；日精度值存为**上海当地零点**（`00:00 Asia/Shanghai`），并记录来源 `source_kind`：`epoch`、`display_full_date`、`display_month_day`、`display_relative`。
  2. **相对时间按该条记录自己的采集时刻推算**。“3天前”“2小时前”“刚刚”“昨天”等以记录里的 `collectedAt`（13 位毫秒）为基准；记录缺失时退回包裹的 `accepted_at`。不得使用当前时间或“今天”，否则历史回填会整体偏移。`N小时前`/`N分钟前` 按实际时长回退后再取上海日期；`N天前`/`昨天` 按上海日历日回退。
  3. **`MM-DD` 的年份用采集日推断**：先取采集时刻所在上海年份；若候选日期晚于采集日，则归入上一年。`YYYY-MM-DD` 原样采用。无法解析的文本不写入，不猜测。
  4. **epoch 优先**。同一评论可同时存在日精度与毫秒行（主键改为 `(content_public_ref, comment_external_id, precision)`）。读取统一走视图 `linggan_material_comment_platform_time_current`，毫秒行优先。日精度行不会阻止以后采到精确时间。
  5. **epoch 的可信范围不变**：协议 v1 的包，或无协议但记录 `sourceTier = 'api'` 的历史包。历史包用 `createdAt` 顶替的 `publishedAt` 仍然不采信，改用展示文本。
  6. **展示文本交叉核对只比较毫秒行**，避免用日精度值验证它自己的来源；API 记录的 `publishedAtText` 是 epoch 的 ISO 序列化而非展示标签，核对时跳过。相对标签常带时分（`昨天 14:35`），解析时剥离时分，标签本身决定日期。
  7. 采用范围：**新采集与历史回填同时进行**。回填由投影器从既有包重放，不重新采评论；重新采集仍按 D-028 的采集控制进行。
- 影响：Q-15 已答复；D-028 的历史数据条款更新；`period_basis = comment_published` 的 90% 覆盖率切换门槛因此可在不依赖评论 API 的情况下达成；日精度进入指标时必须按“当天”归入周期，不虚构时刻。

---

### D-035 唯一问题身份与分类版本历史（2026-11-13）

- 背景：手册 §6.1 的 `problem.id BIGSERIAL` 与已合并的 `0205_opportunity_problem_taxonomy.sql` 不一致；后者以 `problem_ref UUID` 为主键。0205 的 `linggan_opportunity_taxonomy_version` 只保留当前版本，不能仅凭该表恢复更早版本的时间。
- 负责人确认：G0 与 RECON 按已报告的证据范围放行；当前处于开发期，以程序和代码逻辑一致为重点，不建立两套并行身份。历史开发数据的重要性不改变迁移及不可逆处置的授权边界。
- 决定：机会卡问题使用现有 `linggan_opportunity_problem.problem_ref UUID` 作为唯一身份；评论提及、指标和后续机会卡均引用它，不新建 BIGSERIAL 映射身份。`source_ref` 仍指 0177 的 `problem_ref`（D-014）。
- 分类版本仍须满足 R12：通过前向新增的版本历史保存已知的版本及其生效时间；既有当前版本行只作为当前指针，历史与指针必须在同一事务中同步。缺少可靠时间的历史不得伪造，不可确定时明确报告不可用。
- 已应用迁移不改字节；本地运行库和生产环境迁移仍需负责人另行明确授权。隔离测试使用临时测试库，不代表真实环境迁移或业务验收。

---

### D-036 T1.1 归属人数对账（答复 Q-16）

- 现有评论研究问题接口不返回逐问题去重作者人数，依据列表还有 100 条上限，不能将其视为完整人数。
- 从 0177 的完整归属与稳定评论身份独立计算源侧去重人数，与机会卡提及表按相同集合逐问题核对；另行报告机会卡资格排除、已发布问题提及人数不足 2 的情况及原因。
- 对账是验证用途，不建立另一套持久业务计数，也不把发布门槛与机会卡资格后的数值强制当成同一口径。

---

### D-037 评论平台时间按时点选择（答复 Q-17）

- 机会卡读取评论平台时间时，先排除来源包 `accepted_at` 或投影行 `created_at` 晚于 `as_of` 的记录，再按 D-034 在剩余记录中优先选择毫秒精度、其次日精度；没有合格记录则按 `comment_time_unknown` 处理。
- 使用一套共享选择逻辑。当前视图复用该逻辑；资格函数以请求的 `as_of` 调用它。此处更新 D-034“统一走当前视图”的技术表达，保留其精度优先和来源规则。
- 使用前向迁移；不得修改已应用迁移，也不得伪造历史投影时间。本地运行库及生产环境迁移仍需单独授权。

---

### D-038 评论协议归属及指标分组（答复 Q-18）

- 每条稳定评论以首次入档包记录的采集协议作为该评论的协议归属；首次入档包或协议字段缺失时保持未知，不把未知写成协议版本 0 或平台默认排序。
- 问题周期指标按协议分别保存，分组字段属于同一指标表的维度；同一基线不混合协议。基线不足时明确标记 `no_baseline`，不得据此声称显著变化。
- 未知协议可单独成组并明确标记；它不等于任何已知协议。不同协议的评论不得重复计入同一个指标分组。

---

### D-039 评论事实与评论者身份按时点选择（答复 Q-19）

- 唯一的评论事实选择逻辑先限制来源包 `accepted_at <= as_of`、评论观察 `created_at <= as_of`，再按 D-007 优先选首次入档包的合格观察，否则选合格观察中最早的一条。当前事实视图、当前同步和历史计算共用该逻辑。
- 历史指标按同一 HMAC 密钥与算法计算该时点选出的评论者身份哈希；不得把后来覆盖的派生哈希冒充历史哈希，也不另建平行的持久身份体系。
- 首次入档表没有行建立时间，历史计算无法证明当时该行是否已建立；记录这一限制，不伪造时间。来源包与观察时间仍须满足上述约束。

---

### D-040 作品作者与发布时间的观察时点（答复 Q-20）

- 前向修正现有共享 `linggan_material_content_author_at`：保留来源优先级和排序，同时限制详情、发现观察的 `created_at <= as_of`。继续由所有调用方使用同一函数，不新建平行作者口径。
- 机会卡选择作品发布时间锚点时，同样限制详情观察的 `created_at <= as_of`。补充共享调用方及机会卡的隔离回归；本地运行库与生产环境迁移仍需单独授权。

---

### D-041 最新详情缺少发布时间时不回退旧值（答复 Q-21）

- 在 `as_of` 已接纳且已创建的详情观察中先选择最近一条，再检查该条的作品发布时间；为空时标记 `period_anchor_missing`。
- 不再使用旧实现“先筛非空时间、再选最近一条”的回退逻辑。此决定只针对机会卡周期锚点；不改变共享作者函数的来源排序。

---

### D-042 作者集中度按采集协议分组（答复 Q-22）

- 作者集中度沿用 D-038 的首次入档包协议归属；协议缺失保持未知并单独成组，不与已知协议合并。
- 按作者、问题、窗口、分类版本、指标版本和协议分别计算、存储；每个协议组内独立应用 §6.5 的充足性和排序规则，不跨协议比较。
- 后续机会卡的渠道栏标明各协议组，并分别展示组内 `ranked` 和 `insufficient`。协议是同一指标的维度，不新建并行评论事实或业务计数。
- 本地运行库和生产环境迁移仍需负责人另行授权；隔离测试不视为运行环境迁移。

---

### D-043 需求栏按协议分组（答复 Q-23）

- 需求指标按首次入档协议分组展示，未知协议单独成组；每组包含自己的等级、当前周期、基线、趋势以及样本/基线标记，不跨组比较。沿用同一事实和指标链路。

### D-044 问题级办法暂不可用（答复 Q-24）

- 现有办法和经历没有可证明的问题级归属。机会卡明确返回暂不可用及原因，不把评论内共现当作问题归属，不新增模型调用或归属算法；原评论研究入口不变。

### D-045 未评估的供给计数为空（答复 Q-25）

- 供给尚未评估时 `counts = null`，状态为 `not_evaluated`；不得将未知计数显示为 0。

### D-046 作者姓名未知时为空（答复 Q-26）

- 作者姓名未知时 `name = null`，界面显示“姓名未知”；不以作者 ID 冒充姓名。

### D-047 点赞按时点共享读取（答复 Q-27）

- 先限制来源包、评论观察和点赞投影均截至 `as_of` 已存在，再沿用现有“最新已知点赞，否则未知”规则。评论研究和机会卡复用同一选择逻辑，并补跨模块时点回归；不重新采集。

### D-048 标准样本的当前周（答复 Q-28）

- `as_of = 2026-11-10T00:00:00Z` 时，当前周按上海日历为 11 月 9 日开始；标准样本中的 W0 仍表示 11 月 2 日开始的上一周。
- 保留 W0 原有数据和标准答案；卡片快照依真实当前周生成，不改变样本总体与时间。

## 待裁定

- 暂无。
