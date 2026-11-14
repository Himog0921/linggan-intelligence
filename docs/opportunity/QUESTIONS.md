# QUESTIONS（阻塞问题）

> 状态: 草案
> 最后核对: 2026-11-13
> 适用范围: 机会卡实施中的阻塞问题记录；Q-01 至 Q-28 已答复。
> 事实来源: T0.1/T0.1b/C1 勘察与负责人答复记录。
> 冲突时以谁为准: DECISIONS.md 的已裁定内容；未裁定问题不按假设实现。

当前裁定状态：Q-01 至 Q-28 已答复。裁定内容以 `DECISIONS.md` 为准。

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
- **答复（负责人，2026-11-12）**：D-021 先裁定 v1 保持 `work_published_cohort`、不采用解析展示文本（方案 C），并把方案 B 移交采集侧；D-028 进一步把 `comment_published` 定为目标口径，交由阶段 C（C1–C4）补齐采集与历史回填。机会卡不得读取评论的 `publishedAt`。见 DECISIONS D-021、D-028。

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
- **答复（负责人，2026-11-12）**：选 A，并收敛为一处解析：把 `model_secrets.rs:39-43` 的 support 目录解析与私有文件权限校验提取为 `pub` 函数，由 `model_secrets` 与机会卡共同调用；不得再写第二份缺省值。见 D-022。

## Q-13 机会卡统计使用哪种评论排序协议

- **事实**：
  - 小红书顶层/子评论请求都没有排序参数（`plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:356-390`）；记录里的 `sortMode` 固定写 `'unknown'`（`plugins/linggan-intelligence-browser/src/platforms/xhs/commentCollector.js:708`、`:1014`、`:1055`）。
  - 能力登记只证明“当前默认顺序”可见（`docs/platforms/xiaohongshu/capture-capability-registry.md:69`），并明确“默认排序”尚未验证（`:116`）；仓库内没有平台支持时间排序的证据。
  - 标准详情窗口固定最多 30 条（`plugins/linggan-intelligence-browser/src/linggan/localExecutionSupport.js:23`、`:177-181`），因此默认顺序下的 30 条不保证是“最新 30 条”；重复采集一次最多新增 30 条（`crates/intelligence/src/topic_map_research_collection.rs:226-231`）。
  - D-030 要求采集协议版本化，并建议按时间排序，但是否可行要等 C1 结论；C1 无法验证平台是否支持。
- **为什么阻塞**：不先确定协议，C2 无法决定请求是否要带排序参数、`collection_protocol` 记录哪些字段，也无法判断重复采集能否稳定补到新评论；D-030 的“按协议版本分组指标”同样缺少取值。
- **可选方案**：
  - A. 固定使用平台默认顺序；`collection_protocol` 记录 `sort_mode = "platform_default"`，并以多次采集补新评论。推荐：不需要平台未验证的能力。
  - B. 改用时间倒序（若平台支持）；需要在 C2 前先做一次只读/人工验证，确认请求参数与页面行为。
  - C. 两种都支持，按 `collection_protocol.sort_mode` 分组指标；实现与验证成本最高。
- **需要裁定**：选 A/B/C；若选 B 或 C，确认是否允许为验证排序协议访问平台（C1 本身禁止访问平台）。
- **答复（负责人，2026-11-12）**：选 A。固定平台默认顺序，`collection_protocol.sort_mode = "platform_default"`；不访问平台验证时间排序，不实现双协议。采集协议字段与版本标识按 D-030 在 C2 落地，旧包继续接收。见 D-032。

## Q-14 D-024 分布报告触发停止条件后是否改用更长统计周期

- **事实**（2026-11-12 真实库只读查询，0203/0204 未应用，资格逻辑以只读 CTE 复刻）：
  - 按“最近 12 个自然周”计算时，窗口内没有领域-周：数据中最新可作周期锚点的作品发布时间为 2026-10-21，而当前周回溯 12 周的起点为 2026-10-29。
  - 改用数据中最近 12 个有数据的周期（2026-07-27 至 2026-10-12）：领域-周 33 个，周 n 的 P25 = 27、P50 = 57、P75 = 211，最小 4、最大 1390。
  - 有数据的领域 3 个：ADHD 周中位 530.5（12 周）、考研自习 29.5（10 周）、自闭症干预 39.0（11 周）；第 4 个领域无有效数据。
  - 按 D-029 的 `collection_target_per_period = 700`，只有 ADHD 接近目标，另两个有数据领域中位不足 100。
- **为什么阻塞**：D-024 第 4 款规定“多数领域的周 n 中位数低于 100”时停止并报告，可能需要改成长统计周期。当前 3 个有数据领域中 2 个低于 100，已达到停止条件；继续 T1 及之后的任务会把统计口径建立在过短或过稀的周期上。
- **可选方案**：
  - A. 统计周期从 1 周改为 4 周（月窗口），其余口径不变；需要同步修改 `period` 参数、周期函数与所有卡片周期字段。推荐：能直接缓解单领域周样本不足。
  - B. 保持 1 周，但只对有数据的领域计算，并把“样本不足”如实显示；不改变口径，等待采集量提升。风险是多数卡片长期显示样本不足。
  - C. 暂停机会卡实现，先完成阶段 C 采集扩容，再重新出分布报告；最保守。
- **需要裁定**：选 A/B/C；若选 A，确认新周期长度，以及是否仍以 `work_published_cohort` 为 v1 口径。
- **答复（负责人，2026-11-12）**：不按当前库的周期分布裁定。前端采集已手动暂停一个多月，数据库反映的是暂停状态，不代表项目设计或采集能力。保持 `period = week` 与 `period_basis = work_published_cohort`；D-024 分布报告保留为基线观察，采集恢复后重新运行，届时再判断周期口径。实现继续按手册推进。见 D-033。

## Q-15 派发会话拿不到评论平台时间，C4 与 `comment_published` 口径如何推进

- **事实**（2026-11-12 C4 第 1 轮真实采集与只读查询）：
  - 三个 `deep_archive` 批次共 10 个作品、100 条评论记录、95 条回复记录。考研与自闭症两批在扩展重载后执行，评论包已带 `collectionProtocol.version = 1`、`sortMode = platform_default`，但两批共 60 条评论记录与 56 条回复记录全部为 `publishedAt = null`、`sourceTier = dom`、`qualityReason = api_unobserved_dom_fallback`；ADHD 批在重载前执行，无协议字段且 `publishedAt` 写 `0`。
  - 全库历史 31928 条评论/回复记录中，只有 2026-08-31 的 4 个包（用户前台手动批量采集）有 API 记录与毫秒平台时间；其余所有日期、包括全部派发详情会话，API 记录数为 0。
  - 插件 API 优先路径在页面中直接 `fetch('/api/sns/web/v2/comment/page', { credentials: 'include' })`，不带签名头（`plugins/linggan-intelligence-browser/src/injected/xhsApiCapture.js:368`、`src/platforms/xhs/commentApi.js:803-824`）；实测未签名请求被平台网关拒绝（HTTP 500 `create invoker failed, service: jarvis-gateway-default`）。
  - 派发会话用 `chrome.windows.create({ focused: false })` 打开详情页（`src/linggan/background.js:1466`）；能提供签名的页面自身分页流量在现有记录中没有被捕获为评论快照。
  - 覆盖率报告：ADHD 7118 条、考研自习 159 条、自闭症干预 282 条有效评论，有平台时间均为 0；`project-platform-times` 写入 0 条。
- **为什么阻塞**：D-028 把 `comment_published` 定为目标口径，C3 只投影 `collectionProtocol.version = 1` 包中的有效 `publishedAt`。现在派发会话无法产生任何平台时间，C4 无法补齐历史，影子指标即使接入也恒为 0；这不是频率、冷却或授权问题，不能靠重试解决。
- **可选方案**：
  - A. 先做一次前台诊断：人工在前台打开一篇笔记，确认页面自身的评论分页请求是否被注入脚本捕获，再决定修复方式。成本最低，不修改代码。推荐。
  - B. 授权修复插件采集路径（在独立分支实现并评审），让评论 API 请求带签名，或确保页面自身分页流量被捕获；修完后重跑 C4。
  - C. 暂缓 `comment_published` 口径，维持 `work_published_cohort`，C4 只保留作品级覆盖率报告，等插件修复后再启用目标口径。
- **需要裁定**：选 A/B/C；若选 B，需明确允许在插件内新增签名请求实现，并确认不触碰频率限制、风险冷却与并发上限。修复前不得把 `comment_published` 影子指标标为可用。
- **答复（负责人，2026-11-12）**：选 A 的前半段事实成立，但不再继续追 API 修复；约 10 轮探针后确认页面自身分页流量无法作为评论快照捕获。改为采用已经稳定拿到的**日精度展示时间**：`publishedAtText` 覆盖全部评论/回复，按 D-034 以 `precision = 'day'` 入库；相对时间按该条记录自己的采集时刻推算；epoch 仍优先。新采集与历史回填同时进行。C4 的真实采集不再以拿到毫秒 epoch 为前置条件。见 D-034。

## Q-16 T1.1 逐问题人数对账的现有对照不存在

- **状态**：已答复；T1.1 可以继续，T1.2 仍依赖 T1.1。
- **事实**：手册 §7 T1.1 要求迁移后每个问题的去重提及人数与“评论研究中的人数”一致，并逐条解释差异。但现有 `GET /api/local/comment-research/problems` 由 `apps/api/src/local_web/comment_study_command_api.rs:141-149` 调用 `crates/intelligence/src/ai_foundation/comment_problem/read.rs:4-40`，返回问题、最多 100 条来源合格的依据及候选，不返回逐问题的去重作者人数；`apps/web/src/pages/corpus/comments/ResearchDashboard.tsx:12,22` 同样不读取或显示此数字。
- **额外口径差异**：当前发布资格由 `crates/intelligence/src/ai_foundation/comment_problem.rs:38-68` 按创建定义时同一次核验的独立作者及当前来源资格判定；机会卡 §6.2 则对全部归属再应用领域 primary、作品作者、时间锚点、来源限制与作者自评等资格。两者不能默认等同，也不能把最多 100 条依据列表当作全部人数。
- **为什么阻塞**：现有产品没有可直接比较的逐问题人数；自行把另一条临时 SQL 或有限证据列表称为“评论研究中的人数”，会改变 T1.1 的验收定义。
- **方案 A（推荐）**：在测试和迁移核对中，从 0177 的完整归属、稳定评论身份和评论者身份独立计算源侧去重人数；与迁移表按同一原始集合对账，逐项列明缺失和冲突。再单独报告机会卡资格排除后的人数、低于 2 的已发布问题及原因。对账查询只用于验证，不新增另一套持久业务计数。
- **方案 B**：先扩展评论研究接口，提供经过既有来源资格判断的逐问题人数，再与机会卡对账。需明确两边不同的资格口径，并扩大接口及其验证范围。
- **请求负责人决定**：是否采用 A，或选择 B / 指定其他口径。不得擅自修改测试期望或把不同集合的人数写成相等。
- **答复（负责人，2026-11-13）**：选择 A。源侧从现有 0177 归属和评论身份独立计算去重人数，与机会卡迁移结果对账；另列机会卡资格排除及发布门槛差异，不新增第二套业务计数。见 D-036。

## Q-17 评论平台时间的时点查询会使用未来记录

- **事实**：`database/migrations/0208_comment_platform_time_day_precision.sql:337-435` 的资格函数直接关联 `linggan_material_comment_platform_time_current`（`:411-413`），只限制评论的 `first_observed_at <= as_of`，没有按 `as_of` 限制平台时间行的 `created_at` 或来源包的 `accepted_at`。当前视图（`:31-47`）优先选择毫秒行，不接受时点参数。
- **为什么阻塞**：若评论先有日精度时间，之后才采到毫秒时间，回看这两次采集之间的时点时，当前实现可能选用未来才存在的毫秒记录，与 R12 不一致；只有未来记录时也可能把未知写成已知。
- **方案 A（推荐）**：建立唯一的按时点取时间逻辑：先限制来源包接纳时间与投影行创建时间均不晚于 `as_of`，再按 D-034 选择已有的最高精度；无合格时间记为 `comment_time_unknown`。当前视图和资格函数共用该逻辑，使用前向迁移，不改已应用迁移。
- **方案 B**：暂缓评论发布时间影子指标，仅继续作品发布周口径，时点修复另行处理。
- **答复（负责人，2026-11-13）**：选择 A。先做前向修复，再继续 T1.2。见 D-037。

## Q-18 指标按采集协议分组时如何归属重复评论

- **状态**：已答复；T1.2 按 D-038 继续。
- **事实**：D-030 要求按采集协议版本分组并在协议变化后重新积累基线，D-032 固定使用平台默认顺序。`linggan_opportunity_comment_fact` 按稳定评论身份去重，但不返回协议；同一评论可以被多个不同采集包观察。首次入档包保存在 `linggan_material_comment_first_admission.first_package_ref`（`0125_collection_single_run.sql:136-145`），采集协议在包的 `collectionProtocol` 中，旧包可能没有此字段。
- **规格缺口**：现有手册没有规定多次观察的评论使用哪个包的协议；§6.1 的 `problem_metric_period` 主键不含协议，不能为同一问题和周期保存多个协议分组。
- **方案 A（推荐）**：每条稳定评论使用首次入档包的协议；无法取得协议时保留未知。指标按协议分组存储，同一基线不混合协议，基线不足明确标记。协议分组是同一指标的维度，不复制评论事实或建立第二套业务计数。
- **方案 B**：只计算协议明确的评论，协议未知的评论排除并单独计数；已知协议仍分组存储，基线不混合。
- **请求负责人决定**：采用 A、B 或指定其他规则；不得将未知协议默认为已知协议，也不得把不同协议的样本混合后当作可比较的趋势。
- **答复（负责人，2026-11-13）**：选择 A。每条稳定评论归属首次入档包的协议；协议缺失保持未知。指标按协议分别存储和计算，同一基线不混合协议，基线不足明确标记。见 D-038。

## Q-19 评论事实和评论者身份的时点选择

- **状态**：已答复；T1.2 按 D-039 继续。
- **事实**：`0204_opportunity_comment_eligibility.sql:12-69` 的 `linggan_opportunity_comment_fact` 先在全部观察中选择评论者并取最早观察时间；`0208_comment_platform_time_day_precision.sql:337-435` 再按 `first_observed_at <= as_of` 过滤。后续才入库、但观察时间较早的记录可能改变历史评论身份或首次时间。T1.1 的派生表保留当前哈希和最后变更时间，不保存每次哈希的历史值。
- **为什么阻塞**：按现状回看历史可能使用未来才入库的观察或当前哈希，与 R12 不一致；按现有数据也无法直接从派生表还原任意历史哈希。`linggan_material_comment_first_admission` 没有建立时间，不能声称知道该行当时何时建立。
- **方案 A（推荐）**：建立唯一的按时点事实选择逻辑：先筛选来源包 `accepted_at` 和评论观察 `created_at` 不晚于 `as_of` 的观察，再按 D-007 优先首次入档包的合格观察，否则选合格观察中最早的一条。计算历史指标时对该时点选出的身份使用同一密钥和哈希实现，不另建第二套持久身份；当前视图及当前同步也复用该选择逻辑。首次入档行无建立时间的限制须在回测中说明，不伪造该时间。
- **方案 B**：维持当前事实视图和派生哈希；凡无法证明历史身份与当前相同的历史计算返回明确不可用，实时计算继续。实现范围较小，但历史回测覆盖受限。
- **请求负责人决定**：采用 A、B 或指定其他规则。此问题不改变 Q-09、Q-17 或 Q-18 已确认的业务含义。
- **答复（负责人，2026-11-13）**：选择 A。共享按时点事实选择逻辑，计算当时身份的哈希；明确记录首次入档表没有建立时间的限制。见 D-039。

## Q-20 作品作者和作品发布时间的时点选择

- **状态**：已答复；按 D-040 修正共享函数和作品发布时间锚点。
- **事实**：`0115_creator_discovery.sql:4-31` 的共享 `linggan_material_content_author_at` 只限制来源包的 `accepted_at`，未限制详情或发现观察的 `created_at`；`0208_comment_platform_time_day_precision.sql:370-385` 的作品发布时间锚点同样未限制详情观察 `created_at`。后接纳投影的观察可能改变更早时点的作者、自评排除和周期。
- **影响范围**：作者函数还被材料查询、单篇分析及采集运行代码复用；直接修正共享函数会同时修正这些调用的历史时点结果。维护第二份机会卡专属作者归属逻辑则会形成并行口径。
- **方案 A（推荐）**：前向修正现有共享 `linggan_material_content_author_at`，保留既有来源优先级与排序，增加观察 `created_at <= as_of`；机会卡的作品发布时间锚点同样限制观察创建时间。补充共享调用方与机会卡隔离回归，当前功能继续走同一函数。
- **方案 B**：不更改共享函数；对无法证明作者观察在历史时点存在的机会卡查询返回明确不可用。历史覆盖受限，当前共享函数行为不变。
- **请求负责人决定**：采用 A、B 或指定其他规则。此问题涉及现有共享函数，不重开 Q-05。
- **答复（负责人，2026-11-13）**：选择 A。前向修正现有共享作者函数及作品发布时间时点，保留来源优先级，补充跨模块回归。见 D-040。

## Q-21 最新作品详情没有发布时间时是否回退旧值

- **状态**：已答复；T1.2 按 D-041 修复锚点选择。
- **事实**：现有 `0208_comment_platform_time_day_precision.sql` 先筛选 `published_at IS NOT NULL`，再选择最近详情；因此较新的详情未记录发布时间时会采用更早的非空时间。手册 §6.2.1 则写“截至 `as_of` 的最近一次详情观察中，`published_at` 非空”，按字面会把该评论标记为 `period_anchor_missing`。
- **影响**：两种规则会改变分母、分子、周期与排除数；D-040 只裁定观察创建时间，不足以裁定空值回退。
- **方案 A（推荐）**：先选截至 `as_of` 最新的合格详情，再检查其 `published_at`；为空则明确记为 `period_anchor_missing`，不回退旧观察。符合手册当前文字与缺失显式原则。
- **方案 B**：保留既有代码，仅在有非空发布时间的详情中选择最新值；修订手册明确允许回退。历史已知时间可继续使用，但最新观察缺失不会单独表现为缺失。
- **请求负责人决定**：采用 A、B 或指定其他规则。
- **答复（负责人，2026-11-13）**：选择 A。先选截至 `as_of` 最新的合格详情，发布时间为空时标记 `period_anchor_missing`。见 D-041。

## Q-22 作者集中度的采集协议分组与排序

- **状态**：已答复；T1.3 按 D-042 继续。
- **事实**：D-030 要求指标按采集协议版本分组。Q-18 / D-038 已裁定稳定评论按首次入档包的协议归属，未知协议保持未知；T1.2 的问题周期指标已按协议分组。手册 §6.1 的 `creator_problem_density` 主键没有协议维度，§6.5 的作者排序及 §6.7 的 `channel.ranked` / `channel.insufficient` 也没有说明不同协议如何分组和展示。
- **为什么阻塞**：如果把不同协议的评论汇总到同一作者结果，采集方式不同的样本会混算；如果按协议存储并展示，又需要明确作者指标的表结构和卡片输出。Q-18 当时针对 T1.2，尚未明确裁定 T1.3 的排序和后续卡片如何呈现。
- **方案 A（推荐）**：作者集中度沿用首次入档包的协议归属，按协议分别计算、存储和组内排序；未知协议单独成组。后续卡片标明协议分组，不跨组比较。协议是同一指标的维度，不新建另一套评论事实或业务计数。
- **方案 B**：作者集中度汇总所有协议，再按现有单一作者列表排序；需要明确这是 D-030 在作者指标上的例外，并接受采集方式差异可能影响可比性。
- **请求负责人决定**：采用 A、B 或指定其他规则。此问题不重开 Q-18 中已确定的首次入档协议归属与未知协议处理。
- **答复（负责人，2026-11-13）**：选择 A。按首次入档协议分组计算、存储和组内排序，未知协议单列；后续卡片标明协议且不跨组比较。见 D-042。本地运行库与生产环境迁移仍需另行授权。

## Q-23 问题需求指标如何在卡片中呈现采集协议分组

- **状态**：已答复；按 D-043 实施。
- **事实**：D-030、D-038 要求问题周期指标按首次入档协议分组，未知协议单列，同一基线不能混合协议。`database/migrations/0213_opportunity_problem_metric_period.sql:9-28` 已按协议持久化；手册 §6.7 的 `demand` 却只包含一组等级、当前值、基线与趋势，没有协议字段或选组规则。
- **为什么阻塞**：按单组结构不能同时表达多个协议；合并协议会违反 D-038，任意选择一组会隐藏其他数据。修改响应结构涉及手册 §6 规格。
- **方案 A（推荐）**：需求指标按协议分组，每组标明协议、等级、当前值、基线、趋势及该组样本和基线状态；未知单列，不跨组比较，其他通用内容保留。
- **方案 B**：只展示指定协议，并明确提示其他协议组未展示；需另定如何指定协议。
- **请求负责人决定**：采用 A、B 或指定其他规则；不重开首次入档协议归属。
- **答复（负责人，2026-11-13）**：采用 A。需求指标按协议分组，未知单列，同一组内比较；继续使用同一数据链路。

## Q-24 问题级“办法与经历”的数据来源

- **状态**：已答复；按 D-044 实施。
- **事实**：手册 §6.7 要求问题卡的 `user_methods` 提供办法标签和提及人数。现有 `solution` / `experience` 是评论级信号；问题归属只选择 `problem` / `need` 信号（`crates/intelligence/src/ai_foundation/comment_problem.rs:22-31`），非问题信号没有问题框架（`crates/intelligence/src/comment_study_semantic.rs:51-62`）。同一评论可能包含多个问题，当前没有可证明的办法到问题归属。
- **为什么阻塞**：按同一评论共现自动认定办法解决某个问题会引入未经验证的语义；直接返回空数组又无法区分未建立归属与确无办法。
- **方案 A（推荐）**：标记问题级办法暂不可用并保留原因，不猜测归属；不新增模型调用或第二套归属算法。
- **方案 B**：展示同条评论中的办法，但明确标为共现线索，不声称已归属于或能解决该问题。
- **请求负责人决定**：采用 A、B 或指定其他规则。
- **答复（负责人，2026-11-13）**：采用 A。问题级办法暂无法判断并标明原因，不自行推断归属；原有评论研究入口不变。

## Q-25 阶段 1 未评估供给的计数

- **状态**：已答复；按 D-045 实施。
- **事实**：T1.4 要求供给状态固定 `not_evaluated` 且不得填入假数据；手册 §6.7 的 `supply.counts` 却要求四个数字。阶段 1 尚未完成供给搜索和判定。
- **为什么阻塞**：把未评估计数写为 0 会混淆未知与已统计为零；改为 `null` 会改变 §6.7 字段类型。
- **方案 A（推荐）**：未评估时计数为 `null`，保留 `not_evaluated` 状态；已评估后的计数按真实结果填写。
- **方案 B**：计数写 0，同时提示尚未评估；接受数值本身不能区分未知和零。
- **请求负责人决定**：采用 A、B 或指定其他规则。
- **答复（负责人，2026-11-13）**：采用 A。尚未评估时显示未评估，计数为 `null`，不显示为零。

## Q-26 作者姓名未知时的渠道字段

- **状态**：已答复；按 D-046 实施。
- **事实**：手册 §6.7 的 `CreatorDensity.name` 是必填字符串；`database/migrations/0016_material_social_lanes.sql:63-82` 允许作者资料姓名状态为 `UNKNOWN`，且作者归属不保证存在资料记录。
- **为什么阻塞**：缺失姓名不能伪造；返回 `null` 需要修改 §6.7 的类型。
- **方案 A（推荐）**：未知姓名返回 `null`，界面显示“姓名未知”；不以作者 ID 冒充姓名。
- **方案 B**：用作者 ID 作为显示名称，并明确告诉用户该值是 ID。
- **请求负责人决定**：采用 A、B 或指定其他规则。
- **答复（负责人，2026-11-13）**：采用 A。未知姓名为 `null`，界面显示“姓名未知”。

## Q-27 代表原声的点赞数如何按时点读取

- **状态**：已答复；T1.4 按 D-047 继续。
- **事实**：代表原声须按点赞排序（§6.7、D-019），R12 要求只使用截至 `as_of` 已存在的事实。`linggan_material_comment_engagement` 有自己的 `created_at`，迁移回填也可晚于原评论观察（`database/migrations/0122_comment_like_library.sql:4-29`）。现有评论研究的 `like_heads` 只限制来源包和评论观察时间，没有限制点赞投影创建时间（`crates/intelligence/src/comment_study_catalog/comments.sql:16-36`）。因此后来才入库的点赞数可能影响历史排序。
- **为什么阻塞**：直接复用现有逻辑违反 R12；在机会卡另写一套点赞选择逻辑会形成两套不同口径。修正共享读取会影响评论研究的历史回看，需要负责人确认。
- **方案 A（推荐）**：修正并抽取现有点赞选择逻辑，先限制来源包、评论观察和点赞投影均截至 `as_of` 已存在，再沿用现有“最新已知点赞，否则未知”规则；评论研究与机会卡共用，补跨模块时点回归。
- **方案 B**：保留现有评论研究逻辑，暂停机会卡代表原声的点赞排序，待后续明确统一规则后再实现；需修订 T1.4 验收口径。
- **请求负责人决定**：采用 A、B 或指定其他规则。已确认的作品作者时点规则 D-040 不重开。
- **答复（负责人，2026-11-13）**：采用 A。修复共享规则，回看时只使用当时已入库的点赞数；评论研究与机会卡共用，并补跨模块测试，不重新采集。

## Q-28 标准样本的“本周”与统计时间矛盾

- **状态**：已答复；T1.4 按 D-048 继续。
- **事实**：手册 §8.2.1 把 `W0 = 2026-11-02` 称为当前周期，却同时规定 `as_of = 2026-11-10T00:00:00Z`。该时间在上海为 11 月 10 日 08:00，按周一开周的 §6.4.3，当前周期从 11 月 9 日开始。现有 `problem_metrics.rs:153-168` 也从 `as_of` 所在周生成周期；T1.2 的标准答案测试是明确查询 11 月 2 日，而不是声称它是 11 月 10 日所在周。
- **实际结果**：T1.4 隔离测试在该 `as_of` 下读取本周，P1 为 `k=0, n=0`；测试按样本“W0 是本周”期望 `k=10, n=1000` 因而失败。11 月 2 日这一周的原标准答案测试仍通过。
- **为什么阻塞**：卡片 `current` 只能代表一个周期；将 11 月 2 日写成本周会与时区边界和现有周期计算冲突，改变样本时间又会影响撤回和其他时点条件。
- **方案 A（推荐）**：按真实日历周，11 月 9 日为当前周，11 月 2 日的 `k=10, n=1000` 保留为上周；修正 §8.2 的当前周表述，生成对应卡片快照，不改变该周原有标准答案或样本总体。
- **方案 B**：调整标准样本的 `as_of` 及相关撤回等时间，使 11 月 2 日确实是当前周；保留原数值期望，并逐项复核受影响的时点测试。
- **请求负责人决定**：采用 A、B 或指定其他规则。
- **答复（负责人，2026-11-13）**：采用 A。按上海真实日历周，11 月 2 日的 1,000 条作为上周数据，保留原标准答案。
