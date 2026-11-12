# PROGRESS

> 状态: 活跃计划
> 最后核对: 2026-11-12
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
