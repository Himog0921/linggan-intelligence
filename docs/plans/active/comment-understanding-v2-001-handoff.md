# COMMENT-UNDERSTANDING-V2-001 · 交接：从 G2 继续

> 状态: 活跃计划
> 最后核对: 2026-11-19
> 适用范围: 接手评论研究 V2 在同一 PR、同一分支上的剩余代码工作（P0、G2–G6、切换）
> 事实来源: [实施计划](comment-understanding-v2-001.md)（含附录 A 改动清单、附录 B G1 记录）、[评论理解合同](../../data-contracts/comment-understanding.md)、分支 `claude/comment-understanding-v2` 的代码与测试
> 冲突时以谁为准: Mog 最新明确授权、AGENTS.md、真实运行/数据库副作用、当前代码/migration/测试；本文只是入口，不复制计划

## 1. 从哪里开始

1. 目录与分支：`.worktrees/claude-comment-understanding-v2-20261119`，分支 `claude/comment-understanding-v2`，基线 `origin/main@8891d718`。所有工作在这个 worktree 里做，向这个分支推送，更新同一个 PR（[#621](https://github.com/Himog0921/linggan-intelligence/pull/621)，Draft；候选完成并审阅后再转 Ready）。
2. 读取顺序：`AGENTS.md` → [协作协议](../../governance/agent-collaboration.md) → [安全与领域约束](../../agents/safety-invariants.md) → 计划第 2、3、5、6、8、9、10 节 → 评论理解合同 → `database/migrations/0230_comment_understanding.sql` → `crates/intelligence/src/ai_foundation/comment_understanding/`。
3. 计划附录 A.2 是 G2 起的改动清单（文件和函数），附录 B.2 是 G1 对计划的修正。

## 2. 当前状态

- **G0、G1 已实施并通过隔离 PostgreSQL 测试**；结果见计划附录 B.3、B.4。
- 迁移 `0230` **只在隔离测试库应用过**，没有应用到本机运行库（`55432/linggan_intelligence_dev`，迁移头 `0229`），也没有部署。
- `comment_understanding` 任务已注册但停用、未配置模型和预算、**不能入队**（`BatchKind::permits` 对它返回 false）。
- 旧三阶段评论研究仍是运行中的流程，代码和测试都在。
- 模块 `comment_understanding` 只有合同和接纳器（纯函数），**没有任何调用方**，所以接线前不会产生行为变化。

## 3. 保护车道 Claim

| 项 | 内容 |
|---|---|
| 工作包 | COMMENT-UNDERSTANDING-V2-001（Mog 2026-11-19 确认，无 Issue） |
| base / 分支 / worktree | `8891d718` / `claude/comment-understanding-v2` / `.worktrees/claude-comment-understanding-v2-20261119` |
| exclusive | 计划附录 A.2 列出的评论研究文件、迁移 `0230` 及其后续迁移、`comment_understanding` 模块、对应测试与文档 |
| 共享，改前先看并行改动 | `topic_map_research`、`topic_comment_source`、`opportunity`、`comment_study_catalog` 中读取评论研究结果的查询（G2/G3 必须改，按附录 A.3 的 3 点保持兼容） |
| forbidden | 其他工作包的文件；运行库写入；部署 |
| 升级条件 | 见第 7 节 |

## 4. 已确认的决定，不要重开

L1 是评论研究必需上下文，不降级；同一领域串行；候选整理自动执行、合并可撤销；跨模型抽查由 Mog 上线后事后进行；**先完整落地上线，再优化质量**（上线门槛是程序不变量和端到端运行，不是质量指标）；P0 和 V2 沿用 L1 的 `model_config_ref`（`deepseek-flash`，thinking high，输入 64k、输出 40k）；旧流程直接退役，不写兼容层；判定"是不是同一需求"交给 LLM，向量只做宽松排序，默认路径不用向量（计划 §1.3 有实验数字）；拿不准就新建需求；候选/正式不存储，视图推导。

## 5. 剩余工作与顺序

| 阶段 | 内容 | 入口 |
|---|---|---|
| P0 | 提示词冒烟测试，与 G2 并行；**需要 Mog 先授权真实调用**（第 7 节） | 计划 §8 P0 |
| G2 | T0 规划、T1 单元调用、T2 接纳与写入、全量目录模式 | 计划 §5.2–5.4、附录 A.2 |
| G3 | T3 候选整理（含它的任务注册、派发表、输入引用）、Opportunity 改读视图、T4 刷新 | 计划 §5.5–5.6 |
| G5 | 前端 | 计划 §8 G5 |
| 切换 | 迁移退役旧三个任务、删旧 stage 表和旧代码、改写 `data-contracts/research-automation.md` 等文档 | 计划 §4.2 末段 |
| G4 / G6 | 向量（只在满足进入条件时）/ 上线后的评估 | 计划 §8 |

**G2 的具体落点（G1 阶段查到的）：**

- `ai_foundation/batch_store.rs`：给 `InputRef` 增加 `CommentResearchUnit { unit_ref }`，同步 `InputRef::matches` 与 `BatchKind::permits`（V2 只允许 `interactive`）。批次输入引用是 JSON，上限 4096 字节。沿用现有做法：先生成 `unit_ref`，插入批次，再按 `item_key` 找到 `item_ref` 写入 `linggan_comment_research_unit`。
- `ai_foundation/comment_research.rs`：`start` 现在为每条评论建 3 个批次；V2 改为一个批次、每个单元一个 item。`frozen_in` 要按计划 §2 改写：只校验评论原文是否仍可用（含 `linggan_material_comment_restriction`），L1 版本只记录来源，不作为有效性条件。
- `ai_foundation/executor/service.rs`、`executor/queue.rs`：分发 `CommentUnderstanding`；领取时加"同领域已有 running 的单元则不领取"。
- `executor/service/comment_research.rs`、`research_call.rs`：按 `comment_understanding::payload()` 组装载荷，`response_schema()` 作响应 Schema，`material_class` 为 `comment_material`；回写时用 `accept_unit()`，结果行用 `AcceptedTarget::result_json()`，需求和支持关系在**同一事务**里写入。
- 作品语境改为 L1：`content_understanding::core_cards_in(tx, domain, work, cutoff)`，冻结其 `analysisRef`（L1 请求 ID）到 `unit.l1_request_ref`。

## 6. 容易踩的坑

- **"按批次 item 是否成功判断评论成功"的 SQL 有 4 处**，V2 下一个批次含多个单元，必须改为"该评论请求是否有成功的结果行"：`topic_map_research/comment_signals.sql`、`ai_foundation/topic_comment_source/read.sql`、`ai_foundation/topic_research_automation/comment_candidates.sql`、`comment_study_catalog/facts.sql`（还读旧 stage 表）。漏改不会报错，只会让 Topic Map 把失败的评论当成功读走。
- **Opportunity 会静默漏掉 V2 支持**：`opportunity/mentions.rs` 约 137 行用 `membership.review_invocation_ref` 做内连接，V2 的支持行该列为空（用 `origin_invocation_ref`）。G3 必须改读 `linggan_comment_problem_support`，并核对 `linggan_opportunity_comment_mention` 对 `membership(request_ref, signal_ordinal)` 的外键和 `linggan_opportunity_sync_problems` 函数。
- **信号序号**是信号在已存储 `signals[]` 里的下标（0–11），与 `membership.signal_ordinal` 一致；丢弃信号后序号以接纳后的数组为准。
- **迁移不可改**：`0230` 一旦在任何共享环境应用就不能再改（有校验和账本，在 `scripts/local-runtime.sh` 和 `apps/api/src/local_web/full_schema_fixture.rs`）。在那之前改动 `0230` 要同步更新夹具里的校验和。提示词 v1 是草案，改提示词用**新的提示词版本**，不要改迁移。
- **迁移编号**：合并 `main` 前核对 `ls database/migrations`，`main` 上可能出现同号迁移（历史上发生过）。切换迁移计划编号 `0231`，以届时实际为准。
- **别名**：模型只看到 `p<n>`、`t<n>`，T2 靠 `unit_dispatch.catalog_problem_refs`（有序）还原；派发时必须在调用前写入，不能事后重建。
- 别在 `main` 的共享 checkout 里工作；那里有他人未提交的改动。

## 7. 必须回到 Mog 的事项

真实模型调用（P0 以及任何会外发评论的运行）；发给第三方模型前的目的、最小样本、输出边界和处置计划（评论含 ADHD、儿童内容，见 safety-invariants）；启用 `comment_understanding`、配置预算、绑定模型；向本机运行库应用迁移或重启运行时；合并和部署（保护车道的合并授权针对**最终 head**）；改变已确认的决定或产品语义。创建外部 Issue、安排独立审阅子代理，同样按协作协议由用户授权。

## 8. 验证配方

- 隔离 PostgreSQL：用 `scripts/test-comment-study-productization-postgres.sh`（CI 会跑它，新测试已加入）。它启随机容器和卷，用后清理。**不要对 `linggan-intelligence-postgres-1`（端口 55432，运行库）做写入**，只允许 `SET default_transaction_read_only=on` 的只读查询。
- 在新 worktree 里跑依赖 Node SDK 的测试：需要 `apps/pi-adapter/node_modules`。`package-lock.json` 与主检出相同时，可以临时符号链接主检出的目录，或运行 `scripts/runtime/prepare-pi-adapter.sh --install`。用完删除符号链接：`.gitignore` 只忽略目录，符号链接会显示为未跟踪文件，别提交。
- `ai_foundation_postgres` 还需要环境变量 `AI_FOUNDATION_PROOF_NODE`（`node` 路径）和 `LINGGAN_CREATION_ASSET_ROOT`（一个自己创建的临时目录）；机会卡测试需要 `LINGGAN_SUPPORT_DIR` 下有 `opportunity/commenter-digest.key`（`openssl rand 32` 生成，权限 600）。缺这些时的失败是环境问题。
- 已知与本包无关的失败：`linggan-api` 的 `creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work`（503），在基线 `8891d718` 上同样失败。
- 提交前：`bash scripts/ci-preflight.sh origin/main`；新增文件用 `rustfmt --edition 2024` 单独格式化（`cargo fmt --check` 会报仓库里原有文件的差异，别顺手改）。

## 9. 每个阶段的交付要求

候选完成后安排一次独立审阅并关联实际提交；更新 PR 正文里的验证、未验证事项和审阅记录；文档按事实变化同步（合同改变更新 `data-contracts/comment-understanding.md`，切换时改写 `research-automation.md` 并在 `docs/progress/2026-11.md` 记里程碑）。已实现、已测试、已合并、已部署、生产已验证分开写，不互相推断。
