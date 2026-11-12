# QUESTIONS（阻塞问题）

> 状态: 草案
> 最后核对: 2026-11-12
> 适用范围: 机会卡实施中尚未裁定的阻塞问题（Q-11、Q-12 待裁定）。
> 事实来源: T0.1/T0.1b 勘察与负责人答复记录。
> 冲突时以谁为准: DECISIONS.md 的已裁定内容；未裁定问题不按假设实现。

当前裁定状态：Q-01 至 Q-10 已全部答复（2026-11-12），裁定内容以 `DECISIONS.md` 为准；Q-11、Q-12 为 T0.1b 新发现的阻塞，待负责人裁定。

记录规则：每项先写事实与证据，再写为什么阻塞、可选方案和需要负责人裁定的事项。未裁定前不按假设继续实现。

## Q-01 评论没有 `published_at`，无法按发布时间切周

- **事实**：`linggan_material_comment` 没有发布时间列（`database/migrations/0016_material_social_lanes.sql:26-56`）；全库搜索 `ALTER TABLE linggan_material_comment`、`material_comment.*published` 均无结果。`0026_work_resource_read.sql:8-15` 只给作品详情 `linggan_material_content_detail` 加了 `published_at`。
- **为什么阻塞**：T0.2 要求每条评论都有 `published_at`；§6.2.1 的周窗口 SQL 使用 `c.<<RECON:comment_published_col>> >= :p_start_utc`。现有 `observed_at` 是观察时间，不是平台发布时间；不能用它代替，否则“本期提及数”会把旧评论按采集时间错误计入。
- **可选方案**：
  - A. 在 T0.2 新增评论 `published_at timestamptz NULL`，只从生产者 package 中可证明的平台时间/精确原始文本回填；无法证明的保持 NULL，并在查询中显式排除/标记。推荐。
  - B. 从评论正文或作品详情推断发布时间（会引入不可靠推断，不建议）。
  - C. 暂不做周指标，先做全量只读机会卡（与手册阶段 1 冲突）。
- **需要裁定**：是否批准 A；新增列名、允许的时间来源与精确度、历史 NULL 的处理口径。
- **答复（负责人，2026-11-12）**：选方案 A 的变体：v1 以作品发布周为统计基准（`period_basis = work_published_cohort`），同时补充勘察评论时间来源。见 DECISIONS D-008。

## Q-02 没有通用特性开关机制

- **事实**：搜索 `FEATURE_|feature_flag|featureFlag|ENABLE_`（排除 `node_modules`/`*.lock`）无结果；现有最接近的是环境变量阶段开关 `LINGGAN_COLLECTION_UPGRADE_PHASE`（`env.example:19-21`）和 `LINGGAN_MODEL_SYNTHETIC_PREVIEW`（`crates/intelligence/src/model_secrets.rs:158`）。
- **为什么阻塞**：硬规则 R14 要求 `opportunity_card` 默认关闭，且手册 §4 要求沿用现有配置机制。现在没有可沿用的通用机制。
- **可选方案**：
  - A. 新增环境变量 `LINGGAN_OPPORTUNITY_CARD`，缺省/未知为关闭；读取集中在 API/worker 启动处。推荐，改动最小。
  - B. 新增数据库配置表（如 `linggan_feature_flag`），支持运行时开关与审计；工作量更大。
  - C. 复用 `LINGGAN_COLLECTION_UPGRADE_PHASE` 这类阶段变量（语义不匹配，不建议）。
- **需要裁定**：选 A 还是 B；若选 A，变量名与默认值；若选 B，是否允许新增表与迁移。
- **答复（负责人，2026-11-12）**：选 A：`LINGGAN_OPPORTUNITY_CARD`，仅取值 `enabled` 时开启。见 D-006。

## Q-03 没有 Rust 日期时间库，周期函数实现方式未定

- **事实**：`rg -n "chrono|time =" Cargo.toml apps/*/Cargo.toml crates/*/Cargo.toml` 无命中。现有时间以 PostgreSQL `timestamptz` 和 SQL `AT TIME ZONE 'Asia/Shanghai'` 为主（`database/migrations/0131_ai_gen1_provider.sql:192-193`）；Rust 侧常以字符串传时间（`crates/intelligence/src/note_analysis.rs:42`、`database/migrations/0016_material_social_lanes.sql:40`）。
- **为什么阻塞**：T0.3 要“给定 UTC 时间和参数返回 period_start；给定 period_start 返回 UTC 起止区间”，且禁止业务代码出现时区字符串/星期常量。没有日期库时，Rust 侧实现会变成手写时区换算，违反 §6.5。
- **可选方案**：
  - A. 新增 `time` crate（或 `chrono`），集中封装周期函数；需要按 §3.3 第 8 条批准新依赖。推荐 `time`，与现有轻量依赖风格一致。
  - B. 周期切分全部放在 PostgreSQL SQL 函数中，Rust 只调用；不新增 Rust 依赖，但需要新增 migration 和 SQL 函数，且测试要覆盖 DST/周边界。
  - C. 只用 `std::time` 手写换算（禁止，违反 §6.5）。
- **需要裁定**：选 A 还是 B；若 A，批准新增依赖；若 B，批准新增 SQL 函数 migration。
- **答复（负责人，2026-11-12）**：选 B：周期计算放在 PostgreSQL 函数中，不新增 Rust 日期库。见 D-020。

## Q-04 作品与领域是多对多，`n` 的分母口径未定

- **事实**：`linggan_material_content` 没有 `domain` 列（`database/migrations/0015_material_projection.sql:7-14`）。领域关系在 `linggan_material_domain_usage(content_public_ref, domain_ref, role, basis_kind)`（`0103_domain_membership_and_usage.sql:125-149`），后续增加 `use_kind`（`domain_research` / `creator_context`，`0180_creator_preview_discovery_only.sql:8-23`）。生产读取按 `use_kind='domain_research'` 过滤（`crates/evidence/src/material_social_read.rs:528`）。
- **为什么阻塞**：§6.2.1 的 `n` 是“本期领域内有效评论数”，必须先定义哪些作品属于该领域。一个作品可能同时是 primary/reference，也可能有 `creator_context` 用途。
- **可选方案**：
  - A. 分母只计 `use_kind='domain_research' AND role='primary'` 的作品。推荐，语义最接近“该领域的研究材料”。
  - B. 计入 `use_kind='domain_research'` 的 primary + reference；reference 会放大分母，需确认。
  - C. 由用户显式选择作品集合（研究集合），而不是按领域自动取；更灵活但与 §6.2.1 的默认口径不同。
- **需要裁定**：默认 `n` 使用哪种集合；`reference` 和 `creator_context` 是否进入机会卡的任何分母。
- **答复（负责人，2026-11-12）**：选 A：`use_kind = domain_research` 且 `role = primary`；k 与 n 使用同一作品集合。见 D-009。

## Q-05 作品作者是投影/函数，不是单列；未知作者如何处理

- **事实**：`linggan_material_content` 没有作者列。作者归属来自视图 `linggan_material_content_author`（`database/migrations/0063_content_author_attribution.sql:28-35`）或 `as_of` 一致函数 `linggan_material_content_author_at(content_public_ref, as_of)`（`database/migrations/0115_creator_discovery.sql:4`）。`author_external_id` 可为 NULL，归属来源有 `content_detail` / `profile_discovery` 两类（`0063:33-34`）。
- **为什么阻塞**：§6.2.1 需要排除作品作者本人的评论（`c.commenter_col IS DISTINCT FROM w.work_creator_col`）；作者问题集中度也需要创作者身份。若作者未知，直接比较会把所有评论当成非作者，污染指标。
- **可选方案**：
  - A. 用 `linggan_material_content_author_at(work, as_of)` 取作者；作者为 NULL 时把作品标记为 `workAuthorUnknown`，其评论不进入“排除作者”后的分子/分母，或单独列出。推荐。
  - B. 用当前视图 `linggan_material_content_author`（非 as_of）；会破坏 R12 一致性，不建议。
  - C. 只用详情自带的作者，不采用主页扫描归属；会漏掉部分作品。
- **需要裁定**：作者来源优先级、未知作者的排除口径、`creator_problem_density` 是否要求作者非空。
- **答复（负责人，2026-11-12）**：选 A：`linggan_material_content_author_at(..., as_of)`；作者未知的作品从 k 与 n 中同时排除并计数。见 D-010、D-012。

## Q-06 `comment_id` 用 `material_ref` 还是稳定复合身份

- **事实**：`linggan_material_comment.material_ref` 是每次观察的主键（`database/migrations/0016_material_social_lanes.sql:27`）；稳定评论身份是 `(content_public_ref, comment_external_id)`，当前投影按它去重（`database/migrations/0025_comment_current_projection.sql:6-24`）。
- **为什么阻塞**：T0.5/§6.1 的 `comment_mention.comment_id` 用于“提及人数”去重。若用 `material_ref`，同一评论的多次观察会被算成多个提及，直接违反 R5/R8 和 `mentioners` 语义。
- **可选方案**：
  - A. 新表存 `content_ref uuid` + `comment_id text`（`comment_external_id`），唯一键 `(content_ref, comment_id)`；`material_ref` 只作为可选来源观察引用。推荐。
  - B. 用 `comment_external_id` 单列；需确认跨作品是否可能重复（不同作品下同一评论 id 可能重复），因此建议加 `content_ref`。
  - C. 用 `material_ref` 并在查询时 `DISTINCT ON` 到稳定身份；实现复杂且易错，不建议。
- **需要裁定**：采用 A；确认 `comment_id` 的类型与唯一键。
- **答复（负责人，2026-11-12）**：选 A：`(content_ref, comment_external_id)`。见 D-011。

## Q-07 存在两套“用户问题”实体，`source_ref` 指向哪一套

- **事实**：
  - 生产读取使用 `linggan_comment_problem_definition` + `linggan_comment_problem_membership`（`database/migrations/0177_comment_problem_workflow.sql:39-55`；`crates/intelligence/src/ai_foundation/comment_problem.rs:38`；`apps/api/src/local_web/comment_study_command_api.rs:141-149`）。
  - 旧内核仍存在 `linggan_comment_research_problem`、`_problem_definition`、`_atom_problem_membership`（`database/migrations/0064_comment_research_kernel.sql:205-240`）。
- **为什么阻塞**：T0.5 要求 `problem.source_ref` 指回评论研究中的原实体；两套实体都有 `problem_ref`/`definition_ref`，选错会导致问题谱系接错或统计重复。手册 §3.3 第 3 条要求遇到“已有用户问题结构不明”时停止询问。
- **可选方案**：
  - A. 以 0177 为唯一活动实体：`source_ref` 存 `linggan_comment_problem_definition.problem_ref`（稳定问题 id），需要具体版本时另存 `definition_ref`/`review_invocation_ref`。推荐。
  - B. 以 `definition_ref` 作为 `source_ref`（每次定义修订一个新引用）；谱系需要额外去重到 `problem_ref`。
  - C. 兼容两套（增加 source_kind）；复杂且容易双计，不建议。
- **需要裁定**：确认 0177 为唯一活动来源；确认 `source_ref` 是 `problem_ref` 还是 `definition_ref`；确认 0064 旧表是否仅保留为历史、不再新增。
- **答复（负责人，2026-11-12）**：选 A：0177 为唯一活动来源，`source_ref = problem_ref`；开工前确认 0064 无写入、0177 是否有合并语义。见 D-014。

## Q-08 评论者 HMAC 的密钥来源、版本与未知身份处理

- **事实**：已有手写 HMAC-SHA256（`crates/evidence/src/collection_control.rs:1432-1454`），digest version `hmac-sha256-v1`（`database/migrations/0034_collection_control_closure.sql:64`）。现有用法是平台观察账号 identity digest，密钥来自 `LocalWebState.account_digest_key: Option<Arc<Vec<u8>>>`（`apps/api/src/local_web.rs:186`）；公开访问为 `None`（`apps/api/src/local_web/public_access.rs:269`），缺失时 `MissingDigestKey`（`crates/evidence/src/collection_control.rs:579-581`）。
- **为什么阻塞**：T0.6 要求评论者身份只存 HMAC 哈希（R9）。现有 `author_external_id` 是可空的平台标识；需要一个明确的评论者密钥、版本和未知身份口径，否则无法保证不可逆、可轮换、不可跨环境关联。
- **可选方案**：
  - A. 新增独立 `commenter_digest_key`（至少 32 字节），复用 `hmac_sha256_hex`，digest version 命名如 `hmac-sha256-v1`；未知作者保持 NULL，不哈希占位串。推荐。
  - B. 复用 `account_digest_key`；跨用途关联风险更高，不建议。
  - C. 不存哈希，只存 `author_external_id` 的派生不可逆 id；需要额外设计。
- **需要裁定**：密钥来源与配置项名、轮换策略、未知作者是 NULL 还是显式 `unknown`（建议 NULL + 状态位）、是否需要按领域加盐。
- **答复（负责人，2026-11-12）**：选 A：独立密钥文件，复用现有 HMAC 实现；评论者未知时哈希为 NULL，不计入 k。见 D-013。

## Q-09 `observed_at` 命名冲突：现有“观察时间” vs 手册“首次采集时间”

- **事实**：现有 `linggan_material_comment.observed_at` 是该次观察的时间文本（`database/migrations/0016_material_social_lanes.sql:40`），当前投影取最新观察（`database/migrations/0025_comment_current_projection.sql:24`）。首次采集可由 `linggan_material_comment_first_admission` + `linggan_runtime_capture_package.accepted_at` 推导（`database/migrations/0125_collection_single_run.sql:136-153`；`0004_plugin_runtime_all_capabilities.sql:42`），但没有独立列。
- **为什么阻塞**：手册术语表把 `observed_at` 定义为“首次采集时间”，T0.2 要求“确保每条评论都有 `observed_at`（首次采集时间）”。如果直接把现有列改语义，会破坏历史读取和 append-only 观察链；如果新增同名列也不可能。必须决定命名映射。
- **可选方案**：
  - A. 保留现有 `observed_at` 语义不变，新增不可变 `first_observed_at timestamptz NULL`（或 `first_collected_at`），从 first_admission 回填；手册中的 `observed_at` 在机会卡域映射到新列。推荐。
  - B. 新增 `observed_at_first`，并在机会卡 SQL 中显式使用；命名不同但语义一致。
  - C. 重新定义现有 `observed_at` 为首次采集、另加 `last_observed_at`；会改变既有合同，不建议。
- **需要裁定**：选 A/B 的列名；回填来源；回填标记（如 `observed_at_backfilled`）是否也需要。
- **答复（负责人，2026-11-12）**：选 A：机会卡领域统一使用 `first_observed_at`，由视图推导，不修改现有表。见 D-007。

## Q-10 向量来源是否启用：有存储，没有确认的检索调用

- **事实**：pgvector 与 512 维 WeMM profile 存在（`database/migrations/0075_local_embedding_001.sql:7`、`:36-46`）；评论 Atom embedding 表存在（`database/migrations/0064_comment_research_kernel.sql:186-201`）。但 `rg -n "atom_embedding|<=>|cosine" crates apps --glob '!*test*'` 没有找到当前 Rust/SQL 的相似度 Top-K 查询；只有 node_modules 和 Python 注释。现有 embedding 仅覆盖评论 Atom，不覆盖作品理解摘要。
- **为什么阻塞**：T2.2 的“向量来源：问题表述与作品理解摘要的相似度前 20 篇”只有在“已有嵌入与检索能力”确认时才能启用；现在只能确认存储，不能确认检索链路，且作品理解摘要没有 embedding。
- **可选方案**：
  - A. 按手册关闭向量来源，在 `DECISIONS.md` 记录原因，T2.2 只用其他来源。推荐（在检索链路未验证前）。
  - B. 新增作品理解摘要 embedding + 检索任务；需要新的 AI task、预算、许可、embedding 写入和 Top-K 查询，工作量显著。
  - C. 只用评论 Atom embedding 做问题相似度；不满足“作品理解摘要”要求，不建议冒充。
- **需要裁定**：T2.2 是否暂时关闭向量来源；若启用 B，是否批准新增 embedding 任务与外发许可。
- **答复（负责人，2026-11-12）**：选 A：关闭向量来源。见 D-015。

## Q-11 评论平台时间覆盖率仅 0.14%，达不到 D-008 v2 的 90% 门槛

- **事实**：
  - 原始采集包确实带评论时间字段：`linggan_runtime_capture_package.payload.records[].payload.publishedAt`（毫秒 epoch）与 `.publishedAtText`；插件解析见 `plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:93-114`、展示文本见 `:458-492`。
  - 规范化在缺少真实时间时回退到采集时间：`plugins/linggan-intelligence-browser/src/db/recordNormalization.js:162`（`record.publishedAt || record.createdAt`），因此 `0` 不能视为真实发布时间。
  - 只读聚合（comments + replies 包，共 31809 条记录）：`publishedAt` 为 13 位 epoch 的仅 **45 条（0.14%）**，其余 31764 条为 `0`；`publishedAtText` 100% 有值，但 76.8% 是带 `-` 的展示日期（最多日精度），16.6% 是“N 天前”相对文本，6.6% 带时间冒号。
  - 评论研究 API 恒为 null：`crates/intelligence/src/comment_study_catalog/comments.sql:183` 写死 `'publishedAt', NULL`。
  - 合同 fixture 没有评论时间：`crates/contracts/tests/producer_execution_validation.rs:69-73` 的 comments fixture 是 `"records":[]`；唯一 `publishedAt` fixture 是作品级 `publishedAtSourceText`（`crates/contracts/tests/discovery_boundary_contract.rs:31`）。
- **为什么阻塞**：D-008 规定 v2 只有在评论发布时间覆盖率 ≥90% 时才启用，并把 `period_basis` 切到 `comment_published`。按现有数据和链路，精确平台时间只有 0.14%，即使解析展示文本也最多得到日精度且仍含相对文本，无法满足“不混用两种基准”的要求。
- **可选方案**：
  - A. v1 保持 `work_published_cohort`，不建 `linggan_opportunity_comment_published`，把覆盖率写入已知局限。推荐：不改变已裁定的 v1 口径。
  - B. 先修采集侧：让插件把平台返回的 `create_time` 直接写成毫秒 epoch，并新增只追加表保存；历史数据仍只有 0.14%，v2 只能覆盖修复后的新采集。
  - C. 把带 `-` 的展示日期按日精度解析为“可用时间”，并接受相对文本不可用；但覆盖率仍达不到 90%，且精度与 v2 的周窗口不完全匹配。
- **需要裁定**：v2 是否在 v1 范围内关闭；若保留 v2，是否批准采集侧修复与新增只追加表；是否允许把日精度展示文本计入“可用”。
- **答复（负责人，2026-11-12）**：待裁定。

## Q-12 D-013 的 `LINGGAN_SUPPORT_DIR` 缺省路径与代码不一致

- **事实**：
  - 代码的实际缺省是 `$HOME/Library/Application Support/Linggan Intelligence`：`crates/intelligence/src/model_secrets.rs:39-43`；`apps/worker/src/bin/media_worker_support/mod.rs:104-110`；`apps/worker/src/bin/media_worker_support/paddle_ocr.py:19-23`；`docs/runbooks/paddle-ocr-runtime.md:15` 也写“默认 Application Support”。
  - D-013 写的是 `LINGGAN_SUPPORT_DIR` 缺省为 `$HOME/.linggan-support`，并称“与 `model_secrets.rs` 的约定一致”。
  - `model_secrets.rs` 没有可复用的通用密钥读取函数：`from_runtime()` 是私有函数（`:33`），读取固定布局 `model-secrets/<workspace>/<reference>`。
- **为什么阻塞**：D-013 要求密钥放在 `${LINGGAN_SUPPORT_DIR}/opportunity/commenter-digest.key`，缺失时机会卡写入/计算任务拒绝启动。若按 D-013 的缺省把密钥放到 `$HOME/.linggan-support`，而运行时代码按 Application Support 解析，任务会找不到密钥并拒绝启动；反之亦然。两条路径不能同时成立。
- **可选方案**：
  - A. 以代码实际缺省为准：`$HOME/Library/Application Support/Linggan Intelligence/opportunity/commenter-digest.key`，并更正 D-013 的路径说明。推荐：与现有 worker/OCR/model-secrets 一致。
  - B. 机会卡单独使用 `$HOME/.linggan-support` 作为缺省，不改其他模块；会出现两套 support 目录，运维容易放错。
  - C. 要求机会卡显式设置 `LINGGAN_SUPPORT_DIR`，不提供缺省；最安全但增加启动配置负担。
- **需要裁定**：选 A/B/C；确认密钥文件的最终路径，以及是否允许把 `hmac_sha256_hex` 改为 `pub`（D-013 已允许，但需在 PR 单独说明）。
- **答复（负责人，2026-11-12）**：待裁定。
