# PROGRESS

> 状态: 活跃计划
> 最后核对: 2026-11-13
> 适用范围: 机会卡任务的执行状态、验证结果与遗留问题。
> 事实来源: 实际执行的命令、测试输出与勘察结果。
> 冲突时以谁为准: RECON.md、DECISIONS.md 与实际命令输出。

## 2026-11-12 T0.1 代码库勘察

- 状态：done
- PR：无（T0.1 只新增文档，未提交；工作树基线 `main@ad19a4d7`）
- 结果：`docs/opportunity/RECON.md` 已按附录 A 填写；所有条目给出 `文件路径:行号` 或实际命令输出。现有测试运行 `cargo test --workspace --no-fail-fast`：1 个失败、其余通过；失败为 `apps/api/src/local_web/creator_lifecycle_tests.rs:214-232` 的 `target_route_never_reflects_an_invalid_selected_work`（期望 200，实际 503）。`cargo fmt --check` 在既有测试支持文件上失败；`apps/web/node_modules` 与 `apps/web/dist` 不存在，前端 typecheck/lint/test/build 未运行。T0.1 未修改任何代码、未执行 migration、未写数据库。
- 数据：迁移目录 196 个 SQL 文件（数字前缀 0001–0195；0064、0065 各有两份），当前链头 `0195_observation_transition_immutable`；未统计真实业务数据量（T0.1 不读生产数据）。
- 遗留：T0.2 需先回答 Q-01（评论无 `published_at`）与 Q-09（`observed_at` 命名冲突）；T0.3 需回答 Q-03（无 Rust 日期库）；R14 需回答 Q-02（无通用特性开关）；T0.5 需回答 Q-06/Q-07；T0.6 需回答 Q-08；§6.2.1 需回答 Q-04/Q-05；T2.2 需回答 Q-10。详见 `docs/opportunity/QUESTIONS.md`。

## 2026-11-12 T0.1b 基线同步与补充勘察

- 状态：done（勘察完成；Q-11、Q-12 已由 D-021、D-022 答复）
- PR：无（T0.1b 只更新文档，未修改代码；工作树 `.worktrees/opportunity-card-20261112`，分支 `claude/opportunity-card-20261112`，基线 `origin/main@4d65e506`）
- 同步：主工作树 `docs/README.md` 同时有他人未提交改动和上游改动，无法 `pull --ff-only`；按 D-001 停止并询问后，负责人选择独立 worktree。主工作树未改动。迁移文件 203 个（0001–0202），本地 ledger 头 `0202_topic_core_merge_prompts`。
- 差异：`ad19a4d7..HEAD` 在勘察路径中变更 143 个文件（intelligence 97、local_web 16、evidence 14、migrations 7、data-contracts 7、web app 2；`env.example`/`Cargo.toml`/`package.json` 0）。新增 migration 0196–0202 只涉及内容理解、topic research、ops 交接与 AI 任务/外发约束；未改评论/作品/领域/问题表结构。
- 测试：新基线 `cargo test --workspace --no-fail-fast` → `111 passed; 1 failed; 127 ignored`，失败仍是 `creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work`（`apps/api/src/local_web/creator_lifecycle_tests.rs:226:5`，503 vs 200）；`cargo fmt --check` 仍 exit 1，首处 diff 在 `crates/evidence/tests/support/standard_dossier_fixture.rs:31`。与 T0.1 基线属同一失败集合。
- 数据（只读 `SELECT`，仅聚合数字）：
  - first_admission 覆盖 29017/29017 个稳定身份（100%），观察 31771 条；`observed_at` 全部可解析为 `timestamptz`。
  - 评论平台时间：comments+replies 包 31809 条记录中，13 位 epoch 仅 45 条（0.14%），31764 条为 0（采集时间回退）；`publishedAtText` 100% 有值但多为展示日期/相对文本。
  - 每篇作品评论数：1735 篇，P50=14、P90=30、最大 731；1450 篇有 ≥2 个评论包，单作品最多 28 包；1478 个身份（5.1%）在 ≥2 包中被重复观察；多包作品的 28423 个身份中 11643 个（41.0%）首次入档晚于该作品最早的评论包，说明重复采集会补到新评论。
  - `linggan_material_domain_usage.created_at` 可用于 `as_of`，但 legacy 回填行的时间是迁移时间。
  - 0064 problem 表不再被生产代码写入；0177 无合并/取代/问题级撤销语义，且 `problem_ref UNIQUE` 使 `def_version` 不会变化。
  - OCR/转写存储已存在（`linggan_material_derived_text`、`linggan_material_reading`、`linggan_note_material_fragment`）。
  - HMAC 实现私有，可按 D-013 改 `pub`；`model_secrets` 无通用读取函数。
- 遗留：Q-11 由 D-021 答复（v1 保持 `work_published_cohort`，不解析展示文本，采集侧修复移交），并由 D-028 进一步定为目标口径、纳入阶段 C；Q-12 由 D-022 答复（以代码实际缺省为准，提取共享 `pub` 解析函数）。详见 `docs/opportunity/DECISIONS.md`、`docs/opportunity/QUESTIONS.md` 与 `docs/opportunity/RECON.md` §7。
- 遗留（运维）：支持目录解析在 `model_secrets`、`media_worker_support`、`paddle_ocr.py` 等处存在多处副本；按 D-022 不在本项目内合并。

## 2026-11-12 文档同步：D-021 至 D-031、手册 v1.3 与阶段 C

- 状态：done（仅文档更新；未修改代码、未执行 migration、未写数据库）
- 内容：按负责人提供的更新版替换 `DECISIONS.md`（D-001 至 D-031）、`playbook.md`（v1.3）与 `GLOSSARY.md`（追加 D-028 至 D-030 术语）；同步更新 `QUESTIONS.md`（Q-11/Q-12 已答复）、`RECON.md` §7.10 与本文件。
- 影响：新增阶段 C（C1–C4）采集侧任务、`period_basis = comment_published` 目标口径、`linggan_material_comment_platform_time` 只追加表、生产参数恢复为测试参数值、`collection_below_target`/`collection_protocol_changed` 等 flags。
- 未开始：C1 及之后、T0.2 与其他实现任务；等待负责人明确授权。
- 已确认（四参数签名）：资格函数与排除函数统一为 `(domain_ref, as_of, tz, period_basis)`，返回字段 `work_published_at` 改为 `period_anchor_at`；§6.2.1 为唯一签名，§6.2.3 示例、T0.2 任务卡与 D-012 已同步。另约定 `tz`/`period_basis` 只从 `metric_definition.params` 读取，C3 之前传 `comment_published` 必须报 `comment_published_basis_not_ready`，不得回退，需测试覆盖。

## 2026-11-12 C1 采集侧勘察

- 状态：done（只读勘察；未修改代码、未访问平台、未写数据库）
- 结果：`docs/opportunity/RECON.md` 新增 §8“采集侧补充勘察”，覆盖 C1 的六项：排序方式、30 条上限、版本字段、`publishedAt` 类型与校验、`0048` 时间提取复用、重新采集入口与控制。
- 关键事实：
  - 小红书评论请求不带排序参数，`sortMode` 固定 `unknown`；能力登记只证明默认顺序，平台是否支持时间排序未验证。
  - 30 是标准详情窗口硬上限（`localExecutionSupport.js:23`、`:177-181`），批量/深采可通过 `commentLimit` 配置。
  - 包 `contractVersion` 固定 `linggan.producer.capture-package.v1`；`commentCollection.version` 固定为 1 且 Rust 校验恰好等于 1；插件版本不写入包。
  - 包合同不校验单条评论 `publishedAt`；插件缺失时写 `0`，改为 `null` 只影响插件规范化与新增 fixture。
  - `0048` 的毫秒 epoch 判定可复用为 C3 投影语义，但当前写在视图里，不是独立函数。
  - 按作品补评论走 `deepen_comments`，一次最多新增 30 条；授权、风险暂停、安装冷却与并发控制共用现有采集链。
- 遗留：`QUESTIONS.md` Q-13（评论排序协议）待负责人裁定；C2 在 Q-13 裁定前不开始。

## 2026-11-12 Q-13 裁定

- 状态：done（文档更新）
- 结果：负责人选 A，固定平台默认顺序，`collection_protocol.sort_mode = "platform_default"`；见 D-032。`QUESTIONS.md` Q-13 已标记答复，`DECISIONS.md`、`docs/README.md` 同步。

## 2026-11-12 C2 插件写入评论时间并去掉回退

- 状态：实现与本地验证完成；未合并、未发布、未访问平台、未执行真实采集、未写数据库。
- 代码：
  - 小红书与抖音评论映射：只有数值型平台字段可作为时间来源，秒值乘 1000，写入毫秒 epoch；缺失、0、无效值或展示文本写入 `null`。删除展示文本 `Date.parse` 推导。
  - `recordNormalization.js`：删除 `record.publishedAt || record.createdAt` 回退；`publishedAt` 只接受正数，否则 `null`，不再写 0 或采集时间。
  - 评论包新增 `collectionProtocol: { version: 1, sortMode: "platform_default", singleRunLimit, triggerSource }`；顶层字段存在即标记 C2 之后的时间语义，旧包无该字段仍被接收。
  - 调用方传入 TaskSpec：`scheduled` → `scheduled`，带 `incrementalCommentBudget` → `incremental`，本地手动 → `manual`；无法判断 → `unknown`。`singleRunLimit` 取 receipt 的 `requestedLimit`，全量遍历为 `null`。
  - Rust `producer_runtime`：`collectionProtocol` 可选；存在时校验仅用于 comments/replies、版本为 1、排序为 `platform_default`、上限为 `null` 或 1–2048、触发来源为闭集，并要求非空 `publishedAt` 是毫秒 epoch（≥ 1e12）；旧包跳过这些校验。
- 验证：
  - 插件测试（`TZ=Asia/Shanghai npm run test:linggan`）：333 passed, 0 failed。未设 TZ 时 `xhs-note-collector` 的既有“昨天/小时前”测试因本机时区为 PST 失败，与 C2 无关。
  - `npm run check:contracts`、`npm run build`：通过。
  - `cargo test -p linggan-contracts`：通过，新增 `collection_protocol_marks_c2_packages_while_pre_c2_packages_stay_acceptable`。
- 新增测试：
  - 带 `create_time`（秒）与带毫秒值样例映射为毫秒 epoch；无时间样例为 `null`，不等于采集时间；仅有展示文本时为 `null` 且保留 `publishedAtText`。
  - `createdAt` 存在时 `publishedAt` 仍为 `null`；0 视为未知；显式平台时间保留。
  - 评论包携带协议字段；scheduled/incremental/manual/unknown 与全量无上限样例。
- 遗留：独立审查与合并授权；C3 投影器需按 `collectionProtocol.version` 选择时间语义，旧包按 0/缺失处理。

## 2026-11-12 T0.3 口径定义与周期工具

- 状态：实现与隔离库验证完成；未合并，未在共享库或生产库执行 migration，未写真实数据。
- 数据库（`database/migrations/0203_opportunity_metric_definition.sql`）：
  - 按 D-004 使用 `public` schema 与表名前缀：`linggan_opportunity_metric_definition`、`linggan_opportunity_metric_definition_current`；`singleton` 约束保证只有一个当前版本。
  - seed 写入生产参数 v1，数值与 §6.3 固定测试参数集一致（含 `period_basis = work_published_cohort`、`period_tz = Asia/Shanghai`），并指向当前版本。
  - 按 D-020 新增 `linggan_opportunity_period_start(ts timestamptz, tz text) RETURNS date` 与 `linggan_opportunity_period_bounds(p date, tz text) RETURNS TABLE(start_utc timestamptz, end_utc timestamptz)`；时区只由参数传入，函数内不写死。
  - 按仓库约定登记到 `apps/api/src/local_web/full_schema_fixture.rs`（`include_str!` 与 sha256 台账 `1ee95ccf…`）和 `scripts/local-runtime.sh` 的 `apply_migration_once`；未执行到任何共享库或本地运行库。
- Rust（`crates/intelligence/src/opportunity.rs`、`opportunity/metric_definition.rs`）：
  - `load_current_metric_definition` 从当前版本读取 `params`；缺失返回 `opportunity_metric_definition_missing`，读取失败返回 `opportunity_metric_database_unavailable`。
  - 逐项校验：`z` 为正有限数；计数类参数大于 0；比例类参数在 0–1；`period` 只接受 `week`；`period_basis` 只接受 `work_published_cohort`/`comment_published`；`week_start` 只接受 `monday`；`gate_classes` 非空、不重复且只含 `answers`/`mentions`；不合法返回 `opportunity_metric_definition_invalid` 并拒绝加载。
  - 业务模块不含时区字符串或周期常量；`Asia/Shanghai` 只出现在 seed 数据与测试参数集中。
- 验证：
  - `cargo test -p linggan-intelligence --lib --locked`：335 passed, 0 failed, 10 ignored。
  - `cargo test -p linggan-intelligence opportunity:: --locked`：3 passed，覆盖固定测试参数集加载、逐项非法值、缺失字段与未知字段。
  - `scripts/test-opportunity-postgres.sh`：隔离容器内 2 passed，覆盖 §6.4.3 四条周期边界（含 `period_bounds` 的 UTC 起止）与 seed 加载；修改 seed 的 `z = 0` 后加载被拒绝。容器、卷与数据库在退出时删除并校验。
- 遗留：独立审查与合并授权；迁移应用到任何共享/生产数据库前需单独授权。下一步 T0.2（评论事实视图与资格函数）。

## 2026-11-12 T0.2 评论事实视图与资格函数

- 状态：实现与隔离库验证完成；Q-14 已答复，暂停期数据不触发 D-024 停止条件（D-033），任务完成。未合并，未在共享库或生产库执行 migration，未写真实数据。
- 数据库（`database/migrations/0204_opportunity_comment_eligibility.sql`）：
  - 新增 `linggan_opportunity_observed_at(text)`：解析失败返回 `NULL`；按 D-018，`observed_at` 的文本到 `timestamptz` 转换只出现在本迁移。
  - 视图 `linggan_opportunity_comment_fact`：按 `(content_ref, comment_external_id)` 聚合成一行；`first_observed_at` 取首入档包 `accepted_at`，无首入档行时回退最早可解析 `observed_at` 并置 `first_observed_backfilled = true`；同时输出 `first_observed_parseable`、`commenter_external_id_conflict`（§6.2.1）、`is_reply`、观察次数与不可解析观察数。评论者取值优先首入档包，否则按最早观察时间与包/材料引用排序。
  - 唯一内部实现 `linggan_opportunity_comment_eligibility(domain_ref, as_of, tz, period_basis)`：`comment_published` 直接报 `comment_published_basis_not_ready`（不回退）；未知口径报 `unknown_period_basis`；空时区报 `period_tz_required`。按 D-023 处理 `created_at <= as_of` 与 legacy 行；周期锚点取 `as_of` 前最新有 `published_at` 的详情观察；排除原因使用闭集并固定优先级：`not_in_domain_primary`、`work_author_unknown`、`period_anchor_missing`、`observed_at_unparseable`、`restricted`、`creator_self`。
  - 对外两个包装函数 `linggan_opportunity_eligible_comments` 与 `linggan_opportunity_eligibility_exclusions` 均只调用上述内部实现，等式“有效 + 各原因排除 = 全部候选”由结构保证。
  - 按仓库约定登记到 `apps/api/src/local_web/full_schema_fixture.rs`（sha256 `2ad6bda8…`）与 `scripts/local-runtime.sh`；未执行到任何共享库或本地运行库。
- 证明（隔离 PostgreSQL 容器，`scripts/test-opportunity-postgres.sh`）：
  - 4 passed, 0 failed；新增两条 T0.2 证明（§8.2 数据集、口径拒绝），T0.3 的周期边界与 seed 加载继续通过。
  - §8.2：有效 5800；`not_in_domain_primary` 3、`work_author_unknown` 5、`period_anchor_missing` 4、`observed_at_unparseable` 0、`restricted` 2、`creator_self` 1；候选 5815 = 5800 + 15；回填有效 10；回复 50；评论者冲突 0；`as_of = 2026-11-11` 时有效 5803。追加一条不可解析 `observed_at` 后，`observed_at_unparseable` 变为 1，候选变为 5816。
  - `comment_published` 对两个对外函数均报 `comment_published_basis_not_ready`。
- 真实库只读统计（仅 `SELECT`，`default_transaction_read_only=on`；0203/0204 未应用，资格逻辑以只读 CTE 复刻）：
  - `first_observed_at` 来源：稳定身份 29017；首入档包 29017（100%）；回退可解析 0；回退不可解析 0；观察 31771 条；不可解析观察 0。
  - 评论者一致性（§6.2.1）：稳定身份中评论者冲突 0（无多值、无“有值/空值”混合）。
  - 回复占比（D-025）：有效评论 22003 条，其中回复 4858 条，22.08%；可区分。
  - D-024 第 4 款分布：按最近 12 个自然周计算为 0 个领域-周（最新周期锚点 2026-10-21，落在当前周回溯 12 周窗口之外）；改用数据中最近 12 个有数据周期（2026-07-27 至 2026-10-12）：
    - 领域-周 33 个；周 n 的 P25 = 27、P50 = 57、P75 = 211；最小 4，最大 1390。
    - 有数据领域 3 个：ADHD 周中位 530.5（12 周）、考研自习 29.5（10 周）、自闭症干预 39.0（11 周）；第 4 个领域无有效数据。
    - 多数（3 个中的 2 个）领域周 n 中位数低于 100。负责人说明前端采集已手动暂停一个多月，数据库反映暂停状态，不是项目设计或采集能力的结论；暂停期数据不作为停止条件（Q-14 / D-033），保持周周期继续推进。
  - 作者 90 天窗口：有有效评论的作者 417 个；作者有效评论数 P25 = 3、P50 = 11、P75 = 23，最小 1、最大 825；按 `creator_min_n = 50`，充足 15 个、样本不足 402 个。
  - 各证据等级的问题数：无法产出。当前库只有 `linggan_comment_problem_*` 表，没有 evidence level 字段，且 `linggan_comment_problem_definition` 为 0 行；机会卡的问题与证据等级尚未实现（T1+）。
- 遗留：独立审查与合并授权；迁移应用到任何共享/生产数据库前需单独授权；采集恢复后重跑 D-024 只读报告；C3 需要替换 `comment_published_basis_not_ready` 分支。

## 2026-11-12 T0.4 统计函数模块

- 状态：实现与单元测试完成；未合并。纯函数模块，不访问数据库、不读取配置。
- 代码（`crates/intelligence/src/opportunity/stats.rs`，由 `opportunity.rs` 重导出）：
  - `wilson_interval(k, n, z)`：按 §6.4.1 实现；`n = 0` 返回 `EmptySample`，`k > n` 返回 `CountExceedsTotal`，`z` 非有限或非正返回 `InvalidZ`；不把错误改成默认值。
  - `rate_per_1k(k, n)`：`n = 0` 返回 `None`。
  - `evidence_tier(input, params)`：按 `mentioners_total`、`min_total_for_significant`、`min_k_significant`、`min_n_period` 与 Wilson 下界相对基线的比较返回 `candidate` / `established` / `significant`。
- 测试（`cargo test -p linggan-intelligence --lib opportunity::stats --locked`）：
  - 7 passed, 0 failed。
  - §6.4.2 全部 12 条 Wilson 标准答案（容差 `1e-6`）与三类错误情况通过。
  - §6.4.4 全部 7 条证据等级标准答案通过。
  - 三条性质测试使用固定种子各生成 10,000 组 `0 <= k <= n <= 100000`：区间包住点估计、固定 `n` 时 `k` 增大下界不减小、固定比例时 `n` 翻倍区间宽度不增大。RECON §4.8 未发现现成性质测试库，因此未新增依赖，使用测试内的固定种子 LCG。
- 遗留：独立审查与合并授权。

## 2026-11-12 T0.5 问题登记与谱系

- 状态：实现与隔离库验证完成；未合并，未在共享/生产库执行 migration，未写真实数据。
- 数据库（`database/migrations/0205_opportunity_problem_taxonomy.sql`）：
  - 新建 `linggan_opportunity_taxonomy_version`、`linggan_opportunity_problem`、`linggan_opportunity_problem_lineage`；按 D-014，`source_ref = problem_ref`、`source_definition_ref = definition_ref`、`level = 1`、`parent_id = NULL`，类型跟随现有表使用 uuid。
  - `linggan_opportunity_sync_problems()` 从 0177 的 `linggan_comment_problem_definition` 登记问题；同一 `source_ref` 的 `source_definition_ref` 变化时 `def_version` 加 1，未变化不写。迁移末尾执行一次同步。
  - 谱系关系为闭集 `merge` / `rename` / `split`；跨领域关系被拒绝；同一 `operation_ref` 的一次合并或拆分只递增一次领域 `taxonomy_version`；`rename` 不递增。0177 没有合并、改名或拆分语义，因此真实同步只登记问题，不写谱系（D-014）。
  - `linggan_opportunity_resolve_problem_id(id)` 沿 `merge` / `rename` 链返回当前问题与完整路径；遇 `split` 返回所有直接子问题及路径，交给调用方；成环报 `problem_lineage_cycle`，不进入死循环。
  - 按仓库约定登记到 `apps/api/src/local_web/full_schema_fixture.rs`（sha256 `8e7a8ce8…`）与 `scripts/local-runtime.sh`；未执行到任何共享库或本地运行库。
- 验证（隔离 PostgreSQL 容器，`scripts/test-opportunity-postgres.sh`）：
  - 5 passed, 0 failed；新增 T0.5 证明覆盖直达、两跳合并、改名后合并、拆分、成环五种情况。
  - 同步：首次登记 2 条，重复执行 0 条；`definition_ref` 变化后 `def_version` 从 1 变为 2。
  - 版本：一次 2 行拆分只递增 1 次；两次合并、两次成环边、拆分与改名混合后，领域版本为 7。
  - 真实库只读：`linggan_comment_problem_definition` 当前 0 行，因此现有问题登记数为 0，与评论研究一致；这不是同步失败。
- 遗留：独立审查与合并授权；真实库问题数随 0177 数据增长后需重跑同步。

## 2026-11-12 T0.6 评论者哈希

- 状态：实现与单元测试完成；未合并。未写真实数据，未读取真实评论者标识。
- 代码（`crates/intelligence/src/opportunity/commenter_hash.rs`，由 `opportunity.rs` 重导出）：
  - `CommenterDigest::hash(Some(author_external_id))` 返回 `hmac_sha256_hex(secret, author_external_id)` 的小写十六进制（64 字符）；`digest_version()` 固定返回 `commenter-hmac-sha256-v1`。`author_external_id` 为 `None` 时返回 `None`，调用方据此写 `commenter_hash = NULL`、`excluded_reason = 'commenter_unknown'`。
  - 密钥路径固定为 `<support_dir>/opportunity/commenter-digest.key`，要求 `<support_dir>` 与 `opportunity` 目录均为私有目录、密钥文件为私有文件；密钥少于 32 字节时拒绝。没有空密钥回退，也不生成密钥。
- 密钥解析：
  - 从 `crates/intelligence/src/model_secrets.rs` 提取 `pub fn support_dir()`，`model_secrets` 自身改用同一函数；优先级与默认值保持不变：非空 `LINGGAN_SUPPORT_DIR`，否则 `$HOME/Library/Application Support/Linggan Intelligence`，两者都为空时返回 `None` 并拒绝启动。未新增第二份路径解析。
  - 失败原因区分 `SupportDirUnset`、`SupportDirUnavailable`、`KeyUnavailable`、`KeyTooShort`；错误文本包含解析后的路径与最小字节数，不含密钥内容。
- 加密实现：
  - 复用 `crates/evidence/src/collection_control.rs` 的 `hmac_sha256_hex`，仅将其与 `collection_control` 模块导出改为 `pub`，行为未变；`linggan-evidence` 的 RFC 4231 向量测试继续通过（1 passed）。未复制加密代码。
- 测试（`cargo test -p linggan-intelligence --lib opportunity::commenter_hash --locked`）：
  - 8 passed, 0 failed。
  - 同一 ID 与同一密钥得到相同哈希；不同密钥得到不同哈希；`None` 仍为 `None`；31 字节密钥被拒绝且错误文本含最小 32 字节；缺失密钥报错含解析路径；私有目录中的密钥可加载并得到相同哈希。
  - 原始 ID 不出现在哈希、`Display`、`Debug` 或版本字符串中；模块内没有任何 `println!` / `eprintln!` / `log::` / `tracing::` / `dbg!` 输出语句，该断言通过扫描模块源码在测试中执行。
- 验证：
  - `cargo test -p linggan-intelligence --lib --locked`：351 passed, 0 failed, 10 ignored。
  - `bash scripts/ci-preflight.sh origin/main`：通过（51 项脚本测试全过）。
- 遗留：独立审查与合并授权；机会卡写入与计算任务接入时必须调用 `CommenterDigest::load()` 并在失败时拒绝启动，不得自行读取密钥或退回空密钥。

## 2026-11-12 T0.7 测试数据集与不变量检查

- 状态：实现与隔离库验证完成；未合并，未在共享/生产库执行 migration，未写真实数据。
- 数据集构建器（`crates/intelligence/tests/support/opportunity_dataset.rs`）：
  - §8.2 的种子 SQL 从集成测试移入可复用模块，`DOMAIN`、`AS_OF`、`period_tz`、`period_basis` 与手册一致。
  - 所有人工标识改为稳定摘要：包、任务、尝试、材料、详情、评论、领域用途的 UUID 由 `md5(<固定文本>)::uuid` 派生；固定时间的 `created_at` 显式写入。运行结果不再依赖 `gen_random_uuid()` 或当前时钟。
  - 连续两次构建分别写入两个独立隔离 schema；对内容、详情、领域用途、评论、首入档、限制、任务、尝试、捕获包、处置记录与 `linggan_opportunity_comment_fact` 做逐行 `row_to_json` 排序后摘要，两次结果相同，且有效评论数与排除计数一致。
- 不变量检查（`crates/intelligence/src/opportunity/invariants.rs`）：
  - 按 §8.3 登记 I1–I10（含 I6b）。每条检查有稳定 id、名称、依赖表与只读计数 SQL；依赖表不存在时输出 `available = false`、`violations = 0` 与 `missingTables`，不伪装成通过。可用检查出现违规时报告 `failed`。
  - 当前迁移集内只有 I8（谱系不成环）可执行；I1–I7、I6b、I9、I10 依赖的度量子表、判定表、提及表、创作者密度表、机会卡表与质量门尚未在 T0 建表，因此明确标为不可用。
  - I8 使用递归 CTE，合并/改名边走满 100 层上限，成环返回违规行数而不是死循环。
- 命令（`apps/worker/src/bin/opportunity.rs`，二进制 `linggan-opportunity`）：
  - `linggan-opportunity check-invariants [--schema SCHEMA]` 读取 `LINGGAN_LOCAL_DATABASE_URL`；默认连接当前 schema，`--schema` 仅用于隔离证明库，schema 名经过仓库既有的安全校验。
  - 输出 JSON：每条检查的 `id`、`name`、`available`、`violations`、`missingTables`，以及 `totalViolations`、`incomplete`。有违规时退出码失败；仅有不可用检查时退出成功，由 `incomplete` 明示。
- 验证（`scripts/test-opportunity-postgres.sh`）：
  - 7 passed, 0 failed；新增两条 T0.7 证明：两次构建摘要一致、不变量在 §8.2 数据集上全为 0。
  - 人为写入两条互指的 `merge` 谱系后，I8 报告违规且 `failed = true`；删除谱系后恢复为 0。
  - 脚本在测试后以 `check-invariants --schema opportunity_t07_invariants` 调用命令，JSON 中 11 条检查全部 `violations = 0`，退出成功。容器、卷与数据库在退出时删除并校验。
  - `cargo test -p linggan-intelligence --lib --locked`：352 passed, 0 failed, 10 ignored。
  - `bash scripts/ci-preflight.sh origin/main`：通过（51 项脚本测试全过）。
- 遗留：独立审查与合并授权；后续阶段建表后，相应检查自动从 `available = false` 变为可执行，需在 CI 中重新确认全为 0；I10 的 `quality_gate` 表名按手册原文登记，实际建表时若命名不同需同步更新登记。

## 2026-11-12 C3 平台时间投影与校验

- 状态：投影、校验与只读报告实现完成；隔离库验证通过。未合并，未在共享/生产库执行 migration，未写真实数据。
- 数据库（`database/migrations/0206_comment_platform_time.sql`，sha256 `031971ac…`）：
  - 新建只追加表 `linggan_material_comment_platform_time(content_public_ref, comment_external_id, published_at, precision, source_package_ref, protocol_version, created_at)`，以评论身份为主键；`precision` 当前只允许 `millisecond`；更新与删除由既有 `linggan_plugin_runtime_forbid_mutation()` 拒绝。
  - 表名按 D-028 原文固定，不使用 `linggan_opportunity_` 前缀；这是 D-005 对象命名规则的一处明确例外，已在迁移头注释中记录。未 `ALTER` 任何现有评论表。
  - `linggan_opportunity_project_comment_platform_times()` 只扫描 `collectionProtocol.version = '1'` 且 `sortMode = 'platform_default'` 的评论包；按 `accepted_at, package_ref` 顺序调用逐包投影，因此同一身份以第一条有效值为准。`publishedAt` 必须是数字且落在 `[1000000000000, 253402300800000)` 的毫秒 epoch 区间（与 `0048` 的上界一致）；缺失、为 `null` 或超界时跳过，不写入 0、也不使用采集时间。返回 `(inserted, conflicts)`；`conflicts` 是同一身份在不同包中出现多个不同 `published_at` 的身份数。
  - 包内记录与评论按 `record_ordinal = ordinality - 1` 对齐，跟随 C2 采集记录从 0 开始编号的既有约定。
  - `linggan_opportunity_comment_eligibility()` 现已接受 `comment_published`：该口径的周期锚点取平台时间；无平台时间时写 `comment_time_unknown` 排除原因。排除顺序为 `not_in_domain_primary` → `work_author_unknown` → `comment_time_unknown` → `period_anchor_missing` → `observed_at_unparseable` → `restricted` → `creator_self`。未知口径仍明确报错，不会悄悄退回按作品发布周计算。
  - `linggan_opportunity_platform_time_coverage(domain, as_of, tz, periods)` 按工作发布周口径统计每周有效评论数与其中有平台时间的数量，分母为 0 时 `ratio = NULL`。
  - `linggan_opportunity_platform_time_display_comparison / agreement / mismatches` 只在核对时读取展示文本，比较本地月日与可选年份；展示文本不写入任何时间列。
- 代码：
  - `crates/intelligence/src/opportunity/platform_time.rs`：包装投影、覆盖率、展示文本一致率与不一致明细，以及 `timing_violations`（`published_at > first_observed_at` 的行数）；`current_as_of()` 从数据库时钟生成 UTC 文本。日期与时间仍以文本返回，不新增日期库依赖。
  - `crates/intelligence/src/opportunity/invariants.rs`：新增 I11 `platform_time_not_after_first_observation`，把 C3 第一条验收固定为每次不变量检查都会执行的项目。I11 不在手册 §8.3 的 I1–I10 列表中，属于 C3 验收的补充检查；I1–I10 的 id、名称与 SQL 未改。
- 命令（`linggan-opportunity`）：
  - `project-platform-times [--schema SCHEMA]`：执行全量投影，输出 `inserted` 与 `conflicts`。
  - `platform-time-report --domain UUID [--as-of RFC3339] [--tz TZ] [--periods N] [--schema SCHEMA]`：输出最近 N 周（默认 12）的每周覆盖率、展示文本一致率/不一致明细与时间违例数。`--tz` 未给时从当前 `metric_definition.params.period_tz` 读取，不在代码内写死；`--as-of` 未给时取数据库当前时间。存在时间违例时退出码失败。
  - `check-invariants` 行为不变，现在会额外输出可用的 I11。
- 验证（`scripts/test-opportunity-postgres.sh`，隔离 PostgreSQL 容器）：
  - 11 passed, 0 failed。新增 C3 覆盖：口径不回退、投影幂等与冲突计数、第一条值优先、毫秒区间越界跳过、每周覆盖率、展示文本同日一致与逐条不一致、I11 检测。
  - 口径不回退：投影前 `comment_published` 有效评论为 0，5807 条候选因 `comment_time_unknown` 排除；投影后有效 5803 条，仅 1 条缺时间被排除。工作发布周口径始终为 5800，未受新表影响。
  - 幂等与冲突：首次投影 `conflicts = 0`，第二次 `inserted = 0`；追加一个给出不同时间的后到包后 `inserted = 0`、`conflicts = 1`，表中仍保留第一条时间 `2026-10-27T00:01:00Z`。
  - 毫秒区间：把 `c-work-2-1` 的 `publishedAt` 改成上界 `253402300800000` 后，投影不报错且跳过该条；同一包内 `c-work-2-2` 正常写入。
  - 覆盖率：12 周中 5 周有数据；有效评论合计 5800，有平台时间 5799；唯一不足周为 `2026-10-26`（1200 条中 1199 条）。
  - 展示文本：5799 条参与核对，5798 条同日一致（99.9828%），不一致仅 `c-work-1-17`（展示文本 `01-01`，实际 10-27），逐条列出。
  - I11：正常数据 0 违例；人为写入晚于 `first_observed_at` 的时间后 I11 = 1、报告 `failed = true`，命令报告 `timingViolations = 1`。
  - 测试数据集新增 C3 覆盖层（`build_platform_times`），只为评论包补协议标记与毫秒记录，不改变 T0.7 的 `build()` 结果与指纹。
  - 脚本在测试后以 `platform-time-report` 调用 `opportunity_c3_basis` schema，实际输出 12 周覆盖率、99.9828% 一致率与 0 时间违例。
  - `cargo test -p linggan-intelligence --lib --locked`：352 passed, 0 failed, 10 ignored；`cargo test -p linggan-contracts --lib --locked`：25 passed, 0 failed。
  - `bash scripts/ci-preflight.sh origin/main`：通过（51 项脚本测试全过）。
- 独立审查（2026-11-12）：未发现阻断或重大问题；两条 minor 已修复。
  - `publishedAt` 此前只有下界。包合同与 `0206` 投影 SQL 均已补上 `< 253402300800000` 上界（与 `0048` 一致），并新增越界跳过证明；越界值不再可能让整次投影因 `to_timestamp` 溢出而回滚。
  - `DECISIONS.md`、`QUESTIONS.md` 与 `docs/README.md` 的索引范围从 D-032/Q-13 更新为 D-033/Q-14。
- 未完成项：
  - C3 目标中的“影子模式下同步计算并保存这一口径的指标”尚未实现。当前迁移集没有 `linggan_opportunity_problem_metric_period`（T0.7 不变量输出已确认 I1–I3 不可用），该表由 T1.2 创建；在表存在前无法保存影子指标，不能只在内存计算后声称完成。待 T1.2 建表后接入 `comment_published` 的影子计算与保存。
- 遗留：独立审查与合并授权；影子指标依赖 T1.2；采集恢复后按 C4 补齐历史，并每周报告各领域覆盖率；切换口径需负责人批准并新增版本，不得直接改当前指针。

## 2026-11-12 C4 真实重采集（第 1 轮）

- 状态：真实采集与覆盖率报告完成；评论平台时间仍然拿不到，已登记 Q-15。未修改代码、未新增迁移。
- 授权与前置：负责人授权真实重采集；插件扩展已重新加载到当前工作树构建（`releases/linggan-intelligence-browser-v0.8.65`，manifest 0.8.66）。系统代理故障（CN 域名全部 503）由负责人修复后复测：直连 `baidu 200`、`xiaohongshu 302`。
- 执行方式：`POST /api/local/collection/material-deepening` 提交 3 个 `deep_archive` 批次，每个 4 个作品，`commentLimit = 30`、`replyExpandLimit = 2`、媒体/OCR/ASR 关闭；三批依次准入并全部执行完毕。
- 批次结果（数据库实测）：
  - ADHD `264f61a5-44f0-40e4-aa1a-49c0362c33c2`：4/4 作品；`content_detail` 4 包（4 条记录，全部有作品级 `publishedAt`）、`comments` 4 包 40 条记录、`replies` 4 包 39 条记录；12/12 任务完成，派发失败 0。该批在扩展重载前执行，包内无 `collectionProtocol`，`publishedAt` 写 `0`，不进入 C3 投影。
  - 考研自习 `50f53929-e41c-4698-a978-c0dcaec17476`：3/4 作品；`content_detail` 3 包（3 条，全部有作品级 `publishedAt`）、`comments` 3 包 30 条、`replies` 3 包 26 条；9/9 任务完成，派发失败 0。
  - 自闭症干预 `8439bab9-1557-4a16-b9f2-d174c640b8dc`：3/4 作品；`content_detail` 3 包（3 条，全部有作品级 `publishedAt`）、`comments` 3 包 30 条、`replies` 3 包 30 条；12 个任务中 9 个完成、3 个 `input_blocked`，派发失败 1（`execution_input_missing`）。
  - 两批各放弃 1 个作品：考研 `230037d0-cea7-4fe2-b0b2-6ca50dc5e79e`、自闭症 `4ce71ef6-07b4-4d89-9826-19db7db132b8`。两者在 `collection_execution_input_eligibility` 中均为 `input_blocked / execution_input_missing`（无可用签名定位器，分别冻结于 14:21:45 与 15:36:12），admission 按既有输入闸跳过，不是采集失败。
  - 风险冷却与暂停：本轮 `collection_installation_risk_cooldown` 0 行、活跃 `collection_risk_pause` 0 行、今日 `collection_installation_risk_signal` 0 条。
- 评论平台时间（关键事实）：
  - 重载后两批的评论包已带 `collectionProtocol.version = 1`、`sortMode = platform_default`（C2 协议生效），但 60 条评论记录和 56 条回复记录**全部**为 `sourceTier = dom`、`qualityReason = api_unobserved_dom_fallback`、`publishedAt = null`。
  - 全库历史（`comments` 2247 包、`replies` 2238 包，共 31928 条记录）：只有 2026-08-31 的 4 个包（用户前台手动批量采集，scope 为 `all_public_comments` / `maximum_quota`）存在 API 记录与毫秒平台时间；其余所有日期，包括全部派发详情会话，API 记录数为 0。
  - 原因定位（代码 + 实测）：插件 API 优先路径在页面中直接 `fetch('/api/sns/web/v2/comment/page', { credentials: 'include' })`，不带小红书签名头（`src/injected/xhsApiCapture.js:368`；`src/platforms/xhs/commentApi.js:803-824`）；未签名请求被平台网关拒绝（实测 HTTP 500 `create invoker failed, service: jarvis-gateway-default`）。派发会话又用 `chrome.windows.create({ focused: false })` 在非聚焦窗口打开详情页（`src/linggan/background.js:1466`），能拿到签名的只剩页面自身分页流量，而现有记录显示该流量没有被捕获到评论快照。
  - 结论：当前插件版本在派发会话中无法取得评论平台时间；C4 的“新获得平台时间评论数”为 0，`comment_published` 口径仍不可用。已登记 Q-15，等待负责人裁定（修插件 / 暂缓口径 / 先做前台诊断）。
- 覆盖率与不变量（本地运行库，`as_of = 2026-11-12`，`tz = Asia/Shanghai`，`periods = 12`，作品发布周口径）：
  - `project-platform-times`：`inserted = 0`、`conflicts = 0`（没有带协议且 `publishedAt` 有效的记录可投影）。
  - ADHD：12 周内有效评论 7118，有平台时间 0，`ratio = 0.0`；8 周有数据（2026-08-24 至 2026-10-12）。
  - 考研自习：159 / 0，`ratio = 0.0`；6 周有数据。
  - 自闭症干预：282 / 0，`ratio = 0.0`；8 周有数据。
  - 三个领域 `agreement.compared = 0`、`timingViolations = 0`。
  - `check-invariants`：`totalViolations = 0`、`incomplete = true`（I1–I10 依赖的 T1 表尚未建立）；I11 `available = true`、`violations = 0`。
- 操作记录：本地库凭据以容器当前环境为准（工作树 `.env` 的 `POSTGRES_PASSWORD` 与容器内角色口令不一致，直接使用 `.env` 连接会报鉴权失败；本轮连接串从容器环境读取，未写入任何文件）。
- 遗留：Q-15 裁定前，C4 无法产出评论平台时间，`comment_published` 的影子指标仍不可启用；第 2 轮重采集应在插件问题修复后执行，并优先覆盖两个 `input_blocked` 作品（需先获得新的可用签名定位器）。

## 2026-11-12 C3b 日精度平台时间（答复 Q-15）

- 状态：迁移、投影、读取视图与测试实现完成；隔离库验证通过；经负责人授权，已在本地运行库执行迁移与历史回填。未在共享/生产库执行，未发起真实采集。
- 背景：C4 与探针确认派发会话拿不到毫秒 epoch；但只读核查显示 32,219 条评论/回复全部有 `publishedAtText`，日精度可解析率接近 100%。负责人裁定采用日精度（D-034），范围为新采集 + 历史回填。
- 数据库（`database/migrations/0208_comment_platform_time_day_precision.sql`）：
  - 表新增 `source_kind`（`epoch`、`display_full_date`、`display_month_day`、`display_relative`）；`precision` 允许 `day`；`protocol_version` 允许 0（历史包无协议）。
  - 主键改为 `(content_public_ref, comment_external_id, precision)`，历史回填的日精度行不会阻止以后写入精确 epoch。只追加与禁改触发器不变。
  - 新增视图 `linggan_material_comment_platform_time_current`，同一评论毫秒行优先；覆盖率、资格、不变量与时间违例全部改读该视图。
  - `linggan_opportunity_parse_comment_display_date(text, collected_at)` 解析 `YYYY-MM-DD`、`MM-DD`、`昨天`、`刚刚`、`N天前`、`N小时前`、`N分钟前`。`MM-DD` 的年份取采集时刻所在上海年份，候选日期晚于采集日时归入上一年；`N小时前`/`N分钟前` 按实际时长回退；日精度统一存上海当地零点。无法解析的文本返回 `NULL`，不猜测。
  - `linggan_opportunity_comment_collected_at(record, package.accepted_at)` 优先读记录自身 13 位毫秒 `collectedAt`，缺失时退回包裹 `accepted_at`。相对时间按**该条记录的采集时刻**推算，不使用当前时间。
  - epoch 的可信范围保持 C2 规则：协议 v1 包，或无协议但记录 `sourceTier = 'api'` 的历史包。历史 `dom` 回退的 `createdAt` 顶替值不采信，改用展示文本。
  - 投影器分两遍执行：先全部毫秒包，再全部日精度包；冲突只由 epoch 观测计算，日精度不参与。旧的一参重载函数已删除。
  - 展示文本交叉核对只比较毫秒行，避免用日精度值验证自身来源。
- 后续修正（均以独立迁移，0208 保持已应用字节不变）：
  - `0209_comment_display_time_suffix.sql`：真实数据中相对标签会带时分（`昨天 00:04`、`昨天 14:35`），共 2,080 条。解析前剥离末尾 ` H:MM` / ` HH:MM`；标签本身决定日期，时分不改变日精度值。
  - `0210_comment_display_agreement_skips_iso.sql`：API 来源记录的 `publishedAtText` 是自身 epoch 的 ISO 序列化（`2026-08-30T16:06:01.000Z`），不是平台展示标签。此前拿它的 UTC 月日与上海当地日期比较，制造了 14 条假不一致；现在展示核对跳过 ISO 文本。
  - 编号说明：这三条迁移最初写作 0207–0209；合入 `origin/main` 时主干已占用 `0207_topic_single_pass`，故依序后移为 0208–0210，内容字节不变。
- 代码：`platform_time.rs` 与 `invariants.rs` 改读当前视图；测试数据集新增 `build_legacy_day_times()` 覆盖层。
- 验证（`scripts/test-opportunity-postgres.sh`，隔离 PostgreSQL 容器）：
  - 12 passed, 0 failed（原 11 条 + 新增日精度 1 条）。
  - 新增证明：日精度投影写入 10 条（含 `昨天 14:35` 与 `3天前 08:00`），日期分别为 `2026-10-27`、`2026-10-27`、`2026-10-24`、`2026-10-26`、`2026-10-27`、`2026-10-27`、`2025-12-31`、`2026-01-01`、`2026-10-27`、`2026-10-24`；`12-31` 由跨年规则归入 2025；来源分别为全日期、月日、相对时间；`protocol_version = 0`。
  - 日精度值按上海零点存储（`2026-10-27` → `2026-10-26T16:00:00Z`）；只有日精度行时展示核对 `compared = 0`、时间违例 0。
  - 之后补入精确 epoch：日精度行保留，当前视图返回 `precision = millisecond` 的 `2026-10-27T00:01:00Z`。
  - 把一条记录的展示文本改成 ISO 序列化后，展示核对不再把它计入 `compared`，避免 UTC/本地日期差被误报。
  - 改动文件 `rustfmt --check` 通过。
- 本地运行库执行（`linggan_intelligence_dev`，2026-11-12）：
  - 迁移 `0208`、`0209`、`0210` 以事务方式应用，并写入 `linggan_local_schema_migration`；`scripts/local-runtime.sh` 已登记三条迁移。
  - `project-platform-times` 首次 `inserted = 27,730`（日精度 27,689 + 毫秒 41），`conflicts = 0`；应用 `0209` 后再次执行补入 1,373 条，`conflicts = 0`。
  - 最终分布：`display_full_date` 6,173、`display_month_day` 16,784、`display_relative` 6,105、`epoch` 41，合计 29,103 条。
  - 覆盖率：29,104 条评论事实中 29,103 条有平台时间（99.997%）。唯一缺失记录的 `publishedAtText = '2/5'` 实为评论文本被误取为时间（DOM 中真实时间是 `昨天 14:35`），解析器拒绝该值是正确的。
  - 三领域（`as_of = 2026-11-12`，12 周）：ADHD 7,118 / 7,117；考研自习 159 / 159；自闭症干预 283 / 283。`timingViolations = 0`。
  - 展示核对 `compared = 0`：现存 epoch 行的展示文本都是 ISO 序列化，日精度行又不能自证，因此本库无法产生一致率样本；该指标仍由隔离库合成数据覆盖。
  - `check-invariants`：`totalViolations = 0`，I8 = 0、I11 = 0；其余检查因 T1 表未建立而不可用。
- 遗留：独立审查与合并授权；共享/生产库仍需单独授权后才能执行迁移；真实重采集需再次授权，并继续遵守 D-028 的频率、风险冷却与并发上限。C4 第 2 轮可不再以拿到毫秒 epoch 为前置条件。

## 2026-11-13 C4 真实重采集（第 2 轮，日精度）

- 状态：真实采集完成；三个领域各 1 批，日精度平台时间首次在真实回采中按预期落库。未修改代码、未新增迁移。
- 授权与前置：负责人授权真实重采集（沿用 C4 第 1 轮授权范围）。批次按领域轮流、串行执行：前一批租约释放后才提交下一批，未提高并发，未绕过频率、风险冷却或授权。插件 `0.8.66` 在批次开始时心跳正常；三批结束时 `collection_installation_risk_signal` 0 条、`collection_installation_risk_cooldown` 0 行、活跃 `collection_risk_pause` 0 行。
- 作品选择：最近 12 周内发布、领域 `primary` 用途、近 7 天未采过评论、未退役，按 `comment_count` 从高到低；排除第 1 轮两个 `input_blocked` 作品（`230037d0-cea7-4fe2-b0b2-6ca50dc5e79e`、`4ce71ef6-07b4-4d89-9826-19db7db132b8`）。每批 4 个作品，`commentLimit = 30`、`replyExpandLimit = 2`、媒体/OCR/ASR 关闭。
- 批次结果（数据库实测；请求与工单均为实测引用）：
  - ADHD（`request 2d51b359-c632-4db6-9728-373447f15099`，`work_order 2ebfb17f-07f0-4f91-8168-8f663a53462f`）：4 个作品，3 个采集成功。`content_detail` 3 包、`comments` 3 包 30 条记录、`replies` 3 包 28 条记录；12 个任务 9 完成、3 `blocked`；租约以 `partial` 释放。失败 1 个作品 `00685f28-07d8-40fb-b8b2-f8a46f6f83e6`：`detail_page_url_invalid`（`blocked`，预尝试重试预算耗尽），不是输入闸拒绝，也不是限流或冷却。
  - 考研自习（`request 7f9e6e40-971c-4290-8477-48212b6f8001`，`work_order 613e15fc-954e-416b-94d7-66fec1f09b5c`）：4/4 作品；`content_detail` 4 包、`comments` 4 包 33 条、`replies` 4 包 18 条；12/12 任务完成；派发失败 0。
  - 自闭症干预（`request e2d12724-4eda-45b2-8e87-ef26bd0103ca`，`work_order 91539666-18ae-4086-acb6-6f5993f8eeef`）：4/4 作品；`content_detail` 4 包、`comments` 4 包 40 条、`replies` 4 包 40 条；12/12 任务完成；派发失败 0。
- 新获得平台时间的评论：
  - 三批共 11 个 `comments` 包、103 条评论记录，全部带 `collectionProtocol.version = 1`、`sourceTier = dom`、`publishedAt = null`、非空展示文本；`replies` 包同样全部带协议 v1。C2 协议在真实回采中生效，毫秒 epoch 仍不可得（与第 1 轮结论一致），展示文本是唯一可用的时间来源。
  - 每批采集后执行 `project-platform-times`：ADHD `inserted = 19`、考研自习 `inserted = 4`、自闭症干预 `inserted = 37`，合计 60 条，`conflicts = 0`。这 60 条是回采中新出现的评论身份，全部解析出日精度时间（60/60），其中 `display_month_day` 54 条、`display_relative` 6 条。
  - 全部平台时间现为 29,163 条：`display_full_date` 6,173、`display_month_day` 16,838、`display_relative` 6,111、`epoch` 41（`precision = millisecond` 仍为 41）。
- 覆盖率（CLI 默认 `as_of`，`--periods 12`，`--tz Asia/Shanghai`，作品发布周口径）：
  - ADHD：7,137 条有效评论，7,136 条有平台时间（99.986%）；唯一缺失仍在 2026-10-05 周，即 `publishedAtText = '2/5'` 的误取记录。
  - 考研自习：162 / 162（100%）。
  - 自闭症干预：309 / 309（100%）。
  - 三领域 `agreement.compared = 0`（现存 epoch 行的展示文本均为 ISO 序列化，不参与核对）、`timingViolations = 0`。
- `check-invariants`：`totalViolations = 0`，I8 = 0、I11 = 0；其余检查仍因 T1 表未建立而不可用。
- 观察：覆盖率的分母按有效评论口径计算，不完全等于“新增评论身份数”（回复、重复身份与周期归属会改变增量）；本轮只按报告实际返回值记录，不把 60 条新增直接换算为覆盖率增量。
- 遗留：`00685f28` 需要单独判断页面是否已失效（可先看前台是否仍可访问），下一轮不再自动优先重试；`agreement` 仍缺少真实一致率样本；独立审查与合并授权；共享/生产库迁移仍需单独授权。

## 2026-11-13 G0 确认与 T1 工作包

- 负责人确认：G0 与 RECON 按现有证据范围放行。T0.7 的确定性证明和当时可执行的不变量结果有效；因后续表尚未建立而不可用的不变量不视为已经通过，后续任务建表后必须验证。
- 工作包：T1.1 评论提及数据、T1.2 问题指标计算。执行者 Codex；基线 `843e75cc`；T1.1 分支 `codex/opportunity-t11-comment-mention`，worktree `.worktrees/opportunity-t11-comment-mention`。代码与共享装配文件由本工作包独占修改；其他工作树、共享主工作树、本地运行库和生产环境不在写入范围。
- 负责人要求当前代码逻辑一致，不建立两套并行问题身份；采用已存在的 UUID 问题身份，分类版本历史须满足 R12，见 D-035。T1.2 依赖 T1.1；遇到新的规格与现实冲突按 §3.3 停止并询问。
- 授权与边界：可在一次性隔离测试库中执行测试迁移；真实采集、模型外发、本地运行库及生产环境迁移、生产写入和不可逆处置仍需单独明确授权。不得提高采集并发或绕过现有控制。
- Q-16 已答复：按 D-036 从 0177 归属独立计算源侧人数并对账，另列机会卡资格排除及发布门槛差异，不新增持久业务计数。
- Q-17 已答复：按 D-037 以前向迁移修复评论平台时间的 `as_of` 选择；当前视图与资格函数共享选择逻辑。
- Q-18 已答复：按 D-038 使用首次入档包的协议，协议缺失保持未知，指标和基线按协议分组。
- Q-19 已答复：按 D-039 共用按时点事实选择逻辑，历史计算以同一密钥计算该时点身份哈希；首次入档表缺少建立时间的限制明确记录。
- Q-20 已答复：按 D-040 前向修正共享作品作者函数及作品发布时间锚点，补充跨模块回归。
- Q-21 已答复：按 D-041 先选最新合格详情，再检查发布时间；缺失时记 `period_anchor_missing`。
- 当前状态：T1.1、T1.2 已实现并通过隔离验证；D-037、D-039、D-040、D-041 的前置修复随 T1.2 交付。0211–0213 已登记在工作分支的迁移装配文件中；未在本地运行库或生产环境执行。

## 2026-11-13 T1.1 评论提及数据（候选）

- 内容：从 0177 现有归属投影稳定评论与 UUID 问题；记录分类、指标、模型和提示词版本及来源；对每个问题比较源侧与投影侧去重人数，并单列资格和发布门槛差异。已有归属不调用模型；新归属在现有评论研究发布后由同一投影逻辑同步。开关关闭或密钥缺失时不启动机会卡写入。
- 补充保护：重复同步不新增重复行；身份信息更新时更新派生哈希；同版本密钥与既有哈希不匹配时拒绝写入。源与投影集合一致但观察身份冲突时，自动路径仍记录冲突计数。分类版本变更记录可追溯时点，不伪造此前历史。
- 执行 `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target bash scripts/test-opportunity-postgres.sh`：基础 12 passed，提及专项 4 passed；实际 0177 发布链路使用本机回环合成服务，开启、密钥恢复、关闭、持锁后追平、持表锁时排空五种配置各 1 passed；末尾输出 `Opportunity PostgreSQL proof passed`，并确认临时库、容器和卷已清除。I6、I6b、I8、I11 可用且均为 0；其余依赖尚未建立的表，不记为通过（`incomplete = true`）。
- 执行 `bash scripts/test-local-runtime-schema.sh`：`Fresh and repeat runtime schema migration passed; retired tables absent.` 使用脚本自建的隔离库，并清理其资源。
- 执行 `bash scripts/ci-preflight.sh origin/main`：52 passed，`Preflight passed`；`node --test scripts/ci-select.test.mjs`：21 passed。改动 Rust 文件执行 `rustfmt --edition 2024 --config skip_children=true --check`：退出码 0、无输出。
- 基线 `843e75cc` 与候选分别执行 `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target cargo clippy -p linggan-intelligence -p linggan-worker --all-targets --locked`：均退出 101，58 项 `too_many_lines` 错误的位置和内容相同（首次均在既有 `linggan-evidence`）；候选加 `-- -A clippy::too_many_lines` 后完成，退出码 0。原始严格 Clippy 未通过，不能称为通过。
- `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target cargo test -p linggan-worker --bin linggan-opportunity --locked`：1 passed，0 failed。
- 独立审阅完成：此前提出的自动同步、幂等与身份变更、CI 覆盖、分类时间、主调度阻塞、密钥恢复和身份冲突日志问题均已处理；最终增量复核未发现新的阻断问题。审阅者核对代码和测试报告，未独立重跑数据库测试。记录此候选时远端 CI、合并和部署尚未执行；后续 CI 与合并回执见下方「T1.1 / T1.2 合并与 T1.3 待决」。只使用合成数据与隔离测试数据库；未对本地运行库或生产环境执行迁移，也未进行真实采集或模型外发。

## 2026-11-13 T1.2 问题指标计算

- 工作分支 `codex/opportunity-t12-problem-metrics`，基于 T1.1 提交 `67ebfb57`；工作树 `.worktrees/opportunity-t12-problem-metrics`。前向迁移 0212 修复 D-037、D-039、D-040、D-041 的时间选择；0213 建立协议维度指标、影子版本关联与全局密钥校验标记，已登记装配和 checksum。旧迁移字节不变。
- 周期计算从口径定义读取窗口和阈值；同协议单独计算分子、分母、Wilson 区间、基线和累计人数。缺少评论者身份保留在分母，未知协议单独成组；空分母输出空区间和原因，基线不足不判显著。输入未变的问题跳过重算；CLI 支持 `--full` 和不写指标表的 `--preview`。未来时点和用旧时点覆盖新结果均明确拒绝。
- 已启用的现有后台任务按领域串行先同步提及，再计算当前与影子指标；已有归属发布后触发，其他事实变化按现有 `card_rebuild_debounce_seconds` 检查。一个领域的源数据或时点不变量失败不阻止其他领域；每次报告排除、身份冲突和可用的不变量。关闭开关时不启动。首次建立密钥标记前先全库核对旧哈希；无法验证时拒绝，不认领未知密钥。
- 独立审阅发现的影子版本编号占用、缺失影子、未来时点、历史分类不可用、身份冲突、排除零项、时点不变量、历史预览、跨领域故障及密钥一致性均已修正。最终复核未发现新的明确阻断缺陷；审阅者未独立运行数据库测试。
- 运行 `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target bash /private/tmp/opportunity-t12-logic-proof.sh`：提及 6 passed、时点 3 passed、指标 2 passed；输出 `Opportunity targeted proof passed`，并确认隔离库、容器和卷已清除。定向自动链路另执行 `/private/tmp/opportunity-t12-enabled-proof.sh`：1 passed，覆盖一个领域 I11 违例而健康领域仍更新、无新归属的身份与限制变化。
- 完整隔离回归 `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target bash scripts/test-opportunity-postgres.sh`：基础 12 passed、提及 6 passed、时点 3 passed、指标 2 passed、共享作者调用 1 passed、实际 0177 合成发布链路五种配置各 1 passed；输出 `Opportunity PostgreSQL proof passed` 与隔离资源清除确认。此后只提取发布测试辅助函数，再执行 `/private/tmp/opportunity-t12-enabled-proof.sh`，1 passed。
- `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target cargo check -p linggan-worker --bin linggan-opportunity --locked`：`Finished dev profile`，退出码 0；`cargo test -p linggan-intelligence --lib --locked`：371 passed、0 failed、12 ignored；`cargo test -p linggan-worker --bin linggan-opportunity --locked`：2 passed、0 failed。只对改动 Rust 文件运行 `rustfmt --edition 2024 --config skip_children=true --check`，退出码 0、无输出。
- `bash scripts/test-local-runtime-schema.sh`：`Fresh and repeat runtime schema migration passed; retired tables absent.`；`bash scripts/ci-preflight.sh origin/main`：52 passed、`Preflight passed`。脚本自建隔离资源均已清除。
- 严格 `cargo clippy -p linggan-intelligence -p linggan-worker --all-targets --locked` 退出 101：基线与候选均是既有 `linggan-evidence` 的 58 项 `too_many_lines`，位置和内容相同；加 `-- -A clippy::too_many_lines` 后完成，退出码 0。使用 `-- --cap-lints warn` 复核并拆分本次新增长函数；既有大型评论研究集成测试及其他仓库警告仍在，不声称严格 Clippy 全通过。
- I1、I2、I3、I6、I6b、I8、I11 的合成场景结果均为 0；未建表或未实现时点检查的项目标为不可用，报告 `incomplete = true`。历史测试另证明未来才出现的 I11 违例不污染旧时点，已存在于查询时点的 I11 违例会拒绝计算并回滚。
- 已知限制：首次入档表没有建立时间；早于可证明的分类/问题登记或口径定义历史时返回不可用。按领域切换覆盖率与全局 singleton 当前指针不一致，切换任务开始前须按 §3.3 另行裁定。当前为实现并经隔离测试，不代表已合并、部署或获得业务验收；远端 CI 以实际运行结果为准。
- 本任务超出手册建议的单 PR 代码行数：时点选择、协议分组、版本演进、同一事务的不变量与自动刷新必须共同接入并一起验证；拆开会留下无法满足本任务口径的中间结果。

## 2026-11-13 T1.1 / T1.2 合并与 T1.3 待决

- 负责人指示继续推进上一轮建议的下一步。T1.1 [PR #580](https://github.com/Himog0921/linggan-intelligence/pull/580) 以最终 head `67ebfb57` 合并，main 合并提交 `4db4f0c263508187691e314cd437567408769514`；转为 Ready 后的 [Delivery 43467742630](https://github.com/Himog0921/linggan-intelligence/actions/runs/43467742630) 的所有所选任务和 gate 通过。
- T1.2 [PR #581](https://github.com/Himog0921/linggan-intelligence/pull/581) 在 #580 合并后改为以 main 为目标，最终 head `fce43b75` 合并，main 合并提交 `263435f38332f92f49200627bde67c6d875292bb`；重设基线后的 [Delivery 43471357125](https://github.com/Himog0921/linggan-intelligence/actions/runs/43471357125) 的所有所选任务和 gate 通过。合并后的 main 与该 head 的文件差异为 0。
- 执行 `git fetch origin main`，输出 `843e75cc..263435f3  main -> origin/main`；执行 `git worktree add -b codex/opportunity-t13-creator-density .worktrees/opportunity-t13-creator-density origin/main`，输出 `HEAD is now at 263435f3`。共享主工作树的原有未提交改动保持不动。
- T1.3 工作包：执行者 Codex；分支 `codex/opportunity-t13-creator-density`；worktree `.worktrees/opportunity-t13-creator-density`；基线 `263435f38332f92f49200627bde67c6d875292bb`。只使用隔离测试数据库；本地运行库及生产环境迁移、真实采集和模型外发仍需另行授权。
- 状态：**blocked**。D-030 的协议分组与作者集中度表和卡片排序之间缺少规则；事实、选项和建议记录在 Q-22，已向负责人询问。答复前不写作者集中度代码、不修改业务规格。
- 本次文档核对执行 `bash scripts/ci-preflight.sh origin/main`：`tests 52`、`pass 52`、`fail 0`，输出 `Preflight passed; required module proofs and independent review remain separate.`；`git diff --check` 退出码 0。
- 仅合并代码，未部署，也未对本地运行库或生产环境执行迁移；严格 Clippy 的既有基线失败仍按 T1.1 / T1.2 原记录保留。

## 2026-11-13 T1.3 作者集中度工作包

- 负责人答复 Q-22，采用 D-042：按首次入档协议分别计算、存储和组内排序；未知协议单列，后续卡片标明协议且不跨组比较。T1.3 恢复实施。
- 代码范围：作者集中度的前向迁移、共享输入与计算、现有调度接入、隔离 PostgreSQL 与排序验证；继续复用机会卡现有评论资格、按时点身份、问题归属与 Wilson 函数。新迁移同步登记运行脚本及完整 schema fixture。
- 本地运行库和生产环境迁移、真实采集、模型外发仍需另行授权。首次入档记录无创建时间、旧时点不可证明的限制继续保留。

## 2026-11-13 T1.3 作者集中度（候选）

- 前向迁移 `0214_opportunity_creator_problem_density.sql` 建立带协议维度的作者指标表；未知协议为 NULL，唯一约束将未知协议视为同一组。迁移 SHA-256 为 `b8a6a54f6de2edff554d7f4cb633150fdb249c331a160ba2783f794fcc332396`，已同步运行脚本与完整 schema fixture；未执行运行环境迁移。
- 作者指标与问题周期指标共用按时点资格、协议、评论者哈希、问题归属及同一重算事务；作者窗口始终取截至 `as_of` 的作品发布时间。每协议分别累计有效评论和去重提及人数；未知协议单列，组内先区分样本充足与不足，再按手册排序。现有后台刷新、`--full`、增量跳过与不写表的 `--preview` 同步覆盖作者指标；缺少作品发布时间与窗口外样本单独计数。
- 共享 Wilson 函数将零命中下界和全命中上界固定为数学上的 0 和 1，防止浮点残差影响作者并列排序；手册全部原标准答案未更改。I7 已实现按领域和时点检查。
- 标准样本直接重分配既有 W0 评论，在同一隔离库同时验证有效评论 5800、P1/P2/P3 标准答案以及 C1 `k=4 n=60 ci=[0.026229,0.159254] sufficient=true`、C2 `k=3 n=40 ci=[0.025836,0.198642] sufficient=false`。协议已知/未知、跨作品同人去重、未知评论者、撤回、窗口上下边界、历史预览以及增量与全量一致另有独立场景。
- `CARGO_TARGET_DIR=/private/tmp/codex-opportunity-card-t1-01a1d4e2-target bash scripts/test-opportunity-postgres.sh`：基础 12、提及 6、时点 3、问题与作者指标 4、共享作者 1 项通过；实际 0177 合成发布链路的 enabled、missing_key、disabled、locked_release、locked_drain 五种状态各 1 项通过。末尾输出 `Opportunity PostgreSQL proof passed` 与 `Opportunity PostgreSQL proof cleanup verified; isolated proof database, container, and volume were removed`。enabled 证明作者指标跟随资格和撤回刷新；disabled 使用有资格素材，后台跨扫描周期后仍无作者指标写入。
- `cargo test -p linggan-intelligence --lib --locked`：373 passed、0 failed、12 ignored；`cargo test -p linggan-worker --bin linggan-opportunity --locked`：2 passed、0 failed。`bash scripts/test-local-runtime-schema.sh`：`Fresh and repeat runtime schema migration passed; retired tables absent.`；隔离资源均清除。
- `check-invariants`：I1、I2、I3、I6、I6b、I7、I8、I11 可用且各为 0；I4、I5、I9、I10 尚不可用，报告 `incomplete = true`。I7 的人工制造违规在时点预览中被拒绝并回滚。
- 严格 `cargo clippy -p linggan-intelligence -p linggan-worker --all-targets --locked` 退出 101；与基线逐项比较均为既有 `linggan-evidence` 的 58 项 `too_many_lines`，位置及内容相同。加 `-- -A clippy::too_many_lines` 后退出 0；`-- --cap-lints warn` 用于复核新增长函数，既有长测试仍有告警，不声称严格 Clippy 通过。
- 工作区独立审阅提出标准集和关闭开关负例不足，均已修复并经隔离回归；审阅者只读核对代码与日志，未独立运行数据库测试。最终提交和远端 CI 以 PR 回执为准。
- 本任务单 PR 的代码改动超过手册建议的约 400 行：需要把作者指标与现有输入、双口径重算及事务不变量一起接入，并拆分原有长函数；隔离数据与负例测试另占较大部分。拆成并行实现会使中间版本缺少一致的自动刷新和校验。
- 结果为实现与合成测试候选；未部署、未做真实数据或业务验收。本地运行库及生产环境迁移仍需另行授权；首次入档表没有建立时间的历史限制仍在。
